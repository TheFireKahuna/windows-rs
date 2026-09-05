//! Realizes a sprite's brush chain: alpha × light onto one sprite visual. **Front half.**
//!
//! ```text
//! Sprite.Brush = MaskBrush { Mask = <box | run | shape alpha>, Source = <paint brush> }
//! ```
//!
//! The chain is flat. A mask brush is never the mask or the source of another, because the
//! platform throws on that brush combination, so a gradient is one premultiplied FP16 strip.
//!
//! Every paint is a surface brush. The four variants differ only in where the surface comes
//! from, so there is one binding type and one device-loss rebind, and the paint enum picks
//! a constructor rather than a shape.

use crate::backends::Backends;
use crate::cache::{BoxKey, Cells, Gen, SolidKey};
use crate::env::Env;
use crate::node::{Node, Painted, Route, ShadowState, ShapeState};
use crate::prop;
use crate::res::Resources;
use crate::sink::{Cap, Corners, GeomId, Halo, Join, Mask, Paint, Prop, SpriteId, StrokeStyle};
use windows_color::{Radiance, Scrgb};
use windows_composition::{
    BorderMode, Brush, Color, CompositionBrush, CompositionSurfaceBrush, ShadowSource, StrokeCap,
    StrokeJoin, Visual,
};
use windows_core::Result;
use windows_numerics::Vector2;

/// Returns the construction that realizes a shape mask.
///
/// A total function of the mask's value and the sprite's bound channels, so an author never
/// names a route. A clip-route sprite that later receives a trim, a dash phase or its own
/// clip is promoted onto the capture with the same geometry, so a shape's clip colliding
/// with the sink's own costs a promotion rather than a wrong render.
pub(crate) fn route(stroke: Option<StrokeStyle>, draws_on: bool, clip_taken: bool) -> Route {
    if stroke.is_some() || draws_on || clip_taken {
        Route::Capture
    } else {
        Route::Clip
    }
}

/// Borrows everything realizing a sprite reaches for, and nothing else.
///
/// Named once so the functions below thread one borrow set, and so what they touch is a
/// fact the compiler checks. The node arrives separately and mutably, because a realize
/// writes one.
///
/// The fields are the backends brushes are created from, the display environment they are
/// built for, the generation saying what has been invalidated since, and the two stores a
/// chain is assembled out of.
pub(crate) struct Realizer<'a> {
    pub(crate) back: &'a Backends,
    pub(crate) env: Env,
    pub(crate) generation: Gen,
    pub(crate) res: &'a Resources,
    pub(crate) cells: &'a mut Cells,
}

impl Realizer<'_> {
    /// Builds or rebuilds a sprite's brush chain from the declaration held on its node.
    ///
    /// Everything needed is on the node already, so device-loss recovery, a DPI change and
    /// a first bind are the same call: every brush is a pure function of a cache key or a
    /// resource id, and the stroke pattern is held rather than re-read from a patch that may
    /// no longer exist.
    ///
    /// `glow` is the captured group's visual. The caller resolves it, because it belongs to
    /// a different node.
    pub(crate) fn sprite(&mut self, node: &mut Node, glow: Option<&Visual>) -> Result<()> {
        let Some((mask, paint, halo, dashes, owned_clip)) = node
            .painted
            .as_ref()
            .map(|p| (p.mask, p.paint, p.halo, p.dashes, p.owns_the_clip()))
        else {
            return Ok(());
        };

        let paint_brush = self.paint(node, &paint, glow)?;
        // After the paint, because a captured glow claims the same slot on the node and the
        // two must not take turns owning it. A sprite declares one or the other.
        self.halo(node, &paint, halo);
        let (mask_brush, route, insets) = self.mask(node, &mask, dashes.as_slice())?;

        // A shape leaving the clip route takes its clip with it, or it is masked twice by
        // itself and an outward stroke is cut in half along the fill's own outline.
        //
        // `clip.is_none()` is the whole condition, not a guard: the other way onto the
        // capture is the sink claiming the slot, and there the sink's clip is already on the
        // visual.
        if owned_clip && route == Route::Capture && node.clip.is_none() {
            node.visual.clear_clip();
        }
        // The reverse: a mask that stops being a shape leaves a capture behind whose
        // channels would keep taking writes nothing renders.
        if route == Route::Clip && !matches!(mask, Mask::Shape { .. }) {
            node.shape = None;
        }

        let combined = match (&node.sprite, &mask_brush, &paint_brush) {
            // No mask brush: the paint binds directly. A presented buffer requires this,
            // since a mask brush in the chain disqualifies it from a display plane, and it
            // is also what the clip route amounts to.
            (Some(sprite), None, Some(paint)) => {
                sprite.set_brush(paint);
                None
            }
            (Some(sprite), Some(mask), Some(paint)) => {
                let combined = self.back.compositor.create_mask_brush();
                combined.set_mask(mask);
                combined.set_source(paint);
                sprite.set_brush(&combined);
                Some(combined)
            }
            // Nothing to paint with yet. A mask and a paint arrive as separate ops and
            // either order is legal, so a half-declared sprite waits rather than failing.
            _ => None,
        };

        node.painted = Some(Painted {
            combined,
            mask_brush,
            paint_brush,
            mask,
            paint,
            halo,
            dashes,
            route,
            built_at: self.generation,
            insets,
        });
        Ok(())
    }

    /// Builds the alpha half of the chain, the route it took, and the nine-grid insets it
    /// used.
    ///
    /// The insets are `(0.0, 0.0)` for every mask but a rounded box. They come back to the
    /// caller rather than being written here because the caller replaces the whole `Painted`.
    fn mask(
        &mut self,
        node: &mut Node,
        mask: &Mask,
        dashes: &[f32],
    ) -> Result<(Option<CompositionBrush>, Route, (f32, f32))> {
        match *mask {
            // No mask: the paint's own alpha is the shape.
            Mask::None => Ok((None, Route::Clip, NO_INSETS)),

            Mask::Box { radius } => {
                // The profile is clamped against the box before the raster is cut, not after.
                // A nine-grid does not clamp its own insets: where two opposite insets exceed
                // the extent the corner slices overlap, and a pill comes out as a lens and a
                // circle as a diamond. Trimming the *insets* instead leaves the raster's arc
                // longer than the slice reading it, so the tail of the curve lands in the
                // stretched middle and smears — same shape, different cause. Cutting a
                // smaller profile is one more cache key and the corners stay exact.
                let key = BoxKey::new(fit(radius, node.size(), self.env.scale()), self.env.scale());
                let (inset, inset_scale) = nine_slice(&key, self.env.scale());
                let Some(cell) =
                    self.cells
                        .boxes
                        .brush(self.back, self.env, self.generation, &key)?
                else {
                    return Ok((None, Route::Clip, NO_INSETS));
                };
                // Nine-slice, so one raster serves any width and height with exact corners.
                // It reaches the mask slot as the base brush type, which is what that slot
                // accepts.
                let nine = self.back.compositor.create_nine_grid_brush();
                nine.set_source(cell);
                nine.set_insets(inset, inset, inset, inset);
                nine.set_inset_scales(inset_scale);
                Ok((Some(nine.as_brush()), Route::Clip, (inset, inset)))
            }

            Mask::Run(run) => Ok((
                self.res.runs.value(run).map(Brush::as_brush),
                Route::Clip,
                NO_INSETS,
            )),

            Mask::Shape { geom, stroke } => {
                let clip_taken = node.clip.is_some();
                match route(stroke, draws_on(node), clip_taken) {
                    Route::Clip => {
                        self.geometric_clip(node, geom);
                        Ok((None, Route::Clip, NO_INSETS))
                    }
                    Route::Capture => Ok((
                        self.shape_capture(node, geom, stroke, dashes)
                            .map(|b| b.as_brush()),
                        Route::Capture,
                        NO_INSETS,
                    )),
                }
            }
        }
    }

    /// Builds the colour half of the chain, which is always a surface brush.
    fn paint(
        &mut self,
        node: &mut Node,
        paint: &Paint,
        glow: Option<&Visual>,
    ) -> Result<Option<CompositionSurfaceBrush>> {
        match *paint {
            Paint::Solid(light) => {
                // The retained path's draw choke: the one place in this crate a
                // scene-referred value becomes a display-referred one.
                let key = SolidKey::new(self.env.apply(light));
                Ok(self
                    .cells
                    .solids
                    .brush(self.back, self.env, self.generation, &key)?
                    .cloned())
            }
            Paint::Ramp(ramp) => Ok(self.res.ramps.value(ramp).cloned()),
            Paint::Presented(region) => Ok(self.res.region(region).cloned()),
            Paint::Captured { blur, tint, .. } => {
                Ok(glow.and_then(|source| self.glow(node, source, blur, tint)))
            }
        }
    }

    /// Takes the clip route: no mask brush, the paint bound directly, and a geometric clip
    /// carrying the shape with a soft border for an antialiased edge.
    fn geometric_clip(&mut self, node: &mut Node, geom: GeomId) {
        let Some(geometry) = self.res.geoms.value(geom) else {
            return;
        };
        let clip = self.back.compositor.create_geometric_clip(geometry);
        node.visual.set_clip(Some(&clip));
        node.visual.set_border_mode(BorderMode::Soft);
    }

    /// Takes the capture route: an off-tree shape visual captured through a visual surface.
    ///
    /// A sprite shape's fill and stroke brushes do not accept a surface brush, so an FP16
    /// colour cannot reach a shape directly. The captured shape therefore carries alpha
    /// only, and its colour comes from the paint beside it.
    fn shape_capture(
        &mut self,
        node: &mut Node,
        geom: GeomId,
        stroke: Option<StrokeStyle>,
        dashes: &[f32],
    ) -> Option<CompositionSurfaceBrush> {
        let geometry = self.res.geoms.value(geom)?.clone();
        let size = node.size();
        let scale = self.env.scale();

        let host = self.back.compositor.create_shape_visual();
        crate::base_of_shape(&host).set_border_mode(BorderMode::Soft);
        let shape = self.back.compositor.create_sprite_shape(&geometry);
        // Opaque white: the capture is a mask, so its colour comes from the paint beside it,
        // and white is the multiplicative identity that leaves that paint alone.
        let white = self
            .back
            .compositor
            .create_color_brush(Color::rgb(255, 255, 255));
        match stroke {
            None => shape.set_fill_brush(&white),
            Some(k) => {
                shape.set_stroke_brush(&white);
                shape.set_stroke_thickness(k.width);
                shape.set_stroke_caps(cap_of(k.cap));
                shape.set_stroke_dash_cap(cap_of(k.cap));
                shape.set_stroke_join(join_of(k.join));
                shape.set_stroke_dashes(dashes);
            }
        }
        // The scale goes on the shape, not on the visual. A visual surface captures content
        // and ignores the source visual's own transform, so scaling the host would change
        // nothing about what lands in the surface.
        shape.set_scale(Vector2 { x: scale, y: scale });
        host.shapes().append(&shape);

        let captured = self
            .back
            .compositor
            .capture(&crate::base_of_shape(&host), size, scale);
        let brush = captured.brush.clone();

        // A promotion keeps whatever the channels had reached, so a shape that acquires a
        // trim mid-animation does not restart from the identity. Fresh, the trim window is
        // the whole path and the stroke is one DIP with no dash phase.
        let (trim, stroke) = node
            .shape
            .as_ref()
            .map_or(([0.0, 1.0], [1.0, 0.0]), |s| (s.trim, s.stroke));
        let state = ShapeState {
            host,
            captured,
            shape,
            geometry,
            trim,
            stroke,
        };
        // Sizes both extents from one rule, which is what keeps the host out of DIP space:
        // the shape above is scaled, and a host sized in DIPs would clip it.
        state.resize(size, scale);
        node.shape = Some(state);
        Some(brush)
    }

    /// Casts, or removes, the halo declared for this sprite.
    ///
    /// The silhouette is the sprite's own brush alpha, stated through
    /// [`ShadowSource::VisualAlpha`]: with no source policy the platform's default
    /// silhouette is a rectangle the size of the visual, which would square off every
    /// rounded box in the tree.
    ///
    /// A sprite painting [`Paint::Captured`] is left alone. That paint casts a shadow of
    /// its own onto the same slot, and a node holds one, so the two are exclusive by
    /// construction rather than by whichever ran last.
    fn halo(&mut self, node: &mut Node, paint: &Paint, halo: Option<Halo>) {
        if matches!(paint, Paint::Captured { .. }) {
            debug_assert!(
                halo.is_none(),
                "a captured glow and a halo were declared on one sprite"
            );
            return;
        }
        let Some(sprite) = node.sprite.clone() else {
            return;
        };
        let Some(halo) = halo else {
            sprite.clear_shadow();
            node.shadow = None;
            return;
        };
        let shadow = self.back.compositor.create_drop_shadow();
        shadow.set_source(ShadowSource::VisualAlpha);
        shadow.set_blur_radius(halo.blur);
        shadow.set_offset(halo.offset.x, halo.offset.y, 0.0);
        // Eight-bit, like every shadow colour, so the authored tint goes through the display
        // transform here and agrees with the sprite it sits behind.
        shadow.set_color(color_of(self.env.apply(halo.tint)));
        sprite.set_shadow(&shadow);
        // The blur a channel already animated survives a rebind: a device loss mid-hover
        // must not snap the halo back to its authored width.
        let chans = node.shadow.as_ref().map_or([halo.blur, 1.0], |s| s.chans);
        shadow.set_blur_radius(chans[0]);
        shadow.set_opacity(chans[1]);
        node.shadow = Some(ShadowState {
            shadow,
            captured: None,
            offset: halo.offset,
            chans,
        });
    }

    /// Captures a subtree, blurs it, tints it, and casts it behind the sprite.
    ///
    /// The halo under a curve stroke is a capture of that stroke, so the blur radius and the
    /// tint are carried by the paint variant.
    ///
    /// The shadow is cast by the sprite this paint belongs to and not by the subtree it
    /// captures, which puts the blurred silhouette under the real stroke. The state
    /// therefore lands on `node`: the glow's channels are addressed to the sprite, and a
    /// shadow parked on any other node could never take a write.
    fn glow(
        &mut self,
        node: &mut Node,
        source: &Visual,
        blur: f32,
        tint: Radiance,
    ) -> Option<CompositionSurfaceBrush> {
        let captured = self
            .back
            .compositor
            .capture(source, node.size(), self.env.scale());
        let brush = captured.brush.clone();

        let shadow = self.back.compositor.create_drop_shadow();
        shadow.set_blur_radius(blur);
        shadow.set_mask(&brush);
        // At zero offset a shadow is a glow. Its colour is eight-bit and never exceeds the
        // display's white, so the authored tint goes through the display transform first
        // and agrees with everything beside it.
        shadow.set_offset(0.0, 0.0, 0.0);
        shadow.set_color(color_of(self.env.apply(tint)));
        node.sprite.as_ref()?.set_shadow(&shadow);

        let chans = node.shadow.as_ref().map_or([blur, 1.0], |s| s.chans);
        node.shadow = Some(ShadowState {
            shadow,
            captured: Some(captured),
            offset: Vector2 { x: 0.0, y: 0.0 },
            chans,
        });
        Some(brush)
    }
}

/// Returns `true` where a channel only the capture route can carry is held.
///
/// Trim and the dash phase live on a sprite shape, so a clip cannot express them. A held
/// channel counts whether or not its animation is running, so a control does not demote to
/// the clip route between two hovers.
fn draws_on(node: &Node) -> bool {
    [Prop::TrimStart, Prop::TrimEnd, Prop::DashOffset]
        .iter()
        .any(|&p| prop::held(node, prop::desc(p).group) != prop::Held::Free)
}

fn cap_of(cap: Cap) -> StrokeCap {
    match cap {
        Cap::Flat => StrokeCap::Flat,
        Cap::Square => StrokeCap::Square,
        Cap::Round => StrokeCap::Round,
        Cap::Triangle => StrokeCap::Triangle,
    }
}

fn join_of(join: Join) -> StrokeJoin {
    match join {
        Join::Miter => StrokeJoin::Miter,
        Join::Bevel => StrokeJoin::Bevel,
        Join::Round => StrokeJoin::Round,
        Join::MiterOrBevel => StrokeJoin::MiterOrBevel,
    }
}

/// Narrows a display-referred colour to the compositor's eight-bit `Color`, for the shadow
/// slot, which accepts nothing wider.
fn color_of(c: Scrgb) -> Color {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    Color::rgba(byte(c.r), byte(c.g), byte(c.b), byte(c.a))
}

impl crate::Scene {
    /// Realizes a sprite, first resolving the captured group's visual, which lives on
    /// another node.
    pub(crate) fn rebind(&mut self, id: SpriteId, back: &Backends, env: Env) -> Result<()> {
        let glow = match self.nodes.get(id.node()).and_then(|n| n.painted.as_ref()) {
            Some(p) => match p.paint {
                Paint::Captured { group, .. } => {
                    self.nodes.get(group.node()).map(|n| n.visual.clone())
                }
                _ => None,
            },
            None => return Ok(()),
        };
        let generation = self.generation;
        let Some(node) = self.nodes.get_mut(id.node()) else {
            return Ok(());
        };
        Realizer {
            back,
            env,
            generation,
            res: &self.res,
            cells: &mut self.cells,
        }
        .sprite(node, glow.as_ref())
    }
}

/// The insets a mask that is not a rounded box realizes with.
pub(crate) const NO_INSETS: (f32, f32) = (0.0, 0.0);

/// Returns the nine-grid inset and inset scale that paint `key`'s raster one raster pixel to
/// one physical pixel.
///
/// **The inset cuts the source**, so it is stated in the raster's own units, and the raster
/// is allocated in physical pixels. A smaller number leaves the tail of the arc outside the
/// corner slice, in the middle one, which stretches it across the box.
///
/// What the compositor paints is `inset × inset_scale` in the *visual's* units, and the
/// visual hangs under a root carrying the display scale. At the default scale of one, a
/// corner cut at `n` pixels is painted `n` DIPs — `n · scale` pixels — and a box shorter than
/// twice that has its opposite corners overlap, which is a stadium whatever radius it asked
/// for. `1 / scale` is what puts the corner back on the pixels it was drawn for.
pub(crate) fn nine_slice(key: &BoxKey, scale: f32) -> (f32, f32) {
    (key.inset_px(), 1.0 / scale)
}

/// Returns `radius` clamped to what a `size` box can carry, in DIPs.
///
/// A nine-grid does not clamp its own insets, and an inset covers the whole arc: the raster
/// is two insets and the one flat pixel the middle slice stretches from. So a box has to
/// hold `2·inset + 1` pixels on its shorter axis, and a profile that asks for more is cut
/// down until it does. A fully round control is therefore always one pixel short of a
/// semicircle, which is the flat pixel.
///
/// The cap is solved in pixels, because that is the grid the raster is cut on: a cap taken
/// in DIPs is snapped back up to a whole pixel when the key is built, and a radius that
/// rounds up past its own cap overlaps the opposite corner by one pixel — a notch, not a
/// curve.
///
/// A box with no extent yet — what a sprite reads before its first size bind — keeps the
/// profile it asked for: its size arrives in the same patch and rebuilds this.
pub(crate) fn fit(radius: Corners, size: Vector2, scale: f32) -> Corners {
    if size.x <= 0.0 || size.y <= 0.0 {
        return radius;
    }
    let scale = scale.max(1.0);
    // Floored: a box covering a fraction of its last pixel cannot carry a profile cut for
    // the whole one.
    let extent = (size.x.min(size.y) * scale).floor();
    let cap = ((extent - 1.0) * 0.5).floor().max(0.0) / scale;
    Corners {
        tl: radius.tl.min(cap),
        tr: radius.tr.min(cap),
        br: radius.br.min(cap),
        bl: radius.bl.min(cap),
    }
}

#[cfg(test)]
mod tests {
    use super::{fit, nine_slice};
    use crate::cache::BoxKey;
    use crate::quant::snap_detail;
    use crate::sink::Corners;
    use windows_numerics::Vector2;

    /// A rounded box's corner is cut whole and painted at the size it was drawn.
    ///
    /// Two numbers reach the nine-grid and each has its own failure. An inset short of the
    /// arc leaves the tail of the curve in the middle slice, which stretches it across the
    /// box and smears the edge. An inset scale that does not undo the display scale paints
    /// the corner wider than the raster drew it, and a box under twice that comes out a
    /// stadium. Neither shows up as an error, and both look like a radius that was ignored.
    #[test]
    fn a_nine_grid_corner_is_cut_whole_and_painted_at_the_size_it_was_drawn() {
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            for radius in [1.0_f32, 2.5, 6.0, 7.92, 11.0] {
                let key = BoxKey::new(Corners::all(radius), scale);
                let (inset, inset_scale) = nine_slice(&key, scale);
                let arc = snap_detail(radius, scale) * scale;
                assert!(
                    inset >= arc,
                    "{radius} at {scale}x: an inset of {inset} cuts inside an arc of {arc}"
                );
                // The corner covers `inset * inset_scale` DIPs, and the raster drew it
                // `inset_px` pixels across.
                let painted = inset * inset_scale * scale;
                assert!(
                    (painted - key.inset_px()).abs() < 1.0e-3,
                    "{radius} at {scale}x: a corner drawn at {} px is painted at {painted}",
                    key.inset_px()
                );
            }
        }
    }

    /// A rounded box's profile never exceeds half the box it is cut for.
    ///
    /// The compositor does not clamp a nine-grid's insets: where two opposite insets exceed
    /// the extent the corner slices overlap. A pill exactly twice its own radius tall — a
    /// fully rounded control — comes out as a lens, and a circle as a diamond.
    #[test]
    fn a_rounded_profile_is_cut_to_fit_the_box_it_masks() {
        let at = |x: f32, y: f32| Vector2 { x, y };
        // What the box has to hold: two insets and the flat pixel between them, counted
        // through the key that cuts the raster rather than through a second formula.
        let fits = |r: f32, extent: f32, scale: f32| {
            let inset = crate::cache::BoxKey::new(Corners::all(r), scale).inset_px();
            inset * 2.0 + 1.0 <= (extent * scale).floor()
        };

        // Roomy: the profile is what it asked for.
        assert_eq!(
            fit(Corners::all(10.0), at(120.0, 40.0), 1.5),
            Corners::all(10.0)
        );
        // A pill exactly twice its radius tall — a fully rounded control, and the case that
        // rendered as a lens.
        let pill = fit(Corners::all(11.0), at(37.4, 22.0), 1.5).tl;
        assert!(fits(pill, 22.0, 1.5), "pill radius {pill} does not fit 22");
        // A knob asking to be a circle in a box that is not square, which rendered with its
        // corners cut into notches.
        let knob = fit(Corners::all(7.7), at(13.333374, 15.333344), 1.5).tl;
        assert!(
            fits(knob, 13.333374, 1.5),
            "knob radius {knob} does not fit 13.33"
        );
        // The cap is the *shorter* axis: a wide, short box rounds by its height.
        assert!(fit(Corners::all(40.0), at(400.0, 20.0), 1.5).tl < 10.0);
        // What a control asking to be fully round is left with: the cut costs it the one
        // flat pixel the middle slice stretches, and never more.
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            for extent in [15.0_f32, 20.0, 22.0, 13.333374] {
                let cut = fit(Corners::all(extent), at(extent, extent), scale).tl;
                assert!(
                    fits(cut, extent, scale),
                    "{extent} at {scale}x cut to {cut}"
                );
                let short = (extent * scale).floor();
                assert!(
                    cut * scale >= (short - 1.0) * 0.5 - 0.5,
                    "{extent} at {scale}x was cut to {cut}, well under half its box"
                );
            }
        }
        // A box with no extent yet keeps its profile: the size arrives in the same patch.
        assert_eq!(
            fit(Corners::all(11.0), at(0.0, 0.0), 1.5),
            Corners::all(11.0)
        );
    }
}
