//! Declares the three window commands — minimize, maximize and close — over the controls a
//! title bar already mounts.
//!
//! `windows-window` owns every caption behaviour — the drag strip, the eight resize edges,
//! double-click maximize, the window menu, the `SC_*` a press issues — and draws nothing; the
//! application draws the bar and the buttons in it. Two functions join the halves, answering the
//! two questions the window cannot answer from its own state:
//!
//! * what is at this point, per `WM_NCHITTEST`, and
//! * what the pointer is doing to a button whose input the system took.
//!
//! [`hit`] resolves the point through the one hit array, so the drag strip is whatever the bar's
//! controls leave over. [`controls`] answers with the same [`ControlId`]s every other control
//! carries, so a window command hovers and presses down the path a button uses.
//!
//! The table is three `Option<ControlId>` carried by value on both seams, because three ids are
//! cheaper to carry than a hop, and nothing clears an entry when a bar unmounts: a [`ControlId`]
//! is generational, so an id left by an unmounted bar can never equal a live hit's id, and a bar
//! that remounts overwrites its own entries as it goes.

use windows_scene::{ContactKind, ControlId, HitFlags, HitTable, Point};
use windows_window::{CaptionButton, CaptionHit, CaptionState};

/// The three commands, in the order [`slot`] indexes them.
pub(crate) const BUTTONS: [CaptionButton; 3] = [
    CaptionButton::Minimize,
    CaptionButton::Maximize,
    CaptionButton::Close,
];

/// The index a command occupies in the three-slot table.
pub(crate) const fn slot(button: CaptionButton) -> usize {
    match button {
        CaptionButton::Minimize => 0,
        CaptionButton::Maximize => 1,
        CaptionButton::Close => 2,
    }
}

/// Resolves a point in the caption band to a window command, the client area, or the drag strip.
///
/// Answers [`Window::on_caption_hit`]. `x` and `y` are client-space DIPs, the space the window
/// reports and the layout solves in, so this converts no coordinates.
///
/// `table` is the copy the answering thread holds, so this resolves a point without a hop to the
/// thread the bar mounted on.
///
/// [`Window::on_caption_hit`]: windows_window::Window::on_caption_hit
pub(crate) fn hit(x: f32, y: f32, hits: &HitTable, table: [Option<ControlId>; 3]) -> CaptionHit {
    // A point over nothing interactive is the drag strip: the strip is whatever the bar's own
    // controls leave over, not a second rect stated beside them.
    let Some(found) = hits.hit(Point { x, y }, ContactKind::Mouse) else {
        return CaptionHit::Drag;
    };
    if !found.flags.contains(HitFlags::INTERACTIVE) {
        return CaptionHit::Drag;
    }
    match BUTTONS
        .into_iter()
        .find(|b| table[slot(*b)] == Some(found.id))
    {
        Some(button) => CaptionHit::Button(button),
        // An ordinary control answers for the client: the array has already reported something
        // interactive here, and dragging the window from on top of a control would swallow the
        // press.
        None => CaptionHit::Client,
    }
}

/// Resolves the hovered and pressed commands in `state` to the controls that draw them.
///
/// Feeds [`Controls::nonclient`]. Either half is `None` when `state` names no command, or when
/// no mounted bar declared that command.
///
/// [`Controls::nonclient`]: crate::widget::Controls::nonclient
pub(crate) fn controls(
    state: CaptionState,
    table: [Option<ControlId>; 3],
) -> (Option<ControlId>, Option<ControlId>) {
    let of = |button: Option<CaptionButton>| table[slot(button?)];
    (of(state.hover), of(state.pressed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_scene::{HitEntry, NO_ENTRY, NodeId};

    fn cid(raw: u32) -> ControlId {
        ControlId::raw(raw, 1)
    }

    fn entry(id: ControlId, x0: f32, x1: f32) -> HitEntry {
        HitEntry {
            x0,
            y0: 0.0,
            x1,
            y1: 32.0,
            touch_inflate: 0.0,
            clip_parent: NO_ENTRY,
            parent: NO_ENTRY,
            flags: HitFlags::INTERACTIVE,
            scroll_src: NodeId::NONE,
            id,
        }
    }

    /// Resolves a command, an ordinary control and the drag strip from the one hit array.
    ///
    /// The rects stand in for a solve. What is under test is that a point reaches a command
    /// through the array rather than through a rect stated beside the bar, and that bare band
    /// and ordinary control are told apart.
    #[test]
    fn a_point_resolves_to_the_command_the_table_names() {
        let (other, min, max, close) = (cid(1), cid(2), cid(3), cid(4));
        let table = [Some(min), Some(max), Some(close)];
        let mut hits = HitTable::default();
        // An ordinary control first, then the commands, in bar order.
        hits.replace(
            &[
                entry(other, 0.0, 60.0),
                entry(min, 100.0, 146.0),
                entry(max, 146.0, 192.0),
                entry(close, 192.0, 238.0),
            ],
            &[],
        );

        assert_eq!(
            hit(80.0, 16.0, &hits, table),
            CaptionHit::Drag,
            "between them"
        );
        assert_eq!(
            hit(30.0, 16.0, &hits, table),
            CaptionHit::Client,
            "the control"
        );
        assert_eq!(
            hit(120.0, 16.0, &hits, table),
            CaptionHit::Button(CaptionButton::Minimize)
        );
        assert_eq!(
            hit(170.0, 16.0, &hits, table),
            CaptionHit::Button(CaptionButton::Maximize)
        );
        assert_eq!(
            hit(210.0, 16.0, &hits, table),
            CaptionHit::Button(CaptionButton::Close)
        );
    }

    /// Reports no command for a bar that declares none, however interactive its controls are.
    ///
    /// The failure this rules out is a table left populated by an earlier bar: an undeclared
    /// close button would answer `HTCLOSE` over an ordinary control and hand the system a press
    /// the application never drew.
    #[test]
    fn an_undeclared_control_is_never_a_command() {
        let mut hits = HitTable::default();
        hits.replace(&[entry(cid(1), 0.0, 60.0)], &[]);
        assert_eq!(hit(30.0, 16.0, &hits, [None; 3]), CaptionHit::Client);
        assert_eq!(hit(90.0, 16.0, &hits, [None; 3]), CaptionHit::Drag);
    }

    /// A control that has stopped routing pointer events is band, not client.
    #[test]
    fn a_non_interactive_entry_is_the_drag_strip() {
        let mut hits = HitTable::default();
        let mut row = entry(cid(1), 0.0, 60.0);
        row.flags = HitFlags::NONE;
        hits.replace(&[row], &[]);
        assert_eq!(hit(30.0, 16.0, &hits, [None; 3]), CaptionHit::Drag);
    }

    /// Maps a forwarded [`CaptionState`] onto control ids, so a command lights through the same
    /// path an ordinary control's hover uses.
    #[test]
    fn caption_state_names_the_controls_it_lights() {
        let close = cid(4);
        let table = [None, None, Some(close)];
        assert_eq!(controls(CaptionState::default(), table), (None, None));
        assert_eq!(
            controls(
                CaptionState {
                    hover: Some(CaptionButton::Close),
                    pressed: Some(CaptionButton::Close),
                },
                table
            ),
            (Some(close), Some(close))
        );
        assert_eq!(
            controls(
                CaptionState {
                    hover: Some(CaptionButton::Minimize),
                    pressed: None,
                },
                table
            ),
            (None, None),
            "a command no bar declared lights nothing"
        );
    }
}
