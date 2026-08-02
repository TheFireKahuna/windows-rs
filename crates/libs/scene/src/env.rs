//! The environment a scene renders into. **Both halves.**
//!
//! Two facts — how many pixels a DIP is, and how authored light reaches the display — and
//! neither is the scene's. They belong to the window and to its monitor, and they are
//! **stated at every operation that depends on them** rather than pushed into the scene and
//! cached there.
//!
//! That is the whole of why there is no `set_dpi`. A cached environment can be forgotten,
//! and forgetting it is silent: geometry snapped to one pixel grid with rasters built for
//! another is soft text and hairline seams, with nothing to report it. Passing it at use
//! makes forgetting unrepresentable, and it means the two halves cannot disagree about the
//! scale — [`Model::flush`](crate::Model::flush) and [`Scene::apply`](crate::Scene::apply)
//! take the same value.
//!
//! The fields are private and [`scale`](Env::scale) is the only derivation, so the DIP-to-
//! pixel factor a layout snapped against and the one a cache keyed on are the same number
//! by construction rather than by two call sites agreeing.

use crate::quant::snap_scale;
use windows_color::{OutputTransform, Radiance, Scrgb};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Env {
    dpi: f32,
    output: OutputTransform,
}

impl Env {
    /// The environment at `dpi`, presenting through `output`.
    #[must_use]
    pub const fn new(dpi: f32, output: OutputTransform) -> Self {
        Self { dpi, output }
    }

    /// The display's DPI, for the one consumer that needs it raw: a draw bracket, which
    /// sets it on the device context rather than deriving a scale from it.
    #[must_use]
    pub const fn dpi(self) -> f32 {
        self.dpi
    }

    /// The DIP-to-pixel factor, canonicalized so float noise cannot fork a cache.
    #[must_use]
    pub fn scale(self) -> f32 {
        snap_scale(self.dpi / 96.0)
    }

    /// Authored light to display-referred output. **The draw choke**, and the only
    /// conversion — there is no inverse, so the transform runs exactly once per colour.
    #[must_use]
    pub fn apply(self, light: Radiance) -> Scrgb {
        self.output.apply(light)
    }

    /// Whether a change from `self` to `next` invalidates rasterized geometry.
    ///
    /// Every snapped dimension is a function of the scale, so geometry and coverage
    /// re-rasterize and colour is untouched.
    pub(crate) fn geometry_moved(self, next: Self) -> bool {
        self.scale() != next.scale()
    }

    /// Whether a change from `self` to `next` invalidates rasterized colour.
    ///
    /// The same authored light produces a different cell on a different display, so every
    /// colour cell is wrong and no coverage tile is.
    pub(crate) fn light_moved(self, next: Self) -> bool {
        self.output != next.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_color::DisplayCapability;

    fn env(dpi: f32) -> Env {
        Env::new(
            dpi,
            OutputTransform::for_display(DisplayCapability::Sdr, 203.0),
        )
    }

    #[test]
    fn the_scale_is_canonicalized_so_float_noise_cannot_fork_a_cache() {
        // 144/96 is exact, but a DPI that arrives as a rounded integer from a monitor
        // query is not obliged to be — and two populations differing in the last bit are
        // two of every cached cell.
        assert_eq!(env(144.0).scale(), 1.5);
        assert_eq!(env(120.0).scale(), 1.25);
        assert_eq!(env(96.0).scale(), 1.0);
    }

    #[test]
    fn a_dpi_move_invalidates_geometry_and_leaves_light_alone() {
        let (before, after) = (env(96.0), env(144.0));
        assert!(before.geometry_moved(after));
        assert!(!before.light_moved(after));
    }

    #[test]
    fn a_display_move_invalidates_light_and_leaves_geometry_alone() {
        let before = env(96.0);
        let after = Env::new(
            96.0,
            OutputTransform::for_display(
                DisplayCapability::HighDynamicRange {
                    gamut: windows_color::Gamut::REC2020,
                    white_nits: 203.0,
                    peak_nits: 1000.0,
                },
                600.0,
            ),
        );
        assert!(before.light_moved(after));
        assert!(!before.geometry_moved(after));
    }

    #[test]
    fn a_dpi_that_snaps_to_the_same_scale_invalidates_nothing() {
        // The guard is the *snapped* scale, not the raw DPI: a hundredth of a DPI is not
        // a different pixel grid, and re-rasterizing every cell for one would be a
        // monitor query's rounding driving the whole cache.
        let before = env(96.0);
        let after = env(96.02);
        assert!(!before.geometry_moved(after));
        assert!(!before.light_moved(after));
    }
}
