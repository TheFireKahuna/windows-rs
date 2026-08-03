//! Telling a client what changed.
//!
//! Raised from the front thread at the end of the tick, after the publish. Both halves of
//! that matter: an event is only correct if the tree is consistent at the instant of the
//! call, and raising from inside an input handler re-enters a client's callback in the
//! middle of a gesture. Draining at one point gets both without either being remembered.
//!
//! A raise is coalesced to one per element per property per tick — a drag would otherwise
//! raise a property change per pointer sample — and a coalesced one keeps the **oldest**
//! previous value and the **newest** current one, which is what the whole burst amounted
//! to.

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

/// What a tick has to tell a client about.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Raise {
    /// The tree was replaced. One event for the whole change, not one per element.
    Structure,
    Focus(ControlId),
    Invoked(ControlId),
    /// A property, **with what it was and what it is**.
    ///
    /// Both, because automation compares them: a change reported as empty-to-empty is a
    /// change from nothing to nothing, and never reaches a listener.
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

/// The properties this stack announces a change to, and the type each carries.
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
    #[must_use]
    pub const fn id(self) -> i32 {
        match self {
            Self::Range => UIA_RangeValueValuePropertyId,
            Self::Toggle => UIA_ToggleToggleStatePropertyId,
            Self::Selected => UIA_SelectionItemIsSelectedPropertyId,
            Self::Expanded => UIA_ExpandCollapseExpandCollapseStatePropertyId,
        }
    }

    /// A boolean state, in whichever type this property is reported as.
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

/// The pending set. Bounded by the number of elements that changed in one tick, and its
/// allocation is kept across ticks, so a steady drag allocates nothing.
#[derive(Debug, Default)]
pub struct Pending(Vec<Raise>);

impl Pending {
    /// Records an event, folding it into one that says the same thing about the same
    /// element.
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

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Moves what is pending into `out` without raising it. The tests' whole view of this
    /// module.
    ///
    /// Appends rather than hands the buffer over, for the same reason [`flush`](Self::flush)
    /// drains: the set keeps its allocation across ticks, and a helper that took it would
    /// make the next event allocate and the measurement blame the code for the helper.
    #[cfg(test)]
    pub fn take(&mut self, out: &mut Vec<Raise>) {
        out.append(&mut self.0);
    }

    /// Raises everything pending, then empties the set.
    ///
    /// `provider` resolves an element to the object a client holds; an element that has
    /// gone since the event was recorded resolves to nothing and is skipped, which is the
    /// correct outcome rather than a failure.
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
                    // Named on the fragment root — a bulk change over the whole window,
                    // because the array is replaced wholesale and there is no diff to
                    // describe.
                    let Some(root) = provider(ControlId::NONE) else {
                        continue;
                    };
                    // SAFETY: a live provider, and a runtime id of zero length, which is
                    // what "the root itself" means for a bulk change.
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
                    // SAFETY: a live provider, and two variants of the type this property
                    // is reported as. Both are plain scalars, so neither owns an allocation
                    // the callee would have to release.
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
            // SAFETY: a live provider and a constant event id.
            unsafe {
                _ = UiaRaiseAutomationEvent(element.as_raw(), event);
            }
        }
    }
}

/// Whether raising an event could reach anybody.
///
/// A hint rather than a guarantee — measured `true` on a bare desktop with nothing
/// attached — which is why it gates only the cost of a raise and nothing structural.
#[must_use]
pub fn listening() -> bool {
    // SAFETY: no arguments, no state, callable from any thread.
    unsafe { UiaClientsAreListening().as_bool() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minted through the real authority: an id is generational, and forging one would
    /// test a shape the rest of the stack cannot produce.
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
