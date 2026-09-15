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
//! An application declares a command where it authors the bar, with
//! [`Element::caption`](crate::build::Element::caption). Both answers here are the driver's, resolved
//! against the [`Registry`] copy the answering thread holds.

use windows_scene::{ContactKind, ControlId, HitTable, Point, ScrollOffsets};
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
///
/// `Copy`, because the thread that answers `WM_NCHITTEST` holds its own copy rather than
/// reaching the table the mount writes: three ids are cheaper to carry than a hop.
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub(crate) struct Registry([Option<ControlId>; 3]);

impl Registry {
    pub(crate) fn set(&mut self, button: CaptionButton, id: ControlId) {
        self.0[slot(button)] = Some(id);
    }

    pub(crate) fn id(&self, button: CaptionButton) -> Option<ControlId> {
        self.0[slot(button)]
    }

    fn button(&self, id: ControlId) -> Option<CaptionButton> {
        BUTTONS.into_iter().find(|&b| self.id(b) == Some(id))
    }
}

impl From<Registry> for [Option<ControlId>; 3] {
    fn from(registry: Registry) -> Self {
        registry.0
    }
}

impl From<[Option<ControlId>; 3]> for Registry {
    fn from(ids: [Option<ControlId>; 3]) -> Self {
        Self(ids)
    }
}

/// Resolves a point in the caption band to a window command, the client area, or the drag
/// strip.
///
/// Answers [`Window::on_caption_hit`]. `x` and `y` are client-space DIPs, the space the window
/// reports and the layout solves in, so this converts no coordinates.
///
/// `registry` is the copy the answering thread holds, so this resolves a point without a hop
/// to the thread the bar mounted on.
///
/// [`Window::on_caption_hit`]: windows_window::Window::on_caption_hit
#[must_use]
pub(crate) fn hit(
    hits: &HitTable,
    offsets: &dyn ScrollOffsets,
    registry: &Registry,
    x: f32,
    y: f32,
) -> CaptionHit {
    // A point over nothing interactive is the drag strip: the strip is whatever the bar's own
    // controls leave over, not a second rect stated beside them.
    let Some(found) = hits.hit_with(Point { x, y }, ContactKind::Mouse, offsets) else {
        return CaptionHit::Drag;
    };
    match registry.button(found.id) {
        Some(button) => CaptionHit::Button(button),
        // An ordinary control answers for the client: the array has already reported
        // something interactive here, and dragging the window from on top of a control would
        // swallow the press.
        None => CaptionHit::Client,
    }
}

/// Resolves the hovered and pressed commands in `state` to the controls that draw them.
///
/// Feeds [`Controls::nonclient`]. Either half is `None` when `state` names no command, or when
/// no mounted bar declared that command.
///
/// [`Controls::nonclient`]: crate::widget::Controls::nonclient
#[must_use]
pub(crate) fn controls(
    registry: &Registry,
    state: CaptionState,
) -> (Option<ControlId>, Option<ControlId>) {
    (
        state.hover.and_then(|b| registry.id(b)),
        state.pressed.and_then(|b| registry.id(b)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{Host, tests::fixture};
    use crate::layout::row;
    use crate::widget::button;
    use windows_scene::{HitEntry, HitFlags, NO_ENTRY, NodeId, ShadowOffsets};

    /// A bar with the three commands and one ordinary control in it.
    fn bar<'a>(ui: &'a mut crate::build::Ui<'_>) -> crate::build::Element<'a> {
        row(ui, |ui| {
            button(ui, "file").name("File");
            button(ui, "\u{2013}").caption(CaptionButton::Minimize);
            button(ui, "\u{25a1}").caption(CaptionButton::Maximize);
            button(ui, "\u{2715}").caption(CaptionButton::Close);
        })
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
        let _mount = crate::build::Ui::mount_at(
            Host::with(|h| h.model().root()),
            None,
            crate::build::root_scope(),
            None,
            |ui| {
                bar(ui);
            },
        );

        let registry = Host::with(|h| h.caption);
        let [min, max, close] =
            BUTTONS.map(|b| registry.id(b).expect("each command registered at mount"));
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

        assert_eq!(
            hit(&hits, &ShadowOffsets::new(), &registry, 80.0, 16.0),
            CaptionHit::Drag,
            "between them"
        );
        assert_eq!(
            hit(&hits, &ShadowOffsets::new(), &registry, 30.0, 16.0),
            CaptionHit::Client,
            "the control"
        );
        assert_eq!(
            hit(&hits, &ShadowOffsets::new(), &registry, 120.0, 16.0),
            CaptionHit::Button(CaptionButton::Minimize)
        );
        assert_eq!(
            hit(&hits, &ShadowOffsets::new(), &registry, 170.0, 16.0),
            CaptionHit::Button(CaptionButton::Maximize)
        );
        assert_eq!(
            hit(&hits, &ShadowOffsets::new(), &registry, 210.0, 16.0),
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
        let _mount = crate::build::Ui::mount_at(
            Host::with(|h| h.model().root()),
            None,
            crate::build::root_scope(),
            None,
            |ui| {
                row(ui, |ui| {
                    button(ui, "one");
                    button(ui, "two");
                });
            },
        );

        let registry = Host::with(|h| h.caption);
        assert!(BUTTONS.iter().all(|&b| registry.id(b).is_none()));

        let mut hits = HitTable::default();
        hits.replace(&[entry(ControlId::default(), 0.0, 60.0)]);
        assert_eq!(
            hit(&hits, &ShadowOffsets::new(), &registry, 30.0, 16.0),
            CaptionHit::Client
        );
        assert_eq!(
            hit(&hits, &ShadowOffsets::new(), &registry, 90.0, 16.0),
            CaptionHit::Drag
        );
    }

    /// Maps a forwarded [`CaptionState`] onto control ids, so a command lights through the
    /// same path an ordinary control's hover uses.
    #[test]
    fn caption_state_names_the_controls_it_lights() {
        let _patch = fixture();
        let _mount = crate::build::Ui::mount_at(
            Host::with(|h| h.model().root()),
            None,
            crate::build::root_scope(),
            None,
            |ui| {
                bar(ui);
            },
        );
        let registry = Host::with(|h| h.caption);
        let close = registry.id(CaptionButton::Close);

        assert_eq!(controls(&registry, CaptionState::default()), (None, None));
        assert_eq!(
            controls(
                &registry,
                CaptionState {
                    hover: Some(CaptionButton::Close),
                    pressed: Some(CaptionButton::Close),
                }
            ),
            (close, close)
        );
    }

    /// Resolves a command from a registry copy alone, with no host on the thread.
    ///
    /// The copy is what the answering thread holds, so a point has to reach a command
    /// through it and through the hit array and nothing else.
    #[test]
    fn a_registry_copy_resolves_a_command_without_a_host() {
        let close = ControlId::default();
        let registry = Registry::from([None, None, Some(close)]);

        let mut hits = HitTable::default();
        hits.replace(&[entry(close, 192.0, 238.0)]);

        assert_eq!(
            hit(&hits, &ShadowOffsets::new(), &registry, 210.0, 16.0),
            CaptionHit::Button(CaptionButton::Close)
        );
        assert_eq!(
            hit(&hits, &ShadowOffsets::new(), &registry, 90.0, 16.0),
            CaptionHit::Drag
        );
        assert_eq!(
            controls(
                &registry,
                CaptionState {
                    hover: Some(CaptionButton::Close),
                    pressed: None,
                }
            ),
            (Some(close), None)
        );
        assert_eq!(
            <[Option<ControlId>; 3]>::from(registry),
            [None, None, Some(close)],
            "the three ids the seam carries"
        );
    }
}
