//! The control patterns.
//!
//! Every **query** reads the published tree and its live half; every **command** queues an
//! action and returns. That split is the platform's own — `Invoke` "is an asynchronous
//! call and must return immediately without blocking", and the same is written of `Toggle`,
//! `Select` and both `SetValue`s — so returning before the work happens is the contract,
//! and it is also what keeps a client off the critical path of a busy front thread.
//!
//! One object answers all of them; which ones it *admits* to is the role table's business,
//! decided in `GetPatternProvider` before anything here is reached.

use super::action::Action;
use super::element::{At, Element_Impl, NO_PART, gone, none};
use super::live::State;
use super::tree::{ColFlags, Tree, Value};
use crate::bindings::{
    ExpandCollapseState, ExpandCollapseState_Collapsed, ExpandCollapseState_Expanded,
    ExpandCollapseState_LeafNode, IRawElementProviderSimple, SAFEARRAY, ToggleState,
    ToggleState_Off, ToggleState_On,
};
use crate::widget::Range;
use windows_core::{BOOL, BSTR, PCWSTR, Result};
use windows_scene::NO_ENTRY;

/// A large step is a page; a small one is the range's own step, or a hundredth of it where
/// the range is continuous. Both are what a client offers as keyboard equivalents, so they
/// have to be the increments the control itself moves in.
const LARGE_FRACTION: f64 = 0.1;
const SMALL_FRACTION: f64 = 0.01;

impl Element_Impl {
    /// The element's own range, or `None` where it carries no number.
    fn range(&self) -> Option<(At, Range)> {
        let at = self.at().ok()?;
        match at.tree.col(at.at)?.value {
            Value::Range(range) => Some((at, range)),
            Value::None | Value::Text => None,
        }
    }

    /// The live number, whether written by the front thread or by a region's producer.
    ///
    /// A producer's cell wins where there is one: it is the newer of the two by
    /// construction, written from the thread that drew the pixels the number describes.
    fn number(&self) -> Result<f64> {
        let At {
            shared,
            tree,
            at,
            part,
        } = self.at()?;
        if part != NO_PART {
            return shared
                .regions
                .with_parts(self.id(), |parts| {
                    parts
                        .iter()
                        .find(|candidate| candidate.sub == part)
                        .and_then(|part| part.value)
                })
                .ok_or_else(none);
        }
        shared
            .regions
            .value(self.id())
            .or_else(|| tree.live.value(at))
            .ok_or_else(none)
    }

    fn enabled(&self) -> bool {
        self.at()
            .is_ok_and(|At { tree, at, .. }| tree.live.state(at).has(State::ENABLED))
    }
}

impl crate::bindings::IInvokeProvider_Impl for Element_Impl {
    fn Invoke(&self) -> Result<()> {
        if !self.enabled() {
            return Err(disabled());
        }
        self.shared()?.act(Action::Invoke(self.id()));
        Ok(())
    }
}

impl crate::bindings::IToggleProvider_Impl for Element_Impl {
    fn Toggle(&self) -> Result<()> {
        self.shared()?.act(Action::Toggle(self.id()));
        Ok(())
    }

    fn ToggleState(&self) -> Result<ToggleState> {
        let At { tree, at, .. } = self.at()?;
        Ok(if tree.live.state(at).has(State::TOGGLED) {
            ToggleState_On
        } else {
            ToggleState_Off
        })
    }
}

impl crate::bindings::IValueProvider_Impl for Element_Impl {
    fn SetValue(&self, value: &PCWSTR) -> Result<()> {
        // Refused where `IsReadOnly` says so, rather than accepted and dropped: a provider
        // that answers `S_OK` to a write it will not perform tells a client the value
        // changed, and the client then reads the old one back and reports a stuck control.
        //
        // A string-valued surface is always read-only here — the editable one is text
        // services', and it publishes `TextPattern` rather than taking dictated strings
        // through this seam. A number parses, because a client offers "type a value".
        let range = self.range().ok_or_else(readonly)?.1;
        if !self.enabled() {
            return Err(disabled());
        }
        // SAFETY: automation passes a null-terminated wide string it owns for the call.
        let text = unsafe { value.to_string() }.map_err(|_| invalid())?;
        let parsed: f64 = text.trim().parse().map_err(|_| invalid())?;
        if !parsed.is_finite() || parsed < range.min || parsed > range.max {
            return Err(invalid());
        }
        self.shared()?.act(Action::SetValue(self.id(), parsed));
        Ok(())
    }

    fn Value(&self) -> Result<BSTR> {
        let At { tree, at, .. } = self.at()?;
        let col = tree.col(at).ok_or_else(gone)?;
        // A read-only surface reports its own body; a numeric one formats its number to
        // the precision its step implies, so a reader does not speak a slider's float noise.
        if col.value == Value::Text {
            return Ok(super::variant::bstr(tree.text(col.name)));
        }
        let number = self.number()?;
        let step = self.range().map_or(0.0, |(_, range)| range.step);
        Ok(BSTR::from(format(number, step)))
    }

    fn IsReadOnly(&self) -> Result<BOOL> {
        let At { tree, at, .. } = self.at()?;
        let editable = matches!(tree.col(at).map(|col| col.value), Some(Value::Range(_)));
        Ok(BOOL::from(!editable || !self.enabled()))
    }
}

impl crate::bindings::IRangeValueProvider_Impl for Element_Impl {
    fn SetValue(&self, value: f64) -> Result<()> {
        let (_, range) = self.range().ok_or_else(readonly)?;
        if !self.enabled() {
            return Err(disabled());
        }
        if !value.is_finite() || value < range.min || value > range.max {
            return Err(invalid());
        }
        self.shared()?.act(Action::SetValue(self.id(), value));
        Ok(())
    }

    fn Value(&self) -> Result<f64> {
        self.number()
    }

    fn IsReadOnly(&self) -> Result<BOOL> {
        Ok(BOOL::from(!self.enabled()))
    }

    fn Maximum(&self) -> Result<f64> {
        Ok(self.range().ok_or_else(none)?.1.max)
    }

    fn Minimum(&self) -> Result<f64> {
        Ok(self.range().ok_or_else(none)?.1.min)
    }

    fn LargeChange(&self) -> Result<f64> {
        let (_, range) = self.range().ok_or_else(none)?;
        Ok((range.max - range.min) * LARGE_FRACTION)
    }

    fn SmallChange(&self) -> Result<f64> {
        let (_, range) = self.range().ok_or_else(none)?;
        Ok(if range.step > 0.0 {
            range.step
        } else {
            (range.max - range.min) * SMALL_FRACTION
        })
    }
}

impl crate::bindings::ISelectionProvider_Impl for Element_Impl {
    fn GetSelection(&self) -> Result<*mut SAFEARRAY> {
        // A container reports which of its children is selected. Built from the live half,
        // so a selection change does not republish the tree.
        let At {
            shared, tree, at, ..
        } = self.at()?;
        let selected: Vec<IRawElementProviderSimple> = children(&tree, at)
            .filter(|&child| tree.live.state(child).has(State::SELECTED))
            .filter_map(|child| {
                let entry = tree.entry(child)?;
                super::element::provider_for(&shared, entry.id)
            })
            .collect();
        Ok(super::variant::provider_array(&selected))
    }

    fn CanSelectMultiple(&self) -> Result<BOOL> {
        Ok(BOOL::from(false))
    }

    fn IsSelectionRequired(&self) -> Result<BOOL> {
        Ok(BOOL::from(false))
    }
}

impl crate::bindings::ISelectionItemProvider_Impl for Element_Impl {
    fn Select(&self) -> Result<()> {
        self.shared()?.act(Action::Select(self.id()));
        Ok(())
    }

    fn AddToSelection(&self) -> Result<()> {
        // Single-selection, so adding to the selection *is* selecting. Failing instead
        // would make a client believe the control cannot be operated at all.
        crate::bindings::ISelectionItemProvider_Impl::Select(self)
    }

    fn RemoveFromSelection(&self) -> Result<()> {
        // Single-selection: there is no state in which nothing is selected, so clearing
        // one is not a thing the control can be asked to do.
        Err(invalid())
    }

    fn IsSelected(&self) -> Result<BOOL> {
        let At { tree, at, .. } = self.at()?;
        Ok(BOOL::from(tree.live.state(at).has(State::SELECTED)))
    }

    fn SelectionContainer(&self) -> Result<IRawElementProviderSimple> {
        let At {
            shared, tree, at, ..
        } = self.at()?;
        let parent = tree.col(at).ok_or_else(gone)?.parent;
        let entry = tree.entry(parent as usize).ok_or_else(none)?;
        super::element::provider_for(&shared, entry.id).ok_or_else(none)
    }
}

impl crate::bindings::IExpandCollapseProvider_Impl for Element_Impl {
    fn Expand(&self) -> Result<()> {
        self.shared()?.act(Action::Expand(self.id(), true));
        Ok(())
    }

    fn Collapse(&self) -> Result<()> {
        self.shared()?.act(Action::Expand(self.id(), false));
        Ok(())
    }

    fn ExpandCollapseState(&self) -> Result<ExpandCollapseState> {
        let At { tree, at, .. } = self.at()?;
        let col = tree.col(at).ok_or_else(gone)?;
        Ok(if !col.flags.has(ColFlags::EXPANDS) {
            ExpandCollapseState_LeafNode
        } else if tree.live.state(at).has(State::EXPANDED) {
            ExpandCollapseState_Expanded
        } else {
            ExpandCollapseState_Collapsed
        })
    }
}

/// `ScrollPattern` is deliberately **not** implemented.
///
/// It reports where a container stands as a percentage of its *content*, and how much of
/// that content is visible — and this tree publishes neither. The scroll extent lives with
/// the interaction tracker and never reaches the hit array, so every number the pattern
/// asks for would be a plausible fabrication of the viewport's own size. `ScrollItem` is
/// what a reader actually needs, it is a command rather than a measurement, and it is
/// correct. Publishing the extent alongside the offset is what would bring the other one
/// back.
impl crate::bindings::IScrollItemProvider_Impl for Element_Impl {
    fn ScrollIntoView(&self) -> Result<()> {
        self.shared()?.act(Action::Reveal(self.id()));
        Ok(())
    }
}

/// Every child of `at`, through the links the build pass filled.
fn children(tree: &Tree, at: usize) -> impl Iterator<Item = usize> + '_ {
    let mut next = tree.col(at).map_or(NO_ENTRY, |col| col.first_child);
    core::iter::from_fn(move || {
        if next == NO_ENTRY {
            return None;
        }
        let at = next as usize;
        next = tree.col(at).map_or(NO_ENTRY, |col| col.next_sibling);
        Some(at)
    })
}

/// The three failures automation distinguishes, named once rather than spelled at each
/// call site.
fn disabled() -> windows_core::Error {
    windows_core::Error::from_hresult(windows_core::HRESULT(
        crate::bindings::UIA_E_ELEMENTNOTENABLED as i32,
    ))
}

fn invalid() -> windows_core::Error {
    windows_core::Error::from_hresult(windows_core::HRESULT(
        crate::bindings::UIA_E_INVALIDOPERATION as i32,
    ))
}

/// A write to something that does not take one. Distinct from `invalid`, which is a value
/// the control could not have held.
fn readonly() -> windows_core::Error {
    invalid()
}

/// A number at the precision its own step implies.
///
/// A slider reading "-14.500000001 decibels" is a defect a screen reader cannot work
/// around, and rounding at the draw is not rounding at the announcement.
fn format(value: f64, step: f64) -> String {
    let places: usize = if step <= 0.0 {
        2
    } else {
        // The first decimal place the step is not a multiple of, capped where an f64 stops
        // being able to tell.
        (0..=6)
            .find(|places| {
                let scale = 10f64.powi(i32::try_from(*places).unwrap_or(6));
                (step * scale - (step * scale).round()).abs() < 1.0e-6
            })
            .unwrap_or(6)
    };
    format!("{value:.places$}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_read_out_at_the_precision_its_step_implies() {
        assert_eq!(format(-14.500_000_001, 0.5), "-14.5");
        assert_eq!(format(3.0, 1.0), "3");
        assert_eq!(format(0.125, 0.125), "0.125");
        assert_eq!(format(1.0 / 3.0, 0.0), "0.33", "continuous falls back");
    }
}
