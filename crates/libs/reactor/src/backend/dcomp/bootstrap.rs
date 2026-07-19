//! Compositor bootstrap and the per-node visual/surface factory.
//!
//! A system `Windows.UI.Composition.Compositor` is rooted on the bare HWND via
//! `CreateDesktopWindowTarget`, a shared multi-threaded canvas [`GpuDevice`] is
//! exposed to it as a `CompositionGraphicsDevice`, and a **retained per-node
//! composition tree** is built under the root: every reactor node owns a
//! `ContainerVisual` (offset/size/opacity/clip set by layout), and every node
//! with painted chrome additionally owns a `SpriteVisual` backed by an FP16
//! (`R16G16B16A16Float`) `CompositionDrawingSurface` drawn **once** and redrawn
//! only when its own content or size changes. The system compositor composites
//! scRGB FP16 straight to DWM without clamping — whole-window HDR comes free, and
//! moving / fading / clipping a node is a compositor offset/opacity/clip change
//! with no repaint.

use std::cell::Cell;

use super::backdrop::{self, Backdrop};
use crate::system_bindings::{
    Color, CompositionColorBrush, CompositionDrawingSurface, CompositionGraphicsDevice,
    CompositionStretch, CompositionSurfaceBrush, Compositor, ContainerVisual, DesktopWindowTarget,
    DirectXAlphaMode, DirectXPixelFormat, ICompositionDrawingSurface2,
    ICompositionDrawingSurfaceInterop, ICompositionSurface, ICompositionTarget,
    ICompositorDesktopInterop, ICompositorInterop, IVisual, InsetClip, Size, SizeInt32,
    SpriteVisual, Visual, GetMonitorInfoW, MonitorFromWindow, HWND, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows_canvas_core::GpuDevice;
use windows_core::Interface;
use windows_numerics::{Vector2, Vector3};

/// A node's painted chrome: a `SpriteVisual` filled by an FP16 surface brush.
/// Created lazily for nodes that actually draw something; pure layout containers
/// never allocate one (a texture+visual per *painted* node, far cheaper than a
/// WinUI `UIElement`, and drawn once).
pub(crate) struct NodeSurface {
    pub sprite: SpriteVisual,
    pub interop: ICompositionDrawingSurfaceInterop,
    /// Current backing size in physical pixels.
    pub px: (i32, i32),
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

impl Compositing {
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
            px: (px_w, px_h),
            dip: (-1.0, -1.0),
            _surface: surface,
            _brush: brush,
        })
    }
}

impl NodeSurface {
    /// Resize the backing surface in place when the node's size changes, reusing
    /// the sprite/brush/visual (avoids churn). The sprite's DIP size is set by
    /// layout; this only matches the pixel buffer to it.
    pub fn resize(&mut self, px_w: i32, px_h: i32) -> windows_core::Result<()> {
        let (px_w, px_h) = (px_w.max(1), px_h.max(1));
        if (px_w, px_h) == self.px {
            return Ok(());
        }
        let surface2: ICompositionDrawingSurface2 = self.interop.cast()?;
        surface2.Resize(SizeInt32 {
            width: px_w,
            height: px_h,
        })?;
        self.px = (px_w, px_h);
        Ok(())
    }

    /// Set the sprite's presented (DIP) size.
    ///
    /// Gated on the last value pushed, for the reason [`resize`](Self::resize)
    /// is: the paint pass reaches every surface-bearing node on every repaint,
    /// and a node whose size did not change would otherwise pay a `cast` plus a
    /// cross-process `SetSize` per frame. A DIP size is independent of scale, so
    /// a DPI change resizes the pixel buffer without touching this.
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

    /// Set the sprite's offset (DIP) within its parent container.
    pub fn set_offset(&self, x: f32, y: f32) {
        if let Ok(v) = self.sprite.cast::<IVisual>() {
            let _ = v.SetOffset(Vector3::new(x, y, 0.0));
        }
    }

    /// Set the sprite's opacity (used to fade the scroll thumb in/out).
    pub fn set_opacity(&self, a: f32) {
        if let Ok(v) = self.sprite.cast::<IVisual>() {
            let _ = v.SetOpacity(a.clamp(0.0, 1.0));
        }
    }

    /// Snap the sprite's opacity to `a`, first stopping any in-flight
    /// compositor fade on it — a plain property set while an animation holds
    /// the property would otherwise be ignored.
    pub fn snap_opacity(&self, a: f32) {
        if let Ok(o) = self.sprite.cast::<crate::system_bindings::ICompositionObject>() {
            let _ = o.StopAnimation("Opacity");
        }
        self.set_opacity(a);
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
