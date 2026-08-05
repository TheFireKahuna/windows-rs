//! Implements the control patterns on the provider object.
//!
//! A query reads the published tree and its live half. A command queues an [`Action`] and
//! returns without blocking, which is what the platform requires of `Invoke`, `Toggle`,
//! `Select` and both `SetValue`s.
//!
//! One object implements every pattern. Which ones an element advertises is decided by the
//! role table in `GetPatternProvider`, before any method here is reached.

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

/// Fraction of a range's span reported as `LargeChange`, one page of movement.
/// [`SMALL_FRACTION`] is what `SmallChange` reports where the range declares no step of its
/// own. A client offers both as keyboard increments, so they are the steps the control
/// itself moves in.
const LARGE_FRACTION: f64 = 0.1;
const SMALL_FRACTION: f64 = 0.01;

impl Element_Impl {
    /// Resolves the element and returns its numeric range, or `None` where it carries no
    /// number.
    fn range(&self) -> Option<(At, Range)> {
        let at = self.at().ok()?;
        match at.tree.col(at.at)?.value {
            Value::Range(range) => Some((at, range)),
            Value::None | Value::Text => None,
        }
    }

    /// Returns the live number for this element, or for the region part it addresses.
    ///
    /// An element addressing a part reads that part's slot. Otherwise a region's producer
    /// cell is preferred over the tree's live cell, because it is written by the thread
    /// that drew the pixels the number describes.
    ///
    /// # Errors
    ///
    /// Returns the empty error, which reports "no value here" rather than a failure, where
    /// no cell holds a finite number.
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
        // Only a numeric element takes a write, which is what `IsReadOnly` reports. The
        // write is refused rather than accepted and dropped, so a client cannot read the
        // old value back after an `S_OK`. A text-valued element is read-only through this
        // pattern and publishes `TextPattern` instead.
        let range = self.range().ok_or_else(readonly)?.1;
        if !self.enabled() {
            return Err(disabled());
        }
        // SAFETY: the `IValueProvider::SetValue` ABI passes a null-terminated wide string
        // owned by automation and valid for the duration of the call.
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
        // A text element answers with its own body. A numeric one formats to the precision
        // its step implies, so the announced value carries no float noise.
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
        // Read from the live half, so a selection change needs no republish.
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
        // Selection here is single, so adding to it is selecting.
        crate::bindings::ISelectionItemProvider_Impl::Select(self)
    }

    fn RemoveFromSelection(&self) -> Result<()> {
        // Selection here is single: no state has nothing selected, so there is no
        // selection to clear.
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

/// `ScrollItem` is the only scrolling pattern implemented.
///
/// `ScrollPattern` reports a container's position as a percentage of its content and how
/// much of that content is visible. The scroll extent lives with the interaction tracker
/// and never reaches the hit array, so neither number is available here. `ScrollItem` is a
/// command rather than a measurement and needs neither.
impl crate::bindings::IScrollItemProvider_Impl for Element_Impl {
    fn ScrollIntoView(&self) -> Result<()> {
        self.shared()?.act(Action::Reveal(self.id()));
        Ok(())
    }
}

/// Returns each child index of the entry at `at`, in sibling order.
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

/// Returns `UIA_E_ELEMENTNOTENABLED`, the answer to a command on a disabled element.
fn disabled() -> windows_core::Error {
    windows_core::Error::from_hresult(windows_core::HRESULT(
        crate::bindings::UIA_E_ELEMENTNOTENABLED as i32,
    ))
}

/// Returns `UIA_E_INVALIDOPERATION`, the answer to a value or operation the control cannot
/// hold.
fn invalid() -> windows_core::Error {
    windows_core::Error::from_hresult(windows_core::HRESULT(
        crate::bindings::UIA_E_INVALIDOPERATION as i32,
    ))
}

/// Returns the answer to a write on an element that accepts none. Named apart from
/// [`invalid`], which reports a value outside what the control can hold.
fn readonly() -> windows_core::Error {
    invalid()
}

/// Formats `value` at the number of decimal places `step` implies.
///
/// A `step` of zero or less marks a continuous range and formats to two places. The
/// rounding a control applies when drawing does not reach the announced string, so it is
/// applied here.
fn format(value: f64, step: f64) -> String {
    let places: usize = if step <= 0.0 {
        2
    } else {
        // The fewest decimal places that express `step` exactly, capped at six where the
        // comparison stops discriminating in an f64.
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
