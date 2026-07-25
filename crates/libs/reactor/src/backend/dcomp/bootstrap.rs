//! Compositor bootstrap and the per-node visual/surface factory.
//!
//! A system `Windows.UI.Composition.Compositor` is rooted on the bare HWND via
//! `CreateDesktopWindowTarget`, a shared multi-threaded canvas [`GpuDevice`] is
//! exposed to it as a `CompositionGraphicsDevice`, and a **retained per-node
//! composition tree** is built under the root: every reactor node owns a
//! `ContainerVisual` (offset/size/opacity/clip set by layout) whose chrome is
//! retained compositor objects — nine-grid sprites, vector sprite shapes and
//! glyph sprites — sourced from shared, cached FP16 (`Rgba16Float`) rasters. The
//! system compositor composites scRGB FP16 straight to DWM without clamping, so
//! whole-window HDR comes free, and moving / fading / clipping a node is a
//! compositor offset/opacity/clip change with no repaint at all.
//!
//! Nodes used to own a drawing surface each, redrawn whenever their content or
//! size changed. [`NodeSurface`] is what remains of that, and only the popup
//! still mints one.
//!
//! Every composition object here comes from the safe [`windows_composition`]
//! wrapper; `crate::system_bindings` is reached for only the plain Win32 the
//! bootstrap also needs (the window handle and its monitor).

use std::cell::Cell;

use super::backdrop::{self, Backdrop};
use super::census::HeatMap;
use crate::system_bindings::{
    GetMonitorInfoW, MonitorFromWindow, HWND, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_canvas::GpuDevice;
use windows_composition::{
    AlphaMode, Color, CompositionColorBrush, CompositionDrawingSurface, CompositionGraphicsDevice,
    CompositionNineGridBrush, CompositionSurfaceBrush, CompositionVirtualDrawingSurface, Compositor,
    ContainerVisual, DesktopWindowTarget, InsetClip, PixelFormat, SpriteVisual, Stretch, Visual,
};
use windows_numerics::Vector3;

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
    /// The drawn-into surface. It carries its own `BeginDraw`/`EndDraw` bracket
    /// (this used to be a separately stored `ICompositionDrawingSurfaceInterop`),
    /// so holding the surface is both the keep-alive and the draw seam.
    pub surface: CompositionDrawingSurface,
    /// Presented size in DIPs, as last pushed to the sprite. Born deliberately
    /// negative so the first push always lands.
    dip: (f32, f32),
    // Held so the brush outlives the sprite's brush binding.
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
    /// The scaled root: a `set_scale(dpi/96)` makes every descendant work in DIPs.
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
        // The canvas device is the accepting seam: the wrapper does the
        // `ICompositorInterop` QI and the graphics-device QI internally, in the
        // same order the raw calls did.
        let graphics = compositor.create_graphics_device(gpu.d2d_device())?;

        // SAFETY: `hwnd` is the window the host created on this thread moments
        // ago and keeps alive for the whole life of this `Compositing`. The
        // backend mints its own HWND, so the `windows_window::Window` overload
        // does not apply.
        let target = unsafe { compositor.create_desktop_window_target_for_hwnd(hwnd, false)? };
        let root = compositor.create_container_visual();
        target.set_root(&root);

        let scale = (dpi / 96.0).max(0.01);
        let dip_size = (pixel_w.max(1) as f32 / scale, pixel_h.max(1) as f32 / scale);
        // The whole tree is authored in DIPs; one root scale rasterizes to pixels.
        root.set_scale(Vector3::new(scale, scale, 1.0));

        // Opaque window background (bottom-most visual), sized in DIPs. It sits
        // behind the app's own opaque full-window backdrop, so it needs no display
        // colour mapping — a plain colour brush.
        let bg = compositor.create_sprite_visual();
        let bg_brush = compositor.create_color_brush(WINDOW_BG);
        bg.set_brush(&bg_brush);
        bg.set_size(dip_size.0, dip_size.1);
        root.children().insert_at_top(&bg);

        // The backdrop's own band, rooted ONCE directly above the window
        // background. Rebuilding the backdrop (a display change re-fits its
        // colours) only ever repopulates this container, so it can never land
        // above the reactor tree the way a fresh `insert_at_top` on the root
        // would once content is attached.
        let backdrop_host = compositor.create_container_visual();
        root.children().insert_at_top(&backdrop_host);

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
    pub(crate) fn attach_backdrop_visual(&self, v: &Visual) {
        self.backdrop_host.children().insert_at_top(v);
    }

    /// Remove one backdrop layer.
    pub(crate) fn remove_backdrop_visual(&self, v: &Visual) {
        self.backdrop_host.children().remove(v);
    }

    pub fn dip_size(&self) -> (f32, f32) {
        self.dip_size
    }

    /// The system compositor — the seam a downstream crate draws through, e.g. to
    /// parent its own visuals under node containers (`get_native_element`).
    /// Consumed by embedders; unused in-crate for now.
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
        self.root
            .set_scale(Vector3::new(self.scale, self.scale, 1.0));
        self.bg.set_size(self.dip_size.0, self.dip_size.1);
        // Cheap by construction: a few visual property writes, no repaint and no
        // animation restart — see `Backdrop::place`.
        if let Some(b) = &self.backdrop {
            b.place(self.dip_size, self.scale);
        }
    }

    /// Recolor the window background (theme change). The backdrop sits behind the
    /// app's own opaque backdrop, so it takes the colour verbatim — no display map.
    pub fn set_background(&self, color: Color) {
        self.bg_brush.set_color(color);
    }

    /// Attach a reactor root node's container directly above the background.
    pub fn attach_root(&self, container: &ContainerVisual) {
        self.root.children().insert_at_top(container);
    }

    /// Detach a previously attached root node container.
    pub fn detach_root(&self, container: &ContainerVisual) {
        self.root.children().remove(container);
    }

    /// Attach an arbitrary visual at the top of the compositor root (exit-ghost
    /// snapshot sprites / fallback containers).
    pub fn attach_root_visual(&self, v: &Visual) {
        self.root.children().insert_at_top(v);
    }

    /// Remove a visual previously attached by [`Self::attach_root_visual`].
    pub fn remove_root_visual(&self, v: &Visual) {
        // Best-effort: an exit ghost is released either by its scoped batch
        // completing or by the fallback that fires when the batch could not be
        // armed, and a visual detached by the first path must not panic the
        // second. "Already gone" is the goal state here.
        let _ = self.root.children().try_remove(v);
    }

    /// The compositor root, as a plain visual — the starting point for a census
    /// walk of everything this window has parented, chrome and backdrop
    /// included rather than just the reactor's own subtree.
    pub fn root_visual(&self) -> &Visual {
        &self.root
    }

    /// Ask the compositor to tint what it is doing over this window's whole
    /// tree, or stop.
    ///
    /// Fails softly on purpose. Heat maps are a diagnostic facility that a given
    /// system may simply not carry, and a caller reaching for one is by
    /// definition investigating — the answer "this build cannot show you that"
    /// belongs in their report, not in a panic.
    pub fn set_heat_map(&self, map: Option<HeatMap>) -> windows_core::Result<bool> {
        let Some(maps) = self.compositor.debug_heat_maps()? else {
            return Ok(false);
        };
        match map {
            None => maps.hide(&self.root)?,
            Some(HeatMap::Redraw) => maps.show_redraw(&self.root)?,
            Some(HeatMap::Overdraw(kinds)) => maps.show_overdraw(&self.root, kinds)?,
            Some(HeatMap::MemoryUsage) => maps.show_memory_usage(&self.root)?,
        }
        Ok(true)
    }

    /// Whether `visual` IS the compositor root container. Terminates the
    /// exit-ghost parent walk (a visual whose parent chain doesn't reach this is
    /// already detached, so it must not be ghosted again).
    ///
    /// This replaces the old `root_identity()` raw-pointer accessor: `Visual`'s
    /// `PartialEq` is COM identity (canonical `IUnknown`), which is exactly what
    /// the pointer comparison was doing, so the walk's semantics are unchanged
    /// and no raw pointer escapes. `&ContainerVisual` derefs to `&Visual`, so a
    /// parent obtained from `Visual::parent()` can be passed straight in.
    pub fn is_root(&self, visual: &Visual) -> bool {
        visual == &*self.root
    }

    /// Mint a bare container visual for a node (pure layout, no surface).
    pub fn new_container(&self) -> ContainerVisual {
        self.compositor.create_container_visual()
    }

    /// Mint a top-level overlay container (popup host) above the reactor tree,
    /// backed by one FP16 surface of `px_w`×`px_h`. The container is inserted at
    /// the very top of the compositor root so it draws above all content.
    pub fn new_overlay(
        &self,
        px_w: i32,
        px_h: i32,
    ) -> windows_core::Result<(ContainerVisual, NodeSurface)> {
        let container = self.compositor.create_container_visual();
        self.root.children().insert_at_top(&container);
        let surf = self.new_surface(&container, px_w, px_h)?;
        Ok((container, surf))
    }

    /// Remove an overlay container previously inserted by [`Self::new_overlay`].
    pub fn remove_overlay(&self, container: &ContainerVisual) {
        self.root.children().remove(container);
    }

    /// The DIP scale (`dpi/96`) the root applies — popups draw in DIPs too.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Mint an inset clip that tracks a visual's own bounds (for scroll/overflow).
    pub fn new_inset_clip(&self) -> InsetClip {
        self.compositor.create_inset_clip()
    }

    /// Mint a bare sprite visual (chrome parts). Not inserted anywhere — the
    /// caller parents it at the right band position.
    pub fn new_sprite(&self) -> SpriteVisual {
        self.compositor.create_sprite_visual()
    }

    /// Mint a nine-grid (9-slice) brush for a stretchable chrome-part source.
    pub fn new_nine_grid(&self) -> CompositionNineGridBrush {
        self.compositor.create_nine_grid_brush()
    }

    /// Mint an FP16 atlas-source surface of exact pixel size, returning the
    /// surface and a Fill-stretch brush over it. The surface is also the draw
    /// seam — `begin_draw` / `end_draw` / `resize` hang off it.
    pub fn new_source_surface(
        &self,
        px_w: i32,
        px_h: i32,
    ) -> windows_core::Result<(CompositionDrawingSurface, CompositionSurfaceBrush)> {
        self.new_surface_with_format(px_w, px_h, PixelFormat::Rgba16Float)
    }

    /// [`new_source_surface`](Self::new_source_surface) at an explicit pixel
    /// format. FP16 is right for anything whose COLOUR the surface carries; a
    /// pure alpha MASK (the glyph atlas) carries no colour at all and asks for
    /// [`PixelFormat::A8UNorm`] here instead — an 8× cut in bytes per glyph. The
    /// caller owns the fallback: creation fails for a format the composition
    /// device will not accept, and that failure is the only reliable probe (see
    /// `glyph_atlas::GlyphAtlas::format`).
    pub fn new_surface_with_format(
        &self,
        px_w: i32,
        px_h: i32,
        format: PixelFormat,
    ) -> windows_core::Result<(CompositionDrawingSurface, CompositionSurfaceBrush)> {
        // `create_drawing_surface_with_format` is the `CreateDrawingSurface`
        // (float `Size`) call this has always made — NOT the pixel-size sibling,
        // which is a different COM entry point (`CreateDrawingSurface2`). The
        // sizes are handed over as floats exactly as before so allocation and
        // atlas placement are bit-for-bit what they were.
        let surface = self.graphics.create_drawing_surface_with_format(
            px_w.max(1) as f32,
            px_h.max(1) as f32,
            format,
            AlphaMode::Premultiplied,
        )?;
        let brush = self.compositor.create_surface_brush(&surface);
        brush.set_stretch(Stretch::Fill);
        Ok((surface, brush))
    }

    /// Mint an atlas page: a **virtual** surface `px_w`×`px_h` in declared size,
    /// which holds no storage until a region of it is drawn into.
    ///
    /// The raster caches pack every glyph and run into pages rather than taking a
    /// surface each; see [`mask_cache`](super::mask_cache)'s header for why the
    /// population matters more than the pixels.
    pub fn new_mask_page(
        &self,
        px_w: i32,
        px_h: i32,
        format: PixelFormat,
    ) -> windows_core::Result<CompositionVirtualDrawingSurface> {
        self.graphics
            .create_virtual_drawing_surface(px_w, px_h, format, AlphaMode::Premultiplied)
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
        // Same float-sized `CreateDrawingSurface` as ever — see
        // `new_surface_with_format` for why the pixel-size overload is not used.
        let surface = self.graphics.create_drawing_surface_with_format(
            px_w as f32,
            px_h as f32,
            PixelFormat::Rgba16Float,
            AlphaMode::Premultiplied,
        )?;
        let brush = self.compositor.create_surface_brush(&surface);
        brush.set_stretch(Stretch::Fill);

        let sprite = self.compositor.create_sprite_visual();
        // Node-chrome surfaces paint through the app's draw-time colour map (see
        // `node::linear`), so the surface brush is used raw — no compositor effect.
        sprite.set_brush(&brush);
        // Bottom of the parent so the node's own chrome sits behind its children;
        // top for an overlay (the scroll thumb draws over the scrolled content).
        let children = parent.children();
        if at_top {
            children.insert_at_top(&sprite);
        } else {
            children.insert_at_bottom(&sprite);
        }

        Ok(NodeSurface {
            sprite,
            surface,
            dip: (-1.0, -1.0),
            _brush: brush,
        })
    }
}

impl NodeSurface {
    /// Set the sprite's presented (DIP) size.
    ///
    /// Gated on the last value pushed: a popup reaching this with an unchanged
    /// size would otherwise pay a cross-process `SetSize` for nothing.
    pub fn set_dip_size(&mut self, w: f32, h: f32) {
        let (w, h) = (w.max(0.0), h.max(0.0));
        if (w, h) == self.dip {
            return;
        }
        self.sprite.set_size(w, h);
        self.dip = (w, h);
    }
}

/// The opaque window background (dark), composited beneath the reactor tree. This is
/// a compositor `Color` (8-bit sRGB) handed to the compositor's color brush, not a
/// reactor theming `Color`, so it stays a plain ARGB literal.
///
/// It is deliberately NOT tuned to match whatever backdrop sits above it. 8-bit
/// cannot represent a display-fitted base anyway (an HDR fit exceeds 1.0), and a
/// near-match would only disguise a backdrop that failed to cover the window —
/// which is a bug worth seeing, not hiding.
const WINDOW_BG: Color = Color::rgb(14, 14, 17);
