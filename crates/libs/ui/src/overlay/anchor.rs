//! Where an overlay lands: flip, then slide, then clamp.
//!
//! Anchor resolution runs **after the solve**, because it needs both the anchor's rect and
//! the overlay's own measured size. What it produces is one offset, and that offset is an
//! input to the *next* solve rather than a transform applied over one — so a detached
//! subtree's rects stay absolute and the one hit array needs nothing said twice.
//!
//! **Placement never changes the overlay's layout.** Flipping moves the resolved offset and
//! nothing else; an overlay's size does not depend on where it landed, or the two would be a
//! cycle with no fixed point. That is also what makes the placement pass terminate: the
//! second flush computes the same offset from the same size and stops.
//!
//! Everything here is pure. No model, no scene, no device — the whole file unit-tests
//! headless, which is what a geometry rule this fiddly needs.

use windows_numerics::Vector2;
use windows_scene::{ControlId, Rect};

/// What an overlay is positioned against.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum AnchorTo {
    /// A control's rect, read from the one hit array. A menu under its button.
    Control(ControlId),
    /// A raw pointer position. **A context menu's origin is a discrete decision**, so it is
    /// the point the press was at and not wherever the pointer has since moved to.
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
    /// No side: centred on both axes. What a modal dialog wants, and the one placement
    /// `Side` plus [`Align`] cannot otherwise express — an alignment runs *along* a chosen
    /// side, and a centred dialog has not chosen one.
    Center,
}

impl Side {
    /// The side to try when this one does not fit.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Bottom => Self::Top,
            Self::Top => Self::Bottom,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            // Nothing to flip to. A centred overlay that does not fit is clamped, which is
            // the same answer flipping would have reached.
            Self::Center => Self::Center,
        }
    }

    /// Whether the overlay stacks vertically against its anchor, and therefore whether the
    /// cross axis it slides along is x.
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
    Center,
    End,
}

/// How an overlay survives not fitting.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Fit {
    /// Try `side`; if it does not fit, try its opposite; then slide along the cross axis;
    /// then clamp to the window. The default, and the order matters — sliding first would
    /// cover the anchor a flip would have cleared.
    #[default]
    FlipSlideClamp,
    /// Never move. For an anchor guaranteed to have room, where a flip would be surprising.
    Fixed,
}

/// A complete placement rule.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Anchor {
    pub to: AnchorTo,
    pub side: Side,
    pub align: Align,
    /// DIPs, applied after side and align. The gap between a menu and its button.
    pub offset: Vector2,
    pub fit: Fit,
}

impl Anchor {
    /// Under a control, leading edges aligned. What a menu, a picker and a select want.
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

    /// At a point, which is what a context menu and a tooltip are placed by.
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

    /// Centred in the window. What a modal dialog wants.
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

    /// Docked to one of the window's own edges, **inside** it. A drawer or a sheet.
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

    #[must_use]
    pub const fn side(self, side: Side) -> Self {
        Self { side, ..self }
    }

    #[must_use]
    pub const fn align(self, align: Align) -> Self {
        Self { align, ..self }
    }

    /// The gap between the overlay and its anchor, in DIPs.
    ///
    /// The one raw length in this module, and it is a *position* rather than a design
    /// metric: it is measured against the anchor's own box, which the palette does not own.
    #[must_use]
    pub const fn gap(self, x: f32, y: f32) -> Self {
        Self {
            offset: Vector2 { x, y },
            ..self
        }
    }

    #[must_use]
    pub const fn fit(self, fit: Fit) -> Self {
        Self { fit, ..self }
    }
}

/// Places `size` against the rect `against` inside `window`, in absolute window DIPs.
///
/// A `Window` anchor is placed against the window box itself, which is what makes "centre a
/// modal" and "dock a drawer to the right edge" the same rule rather than a special case.
#[must_use]
pub fn place(size: Vector2, against: Rect, anchor: Anchor, window: Vector2) -> Vector2 {
    let how = anchor.to.seat();
    let mut at = seat(size, against, anchor.side, anchor.align, how);
    at.x += anchor.offset.x;
    at.y += anchor.offset.y;

    if anchor.fit == Fit::Fixed {
        return at;
    }

    // 1. Flip. Only if the opposite side has room the chosen one does not — flipping into
    //    an equally bad fit trades one overhang for another and moves the overlay for
    //    nothing.
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

    // 2. Slide along the cross axis, and 3. clamp on the main one. Both are the same
    //    operation — pull the box back inside the window — and differ only in which axis
    //    the flip already had its chance at.
    Vector2 {
        x: clamp_axis(at.x, size.x, window.x),
        y: clamp_axis(at.y, size.y, window.y),
    }
}

/// Which way an anchor is occupied.
///
/// The distinction the [`Window`](AnchorTo::Window) case turns on, and getting it wrong puts
/// a modal one window-height below the window: an overlay sits **beside** a control and
/// **within** the window, and the same `Side` means opposite offsets in the two.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Seat {
    /// Outside the anchor, touching the named side. A menu under its button.
    Beside,
    /// Inside it, docked to the named side. A drawer on the window's right edge.
    Within,
}

impl AnchorTo {
    const fn seat(self) -> Seat {
        match self {
            Self::Control(_) | Self::Point(_) => Seat::Beside,
            Self::Window => Seat::Within,
        }
    }
}

/// Where the overlay sits before anything is done about fit.
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
        // No side at all: centred on both axes, which is the one placement a modal wants
        // and the one `Side` plus `Align` cannot otherwise say — `Align` runs along the
        // chosen side, and a centred dialog has not chosen one.
        Side::Center => Vector2 {
            x: middle(anchor.x0, anchor.width(), size.x),
            y: middle(anchor.y0, anchor.height(), size.y),
        },
    }
}

/// Whether the overlay runs off the window on the side it was seated against.
///
/// The **main axis only**: the cross axis is what sliding fixes, and treating a cross-axis
/// overhang as a reason to flip would send a menu above its button because it was too wide.
fn overhangs(at: Vector2, size: Vector2, window: Vector2, side: Side) -> bool {
    match side {
        Side::Bottom => at.y + size.y > window.y,
        Side::Top => at.y < 0.0,
        Side::Right => at.x + size.x > window.x,
        Side::Left => at.x < 0.0,
        // Nothing to flip to, and nothing it could hang off that the clamp does not answer.
        Side::Center => false,
    }
}

/// Pulls one axis back inside the window.
///
/// Leading edge wins when the overlay is larger than the window: a menu taller than the
/// screen shows its first items, which are the ones it was opened for. The alternative
/// scrolls its top off and looks like a rendering fault.
fn clamp_axis(at: f32, size: f32, window: f32) -> f32 {
    at.min(window - size).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_scene::Ids;

    /// The `n`th id a fresh authority mints.
    ///
    /// A `ControlId` is a generational index with no public constructor, which is the point:
    /// it can only come from an [`Ids`]. Minting densely from a fresh one is deterministic,
    /// so this is stable across calls and distinct per `n` without any shared state.
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
        // Four DIPs *below* the button, and four DIPs *above* it once it flips — not four
        // further down, which would open a gap on one side and overlap on the other.
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
        // Taller than the window: neither side fits, so flipping would move it for nothing
        // and then clamp to the same place anyway. Staying put keeps the anchor's own end
        // of the overlay where the user is looking.
        let anchor = rect(40.0, 140.0, 140.0, 164.0);
        let at = place(size(120.0, 400.0), anchor, Anchor::below(cid(1)), WINDOW);
        assert_eq!(at.y, 0.0, "clamped, not flipped");
    }

    #[test]
    fn a_cross_axis_overhang_slides_rather_than_flipping() {
        // Too wide for where it was seated, but there is nothing wrong with being *below*.
        // Flipping here would send a menu above its button because it was wide, which reads
        // as a bug.
        let anchor = rect(340.0, 50.0, 380.0, 74.0);
        let at = place(size(120.0, 80.0), anchor, Anchor::below(cid(1)), WINDOW);
        assert_eq!(at.y, 74.0, "still below its anchor");
        assert_eq!(at.x, 280.0, "slid back inside the window");
    }

    #[test]
    fn nothing_ever_leaves_the_window() {
        // Invariant 3, over every side, alignment and a deliberately awkward anchor.
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
        // The whole point of the mode: an anchor guaranteed to have room, where a flip
        // would be more surprising than an overhang.
        let anchor = rect(40.0, 260.0, 140.0, 284.0);
        let a = Anchor::below(cid(1)).fit(Fit::Fixed);
        let at = place(size(120.0, 80.0), anchor, a, WINDOW);
        assert_eq!(at.y, 284.0);
    }

    #[test]
    fn a_window_anchor_seats_inside_the_window_and_not_beside_it() {
        // The case that reads as an ordinary side and is the opposite of one. A control is
        // something an overlay sits *next to*; the window is something it sits *in*, so the
        // same `Side::Bottom` means "below the button" against one and "along the bottom
        // edge" against the other. Seating a modal the first way puts it one window-height
        // off the screen, and `Fit::Fixed` means nothing clamps it back.
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

        // And every one of them is on screen, which is the claim that actually matters.
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
        // What `Side` plus `Align` cannot say: an alignment runs *along* a chosen side, so
        // "centred" needs a side that is not one.
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
        // What makes the placement pass terminate: the offset is a function of the size,
        // and the size does not depend on the offset. If this ever fails, the flush loop
        // has become a fixed-point search rather than two passes.
        let anchor = rect(340.0, 250.0, 380.0, 274.0);
        let a = Anchor::below(cid(1)).gap(0.0, 4.0);
        let overlay = size(120.0, 80.0);
        let first = place(overlay, anchor, a, WINDOW);
        assert_eq!(first, place(overlay, anchor, a, WINDOW));
    }
}
