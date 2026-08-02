//! Scroll and virtualization: the two policies that sit over the tracker.
//!
//! **Scroll is tracker-delegated, always. There is no CPU-sprung scroll.** The viewport does
//! not move and clips; the content's offset and the thumb's offset are both bound to the one
//! tracker, so the thumb follows the content with **no front-thread work at all** — no
//! per-frame thumb positioning, which was a measured cost before.
//!
//! Two things live here rather than one layer down, and for the same reason: how wide a
//! thumb is and which rows are worth existing are decisions shaped by the widget that
//! consumes them, and neither is widget-agnostic. `windows-scene` supplies the tracker and
//! the binding and stops there.

use crate::build::{Any, El, IntoChildren, View};
use crate::role::Metric;
use crate::signal::{Cell, Memo};
use core::ops::Range;

/// When the thumb is visible.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Reveal {
    /// Always, taking room from the content.
    Always,
    /// Never — for a surface whose extent is obvious from what is in it.
    Never,
    /// While the content is moving, and for a moment after.
    #[default]
    OnDemand,
}

// ── thumb geometry ───────────────────────────────────────────────────────────────
//
// Raw DIPs, and deliberately so: these are the scrollbar's own dimensions rather than the
// palette's spacing scale, and a `Metric` for them would put a component's private geometry
// in the theme — which is the token-bloat smell the role layer exists to avoid.

/// How wide the thumb is.
pub const THUMB_W: f32 = 6.0;
/// How far it is inset from the right edge, and from each end of its travel.
pub const THUMB_MARGIN: f32 = 2.0;
/// Its floor, however long the content: a thumb that shrinks to nothing cannot be grabbed.
pub const THUMB_MIN_H: f32 = 24.0;

/// What a scrollbar is, at one pair of extents.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct ThumbGeom {
    /// Whether there is anything to scroll. A viewport larger than its content has no
    /// thumb and no tracker travel.
    pub overflow: bool,
    /// How far the content can move.
    pub max_scroll: f32,
    pub thumb_h: f32,
    /// How far the thumb itself travels, which is not how far the content does.
    pub travel: f32,
}

/// The scrollbar for a viewport of `viewport_h` showing `content_h` of content.
#[must_use]
pub fn thumb_geom(viewport_h: f32, content_h: f32) -> ThumbGeom {
    let max_scroll = (content_h - viewport_h).max(0.0);
    let track_h = (viewport_h - 2.0 * THUMB_MARGIN).max(0.0);
    if max_scroll <= 0.0 || track_h <= 0.0 {
        return ThumbGeom {
            overflow: false,
            max_scroll: 0.0,
            thumb_h: 0.0,
            travel: 0.0,
        };
    }
    // Proportional, then floored — and floored *after*, so a very long document still gets
    // a grabbable thumb and the travel below is corrected for it rather than overflowing.
    let ratio = (viewport_h / content_h).clamp(0.0, 1.0);
    let thumb_h = (track_h * ratio).max(THUMB_MIN_H).min(track_h);
    ThumbGeom {
        overflow: true,
        max_scroll,
        thumb_h,
        travel: track_h - thumb_h,
    }
}

/// The thumb's own box: a bar at the right edge, as tall as the scrollbar says.
///
/// Built here rather than through the [`Over`](super::Over) vocabulary for the same reason a
/// shaped line's box is: the numbers are this component's own geometry, resolved from
/// extents the solve produced, and `Len` deliberately cannot say them. It is also
/// **absolutely positioned inside a rail**, so the offset the tracker drives is the only
/// thing that moves it — a laid-out thumb would have layout and the tracker writing one
/// channel.
#[must_use]
pub fn thumb_style(geom: ThumbGeom) -> windows_scene::taffy::Style {
    use windows_scene::taffy;
    use windows_scene::taffy::style_helpers::TaffyAuto;
    taffy::Style {
        position: taffy::Position::Absolute,
        size: taffy::Size {
            width: taffy::Dimension::length(THUMB_W),
            height: taffy::Dimension::length(geom.thumb_h),
        },
        inset: taffy::Rect {
            left: taffy::LengthPercentageAuto::AUTO,
            right: taffy::LengthPercentageAuto::length(THUMB_MARGIN),
            top: taffy::LengthPercentageAuto::length(0.0),
            bottom: taffy::LengthPercentageAuto::AUTO,
        },
        ..taffy::Style::DEFAULT
    }
}

/// A scrolling container.
///
/// The children go into a content group of their own, because the viewport must not move:
/// it is the thing that clips, and an offset on it would take the clip with it.
#[must_use]
pub fn scroll(children: impl IntoChildren) -> View {
    scroll_with(Reveal::default(), children)
}

/// The same, with an explicit reveal policy.
#[must_use]
pub fn scroll_with(reveal: Reveal, children: impl IntoChildren) -> View {
    let content = super::stack(children);
    El::<Any>::viewport(reveal, content)
}

// ── virtualization ───────────────────────────────────────────────────────────────

/// What a list needs to decide which rows exist.
///
/// **Uniform extents.** With a fixed row height an offset is affine in its index and the
/// maximum position is a constant, so neither of the hard problems with variable extents
/// arises: there is no progressively corrected estimate, and therefore no maximum shifting
/// mid-fling and moving content under the user's finger.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ListSpec {
    pub count: usize,
    /// The row height, as the palette's — so a list is as dense as the user asked for.
    pub row_h: Metric,
    /// Rows realized beyond the viewport on each side. Two or three: enough that a row
    /// exists before it is looked at, few enough that a fling does not realize a screen it
    /// will never show.
    pub overscan: usize,
}

/// Which rows are worth existing at `scroll_y`.
///
/// **Always inside `0..count`, at any `scroll_y`.** A tracker's position travels outside its
/// bounds during a manipulation — the overpan is the bounce, and it is wanted — so this is
/// asked about positions past the end of the content, and a window that ran past the last row
/// would make the trailing spacer's row count negative.
#[must_use]
pub fn window(scroll_y: f32, viewport_h: f32, row_h: f32, spec: &ListSpec) -> Range<usize> {
    if row_h <= 0.0 || spec.count == 0 {
        return 0..0;
    }
    let first = (((scroll_y / row_h).floor() as isize - spec.overscan as isize).max(0) as usize)
        .min(spec.count);
    let last = (((scroll_y + viewport_h) / row_h).ceil() as usize + spec.overscan).min(spec.count);
    first..last.max(first)
}

/// Where a scroll container has got to, as values rather than as events.
///
/// The tracker reports through [`SceneEvent`](windows_scene::SceneEvent), and a reported
/// position is **the only trustworthy read of one** — its getter answers with what was last
/// set, not with what the compositor is evaluating. So the frame loop writes what it was
/// told into here, and everything above reads it as an ordinary signal: the realization
/// window is then a [`Memo`], and no part of this needs a mechanism of its own.
///
/// The two writes to make, both through [`moved`](Self::moved):
///
/// * `ValuesChanged` — realize what the current position needs.
/// * `InertiaStarting` — at the instant inertia begins the destination is **already known**,
///   so the rows it lands on can be realized while the compositor animates, which is the
///   difference between arriving at content and arriving at a blank.
#[derive(Copy, Clone, Debug)]
pub struct ScrollState {
    offset: Cell<f32>,
    viewport: Cell<f32>,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            offset: Cell::new(0.0),
            viewport: Cell::new(0.0),
        }
    }

    /// Where the content has got to, or is going to.
    ///
    /// One method for both writes, because they are one fact: the position the realization
    /// window should be computed from. A `ValuesChanged` reports where it *is*; an
    /// `InertiaStarting` reports where it **will be**, and applying that immediately is what
    /// realizes the rows a fling lands on while the compositor is still animating. Pass the
    /// *modified* destination in the second case — snap points are applied to it and not to
    /// the natural one.
    pub fn moved(self, y: f32) {
        self.offset.set(y);
    }

    /// The viewport's own height, from the solved layout.
    pub fn resized(self, height: f32) {
        self.viewport.set(height);
    }

    #[must_use]
    pub fn offset(self) -> f32 {
        self.offset.get()
    }

    #[must_use]
    pub fn viewport(self) -> f32 {
        self.viewport.get()
    }
}

/// A virtualized list: a scroll container whose rows exist only near the viewport.
///
/// Rows are keyed by index and reconciled by the same keyed `each` as any other list, so
/// **recycling is re-binding rather than re-creating**: a row leaving the window and one
/// entering it are a move plus a value change, not a destroy plus a create.
///
/// The space the unrealized rows occupy is two spacers, one at each end — which is what
/// makes the scroll extent right without every row having to exist to contribute its
/// height.
#[must_use]
pub fn list<T, V>(
    state: ScrollState,
    spec: impl Fn() -> ListSpec + 'static,
    items: impl Fn(Range<usize>) -> Vec<(usize, T)> + 'static,
    view: impl Fn(&T) -> El<V> + 'static,
) -> View
where
    T: 'static,
    V: 'static,
{
    // One memo, tracking the offset, the viewport and whatever `spec` reads. It settles by
    // value, so a fling that moves the position without moving the window reconciles
    // nothing — which is most of the frames of a fling.
    let visible = Memo::new(move || {
        let spec = spec();
        // The root scope, and correctly so: a row height varies with **density**, which is a
        // root axis, and not with elevation or width — which are the axes a surface pushes.
        // Resolving against the list's own scope would be the same number by a longer route.
        let row_h = crate::role::metric(spec.row_h, crate::build::Host::with(|h| h.root_scope));
        (spec, window(state.offset(), state.viewport(), row_h, &spec))
    });

    scroll(super::stack((
        spacer_rows(
            move || visible.get().1.start as f32,
            move || visible.get().0.row_h,
        ),
        crate::build::each(move || items(visible.get().1), view),
        spacer_rows(
            move || {
                let (spec, window) = visible.get();
                (spec.count - window.end) as f32
            },
            move || visible.get().0.row_h,
        ),
    )))
}

/// The room `n` unrealized rows take up.
///
/// A height of `n` row heights, which [`Len::Times`](super::Len::Times) can say and a raw
/// DIP cannot — the number is a **count**, and the height is still the palette's.
fn spacer_rows(n: impl Fn() -> f32 + 'static, row: impl Fn() -> Metric + 'static) -> View {
    El::<Any>::seed_bare().height_rows(row, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A viewport bigger than its content has no thumb, no travel and nothing to scroll.
    #[test]
    fn content_that_fits_has_no_scrollbar() {
        let g = thumb_geom(400.0, 200.0);
        assert!(!g.overflow);
        assert_eq!((g.max_scroll, g.thumb_h, g.travel), (0.0, 0.0, 0.0));
    }

    /// A very long document still gets a thumb big enough to grab, and its travel is
    /// corrected for that floor rather than running past the end of the track.
    #[test]
    fn a_long_document_keeps_a_grabbable_thumb_inside_its_track() {
        let g = thumb_geom(400.0, 100_000.0);
        assert!(g.overflow);
        assert!((g.thumb_h - THUMB_MIN_H).abs() < 1.0e-3);
        let track = 400.0 - 2.0 * THUMB_MARGIN;
        assert!(g.travel >= 0.0 && g.thumb_h + g.travel <= track + 1.0e-3);
    }

    /// The window covers the viewport, plus the overscan on each side, and never runs past
    /// the ends.
    #[test]
    fn the_realization_window_covers_the_viewport_and_is_clamped_at_both_ends() {
        let spec = ListSpec {
            count: 100,
            row_h: Metric::RowH,
            overscan: 2,
        };
        let top = window(0.0, 100.0, 20.0, &spec);
        assert_eq!(top.start, 0, "the overscan cannot go negative");
        assert!(top.end >= 5 && top.end <= 8);

        let middle = window(400.0, 100.0, 20.0, &spec);
        assert_eq!(middle.start, 18, "twenty rows in, less two of overscan");
        assert_eq!(middle.end, 27, "five visible, plus two either side");

        let bottom = window(1900.0, 100.0, 20.0, &spec);
        assert_eq!(bottom.end, 100, "the overscan cannot go past the last row");
    }

    /// A tracker overpans past the end of the content, and the window stays inside the list.
    ///
    /// The bounce is wanted, so this position is reachable by ordinary use; a window running
    /// past the last row makes `count - window.end` negative, which is a debug panic and, in
    /// release, a trailing spacer some eighteen quintillion rows tall.
    #[test]
    fn an_overpanned_window_stays_inside_the_list() {
        let spec = ListSpec {
            count: 100,
            row_h: Metric::RowH,
            overscan: 2,
        };
        let bounced = window(2100.0, 100.0, 20.0, &spec);
        assert!(bounced.start <= spec.count && bounced.end <= spec.count);
        assert!(
            bounced.is_empty(),
            "nothing past the end is worth realizing"
        );
    }

    /// An empty list realizes nothing, and a zero row height does not divide by itself.
    #[test]
    fn a_degenerate_list_realizes_nothing() {
        let spec = ListSpec {
            count: 0,
            row_h: Metric::RowH,
            overscan: 2,
        };
        assert!(window(0.0, 100.0, 20.0, &spec).is_empty());
        assert!(window(0.0, 100.0, 0.0, &ListSpec { count: 10, ..spec }).is_empty());
    }
}
