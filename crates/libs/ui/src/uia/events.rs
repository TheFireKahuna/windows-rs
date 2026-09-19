//! One queue of what a tick changed, folded, and raised at the end of the tick.
//!
//! A flush runs on the front thread after the publish, so the snapshot is consistent when a
//! client reads it back and no raise re-enters a client's callback in the middle of an input
//! handler.
//!
//! A row carries only where the value came *from*. Where it went is read from the snapshot at
//! raise time, which is after every write this tick, so a burst of pointer samples folds by
//! dropping every row after the first for that element and property: the one that survives
//! reports the whole burst, from where it started to where it ended. Automation compares the
//! two values, and a raise whose values are both empty describes a change from nothing to
//! nothing and reaches no listener.

use super::provider::{Shared, packed};
use super::snapshot::{ColFlags, State, Tree};
use super::variant;
use crate::VARIANT;
use crate::bindings::{
    StructureChangeType_ChildrenBulkAdded, UIA_AutomationFocusChangedEventId,
    UIA_ExpandCollapseExpandCollapseStatePropertyId, UIA_Invoke_InvokedEventId,
    UIA_LiveRegionChangedEventId, UIA_MenuClosedEventId, UIA_MenuOpenedEventId,
    UIA_RangeValueValuePropertyId, UIA_SelectionItemIsSelectedPropertyId,
    UIA_Text_TextChangedEventId, UIA_Text_TextSelectionChangedEventId,
    UIA_ToggleToggleStatePropertyId, UIA_ToolTipOpenedEventId, UIA_ValueValuePropertyId,
    UiaClientsAreListening, UiaRaiseAutomationEvent, UiaRaiseAutomationPropertyChangedEvent,
    UiaRaiseStructureChangedEvent,
};
use std::sync::Arc;
use windows_core::Interface;
use windows_scene::ControlId;

/// What a row reports as the property that changed.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Property {
    /// `RangeValue.Value`, as a double.
    Range,
    /// `Toggle.ToggleState`, as the enumeration's integer.
    Toggle,
    /// `SelectionItem.IsSelected`, as a boolean.
    Selected,
    /// `ExpandCollapse.ExpandCollapseState`, as the enumeration's integer.
    Expanded,
    /// `Value.Value`, as the string an editable document now holds.
    Text,
}

/// One event a tick has to raise.
///
/// A plain event names an element and an automation event id. A property change names the
/// property and what it changed **from**; what it changed to is read at raise time, which is
/// after every write this tick.
#[derive(Clone, PartialEq, Debug)]
pub enum Raise {
    /// The tree was replaced. One event for the whole change, not one per element.
    Structure,
    Event(ControlId, i32),
    Property(ControlId, Property, Val),
}

impl Raise {
    pub const fn focus(id: ControlId) -> Self {
        Self::Event(id, UIA_AutomationFocusChangedEventId)
    }
    pub const fn invoked(id: ControlId) -> Self {
        Self::Event(id, UIA_Invoke_InvokedEventId)
    }
    pub const fn live(id: ControlId) -> Self {
        Self::Event(id, UIA_LiveRegionChangedEventId)
    }
    pub const fn menu_opened(id: ControlId) -> Self {
        Self::Event(id, UIA_MenuOpenedEventId)
    }
    pub const fn menu_closed(id: ControlId) -> Self {
        Self::Event(id, UIA_MenuClosedEventId)
    }
    pub const fn tooltip_opened(id: ControlId) -> Self {
        Self::Event(id, UIA_ToolTipOpenedEventId)
    }
    pub const fn selection_changed(id: ControlId) -> Self {
        Self::Event(id, UIA_Text_TextSelectionChangedEventId)
    }
    pub const fn text_changed(id: ControlId) -> Self {
        Self::Event(id, UIA_Text_TextChangedEventId)
    }

    /// Returns what this row folds on: the element, the property where it is a property
    /// change, and the event id where it is a plain event.
    fn key(&self) -> (ControlId, Option<Property>, i32) {
        match *self {
            Self::Structure => (ControlId::NONE, None, 0),
            Self::Event(id, event) => (id, None, event),
            Self::Property(id, what, _) => (id, Some(what), 0),
        }
    }
}

/// A value in the variant type automation expects for its property.
#[derive(Clone, PartialEq, Debug)]
pub enum Val {
    Empty,
    Number(f64),
    Int(i32),
    Bool(bool),
    /// UTF-16 already, because that is what the snapshot holds and what a `BSTR` takes.
    Text(Arc<[u16]>),
}

impl Val {
    fn variant(&self) -> VARIANT {
        match *self {
            Self::Empty => variant::empty(),
            Self::Number(v) => variant::r8(v),
            Self::Int(v) => variant::i4(v),
            Self::Bool(v) => variant::bool(v),
            Self::Text(ref v) => variant::wide(v),
        }
    }
}

/// Returns what `what` now reads as on the element at `at`.
fn now(what: Property, tree: &Tree, id: ControlId, at: u16) -> Val {
    let state = tree.state(at);
    match what {
        Property::Range => tree.value(at).map_or(Val::Empty, Val::Number),
        Property::Toggle => Val::Int(i32::from(state.has(State::TOGGLED))),
        Property::Selected => Val::Bool(state.has(State::SELECTED)),
        Property::Expanded => Val::Int(i32::from(state.has(State::EXPANDED))),
        Property::Text => tree
            .field(id)
            .map_or(Val::Empty, |field| Val::Text(Arc::clone(&field.text))),
    }
}

/// Returns the automation property id `what` is raised under.
const fn property_id(what: Property) -> i32 {
    match what {
        Property::Range => UIA_RangeValueValuePropertyId,
        Property::Toggle => UIA_ToggleToggleStatePropertyId,
        Property::Selected => UIA_SelectionItemIsSelectedPropertyId,
        Property::Expanded => UIA_ExpandCollapseExpandCollapseStatePropertyId,
        Property::Text => UIA_ValueValuePropertyId,
    }
}

/// The events queued for the current tick.
///
/// Bounded by the number of elements that changed in it. The allocation is kept across ticks,
/// so a steady drag allocates nothing.
#[derive(Default)]
pub struct Pending(Vec<Raise>);

impl Pending {
    /// Records `raise`, dropping it where the queue already names the same element and the
    /// same property or event.
    pub fn push(&mut self, raise: Raise) {
        let key = raise.key();
        if self.0.iter().any(|queued| queued.key() == key) {
            return;
        }
        self.0.push(raise);
    }

    /// Returns whether nothing is queued.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Raises every queued event against the current snapshot, then empties the queue.
    ///
    /// An element that has unmounted since its row was recorded resolves to nothing and is
    /// skipped rather than failing the flush. Nothing is raised when no client is
    /// [`listening`], and the queue is emptied either way.
    pub fn flush(&mut self, shared: &Arc<Shared>, tree: &Tree) {
        if listening() {
            for raise in self.0.drain(..) {
                one(&raise, shared, tree);
            }
        }
        self.0.clear();
    }

    /// Moves the queued rows into `out` without raising them.
    #[cfg(test)]
    pub fn take(&mut self, out: &mut Vec<Raise>) {
        out.append(&mut self.0);
    }
}

/// Raises one queued row.
fn one(raise: &Raise, shared: &Arc<Shared>, tree: &Tree) {
    let (id, _, event) = raise.key();
    let Some(provider) = super::provider::provider_for(shared, id) else {
        return;
    };
    match *raise {
        // Raised on the fragment root as a bulk change: the table is replaced wholesale, so
        // there is no per-element diff to describe. A null runtime id of length zero names the
        // root itself, which is the form a bulk change takes.
        //
        // SAFETY: `provider` is a provider object alive for the call, and the null runtime id
        // is the documented argument for a change that names no one element.
        Raise::Structure => unsafe {
            _ = UiaRaiseStructureChangedEvent(
                provider.as_raw(),
                StructureChangeType_ChildrenBulkAdded,
                core::ptr::null_mut(),
                0,
            );
        },
        // SAFETY: `provider` is a provider object alive for the call, and `event` is one of
        // the event id constants the constructors above write.
        Raise::Event(..) => unsafe {
            _ = UiaRaiseAutomationEvent(provider.as_raw(), event);
        },
        Raise::Property(_, what, ref was) => {
            let Some(at) = tree.index_of(id) else {
                return;
            };
            // SAFETY: `provider` is alive for the call, both variants carry the type this
            // property is reported as, and each owns whatever allocation it holds until the
            // call returns, which is where they are dropped.
            unsafe {
                _ = UiaRaiseAutomationPropertyChangedEvent(
                    provider.as_raw(),
                    property_id(what),
                    was.variant(),
                    now(what, tree, id, at).variant(),
                );
            }
        }
    }
}

/// Returns whether a raised event could reach a client.
///
/// A hint rather than a guarantee: it answers `true` on a desktop with no client attached, so
/// it gates only the cost of raising and nothing structural.
#[must_use]
pub fn listening() -> bool {
    // SAFETY: the call takes no arguments, reads no caller state, and is callable from any
    // thread.
    unsafe { UiaClientsAreListening().as_bool() }
}

/// Returns whether the element at `at` is a live region, and so owes an announcement.
#[must_use]
pub fn is_live(tree: &Tree, at: u16) -> bool {
    tree.at(at).is_some_and(|entry| {
        entry.flags.has(ColFlags::LIVE_POLITE) || entry.flags.has(ColFlags::LIVE_ASSERTIVE)
    })
}

/// Returns the focused control as a packed id, or `u64::MAX` where nothing holds focus.
#[must_use]
pub fn packed_focus(id: Option<ControlId>) -> u64 {
    id.map_or(u64::MAX, packed)
}
