//! Declares the three window commands — minimize, maximize and close — over the controls a
//! title bar already mounts.
//!
//! `windows-window` owns every caption behaviour — the drag strip, the eight resize edges,
//! double-click maximize, the window menu, the `SC_*` a press issues — and draws nothing; the
//! application draws the bar and the buttons in it. Two functions join the halves, answering
//! the two questions the window cannot answer from its own state:
//!
//! * what is at this point, per `WM_NCHITTEST`, and
//! * what the pointer is doing to a button whose input the system took.
//!
//! [`hit`] resolves the point through the one hit array, so the drag strip is whatever the
//! bar's controls leave over. [`controls`] answers with the same [`ControlId`]s every other
//! control carries, so a window command hovers and presses down the path a button uses.
//!
//! ```no_run
//! # use windows_ui::{caption, widget::{button, Controls, Front}};
//! # use windows_window::{CaptionButton, CaptionHit, CaptionState, Window};
//! # fn f(window: &Window, hits: &windows_scene::HitTable, controls: &mut Controls,
//! #      front: &mut Front<'_>, state: CaptionState) -> windows_core::Result<()> {
//! // Declared where the bar is authored.
//! let close = button("\u{2715}").ghost().caption(CaptionButton::Close);
//!
//! // Answered from the array the same mount built.
//! let _: CaptionHit = caption::hit(hits, 12.0, 8.0);
//!
//! // And the state the window forwards, applied through the ordinary control path.
//! let (hover, pressed) = caption::controls(state);
//! controls.nonclient(hover, pressed, front)?;
//! # Ok(()) }
//! ```

use crate::build::Host;
use windows_scene::{ContactKind, ControlId, HitTable, Point};
use windows_window::{CaptionButton, CaptionHit, CaptionState};

/// Lists the three commands in the order [`slot`] indexes them.
const BUTTONS: [CaptionButton; 3] = [
    CaptionButton::Minimize,
    CaptionButton::Maximize,
    CaptionButton::Close,
];

const fn slot(button: CaptionButton) -> usize {
    match button {
        CaptionButton::Minimize => 0,
        CaptionButton::Maximize => 1,
        CaptionButton::Close => 2,
    }
}

/// Maps each window command to the control that draws it.
///
/// Nothing clears an entry when a bar unmounts. A [`ControlId`] is generational, so an id left
/// by an unmounted bar can never equal a live hit's id, and a bar that remounts overwrites its
/// own entries as it goes.
#[derive(Default)]
pub(crate) struct Registry([Option<ControlId>; 3]);

impl Registry {
    pub(crate) fn set(&mut self, button: CaptionButton, id: ControlId) {
        self.0[slot(button)] = Some(id);
    }

    fn id(&self, button: CaptionButton) -> Option<ControlId> {
        self.0[slot(button)]
    }

    fn button(&self, id: ControlId) -> Option<CaptionButton> {
        BUTTONS.into_iter().find(|&b| self.id(b) == Some(id))
    }
}

/// Resolves a point in the caption band to a window command, the client area, or the drag
/// strip.
///
/// Answers [`Window::on_caption_hit`]. `x` and `y` are client-space DIPs, the space the window
/// reports and the layout solves in, so this converts no coordinates.
///
/// [`Window::on_caption_hit`]: windows_window::Window::on_caption_hit
#[must_use]
pub fn hit(hits: &HitTable, x: f32, y: f32) -> CaptionHit {
    // A point over nothing interactive is the drag strip: the strip is whatever the bar's own
    // controls leave over, not a second rect stated beside them.
    let Some(found) = hits.hit(Point { x, y }, ContactKind::Mouse) else {
        return CaptionHit::Drag;
    };
    match Host::try_with(|h| h.caption.button(found.id)) {
        Some(Some(button)) => CaptionHit::Button(button),
        // An ordinary control, or a re-entrant call that could not borrow the host. Both
        // answer for the client: the array has already reported something interactive here,
        // and dragging the window from on top of a control would swallow the press.
        _ => CaptionHit::Client,
    }
}

/// Resolves the hovered and pressed commands in `state` to the controls that draw them.
///
/// Feeds [`Controls::nonclient`]. Either half is `None` when `state` names no command, when no
/// mounted bar declared that command, or when the call is re-entrant on the host.
///
/// [`Controls::nonclient`]: crate::widget::Controls::nonclient
#[must_use]
pub fn controls(state: CaptionState) -> (Option<ControlId>, Option<ControlId>) {
    Host::try_with(|h| {
        (
            state.hover.and_then(|b| h.caption.id(b)),
            state.pressed.and_then(|b| h.caption.id(b)),
        )
    })
    .unwrap_or((None, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{mount, tests::fixture};
    use crate::layout::row;
    use crate::widget::button;
    use windows_scene::{HitEntry, HitFlags, NO_ENTRY, NodeId};

    /// A bar with the three commands and one ordinary control in it.
    fn bar() -> crate::build::View {
        row((
            button("file").name("File"),
            button("\u{2013}").caption(CaptionButton::Minimize),
            button("\u{25a1}").caption(CaptionButton::Maximize),
            button("\u{2715}").caption(CaptionButton::Close),
        ))
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
    fn a_point_resolves_to_the_command_the_mount_declared() {
        let _patch = fixture();
        let _mount = mount(bar(), Host::with(|h| h.model().root()));

        let ids = Host::with(|h| BUTTONS.map(|b| h.caption.id(b)));
        let [min, max, close] = ids.map(|id| id.expect("each command registered at mount"));
        assert!(
            min != max && max != close && min != close,
            "three commands, three identities"
        );

        let mut hits = HitTable::default();
        // An ordinary control first, then the commands, in bar order.
        hits.replace(&[
            entry(ControlId::default(), 0.0, 60.0),
            entry(min, 100.0, 146.0),
            entry(max, 146.0, 192.0),
            entry(close, 192.0, 238.0),
        ]);

        assert_eq!(hit(&hits, 80.0, 16.0), CaptionHit::Drag, "between them");
        assert_eq!(hit(&hits, 30.0, 16.0), CaptionHit::Client, "the control");
        assert_eq!(
            hit(&hits, 120.0, 16.0),
            CaptionHit::Button(CaptionButton::Minimize)
        );
        assert_eq!(
            hit(&hits, 170.0, 16.0),
            CaptionHit::Button(CaptionButton::Maximize)
        );
        assert_eq!(
            hit(&hits, 210.0, 16.0),
            CaptionHit::Button(CaptionButton::Close)
        );
    }

    /// Reports no command for a bar that declares none, however interactive its controls are.
    ///
    /// The failure this rules out is a registry left populated by an earlier bar: an
    /// undeclared close button would answer `HTCLOSE` over an ordinary control and hand the
    /// system a press the application never drew.
    #[test]
    fn an_undeclared_control_is_never_a_command() {
        let _patch = fixture();
        let _mount = mount(
            row((button("one"), button("two"))),
            Host::with(|h| h.model().root()),
        );

        assert!(Host::with(|h| BUTTONS
            .iter()
            .all(|&b| h.caption.id(b).is_none())));

        let mut hits = HitTable::default();
        hits.replace(&[entry(ControlId::default(), 0.0, 60.0)]);
        assert_eq!(hit(&hits, 30.0, 16.0), CaptionHit::Client);
        assert_eq!(hit(&hits, 90.0, 16.0), CaptionHit::Drag);
    }

    /// Maps a forwarded [`CaptionState`] onto control ids, so a command lights through the
    /// same path an ordinary control's hover uses.
    #[test]
    fn caption_state_names_the_controls_it_lights() {
        let _patch = fixture();
        let _mount = mount(bar(), Host::with(|h| h.model().root()));
        let close = Host::with(|h| h.caption.id(CaptionButton::Close));

        assert_eq!(controls(CaptionState::default()), (None, None));
        assert_eq!(
            controls(CaptionState {
                hover: Some(CaptionButton::Close),
                pressed: Some(CaptionButton::Close),
            }),
            (close, close)
        );
    }
}
