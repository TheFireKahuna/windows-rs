//! `ITfContextOwnerCompositionSink` — where composition boundaries come from.
//!
//! TSF calls this back on the front thread when a TIP opens, moves or closes a
//! composition on our context. It is the *only* source of the composing span:
//! composing text itself arrives through the text store as ordinary range
//! replacements, so without this sink the app would show committed-looking text
//! with no underline and the §7.2 guard would never engage.
//!
//! The span is read as ACP offsets by casting the composition's `ITfRange` to
//! `ITfRangeACP` — the same range the store's own offsets live in — so no
//! coordinate translation is needed.

use windows_core::{implement_decl, Interface, Ref, Result, BOOL};

use super::acp::TextInput;
use crate::system_bindings::{
    ITfCompositionView, ITfContextOwnerCompositionSink, ITfContextOwnerCompositionSink_Impl,
    ITfRange, ITfRangeACP,
};

/// The sink object handed to `ITfSource::AdviseSink` on the context.
pub(crate) struct CompositionSink {
    input: TextInput,
}

implement_decl! {
    impl CompositionSink as pub(crate) CompositionSink_Impl: [ITfContextOwnerCompositionSink]
}

impl CompositionSink {
    pub(crate) fn new(input: TextInput) -> Self {
        Self { input }
    }
}

#[allow(non_snake_case)]
impl ITfContextOwnerCompositionSink_Impl for CompositionSink_Impl {
    fn OnStartComposition(&self, pcomposition: Ref<ITfCompositionView>) -> Result<BOOL> {
        if let Some((start, len)) = view_extent(pcomposition) {
            self.input.on_composition_update(start, len);
        }
        // `TRUE` = allow the composition. We never refuse one: refusing is for
        // apps that own a region they will not let a TIP edit, and every field
        // that can be focused here is editable by definition.
        Ok(BOOL(1))
    }

    fn OnUpdateComposition(
        &self,
        _pcomposition: Ref<ITfCompositionView>,
        prangenew: Ref<ITfRange>,
    ) -> Result<()> {
        // The new range is authoritative when supplied; TSF passes null when the
        // composition's text changed but its extent did not, in which case the
        // span the store already marked still holds.
        if let Some((start, len)) = prangenew.ok().ok().and_then(range_extent) {
            self.input.on_composition_update(start, len);
        }
        Ok(())
    }

    fn OnEndComposition(&self, _pcomposition: Ref<ITfCompositionView>) -> Result<()> {
        self.input.on_composition_end();
        Ok(())
    }
}

/// ACP `(start, len)` of a composition view's range.
fn view_extent(view: Ref<ITfCompositionView>) -> Option<(usize, usize)> {
    // SAFETY: a live view for the duration of the callback.
    let range = unsafe { view.ok().ok()?.GetRange() }.ok()?;
    range_extent(&range)
}

/// ACP `(start, len)` of a range, or `None` if it is not an ACP range (it always
/// is for an ACP store, but a cast failure must not panic inside a TIP call).
fn range_extent(range: &ITfRange) -> Option<(usize, usize)> {
    let acp: ITfRangeACP = range.cast().ok()?;
    let (mut anchor, mut cch) = (0i32, 0i32);
    // SAFETY: both out-parameters are valid for the call.
    if unsafe { acp.GetExtent(&mut anchor, &mut cch) }.is_err() {
        return None;
    }
    Some((anchor.max(0) as usize, cch.max(0) as usize))
}
