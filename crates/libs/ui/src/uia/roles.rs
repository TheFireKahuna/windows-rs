//! One `const` row per [`UiaRole`], and the three lookups over it.
//!
//! A row carries the control type, the spoken type name, the patterns the role answers to,
//! and whether the element appears in the content view. The table is indexed by the enum's
//! own discriminant, so each role has exactly one row.

use crate::bindings::{
    UIA_ButtonControlTypeId, UIA_CheckBoxControlTypeId, UIA_ComboBoxControlTypeId,
    UIA_CustomControlTypeId, UIA_EditControlTypeId, UIA_ExpandCollapsePatternId,
    UIA_GroupControlTypeId, UIA_InvokePatternId, UIA_ListControlTypeId, UIA_ListItemControlTypeId,
    UIA_MenuControlTypeId, UIA_MenuItemControlTypeId, UIA_ProgressBarControlTypeId,
    UIA_RadioButtonControlTypeId, UIA_RangeValuePatternId, UIA_ScrollItemPatternId, UIA_ScrollPatternId,
    UIA_SelectionItemPatternId, UIA_SelectionPatternId, UIA_SliderControlTypeId,
    UIA_TextControlTypeId, UIA_TextPatternId, UIA_TogglePatternId, UIA_ToolTipControlTypeId,
    UIA_ValuePatternId,
    UIA_WindowControlTypeId, UIA_TabControlTypeId, UIA_TabItemControlTypeId,
};
use crate::widget::UiaRole;

/// The set of patterns a role answers to, as a bit mask, so a support check is one test.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Patterns(u16);

impl Patterns {
    pub const NONE: Self = Self(0);
    pub const INVOKE: Self = Self(1 << 0);
    pub const TOGGLE: Self = Self(1 << 1);
    pub const VALUE: Self = Self(1 << 2);
    pub const RANGE: Self = Self(1 << 3);
    pub const SELECTION: Self = Self(1 << 4);
    pub const SELECTION_ITEM: Self = Self(1 << 5);
    pub const EXPAND: Self = Self(1 << 6);
    pub const SCROLL_ITEM: Self = Self(1 << 7);
    pub const TEXT: Self = Self(1 << 8);
    pub const SCROLL: Self = Self(1 << 9);

    /// Returns whether every pattern in `other` is in this set.
    #[must_use]
    pub const fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns the union of the two sets.
    #[must_use]
    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns this set with the patterns in `other` cleared.
    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

/// One role, as automation sees it.
pub struct Row {
    pub control_type: i32,
    /// The spoken name of the control type. A custom control type has no platform default
    /// name, so without this it reads as silence.
    pub localized: &'static str,
    pub patterns: Patterns,
    /// Whether the element is in the **content** view as well as the control view. A
    /// container's decorative frame is a control and not content; a label is both.
    pub content: bool,
}

impl Row {
    const fn new(
        control_type: i32,
        localized: &'static str,
        patterns: Patterns,
        content: bool,
    ) -> Self {
        Self {
            control_type,
            localized,
            patterns,
            content,
        }
    }
}

const P: Patterns = Patterns::NONE;

/// Indexed by [`UiaRole`]'s discriminant through [`row`], never by a bare integer.
///
/// `UiaRole` is fieldless and its variants carry no explicit discriminant, so a role's
/// position in that declaration is its index here, and `every_role_has_its_own_row` is what
/// holds the two orders together.
static ROWS: [Row; 16] = [
    // None — never published; present so the table is total over the enum.
    Row::new(UIA_CustomControlTypeId, "", P, false),
    // A static run publishes its body as a text document, so it can be read, selected and
    // navigated. An editable document belongs to text services and is not this pattern.
    Row::new(UIA_TextControlTypeId, "text", P.or(Patterns::TEXT), true),
    Row::new(UIA_GroupControlTypeId, "group", P, false),
    // A button that opens a flyout is still a button, so the role carries expand-collapse;
    // whether an element answers it is the entry's `EXPANDS` flag.
    Row::new(
        UIA_ButtonControlTypeId,
        "button",
        P.or(Patterns::INVOKE).or(Patterns::EXPAND),
        true,
    ),
    Row::new(
        UIA_CheckBoxControlTypeId,
        "check box",
        P.or(Patterns::TOGGLE),
        true,
    ),
    // A radio button reports `SelectionItem`, which a screen reader announces as "3 of 5"
    // rather than as "checked".
    Row::new(
        UIA_RadioButtonControlTypeId,
        "radio button",
        P.or(Patterns::SELECTION_ITEM),
        true,
    ),
    Row::new(
        UIA_SliderControlTypeId,
        "slider",
        P.or(Patterns::RANGE).or(Patterns::VALUE),
        true,
    ),
    // Queries read the published document; writes return through the editor queue.
    Row::new(
        UIA_EditControlTypeId,
        "edit",
        P.or(Patterns::VALUE).or(Patterns::TEXT),
        true,
    ),
    Row::new(
        UIA_ComboBoxControlTypeId,
        "combo box",
        P.or(Patterns::EXPAND).or(Patterns::VALUE).or(Patterns::SELECTION),
        true,
    ),
    Row::new(
        UIA_ListControlTypeId,
        "list",
        P.or(Patterns::SELECTION).or(Patterns::SCROLL_ITEM),
        true,
    ),
    Row::new(UIA_MenuControlTypeId, "menu", P, true),
    Row::new(
        UIA_ProgressBarControlTypeId,
        "progress bar",
        P.or(Patterns::RANGE).or(Patterns::VALUE),
        true,
    ),
    // Automation has no graph control type, so a graph is a custom control that reports a
    // value, which is what makes a presented analyzer readable.
    Row::new(
        UIA_CustomControlTypeId,
        "graph",
        P.or(Patterns::VALUE).or(Patterns::RANGE),
        true,
    ),
    // Content, because a description is what a reader is meant to hear; the element it
    // describes carries the same words as its help text.
    Row::new(UIA_ToolTipControlTypeId, "tooltip", P, true),
    Row::new(UIA_TabControlTypeId, "tab", Patterns::SELECTION, true),
    Row::new(UIA_TabItemControlTypeId, "tab item", Patterns::SELECTION_ITEM, true),
];

/// Returns the row for `role`.
#[must_use]
pub fn row(role: UiaRole) -> &'static Row {
    &ROWS[role as usize]
}

/// Returns the control type `role` reports inside `parent`, and the name it is spoken by.
///
/// A button is a menu item inside a menu and a list item inside a list, because the same
/// widget is authored for either container. Every other pairing reports the role's own
/// control type.
///
/// Both together, because they are two statements about one element: reporting the type of a
/// menu item while calling it a button is what a reader announces, and nothing downstream
/// would catch the two having been resolved apart.
#[must_use]
pub fn control_type_in(role: UiaRole, parent: UiaRole) -> (i32, &'static str) {
    match (parent, role) {
        (UiaRole::Menu, UiaRole::Button | UiaRole::CheckBox | UiaRole::RadioButton) => (UIA_MenuItemControlTypeId, "menu item"),
        (UiaRole::List, UiaRole::Button) => (UIA_ListItemControlTypeId, "list item"),
        _ => {
            let row = row(role);
            (row.control_type, row.localized)
        }
    }
}

/// The control type a popup reports, which makes a reader announce its title before its
/// content, and the name it is spoken by.
pub const DIALOG_CONTROL_TYPE: i32 = UIA_WindowControlTypeId;
pub const DIALOG_NAME: &str = "dialog";

/// Returns the mask bit standing for automation's pattern `id`, as `GetPatternProvider`
/// needs it, or [`Patterns::NONE`] for a pattern this stack does not answer.
#[must_use]
pub fn pattern_of(id: i32) -> Patterns {
    match id {
        UIA_InvokePatternId => Patterns::INVOKE,
        UIA_TogglePatternId => Patterns::TOGGLE,
        UIA_ValuePatternId => Patterns::VALUE,
        UIA_RangeValuePatternId => Patterns::RANGE,
        UIA_SelectionPatternId => Patterns::SELECTION,
        UIA_SelectionItemPatternId => Patterns::SELECTION_ITEM,
        UIA_ExpandCollapsePatternId => Patterns::EXPAND,
        UIA_ScrollItemPatternId => Patterns::SCROLL_ITEM,
        UIA_ScrollPatternId => Patterns::SCROLL,
        UIA_TextPatternId => Patterns::TEXT,
        _ => Patterns::NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is indexed by the enum's discriminant, so a role added without a row would
    /// read whichever row sits at its position.
    #[test]
    fn every_role_has_its_own_row() {
        let all = [
            UiaRole::None,
            UiaRole::Text,
            UiaRole::Group,
            UiaRole::Button,
            UiaRole::CheckBox,
            UiaRole::RadioButton,
            UiaRole::Slider,
            UiaRole::Edit,
            UiaRole::ComboBox,
            UiaRole::List,
            UiaRole::Menu,
            UiaRole::ProgressBar,
            UiaRole::Graph,
            UiaRole::ToolTip,
            UiaRole::Tab,
            UiaRole::TabItem,
        ];
        assert_eq!(all.len(), ROWS.len(), "a role was added without a row");
        for (at, role) in all.into_iter().enumerate() {
            assert_eq!(role as usize, at, "{role:?} indexes the wrong row");
        }
        // Every published role names its type; only `None`, which is never published, may
        // be silent.
        for role in all.into_iter().skip(1) {
            assert!(!row(role).localized.is_empty(), "{role:?} is unnamed");
        }
    }

    #[test]
    fn a_menu_row_is_a_menu_item_and_a_loose_button_is_a_button() {
        assert_eq!(
            control_type_in(UiaRole::Button, UiaRole::Menu),
            (UIA_MenuItemControlTypeId, "menu item")
        );
        assert_eq!(
            control_type_in(UiaRole::Button, UiaRole::Group),
            (UIA_ButtonControlTypeId, "button")
        );
    }

    #[test]
    fn a_radio_button_selects_and_does_not_toggle() {
        let radio = row(UiaRole::RadioButton).patterns;
        assert!(radio.has(Patterns::SELECTION_ITEM));
        assert!(!radio.has(Patterns::TOGGLE));
        assert!(row(UiaRole::CheckBox).patterns.has(Patterns::TOGGLE));
    }
}
