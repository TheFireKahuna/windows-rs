use super::*;

/// Definition of a single item in a [`SelectorBar`].
#[derive(Clone, Debug, PartialEq)]
pub struct SelectorBarItemDef {
    /// Display text.
    pub text: String,
    /// Optional symbol icon.
    pub icon: Option<Symbol>,
}

impl SelectorBarItemDef {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            icon: None,
        }
    }

    pub fn icon(mut self, icon: Symbol) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// Builder for a [`SelectorBarItemDef`].
pub fn selector_bar_item(text: impl Into<String>) -> SelectorBarItemDef {
    SelectorBarItemDef::new(text)
}

/// `Microsoft.UI.Xaml.Controls.SelectorBar`. A horizontal segmented selector.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct SelectorBar {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub items: Vec<SelectorBarItemDef>,
    /// Controlled selection. `None` leaves the control uncontrolled (the
    /// backend keeps its own selection, defaulting to the first item).
    pub selected_index: Option<i32>,
    /// Accent variant: a filled accent pill in a fully-rounded tray, instead of
    /// the default subtle grey-fill segments.
    pub accent: bool,
    pub disabled: bool,
    pub on_selection_changed: Option<Callback<String>>,
}

impl SelectorBar {
    pub fn new(items: Vec<SelectorBarItemDef>) -> Self {
        Self {
            items,
            ..Default::default()
        }
    }

    /// Set the controlled selected index.
    pub fn selected_index(mut self, i: i32) -> Self {
        self.selected_index = Some(i);
        self
    }

    /// Use the accent-pill visual variant.
    pub fn accent(mut self) -> Self {
        self.accent = true;
        self
    }

    /// Dim and disable interaction.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_selection_changed(mut self, f: impl IntoCallback<String>) -> Self {
        self.on_selection_changed = Some(f.into_callback());
        self
    }
}

impl Widget for SelectorBar {
    widget_header!(ControlKind::SelectorBar);
    fn bindings(&self) -> PropBindings {
        let mut out = generated::selector_bar_bindings(self);
        out.push(Binding::Prop(
            Prop::Items,
            PropValue::SelectorBarItems(self.items.clone()),
        ));
        if let Some(i) = self.selected_index {
            out.push(Binding::Prop(Prop::SelectedIndex, PropValue::I32(i)));
        }
        out.push(Binding::Prop(
            Prop::StyleVariant,
            PropValue::I32(if self.accent { 1 } else { 0 }),
        ));
        out.push(Binding::Prop(Prop::IsEnabled, PropValue::Bool(!self.disabled)));
        out
    }
}

pub fn selector_bar(items: Vec<SelectorBarItemDef>) -> SelectorBar {
    SelectorBar::new(items)
}
