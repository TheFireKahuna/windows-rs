//! Gradient colour sources built on the compositor, not on a raster.
//!
//! ## The rule, restated
//!
//! The compositor may carry ALPHA, never COLOUR (see [`super::path_shape`]).
//! Measured on a 240-nit Advanced Color desktop where paper white is scRGB 3.0:
//! an effect-graph brush clamps at 1.0; a compositor-carried gradient *colour*
//! clamps at 2.9570 **and** bends the transfer function on the way (it reads
//! 0.8955 where 1.970 was asked for); an alpha mask over an app-allocated
//! `Rgba16Float` source reaches 3.9668 against an ideal 3.970, ramp
//! linear-exact. So colour stays in FP16 surfaces. Always.
//!
//! What is new here is that a `CompositionLinearGradientBrush` is a legal
//! `CompositionMaskBrush.Mask` — undocumented, but real, and unclamped because
//! only its alpha is read. That is what lets a ramp stop being a raster.
//!
//! ## Single hue: the mask carries the SHAPE, the source carries the LEVEL
//!
//! The compositor's alpha intermediate is 8 bits, so a ramp's quality is decided
//! by how much of `0..1` it uses. A spectrum bar's body fades `0.18 -> 0.03`;
//! authored literally that is 0.15 of the range, 37 distinct levels down a
//! 300-row strip and visible banding. Normalized — mask `1.0 -> 0.1667`, with
//! the 0.18 moved into the FP16 source's brightness — the composited ramp is
//! identical and it has 229 levels.
//!
//! So: the mask is the ramp's shape stretched to the full alpha range, and the
//! source is one flat FP16 surface at the ramp's peak alpha. Because a surface
//! at alpha `A` is stored premultiplied as `(c·A, A)`, masking it with `a/A`
//! yields `(c·a, a)` — bit for bit what the rasterized ramp produced.
//!
//! ## Multi hue: a staircase of layers
//!
//! A multi-stop ramp is piecewise-linear interpolation between constant colours,
//! and source-over compositing **is** that interpolation when the alphas
//! partition: `result = c₀·(1−α) + c₁·α`. So an opaque base of `c₀`, then one
//! layer per later stop whose source is a flat FP16 `cₙ` and whose mask ramps
//! `0 → 1` across that stop's segment and holds `1` after it. Every segment
//! spans the whole alpha range, so every one of them is the normalized case.
//!
//! Those layers are visuals, and a stack of visuals is not a brush. They are
//! composited into a container and captured once by a
//! [`CompositionVisualSurface`] — which is safe **only** because what is
//! captured is a fully opaque COLOUR field. Capturing a ramp as alpha mangles it
//! by ~40% (measured); capturing the composited colour costs 0.6% of ceiling
//! (3.9414 against 3.9668) and nothing of linearity, because the alpha channel
//! inside the container is 1 everywhere and the 8-bit alpha intermediate has
//! nothing to quantize.
//!
//! Any alpha ramp over a multi-hue source is then a SECOND mask outside the
//! capture, never inside it — one gradient brush with the authored alphas.
//!
//! ## Where the two alphas meet
//!
//! A ramp has to be confined to a shape, so shape-alpha and ramp-alpha must
//! multiply — and a `CompositionMaskBrush` has exactly one `Mask`. Two routes
//! were measured and both are closed:
//!
//! * **nesting** a mask brush inside another is rejected outright —
//!   `Source` throws `E_INVALIDARG`, *"Unsupported source brush type"*. A mask's
//!   source is always a surface brush.
//! * **painting the mask SHAPE with the ramp** binds and looks plausible, and is
//!   wrong: the shape is captured through a `CompositionVisualSurface`, and a
//!   capture does not carry alpha linearly. Measured on a full-box white ramp,
//!   the composited mid-point read `0.302` where `0.500` was authored — 40% low,
//!   with an effective exponent around 1.7 that flattens toward the ends. That is
//!   the same mangling the standalone probe found, and no ramp may cross a
//!   capture.
//!
//! What a capture DOES carry exactly is COLOUR. The multi-hue staircase below is
//! read back bit-accurate through one (`0.6743` measured against `0.6745`
//! computed, on every channel of every segment).
//!
//! So the ramp goes LAST, outside every capture — [`RampStage`]. The layer's
//! existing shape mask paints an off-tree sprite, that sprite is captured (colour
//! and coverage, no ramp), and the ramp masks the capture as a live gradient
//! brush. The shape mechanism above it is untouched: same geometry, same trim,
//! same white mask shape.
//!
//! A caller with no shape of its own (a spectrum bar is a plain sprite) needs no
//! stage at all — it takes [`brush`](RampSource::brush), one mask brush with the
//! ramp as its mask and the FP16 colour as its source.
//!
//! ## And nothing re-rasterizes
//!
//! `MappingMode::Relative` measures the ramp in fractions of the visual it
//! paints, so a resize moves the ramp with the sprite and costs no draw at all.
//! Only the flat FP16 sources are rasterized, they are one atlas cell each, and
//! they are shared with every other solid in the backend.

use windows_composition::{
    Brush, CompositionBrush, CompositionLinearGradientBrush, CompositionMaskBrush,
    CompositionSurfaceBrush, CompositionVisualSurface, ContainerVisual, MappingMode, SpriteVisual,
    Stretch,
};
use windows_numerics::Vector2;

use super::bootstrap::Compositing;
use crate::{Color, GradientAxis};

/// A colour source for one gradient ramp: the FP16 colour a mask should reveal,
/// the alpha ramp that shapes it, and whatever retained compositor objects have
/// to outlive both.
pub(crate) struct RampSource {
    /// The colour, always a SURFACE brush — a mask brush's `Source` accepts
    /// nothing else. Flat FP16 for a single hue; the staircase's capture for
    /// many.
    colour: CompositionSurfaceBrush,
    /// The ramp, as alpha. A layer with a shape paints its MASK SHAPE with this;
    /// a bare sprite takes [`Self::brush`], which masks `colour` with it.
    ramp: CompositionLinearGradientBrush,
    /// The standalone form, built once so a caller with no shape of its own
    /// binds one object.
    brush: CompositionBrush,
    /// The multi-hue staircase, when there is one. Its container has to be sized
    /// (its layers are relative to it) and its capture has to be told the same
    /// extent — everything else here is resolution-independent.
    stack: Option<Staircase>,
    /// Held so the compositor keeps them alive: the FP16 sources and the mask
    /// and gradient brushes are otherwise only referenced by properties.
    _keep: Vec<CompositionBrush>,
}

/// The composited layer stack behind a multi-hue ramp, and the surface that
/// captures it.
struct Staircase {
    container: ContainerVisual,
    capture: CompositionVisualSurface,
}

/// What either route returns: the colour surface, the alpha ramp, the staircase
/// (if any), and the objects to keep alive.
type Built = (
    CompositionSurfaceBrush,
    CompositionLinearGradientBrush,
    Option<Staircase>,
    Vec<CompositionBrush>,
);

impl RampSource {
    /// Build the source for `stops` running along `axis`.
    ///
    /// `None` when the ramp cannot be expressed — an empty stop list, a ramp
    /// transparent throughout, or an FP16 source the atlas declined to hand
    /// back. The caller keeps whatever it had.
    pub(crate) fn build(
        comp: &Compositing,
        stops: &[(f64, Color)],
        axis: GradientAxis,
        scale: f32,
    ) -> Option<Self> {
        if stops.is_empty() {
            return None;
        }
        let (colour, ramp, stack, mut keep) = if single_hue(stops) {
            Self::single_hue(comp, stops, axis, scale)?
        } else {
            Self::multi_hue(comp, stops, axis, scale)?
        };
        let mask = comp.compositor().create_mask_brush();
        mask.set_mask(&ramp);
        mask.set_source(&colour);
        keep.push(mask.as_brush());
        Some(Self { colour, ramp, brush: mask.as_brush(), stack, _keep: keep })
    }

    /// The FP16 colour a shape-masked layer should reveal — its mask brush's
    /// `Source`.
    pub(crate) fn colour(&self) -> &CompositionSurfaceBrush {
        &self.colour
    }

    /// The alpha ramp a shape-masked layer should draw its MASK SHAPE with, so
    /// the captured coverage is already `shape x ramp`.
    pub(crate) fn ramp(&self) -> &CompositionLinearGradientBrush {
        &self.ramp
    }

    /// The whole ramp as one brush, for a caller with no shape of its own.
    pub(crate) fn brush(&self) -> &CompositionBrush {
        &self.brush
    }

    /// Give the staircase an extent. A no-op for a single-hue ramp, which is
    /// `MappingMode::Relative` end to end and has no extent of its own.
    ///
    /// The container is sized in PHYSICAL pixels because that is what its
    /// capture reads, and the display sprite that finally paints the capture is
    /// in DIPs under the root scale — the same split every mask in this backend
    /// makes.
    pub(crate) fn resize(&self, w: f32, h: f32, scale: f32) {
        let Some(stack) = self.stack.as_ref() else {
            return;
        };
        let phys = Vector2::new((w * scale).max(1.0), (h * scale).max(1.0));
        stack.container.set_size(phys.x, phys.y);
        stack.capture.set_source_size(phys);
    }

    // -- single hue ---------------------------------------------------------

    fn single_hue(
        comp: &Compositing,
        stops: &[(f64, Color)],
        axis: GradientAxis,
        scale: f32,
    ) -> Option<Built> {
        let peak = stops.iter().fold(0.0f32, |m, (_, c)| m.max(c.a));
        if peak <= 0.0 {
            return None;
        }
        let hue = stops[0].1;
        // The FP16 source at the ramp's PEAK alpha. Premultiplied storage makes
        // this exact: `(c*peak, peak)` masked by `a/peak` is `(c*a, a)`.
        let colour = super::parts::build_solid_surface(
            comp,
            Color { r: hue.r, g: hue.g, b: hue.b, a: peak },
            scale,
        )?;
        let ramp = ramp_brush(
            comp,
            axis,
            &stops.iter().map(|(o, c)| (*o as f32, c.a / peak)).collect::<Vec<_>>(),
        );
        let keep = vec![colour.as_brush(), ramp.as_brush()];
        Some((colour, ramp, None, keep))
    }

    // -- multi hue ----------------------------------------------------------

    fn multi_hue(
        comp: &Compositing,
        stops: &[(f64, Color)],
        axis: GradientAxis,
        scale: f32,
    ) -> Option<Built> {
        let compositor = comp.compositor();
        let container = compositor.create_container_visual();
        let mut keep: Vec<CompositionBrush> = Vec::with_capacity(stops.len() * 3);

        // The base: `c0` opaque across the whole box, so the captured field has
        // alpha 1 everywhere and every later layer composites over a defined
        // colour rather than over nothing.
        let base = comp.new_sprite();
        let base_src = super::parts::build_solid_surface(comp, opaque(stops[0].1), scale)?;
        base.set_brush(&base_src);
        base.set_relative_size_adjustment(Vector2::new(1.0, 1.0));
        keep.push(base_src.as_brush());
        container.children().insert_at_top(&base);

        // One layer per later stop: its colour, revealed across its own segment
        // and held from there on. Holding is what makes source-over an
        // interpolation rather than a pile-up — a layer must fully occlude
        // everything below it past its own stop.
        for pair in stops.windows(2) {
            let (from, to) = (pair[0].0 as f32, pair[1].0 as f32);
            let src = super::parts::build_solid_surface(comp, opaque(pair[1].1), scale)?;
            let seg = ramp_brush(
                comp,
                axis,
                &[(0.0, 0.0), (from, 0.0), (to.max(from), 1.0), (1.0, 1.0)],
            );
            let mask = compositor.create_mask_brush();
            mask.set_mask(&seg);
            mask.set_source(&src);
            let layer = comp.new_sprite();
            layer.set_brush(&mask);
            layer.set_relative_size_adjustment(Vector2::new(1.0, 1.0));
            keep.extend([src.as_brush(), seg.as_brush(), mask.as_brush()]);
            container.children().insert_at_top(&layer);
        }

        // Flatten the stack to one surface. The container is off-tree, so state
        // its edge quality rather than inheriting from a parent it does not have.
        container.set_border_mode(windows_composition::BorderMode::Soft);
        let capture = compositor.create_visual_surface();
        capture.set_source_visual(&container);
        capture.set_source_offset(Vector2::new(0.0, 0.0));
        let colour: CompositionSurfaceBrush = compositor.create_surface_brush(&capture);
        colour.set_stretch(Stretch::Fill);
        keep.push(colour.as_brush());

        // The ramp in ALPHA is authored OUTSIDE the capture — inside it the
        // alpha channel must stay 1, and a captured alpha ramp is the case
        // measured to mangle. Opaque stops leave it flat, which costs nothing.
        let ramp = ramp_brush(
            comp,
            axis,
            &stops.iter().map(|(o, c)| (*o as f32, c.a.min(1.0))).collect::<Vec<_>>(),
        );
        keep.push(ramp.as_brush());
        Some((colour, ramp, Some(Staircase { container, capture }), keep))
    }
}

/// Whether every stop shares one RGB, so the ramp is a fade of one colour and
/// needs no layer stack at all.
fn single_hue(stops: &[(f64, Color)]) -> bool {
    let first = stops[0].1;
    stops
        .iter()
        .all(|(_, c)| c.r == first.r && c.g == first.g && c.b == first.b)
}

/// `c` at full alpha — the staircase carries alpha separately.
fn opaque(c: Color) -> Color {
    Color { a: 1.0, ..c }
}

/// A `(offset, alpha)` ramp along `axis`, measured in fractions of whatever
/// visual ends up painted with it.
fn ramp_brush(
    comp: &Compositing,
    axis: GradientAxis,
    stops: &[(f32, f32)],
) -> CompositionLinearGradientBrush {
    let brush = comp.compositor().create_linear_gradient_brush();
    brush.set_mapping_mode(MappingMode::Relative);
    brush.set_line(
        Vector2::new(0.0, 0.0),
        match axis {
            GradientAxis::Horizontal => Vector2::new(1.0, 0.0),
            GradientAxis::Vertical => Vector2::new(0.0, 1.0),
        },
    );
    brush.set_alpha_stops(comp.compositor(), stops);
    brush
}


/// The last stage of a shape-confined ramp: the layer's shape-masked colour,
/// captured, then masked by the ramp as a live gradient.
///
/// One indirection buys the thing that cannot be had any other way — two alphas
/// multiplied with neither of them crossing a capture. The inner brush (shape ×
/// colour) is what the layer already had; this only re-homes it onto an off-tree
/// sprite and puts the ramp on top.
///
/// Built once per layer that ever shows a ramp and then retargeted: a recolour or
/// a new stop list is one `set_mask`, and a layer that goes back to a flat colour
/// simply binds its inner brush to the display again.
pub(crate) struct RampStage {
    /// Off-tree sprite painted with the layer's own shape mask. Sized in
    /// PHYSICAL pixels, like every capture source in this backend.
    sprite: SpriteVisual,
    capture: CompositionVisualSurface,
    outer: CompositionMaskBrush,
    _surface: CompositionSurfaceBrush,
}

impl RampStage {
    /// Wrap `inner` — the layer's `MaskBrush(shape, colour)` — in a capture the
    /// ramp can mask.
    pub(crate) fn new(comp: &Compositing, inner: &CompositionMaskBrush) -> Self {
        let compositor = comp.compositor();
        let sprite = comp.new_sprite();
        sprite.set_brush(inner);
        // Off-tree and captured: state the edge quality rather than inheriting
        // from a parent that does not exist.
        sprite.set_border_mode(windows_composition::BorderMode::Soft);

        let capture = compositor.create_visual_surface();
        capture.set_source_visual(&sprite);
        capture.set_source_offset(Vector2::new(0.0, 0.0));
        let surface = compositor.create_surface_brush(&capture);
        surface.set_stretch(Stretch::Fill);

        let outer = compositor.create_mask_brush();
        outer.set_source(&surface);
        Self { sprite, capture, outer, _surface: surface }
    }

    /// The brush the display sprite binds while a ramp is showing.
    pub(crate) fn brush(&self) -> &CompositionMaskBrush {
        &self.outer
    }

    /// Point the stage at a new ramp. One property write.
    pub(crate) fn set_ramp(&self, ramp: &CompositionLinearGradientBrush) {
        self.outer.set_mask(ramp);
    }

    /// Size the captured sprite, in physical pixels — the display sprite that
    /// paints the result stays in DIPs under the root scale.
    pub(crate) fn resize(&self, w: f32, h: f32, scale: f32) {
        let phys = Vector2::new((w * scale).max(1.0), (h * scale).max(1.0));
        self.sprite.set_size(phys.x, phys.y);
        self.capture.set_source_size(phys);
    }
}
