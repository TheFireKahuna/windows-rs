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
use crate::bindings::*;
use std::sync::{Arc, Mutex, PoisonError};
use windows_core::Interface;
use windows_scene::ControlId;

/// What a row reports as the property that changed.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Property {
    Native(i32),
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
    Structure(ControlId, StructureChangeType),
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
            Self::Structure(id, _) => (id, None, UIA_StructureChangedEventId),
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
    Rect([f64; 4]),
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
            Self::Rect(v) => variant::rect_property(&v),
            Self::Text(ref v) => variant::wide(v),
        }
    }
}

/// Returns what `what` now reads as on the element at `at`.
fn now(what: Property, tree: &Tree, id: ControlId, at: u16) -> Val {
    let state = tree.state(at);
    match what {
        Property::Native(property) => native(property, tree, at),
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
        Property::Native(property) => property,
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
                if let Raise::Property(id, Property::Selected, Val::Bool(was)) = raise {
                    if let Some(at) = tree.index_of(id) {
                        let selected = tree.state(at).has(State::SELECTED);
                        if selected != was {
                            let event = if selected {
                                UIA_SelectionItem_ElementSelectedEventId
                            } else {
                                UIA_SelectionItem_ElementRemovedFromSelectionEventId
                            };
                            one(&Raise::Event(id, event), shared, tree);
                        }
                    }
                }
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

/// Raises one subscribed event against the published tree.
pub(super) fn one(raise: &Raise, shared: &Arc<Shared>, tree: &Tree) {
    let (id, property, event) = raise.key();
    let advised = if property.is_some() {
        UIA_AutomationPropertyChangedEventId
    } else {
        event
    };
    if !shared.advised.wanted(advised) {
        return;
    }
    let Some(provider) = super::provider::provider_for(shared, id) else {
        return;
    };
    match *raise {
        // SAFETY: bulk and invalidation events identify the provider's children and take no runtime id.
        Raise::Structure(_, kind) => unsafe {
            _ = UiaRaiseStructureChangedEvent(provider.as_raw(), kind, core::ptr::null_mut(), 0);
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
            let value = now(what, tree, id, at);
            if *was == value {
                return;
            }
            // SAFETY: `provider` is alive for the call, both variants carry the type this
            // property is reported as, and each owns whatever allocation it holds until the
            // call returns, which is where they are dropped.
            unsafe {
                _ = UiaRaiseAutomationPropertyChangedEvent(
                    provider.as_raw(),
                    property_id(what),
                    was.variant(),
                    value.variant(),
                );
            }
        }
    }
}

/// What clients have subscribed to, as a count per event id.
///
/// Counts explicit subscriptions independently for each event. An event absent from the
/// table has no subscription information; a zero count suppresses only that event.
#[derive(Default)]
pub struct Advised(Mutex<Vec<(i32, u32)>>);

impl Advised {
    /// Records that a client is listening for `event`.
    pub fn added(&self, event: i32) {
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match held.iter_mut().find(|(id, _)| *id == event) {
            Some((_, count)) => *count += 1,
            None => held.push((event, 1)),
        }
    }

    /// Records that a client has stopped listening for `event`.
    pub fn removed(&self, event: i32) {
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(at) = held.iter().position(|(id, _)| *id == event) {
            held[at].1 = held[at].1.saturating_sub(1);
        }
    }

    /// Returns whether `event` is worth raising.
    #[cfg(test)]
    pub(super) fn wanted_for_test(&self, event: i32) -> bool {
        self.wanted(event)
    }

    /// Returns whether `event` is worth raising.
    fn wanted(&self, event: i32) -> bool {
        let held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        held.iter()
            .find(|&&(id, _)| id == event)
            .is_none_or(|&(_, count)| count > 0)
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

/// Reads the native property payload from the same published columns as the provider.
pub(super) fn native(property: i32, tree: &Tree, at: u16) -> Val {
    let Some(entry) = tree.at(at) else {
        return Val::Empty;
    };
    let enabled = tree.state(at).has(State::ENABLED);
    let read_only = !(entry.flags.has(ColFlags::FIELD) || entry.flags.has(ColFlags::RANGED))
        || entry.flags.has(ColFlags::READ_ONLY)
        || !enabled;
    match property {
        UIA_ValueValuePropertyId => tree
            .choice(at)
            .map_or(Val::Empty, |(_, name)| Val::Text(name.into())),
        UIA_NamePropertyId => Val::Text(tree.text(entry.name).into()),
        UIA_HelpTextPropertyId => Val::Text(tree.help(at).into()),
        UIA_IsEnabledPropertyId => Val::Bool(enabled),
        UIA_IsKeyboardFocusablePropertyId => Val::Bool(entry.flags.has(ColFlags::FOCUSABLE)),
        UIA_IsOffscreenPropertyId => Val::Bool(tree.clipped(at)),
        UIA_BoundingRectanglePropertyId => Val::Rect(tree.bounds(at)),
        UIA_ValueIsReadOnlyPropertyId | UIA_RangeValueIsReadOnlyPropertyId => Val::Bool(read_only),
        UIA_OrientationPropertyId => {
            Val::Int(tree.range(at).map_or(0, |r| if r.vertical { 2 } else { 1 }))
        }
        UIA_RangeValueMinimumPropertyId => {
            tree.range(at).map_or(Val::Empty, |r| Val::Number(r.min))
        }
        UIA_RangeValueMaximumPropertyId => {
            tree.range(at).map_or(Val::Empty, |r| Val::Number(r.max))
        }
        UIA_RangeValueSmallChangePropertyId => tree.range(at).map_or(Val::Empty, |r| {
            Val::Number(if r.step > 0.0 {
                r.step
            } else {
                (r.max - r.min) * 0.01
            })
        }),
        UIA_RangeValueLargeChangePropertyId => tree
            .range(at)
            .map_or(Val::Empty, |r| Val::Number((r.max - r.min) * 0.1)),
        _ => {
            let Some((view, offset)) = tree.viewport(at) else {
                return Val::Empty;
            };
            let travel = view.travel();
            let (extent, content, travel, offset) = match property {
                UIA_ScrollHorizontalScrollPercentPropertyId
                | UIA_ScrollHorizontalViewSizePropertyId
                | UIA_ScrollHorizontallyScrollablePropertyId => {
                    (view.view.x, view.content.x, travel.x, offset.x)
                }
                _ => (view.view.y, view.content.y, travel.y, offset.y),
            };
            match property {
                UIA_ScrollHorizontalScrollPercentPropertyId
                | UIA_ScrollVerticalScrollPercentPropertyId => {
                    Val::Number(f64::from(if travel <= 0.0 {
                        -1.0
                    } else {
                        (offset / travel).clamp(0.0, 1.0) * 100.0
                    }))
                }
                UIA_ScrollHorizontalViewSizePropertyId | UIA_ScrollVerticalViewSizePropertyId => {
                    Val::Number(f64::from(if content <= 0.0 {
                        100.0
                    } else {
                        (extent / content).clamp(0.0, 1.0) * 100.0
                    }))
                }
                UIA_ScrollHorizontallyScrollablePropertyId
                | UIA_ScrollVerticallyScrollablePropertyId => Val::Bool(travel > 0.0),
                _ => Val::Empty,
            }
        }
    }
}
