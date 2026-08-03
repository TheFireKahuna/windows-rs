//! The menu: the one overlay with enough structure to be worth a widget.
//!
//! A menu is a [`Flyout`](super::Kind::Flyout) whose child is a keyboard-navigable item
//! list. Everything else about it — anchoring, light dismiss, the focus scope, `Esc` — is
//! the overlay layer's and is not restated here.
//!
//! # Why the items are typed
//!
//! [`MenuItem::Check`] and [`MenuItem::Radio`] exist rather than a generic label plus
//! whatever the call site composes, because a menu that reports state — a channel scope
//! picker, a routing preset list — then needs no per-item composition *and* reports the
//! right automation pattern automatically. A generic item makes both the caller's problem,
//! and the second one is the half that silently does not happen.
//!
//! # Keyboard
//!
//! `Up`/`Down` move, `Home`/`End` jump, `Right`/`Enter` open a submenu or invoke,
//! `Left`/`Esc` close one level, and printable characters type-ahead by first letter.
//! **Disabled items are skipped by navigation but remain in the automation tree** — a
//! screen reader announcing "Export, dimmed" is telling the user something true, where an
//! item that simply is not there is telling them something false.
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
    Command {
        label: &'static str,
        enabled: bool,
        on_invoke: Box<dyn Fn()>,
    },
    Check {
        label: &'static str,
        checked: Box<dyn Fn() -> bool>,
        on_toggle: Box<dyn Fn()>,
    },
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
    /// A plain command.
    #[must_use]
    pub fn command(label: &'static str, on_invoke: impl Fn() + 'static) -> Self {
        Self::Command {
            label,
            enabled: true,
            on_invoke: Box::new(on_invoke),
        }
    }

    /// A command that is present but cannot be invoked. **Present** is the point: it stays
    /// in the automation tree and out of the focus order.
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

    /// A two-state item, reporting `Toggle`.
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

    /// One of a set, reporting `SelectionItem`.
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

    /// A nested menu.
    #[must_use]
    pub fn submenu(label: &'static str, items: impl Fn() -> Vec<Self> + 'static) -> Self {
        Self::Submenu {
            label,
            items: Box::new(items),
        }
    }

    /// The automation role this variant reports. Derived rather than declared, which is the
    /// whole reason the variants are typed.
    #[must_use]
    pub const fn role(&self) -> UiaRole {
        match self {
            Self::Command { .. } | Self::Submenu { .. } => UiaRole::Button,
            Self::Check { .. } => UiaRole::CheckBox,
            Self::Radio { .. } => UiaRole::RadioButton,
            Self::Separator => UiaRole::None,
        }
    }

    /// Whether navigation stops here. A separator is not a stop, and neither is a disabled
    /// command — though both remain in the tree.
    #[must_use]
    pub const fn is_stop(&self) -> bool {
        match self {
            Self::Command { enabled, .. } => *enabled,
            Self::Check { .. } | Self::Radio { .. } | Self::Submenu { .. } => true,
            Self::Separator => false,
        }
    }

    /// Its label, where it has one.
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

/// Whether a control's accessible name answers a type-ahead keystroke.
///
/// The **name**, and not a list of items kept beside the menu: navigation resolves through
/// the focus ring, so type-ahead has to walk the same candidates in the same order or the
/// two disagree the moment an item is disabled. A menu row states its label as its name for
/// exactly this reason, and a screen reader gets it for free.
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

/// A menu's body, for [`El::flyout`](crate::build::El::flyout) or for
/// [`Overlays::open`](super::Overlays::open).
///
/// The items come from a closure rather than a list because a menu that reports state has to
/// read that state **when it opens**. This runs it immediately, which is the same moment:
/// the overlay layer calls a flyout's body at open time, and an `El` is an index into an
/// arena that the next mount clears, so there is nowhere else it could be called from. It is
/// run once and never stored, so it is bounded by neither `'static` nor `Fn`.
#[must_use]
pub fn menu(items: impl FnOnce() -> Vec<MenuItem>) -> View {
    rows(items())
}

/// One item, lowered to the widget it is.
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

    // The label is also the accessible name, which is what type-ahead matches on: the focus
    // ring is the one order, so a letter has to select from the same candidates the arrows
    // walk rather than from a list kept beside them.
    let base = base.name(label);

    // A disabled item keeps its row and its automation peer and loses its target, which is
    // exactly what "skipped by navigation, present in the tree" has to mean: the focus order
    // is the hit array filtered to `INTERACTIVE`, so dropping the flag *is* the skip.
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
        // The nested list is the item's own flyout, so opening it is the layer's ordinary
        // open path and a submenu is a menu anchored to a row.
        MenuItem::Submenu { items, .. } => base.flyout(move || rows(items())),
        // `label()` answers `None` for a separator alone, and that arm already returned.
        // Stated rather than folded into a catch-all, so a variant that grows a `None` label
        // fails here instead of quietly becoming an interactive row.
        MenuItem::Separator => unreachable!("a separator has no label and returned above"),
    }
}

/// The surface, shared by a menu and its submenus — a submenu **is** a menu, anchored to the
/// row that owns it and nesting through the overlay stack like anything else, so it is the
/// same function and not a copy of one.
///
/// Takes the built items rather than a closure, so a boxed one and a caller's own generic one
/// reach the same body without a second instantiation.
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
        // The distinction the whole disabled treatment rests on: unreachable by keyboard,
        // and announced by a screen reader. An item that is simply absent tells the user
        // something false.
        let items = sample();
        assert!(!items[3].is_stop(), "the dimmed item is not a focus stop");
        assert_eq!(items[3].role(), UiaRole::Button, "and it is still a button");
        assert!(!items[2].is_stop());
        assert_eq!(items[2].role(), UiaRole::None, "a rule has no peer");
    }

    #[test]
    fn type_ahead_matches_a_name_by_its_first_letter_either_case() {
        // The order and the wrapping are `FocusRing::step_to`'s — a menu carrying its own
        // item cursor beside that ring would be the second order the one hit array exists
        // to prevent. What is left here is the predicate, which is all this owns.
        assert!(answers(Some("Copy"), 'c'));
        assert!(answers(Some("Copy"), 'C'), "case folds");
        assert!(!answers(Some("Copy"), 'z'));
        assert!(!answers(None, 'c'), "an unnamed control answers nothing");
        assert!(!answers(Some(""), 'c'));
    }

    #[test]
    fn a_row_states_its_label_as_the_name_type_ahead_reads() {
        // The join between the two halves. If a row ever stops naming itself, type-ahead
        // silently matches nothing and the only symptom is a keystroke that does nothing.
        for item in sample() {
            let label = item.label();
            if let Some(label) = label {
                assert!(answers(Some(label), label.chars().next().unwrap()));
            }
        }
    }

    #[test]
    fn a_variant_reports_its_own_automation_pattern() {
        // The reason the variants are typed rather than one labelled item: this is derived,
        // so a menu that reports state cannot forget to say so.
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
