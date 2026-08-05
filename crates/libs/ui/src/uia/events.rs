//! Queues and raises the automation events for what a tick changed.
//!
//! A flush runs on the front thread at the end of the tick, after the publish, so the tree
//! is consistent when a client reads it back and no raise re-enters a client's callback in
//! the middle of an input handler.
//!
//! A property raise is folded to one per element per property per tick, keeping the oldest
//! previous value and the newest current one, so a drag reports one change spanning the
//! whole burst rather than one per pointer sample.

use crate::bindings::{
    IRawElementProviderSimple, StructureChangeType_ChildrenBulkAdded,
    UIA_AutomationFocusChangedEventId, UIA_ExpandCollapseExpandCollapseStatePropertyId,
    UIA_Invoke_InvokedEventId, UIA_LiveRegionChangedEventId, UIA_MenuClosedEventId,
    UIA_MenuOpenedEventId, UIA_RangeValueValuePropertyId, UIA_SelectionItemIsSelectedPropertyId,
    UIA_ToggleToggleStatePropertyId, UIA_ToolTipOpenedEventId, UiaClientsAreListening,
    UiaRaiseAutomationEvent, UiaRaiseAutomationPropertyChangedEvent, UiaRaiseStructureChangedEvent,
    VARIANT,
};
use windows_core::Interface;
use windows_scene::ControlId;

/// One event a tick has to raise.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Raise {
    /// The tree was replaced. One event for the whole change, not one per element.
    Structure,
    Focus(ControlId),
    Invoked(ControlId),
    /// A property change, carrying both the previous and the current value.
    ///
    /// Automation compares the two: a raise whose values are both empty describes a
    /// change from nothing to nothing and reaches no listener.
    Property {
        id: ControlId,
        what: Property,
        from: Val,
        to: Val,
    },
    Live(ControlId),
    MenuOpened(ControlId),
    MenuClosed(ControlId),
    TooltipOpened(ControlId),
}

/// A property this stack raises changes for, and the type it is reported as.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Property {
    /// `RangeValue.Value`, as a double.
    Range,
    /// `Toggle.ToggleState`, as the enumeration's integer.
    Toggle,
    /// `SelectionItem.IsSelected`, as a boolean.
    Selected,
    /// `ExpandCollapse.ExpandCollapseState`, as the enumeration's integer.
    Expanded,
}

impl Property {
    /// Returns the automation property id this property is raised under.
    #[must_use]
    pub const fn id(self) -> i32 {
        match self {
            Self::Range => UIA_RangeValueValuePropertyId,
            Self::Toggle => UIA_ToggleToggleStatePropertyId,
            Self::Selected => UIA_SelectionItemIsSelectedPropertyId,
            Self::Expanded => UIA_ExpandCollapseExpandCollapseStatePropertyId,
        }
    }

    /// Returns `on` in the variant type this property is reported as.
    #[must_use]
    pub const fn of(self, on: bool) -> Val {
        match self {
            Self::Selected => Val::Bool(on),
            // `ToggleState` and `ExpandCollapseState` are both off at 0 and on at 1.
            _ => Val::Int(on as i32),
        }
    }
}

/// A property's value, in the variant type automation expects for it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Val {
    Number(f64),
    Int(i32),
    Bool(bool),
}

impl Val {
    fn variant(self) -> VARIANT {
        match self {
            Self::Number(v) => super::variant::r8(v),
            Self::Int(v) => super::variant::i4(v),
            Self::Bool(v) => super::variant::bool(v),
        }
    }
}

/// The events queued for the current tick.
///
/// Bounded by the number of elements that changed in that tick. The allocation is kept
/// across ticks, so a steady drag allocates nothing.
#[derive(Debug, Default)]
pub struct Pending(Vec<Raise>);

impl Pending {
    /// Records `raise`, folding it into a queued event that names the same element.
    ///
    /// A property change merges into the queued change for that element and property,
    /// keeping the queued previous value and taking the new current one. Any other event
    /// is recorded once and a duplicate is dropped.
    pub fn push(&mut self, raise: Raise) {
        if let Raise::Property { id, what, to, .. } = raise {
            // The burst becomes one change: from where it started to where it ended.
            if let Some(Raise::Property { to: last, .. }) = self.0.iter_mut().find(|queued| {
                matches!(queued, Raise::Property { id: a, what: b, .. } if *a == id && *b == what)
            }) {
                *last = to;
                return;
            }
        } else if self.0.contains(&raise) {
            return;
        }
        self.0.push(raise);
    }

    /// Returns whether nothing is queued.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Moves everything queued into `out` without raising it.
    ///
    /// Appends rather than handing the buffer over, as [`flush`](Self::flush) drains
    /// rather than replacing it, so the set keeps its allocation across ticks.
    #[cfg(test)]
    pub fn take(&mut self, out: &mut Vec<Raise>) {
        out.append(&mut self.0);
    }

    /// Raises every queued event, then empties the set.
    ///
    /// `provider` resolves an element to the object a client holds. An element that has
    /// unmounted since the event was recorded resolves to `None` and is skipped rather
    /// than failing the flush. Nothing is raised when no client is [`listening`], and the
    /// set is emptied either way.
    pub fn flush(&mut self, provider: impl Fn(ControlId) -> Option<IRawElementProviderSimple>) {
        if self.0.is_empty() {
            return;
        }
        if !listening() {
            self.0.clear();
            return;
        }
        for raise in self.0.drain(..) {
            let (id, event) = match raise {
                Raise::Structure => {
                    // Raised on the fragment root as a bulk change: the array is replaced
                    // wholesale, so there is no per-element diff to describe.
                    let Some(root) = provider(ControlId::NONE) else {
                        continue;
                    };
                    // SAFETY: `root` is a provider object alive for the call, and a null
                    // runtime id of length zero names the root itself, which is the form a
                    // bulk change takes.
                    unsafe {
                        _ = UiaRaiseStructureChangedEvent(
                            root.as_raw(),
                            StructureChangeType_ChildrenBulkAdded,
                            core::ptr::null_mut(),
                            0,
                        );
                    }
                    continue;
                }
                Raise::Property { id, what, from, to } => {
                    let Some(element) = provider(id) else {
                        continue;
                    };
                    // SAFETY: `element` is a provider object alive for the call, and both
                    // variants carry the type this property is reported as. Each holds a
                    // scalar, so neither owns an allocation the callee would release.
                    unsafe {
                        _ = UiaRaiseAutomationPropertyChangedEvent(
                            element.as_raw(),
                            what.id(),
                            from.variant(),
                            to.variant(),
                        );
                    }
                    continue;
                }
                Raise::Focus(id) => (id, UIA_AutomationFocusChangedEventId),
                Raise::Invoked(id) => (id, UIA_Invoke_InvokedEventId),
                Raise::Live(id) => (id, UIA_LiveRegionChangedEventId),
                Raise::MenuOpened(id) => (id, UIA_MenuOpenedEventId),
                Raise::MenuClosed(id) => (id, UIA_MenuClosedEventId),
                Raise::TooltipOpened(id) => (id, UIA_ToolTipOpenedEventId),
            };
            let Some(element) = provider(id) else {
                continue;
            };
            // SAFETY: `element` is a provider object alive for the call, and `event` is one
            // of the event id constants above.
            unsafe {
                _ = UiaRaiseAutomationEvent(element.as_raw(), event);
            }
        }
    }
}

/// Returns whether a raised event could reach a client.
///
/// A hint rather than a guarantee: it answers `true` on a desktop with no client attached,
/// so it gates only the cost of raising and nothing structural.
#[must_use]
pub fn listening() -> bool {
    // SAFETY: the call takes no arguments, reads no caller state, and is callable from any
    // thread.
    unsafe { UiaClientsAreListening().as_bool() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mints `count` ids through the id authority. An id is generational, so a value not
    /// minted here is not one the stack can produce.
    fn ids(count: usize) -> Vec<ControlId> {
        let mut authority = windows_scene::Ids::<windows_scene::Control>::new();
        (0..count).map(|_| authority.mint()).collect()
    }

    fn moved(id: ControlId, from: f64, to: f64) -> Raise {
        Raise::Property {
            id,
            what: Property::Range,
            from: Val::Number(from),
            to: Val::Number(to),
        }
    }

    #[test]
    fn a_drag_raises_one_change_spanning_the_whole_burst() {
        let id = ids(2);
        let mut pending = Pending::default();
        for step in 0..64 {
            pending.push(moved(id[0], f64::from(step), f64::from(step) + 1.0));
        }
        pending.push(moved(id[1], 0.0, 9.0));

        assert_eq!(pending.0.len(), 2, "one per element per property");
        assert_eq!(
            pending.0[0],
            moved(id[0], 0.0, 64.0),
            "from where the burst started to where it ended, not the last sample's pair"
        );
    }

    #[test]
    fn two_properties_of_one_element_do_not_fold_together() {
        let id = ids(1);
        let mut pending = Pending::default();
        pending.push(moved(id[0], 0.0, 1.0));
        pending.push(Raise::Property {
            id: id[0],
            what: Property::Toggle,
            from: Val::Int(0),
            to: Val::Int(1),
        });
        assert_eq!(pending.0.len(), 2);
    }

    #[test]
    fn one_structure_event_describes_a_whole_republish() {
        let mut pending = Pending::default();
        pending.push(Raise::Structure);
        pending.push(Raise::Structure);
        assert_eq!(pending.0.len(), 1);
    }
}
