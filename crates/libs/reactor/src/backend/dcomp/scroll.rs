//! ScrollViewer thumb geometry — the auto-hiding overlay scrollbar shared by the
//! paint pass (which draws/positions the thumb sprite) and input (hit-testing the
//! thumb for drag-to-scroll). Pure math: no compositor state lives here.

/// Thumb bar width (DIP).
pub(crate) const THUMB_W: f32 = 6.0;
/// Inset of the thumb from the right/top/bottom edges of the viewport (DIP).
pub(crate) const THUMB_MARGIN: f32 = 2.0;
/// Smallest the thumb is allowed to shrink to, however long the content (DIP).
pub(crate) const THUMB_MIN_H: f32 = 24.0;

/// The resolved scrollbar geometry for one scroll container.
#[derive(Clone, Copy)]
pub(crate) struct ThumbGeom {
    /// Content overflows the viewport (a thumb is warranted at all).
    pub overflow: bool,
    /// Maximum scroll offset (content_h − viewport_h, ≥ 0).
    pub max_scroll: f32,
    /// Thumb height (DIP), proportional to viewport/content, floored at the min.
    pub thumb_h: f32,
    /// Thumb top, relative to the viewport's top-left (DIP).
    pub thumb_y: f32,
}

/// Resolve the thumb geometry from the viewport height, total content height, and
/// the current scroll offset.
pub(crate) fn thumb_geom(viewport_h: f32, content_h: f32, scroll: f32) -> ThumbGeom {
    let max_scroll = (content_h - viewport_h).max(0.0);
    let overflow = max_scroll > 0.5 && viewport_h > 0.0 && content_h > 0.0;
    let track_h = (viewport_h - 2.0 * THUMB_MARGIN).max(0.0);
    let ratio = if content_h > 0.0 { (viewport_h / content_h).clamp(0.0, 1.0) } else { 1.0 };
    let thumb_h = (track_h * ratio).max(THUMB_MIN_H).min(track_h);
    let frac = if max_scroll > 0.0 { (scroll / max_scroll).clamp(0.0, 1.0) } else { 0.0 };
    let thumb_y = THUMB_MARGIN + frac * (track_h - thumb_h);
    ThumbGeom { overflow, max_scroll, thumb_h, thumb_y }
}

/// Map a thumb top (DIP, viewport-relative) back to a scroll offset — the inverse
/// of `thumb_y`, used while dragging the thumb.
pub(crate) fn scroll_for_thumb_y(thumb_y: f32, viewport_h: f32, content_h: f32) -> f32 {
    let g = thumb_geom(viewport_h, content_h, 0.0);
    let travel = (viewport_h - 2.0 * THUMB_MARGIN) - g.thumb_h;
    if travel <= 0.0 {
        return 0.0;
    }
    let frac = ((thumb_y - THUMB_MARGIN) / travel).clamp(0.0, 1.0);
    frac * g.max_scroll
}
