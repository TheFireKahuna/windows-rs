//! What a role means to automation: one `const` row each.
//!
//! The reference spent seven functions and a `ControlKind` cross-product on this — control
//! type, localized type, presentational-or-not, value-is-a-string, item type, is-a-
//! container, pattern-supported — each a match that had to be extended in step with the
//! others. A row cannot fall out of step with itself, and a new role is visibly a new row.

use crate::bindings::{
    UIA_ButtonControlTypeId, UIA_CheckBoxControlTypeId, UIA_ComboBoxControlTypeId,
    UIA_CustomControlTypeId, UIA_EditControlTypeId, UIA_ExpandCollapsePatternId,
    UIA_GroupControlTypeId, UIA_InvokePatternId, UIA_ListControlTypeId, UIA_ListItemControlTypeId,
    UIA_MenuControlTypeId, UIA_MenuItemControlTypeId, UIA_ProgressBarControlTypeId,
    UIA_RadioButtonControlTypeId, UIA_RangeValuePatternId, UIA_ScrollItemPatternId,
    UIA_SelectionItemPatternId, UIA_SelectionPatternId, UIA_SliderControlTypeId,
    UIA_TextControlTypeId, UIA_TextPatternId, UIA_TogglePatternId, UIA_ValuePatternId,
    UIA_WindowControlTypeId,
};
use crate::widget::UiaRole;

/// Which patterns a role answers to. A mask rather than a list, so support is one test.
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
    pub const SCROLL_ITEM: Self = Self(1 << 8);
    pub const TEXT: Self = Self(1 << 9);

    #[must_use]
    pub const fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// The same set without `other`.
    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

/// One role, as automation sees it.
pub struct Row {
    pub control_type: i32,
    /// The spoken name of the type. Automation has a default per control type, but a
    /// custom type has none at all and reads as silence.
    pub localized: &'static str,
    pub patterns: Patterns,
    /// Whether the element is in the **content** view as well as the control view. A
    /// container's decorative frame is a control and not content; a label is both.
    pub content: bool,
}

const P: Patterns = Patterns::NONE;

/// Indexed by [`UiaRole`] through [`row`], never by a bare integer.
static ROWS: [Row; 13] = [
    // None — never published; present so the table is total over the enum.
    Row {
        control_type: UIA_CustomControlTypeId,
        localized: "",
        patterns: P,
        content: false,
    },
    Row {
        control_type: UIA_TextControlTypeId,
        localized: "text",
        // A static run publishes its body as a text document, so it can be read, selected
        // and navigated. This is the surface `TextPattern` exists for here — an editable
        // one belongs to text services, and advertising it before that lands would be a
        // pattern that answers every call by refusing.
        patterns: P.or(Patterns::TEXT),
        content: true,
    },
    Row {
        control_type: UIA_GroupControlTypeId,
        localized: "group",
        patterns: P,
        content: false,
    },
    Row {
        control_type: UIA_ButtonControlTypeId,
        localized: "button",
        // Expand-collapse rides along because a button that opens a flyout is still a
        // button; whether it *answers* is the entry's own flag, not the role's.
        patterns: P.or(Patterns::INVOKE).or(Patterns::EXPAND),
        content: true,
    },
    Row {
        control_type: UIA_CheckBoxControlTypeId,
        localized: "check box",
        patterns: P.or(Patterns::TOGGLE),
        content: true,
    },
    // A radio button reports `SelectionItem`, which a screen reader announces as "3 of 5"
    // rather than as "checked". That distinction is the whole reason the role exists.
    Row {
        control_type: UIA_RadioButtonControlTypeId,
        localized: "radio button",
        patterns: P.or(Patterns::SELECTION_ITEM),
        content: true,
    },
    Row {
        control_type: UIA_SliderControlTypeId,
        localized: "slider",
        patterns: P.or(Patterns::RANGE).or(Patterns::VALUE),
        content: true,
    },
    Row {
        control_type: UIA_EditControlTypeId,
        localized: "edit",
        // No `TEXT`: an editable document is text services' and is not written yet. A role
        // that advertised the pattern and then refused every call would be worse than one
        // that does not claim it.
        patterns: P.or(Patterns::VALUE),
        content: true,
    },
    Row {
        control_type: UIA_ComboBoxControlTypeId,
        localized: "combo box",
        patterns: P.or(Patterns::EXPAND).or(Patterns::VALUE),
        content: true,
    },
    Row {
        control_type: UIA_ListControlTypeId,
        localized: "list",
        patterns: P.or(Patterns::SELECTION).or(Patterns::SCROLL_ITEM),
        content: true,
    },
    Row {
        control_type: UIA_MenuControlTypeId,
        localized: "menu",
        patterns: P,
        content: true,
    },
    Row {
        control_type: UIA_ProgressBarControlTypeId,
        localized: "progress bar",
        patterns: P.or(Patterns::RANGE).or(Patterns::VALUE),
        content: true,
    },
    // A graph has no control type of its own. It is a custom control that reports a value,
    // which is what makes a presented analyzer readable at all.
    Row {
        control_type: UIA_CustomControlTypeId,
        localized: "graph",
        patterns: P.or(Patterns::VALUE).or(Patterns::RANGE),
        content: true,
    },
];

/// The row for `role`.
#[must_use]
pub fn row(role: UiaRole) -> &'static Row {
    &ROWS[index(role)]
}

/// A menu's rows are menu items, not buttons, and a list's are list items — the container
/// decides, because the same widget is authored either way.
#[must_use]
pub fn control_type_in(role: UiaRole, parent: UiaRole) -> i32 {
    match (parent, role) {
        (UiaRole::Menu, UiaRole::Button) => UIA_MenuItemControlTypeId,
        (UiaRole::List, UiaRole::Button) => UIA_ListItemControlTypeId,
        _ => row(role).control_type,
    }
}

/// A popup announces itself as a dialog, so a reader reads its title before its content.
pub const DIALOG_CONTROL_TYPE: i32 = UIA_WindowControlTypeId;

/// The pattern id a mask bit stands for, for `GetPatternProvider`.
#[must_use]
pub fn pattern_of(id: i32) -> Patterns {
    match id {
        _ if id == UIA_InvokePatternId => Patterns::INVOKE,
        _ if id == UIA_TogglePatternId => Patterns::TOGGLE,
        _ if id == UIA_ValuePatternId => Patterns::VALUE,
        _ if id == UIA_RangeValuePatternId => Patterns::RANGE,
        _ if id == UIA_SelectionPatternId => Patterns::SELECTION,
        _ if id == UIA_SelectionItemPatternId => Patterns::SELECTION_ITEM,
        _ if id == UIA_ExpandCollapsePatternId => Patterns::EXPAND,
        _ if id == UIA_ScrollItemPatternId => Patterns::SCROLL_ITEM,
        _ if id == UIA_TextPatternId => Patterns::TEXT,
        _ => Patterns::NONE,
    }
}

const fn index(role: UiaRole) -> usize {
    match role {
        UiaRole::None => 0,
        UiaRole::Text => 1,
        UiaRole::Group => 2,
        UiaRole::Button => 3,
        UiaRole::CheckBox => 4,
        UiaRole::RadioButton => 5,
        UiaRole::Slider => 6,
        UiaRole::Edit => 7,
        UiaRole::ComboBox => 8,
        UiaRole::List => 9,
        UiaRole::Menu => 10,
        UiaRole::ProgressBar => 11,
        UiaRole::Graph => 12,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is indexed by the enum, so a role added without a row would read whatever
    /// sits at that position. This is the assertion that says so.
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
        ];
        assert_eq!(all.len(), ROWS.len(), "a role was added without a row");
        for (at, role) in all.into_iter().enumerate() {
            assert_eq!(index(role), at, "{role:?} indexes the wrong row");
        }
        // Every published role names its type out loud. Only `None`, which is never
        // published, is allowed to be silent.
        for role in all.into_iter().skip(1) {
            assert!(!row(role).localized.is_empty(), "{role:?} is unnamed");
        }
    }

    #[test]
    fn a_menu_row_is_a_menu_item_and_a_loose_button_is_a_button() {
        assert_eq!(
            control_type_in(UiaRole::Button, UiaRole::Menu),
            UIA_MenuItemControlTypeId
        );
        assert_eq!(
            control_type_in(UiaRole::Button, UiaRole::Group),
            UIA_ButtonControlTypeId
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
