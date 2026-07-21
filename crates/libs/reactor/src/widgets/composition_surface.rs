//! Off-tree composition surfaces: Direct2D content drawn into a
//! `CompositionDrawingSurface` and shown through a `SpriteVisual` parented under an
//! arbitrary `ContainerVisual` in the system (`Windows.UI.Composition`) tree — the
//! self-hosted DirectComposition backend's compositor.
//!
//! A child-visual composition surface lives in the *composition* tree: redrawing it
//! never invalidates layout or the render walk, and it consumes **no swap chain**.
//! One [`CompositionSurfaceFactory`] (backed by a single Direct2D device, of which
//! there is naturally one compositor per UI thread) mints many
//! [`surfaces`](CompositionSurfaceFactory::create_under), so an arbitrary number of
//! live surfaces costs one graphics device and zero swap chains.
//!
//! With a multi-threaded Direct2D device the returned [`CompositionDrawSurface`]
//! is `Send`, so a worker thread can draw the content while the UI thread runs.
//! One rule governs that concurrency: a `CompositionGraphicsDevice` admits only
//! **one outstanding [`begin_draw`](CompositionDrawSurface::begin_draw)** across
//! all of its surfaces — a second concurrent `BeginDraw` on any surface of the
//! same graphics device *fails* (`0x80131509`), it does not block. So surfaces
//! drawn by different threads must come from different factories (one factory =
//! one graphics device); within one factory the owner's own sequential
//! bracketing of `begin_draw`/`end_draw` is the serialization, and no extra
//! cross-thread lock is needed. Crucially, none is *held across* the surface's
//! `EndDraw` either: an `ID2D1Multithread` lock held there would invert against the
//! UI thread's compositor commit (which takes the composition lock, then the D2D
//! lock) and deadlock. The visual tree itself (create / size / attach / detach)
//! stays on the UI thread, where the compositor lives.
//!
//! That thread split is expressed in the types, not in a comment: everything
//! thread-affine — the compositor, the graphics device, the surface, its brush and
//! its visual — stays behind in [`CompositionSurfaceFactory`] and
//! [`CompositionChildVisual`], while [`CompositionDrawSurface`] carries only a
//! [`CompositionDrawHandle`](windows_composition::CompositionDrawHandle), the
//! drawing half `windows-composition` detaches for exactly this purpose. The
//! `Send` soundness argument lives there, on the handle, rather than being
//! re-asserted here.

use super::*;
use windows_composition::{
    AlphaMode, CompositionDrawHandle, CompositionGraphicsDevice, Compositor, ContainerVisual,
    PixelFormat, Visual,
};

/// Mints composition drawing surfaces backed by one Direct2D device, each shown as
/// a child visual under a system `ContainerVisual`. Build once — ideally app-wide,
/// since there is a single compositor per UI thread — and reuse it for every
/// surface; `N` live surfaces then share this one graphics device and cost zero
/// swap chains.
///
/// Pass a **multi-threaded** Direct2D device to [`from_compositor`](Self::from_compositor)
/// to draw the surfaces off the UI thread (the returned [`CompositionDrawSurface`]
/// is then `Send`). Build it on the UI thread.
pub struct CompositionSurfaceFactory {
    compositor: Compositor,
    graphics: CompositionGraphicsDevice,
}

impl CompositionSurfaceFactory {
    /// Create the factory from the system
    /// [`Compositor`](windows_composition::Compositor) — the self-hosted
    /// DirectComposition backend's compositor. Pair with
    /// [`create_under`](Self::create_under) to host surfaces under a backend node's
    /// `ContainerVisual`. `d2d_device` is an `ID2D1Device` (multi-threaded to draw
    /// off the UI thread).
    pub fn from_compositor(compositor: &Compositor, d2d_device: &impl Interface) -> Result<Self> {
        // `create_graphics_device` is the accepting seam: it casts the rendering
        // device to `IUnknown` and calls `ICompositorInterop::CreateGraphicsDevice`
        // itself, so the D2D device goes straight in.
        let graphics = compositor.create_graphics_device(d2d_device)?;
        Ok(Self {
            compositor: compositor.clone(),
            graphics,
        })
    }

    /// Create an FP16 surface `pixel_size` pixels large, presented at `dip_size`
    /// DIPs, and parent its sprite **at the top** of `parent`'s child collection.
    ///
    /// Returns a [`CompositionChildVisual`] (drop removes the sprite from `parent`)
    /// and a [`CompositionDrawSurface`] that draws the content (move it to a worker
    /// thread when the factory's device is multi-threaded). The surface resizes in
    /// place: call [`CompositionDrawSurface::resize`] on the drawing side to grow the
    /// backing and [`CompositionChildVisual::set_dip_size`] on the hosting side to
    /// grow the presented sprite — no need to drop and recreate.
    pub fn create_under(
        &self,
        parent: &ContainerVisual,
        pixel_size: (i32, i32),
        dip_size: (f32, f32),
        opaque: bool,
    ) -> Result<(CompositionChildVisual, CompositionDrawSurface)> {
        let alpha = if opaque {
            AlphaMode::Ignore
        } else {
            AlphaMode::Premultiplied
        };
        // FP16 scRGB (`Rgba16Float`), matching the backend's HDR composition
        // pipeline (the node-chrome surfaces and the whole-window FP16 path) so a
        // meter/accent can author values past 1.0 and pop. The system compositor
        // presents this surface as scRGB-*linear*; the viz draw closures author
        // colours in sRGB, so the surface is flagged linear below and the draw
        // session (see `CompositionDrawTarget`) gamma-decodes every colour onto it —
        // a near-black #1c1c1c backdrop lands near-black, not the mid-grey that writing
        // sRGB values raw onto a linear surface would produce.
        //
        // The *pixel-size* allocator, not the DIP-size one: a viz surface must be
        // exactly N physical pixels (the drawing side scales DIP→pixel itself), and
        // the DIP variant would round that extent through the device scale.
        let surface = self.graphics.create_drawing_surface_with_pixel_size(
            pixel_size.0.max(1),
            pixel_size.1.max(1),
            PixelFormat::Rgba16Float,
            alpha,
        )?;

        let brush = self.compositor.create_surface_brush(&surface);
        // `Fill` maps the surface onto the sprite one-to-one; composition's default
        // (`Uniform`) would letterbox whenever the aspect ratios diverge mid-resize.
        brush.set_stretch(windows_composition::Stretch::Fill);
        // Live-viz surfaces are painted by the app through its own draw-time colour
        // map (`DrawKit`), so the surface brush is used raw — no compositor effect.
        let sprite = self.compositor.create_sprite_visual();
        sprite.set_size(dip_size.0, dip_size.1);
        sprite.set_brush(&brush);

        // The sprite as its base `Visual`: the same composition object, which is what
        // keeps the sprite — and through it the brush and the surface — alive for as
        // long as the child handle lives.
        let visual = Visual::clone(&sprite);
        parent.children().insert_at_top(&visual);

        // Only the drawing half crosses to the requester; the surface itself, its
        // brush and its visual stay here, on the thread that owns the compositor.
        let draw = CompositionDrawSurface {
            handle: surface.draw_handle(),
        };
        Ok((
            CompositionChildVisual {
                parent: parent.clone(),
                visual,
            },
            draw,
        ))
    }
}

/// The UI-thread side of a system-compositor child-visual surface: keeps the
/// sprite parented under its host `ContainerVisual`. **Dropping it removes the
/// sprite** from the parent. Not `Send` — the visual tree belongs to the UI thread.
pub struct CompositionChildVisual {
    parent: ContainerVisual,
    // Held so the visual (and its brush + surface) outlives this handle even if
    // the draw side drops first; also the thing we detach on drop.
    visual: Visual,
}

impl CompositionChildVisual {
    /// Resize the presented sprite to `dip` DIPs, in place. Paired with
    /// [`CompositionDrawSurface::resize`] (which grows the backing on the drawing
    /// side): the brush stretches its surface to the sprite, so both must move for a
    /// crisp result, but neither requires re-parenting or recreating the visual.
    /// Runs on the UI thread, which owns the visual tree.
    pub fn set_dip_size(&self, dip: (f32, f32)) {
        self.visual.set_size(dip.0, dip.1);
    }
}

impl Drop for CompositionChildVisual {
    fn drop(&mut self) {
        // Detaching is best-effort: this runs during teardown, where the parent
        // may already have dropped the child on its own. A panic here would
        // unwind out of a `Drop`, so a removal that finds nothing to remove is
        // simply the state we wanted.
        let _ = self.parent.children().try_remove(&self.visual);
    }
}

/// The drawing side of a composition surface: brackets each frame between
/// [`begin_draw`](Self::begin_draw) and [`end_draw`](Self::end_draw).
///
/// `Send` — automatically, because the only thing it holds is a
/// [`CompositionDrawHandle`](windows_composition::CompositionDrawHandle), which is
/// the composition crate's detached drawing half and carries the `Send` soundness
/// argument itself. So on a multi-threaded device this can be moved to a worker
/// render thread, where its `BeginDraw`/`EndDraw` serialize against the device
/// through the Direct2D factory lock. Deliberately not `Sync`, for the same reason
/// the handle is not: the draw bracket is stateful and must be owned by one thread
/// at a time.
pub struct CompositionDrawSurface {
    handle: CompositionDrawHandle,
}

impl CompositionDrawSurface {
    /// Whether the backing surface stores linear scRGB. Always true here — the
    /// system-composited viz surfaces are FP16 `Rgba16Float`. A draw path uses
    /// this to enable sRGB→linear color conversion so sRGB-authored content renders
    /// correctly on the FP16 surface.
    pub fn is_linear(&self) -> bool {
        true
    }

    /// Begin drawing the whole surface, returning the drawing target `T` (typically
    /// `ID2D1DeviceContext`) and the `(x, y)` pixel offset within the backing atlas
    /// to translate drawing by. Pair with [`end_draw`](Self::end_draw).
    pub fn begin_draw<T: Interface>(&self) -> Result<(T, (i32, i32))> {
        // Whole-surface redraw (no update rect) — we repaint every frame.
        self.handle.begin_draw::<T>()
    }

    /// Finish drawing and commit the surface contents to the compositor.
    pub fn end_draw(&self) -> Result<()> {
        self.handle.end_draw()
    }

    /// Resize the backing surface in place to `pixel` physical pixels, keeping the
    /// same visual, brush and atlas — no re-parenting and no fresh
    /// [`create_under`](CompositionSurfaceFactory::create_under). The next
    /// [`begin_draw`](Self::begin_draw) fills the new extent. Pair with
    /// [`CompositionChildVisual::set_dip_size`] so the presented sprite matches.
    /// `ICompositionDrawingSurfaceInterop::Resize` is internally synchronized, so
    /// this is safe from the drawing thread between frames.
    pub fn resize(&self, pixel: (i32, i32)) -> Result<()> {
        // Clamped to a non-empty extent here rather than in the wrapper: a zero-sized
        // surface is rejected by composition, and a viz host legitimately reaches zero
        // while its control is collapsed mid-layout.
        self.handle.resize(pixel.0.max(1), pixel.1.max(1))
    }
}
