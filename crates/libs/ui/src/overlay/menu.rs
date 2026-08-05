//! Menus: a flyout whose child is a keyboard-navigable item list.
//!
//! A menu is a [`Flyout`](super::Kind::Flyout). Anchoring, light dismiss, the focus scope and
//! `Esc` belong to the overlay layer and are not restated here.
//!
//! # Typed items
//!
//! [`MenuItem::Check`] and [`MenuItem::Radio`] are variants rather than a generic label the
//! call site composes, so a menu that reports state needs no per-item composition and reports
//! its automation pattern from the variant.
//!
//! # Keyboard
//!
//! `Up`/`Down` move, `Home`/`End` jump, `Right`/`Enter` open a submenu or invoke,
//! `Left`/`Esc` close one level, and printable characters type-ahead by first letter.
//! Disabled items are skipped by navigation and stay in the automation tree, so a screen
//! reader announces "Export, dimmed" rather than omitting the item.
//!
//! Navigation is the focus ring's, restricted to the menu's own scope, so `Up` and `Down`
//! are `Tab` and `Shift-Tab` under different names and there is no second order to keep in
//! step with the first.

use crate::build::{Any, El, View};
use crate::layout::Preset;
use crate::role::{Metric, TypeRole};
use crate::signal::Signal;
use crate::widget::{Interaction, UiaRole, roles};
use windows_scene::HitFlags;

/// One row of a menu.
pub enum MenuItem {
    /// A row that invokes an action. One with `enabled` clear keeps its place in the
    /// automation tree and declares no target.
    Command {
        label: &'static str,
        enabled: bool,
        on_invoke: Box<dyn Fn()>,
    },
    /// A two-state row, reporting the `Toggle` pattern.
    Check {
        label: &'static str,
        checked: Box<dyn Fn() -> bool>,
        on_toggle: Box<dyn Fn()>,
    },
    /// One of a set, reporting the `SelectionItem` pattern.
    Radio {
        label: &'static str,
        selected: Box<dyn Fn() -> bool>,
        on_select: Box<dyn Fn()>,
    },
    /// A rule. Not a target, not in the focus order, and not in the automation tree.
    Separator,
    /// A nested menu, opened by hover after [`SUBMENU_DELAY_MS`](super::SUBMENU_DELAY_MS),
    /// or at once by `Right`, `Enter` or a tap.
    Submenu {
        label: &'static str,
        items: Box<dyn Fn() -> Vec<Self>>,
    },
}

impl MenuItem {
    /// Returns an enabled command row labelled `label`.
    #[must_use]
    pub fn command(label: &'static str, on_invoke: impl Fn() + 'static) -> Self {
        Self::Command {
            label,
            enabled: true,
            on_invoke: Box::new(on_invoke),
        }
    }

    /// Returns this command with invocation disabled, which keeps it in the automation tree
    /// and out of the focus order. Every other variant is returned unchanged.
    #[must_use]
    pub fn disabled(self) -> Self {
        match self {
            Self::Command {
                label, on_invoke, ..
            } => Self::Command {
                label,
                enabled: false,
                on_invoke,
            },
            other => other,
        }
    }

    /// Returns a two-state row labelled `label`, reporting `Toggle` and reading `checked`
    /// for its state.
    #[must_use]
    pub fn check<M>(
        label: &'static str,
        checked: impl Signal<bool, M> + 'static,
        on_toggle: impl Fn() + 'static,
    ) -> Self {
        Self::Check {
            label,
            checked: Box::new(move || checked.read()),
            on_toggle: Box::new(on_toggle),
        }
    }

    /// Returns a row labelled `label` that is one of a set, reporting `SelectionItem` and
    /// reading `selected` for its state.
    #[must_use]
    pub fn radio<M>(
        label: &'static str,
        selected: impl Signal<bool, M> + 'static,
        on_select: impl Fn() + 'static,
    ) -> Self {
        Self::Radio {
            label,
            selected: Box::new(move || selected.read()),
            on_select: Box::new(on_select),
        }
    }

    /// Returns a row labelled `label` that opens a nested menu of the items `items` builds.
    #[must_use]
    pub fn submenu(label: &'static str, items: impl Fn() -> Vec<Self> + 'static) -> Self {
        Self::Submenu {
            label,
            items: Box::new(items),
        }
    }

    /// Returns the automation role this row reports, derived from the variant rather than
    /// declared per item.
    #[must_use]
    pub const fn role(&self) -> UiaRole {
        match self {
            Self::Command { .. } | Self::Submenu { .. } => UiaRole::Button,
            Self::Check { .. } => UiaRole::CheckBox,
            Self::Radio { .. } => UiaRole::RadioButton,
            Self::Separator => UiaRole::None,
        }
    }

    /// Returns whether navigation stops on this row. A separator is not a stop and neither
    /// is a disabled command, though both stay in the automation tree.
    #[must_use]
    pub const fn is_stop(&self) -> bool {
        match self {
            Self::Command { enabled, .. } => *enabled,
            Self::Check { .. } | Self::Radio { .. } | Self::Submenu { .. } => true,
            Self::Separator => false,
        }
    }

    /// Returns the row's label, or `None` for a separator.
    #[must_use]
    pub const fn label(&self) -> Option<&'static str> {
        match self {
            Self::Command { label, .. }
            | Self::Check { label, .. }
            | Self::Radio { label, .. }
            | Self::Submenu { label, .. } => Some(label),
            Self::Separator => None,
        }
    }
}

/// Returns whether `name` answers a type-ahead of `key`, comparing first characters with
/// case folded. An absent or empty name answers nothing.
///
/// Matching runs against the accessible name rather than a list of items kept beside the
/// menu, so type-ahead walks the same candidates the focus ring does and the two cannot
/// disagree once an item is disabled. A menu row states its label as its name.
#[must_use]
pub fn answers(name: Option<&str>, key: char) -> bool {
    let Some(name) = name else { return false };
    let Some(key) = key.to_lowercase().next() else {
        return false;
    };
    name.chars()
        .next()
        .and_then(|first| first.to_lowercase().next())
        == Some(key)
}

/// Returns a menu's body, for [`El::flyout`](crate::build::El::flyout) or for
/// [`Overlays::open`](super::Overlays::open).
///
/// `items` runs immediately and is never stored, so it is bounded by neither `'static` nor
/// `Fn`. Taking a closure rather than a list is what lets a menu that reports state read that
/// state as it opens: the overlay layer calls a flyout's body at open time, and an `El` is an
/// index into an arena the next mount clears.
#[must_use]
pub fn menu(items: impl FnOnce() -> Vec<MenuItem>) -> View {
    rows(items())
}

/// Returns one item lowered to its widget.
fn row(item: MenuItem) -> View {
    let Some(label) = item.label() else {
        // A rule: one hairline sprite, no hit entry, no focus stop, no automation peer.
        return El::<Any>::seed(Preset::Bare)
            .chrome(roles::SURFACE, roles::SURFACE_FLYOUT, Metric::BorderW)
            .height(Metric::BorderW);
    };
    let uia = item.role();
    let stop = item.is_stop();

    let base = El::<Any>::seed(Preset::Bare)
        .control()
        .chrome(roles::OPTION, 0, Metric::Radius)
        .state(crate::widget::StatePolicy::Wash {
            hover: crate::widget::Wash::Ink,
            press: crate::widget::Wash::Ink,
        })
        .row(El::<Any>::seed(Preset::Text).text_seed(
            label.into(),
            TypeRole::Body,
            None,
            crate::widget::Flow::Line,
        ));

    // The label is also the accessible name, which is what `answers` matches on, so
    // type-ahead selects from the same candidates the arrow keys walk.
    let base = base.name(label);

    // A disabled item keeps its row and its automation peer and loses its target. The focus
    // order is the hit array filtered to `INTERACTIVE`, so dropping that flag is the skip.
    let base = if stop {
        base.hit(HitFlags::INTERACTIVE | HitFlags::GESTURE, uia)
    } else {
        base.hit(HitFlags::UIA, uia)
    };

    match item {
        MenuItem::Command { on_invoke, .. } => base.on_click(on_invoke),
        MenuItem::Check {
            checked, on_toggle, ..
        } => base
            .selected(checked)
            .interaction(Interaction::Press)
            .on_click(on_toggle),
        MenuItem::Radio {
            selected,
            on_select,
            ..
        } => base
            .selected(selected)
            .interaction(Interaction::Press)
            .on_click(on_select),
        // The nested list is the item's own flyout, so it opens through the overlay layer's
        // ordinary path, anchored to this row.
        MenuItem::Submenu { items, .. } => base.flyout(move || rows(items())),
        // `Separator` is the only variant `label()` answers `None` for, and that arm
        // returned above. Stated rather than folded into a catch-all, so a variant that
        // grows a `None` label panics here instead of becoming an interactive row.
        MenuItem::Separator => unreachable!("a separator has no label and returned above"),
    }
}

/// Returns the menu surface holding `items`, shared by a menu and its submenus: a submenu is
/// a menu anchored to the row that owns it and nested through the overlay stack.
///
/// Takes built items rather than a closure, so a boxed closure and a caller's own generic one
/// reach this body without a second instantiation.
fn rows(items: Vec<MenuItem>) -> View {
    let rows: Vec<View> = items.into_iter().map(row).collect();
    crate::widget::flyout()
        .stack(rows)
        // An automation container and nothing else: the items route the pointer, and a
        // target over the whole menu would swallow the gaps between them.
        .hit(HitFlags::NONE, UiaRole::Menu)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<MenuItem> {
        vec![
            MenuItem::command("Cut", || {}),
            MenuItem::command("Copy", || {}),
            MenuItem::Separator,
            MenuItem::command("Paste", || {}).disabled(),
            MenuItem::command("Clear", || {}),
        ]
    }

    #[test]
    fn every_item_that_is_not_a_stop_is_still_in_the_tree() {
        // A disabled item is unreachable by keyboard and still announced by a screen reader.
        let items = sample();
        assert!(!items[3].is_stop(), "the dimmed item is not a focus stop");
        assert_eq!(items[3].role(), UiaRole::Button, "and it is still a button");
        assert!(!items[2].is_stop());
        assert_eq!(items[2].role(), UiaRole::None, "a rule has no peer");
    }

    #[test]
    fn type_ahead_matches_a_name_by_its_first_letter_either_case() {
        // The candidate order and the wrapping belong to `FocusRing::step_to`; `answers` is
        // only the predicate that order is filtered by.
        assert!(answers(Some("Copy"), 'c'));
        assert!(answers(Some("Copy"), 'C'), "case folds");
        assert!(!answers(Some("Copy"), 'z'));
        assert!(!answers(None, 'c'), "an unnamed control answers nothing");
        assert!(!answers(Some(""), 'c'));
    }

    #[test]
    fn a_row_states_its_label_as_the_name_type_ahead_reads() {
        // A row that stopped naming itself would leave type-ahead matching nothing.
        for item in sample() {
            let label = item.label();
            if let Some(label) = label {
                assert!(answers(Some(label), label.chars().next().unwrap()));
            }
        }
    }

    #[test]
    fn a_variant_reports_its_own_automation_pattern() {
        // The role is derived from the variant, so a menu that reports state cannot omit its
        // automation pattern.
        assert_eq!(
            MenuItem::check("Mono", || true, || {}).role(),
            UiaRole::CheckBox
        );
        assert_eq!(
            MenuItem::radio("Flat", || true, || {}).role(),
            UiaRole::RadioButton
        );
        assert_eq!(
            MenuItem::submenu("More", Vec::new).role(),
            UiaRole::Button,
            "a submenu invokes like a button and is expanded by the overlay layer"
        );
    }
}
