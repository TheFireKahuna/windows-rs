//! The environment a scene renders into. **Both halves.**
//!
//! Two facts belonging to the window and its monitor: how many pixels a DIP is, and how
//! authored light reaches the display. Neither is stored on the scene. Both are stated at
//! every operation that depends on them, so [`Model::flush`](crate::Model::flush) and
//! [`Scene::apply`](crate::Scene::apply) take the same value and the two halves cannot
//! disagree about the scale. There is no `set_dpi`: a cached environment can go stale
//! without saying so, and geometry snapped to one pixel grid with rasters built for another
//! is soft text and hairline seams that nothing reports.
//!
//! The fields are private and [`scale`](Env::scale) is the only derivation, so the DIP-to-
//! pixel factor a layout snapped against and the one a cache keyed on are the same number by
//! construction rather than by two call sites agreeing.

use crate::quant::snap_scale;
use windows_color::{OutputTransform, Radiance, Scrgb};

/// The DPI and the output transform every operation on a scene is stated against.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Env {
    dpi: f32,
    output: OutputTransform,
}

impl Env {
    /// Returns the environment at `dpi`, presenting through `output`.
    #[must_use]
    pub const fn new(dpi: f32, output: OutputTransform) -> Self {
        Self { dpi, output }
    }

    /// Returns the display's DPI, raw. A draw bracket sets it on the device context;
    /// everything else takes [`scale`](Env::scale).
    #[must_use]
    pub const fn dpi(self) -> f32 {
        self.dpi
    }

    /// Returns the DIP-to-pixel factor, canonicalized so float noise cannot fork a cache.
    #[must_use]
    pub fn scale(self) -> f32 {
        snap_scale(self.dpi / 96.0)
    }

    /// Converts authored light to display-referred output. The only such conversion, and it
    /// has no inverse, so the transform runs exactly once per colour.
    #[must_use]
    pub fn apply(self, light: Radiance) -> Scrgb {
        self.output.apply(light)
    }

    /// Returns whether a change from `self` to `next` invalidates rasterized geometry.
    ///
    /// Every snapped dimension is a function of the scale, so geometry and coverage
    /// re-rasterize and colour is untouched.
    pub(crate) fn geometry_moved(self, next: Self) -> bool {
        self.scale() != next.scale()
    }

    /// Returns whether a change from `self` to `next` invalidates rasterized colour.
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
        // The guard is the snapped scale and not the raw DPI: a hundredth of a DPI is not a
        // different pixel grid, and re-rasterizing every cell for one would put a monitor
        // query's rounding in charge of the cache.
        let before = env(96.0);
        let after = env(96.02);
        assert!(!before.geometry_moved(after));
        assert!(!before.light_moved(after));
    }
}
