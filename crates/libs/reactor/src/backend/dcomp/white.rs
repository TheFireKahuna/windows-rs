//! Compositor-evaluated display white-level adjustment.
//!
//! DWM pins linear scRGB `1.0` = 80 nits for FP16 advanced-color content and
//! does **not** apply the OS SDR-white slider to it, so an app that anchors its
//! palette at some reference white (e.g. BT.2408's 203-nit paper white) renders
//! too dim or too bright as the monitor's SDR white level differs from that
//! anchor. The fix lives entirely compositor-side: every surface visual's
//! content brush is wrapped in a `CompositionEffectBrush` running a D2D
//! **Exposure** effect (a pure linear multiply), and each brush's `W.Exposure`
//! scalar is bound by an `ExpressionAnimation` to **one** shared
//! `CompositionPropertySet`. The window host re-queries the monitor's SDR white
//! level (displayconfig) on display events only and writes
//! `exposure = log2(sdr_white_nits / reference_white_nits)` (clamped to the D2D
//! Exposure effect's ±2 range) into that set — a single write rescales every
//! cached surface with **zero app repaints** and no polling.
//!
//! The mechanism is policy-free: it stays inert (plain surface brushes, zero
//! cost) until the app opts in with [`set_hdr_reference_white_nits`] **before
//! the window is created**. The reference white is app data — no luminance
//! anchor is baked into this crate.
//!
//! `CompositionColorBrush` content (the window backdrop) cannot be an effect
//! input; those brushes register here instead and get their stored base color
//! rescaled (a linear-light multiply) on the same refresh events.
#![allow(non_snake_case)]

use std::cell::{Cell, OnceCell, RefCell};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::system_bindings::{
    Color, CompositionAnimation, CompositionBrush, CompositionColorBrush, CompositionEffectBrush,
    CompositionEffectFactory, CompositionEffectSourceParameter, CompositionPropertySet, Compositor,
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, GetMonitorInfoW, ICompositionObject,
    IGraphicsEffect, IGraphicsEffectD2D1Interop, IGraphicsEffectD2D1Interop_Impl,
    IGraphicsEffectSource, IGraphicsEffectSource_Impl, IGraphicsEffect_Impl, MonitorFromWindow,
    PropertyValue, QueryDisplayConfig, CLSID_D2D1Exposure, DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SDR_WHITE_LEVEL,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, GRAPHICS_EFFECT_PROPERTY_MAPPING,
    GRAPHICS_EFFECT_PROPERTY_MAPPING_DIRECT, HWND, MONITORINFO, MONITORINFOEXW,
    MONITOR_DEFAULTTONEAREST, QDC_ONLY_ACTIVE_PATHS,
};
use windows_core::{implement_decl, Interface, HSTRING, PCWSTR};

/// The app-declared reference white (nits) the palette's diffuse white is
/// anchored to, stored as `f32` bits. `0` bits = feature off (the default):
/// every wrap is a plain passthrough and no displayconfig query ever runs.
static REF_NITS: AtomicU32 = AtomicU32::new(0);

/// Opt the process into compositor-side white-level adjustment, anchoring it to
/// the given reference white (nits, e.g. `203.0` for BT.2408 paper white).
///
/// Must be called **before** the app window is created: brushes minted earlier
/// are not wrapped retroactively. Non-finite or non-positive values are ignored
/// (the feature stays off).
pub fn set_hdr_reference_white_nits(nits: f32) {
    if nits.is_finite() && nits > 0.0 {
        REF_NITS.store(nits.to_bits(), Ordering::Relaxed);
    }
}

fn ref_nits() -> Option<f32> {
    let bits = REF_NITS.load(Ordering::Relaxed);
    (bits != 0).then(|| f32::from_bits(bits))
}

/// The D2D Exposure effect's documented valid range for its exposure value.
const EXPOSURE_MIN: f32 = -2.0;
const EXPOSURE_MAX: f32 = 2.0;

/// The compositor-side exposure (in stops) that maps content authored against
/// `reference_white_nits` onto a display whose SDR white is `sdr_white_nits`:
/// `log2(sdr/ref)`, clamped to the D2D Exposure effect's valid range.
fn exposure_for(sdr_white_nits: f32, reference_white_nits: f32) -> f32 {
    (sdr_white_nits / reference_white_nits)
        .log2()
        .clamp(EXPOSURE_MIN, EXPOSURE_MAX)
}

// ── The IGraphicsEffect describing one D2D Exposure pass ─────────────────────

/// Name of the effect inside the factory's effect graph; the animatable
/// property path is `"W.Exposure"`.
const EFFECT_NAME: &str = "W";
/// Name of the effect's single source parameter (bound per-brush to the
/// wrapped surface brush via `SetSourceParameter`).
const SOURCE_NAME: &str = "src";
/// The Exposure effect's one property, per Win2D's naming; maps directly to
/// `D2D1_EXPOSURE_PROP_EXPOSURE_VALUE` (index 0).
const EXPOSURE_PROP: &str = "Exposure";
/// The shared property set's scalar every brush's expression reads.
const SET_SCALAR: &str = "E";

const E_INVALIDARG: windows_core::HRESULT = windows_core::HRESULT(0x80070057u32 as i32);

/// A minimal `IGraphicsEffect` describing a single D2D Exposure effect with one
/// source parameter ("src") and one property (`Exposure`, default 0.0 = 1×).
struct WhiteScaleEffect {
    source: IGraphicsEffectSource,
}

implement_decl! {
    impl WhiteScaleEffect as WhiteScaleEffect_Impl: [
        IGraphicsEffect,
        IGraphicsEffectSource,
        IGraphicsEffectD2D1Interop
    ]
}

impl IGraphicsEffectSource_Impl for WhiteScaleEffect_Impl {}

impl IGraphicsEffect_Impl for WhiteScaleEffect_Impl {
    fn Name(&self) -> windows_core::Result<HSTRING> {
        Ok(HSTRING::from(EFFECT_NAME))
    }
    fn SetName(&self, _name: &HSTRING) -> windows_core::Result<()> {
        // The name is fixed; the factory only reads it.
        Ok(())
    }
}

impl IGraphicsEffectD2D1Interop_Impl for WhiteScaleEffect_Impl {
    fn GetEffectId(&self) -> windows_core::Result<windows_core::GUID> {
        Ok(CLSID_D2D1Exposure)
    }

    fn GetNamedPropertyMapping(
        &self,
        name: &PCWSTR,
        index: *mut u32,
        mapping: *mut GRAPHICS_EFFECT_PROPERTY_MAPPING,
    ) -> windows_core::Result<()> {
        // The factory resolves "W.Exposure" to effect "W", then asks it to map
        // "Exposure" (case-insensitive, matching Win2D) to a D2D property.
        let wide = unsafe { name.as_wide() };
        let matches = wide.len() == EXPOSURE_PROP.len()
            && wide
                .iter()
                .zip(EXPOSURE_PROP.bytes())
                .all(|(&w, a)| u32::from(w) == u32::from(a.to_ascii_lowercase())
                    || u32::from(w) == u32::from(a.to_ascii_uppercase()));
        if !matches {
            return Err(windows_core::Error::from_hresult(E_INVALIDARG));
        }
        unsafe {
            // D2D1_EXPOSURE_PROP_EXPOSURE_VALUE = 0, passed through unchanged.
            *index = 0;
            *mapping = GRAPHICS_EFFECT_PROPERTY_MAPPING_DIRECT;
        }
        Ok(())
    }

    fn GetPropertyCount(&self) -> windows_core::Result<u32> {
        Ok(1)
    }

    fn GetProperty(&self, index: u32) -> windows_core::Result<windows_core::IInspectable> {
        if index == 0 {
            // Baked default: 0 stops (identity). Overridden per-brush through
            // the animatable "W.Exposure" property.
            PropertyValue::CreateSingle(0.0)
        } else {
            Err(windows_core::Error::from_hresult(E_INVALIDARG))
        }
    }

    fn GetSource(&self, index: u32) -> windows_core::Result<IGraphicsEffectSource> {
        if index == 0 {
            Ok(self.source.clone())
        } else {
            Err(windows_core::Error::from_hresult(E_INVALIDARG))
        }
    }

    fn GetSourceCount(&self) -> windows_core::Result<u32> {
        Ok(1)
    }
}

// ── UI-thread state ──────────────────────────────────────────────────────────

/// The once-per-thread effect factory (created with `"W.Exposure"` animatable)
/// and the one shared property set every wrapped brush's expression references.
struct UiState {
    factory: CompositionEffectFactory,
    props: CompositionPropertySet,
}

thread_local! {
    /// `None` inside = init failed once (old OS / factory error); wraps then
    /// permanently fall back to plain brushes with no retries and no logging.
    static STATE: OnceCell<Option<UiState>> = const { OnceCell::new() };
    /// Current exposure in stops (source of truth; the property set mirrors it).
    static EXPOSURE: Cell<f32> = const { Cell::new(0.0) };
    /// Live color brushes used as visual content fills (window backdrop), with
    /// their unscaled base colors. One entry per brush, updated in place — the
    /// backdrop brush lives for the window, so this stays a single-element list.
    static COLOR_BRUSHES: RefCell<Vec<(CompositionColorBrush, Color)>> =
        const { RefCell::new(Vec::new()) };
    /// The HMONITOR the window was last seen on (`0` = never queried), so
    /// WM_WINDOWPOSCHANGED only re-queries displayconfig on a monitor change.
    static LAST_MONITOR: Cell<isize> = const { Cell::new(0) };
}

fn init_state(compositor: &Compositor) -> windows_core::Result<UiState> {
    let source = CompositionEffectSourceParameter::Create(SOURCE_NAME)?;
    let effect: IGraphicsEffect = WhiteScaleEffect {
        source: source.cast()?,
    }
    .into();
    // The property must be declared animatable at factory creation for the
    // per-brush `StartAnimation("W.Exposure", …)` binding to take effect.
    let animatable = windows_collections::IIterable::<HSTRING>::from(vec![HSTRING::from(
        format!("{EFFECT_NAME}.{EXPOSURE_PROP}"),
    )]);
    let factory = compositor.CreateEffectFactoryWithProperties(&effect, &animatable)?;
    let props = compositor.CreatePropertySet()?;
    props.InsertScalar(SET_SCALAR, EXPOSURE.with(Cell::get))?;
    Ok(UiState { factory, props })
}

/// Wrap a surface brush in the shared white-level Exposure effect, returning
/// the brush to set as the visual's content. Pure passthrough (the plain brush,
/// zero extra cost) when the app has not opted in; on **any** failure (old OS,
/// factory error) it silently falls back to the plain brush.
pub(crate) fn wrap_surface_brush(
    compositor: &Compositor,
    brush: &crate::system_bindings::CompositionSurfaceBrush,
) -> windows_core::Result<CompositionBrush> {
    if ref_nits().is_some()
        && let Some(wrapped) = try_wrap(compositor, &brush.cast::<CompositionBrush>()?)
    {
        return Ok(wrapped);
    }
    brush.cast()
}

fn try_wrap(compositor: &Compositor, source: &CompositionBrush) -> Option<CompositionBrush> {
    STATE.with(|cell| {
        let state = cell.get_or_init(|| init_state(compositor).ok()).as_ref()?;
        let brush: CompositionEffectBrush = state.factory.CreateBrush().ok()?;
        brush.SetSourceParameter(SOURCE_NAME, source).ok()?;
        // Bind this brush's exposure to the shared set: one InsertScalar on the
        // set then rescales every wrapped brush, entirely compositor-side.
        let expr = compositor
            .CreateExpressionAnimationWithExpression(&format!("props.{SET_SCALAR}"))
            .ok()?;
        let anim: CompositionAnimation = expr.cast().ok()?;
        anim.SetReferenceParameter("props", &state.props).ok()?;
        brush
            .cast::<ICompositionObject>()
            .ok()?
            .StartAnimation(&format!("{EFFECT_NAME}.{EXPOSURE_PROP}"), &anim)
            .ok()?;
        brush.cast().ok()
    })
}

/// Register (or re-base) a color brush used as visual content fill. Color
/// brushes cannot be effect inputs, so their stored base color is rescaled by
/// the same linear multiply on every refresh. Applies the current scale
/// immediately; passthrough (base color verbatim) when the feature is off.
pub(crate) fn register_color_brush(brush: &CompositionColorBrush, base: Color) {
    COLOR_BRUSHES.with_borrow_mut(|brushes| {
        if let Some(slot) = brushes.iter_mut().find(|(b, _)| b == brush) {
            slot.1 = base;
        } else {
            brushes.push((brush.clone(), base));
        }
    });
    let _ = brush.SetColor(scale_color(base, EXPOSURE.with(Cell::get).exp2()));
}

/// Re-query the SDR white level of the monitor hosting `hwnd` and, if the
/// resulting exposure moved, write it into the shared property set (rescaling
/// every wrapped surface brush) and recolor registered color brushes. No-op
/// when the app has not opted in or the query fails (no change).
pub(crate) fn refresh_from_display(hwnd: HWND) {
    let Some(reference) = ref_nits() else { return };
    let Some(nits) = query_sdr_white_nits(hwnd) else { return };
    let exposure = exposure_for(nits, reference);
    if (exposure - EXPOSURE.with(Cell::get)).abs() <= 1e-3 {
        return;
    }
    EXPOSURE.with(|e| e.set(exposure));
    STATE.with(|cell| {
        if let Some(Some(state)) = cell.get() {
            let _ = state.props.InsertScalar(SET_SCALAR, exposure);
        }
    });
    let scale = exposure.exp2();
    COLOR_BRUSHES.with_borrow(|brushes| {
        for (brush, base) in brushes {
            let _ = brush.SetColor(scale_color(*base, scale));
        }
    });
}

/// `WM_WINDOWPOSCHANGED` helper: refresh only when the window's nearest monitor
/// actually changed (dragging between displays), keeping the common move/resize
/// path free of displayconfig queries.
pub(crate) fn refresh_if_monitor_changed(hwnd: HWND) {
    if ref_nits().is_none() {
        return;
    }
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) } as isize;
    if monitor != 0 && LAST_MONITOR.with(|c| c.replace(monitor)) != monitor {
        refresh_from_display(hwnd);
    }
}

// ── displayconfig query ──────────────────────────────────────────────────────

/// The SDR white level (nits) of the monitor nearest `hwnd`:
/// `MonitorFromWindow` → GDI device name → matching active displayconfig path →
/// `DISPLAYCONFIG_SDR_WHITE_LEVEL` (thousandths of the 80-nit scRGB unit).
/// On SDR displays this reports 80 nits. Any failure → `None` (no change).
fn query_sdr_white_nits(hwnd: HWND) -> Option<f32> {
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() {
            return None;
        }
        let mut info = MONITORINFOEXW {
            monitorInfo: MONITORINFO {
                cbSize: size_of::<MONITORINFOEXW>() as u32,
                ..MONITORINFO::default()
            },
            ..MONITORINFOEXW::default()
        };
        if !GetMonitorInfoW(monitor, &mut info.monitorInfo).as_bool() {
            return None;
        }

        let (mut n_paths, mut n_modes) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut n_paths, &mut n_modes).0 != 0 {
            return None;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); n_paths as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); n_modes as usize];
        if QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut n_paths,
            paths.as_mut_ptr(),
            &mut n_modes,
            modes.as_mut_ptr(),
            core::ptr::null_mut(),
        )
        .0 != 0
        {
            return None;
        }

        for path in paths.iter().take(n_paths as usize) {
            // Resolve the path's source to its GDI device name and match it
            // against the monitor's (e.g. `\\.\DISPLAY1`).
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                    adapterId: path.sourceInfo.adapterId,
                    id: path.sourceInfo.id,
                },
                ..DISPLAYCONFIG_SOURCE_DEVICE_NAME::default()
            };
            if DisplayConfigGetDeviceInfo(&mut source.header) != 0 {
                continue;
            }
            if source.viewGdiDeviceName != info.szDevice {
                continue;
            }
            let mut level = DISPLAYCONFIG_SDR_WHITE_LEVEL {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
                    size: size_of::<DISPLAYCONFIG_SDR_WHITE_LEVEL>() as u32,
                    adapterId: path.targetInfo.adapterId,
                    id: path.targetInfo.id,
                },
                SDRWhiteLevel: 0,
            };
            if DisplayConfigGetDeviceInfo(&mut level.header) != 0 {
                return None;
            }
            let nits = level.SDRWhiteLevel as f32 / 1000.0 * 80.0;
            return (nits > 0.0).then_some(nits);
        }
        None
    }
}

// ── Color-brush rescale (linear-light multiply of an 8-bit sRGB color) ───────

fn srgb_to_linear(u: f32) -> f32 {
    if u <= 0.04045 {
        u / 12.92
    } else {
        ((u + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(l: f32) -> f32 {
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// Scale an 8-bit sRGB color's luminance in linear light (matching the linear
/// multiply the Exposure effect applies to surface content), re-encoding to
/// sRGB. Alpha is untouched.
fn scale_color(c: Color, scale: f32) -> Color {
    if (scale - 1.0).abs() < 1e-4 {
        return c;
    }
    let ch = |u: u8| {
        let lin = srgb_to_linear(f32::from(u) / 255.0) * scale;
        (linear_to_srgb(lin).clamp(0.0, 1.0) * 255.0 + 0.5) as u8
    };
    Color {
        a: c.a,
        r: ch(c.r),
        g: ch(c.g),
        b: ch(c.b),
    }
}

#[cfg(test)]
mod tests {
    use super::{exposure_for, scale_color, Color, EXPOSURE_MAX, EXPOSURE_MIN};

    #[test]
    fn exposure_log2_and_clamp() {
        // Equal white levels = identity (0 stops).
        assert_eq!(exposure_for(203.0, 203.0), 0.0);
        // An SDR panel (80-nit SDR white) against a 203-nit reference darkens:
        // 2^exposure must equal 80/203 exactly (diffuse white lands on 1.0).
        let e = exposure_for(80.0, 203.0);
        assert!((e.exp2() - 80.0 / 203.0).abs() < 1e-6);
        assert!(e < 0.0);
        // Doubling the SDR white is exactly +1 stop.
        assert!((exposure_for(406.0, 203.0) - 1.0).abs() < 1e-6);
        // Clamped to the D2D Exposure effect's valid ±2-stop range.
        assert_eq!(exposure_for(10_000.0, 80.0), EXPOSURE_MAX);
        assert_eq!(exposure_for(1.0, 203.0), EXPOSURE_MIN);
        // Degenerate zero nits (log2 → -inf) still clamps.
        assert_eq!(exposure_for(0.0, 203.0), EXPOSURE_MIN);
    }

    #[test]
    fn color_scale_is_linear_light() {
        let c = Color { a: 255, r: 255, g: 255, b: 255 };
        // Identity scale returns the color unchanged.
        assert_eq!(scale_color(c, 1.0), c);
        // Halving in linear light: white → sRGB-encoded 0.5 linear ≈ 188.
        let half = scale_color(c, 0.5);
        assert_eq!(half.a, 255);
        assert!((half.r as i32 - 188).abs() <= 1, "got {}", half.r);
        // Boosting clamps at white instead of wrapping.
        assert_eq!(scale_color(c, 2.0).r, 255);
    }
}
