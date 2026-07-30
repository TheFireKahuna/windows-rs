//! The device, which *is* the context.
//!
//! Direct2D charges a fixed cost per `BeginDraw`/`EndDraw` pair and a delayed Direct3D
//! device-context-state swap whenever a second context on the same device starts
//! drawing. Both are avoided the same way: there is one context per device, it is
//! created here, and no method hands one out or makes another. A cached intermediate is
//! rendered by retargeting the open bracket ([`Pass::draw`](crate::Pass::draw)), which
//! `SetTarget` permits at any time, including while the context is drawing.

use super::*;
use core::cell::Cell;
use std::rc::Rc;

/// A Direct2D device and its one device context, plus the Direct3D 11 device underneath.
///
/// Thread-affine for its whole life. [`Clone`] shares the same underlying objects — a
/// presentation group holds the device its regions draw through, and cloning is how it
/// does that without a second device — so a clone is emphatically not a second device
/// and stays on the thread that built the first.
#[derive(Clone)]
pub struct Gpu(Rc<Inner>);

struct Inner {
    factory: ID2D1Factory7,
    /// The Direct3D 11 device, held through its DXGI face because that is the one the
    /// Direct2D factory wants. COM identity belongs to the object rather than to the
    /// interface, so a sibling crate casting this to its own `ID3D11Device` projection
    /// gets the same device.
    dxgi: IDXGIDevice,
    device: ID2D1Device6,
    ctx: ID2D1DeviceContext6,
    /// Whether a [`Pass`](crate::Pass) is open. A second one is an error rather than a
    /// silently corrupted context: Direct2D documents a doubled `BeginDraw` as putting
    /// the target into an error state where nothing further draws, reported only at
    /// `EndDraw`.
    drawing: Cell<bool>,
}

/// The descending feature-level ladder, which is what `D3D11CreateDevice` is meant to be
/// given: it walks the list and returns the first level the machine supports. A
/// single-entry array would be simultaneously a floor no weaker GPU could meet and a
/// ceiling no stronger one could exceed.
const LEVELS: [D3D_FEATURE_LEVEL; 7] = [
    D3D_FEATURE_LEVEL_11_1,
    D3D_FEATURE_LEVEL_11_0,
    D3D_FEATURE_LEVEL_10_1,
    D3D_FEATURE_LEVEL_10_0,
    D3D_FEATURE_LEVEL_9_3,
    D3D_FEATURE_LEVEL_9_2,
    D3D_FEATURE_LEVEL_9_1,
];

impl Gpu {
    /// The window thread's device.
    pub fn for_window() -> Result<Self> {
        Self::new(0)
    }

    /// A present thread's device. Additionally suppresses the display driver's own
    /// worker pool, which a presentation device does not want and pays for per device.
    pub fn for_presentation() -> Result<Self> {
        Self::new(D3D11_CREATE_DEVICE_PREVENT_INTERNAL_THREADING_OPTIMIZATIONS)
    }

    fn new(extra: D3D11_CREATE_DEVICE_FLAG) -> Result<Self> {
        let flags =
            (D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_SINGLETHREADED | extra) as u32;
        // One fallback, for the whole device, and then a hard error. A machine with no
        // hardware Direct3D at all still renders through WARP; a machine where WARP also
        // fails has no graphics stack to host a window on.
        let d3d = d3d11(D3D_DRIVER_TYPE_HARDWARE, flags)
            .or_else(|_| d3d11(D3D_DRIVER_TYPE_WARP, flags))?;
        let dxgi: IDXGIDevice = d3d.cast()?;
        let factory = factory()?;
        let device = unsafe { factory.CreateDevice(&dxgi)? };
        let ctx = unsafe { device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)? };
        configure(&ctx)?;
        Ok(Self(Rc::new(Inner {
            factory,
            dxgi,
            device,
            ctx,
            drawing: Cell::new(false),
        })))
    }

    /// The Direct2D device, for a compositor's graphics-device interop.
    ///
    /// One `Gpu` per compositor, and only this one: when the compositor realizes a
    /// [`CompositionPath`] it asks the geometry source for geometry belonging to a
    /// factory of its own choosing, and nothing on either side of that callback can
    /// verify the match. A geometry from a different `Gpu` surfaces as content that
    /// never appears, not as an error.
    ///
    /// [`CompositionPath`]: https://learn.microsoft.com/uwp/api/windows.ui.composition.compositionpath
    pub fn d2d(&self) -> &impl Interface {
        &self.0.device
    }

    /// The Direct3D 11 device, for a presentation region's own buffer allocation. Cast
    /// it to the caller's `ID3D11Device` projection; it is the same object.
    pub fn d3d(&self) -> &impl Interface {
        &self.0.dxgi
    }

    /// Opens the drawing bracket. `Err` if one is already open on this device.
    pub fn pass(&self) -> Result<Pass<'_>> {
        if self.0.drawing.replace(true) {
            return Err(windows_core::Error::from_hresult(E_INVALIDARG));
        }
        unsafe { self.0.ctx.BeginDraw() };
        Ok(Pass::new(self))
    }

    pub(crate) fn ctx(&self) -> &ID2D1DeviceContext6 {
        &self.0.ctx
    }

    pub(crate) fn factory(&self) -> &ID2D1Factory7 {
        &self.0.factory
    }

    pub(crate) fn drawing(&self) -> &Cell<bool> {
        &self.0.drawing
    }
}

fn factory() -> Result<ID2D1Factory7> {
    let mut out = core::ptr::null_mut();
    unsafe {
        D2D1CreateFactory(
            D2D1_FACTORY_TYPE_SINGLE_THREADED,
            &ID2D1Factory7::IID,
            core::ptr::null(),
            &mut out,
        )
        .ok()?;
        Ok(ID2D1Factory7::from_raw(out))
    }
}

fn d3d11(kind: D3D_DRIVER_TYPE, flags: u32) -> Result<windows_core::IUnknown> {
    let mut device = core::ptr::null_mut();
    unsafe {
        D3D11CreateDevice(
            core::ptr::null_mut(),
            kind,
            core::ptr::null_mut(),
            flags,
            LEVELS.as_ptr(),
            LEVELS.len() as u32,
            D3D11_SDK_VERSION as u32,
            &mut device,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
        )
        .ok()?;
        Ok(windows_core::IUnknown::from_raw(device))
    }
}

/// Probes the two capabilities this stack has no fallback for, then sets the context
/// state that is a property of the pipeline rather than of a frame.
///
/// Shared with the composition bridge, because a composition drawing surface hands back
/// a context created for that call rather than this one — so the same settings have to be
/// restated there, and restating them from one function is how they cannot drift.
pub(crate) fn configure(ctx: &ID2D1DeviceContext6) -> Result<()> {
    unsafe {
        // Feature-level 9 hardware may support neither, and quietly falling back to 8
        // bits per channel is the exact failure the FP16 pipeline exists to prevent: the
        // surface becomes a colour-managed island whose transform we do not control.
        // There is no fallback path in this stack, so the honest report is a hard error
        // at construction rather than washed-out colour at run time.
        if !ctx.IsDxgiFormatSupported(FORMAT).as_bool()
            || !ctx
                .IsBufferPrecisionSupported(D2D1_BUFFER_PRECISION_16BPC_FLOAT)
                .as_bool()
        {
            return Err(windows_core::Error::from_hresult(E_FAIL));
        }

        ctx.SetPrimitiveBlend(D2D1_PRIMITIVE_BLEND_SOURCE_OVER);

        // DIPs, everywhere, for the whole life of the device — the one coordinate space this
        // crate has. Direct2D applies the DPI scale by default in this mode, which is what
        // stops every call site from having to discover and apply the scalar itself; the
        // documentation names that as one of the simplest reasons applications get high-DPI
        // wrong. It is also the space DirectWrite measures in, so a glyph run needs no
        // hand-scaled em size, and the space layout solves in.
        //
        // Pixels are available and are *not* used. They would suit a cache cell, whose
        // identity is a pixel extent — but the cell's radius and its glyph sizes are
        // authored in DIPs, so pixel space would convert them down only for Direct2D to
        // scale them back up. Allocation is in pixels; coordinates never are.
        ctx.SetUnitMode(D2D1_UNIT_MODE_DIPS);

        // ClearType on a surface with an alpha channel produces unpredictable results,
        // and Direct2D switches away from it on its own for any alpha mode but IGNORE.
        // Stating it means a target that *does* ignore alpha gets the same glyphs.
        ctx.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);

        // Direct2D "makes no guarantees about if or where" it materializes an effect
        // graph's intermediates, and their default precision is limited-range — which
        // would silently clamp every extended-range value passing through one. Setting
        // it here, before any caller sees the context, means a graph built later inherits
        // the right precision by default and cannot be built without it.
        //
        // Read-modify-write rather than construct: the struct also carries a tile
        // allocation size, and zero is rejected outright, so the only way to change
        // precision without inventing a tile size is to put back the one Direct2D chose.
        let mut controls = ctx.GetRenderingControls();
        controls.bufferPrecision = D2D1_BUFFER_PRECISION_16BPC_FLOAT;
        ctx.SetRenderingControls(&controls);
    }
    Ok(())
}

/// What a failing `HRESULT` means for recovery. Two outcomes and not a boolean, because
/// the two have independent scopes: a lost target does not cost the device, and a lost
/// device costs every target, brush, path and realization built from it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Loss {
    /// Not a loss.
    None,
    /// Rebuild targets. The device, and everything else created from it, survives.
    RecreateTarget,
    /// Rebuild the [`Gpu`] and every resource under it.
    DeviceRemoved,
}

/// Classifies a failing `HRESULT` into a recovery domain.
#[must_use]
pub fn classify(hr: windows_core::HRESULT) -> Loss {
    if hr == D2DERR_RECREATE_TARGET {
        Loss::RecreateTarget
    } else if hr == DXGI_ERROR_DEVICE_REMOVED
        || hr == DXGI_ERROR_DEVICE_RESET
        || hr == DXGI_ERROR_DEVICE_HUNG
        || hr == DXGI_ERROR_DRIVER_INTERNAL_ERROR
    {
        Loss::DeviceRemoved
    } else {
        Loss::None
    }
}
