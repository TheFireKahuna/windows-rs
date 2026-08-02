//! The universal construction: alpha × light → one sprite visual. **Front half.**
//!
//! ```text
//! Sprite.Brush = MaskBrush { Mask = <box | run | shape alpha>, Source = <paint brush> }
//! ```
//!
//! **The chain is flat, always.** A mask brush is never the mask or the source of another —
//! the platform's brush-combination table lists that under "using an unsupported
//! combination will throw an exception" — so a gradient is one premultiplied FP16 strip.
//!
//! **Every paint is a surface brush.** The four variants differ only in where the surface
//! comes from, so there is one binding type and one device-loss rebind, and the enum picks
//! a constructor, not a shape.

use crate::backends::Backends;
use crate::cache::{BoxKey, Cells, Gen, SolidKey};
use crate::env::Env;
use crate::node::{Node, Painted, Route, ShadowState, ShapeState};
use crate::prop;
use crate::res::Resources;
use crate::sink::{Cap, GeomId, Join, Mask, Paint, Prop, SpriteId, StrokeStyle};
use windows_color::{Radiance, Scrgb};
use windows_composition::{
    BorderMode, Brush, Color, CompositionBrush, CompositionSurfaceBrush, StrokeCap, StrokeJoin,
    Visual,
};
use windows_core::Result;
use windows_numerics::Vector2;

/// Which construction realizes a shape mask.
///
/// **The author never names a route.** A total function of the mask's value and the
/// sprite's bound channels; a clip-route sprite that later receives a trim, a dash phase or
/// its own clip is *promoted* onto the capture, keeping the same geometry. So the conflict
/// between a shape's clip and the sink's own costs a promotion, never a wrong render.
pub(crate) fn route(stroke: Option<StrokeStyle>, draws_on: bool, clip_taken: bool) -> Route {
    if stroke.is_some() || draws_on || clip_taken {
        Route::Capture
    } else {
        Route::Clip
    }
}

/// Everything realizing a sprite reaches for, and nothing else.
///
/// Named once so the functions below thread one borrow set, and so what they touch is a
/// fact the compiler checks. The node arrives separately and mutably: a realize writes one.
///
/// Five fields, each one fact: what to make things with, what display they are for, what
/// has been invalidated since, and the two stores a chain is assembled from.
pub(crate) struct Realizer<'a> {
    pub(crate) back: &'a Backends,
    pub(crate) env: Env,
    pub(crate) generation: Gen,
    pub(crate) res: &'a Resources,
    pub(crate) cells: &'a mut Cells,
}

impl Realizer<'_> {
    /// Builds or rebuilds a sprite's brush chain from its declaration.
    ///
    /// **Everything needed is on the node already**, which makes device-loss recovery, a
    /// DPI change and a first bind the same call: every brush is a pure function of a cache
    /// key or a resource id, and the stroke pattern is held, not re-read from a patch that
    /// may not exist.
    ///
    /// `glow` is the captured group's visual, resolved by the caller because it belongs to
    /// a *different* node.
    pub(crate) fn sprite(&mut self, node: &mut Node, glow: Option<&Visual>) -> Result<()> {
        let Some((mask, paint, dashes, owned_clip)) = node
            .painted
            .as_ref()
            .map(|p| (p.mask, p.paint, p.dashes, p.owns_the_clip()))
        else {
            return Ok(());
        };

        let paint_brush = self.paint(node, &paint, glow)?;
        let (mask_brush, route) = self.mask(node, &mask, dashes.as_slice())?;

        // A shape leaving the clip route takes its clip with it, or it is masked *twice* by
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
            // No mask brush at all: the paint binds directly. Required for a presented
            // buffer, since a mask brush in the chain disqualifies it from a display plane —
            // and it is also what the clip route is.
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
            dashes,
            route,
            built_at: self.generation,
        });
        Ok(())
    }

    /// The alpha half.
    fn mask(
        &mut self,
        node: &mut Node,
        mask: &Mask,
        dashes: &[f32],
    ) -> Result<(Option<CompositionBrush>, Route)> {
        match *mask {
            // No mask: the paint's own alpha is the shape.
            Mask::None => Ok((None, Route::Clip)),

            Mask::Box { radius } => {
                let key = BoxKey::new(radius, self.env.scale());
                let inset = key.inset_px() / self.env.scale();
                let Some(cell) =
                    self.cells
                        .boxes
                        .brush(self.back, self.env, self.generation, &key)?
                else {
                    return Ok((None, Route::Clip));
                };
                // Nine-slice, so one raster serves any width and height with pristine
                // corners. It reaches the mask slot as the base brush type, which is what
                // that slot actually accepts.
                let nine = self.back.compositor.create_nine_grid_brush();
                nine.set_source(cell);
                nine.set_insets(inset, inset, inset, inset);
                Ok((Some(nine.as_brush()), Route::Clip))
            }

            Mask::Run(run) => Ok((self.res.runs.value(run).map(Brush::as_brush), Route::Clip)),

            Mask::Shape { geom, stroke } => {
                let clip_taken = node.clip.is_some();
                match route(stroke, draws_on(node), clip_taken) {
                    Route::Clip => {
                        self.geometric_clip(node, geom);
                        Ok((None, Route::Clip))
                    }
                    Route::Capture => Ok((
                        self.shape_capture(node, geom, stroke, dashes)
                            .map(|b| b.as_brush()),
                        Route::Capture,
                    )),
                }
            }
        }
    }

    /// The colour half. Always a surface brush.
    fn paint(
        &mut self,
        node: &mut Node,
        paint: &Paint,
        glow: Option<&Visual>,
    ) -> Result<Option<CompositionSurfaceBrush>> {
        match *paint {
            Paint::Solid(light) => {
                // **The retained path's draw choke**, and the only place in this crate a
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

    /// The cheap route: no mask brush, the paint bound directly, and a geometric clip
    /// carrying the shape with a soft border for an antialiased edge.
    fn geometric_clip(&mut self, node: &mut Node, geom: GeomId) {
        let Some(geometry) = self.res.geoms.value(geom) else {
            return;
        };
        let clip = self.back.compositor.create_geometric_clip(geometry);
        node.visual.set_clip(Some(&clip));
        node.visual.set_border_mode(BorderMode::Soft);
    }

    /// The general route: an off-tree shape visual captured through a visual surface.
    ///
    /// It exists because a sprite shape's fill and stroke brushes do not accept a surface
    /// brush, so an FP16 colour cannot reach a shape directly — which is the whole reason
    /// the expensive route is the general one, and why a shape carries only alpha.
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
        host.set_size(size.x, size.y);
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
        // **The scale goes on the shape, not on the visual.** A visual surface captures
        // *content* and ignores the source visual's own transform, so scaling the host would
        // change nothing about what lands in the surface.
        shape.set_scale(Vector2 { x: scale, y: scale });
        host.shapes().append(&shape);

        let brush = self
            .back
            .compositor
            .capture(&crate::base_of_shape(&host), size, scale);

        // A promotion keeps whatever the channels had reached, so a shape that acquires a
        // trim mid-animation does not restart from the identity. Fresh, the window is the
        // whole path and the stroke is one DIP with no dash phase.
        let (trim, stroke) = node
            .shape
            .as_ref()
            .map_or(([0.0, 1.0], [1.0, 0.0]), |s| (s.trim, s.stroke));
        node.shape = Some(ShapeState {
            host,
            shape,
            geometry,
            trim,
            stroke,
        });
        Some(brush)
    }

    /// The glow: capture a subtree, blur it, tint it, and cast it behind the sprite.
    ///
    /// The halo under a curve stroke *is* a capture of that stroke, which is why the blur
    /// and tint live on the paint variant — without them the construction has nowhere to
    /// state its parameters.
    ///
    /// The shadow is cast by **the sprite this paint belongs to**, not the subtree it
    /// captures: that puts the blurred silhouette under the real stroke, and it is why the
    /// state lands on `node`. The glow's channels are addressed to the sprite, so a shadow
    /// parked anywhere else could never take a write.
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

        let shadow = self.back.compositor.create_drop_shadow();
        shadow.set_blur_radius(blur);
        shadow.set_mask(&captured);
        // At zero offset a shadow *is* a glow. Its colour is eight-bit, and that is the
        // right precision for it — a shadow is darkness rather than light and never needs to
        // exceed the display's white — so the authored tint goes through the display
        // transform first and agrees with everything beside it.
        shadow.set_offset(0.0, 0.0, 0.0);
        shadow.set_color(color_of(self.env.apply(tint)));
        node.sprite.as_ref()?.set_shadow(&shadow);

        let chans = node.shadow.as_ref().map_or([blur, 1.0], |s| s.chans);
        node.shadow = Some(ShadowState { shadow, chans });
        Some(captured)
    }
}

/// Whether anything is driving a channel only the capture route can carry.
///
/// Trim and the dash phase live on a sprite shape, so a clip cannot express them. A channel
/// driven once will be driven again, so a stopped animation counts as much as a running
/// one — otherwise a control demotes to the clip route between two hovers.
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

/// Eight-bit, for the one place the platform offers nothing better.
fn color_of(c: Scrgb) -> Color {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    Color::rgba(byte(c.r), byte(c.g), byte(c.b), byte(c.a))
}

impl crate::Scene {
    /// Realizes a sprite, resolving the one thing that lives on another node first.
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
