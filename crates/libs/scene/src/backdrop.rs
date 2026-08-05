//! The window's own ground. **Front half.**
//!
//! Minted inside [`Scene::new`](crate::Scene::new), before the window is shown, and not an
//! arena node: it sits under everything the model names, layout cannot reach it, and it is
//! not in the hit array. An application configures it and may move its glows; it does not
//! compose it.
//!
//! # Why it is not an ordinary element
//!
//! A content surface arrives only after its size is delivered and the request is serviced on
//! a later commit, so a backdrop authored as content cannot exist on the first composited
//! frame: the shell lands on an unpainted window and the ground appears a couple of commits
//! later. Minting it here puts it on frame one.
//!
//! A resize writes nothing here. Every layer is a [`Spread`] ramp — a strip or a 64×64 tile,
//! stretched to fill — so no layer carries the window's extent and a resize re-points no
//! surface and re-rasterizes nothing. Every box is stated the same way, as fractions of the
//! band above it ([`place`]), so the compositor re-derives all of them from the one extent
//! [`Scene::resize`](crate::Scene::resize) puts on the root.
//!
//! # The stack
//!
//! Bottom to top: the base tilt, then one blob per glow. Layers composite source-over, and
//! source-over is associative, so splitting them across sprites is the identical composite
//! one surface drawing them in sequence performs, at lower cost.

use crate::backends::Backends;
use crate::env::Env;
use crate::sink::Spread;
use windows_color::Radiance;
use windows_composition::{SpriteVisual, Stretch};
use windows_core::Result;
use windows_numerics::{Vector2, Vector3};

/// One radial layer.
#[derive(Clone, Debug, PartialEq)]
pub struct Glow {
    /// The profile, centre to edge. The last stop should be transparent, or the blob
    /// ends on a visible edge where its tile does.
    pub stops: Vec<(f32, Radiance)>,
    /// Centre, as a fraction of the window.
    pub at: Vector2,
    /// Extent, as a fraction of the window. The tile is square and this is what
    /// stretches it into an ellipse.
    pub size: Vector2,
}

/// What an application says the window's ground looks like.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BackdropSpec {
    /// The base tilt, top to bottom, covering the whole window. Empty for no base,
    /// which leaves whatever is behind the window showing through.
    pub base: Vec<(f32, Radiance)>,
    /// The radial layers, bottom to top.
    pub glows: Vec<Glow>,
}

/// One realized layer: its sprite, and where the spec puts it.
struct Layer {
    sprite: SpriteVisual,
    /// Fractions of the window. The base is the whole of it.
    at: Vector2,
    size: Vector2,
}

/// The realized stack.
pub(crate) struct Backdrop {
    spec: BackdropSpec,
    layers: Vec<Layer>,
}

impl Backdrop {
    /// Rasterizes `spec` and mints one sprite per layer, bottom first.
    ///
    /// The caller seats the sprites. `env` is taken here rather than at the first operation
    /// because the ground is painted before the window is shown and its colours are the
    /// display's.
    pub(crate) fn new(spec: BackdropSpec, back: &Backends, env: Env) -> Result<Self> {
        let mut layers = Vec::with_capacity(1 + spec.glows.len());
        if !spec.base.is_empty() {
            layers.push(Layer {
                sprite: Self::sprite(&spec.base, Spread::Vertical, back, env)?,
                at: Vector2 { x: 0.5, y: 0.5 },
                size: Vector2 { x: 1.0, y: 1.0 },
            });
        }
        for glow in &spec.glows {
            layers.push(Layer {
                sprite: Self::sprite(&glow.stops, Spread::Radial, back, env)?,
                at: glow.at,
                size: glow.size,
            });
        }
        for layer in &layers {
            place(layer);
        }
        Ok(Self { spec, layers })
    }

    /// Returns the sprites, bottom first, for the caller to seat.
    pub(crate) fn sprites(&self) -> impl Iterator<Item = &SpriteVisual> {
        self.layers.iter().map(|layer| &layer.sprite)
    }

    /// Moves one glow's centre, as a fraction of the window.
    ///
    /// `index` is into [`BackdropSpec::glows`]. An index out of range is ignored rather
    /// than panicking.
    pub(crate) fn move_glow(&mut self, index: usize, at: Vector2) {
        let Some(glow) = self.spec.glows.get_mut(index) else {
            return;
        };
        glow.at = at;
        // The base occupies slot zero whenever there is one, so a glow's layer sits
        // after it.
        let offset = usize::from(!self.spec.base.is_empty());
        let Some(layer) = self.layers.get_mut(index + offset) else {
            return;
        };
        layer.at = at;
        place(layer);
    }

    /// Re-rasterizes every layer for a display that moved.
    ///
    /// A ramp carries no snapped dimension, so only the light is rebuilt and the sprites
    /// keep their places.
    pub(crate) fn relight(&mut self, back: &Backends, env: Env) -> Result<()> {
        let base = usize::from(!self.spec.base.is_empty());
        if base == 1 {
            Self::repoint(
                &self.layers[0],
                &self.spec.base,
                Spread::Vertical,
                back,
                env,
            )?;
        }
        for (glow, layer) in self.spec.glows.iter().zip(&self.layers[base..]) {
            Self::repoint(layer, &glow.stops, Spread::Radial, back, env)?;
        }
        Ok(())
    }

    fn sprite(
        stops: &[(f32, Radiance)],
        spread: Spread,
        back: &Backends,
        env: Env,
    ) -> Result<SpriteVisual> {
        let sprite = back.compositor.create_sprite_visual();
        if let Some(surface) = back.raster_ramp(env, stops, spread)? {
            sprite.set_brush(&back.brush(&surface, Stretch::Fill));
        }
        Ok(sprite)
    }

    fn repoint(
        layer: &Layer,
        stops: &[(f32, Radiance)],
        spread: Spread,
        back: &Backends,
        env: Env,
    ) -> Result<()> {
        if let Some(surface) = back.raster_ramp(env, stops, spread)? {
            layer.sprite.set_brush(&back.brush(&surface, Stretch::Fill));
        }
        Ok(())
    }
}

/// Centres a layer's box on its fractional position, as fractions of its parent's extent.
///
/// Both the extent and the offset are pure fractions: centring `size` on `at` is
/// `window * at - window * size / 2`, and the window cancels, so the layer's corner sits at
/// `at - size/2` of the window whatever the window is. The compositor re-derives the box
/// from its parent's extent, so a resize writes no property here, for any number of glows.
fn place(layer: &Layer) {
    layer.sprite.set_relative_size_adjustment(layer.size);
    layer.sprite.set_relative_offset_adjustment(Vector3 {
        x: layer.at.x - layer.size.x / 2.0,
        y: layer.at.y - layer.size.y / 2.0,
        z: 0.0,
    });
}
