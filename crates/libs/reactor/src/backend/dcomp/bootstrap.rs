//! Compositor bootstrap and the per-node visual/surface factory.
//!
//! A system `Windows.UI.Composition.Compositor` is rooted on the bare HWND via
//! `CreateDesktopWindowTarget`, a shared multi-threaded canvas [`GpuDevice`] is
//! exposed to it as a `CompositionGraphicsDevice`, and a **retained per-node
//! composition tree** is built under the root: every reactor node owns a
//! `ContainerVisual` (offset/size/opacity/clip set by layout) whose chrome is
//! retained compositor objects — nine-grid sprites, vector sprite shapes and
//! glyph sprites — sourced from shared, cached FP16 (`R16G16B16A16Float`)
//! rasters. The system compositor composites scRGB FP16 straight to DWM without
//! clamping, so whole-window HDR comes free, and moving / fading / clipping a
//! node is a compositor offset/opacity/clip change with no repaint at all.
//!
//! Nodes used to own a drawing surface each, redrawn whenever their content or
//! size changed. [`NodeSurface`] is what remains of that, and only the popup
//! still mints one.

use std::cell::Cell;

use super::backdrop::{self, Backdrop};
use crate::system_bindings::{
    Color, CompositionColorBrush, CompositionDrawingSurface, CompositionGraphicsDevice,
    CompositionStretch, CompositionSurfaceBrush, Compositor, ContainerVisual, DesktopWindowTarget,
    DirectXAlphaMode, DirectXPixelFormat, ICompositionDrawingSurfaceInterop,
    ICompositionSurface, ICompositionTarget, ICompositorDesktopInterop, ICompositorInterop,
    IVisual, InsetClip, Size, SpriteVisual, Visual, GetMonitorInfoW, MonitorFromWindow, HWND,
    MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_canvas_core::GpuDevice;
use windows_core::Interface;
use windows_numerics::{Vector2, Vector3};

/// A drawn surface: a `SpriteVisual` filled by an FP16 surface brush.
///
/// The library's one remaining immediate-mode surface, and it belongs to the
/// popup ([`super::popup`]) — a flyout's list is drawn in one pass because its
/// rows are transient, unparented content rather than arena nodes with retained
/// chrome of their own.
///
/// Every OTHER control's appearance is compositor objects. This type used to be
/// per-node chrome, minted for anything that drew; nothing else draws now.
pub(crate) struct NodeSurface {
    pub sprite: SpriteVisual,
    pub interop: ICompositionDrawingSurfaceInterop,
    /// Presented size in DIPs, as last pushed to the sprite. Born deliberately
    /// negative so the first push always lands.
    dip: (f32, f32),
    // Held so the surface + brush outlive the interop handle and the sprite's
    // brush binding.
    _surface: CompositionDrawingSurface,
    _brush: CompositionSurfaceBrush,
}

/// All window-level composition state. Owns the shared GpuDevice, the system
/// compositor, the desktop target, and the scaled root container. Per-node
/// visuals/surfaces are minted from here but owned by their nodes.
pub(crate) struct Compositing {
    // Held so the shared D3D/D2D device outlives the graphics device and surfaces.
    #[allow(dead_code)]
    pub gpu: GpuDevice,
    /// Latched when any rasterizer sees a device-loss HRESULT, and cleared only
    /// by the frame that acts on it ([`Self::took_device_loss`]).
    ///
    /// It LATCHES rather than being reset before each draw, which is what it
    /// used to do: several sources can rasterize in one frame, and clearing the
    /// flag at the top of each one meant a loss reported by the first was erased
    /// by the second. Nothing raised it at all until the node painter was
    /// retired — loss was noticed only because that painter propagated its
    /// `BeginDraw` error out of the walk, so the flag beside it was dead. Every
    /// raster path reports here now.
    pub device_lost: Cell<bool>,
    compositor: Compositor,
    graphics: CompositionGraphicsDevice,
    /// The scaled root: a `SetScale(dpi/96)` makes every descendant work in DIPs.
    root: ContainerVisual,
    /// Opaque window background, kept at the bottom of the root (in DIPs).
    bg: SpriteVisual,
    bg_brush: CompositionColorBrush,
    /// The band the backdrop's layers live in — above `bg`, below the reactor
    /// tree, rooted once so a rebuild cannot disturb z-order.
    backdrop_host: ContainerVisual,
    /// Kept for [`Self::monitor_px`] — screen-fixed backdrop layers size to the
    /// monitor the window is on.
    hwnd: HWND,
    /// The app's backdrop layer stack. Built here (before the window is shown)
    /// rather than as a reactor element, so it is present on the very first
    /// composited frame.
    backdrop: Option<Backdrop>,
    _target: DesktopWindowTarget,
    dip_size: (f32, f32),
    scale: f32,
}

/// Whether an HRESULT means the GPU device is gone and every resource built on
/// it has to be rebuilt.
///
/// `D2DERR_RECREATE_TARGET` is the one Direct2D reports from `EndDraw`;
/// `DXGI_ERROR_DEVICE_REMOVED` / `_RESET` come from the layer below when the
/// adapter itself was lost (a driver reset, a TDR, an external GPU unplugged).
/// Any other failure is a bad call, not a lost device, and must NOT trigger a
/// rebuild — the rebuild would fail identically and the frame would loop.
pub(crate) fn is_device_loss(e: &windows_core::Error) -> bool {
    matches!(e.code().0 as u32, 0x8899_000C | 0x887A_0005 | 0x887A_0007)
}

impl Compositing {
    /// Latch a rasterizer's failure if it was a device loss. Every `BeginDraw` /
    /// `EndDraw` failure path calls this; the ones that are not device loss fall
    /// through and the caller's own `None` handles them.
    pub(crate) fn note_error(&self, e: &windows_core::Error) {
        if is_device_loss(e) {
            self.device_lost.set(true);
        }
    }

    /// Consume the latch: `true` once per loss, for the frame that rebuilds.
    pub(crate) fn took_device_loss(&self) -> bool {
        self.device_lost.replace(false)
    }

    pub fn new(hwnd: HWND, pixel_w: i32, pixel_h: i32, dpi: f32) -> windows_core::Result<Self> {
        let gpu = GpuDevice::new_multi_threaded()?;

        let compositor = Compositor::new()?;
        let interop: ICompositorInterop = compositor.cast()?;
        // The interop factories hand back the WinRT *interface*; the concrete
        // runtime class is one QI away.
        let graphics: CompositionGraphicsDevice =
            unsafe { interop.CreateGraphicsDevice(gpu.d2d_device())? }.cast()?;

        let desktop: ICompositorDesktopInterop = compositor.cast()?;
        let target: DesktopWindowTarget =
            unsafe { desktop.CreateDesktopWindowTarget(hwnd, false)? }.cast()?;
        let root = compositor.CreateContainerVisual()?;
        target.cast::<ICompositionTarget>()?.SetRoot(&root)?;

        let scale = (dpi / 96.0).max(0.01);
        let dip_size = (pixel_w.max(1) as f32 / scale, pixel_h.max(1) as f32 / scale);
        // The whole tree is authored in DIPs; one root scale rasterizes to pixels.
        root.cast::<IVisual>()?
            .SetScale(Vector3::new(scale, scale, 1.0))?;

        // Opaque window background (bottom-most visual), sized in DIPs. It sits
        // behind the app's own opaque full-window backdrop, so it needs no display
        // colour mapping — a plain colour brush.
        let bg = compositor.CreateSpriteVisual()?;
        let bg_brush = compositor.CreateColorBrushWithColor(WINDOW_BG)?;
        bg.SetBrush(&bg_brush.cast::<crate::system_bindings::CompositionBrush>()?)?;
        bg.cast::<IVisual>()?
            .SetSize(Vector2::new(dip_size.0, dip_size.1))?;
        root.Children()?.InsertAtTop(&bg.cast::<Visual>()?)?;

        // The backdrop's own band, rooted ONCE directly above the window
        // background. Rebuilding the backdrop (a display change re-fits its
        // colours) only ever repopulates this container, so it can never land
        // above the reactor tree the way a fresh `InsertAtTop` on the root
        // would once content is attached.
        let backdrop_host = compositor.CreateContainerVisual()?;
        root.Children()?
            .InsertAtTop(&backdrop_host.cast::<Visual>()?)?;

        let mut this = Self {
            gpu,
            device_lost: Cell::new(false),
            compositor,
            graphics,
            root,
            bg,
            bg_brush,
            backdrop_host,
            hwnd,
            backdrop: None,
            _target: target,
            dip_size,
            scale,
        };
        // Every layer is painted and rooted before this returns, and the caller
        // runs before `ShowWindow` — so there is no frame in which the window is
        // mapped without its backdrop.
        this.build_backdrop();
        Ok(this)
    }

    /// (Re)build the app's backdrop from the registered provider, replacing any
    /// existing one. Called at startup and whenever the display's colour
    /// capability may have changed — the provider re-queries the app's colour
    /// fit, so the backdrop can never be left holding a stale mapping.
    pub fn build_backdrop(&mut self) {
        let Some(spec) = backdrop::spec() else {
            if let Some(old) = self.backdrop.take() {
                old.remove(self);
            }
            return;
        };
        // Build the replacement BEFORE detaching the incumbent, and only then
        // swap. Tearing down first leaves a frame with no backdrop at all — not
        // a theoretical race: painting the new layers runs `BeginDraw`/`EndDraw`,
        // which flushes the compositor batch, so the empty state is guaranteed to
        // be composited. The new layers are opaque and land on top of the old, so
        // the incumbent is covered the instant they exist.
        let Some(built) = Backdrop::build(self, &spec, self.dip_size, self.scale, self.monitor_px())
        else {
            // Keep whatever is already up: a backdrop fitted for the previous
            // display beats no backdrop.
            return;
        };
        if let Some(old) = self.backdrop.take() {
            old.remove(self);
        }
        self.backdrop = Some(built);
    }

    /// The pixel size of the monitor this window is on. Screen-fixed backdrop
    /// layers are allocated at this size so no window resize — up to and
    /// including a maximize — can outgrow them and force a repaint.
    fn monitor_px(&self) -> (i32, i32) {
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let mon = unsafe { MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST) };
        if mon.is_null() || !unsafe { GetMonitorInfoW(mon, &mut info) }.as_bool() {
            // No monitor answer: fall back to the window itself. A later display
            // change rebuilds the backdrop and gets another chance at this.
            return (
                ((self.dip_size.0 * self.scale).round() as i32).max(1),
                ((self.dip_size.1 * self.scale).round() as i32).max(1),
            );
        }
        let r = info.rcMonitor;
        ((r.right - r.left).max(1), (r.bottom - r.top).max(1))
    }

    /// Insert one backdrop layer at the top of the backdrop band. Layers are
    /// added bottom-up, so call order is z-order.
    pub(crate) fn attach_backdrop_visual(&self, v: &Visual) -> windows_core::Result<()> {
        self.backdrop_host.Children()?.InsertAtTop(v)?;
        Ok(())
    }

    /// Remove one backdrop layer.
    pub(crate) fn remove_backdrop_visual(&self, v: &Visual) {
        if let Ok(children) = self.backdrop_host.Children() {
            let _ = children.Remove(v);
        }
    }

    pub fn dip_size(&self) -> (f32, f32) {
        self.dip_size
    }

    /// The system compositor — the seam a downstream crate draws through: it
    /// builds a `CompositionSurfaceFactory::from_compositor` with this and mints
    /// child surfaces under node containers (`get_native_element`). Consumed by
    /// embedders; unused in-crate for now.
    #[allow(dead_code)]
    pub fn compositor(&self) -> &Compositor {
        &self.compositor
    }

    /// Re-fold a new DPI / pixel size: rescale the root and resize the window
    /// background. Per-node surfaces are rebuilt by the paint pass.
    pub fn set_scale_and_pixels(&mut self, pixel_w: i32, pixel_h: i32, dpi: f32) {
        self.scale = (dpi / 96.0).max(0.01);
        self.dip_size = (
            pixel_w.max(1) as f32 / self.scale,
            pixel_h.max(1) as f32 / self.scale,
        );
        if let Ok(v) = self.root.cast::<IVisual>() {
            let _ = v.SetScale(Vector3::new(self.scale, self.scale, 1.0));
        }
        if let Ok(v) = self.bg.cast::<IVisual>() {
            let _ = v.SetSize(Vector2::new(self.dip_size.0, self.dip_size.1));
        }
        // Cheap by construction: a few visual property writes, no repaint and no
        // animation restart — see `Backdrop::place`.
        if let Some(b) = &self.backdrop {
            b.place(self.dip_size, self.scale);
        }
    }

    /// Recolor the window background (theme change). The backdrop sits behind the
    /// app's own opaque backdrop, so it takes the colour verbatim — no display map.
    pub fn set_background(&self, color: Color) {
        let _ = self.bg_brush.SetColor(color);
    }

    /// Attach a reactor root node's container directly above the background.
    pub fn attach_root(&self, container: &ContainerVisual) -> windows_core::Result<()> {
        if let Ok(v) = container.cast::<Visual>() {
            self.root.Children()?.InsertAtTop(&v)?;
        }
        Ok(())
    }

    /// Detach a previously attached root node container.
    pub fn detach_root(&self, container: &ContainerVisual) {
        if let (Ok(children), Ok(v)) = (self.root.Children(), container.cast::<Visual>()) {
            let _ = children.Remove(&v);
        }
    }

    /// Attach an arbitrary visual at the top of the compositor root (exit-ghost
    /// snapshot sprites / fallback containers).
    pub fn attach_root_visual(&self, v: &Visual) -> windows_core::Result<()> {
        self.root.Children()?.InsertAtTop(v)?;
        Ok(())
    }

    /// Remove a visual previously attached by [`Self::attach_root_visual`].
    pub fn remove_root_visual(&self, v: &Visual) {
        if let Ok(children) = self.root.Children() {
            let _ = children.Remove(v);
        }
    }

    /// Canonical COM identity of the compositor root container. Terminates the
    /// exit-ghost parent walk (a visual whose parent chain doesn't reach this
    /// is already detached, so it must not be ghosted again).
    pub fn root_identity(&self) -> *mut core::ffi::c_void {
        self.root
            .cast::<windows_core::IUnknown>()
            .map(|u| u.as_raw())
            .unwrap_or(core::ptr::null_mut())
    }

    /// Mint a bare container visual for a node (pure layout, no surface).
    pub fn new_container(&self) -> windows_core::Result<ContainerVisual> {
        self.compositor.CreateContainerVisual()
    }

    /// Mint a top-level overlay container (popup host) above the reactor tree,
    /// backed by one FP16 surface of `px_w`×`px_h`. The container is inserted at
    /// the very top of the compositor root so it draws above all content.
    pub fn new_overlay(
        &self,
        px_w: i32,
        px_h: i32,
    ) -> windows_core::Result<(ContainerVisual, NodeSurface)> {
        let container = self.compositor.CreateContainerVisual()?;
        self.root
            .Children()?
            .InsertAtTop(&container.cast::<Visual>()?)?;
        let surf = self.new_surface(&container, px_w, px_h)?;
        Ok((container, surf))
    }

    /// Remove an overlay container previously inserted by [`Self::new_overlay`].
    pub fn remove_overlay(&self, container: &ContainerVisual) {
        if let (Ok(children), Ok(v)) = (self.root.Children(), container.cast::<Visual>()) {
            let _ = children.Remove(&v);
        }
    }

    /// The DIP scale (`dpi/96`) the root applies — popups draw in DIPs too.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Mint an inset clip that tracks a visual's own bounds (for scroll/overflow).
    pub fn new_inset_clip(&self) -> windows_core::Result<InsetClip> {
        self.compositor.CreateInsetClip()
    }

    /// Mint a bare sprite visual (chrome parts). Not inserted anywhere — the
    /// caller parents it at the right band position.
    pub fn new_sprite(&self) -> windows_core::Result<SpriteVisual> {
        self.compositor.CreateSpriteVisual()
    }

    /// Mint a nine-grid (9-slice) brush for a stretchable chrome-part source.
    pub fn new_nine_grid(
        &self,
    ) -> windows_core::Result<crate::system_bindings::CompositionNineGridBrush> {
        use windows_core::Interface;
        self.compositor
            .cast::<crate::system_bindings::ICompositor2>()?
            .CreateNineGridBrush()
    }

    /// Mint an FP16 atlas-source surface of exact pixel size, returning the
    /// surface, its draw interop, and a Fill-stretch brush over it.
    pub fn new_source_surface(
        &self,
        px_w: i32,
        px_h: i32,
    ) -> windows_core::Result<(
        CompositionDrawingSurface,
        ICompositionDrawingSurfaceInterop,
        CompositionSurfaceBrush,
    )> {
        self.new_surface_with_format(px_w, px_h, DirectXPixelFormat::R16G16B16A16Float)
    }

    /// [`new_source_surface`](Self::new_source_surface) at an explicit pixel
    /// format. FP16 is right for anything whose COLOUR the surface carries; a
    /// pure alpha MASK (the glyph atlas) carries no colour at all and asks for
    /// `A8UIntNormalized` here instead — an 8× cut in bytes per glyph. The
    /// caller owns the fallback: `CreateDrawingSurface` fails for a format the
    /// composition device will not accept, and that failure is the only reliable
    /// probe (see `glyph_atlas::GlyphAtlas::format`).
    pub fn new_surface_with_format(
        &self,
        px_w: i32,
        px_h: i32,
        format: DirectXPixelFormat,
    ) -> windows_core::Result<(
        CompositionDrawingSurface,
        ICompositionDrawingSurfaceInterop,
        CompositionSurfaceBrush,
    )> {
        let surface = self.graphics.CreateDrawingSurface(
            Size {
                width: px_w.max(1) as f32,
                height: px_h.max(1) as f32,
            },
            format,
            DirectXAlphaMode::Premultiplied,
        )?;
        let brush = self
            .compositor
            .CreateSurfaceBrushWithSurface(&surface.cast::<ICompositionSurface>()?)?;
        let _ = brush.SetStretch(CompositionStretch::Fill);
        let interop: ICompositionDrawingSurfaceInterop = surface.cast()?;
        Ok((surface, interop, brush))
    }

    /// Create (or recreate) a node's painted-chrome surface at `px` pixels and
    /// insert its sprite at the bottom of `parent` (behind child-node visuals).
    pub fn new_surface(
        &self,
        parent: &ContainerVisual,
        px_w: i32,
        px_h: i32,
    ) -> windows_core::Result<NodeSurface> {
        self.new_surface_at(parent, px_w, px_h, false)
    }

    /// Like [`new_surface`](Self::new_surface) but inserts the sprite at the top of
    /// `parent` (above child-node visuals) — used for overlay chrome such as the
    /// ScrollViewer thumb that must paint over the scrolled content.
    pub fn new_surface_at(
        &self,
        parent: &ContainerVisual,
        px_w: i32,
        px_h: i32,
        at_top: bool,
    ) -> windows_core::Result<NodeSurface> {
        let (px_w, px_h) = (px_w.max(1), px_h.max(1));
        let surface = self.graphics.CreateDrawingSurface(
            Size {
                width: px_w as f32,
                height: px_h as f32,
            },
            DirectXPixelFormat::R16G16B16A16Float,
            DirectXAlphaMode::Premultiplied,
        )?;
        let brush = self
            .compositor
            .CreateSurfaceBrushWithSurface(&surface.cast::<ICompositionSurface>()?)?;
        let _ = brush.SetStretch(CompositionStretch::Fill);
        // Node-chrome surfaces paint through the app's draw-time colour map (see
        // `node::linear`), so the surface brush is used raw — no compositor effect.
        let content = brush.cast::<crate::system_bindings::CompositionBrush>()?;

        let sprite = self.compositor.CreateSpriteVisual()?;
        sprite.SetBrush(&content)?;
        // Bottom of the parent so the node's own chrome sits behind its children;
        // top for an overlay (the scroll thumb draws over the scrolled content).
        let children = parent.Children()?;
        let v = sprite.cast::<Visual>()?;
        if at_top {
            children.InsertAtTop(&v)?;
        } else {
            children.InsertAtBottom(&v)?;
        }

        let interop: ICompositionDrawingSurfaceInterop = surface.cast()?;
        Ok(NodeSurface {
            sprite,
            interop,
            dip: (-1.0, -1.0),
            _surface: surface,
            _brush: brush,
        })
    }
}

impl NodeSurface {
    /// Set the sprite's presented (DIP) size.
    ///
    /// Gated on the last value pushed: a popup reaching this with an unchanged
    /// size would otherwise pay a `cast` plus a cross-process `SetSize` for
    /// nothing.
    pub fn set_dip_size(&mut self, w: f32, h: f32) {
        let (w, h) = (w.max(0.0), h.max(0.0));
        if (w, h) == self.dip {
            return;
        }
        if let Ok(v) = self.sprite.cast::<IVisual>() {
            let _ = v.SetSize(Vector2::new(w, h));
            self.dip = (w, h);
        }
    }

}

/// The opaque window background (dark), composited beneath the reactor tree. This is
/// a raw WinRT `Windows.UI.Color` (8-bit sRGB) handed to the compositor's color
/// brush, not a reactor theming `Color`, so it stays a plain ARGB literal.
///
/// It is deliberately NOT tuned to match whatever backdrop sits above it. 8-bit
/// cannot represent a display-fitted base anyway (an HDR fit exceeds 1.0), and a
/// near-match would only disguise a backdrop that failed to cover the window —
/// which is a bug worth seeing, not hiding.
const WINDOW_BG: Color = Color {
    a: 255,
    r: 14,
    g: 14,
    b: 17,
};
