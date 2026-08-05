//! Where an overlay lands: flip, then slide, then clamp.
//!
//! Anchor resolution runs after the solve, because it needs both the anchor's rect and the
//! overlay's own measured size. It produces one offset, and that offset is an input to the
//! next solve rather than a transform applied over one, so a detached subtree's rects stay
//! absolute in window space.
//!
//! Placement never changes the overlay's layout. Flipping moves the resolved offset and
//! nothing else, and an overlay's size does not depend on where it landed. That is what makes
//! the placement pass terminate: the second flush computes the same offset from the same size
//! and stops.
//!
//! Every function here is pure — no model, no scene, no device.

use windows_numerics::Vector2;
use windows_scene::{ControlId, Rect};

/// What an overlay is positioned against.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum AnchorTo {
    /// A control's rect, read from the one hit array. A menu under its button.
    Control(ControlId),
    /// A raw pointer position: the point a press was at, not wherever the pointer has since
    /// moved to.
    Point(Vector2),
    /// The window itself. What a modal centres against.
    Window,
}

/// Which side of the anchor the overlay sits on.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Side {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
    /// No side: centred on both axes. [`Align`] runs along a chosen side, so centring on
    /// both needs its own variant.
    Center,
}

impl Side {
    /// Returns the side to try when this one does not fit.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Bottom => Self::Top,
            Self::Top => Self::Bottom,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            // Nothing to flip to; a centred overlay that does not fit is clamped instead.
            Self::Center => Self::Center,
        }
    }

    /// Returns whether the overlay stacks vertically against its anchor, so the cross axis
    /// it slides along is x.
    #[must_use]
    pub const fn is_vertical(self) -> bool {
        matches!(self, Self::Bottom | Self::Top)
    }
}

/// Where along the chosen side the overlay lines up.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Align {
    /// Leading edges together — a menu's left edge under its button's left edge.
    #[default]
    Start,
    /// Midpoints together.
    Center,
    /// Trailing edges together.
    End,
}

/// How an overlay survives not fitting.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Fit {
    /// Try `side`; if it does not fit, try its opposite; then slide along the cross axis;
    /// then clamp to the window. Flipping runs before sliding, so an overlay clears its
    /// anchor rather than sliding across it.
    #[default]
    FlipSlideClamp,
    /// Never move, whatever the overlay overhangs.
    Fixed,
}

/// A complete placement rule.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Anchor {
    /// What the overlay is positioned against.
    pub to: AnchorTo,
    /// Which side of the anchor it seats on.
    pub side: Side,
    /// Where along that side it lines up.
    pub align: Align,
    /// DIPs, applied after side and align. The gap between a menu and its button.
    pub offset: Vector2,
    /// How it survives not fitting.
    pub fit: Fit,
}

impl Anchor {
    /// Returns a rule seating the overlay under `to`, leading edges aligned.
    #[must_use]
    pub const fn below(to: ControlId) -> Self {
        Self {
            to: AnchorTo::Control(to),
            side: Side::Bottom,
            align: Align::Start,
            offset: Vector2 { x: 0.0, y: 0.0 },
            fit: Fit::FlipSlideClamp,
        }
    }

    /// Returns a rule seating the overlay against `point`, as a context menu is placed.
    #[must_use]
    pub const fn at(point: Vector2) -> Self {
        Self {
            to: AnchorTo::Point(point),
            side: Side::Bottom,
            align: Align::Start,
            offset: Vector2 { x: 0.0, y: 0.0 },
            fit: Fit::FlipSlideClamp,
        }
    }

    /// Returns a rule centring the overlay in the window, with no fit adjustment.
    #[must_use]
    pub const fn centered() -> Self {
        Self {
            to: AnchorTo::Window,
            side: Side::Center,
            align: Align::Center,
            offset: Vector2 { x: 0.0, y: 0.0 },
            fit: Fit::Fixed,
        }
    }

    /// Returns a rule docking the overlay inside the window against `side`, as a drawer or a
    /// sheet is placed.
    #[must_use]
    pub const fn window(side: Side) -> Self {
        Self {
            to: AnchorTo::Window,
            side,
            align: Align::Center,
            offset: Vector2 { x: 0.0, y: 0.0 },
            fit: Fit::Fixed,
        }
    }

    /// Returns this rule seating the overlay on `side` of its anchor.
    #[must_use]
    pub const fn side(self, side: Side) -> Self {
        Self { side, ..self }
    }

    /// Returns this rule lining the overlay up by `align` along the chosen side.
    #[must_use]
    pub const fn align(self, align: Align) -> Self {
        Self { align, ..self }
    }

    /// Returns this rule with a gap of `x` by `y` DIPs between the overlay and its anchor.
    ///
    /// A raw length rather than a palette metric, because it is measured against the
    /// anchor's own box. [`place`] reverses it when the overlay flips.
    #[must_use]
    pub const fn gap(self, x: f32, y: f32) -> Self {
        Self {
            offset: Vector2 { x, y },
            ..self
        }
    }

    /// Returns this rule with `fit` deciding what happens when the overlay does not fit.
    #[must_use]
    pub const fn fit(self, fit: Fit) -> Self {
        Self { fit, ..self }
    }
}

/// Returns the absolute window-DIP origin for an overlay of `size` placed against the rect
/// `against` inside a client box of `window`.
///
/// `against` is the anchor's own rect, which for [`AnchorTo::Window`] is the window box, so
/// centring a modal and docking a drawer to an edge run through this one rule.
#[must_use]
pub fn place(size: Vector2, against: Rect, anchor: Anchor, window: Vector2) -> Vector2 {
    let how = anchor.to.seat();
    let mut at = seat(size, against, anchor.side, anchor.align, how);
    at.x += anchor.offset.x;
    at.y += anchor.offset.y;

    if anchor.fit == Fit::Fixed {
        return at;
    }

    // 1. Flip, only where the opposite side has room the chosen one does not. Flipping into
    //    an equally bad fit would move the overlay and still overhang.
    if overhangs(at, size, window, anchor.side) {
        let flipped = {
            let mut flipped = seat(size, against, anchor.side.opposite(), anchor.align, how);
            // The offset is a gap from the anchor, so it reverses with the side. A menu 4
            // DIPs below its button is 4 DIPs above it when flipped, not 4 further down.
            flipped.x -= anchor.offset.x;
            flipped.y -= anchor.offset.y;
            flipped
        };
        if !overhangs(flipped, size, window, anchor.side.opposite()) {
            at = flipped;
        }
    }

    // 2. Slide along the cross axis, and 3. clamp on the main one. Both pull the box back
    //    inside the window, and differ only in which axis the flip already had a chance at.
    Vector2 {
        x: clamp_axis(at.x, size.x, window.x),
        y: clamp_axis(at.y, size.y, window.y),
    }
}

/// Whether an overlay seats outside its anchor or inside it.
///
/// An overlay sits beside a control and within the window, so the same [`Side`] resolves to
/// opposite offsets in the two: seating a modal beside the window box would put it one
/// window-height off screen.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Seat {
    /// Outside the anchor, touching the named side. A menu under its button.
    Beside,
    /// Inside it, docked to the named side. A drawer on the window's right edge.
    Within,
}

impl AnchorTo {
    /// Returns how an overlay seats against this anchor.
    const fn seat(self) -> Seat {
        match self {
            Self::Control(_) | Self::Point(_) => Seat::Beside,
            Self::Window => Seat::Within,
        }
    }
}

/// Returns where the overlay sits before anything is done about fit.
fn seat(size: Vector2, anchor: Rect, side: Side, align: Align, how: Seat) -> Vector2 {
    let along = |start: f32, extent: f32, own: f32| match align {
        Align::Start => start,
        Align::Center => start + (extent - own) * 0.5,
        Align::End => start + extent - own,
    };
    let middle = |start: f32, extent: f32, own: f32| start + (extent - own) * 0.5;
    let (top, bottom, left, right) = match how {
        Seat::Beside => (anchor.y0 - size.y, anchor.y1, anchor.x0 - size.x, anchor.x1),
        Seat::Within => (anchor.y0, anchor.y1 - size.y, anchor.x0, anchor.x1 - size.x),
    };
    match side {
        Side::Bottom => Vector2 {
            x: along(anchor.x0, anchor.width(), size.x),
            y: bottom,
        },
        Side::Top => Vector2 {
            x: along(anchor.x0, anchor.width(), size.x),
            y: top,
        },
        Side::Right => Vector2 {
            x: right,
            y: along(anchor.y0, anchor.height(), size.y),
        },
        Side::Left => Vector2 {
            x: left,
            y: along(anchor.y0, anchor.height(), size.y),
        },
        // Centred on both axes: `Align` runs along a chosen side, and this has none.
        Side::Center => Vector2 {
            x: middle(anchor.x0, anchor.width(), size.x),
            y: middle(anchor.y0, anchor.height(), size.y),
        },
    }
}

/// Returns whether the overlay runs off the window on the side it was seated against.
///
/// Tests the main axis only. Sliding fixes the cross axis, so a cross-axis overhang is not a
/// reason to flip a menu above the button it was merely too wide for.
fn overhangs(at: Vector2, size: Vector2, window: Vector2, side: Side) -> bool {
    match side {
        Side::Bottom => at.y + size.y > window.y,
        Side::Top => at.y < 0.0,
        Side::Right => at.x + size.x > window.x,
        Side::Left => at.x < 0.0,
        // Nothing to flip to; the clamp answers any overhang.
        Side::Center => false,
    }
}

/// Returns `at` pulled back inside the window on one axis.
///
/// The leading edge wins where the overlay is larger than the window, so a menu taller than
/// the client box keeps its first items on screen.
fn clamp_axis(at: f32, size: f32, window: f32) -> f32 {
    at.min(window - size).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_scene::Ids;

    /// Returns the `n`th id a fresh [`Ids`] mints.
    ///
    /// A `ControlId` is a generational index with no public constructor, so it can only come
    /// from an `Ids`. Minting densely from a fresh one is deterministic, so the result is
    /// stable across calls and distinct per `n` without any shared state.
    fn cid(n: u32) -> ControlId {
        let mut ids = Ids::<windows_scene::Control>::new();
        let mut id = ids.mint();
        for _ in 1..n {
            id = ids.mint();
        }
        id
    }

    const WINDOW: Vector2 = Vector2 { x: 400.0, y: 300.0 };

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::new(x0, y0, x1, y1)
    }

    fn size(x: f32, y: f32) -> Vector2 {
        Vector2 { x, y }
    }

    #[test]
    fn a_menu_seats_under_its_button_with_leading_edges_aligned() {
        let anchor = rect(40.0, 50.0, 140.0, 74.0);
        let at = place(size(120.0, 80.0), anchor, Anchor::below(cid(1)), WINDOW);
        assert_eq!(at, Vector2 { x: 40.0, y: 74.0 });
    }

    #[test]
    fn alignment_runs_along_the_chosen_side() {
        let anchor = rect(40.0, 50.0, 140.0, 74.0);
        let a = Anchor::below(cid(1));
        let overlay = size(40.0, 20.0);
        assert_eq!(
            place(overlay, anchor, a.align(Align::Start), WINDOW).x,
            40.0
        );
        assert_eq!(
            place(overlay, anchor, a.align(Align::Center), WINDOW).x,
            70.0
        );
        assert_eq!(place(overlay, anchor, a.align(Align::End), WINDOW).x, 100.0);
    }

    #[test]
    fn a_gap_reverses_with_the_side_it_is_measured_from() {
        // Four DIPs below the button, and four DIPs above it once it flips, rather than four
        // further down.
        let anchor = rect(40.0, 260.0, 140.0, 284.0);
        let a = Anchor::below(cid(1)).gap(0.0, 4.0);
        let at = place(size(120.0, 80.0), anchor, a, WINDOW);
        assert_eq!(at.y, 260.0 - 80.0 - 4.0, "flipped, with the gap reversed");
    }

    #[test]
    fn an_overlay_with_no_room_below_flips_above() {
        let anchor = rect(40.0, 250.0, 140.0, 274.0);
        let at = place(size(120.0, 80.0), anchor, Anchor::below(cid(1)), WINDOW);
        assert_eq!(at.y, 170.0, "seated above the anchor rather than below it");
    }

    #[test]
    fn a_flip_into_an_equally_bad_fit_does_not_happen() {
        // Taller than the window: neither side fits, so flipping would move it and clamp to
        // the same place.
        let anchor = rect(40.0, 140.0, 140.0, 164.0);
        let at = place(size(120.0, 400.0), anchor, Anchor::below(cid(1)), WINDOW);
        assert_eq!(at.y, 0.0, "clamped, not flipped");
    }

    #[test]
    fn a_cross_axis_overhang_slides_rather_than_flipping() {
        // Too wide for where it was seated, and still correctly below its anchor: a
        // cross-axis overhang slides rather than flipping.
        let anchor = rect(340.0, 50.0, 380.0, 74.0);
        let at = place(size(120.0, 80.0), anchor, Anchor::below(cid(1)), WINDOW);
        assert_eq!(at.y, 74.0, "still below its anchor");
        assert_eq!(at.x, 280.0, "slid back inside the window");
    }

    #[test]
    fn nothing_ever_leaves_the_window() {
        // Every side and alignment, against an anchor straddling two window edges.
        let anchor = rect(-20.0, 290.0, 30.0, 340.0);
        for side in [Side::Bottom, Side::Top, Side::Left, Side::Right] {
            for align in [Align::Start, Align::Center, Align::End] {
                let a = Anchor::below(cid(1)).side(side).align(align);
                let overlay = size(180.0, 120.0);
                let at = place(overlay, anchor, a, WINDOW);
                assert!(at.x >= 0.0 && at.y >= 0.0, "{side:?}/{align:?}: {at:?}");
                assert!(
                    at.x + overlay.x <= WINDOW.x && at.y + overlay.y <= WINDOW.y,
                    "{side:?}/{align:?}: {at:?}"
                );
            }
        }
    }

    #[test]
    fn an_overlay_larger_than_the_window_keeps_its_leading_edge() {
        let anchor = rect(40.0, 50.0, 140.0, 74.0);
        let at = place(size(900.0, 900.0), anchor, Anchor::below(cid(1)), WINDOW);
        assert_eq!(at, Vector2 { x: 0.0, y: 0.0 });
    }

    #[test]
    fn fixed_never_moves() {
        // `Fit::Fixed` keeps the seated position even where the overlay overhangs.
        let anchor = rect(40.0, 260.0, 140.0, 284.0);
        let a = Anchor::below(cid(1)).fit(Fit::Fixed);
        let at = place(size(120.0, 80.0), anchor, a, WINDOW);
        assert_eq!(at.y, 284.0);
    }

    #[test]
    fn a_window_anchor_seats_inside_the_window_and_not_beside_it() {
        // A control is something an overlay sits next to; the window is something it sits
        // in, so the same `Side::Bottom` means "below the button" against one and "along the
        // bottom edge" against the other. Seating a modal the first way puts it one
        // window-height off screen, and `Fit::Fixed` clamps nothing back.
        let window = Rect::new(0.0, 0.0, WINDOW.x, WINDOW.y);
        let overlay = size(200.0, 100.0);

        let bottom = place(overlay, window, Anchor::window(Side::Bottom), WINDOW);
        assert_eq!(bottom.y, WINDOW.y - overlay.y, "docked to the bottom edge");
        let right = place(overlay, window, Anchor::window(Side::Right), WINDOW);
        assert_eq!(right.x, WINDOW.x - overlay.x, "docked to the right edge");
        let top = place(overlay, window, Anchor::window(Side::Top), WINDOW);
        assert_eq!(top.y, 0.0);
        let left = place(overlay, window, Anchor::window(Side::Left), WINDOW);
        assert_eq!(left.x, 0.0);

        // And every one of them is on screen.
        for at in [bottom, right, top, left] {
            assert!(at.x >= 0.0 && at.y >= 0.0, "{at:?}");
            assert!(
                at.x + overlay.x <= WINDOW.x && at.y + overlay.y <= WINDOW.y,
                "{at:?}"
            );
        }
    }

    #[test]
    fn a_modal_is_centred_on_both_axes() {
        // `Align` runs along a chosen side, so centring on both axes needs `Side::Center`.
        let window = Rect::new(0.0, 0.0, WINDOW.x, WINDOW.y);
        let overlay = size(200.0, 100.0);
        let at = place(overlay, window, Anchor::centered(), WINDOW);
        assert_eq!(
            at,
            Vector2 {
                x: (WINDOW.x - overlay.x) * 0.5,
                y: (WINDOW.y - overlay.y) * 0.5,
            }
        );
    }

    #[test]
    fn placing_twice_lands_in_the_same_place() {
        // The placement pass terminates because the offset is a function of the size and the
        // size does not depend on the offset.
        let anchor = rect(340.0, 250.0, 380.0, 274.0);
        let a = Anchor::below(cid(1)).gap(0.0, 4.0);
        let overlay = size(120.0, 80.0);
        let first = place(overlay, anchor, a, WINDOW);
        assert_eq!(first, place(overlay, anchor, a, WINDOW));
    }
}
