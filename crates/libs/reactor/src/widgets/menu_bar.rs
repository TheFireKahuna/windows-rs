use super::*;

/// Definition of a single item within a menu (used by both [`MenuBar`]
/// and the menu flyout modifier on buttons).
#[derive(Clone, Debug, PartialEq)]
pub enum MenuItemDef {
    /// A clickable menu item with a text label, plus optional per-item
    /// decoration (leading glyph icon, destructive styling, disabled state,
    /// trailing keyboard-shortcut text).
    Item {
        text: String,
        /// Optional leading glyph (a `Symbol`), rendered as the item's icon.
        icon: Option<Symbol>,
        /// Destructive / dangerous action — rendered with error-red foreground.
        danger: bool,
        /// When `false`, the item is greyed out and not clickable.
        enabled: bool,
        /// Trailing accelerator hint text (e.g. `"Ctrl+Z"`). Display-only —
        /// this does NOT register a live keyboard accelerator, it overrides the
        /// shortcut text shown on the right of the item.
        shortcut: Option<String>,
    },
    /// A visual separator line.
    Separator,
    /// A submenu containing nested items.
    SubItem {
        text: String,
        children: Vec<Self>,
    },
}

impl MenuItemDef {
    /// Set a leading glyph icon on an [`MenuItemDef::Item`] (no-op on other
    /// variants).
    pub fn icon(mut self, sym: Symbol) -> Self {
        if let Self::Item { icon, .. } = &mut self {
            *icon = Some(sym);
        }
        self
    }

    /// Mark an [`MenuItemDef::Item`] as a destructive action (error-red text).
    pub fn danger(mut self) -> Self {
        if let Self::Item { danger, .. } = &mut self {
            *danger = true;
        }
        self
    }

    /// Disable an [`MenuItemDef::Item`] (greyed out, not clickable).
    pub fn disabled(mut self) -> Self {
        if let Self::Item { enabled, .. } = &mut self {
            *enabled = false;
        }
        self
    }

    /// Set the trailing keyboard-shortcut hint text on an
    /// [`MenuItemDef::Item`] (display-only).
    pub fn shortcut(mut self, text: impl Into<String>) -> Self {
        if let Self::Item { shortcut, .. } = &mut self {
            *shortcut = Some(text.into());
        }
        self
    }
}

/// Builder for a [`MenuItemDef::Item`]. Decorate with the chainable
/// [`icon`](MenuItemDef::icon) / [`danger`](MenuItemDef::danger) /
/// [`disabled`](MenuItemDef::disabled) / [`shortcut`](MenuItemDef::shortcut)
/// methods.
pub fn menu_item(text: impl Into<String>) -> MenuItemDef {
    MenuItemDef::Item {
        text: text.into(),
        icon: None,
        danger: false,
        enabled: true,
        shortcut: None,
    }
}

/// Builder for a [`MenuItemDef::Separator`].
pub fn menu_separator() -> MenuItemDef {
    MenuItemDef::Separator
}

/// Builder for a [`MenuItemDef::SubItem`].
pub fn menu_sub_item(text: impl Into<String>, children: Vec<MenuItemDef>) -> MenuItemDef {
    MenuItemDef::SubItem {
        text: text.into(),
        children,
    }
}

/// Definition of a top-level menu in a [`MenuBar`].
#[derive(Clone, Debug, PartialEq)]
pub struct MenuBarItemDef {
    pub title: String,
    pub items: Vec<MenuItemDef>,
}

impl MenuBarItemDef {
    pub fn new(title: impl Into<String>, items: Vec<MenuItemDef>) -> Self {
        Self {
            title: title.into(),
            items,
        }
    }
}

/// Builder for a [`MenuBarItemDef`].
pub fn menu_bar_item(title: impl Into<String>, items: Vec<MenuItemDef>) -> MenuBarItemDef {
    MenuBarItemDef::new(title, items)
}

/// `Microsoft.UI.Xaml.Controls.MenuBar`. A horizontal bar of dropdown menus.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct MenuBar {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub items: Vec<MenuBarItemDef>,
    pub on_item_clicked: Option<Callback<String>>,
}

impl MenuBar {
    pub fn new(items: Vec<MenuBarItemDef>) -> Self {
        Self {
            items,
            ..Default::default()
        }
    }

    pub fn on_item_clicked<F: Fn(String) + 'static>(mut self, f: F) -> Self {
        self.on_item_clicked = Some(Callback::new(f));
        self
    }
}

impl Widget for MenuBar {
    widget_header!(ControlKind::MenuBar);
    fn bindings(&self) -> PropBindings {
        let mut out = generated::menu_bar_bindings(self);
        out.push(Binding::Prop(
            Prop::Items,
            PropValue::MenuBarItems(self.items.clone()),
        ));
        out
    }
}

pub fn menu_bar(items: Vec<MenuBarItemDef>) -> MenuBar {
    MenuBar::new(items)
}
