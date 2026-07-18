//! Retained **chrome parts**: the animated fragments of the drawn controls
//! (indicator pills, toggle knobs, slider fills, hover/press ink) lifted out of
//! the per-node painted surface into their own compositor sprites, so their
//! motion runs entirely on the system compositor (DWM) — no app tick, no
//! repaint, no timer while they move.
//!
//! Three pieces:
//!
//! - [`Atlas`] — tiny FP16 source surfaces (a solid, a rounded bar, a circle)
//!   rasterized ONCE per (shape, colour, scale) and shared by every part that
//!   needs that look. Rounded bars stretch through a `CompositionNineGridBrush`
//!   (9-slice), so one source serves any width with pristine corners. Cleared
//!   whenever the mapped colours may have changed (display change, DPI change,
//!   device loss) — parts re-bind by epoch.
//! - [`Part`] — one `SpriteVisual` plus cached **retargetable compositor
//!   springs** for Offset / Size / Opacity. A state change is `SetFinalValue`
//!   + `StartAnimation` on the cached object (no per-event allocation); a drag
//!   snap is `StopAnimation` + a plain property set. Spring tuning matches the
//!   retired CPU springs (`k = 520, c = 40`), so the feel is unchanged.
//! - Per-kind `sync` — the single writer that reconciles a control's parts
//!   against its logical state from the paint pass: glides on a state change,
//!   snaps on first placement / resize (mounting never flies in).
//!
//! Interaction events (hover, press, drag) retarget the springs directly via
//! [`ink_state_changed`] / [`slider_drag`] / [`seg_hot_changed`] — no frame
//! timer is involved anywhere in control motion.
//!
//! Z-order contract (upheld here at creation and by `layout::sync` on a child
//! re-sync): *below* parts sit under the node's painted surface (tray / pill /
//! indicator under the labels), *above* parts sit over it (ink wash, slider
//! fill + thumb over the painted groove).

use rustc_hash::FxHashMap;

use super::bootstrap::Compositing;
use super::node::{linear, Node};
use super::theme;
use crate::backend::ControlKind;
use crate::system_bindings::{
    AnimationIterationBehavior, CompositionAnimation, CompositionBrush,
    CompositionDrawingSurface, CompositionEasingFunction, CompositionNineGridBrush,
    CompositionSurfaceBrush, ICompositionObject, ICompositor2, ICompositor4, IKeyFrameAnimation,
    ISpringVector2NaturalMotionAnimation, ISpringVector3NaturalMotionAnimation,
    IVector2NaturalMotionAnimation, IVector3NaturalMotionAnimation, IVisual,
    SpringVector2NaturalMotionAnimation, SpringVector3NaturalMotionAnimation, SpriteVisual,
    TimeSpan, Visual,
};
use windows_canvas_core::{
    Brush, ColorF, DrawingSession, Ellipse, Rect, RoundedRect, Vector2 as CVec2,
};
use windows_core::Interface;
use windows_numerics::{Matrix3x2, Vector2, Vector3};

/// Spring tuning, matching the retired CPU spring (`node::Spring`: `k = 520`,
/// `c = 40`): natural period `2π/√k`, damping ratio `c / (2√k)`. Shared with
/// the scroll-carrier glide (`Node::scroll_glide`) so scrolling feels the same
/// as it did on the CPU spring.
pub(crate) const SPRING_PERIOD: f32 = 0.2756;
pub(crate) const SPRING_DAMPING: f32 = 0.877;

/// `TimeSpan` (100 ns units) from seconds.
fn ts_secs(s: f32) -> TimeSpan {
    TimeSpan {
        duration: (s.max(0.001) * 1.0e7) as i64,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Atlas — shared rasterized part sources
// ─────────────────────────────────────────────────────────────────────────────

/// The rasterized shape of an atlas source. Dimensions are DIP `f32` bit
/// patterns so the key is `Eq + Hash` without float caveats.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ShapeKey {
    /// A solid fill; stretches to any size (4×4 px source).
    Solid,
    /// A horizontally stretchable rounded bar of fixed DIP height `h` and
    /// corner radius `r`; filled when `stroke_w == 0`, stroked otherwise.
    /// Served through a per-part nine-grid brush.
    HBar { h: u32, r: u32, stroke_w: u32 },
    /// An exact circle of DIP diameter `d` (drawn 1:1, no stretch).
    Circle { d: u32 },
    /// A checkmark glyph (two strokes) in a `d`×`d` DIP box (drawn 1:1).
    Check { d: u32 },
}

/// Atlas cache key: shape + the *authored* token colour (the display colour
/// map is applied at rasterize time) + the DIP→px scale it was drawn at.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct AtlasKey {
    shape: ShapeKey,
    color: [u32; 4],
    scale: u32,
}

impl AtlasKey {
    fn solid(c: crate::Color, scale: f32) -> Self {
        Self { shape: ShapeKey::Solid, color: color_bits(c), scale: scale.to_bits() }
    }
    fn hbar(h: f32, r: f32, stroke_w: f32, c: crate::Color, scale: f32) -> Self {
        Self {
            shape: ShapeKey::HBar { h: h.to_bits(), r: r.to_bits(), stroke_w: stroke_w.to_bits() },
            color: color_bits(c),
            scale: scale.to_bits(),
        }
    }
    fn circle(d: f32, c: crate::Color, scale: f32) -> Self {
        Self { shape: ShapeKey::Circle { d: d.to_bits() }, color: color_bits(c), scale: scale.to_bits() }
    }
    fn check(d: f32, c: crate::Color, scale: f32) -> Self {
        Self { shape: ShapeKey::Check { d: d.to_bits() }, color: color_bits(c), scale: scale.to_bits() }
    }
    /// The nine-grid corner inset in source pixels (`r * scale`), 0 for the
    /// shapes that stretch uniformly.
    fn inset_px(&self) -> f32 {
        match self.shape {
            ShapeKey::HBar { r, .. } => f32::from_bits(r) * f32::from_bits(self.scale),
            _ => 0.0,
        }
    }
    fn is_hbar(&self) -> bool {
        matches!(self.shape, ShapeKey::HBar { .. })
    }
}

fn color_bits(c: crate::Color) -> [u32; 4] {
    [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]
}

struct AtlasEntry {
    brush: CompositionSurfaceBrush,
    // Keeps the pixels alive behind the brush.
    _surface: CompositionDrawingSurface,
}

/// Rasterized part sources, shared across every control. Bounded: a handful of
/// shapes × the token palette; cleared wholesale on any edge that can change
/// the mapped colours or the pixel scale.
#[derive(Default)]
pub(crate) struct Atlas {
    map: FxHashMap<AtlasKey, AtlasEntry>,
    /// Bumped on [`clear`](Self::clear); parts re-bind when their bound epoch
    /// no longer matches.
    epoch: u32,
}

impl Atlas {
    /// Drop every cached source (display / DPI / theme / device edge). Parts
    /// keep their current brush alive via the sprite's own COM reference until
    /// they re-bind on the next sync.
    pub(crate) fn clear(&mut self) {
        self.map.clear();
        self.epoch = self.epoch.wrapping_add(1);
    }

    fn entry(&mut self, comp: &Compositing, key: AtlasKey) -> Option<&AtlasEntry> {
        use std::collections::hash_map::Entry;
        match self.map.entry(key) {
            Entry::Occupied(e) => Some(e.into_mut()),
            Entry::Vacant(v) => {
                let entry = rasterize(comp, &key)?;
                Some(v.insert(entry))
            }
        }
    }
}

/// Draw one atlas source: an FP16 surface of the shape's exact pixel size,
/// painted through the app's output colour map ([`linear`]).
fn rasterize(comp: &Compositing, key: &AtlasKey) -> Option<AtlasEntry> {
    let scale = f32::from_bits(key.scale).max(0.01);
    let color = crate::Color {
        r: f32::from_bits(key.color[0]),
        g: f32::from_bits(key.color[1]),
        b: f32::from_bits(key.color[2]),
        a: f32::from_bits(key.color[3]),
    };
    // DIP geometry of the source.
    let (dip_w, dip_h) = match key.shape {
        ShapeKey::Solid => (4.0 / scale, 4.0 / scale),
        // Corners plus a 2-DIP stretchable centre column.
        ShapeKey::HBar { h, r, .. } => (2.0 * f32::from_bits(r) + 2.0, f32::from_bits(h)),
        ShapeKey::Circle { d } | ShapeKey::Check { d } => (f32::from_bits(d), f32::from_bits(d)),
    };
    let px_w = ((dip_w * scale).round() as i32).max(1);
    let px_h = ((dip_h * scale).round() as i32).max(1);

    let (surface, interop, brush) = comp.new_source_surface(px_w, px_h).ok()?;
    let mut origin = crate::system_bindings::POINT::default();
    comp.device_lost.set(false);
    let ctx = unsafe { interop.BeginDraw(None, &mut origin).ok()? };
    let session = DrawingSession::new_borrowed(&ctx, &comp.device_lost);
    session.set_transform(&Matrix3x2 {
        m11: scale,
        m12: 0.0,
        m21: 0.0,
        m22: scale,
        m31: origin.x as f32,
        m32: origin.y as f32,
    });
    session.clear(ColorF::new(0.0, 0.0, 0.0, 0.0));
    if let Ok(b) = session.create_solid_brush(linear(color)) {
        draw_shape(&session, &b, key.shape, dip_w, dip_h);
    }
    unsafe { interop.EndDraw().ok()? };
    Some(AtlasEntry { brush, _surface: surface })
}

fn draw_shape(session: &DrawingSession, brush: &Brush, shape: ShapeKey, w: f32, h: f32) {
    match shape {
        ShapeKey::Solid => session.fill_rect(&Rect::from_xywh(0.0, 0.0, w, h), brush),
        ShapeKey::HBar { r, stroke_w, .. } => {
            let radius = f32::from_bits(r);
            let sw = f32::from_bits(stroke_w);
            let rect = Rect::from_xywh(0.0, 0.0, w, h);
            if sw <= 0.0 {
                session.fill_rounded_rect(&RoundedRect::uniform(rect, radius), brush);
            } else {
                // Stroke drawn inset by half its width, like `controls::stroke_rr`.
                let inset =
                    Rect::new(sw / 2.0, sw / 2.0, w - sw / 2.0, h - sw / 2.0);
                session.draw_rounded_rect(&RoundedRect::uniform(inset, radius), brush, sw);
            }
        }
        ShapeKey::Circle { d } => {
            let radius = f32::from_bits(d) / 2.0;
            session.fill_ellipse(
                &Ellipse::new(CVec2::new(radius, radius), radius, radius),
                brush,
            );
        }
        // Stroke coordinates mirror the retired painted checkmark (authored in
        // an 18-DIP box), scaled to `d`.
        ShapeKey::Check { d } => {
            let s = f32::from_bits(d) / 18.0;
            session.draw_line(
                CVec2::new(4.0 * s, 9.0 * s),
                CVec2::new(7.5 * s, 12.5 * s),
                brush,
                2.0 * s,
            );
            session.draw_line(
                CVec2::new(7.5 * s, 12.5 * s),
                CVec2::new(14.0 * s, 5.5 * s),
                brush,
                2.0 * s,
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Part — one retained sprite with retargetable compositor springs
// ─────────────────────────────────────────────────────────────────────────────

/// One chrome-part sprite. All mutation is change-gated against the last
/// written *target* so an unchanged sync costs nothing, and all motion is a
/// retarget of a cached compositor spring — zero allocation per event.
pub(crate) struct Part {
    sprite: SpriteVisual,
    vis: IVisual,
    obj: ICompositionObject,
    /// Nine-grid wrapper (HBar sources only); built once, re-sourced on re-bind.
    nine: Option<CompositionNineGridBrush>,
    /// The atlas source currently bound + the epoch it came from.
    key: Option<AtlasKey>,
    epoch: u32,
    /// Last written targets (`None` = never placed → the next write snaps).
    off: Option<(f32, f32)>,
    size: Option<(f32, f32)>,
    opacity: Option<f32>,
    /// Whether a spring may currently hold the property (snap must stop it).
    off_gliding: bool,
    size_gliding: bool,
    op_gliding: bool,
    // Cached retargetable motion springs, built on first glide.
    s_off: Option<SpringVector3NaturalMotionAnimation>,
    s_size: Option<SpringVector2NaturalMotionAnimation>,
}

impl Part {
    fn new(comp: &Compositing) -> Option<Self> {
        let sprite = comp.new_sprite().ok()?;
        let vis: IVisual = sprite.cast().ok()?;
        let obj: ICompositionObject = sprite.cast().ok()?;
        Some(Self {
            sprite,
            vis,
            obj,
            nine: None,
            key: None,
            epoch: 0,
            off: None,
            size: None,
            opacity: None,
            off_gliding: false,
            size_gliding: false,
            op_gliding: false,
            s_off: None,
            s_size: None,
        })
    }

    /// The sprite as a plain `Visual` (for tree insertion / re-sync).
    pub(crate) fn visual(&self) -> Option<Visual> {
        self.sprite.cast().ok()
    }

    /// Bind (or re-bind) this part's brush to the atlas source for `key`.
    /// No-op while the key and atlas epoch are unchanged.
    fn bind(&mut self, comp: &Compositing, atlas: &mut Atlas, key: AtlasKey) {
        if self.key == Some(key) && self.epoch == atlas.epoch {
            return;
        }
        let epoch = atlas.epoch;
        let Some(entry) = atlas.entry(comp, key) else { return };
        let brush: Option<CompositionBrush> = if key.is_hbar() {
            // Corners map 1:1 back to DIPs: source insets are `r * scale` px,
            // scaled down by `1 / scale` on the destination.
            let nine = match &self.nine {
                Some(n) => n.clone(),
                None => match comp.new_nine_grid() {
                    Ok(n) => {
                        self.nine = Some(n.clone());
                        n
                    }
                    Err(_) => return,
                },
            };
            let inset = key.inset_px();
            let scale = f32::from_bits(key.scale).max(0.01);
            let ok = nine.SetInsetsWithValues(inset, 0.0, inset, 0.0).is_ok()
                && nine.SetInsetScales(1.0 / scale).is_ok()
                && entry
                    .brush
                    .cast::<CompositionBrush>()
                    .and_then(|src| nine.SetSource(&src))
                    .is_ok();
            ok.then(|| nine.cast().ok()).flatten()
        } else {
            entry.brush.cast().ok()
        };
        if let Some(b) = brush
            && self.sprite.SetBrush(&b).is_ok()
        {
            self.key = Some(key);
            self.epoch = epoch;
        }
    }

    /// Snap position + size (stopping any in-flight glide first).
    fn place(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if self.off != Some((x, y)) {
            if self.off_gliding {
                let _ = self.obj.StopAnimation("Offset");
                self.off_gliding = false;
            }
            let _ = self.vis.SetOffset(Vector3::new(x, y, 0.0));
            self.off = Some((x, y));
        }
        if self.size != Some((w, h)) {
            if self.size_gliding {
                let _ = self.obj.StopAnimation("Size");
                self.size_gliding = false;
            }
            let _ = self.vis.SetSize(Vector2::new(w, h));
            self.size = Some((w, h));
        }
    }

    /// Spring-glide position + size to a new target. First placement snaps
    /// (mounting must never fly in from the visual's zeroed defaults).
    fn glide(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if self.off.is_none() || self.size.is_none() {
            self.place(x, y, w, h);
            return;
        }
        if self.off != Some((x, y)) {
            if self.glide_offset(x, y).is_some() {
                self.off = Some((x, y));
                self.off_gliding = true;
            } else {
                self.place(x, y, w, h);
                return;
            }
        }
        if self.size != Some((w, h)) {
            if self.glide_size(w, h).is_some() {
                self.size = Some((w, h));
                self.size_gliding = true;
            } else {
                self.place(x, y, w, h);
            }
        }
    }

    fn glide_offset(&mut self, x: f32, y: f32) -> Option<()> {
        if self.s_off.is_none() {
            let c = self.obj.Compositor().ok()?;
            let a = c.cast::<ICompositor4>().ok()?.CreateSpringVector3Animation().ok()?;
            let sa: ISpringVector3NaturalMotionAnimation = a.cast().ok()?;
            sa.SetDampingRatio(SPRING_DAMPING).ok()?;
            sa.SetPeriod(ts_secs(SPRING_PERIOD)).ok()?;
            self.s_off = Some(a);
        }
        let a = self.s_off.as_ref()?;
        a.cast::<IVector3NaturalMotionAnimation>()
            .ok()?
            .SetFinalValue(Some(Vector3::new(x, y, 0.0)))
            .ok()?;
        self.obj
            .StartAnimation("Offset", &a.cast::<CompositionAnimation>().ok()?)
            .ok()
    }

    fn glide_size(&mut self, w: f32, h: f32) -> Option<()> {
        if self.s_size.is_none() {
            let c = self.obj.Compositor().ok()?;
            let a = c.cast::<ICompositor4>().ok()?.CreateSpringVector2Animation().ok()?;
            let sa: ISpringVector2NaturalMotionAnimation = a.cast().ok()?;
            sa.SetDampingRatio(SPRING_DAMPING).ok()?;
            sa.SetPeriod(ts_secs(SPRING_PERIOD)).ok()?;
            self.s_size = Some(a);
        }
        let a = self.s_size.as_ref()?;
        a.cast::<IVector2NaturalMotionAnimation>()
            .ok()?
            .SetFinalValue(Some(Vector2::new(w, h)))
            .ok()?;
        self.obj
            .StartAnimation("Size", &a.cast::<CompositionAnimation>().ok()?)
            .ok()
    }

    /// Snap opacity (stopping any in-flight fade first).
    fn set_opacity(&mut self, a: f32) {
        if self.opacity == Some(a) {
            return;
        }
        if self.op_gliding {
            let _ = self.obj.StopAnimation("Opacity");
            self.op_gliding = false;
        }
        let _ = self.vis.SetOpacity(a);
        self.opacity = Some(a);
    }

    /// Fade opacity to a target — a compositor keyframe glide (the mechanism
    /// the scroll-thumb reveal already proves out; the scalar natural-motion
    /// spring runs far slower than its tuning promises, so it is not used for
    /// opacity). Quick in, gentler out, retargeting smoothly mid-flight.
    /// First write snaps.
    fn fade_to(&mut self, a: f32) {
        if self.opacity == Some(a) {
            return;
        }
        let Some(prev) = self.opacity else {
            self.set_opacity(a);
            return;
        };
        let run = || -> Option<()> {
            let comp = self.obj.Compositor().ok()?;
            let v = self.sprite.cast::<Visual>().ok()?;
            let ms = if a > prev { FADE_IN_MS } else { FADE_OUT_MS };
            super::animate::fade_opacity(
                &comp,
                &v,
                a,
                std::time::Duration::from_millis(ms),
                crate::style::Easing::EaseOut,
            );
            Some(())
        };
        if run().is_some() {
            self.opacity = Some(a);
            self.op_gliding = true;
        } else {
            self.set_opacity(a);
        }
    }

    /// Start a FOREVER-looping constant-velocity sweep of this sprite's
    /// `Offset.X` from `from` to `to` over `secs` — the indeterminate-progress
    /// travel, playing entirely on the compositor (the app never ticks it).
    /// Set the resting offset (`place`) BEFORE starting: the loop owns only the
    /// X subchannel; Y stays where it was placed.
    fn loop_x(&mut self, from: f32, to: f32, secs: f32) -> bool {
        let run = || -> Option<()> {
            let comp = self.obj.Compositor().ok()?;
            let lin: CompositionEasingFunction =
                comp.CreateLinearEasingFunction().ok()?.cast().ok()?;
            let a = comp.CreateScalarKeyFrameAnimation().ok()?;
            a.InsertKeyFrameWithEasingFunction(0.0, from, &lin).ok()?;
            a.InsertKeyFrameWithEasingFunction(1.0, to, &lin).ok()?;
            let kf: IKeyFrameAnimation = a.cast().ok()?;
            kf.SetDuration(ts_secs(secs)).ok()?;
            kf.SetIterationBehavior(AnimationIterationBehavior::Forever).ok()?;
            let _ = self.obj.StopAnimation("Offset.X");
            self.obj
                .StartAnimation("Offset.X", &a.cast::<CompositionAnimation>().ok()?)
                .ok()
        };
        // The loop owns Offset.X from here; drop the offset gate so a later
        // place() rewrites the full offset unconditionally.
        self.off = None;
        run().is_some()
    }

    /// Stop the looping sweep (back to determinate); the next `place`
    /// re-anchors the offset.
    fn stop_loop_x(&mut self) {
        let _ = self.obj.StopAnimation("Offset.X");
        self.off = None;
    }
}

/// Ink/halo fade durations (ms): a quick reveal, a slightly gentler conceal —
/// the perceptual speed of the retired `(520, 40)` CPU hover spring.
const FADE_IN_MS: u64 = 120;
const FADE_OUT_MS: u64 = 220;

// ─────────────────────────────────────────────────────────────────────────────
// Parts — a node's part set + last-synced logical state
// ─────────────────────────────────────────────────────────────────────────────

/// A converted control's retained parts and the logical state they were last
/// reconciled against. Boxed on the node; only converted kinds allocate one.
pub(crate) struct Parts {
    /// Sprites under the node's painted surface (tray / pill / indicator).
    below: Vec<Part>,
    /// Sprites over it (ink wash, slider fill / halo / thumb).
    above: Vec<Part>,
    /// First sync completed — until then every write snaps.
    init: bool,
    /// Last glided-to selection (segmented / nav).
    sel: i32,
    /// Last toggle state.
    on: bool,
    /// Last slider fraction.
    frac: f32,
    /// Last node size; a change snaps (resize must not glide).
    geom: (f32, f32),
    /// Segmented: checksum of the segment edges (labels / widths changed).
    edges_sig: f32,
    /// Progress: a forever-looping compositor animation is running (the
    /// indeterminate bar sweep / ring spin).
    looping: bool,
}

impl Parts {
    fn new() -> Self {
        Self {
            below: Vec::new(),
            above: Vec::new(),
            init: false,
            sel: -1,
            on: false,
            frac: 0.0,
            geom: (0.0, 0.0),
            edges_sig: 0.0,
            looping: false,
        }
    }

    pub(crate) fn below_visuals(&self) -> impl Iterator<Item = Visual> + '_ {
        self.below.iter().filter_map(Part::visual)
    }
    pub(crate) fn above_visuals(&self) -> impl Iterator<Item = Visual> + '_ {
        self.above.iter().filter_map(Part::visual)
    }
}

/// Kinds whose dynamic chrome is fully part-driven (their springs never enter
/// the frame tick; hover / press / activation retarget compositor springs or
/// repaint once, event-driven). The HyperlinkButton has no parts at all — it
/// is listed so its hover recolor stays a single repaint instead of a tick.
pub(crate) fn converted(kind: ControlKind) -> bool {
    matches!(
        kind,
        ControlKind::Button
            | ControlKind::ToggleButton
            | ControlKind::RepeatButton
            | ControlKind::SplitButton
            | ControlKind::HyperlinkButton
            | ControlKind::ComboBox
            | ControlKind::DropDownButton
            | ControlKind::ToggleSwitch
            | ControlKind::CheckBox
            | ControlKind::Slider
            | ControlKind::SelectorBar
            | ControlKind::NavigationView
            | ControlKind::Expander
            | ControlKind::ProgressBar
            | ControlKind::ProgressRing
    )
}

/// Ensure `node.parts` exists with `n_below`/`n_above` parts, inserted at the
/// correct band positions around the painted surface sprite.
fn ensure(comp: &Compositing, node: &mut Node, n_below: usize, n_above: usize) -> bool {
    if node.parts.is_some() {
        return true;
    }
    let Some(surf) = node.surf.as_ref() else { return false };
    let Ok(surf_vis) = surf.sprite.cast::<Visual>() else { return false };
    let Ok(children) = node.container.Children() else { return false };

    let mut parts = Box::new(Parts::new());
    // Creation order = bottom→top within the band: each `InsertBelow(surface)`
    // lands directly under the surface, pushing earlier parts further down.
    for _ in 0..n_below {
        let Some(p) = Part::new(comp) else { return false };
        let Some(v) = p.visual() else { return false };
        if children.InsertBelow(&v, &surf_vis).is_err() {
            return false;
        }
        parts.below.push(p);
    }
    for _ in 0..n_above {
        let Some(p) = Part::new(comp) else { return false };
        let Some(v) = p.visual() else { return false };
        if children.InsertAtTop(&v).is_err() {
            return false;
        }
        parts.above.push(p);
    }
    node.parts = Some(parts);
    true
}

/// The ink/halo target opacity for the *converted* alpha of an authored wash
/// (endpoint-exact with the retired painted `theme::w(wash)`).
fn wash(authored: f32) -> f32 {
    theme::wash_alpha(authored)
}

/// The uniform disabled dim, as the paint path applies it.
fn dim_of(node: &Node) -> f32 {
    if node.paint.is_enabled {
        1.0
    } else {
        theme::disabled_opacity()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Per-kind sync (the single writer, run from the paint pass on a dirty node)
// ─────────────────────────────────────────────────────────────────────────────

/// Reconcile a converted node's parts against its logical state. Called from
/// the paint pass after the node's surface exists (dirty nodes only — every
/// state change that matters marks the node dirty; pure hover/drag updates go
/// through the direct event entry points below instead).
pub(crate) fn sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    match node.kind {
        ControlKind::ToggleSwitch => toggle_sync(comp, atlas, node, scale),
        ControlKind::CheckBox => check_sync(comp, atlas, node, scale),
        ControlKind::Slider => slider_sync(comp, atlas, node, scale),
        ControlKind::SelectorBar => segmented_sync(comp, atlas, node, scale),
        ControlKind::NavigationView => nav_sync(comp, atlas, node, scale),
        ControlKind::Expander => expander_sync(comp, atlas, node, scale),
        ControlKind::ProgressBar => progress_sync(comp, atlas, node, scale),
        ControlKind::ProgressRing => ring_sync(comp, node),
        ControlKind::Button
        | ControlKind::ToggleButton
        | ControlKind::RepeatButton
        | ControlKind::SplitButton
        | ControlKind::ComboBox
        | ControlKind::DropDownButton => ink_sync(comp, atlas, node, scale),
        // HyperlinkButton: painted only (hover recolor is an event repaint).
        _ => {}
    }
}

// ── Hover / press ink (button family + select triggers) ──────────────────────

/// Button-family ink geometry: full node rect at the control's corner radius.
fn ink_radius(node: &Node) -> f32 {
    match node.kind {
        ControlKind::ComboBox | ControlKind::DropDownButton => theme::RADIUS_SM,
        _ => node.paint.corner_radius.max(theme::RADIUS_MD),
    }
}

fn ink_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 0, 1) {
        return;
    }
    let (w, h) = (node.rect.w, node.rect.h);
    let key = AtlasKey::hbar(h, ink_radius(node), 0.0, theme::w(1.0), scale);
    let target = ink_target(node);
    let Some(parts) = node.parts.as_mut() else { return };
    parts.above[0].bind(comp, atlas, key);
    parts.above[0].place(0.0, 0.0, w, h);
    if parts.init {
        parts.above[0].fade_to(target);
    } else {
        parts.above[0].set_opacity(target);
        parts.init = true;
    }
    parts.geom = (w, h);
}

/// The combined hover + press wash target (endpoint parity with the painted
/// `0.06·hover + 0.04·press` white wash).
fn ink_target(node: &Node) -> f32 {
    let mut authored = 0.0;
    if node.hovered {
        authored += 0.06;
    }
    if node.pressed {
        authored += 0.04;
    }
    wash(authored) * dim_of(node)
}

/// Direct event entry: hover / press flipped on a converted control. Retargets
/// the relevant opacity springs; no tick, no repaint.
pub(crate) fn ink_state_changed(node: &mut Node) {
    match node.kind {
        ControlKind::ToggleSwitch => toggle_fades(node),
        ControlKind::Slider => {
            let t = halo_target(node);
            if let Some(parts) = node.parts.as_mut()
                && parts.above.len() == 3
            {
                parts.above[1].fade_to(t);
            }
        }
        // Place *and* fade: on hover entry the hot segment was recorded
        // before this flip, so the ink must land on it, not fade in wherever
        // it last sat.
        ControlKind::SelectorBar => seg_hot_changed(node),
        // No ink: the CheckBox / hyperlink hover feedback is painted (the
        // caller repaints once, event-driven); progress is not interactive.
        // Careful: the CheckBox's above[0] is its CHECKMARK — the default arm
        // would fade it on hover.
        ControlKind::CheckBox
        | ControlKind::HyperlinkButton
        | ControlKind::ProgressBar
        | ControlKind::ProgressRing => {}
        _ => {
            let t = ink_target(node);
            if let Some(parts) = node.parts.as_mut()
                && !parts.above.is_empty()
            {
                parts.above[0].fade_to(t);
            }
        }
    }
}

// ── ToggleSwitch ─────────────────────────────────────────────────────────────

/// Track geometry mirrors the retired `paint_toggle_switch`.
const TRACK_W: f32 = 40.0;
const TRACK_H: f32 = 20.0;
const KNOB_D: f32 = 12.0;

/// The off-track outline is authored at the hover-bright alpha; the rest state
/// is expressed as sprite opacity, so hover is a pure compositor fade.
/// (`wash_alpha(0.20) / wash_alpha(0.28)` — endpoint-exact with the painted
/// `w((0.20 + 0.08·hover)·(1−t))` stroke.)
const OUTLINE_AUTHORED: f32 = 0.28;
fn outline_rest_factor() -> f32 {
    wash(0.20) / wash(OUTLINE_AUTHORED)
}

fn toggle_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 3, 0) {
        return;
    }
    let cy = node.rect.h / 2.0;
    let on = node.ctrl.is_on;
    let dim = dim_of(node);
    let (kx_off, kx_on) = knob_xs();
    let kx = if on { kx_on } else { kx_off };

    let k_on = AtlasKey::hbar(TRACK_H, TRACK_H / 2.0, 0.0, theme::accent(), scale);
    let k_off = AtlasKey::hbar(TRACK_H, TRACK_H / 2.0, 1.5, theme::w(OUTLINE_AUTHORED), scale);
    let k_knob = AtlasKey::circle(KNOB_D, theme::w(1.0), scale);

    let (on_t, off_t) = track_targets(on, node.hovered, dim);
    let geom = (node.rect.w, node.rect.h);
    let Some(parts) = node.parts.as_mut() else { return };
    let snap = !parts.init || parts.geom != geom;

    parts.below[0].bind(comp, atlas, k_on);
    parts.below[1].bind(comp, atlas, k_off);
    parts.below[2].bind(comp, atlas, k_knob);
    parts.below[0].place(0.0, cy - TRACK_H / 2.0, TRACK_W, TRACK_H);
    parts.below[1].place(0.0, cy - TRACK_H / 2.0, TRACK_W, TRACK_H);

    let ky = cy - KNOB_D / 2.0;
    if snap || parts.on == on {
        parts.below[2].place(kx, ky, KNOB_D, KNOB_D);
        parts.below[0].set_opacity(on_t);
        parts.below[1].set_opacity(off_t);
    } else {
        parts.below[2].glide(kx, ky, KNOB_D, KNOB_D);
        parts.below[0].fade_to(on_t);
        parts.below[1].fade_to(off_t);
    }
    parts.on = on;
    parts.geom = geom;
    parts.init = true;
}

fn knob_xs() -> (f32, f32) {
    // Knob centres 8 / 32 DIPs into the 40-DIP track (2-DIP end margins).
    let r = KNOB_D / 2.0;
    (2.0, TRACK_W - 2.0 - 2.0 * r)
}

fn track_targets(on: bool, hovered: bool, dim: f32) -> (f32, f32) {
    let on_t = if on { dim } else { 0.0 };
    let off_t = if on {
        0.0
    } else {
        (outline_rest_factor() + (1.0 - outline_rest_factor()) * f32::from(hovered as u8)) * dim
    };
    (on_t, off_t)
}

/// Hover flipped on the toggle: refade the off-track outline.
fn toggle_fades(node: &mut Node) {
    let (_, off_t) = track_targets(node.ctrl.is_on, node.hovered, dim_of(node));
    if let Some(parts) = node.parts.as_mut()
        && parts.below.len() == 3
    {
        parts.below[1].fade_to(off_t);
    }
}

// ── CheckBox ─────────────────────────────────────────────────────────────────

/// Box side, mirroring the retired `paint_check_box`.
const CHECK_BOX_D: f32 = 18.0;

/// Below: `[accent box fill]` (under the painted stroke + label). Above:
/// `[checkmark]`. A check/uncheck is a pair of compositor fades — endpoint
/// parity with the retired painted crossfade (`transparent→accent` fill,
/// `w(on)` checkmark).
fn check_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 1, 1) {
        return;
    }
    let on = node.ctrl.is_checked;
    let t = if on { dim_of(node) } else { 0.0 };
    let y = node.rect.h / 2.0 - CHECK_BOX_D / 2.0;
    let k_fill = AtlasKey::hbar(CHECK_BOX_D, theme::RADIUS_SM, 0.0, theme::accent(), scale);
    let k_check = AtlasKey::check(CHECK_BOX_D, theme::w(1.0), scale);
    let geom = (node.rect.w, node.rect.h);
    let Some(parts) = node.parts.as_mut() else { return };
    let snap = !parts.init || parts.geom != geom;

    parts.below[0].bind(comp, atlas, k_fill);
    parts.above[0].bind(comp, atlas, k_check);
    parts.below[0].place(0.0, y, CHECK_BOX_D, CHECK_BOX_D);
    parts.above[0].place(0.0, y, CHECK_BOX_D, CHECK_BOX_D);
    if snap || parts.on == on {
        parts.below[0].set_opacity(t);
        parts.above[0].set_opacity(t);
    } else {
        parts.below[0].fade_to(t);
        parts.above[0].fade_to(t);
    }
    parts.on = on;
    parts.geom = geom;
    parts.init = true;
}

// ── Slider ───────────────────────────────────────────────────────────────────

fn halo_target(node: &Node) -> f32 {
    if node.hovered || node.pressed {
        wash(0.10) * dim_of(node)
    } else {
        0.0
    }
}

/// Above-band roles: `[fill, halo, thumb]`.
fn slider_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 0, 3) {
        return;
    }
    let frac = super::ctrl_value_frac(node) as f32;
    let dim = dim_of(node);
    let halo_t = halo_target(node);
    let k_fill = AtlasKey::hbar(theme::SLIDER_TRACK, theme::SLIDER_TRACK / 2.0, 0.0, theme::accent(), scale);
    let k_halo = AtlasKey::circle(theme::SLIDER_THUMB + 6.0, theme::w(1.0), scale);
    let k_thumb = AtlasKey::circle(theme::SLIDER_THUMB, theme::w(1.0), scale);

    let g = slider_geom(node.rect.w, node.rect.h, frac);
    let geom = (node.rect.w, node.rect.h);
    let Some(parts) = node.parts.as_mut() else { return };
    let snap = !parts.init || parts.geom != geom || node.pressed;

    parts.above[0].bind(comp, atlas, k_fill);
    parts.above[1].bind(comp, atlas, k_halo);
    parts.above[2].bind(comp, atlas, k_thumb);

    slider_apply(parts, &g, snap);
    parts.above[0].set_opacity(dim);
    parts.above[1].fade_to(halo_t);
    parts.above[2].set_opacity(dim);
    parts.frac = frac;
    parts.geom = geom;
    parts.init = true;
}

struct SliderGeom {
    fill: (f32, f32, f32, f32),
    halo: (f32, f32, f32, f32),
    thumb: (f32, f32, f32, f32),
}

fn slider_geom(w: f32, h: f32, frac: f32) -> SliderGeom {
    let cy = h / 2.0;
    let inset = theme::SLIDER_THUMB / 2.0;
    let x0 = inset;
    let x1 = (w - inset).max(x0);
    let frac = frac.clamp(0.0, 1.0);
    let thumb_x = x0 + (x1 - x0) * frac;
    let tr = theme::SLIDER_TRACK;
    let halo_d = theme::SLIDER_THUMB + 6.0;
    SliderGeom {
        fill: (x0, cy - tr / 2.0, thumb_x - x0, tr),
        halo: (thumb_x - halo_d / 2.0, cy - halo_d / 2.0, halo_d, halo_d),
        thumb: (
            thumb_x - theme::SLIDER_THUMB / 2.0,
            cy - theme::SLIDER_THUMB / 2.0,
            theme::SLIDER_THUMB,
            theme::SLIDER_THUMB,
        ),
    }
}

fn slider_apply(parts: &mut Parts, g: &SliderGeom, snap: bool) {
    let put = |p: &mut Part, r: (f32, f32, f32, f32), snap: bool| {
        if snap {
            p.place(r.0, r.1, r.2, r.3);
        } else {
            p.glide(r.0, r.1, r.2, r.3);
        }
    };
    put(&mut parts.above[0], g.fill, snap);
    put(&mut parts.above[1], g.halo, snap);
    put(&mut parts.above[2], g.thumb, snap);
}

/// Direct event entry: a pointer drag scrubs the slider 1:1 — snap the fill /
/// halo / thumb to `frac` with plain property sets (no repaint, no tick).
pub(crate) fn slider_drag(node: &mut Node, frac: f32) -> bool {
    let g = slider_geom(node.rect.w, node.rect.h, frac);
    let Some(parts) = node.parts.as_mut() else { return false };
    if parts.above.len() != 3 {
        return false;
    }
    slider_apply(parts, &g, true);
    parts.frac = frac;
    true
}

// ── Segmented (SelectorBar) ──────────────────────────────────────────────────

/// Below-band roles: `[tray fill, tray stroke, pill, hover ink]`.
fn segmented_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 4, 0) {
        return;
    }
    let n = node.ctrl.items.len();
    let (w, h) = (node.rect.w, node.rect.h);
    let accent = node.paint.style_variant == 1;
    let m = super::controls::seg_metrics(node.paint.style_variant, node.paint.font_size);
    let edges = super::controls::segment_edges(node);
    let edges_sig = edges.iter().sum::<f32>() + edges.len() as f32;
    let dim = dim_of(node);

    let tray_radius = if accent { h / 2.0 } else { theme::RADIUS_SM };
    let tray_bg = if accent { theme::w(0.06) } else { theme::stroke_subtle() };
    let pill_h = (h - 2.0 * m.tray).max(0.0);
    let seg_radius = if accent { pill_h / 2.0 } else { theme::RADIUS_BADGE };
    let pill_fill = if accent { theme::accent() } else { theme::stroke() };

    let k_tray = AtlasKey::hbar(h, tray_radius, 0.0, tray_bg, scale);
    let k_stroke = AtlasKey::hbar(h, tray_radius, theme::BORDER_W, theme::stroke(), scale);
    let k_pill = AtlasKey::hbar(pill_h, seg_radius, 0.0, pill_fill, scale);
    let k_ink = AtlasKey::hbar(pill_h, seg_radius, 0.0, theme::w(1.0), scale);

    let sel = if n == 0 { -1 } else { (node.ctrl.selected_index.max(0)).min(n as i32 - 1) };
    let seg_rect = |i: i32| -> Option<(f32, f32, f32, f32)> {
        let i = usize::try_from(i).ok()?;
        let (a, b) = (*edges.get(i)?, *edges.get(i + 1)?);
        Some((a, m.tray, b - a, pill_h))
    };
    let pill = seg_rect(sel);
    let hot = node.ctrl.hot_index;
    let ink = seg_rect(hot);
    let ink_t = seg_ink_target(node);

    let Some(parts) = node.parts.as_mut() else { return };
    let snap = !parts.init || parts.geom != (w, h) || parts.edges_sig != edges_sig;

    parts.below[0].bind(comp, atlas, k_tray);
    parts.below[1].bind(comp, atlas, k_stroke);
    parts.below[2].bind(comp, atlas, k_pill);
    parts.below[3].bind(comp, atlas, k_ink);

    parts.below[0].place(0.0, 0.0, w, h);
    parts.below[1].place(0.0, 0.0, w, h);
    parts.below[0].set_opacity(dim);
    parts.below[1].set_opacity(dim);

    match pill {
        Some(r) => {
            if snap || parts.sel == sel {
                parts.below[2].place(r.0, r.1, r.2, r.3);
            } else {
                parts.below[2].glide(r.0, r.1, r.2, r.3);
            }
            parts.below[2].set_opacity(dim);
        }
        None => parts.below[2].set_opacity(0.0),
    }
    if let Some(r) = ink {
        parts.below[3].place(r.0, r.1, r.2, r.3);
    }
    if parts.init {
        parts.below[3].fade_to(ink_t);
    } else {
        parts.below[3].set_opacity(ink_t);
    }

    parts.sel = sel;
    parts.geom = (w, h);
    parts.edges_sig = edges_sig;
    parts.init = true;
}

fn seg_ink_target(node: &Node) -> f32 {
    if node.paint.is_enabled && node.hovered && node.ctrl.hot_index >= 0 {
        wash(0.05)
    } else {
        0.0
    }
}

/// Direct event entry: the hovered segment changed — snap the ink to the hot
/// segment and refade. (The caller still repaints the surface for the label
/// hover brightening.)
pub(crate) fn seg_hot_changed(node: &mut Node) {
    let m = super::controls::seg_metrics(node.paint.style_variant, node.paint.font_size);
    let edges = super::controls::segment_edges(node);
    let pill_h = (node.rect.h - 2.0 * m.tray).max(0.0);
    let hot = node.ctrl.hot_index;
    let rect = usize::try_from(hot).ok().and_then(|i| {
        let (a, b) = (*edges.get(i)?, *edges.get(i + 1)?);
        Some((a, m.tray, b - a, pill_h))
    });
    let t = seg_ink_target(node);
    if let Some(parts) = node.parts.as_mut()
        && parts.below.len() == 4
    {
        if let Some(r) = rect {
            parts.below[3].place(r.0, r.1, r.2, r.3);
        }
        parts.below[3].fade_to(t);
    }
}

// ── NavigationView (icon rail) ───────────────────────────────────────────────

/// Below-band roles: `[rail background, active tile, accent bar]`.
fn nav_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 3, 0) {
        return;
    }
    let h = node.rect.h;
    let dim = dim_of(node);
    let item_h = super::controls::NAV_ITEM_H;
    let sel = node.ctrl.selected_index;
    let visible = sel >= 0 && !node.ctrl.items.is_empty();
    let iy = sel.max(0) as f32 * item_h;

    let k_bg = AtlasKey::solid(theme::surface_sunken(), scale);
    let k_tile = AtlasKey::hbar(item_h - theme::SPACE_8, theme::RADIUS_SM, 0.0, theme::accent_fill(), scale);
    let bar_h = theme::SPACE_16;
    let k_bar = AtlasKey::hbar(bar_h, theme::BORDER_W, 0.0, theme::accent(), scale);

    let tile = (
        theme::SPACE_4,
        iy + theme::SPACE_4,
        theme::NAV_RAIL_W - theme::SPACE_8,
        item_h - theme::SPACE_8,
    );
    let bar = (0.0, iy + (item_h - bar_h) / 2.0, theme::BORDER_W * 3.0, bar_h);

    let Some(parts) = node.parts.as_mut() else { return };
    let snap = !parts.init || parts.geom != (theme::NAV_RAIL_W, h);

    parts.below[0].bind(comp, atlas, k_bg);
    parts.below[1].bind(comp, atlas, k_tile);
    parts.below[2].bind(comp, atlas, k_bar);
    parts.below[0].place(0.0, 0.0, theme::NAV_RAIL_W, h);
    parts.below[0].set_opacity(dim);

    if visible {
        if snap || parts.sel == sel {
            parts.below[1].place(tile.0, tile.1, tile.2, tile.3);
            parts.below[2].place(bar.0, bar.1, bar.2, bar.3);
        } else {
            parts.below[1].glide(tile.0, tile.1, tile.2, tile.3);
            parts.below[2].glide(bar.0, bar.1, bar.2, bar.3);
        }
        parts.below[1].set_opacity(dim);
        parts.below[2].set_opacity(dim);
    } else {
        parts.below[1].set_opacity(0.0);
        parts.below[2].set_opacity(0.0);
    }
    parts.sel = sel;
    parts.geom = (theme::NAV_RAIL_W, h);
    parts.init = true;
}

// ── Expander ─────────────────────────────────────────────────────────────────

/// Above: `[header ink]` — the hover/press wash over the header strip only
/// (the body below it stays wash-free). Chevron + header chrome are painted;
/// the chevron flip is a single event-driven repaint.
fn expander_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 0, 1) {
        return;
    }
    let header_h = theme::ROW_H + theme::SPACE_8;
    let w = node.rect.w;
    let key = AtlasKey::hbar(header_h, theme::RADIUS_MD, 0.0, theme::w(1.0), scale);
    let target = ink_target(node);
    let Some(parts) = node.parts.as_mut() else { return };
    parts.above[0].bind(comp, atlas, key);
    parts.above[0].place(0.0, 0.0, w, header_h);
    if parts.init {
        parts.above[0].fade_to(target);
    } else {
        parts.above[0].set_opacity(target);
        parts.init = true;
    }
    parts.geom = (w, node.rect.h);
}

// ── Progress (bar + ring) ────────────────────────────────────────────────────

/// One indeterminate sweep / revolution, matching the retired tick's
/// `phase += dt · 0.6` advance (a full cycle per `1 / 0.6` seconds).
const PROGRESS_CYCLE_SECS: f32 = 1.0 / 0.6;

/// The bar's lane height, mirroring the retired `paint_progress_bar`.
fn progress_bar_h(node_h: f32) -> f32 {
    node_h.min(6.0).max(4.0)
}

/// Below: `[track, determinate fill, indeterminate sweep segment]`. The
/// surface paints nothing — a value change glides the fill (spring size), and
/// the indeterminate sweep is a forever-looping compositor keyframe animation:
/// the app is fully idle while the bar animates. The node's container carries
/// an InsetClip (set at create) so the sweep slides in/out at the track edges
/// instead of overhanging them.
fn progress_sync(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    if !ensure(comp, node, 3, 0) {
        return;
    }
    let (w, h) = (node.rect.w, node.rect.h);
    let bar_h = progress_bar_h(h);
    let y = h / 2.0 - bar_h / 2.0;
    let dim = dim_of(node);
    let frac = (super::ctrl_value_frac(node) as f32).clamp(0.0, 1.0);
    let ind = node.ctrl.indeterminate;
    let k_track = AtlasKey::hbar(bar_h, bar_h / 2.0, 0.0, theme::w(0.08), scale);
    let k_fill = AtlasKey::hbar(bar_h, bar_h / 2.0, 0.0, theme::accent(), scale);

    let Some(parts) = node.parts.as_mut() else { return };
    let snap = !parts.init || parts.geom != (w, h);

    parts.below[0].bind(comp, atlas, k_track);
    parts.below[1].bind(comp, atlas, k_fill);
    parts.below[2].bind(comp, atlas, k_fill);
    parts.below[0].place(0.0, y, w, bar_h);
    parts.below[0].set_opacity(dim);

    if ind {
        parts.below[1].set_opacity(0.0);
        // A travelling lit segment (one-third width), sweeping forever.
        let seg_w = w * 0.33;
        if snap || !parts.looping {
            parts.below[2].place(-seg_w, y, seg_w, bar_h);
            parts.looping = parts.below[2].loop_x(-seg_w, w, PROGRESS_CYCLE_SECS);
        }
        parts.below[2].set_opacity(dim);
    } else {
        if parts.looping {
            parts.below[2].stop_loop_x();
            parts.looping = false;
        }
        parts.below[2].set_opacity(0.0);
        // Floor the fill at one full pill so the nine-grid corners never
        // degenerate at tiny fractions.
        let fw = if frac > 0.0 { (w * frac).max(bar_h) } else { 0.0 };
        if snap || parts.frac == frac {
            parts.below[1].place(0.0, y, fw.max(0.01), bar_h);
        } else {
            parts.below[1].glide(0.0, y, fw.max(0.01), bar_h);
        }
        parts.below[1].set_opacity(if frac > 0.0 { dim } else { 0.0 });
        parts.frac = frac;
    }
    parts.geom = (w, h);
    parts.init = true;
}

/// The ring has no sprite parts — its track + arc stay painted (drawn once).
/// Indeterminate spin is a forever-looping `RotationAngle` animation on the
/// painted surface sprite itself: the track ring is rotation-invariant, so
/// only the arc appears to revolve, and the app never ticks.
fn ring_sync(comp: &Compositing, node: &mut Node) {
    if !ensure(comp, node, 0, 0) {
        return;
    }
    let (w, h) = (node.rect.w, node.rect.h);
    let ind = node.ctrl.indeterminate;
    let Some(surf) = node.surf.as_ref() else { return };
    let (Ok(vis), Ok(obj)) = (
        surf.sprite.cast::<IVisual>(),
        surf.sprite.cast::<ICompositionObject>(),
    ) else {
        return;
    };
    let Some(parts) = node.parts.as_mut() else { return };

    if ind {
        if !parts.looping || parts.geom != (w, h) {
            let _ = vis.SetCenterPoint(Vector3::new(w / 2.0, h / 2.0, 0.0));
            let run = || -> Option<()> {
                let c = obj.Compositor().ok()?;
                let lin: CompositionEasingFunction =
                    c.CreateLinearEasingFunction().ok()?.cast().ok()?;
                let a = c.CreateScalarKeyFrameAnimation().ok()?;
                a.InsertKeyFrameWithEasingFunction(0.0, 0.0, &lin).ok()?;
                a.InsertKeyFrameWithEasingFunction(1.0, std::f32::consts::TAU, &lin)
                    .ok()?;
                let kf: IKeyFrameAnimation = a.cast().ok()?;
                kf.SetDuration(ts_secs(PROGRESS_CYCLE_SECS)).ok()?;
                kf.SetIterationBehavior(AnimationIterationBehavior::Forever).ok()?;
                let _ = obj.StopAnimation("RotationAngle");
                obj.StartAnimation("RotationAngle", &a.cast::<CompositionAnimation>().ok()?)
                    .ok()
            };
            parts.looping = run().is_some();
        }
    } else if parts.looping {
        let _ = obj.StopAnimation("RotationAngle");
        let _ = vis.SetRotationAngle(0.0);
        parts.looping = false;
    }
    parts.geom = (w, h);
    parts.init = true;
}

// ─────────────────────────────────────────────────────────────────────────────
// Caret — the focused text editor's blinking insertion bar
// ─────────────────────────────────────────────────────────────────────────────

/// The focused editor's caret: a 1-DIP sprite above the painted text whose
/// blink is an INFINITE square-wave opacity animation evaluated by the system
/// compositor. The app touches it only on input edges (type / caret move /
/// focus / activation) — no timer, no per-blink repaint.
pub(crate) struct Caret {
    sprite: SpriteVisual,
    vis: IVisual,
    obj: ICompositionObject,
    /// The atlas source currently bound + the epoch it came from.
    key: Option<AtlasKey>,
    epoch: u32,
    /// Last placed box (change-gated writes).
    rect: Option<(f32, f32, f32, f32)>,
    /// Whether the sprite is currently shown (blink running or solid).
    shown: bool,
}

impl Caret {
    /// Create the sprite as the TOPMOST child of the editor's container, above
    /// its painted surface.
    fn new(comp: &Compositing, node: &Node) -> Option<Self> {
        let sprite = comp.new_sprite().ok()?;
        let vis: IVisual = sprite.cast().ok()?;
        let obj: ICompositionObject = sprite.cast().ok()?;
        let v: Visual = sprite.cast().ok()?;
        node.container.Children().ok()?.InsertAtTop(&v).ok()?;
        Some(Self { sprite, vis, obj, key: None, epoch: 0, rect: None, shown: false })
    }

    /// Bind (or re-bind) the solid atlas source for `key` (same epoch contract
    /// as [`Part::bind`]).
    fn bind(&mut self, comp: &Compositing, atlas: &mut Atlas, key: AtlasKey) {
        if self.key == Some(key) && self.epoch == atlas.epoch {
            return;
        }
        let epoch = atlas.epoch;
        let Some(entry) = atlas.entry(comp, key) else { return };
        if let Ok(b) = entry.brush.cast::<CompositionBrush>()
            && self.sprite.SetBrush(&b).is_ok()
        {
            self.key = Some(key);
            self.epoch = epoch;
        }
    }

    fn place(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if self.rect != Some((x, y, w, h)) {
            let _ = self.vis.SetOffset(Vector3::new(x, y, 0.0));
            let _ = self.vis.SetSize(Vector2::new(w, h));
            self.rect = Some((x, y, w, h));
        }
    }

    /// Stop the blink and hide the sprite (blur / window deactivated).
    fn hide(&mut self) {
        if !self.shown {
            return;
        }
        let _ = self.obj.StopAnimation("Opacity");
        let _ = self.vis.SetIsVisible(false);
        self.shown = false;
    }

    /// (Re)start the blink solid-first — or pin a solid caret when the system
    /// blink is disabled (`GetCaretBlinkTime` of 0 / INFINITE).
    fn start_blink(&mut self, comp: &Compositing) {
        let _ = self.vis.SetIsVisible(true);
        self.shown = true;
        let interval = unsafe { crate::system_bindings::GetCaretBlinkTime() };
        if interval == 0 || interval == u32::MAX || self.blink(comp, interval).is_none() {
            // Blinking disabled (or animation setup failed): a solid caret.
            let _ = self.obj.StopAnimation("Opacity");
            let _ = self.vis.SetOpacity(1.0);
        }
    }

    /// A square wave on Opacity: solid for `interval_ms`, hidden for
    /// `interval_ms`, repeated forever — steps(1) easing holds each level and
    /// jumps at the segment boundary. Runs entirely on the DWM.
    fn blink(&self, comp: &Compositing, interval_ms: u32) -> Option<()> {
        let compositor = comp.compositor();
        let a = compositor.CreateScalarKeyFrameAnimation().ok()?;
        let kf: IKeyFrameAnimation = a.cast().ok()?;
        kf.SetDuration(ts_secs(interval_ms as f32 * 2.0 / 1000.0)).ok()?;
        kf.SetIterationBehavior(AnimationIterationBehavior::Forever).ok()?;
        let step: CompositionEasingFunction = compositor
            .cast::<ICompositor2>()
            .ok()?
            .CreateStepEasingFunction()
            .ok()?
            .cast()
            .ok()?;
        a.InsertKeyFrame(0.0, 1.0).ok()?;
        a.InsertKeyFrameWithEasingFunction(0.5, 0.0, &step).ok()?;
        a.InsertKeyFrameWithEasingFunction(1.0, 0.0, &step).ok()?;
        self.obj
            .StartAnimation("Opacity", &a.cast::<CompositionAnimation>().ok()?)
            .ok()
    }
}

/// Reconcile an editor node's caret sprite against the state just painted:
/// shown while the node is focused (and the window active), placed from the
/// same text metrics the paint used, blink restarted solid-first on caret
/// movement. Rides the repaint choke — every state change that can move the
/// caret already repaints the field.
pub(crate) fn sync_caret(comp: &Compositing, atlas: &mut Atlas, node: &mut Node, scale: f32) {
    let show = node.focused
        && node.paint.is_enabled
        && node.editor.as_ref().is_some_and(|e| e.caret_shown);
    if !show {
        if let Some(c) = &mut node.caret {
            c.hide();
        }
        return;
    }
    let Some(bx) = super::controls::editor_caret_box(node) else { return };
    if node.caret.is_none() {
        node.caret = Caret::new(comp, node);
    }
    let Some(mut caret) = node.caret.take() else { return };
    caret.bind(comp, atlas, AtlasKey::solid(theme::text(), scale));
    caret.place(bx.left, bx.top, bx.width(), bx.height());
    let moved = node.editor.as_ref().is_some_and(|e| e.caret_moved);
    if moved || !caret.shown {
        caret.start_blink(comp);
    }
    node.caret = Some(caret);
    if let Some(e) = node.editor.as_mut() {
        e.caret_moved = false;
    }
}
