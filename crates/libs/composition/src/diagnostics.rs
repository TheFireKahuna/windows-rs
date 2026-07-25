//! The composition engine's own debug heat maps.
//!
//! `Windows.UI.Composition.Diagnostics` asks the compositor process to tint what
//! it is actually doing, straight onto the live window: which regions it redrew
//! this frame, which pixels more than one visual touched, and how much GPU
//! memory the tree holds. It answers questions no in-process counter can —
//! a redraw map shows the dirty region DWM *derived*, not the one the app
//! believes it caused — and it needs no tracing session, no symbols and no
//! second run.
//!
//! The namespace is deliberately absent from the generated bindings: those are
//! produced from one filter across both composition stacks, and pulling in a
//! diagnostics-only namespace to reach three interfaces and four methods would
//! churn the generated file for every consumer. The interfaces are declared
//! here instead, against the same IIDs, and stay behind the safe wrapper at the
//! bottom of the file.
//!
//! Heat maps are a system-stack facility, requested per subtree. `Hide` is not
//! optional bookkeeping: a heat map left on outlives the call that set it.

use super::*;

windows_core::imp::define_interface!(
    ICompositionDebugHeatMaps,
    ICompositionDebugHeatMaps_Vtbl,
    0xe49c90ac_2ff3_5805_718c_b725ee07650f
);

#[repr(C)]
pub struct ICompositionDebugHeatMaps_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Hide: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ShowMemoryUsage: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ShowOverdraw: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        u32,
    ) -> windows_core::HRESULT,
    pub ShowRedraw: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}

windows_core::imp::define_interface!(
    ICompositionDebugSettings,
    ICompositionDebugSettings_Vtbl,
    0x2831987e_1d82_4d38_b7b7_efd11c7bc3d1
);

#[repr(C)]
pub struct ICompositionDebugSettings_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub HeatMaps: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}

windows_core::imp::define_interface!(
    ICompositionDebugSettingsStatics,
    ICompositionDebugSettingsStatics_Vtbl,
    0x64ec1f1e_6af8_4af8_b814_c870fd5a9505
);

#[repr(C)]
pub struct ICompositionDebugSettingsStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub TryGetSettings: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}

/// Names the runtime class whose activation factory carries
/// [`ICompositionDebugSettingsStatics`]. It is never instantiated — the class
/// has no constructible surface, only statics — so it exists purely to give the
/// factory cache a name to resolve.
struct DebugSettingsClass;

impl windows_core::RuntimeName for DebugSettingsClass {
    const NAME: &'static str = "Windows.UI.Composition.Diagnostics.CompositionDebugSettings";
}

/// Which kinds of content an overdraw map should account for.
///
/// `OffscreenRendered` is the interesting one for a retained tree: it highlights
/// exactly the content the compositor had to render to an intermediate surface
/// before it could composite it, which is the cost a visual-surface mask or an
/// effect brush imposes and which nothing else makes visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverdrawKinds {
    OffscreenRendered,
    Colors,
    Effects,
    Shadows,
    Lights,
    Surfaces,
    SwapChains,
    All,
}

impl OverdrawKinds {
    fn bits(self) -> u32 {
        match self {
            Self::OffscreenRendered => 1,
            Self::Colors => 2,
            Self::Effects => 4,
            Self::Shadows => 8,
            Self::Lights => 16,
            Self::Surfaces => 32,
            Self::SwapChains => 64,
            Self::All => u32::MAX,
        }
    }
}

/// The compositor's heat maps, obtained from
/// [`Compositor::debug_heat_maps`](crate::Compositor::debug_heat_maps).
///
/// Each call replaces whatever map that subtree was showing; [`hide`](Self::hide)
/// clears it.
#[derive(Clone)]
pub struct DebugHeatMaps(ICompositionDebugHeatMaps);

impl DebugHeatMaps {
    /// Tint the regions the compositor recomposed, per frame. A window whose
    /// content is genuinely static tints nothing; a window that tints its whole
    /// area every frame is asking DWM to redo work it did not need to.
    pub fn show_redraw(&self, subtree: &Visual) -> Result<()> {
        unsafe {
            (Interface::vtable(&self.0).ShowRedraw)(
                Interface::as_raw(&self.0),
                Interface::as_raw(&subtree.0),
            )
            .ok()
        }
    }

    /// Tint pixels that more than one visual contributes to, for the selected
    /// kinds of content.
    pub fn show_overdraw(&self, subtree: &Visual, kinds: OverdrawKinds) -> Result<()> {
        unsafe {
            (Interface::vtable(&self.0).ShowOverdraw)(
                Interface::as_raw(&self.0),
                Interface::as_raw(&subtree.0),
                kinds.bits(),
            )
            .ok()
        }
    }

    /// Tint the subtree by the GPU memory its content holds.
    pub fn show_memory_usage(&self, subtree: &Visual) -> Result<()> {
        unsafe {
            (Interface::vtable(&self.0).ShowMemoryUsage)(
                Interface::as_raw(&self.0),
                Interface::as_raw(&subtree.0),
            )
            .ok()
        }
    }

    /// Clear whatever map this subtree is showing.
    pub fn hide(&self, subtree: &Visual) -> Result<()> {
        unsafe {
            (Interface::vtable(&self.0).Hide)(
                Interface::as_raw(&self.0),
                Interface::as_raw(&subtree.0),
            )
            .ok()
        }
    }
}

impl Compositor {
    /// The compositor's debug heat maps, when this system exposes them.
    ///
    /// Absent twice over, and both are ordinary rather than exceptional: the
    /// diagnostics class may not be registered at all (it is not carried by
    /// every edition), which is the `Err`; and `TryGetSettings` reports "no
    /// debug settings for this compositor" as a null out-parameter with `S_OK`,
    /// which is the `Ok(None)`. A caller that cannot show a heat map carries on
    /// without one.
    pub fn debug_heat_maps(&self) -> Result<Option<DebugHeatMaps>> {
        static FACTORY: windows_core::imp::FactoryCache<
            DebugSettingsClass,
            ICompositionDebugSettingsStatics,
        > = windows_core::imp::FactoryCache::new();

        let settings = FACTORY.call(|statics| unsafe {
            let mut raw = core::ptr::null_mut();
            (Interface::vtable(statics).TryGetSettings)(
                Interface::as_raw(statics),
                Interface::as_raw(&self.0),
                &mut raw,
            )
            .ok()?;
            Ok((!raw.is_null()).then(|| ICompositionDebugSettings::from_raw(raw)))
        })?;

        let Some(settings) = settings else {
            return Ok(None);
        };
        unsafe {
            let mut raw = core::ptr::null_mut();
            (Interface::vtable(&settings).HeatMaps)(Interface::as_raw(&settings), &mut raw).ok()?;
            Ok((!raw.is_null()).then(|| DebugHeatMaps(ICompositionDebugHeatMaps::from_raw(raw))))
        }
    }
}
