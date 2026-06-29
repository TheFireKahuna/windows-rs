use super::command_bar::CommandBarCommandDef;
use super::menu_bar::MenuItemDef;
use super::*;

/// Visual style for a [`Button`]. Not a WinRT enum — maps to resource key strings.
#[repr(transparent)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct ButtonStyle(pub i32);
#[expect(non_upper_case_globals)]
impl ButtonStyle {
    /// Standard button (framework default).
    pub const Default: Self = Self(0);
    /// Accent-colored button (primary action).
    pub const Accent: Self = Self(1);
    /// Chromeless subtle button (secondary action).
    pub const Subtle: Self = Self(2);
    /// Borderless text-link style (inline hyperlink pattern).
    pub const TextLink: Self = Self(3);
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Button {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub content: String,
    /// Optional rich element content, mounted via the child reconciler into the
    /// button's `ContentControl` slot. When set, `content` (text) is ignored.
    /// The element is `None` for the common text-button case, so this stays a
    /// zero-cost addition for existing call sites.
    pub content_element: Option<Box<Element>>,
    pub is_enabled: bool,
    pub style: ButtonStyle,
    pub icon: Option<Symbol>,
    pub on_click: Option<Callback<()>>,
    pub flyout: Option<FlyoutDef>,
    pub menu_flyout_items: Option<Vec<MenuItemDef>>,
    pub on_item_clicked: Option<Callback<String>>,
    pub command_bar_flyout_primary: Option<Vec<CommandBarCommandDef>>,
    pub command_bar_flyout_secondary: Option<Vec<CommandBarCommandDef>>,
    pub on_command_bar_flyout_click: Option<Callback<String>>,
}
impl Button {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_enabled: true,
            ..Default::default()
        }
    }
}

impl Widget for Button {
    widget_header!(ControlKind::Button);
    fn children(&self) -> Children<'_> {
        match &self.content_element {
            Some(el) => Children::PositionalSingle(el),
            None => Children::None,
        }
    }
    fn bindings(&self) -> PropBindings {
        let mut out = generated::button_bindings(self);
        // A rich element child (mounted by the child reconciler into the
        // ContentControl slot) owns the Content; binding the text Content too
        // would clobber it on the next prop-apply. Bind text only without a child.
        if self.content_element.is_none() {
            out.push(Binding::Prop(
                Prop::Content,
                PropValue::Str(self.content.clone()),
            ));
        }
        if let Some(v) = self.icon {
            out.push(Binding::Prop(Prop::Icon, PropValue::I32(v.0)));
        }
        if let Some(v) = &self.menu_flyout_items {
            out.push(Binding::Prop(
                Prop::MenuFlyoutItems,
                PropValue::MenuFlyoutItems(v.clone()),
            ));
        }
        if self.style != ButtonStyle::Default {
            out.push(Binding::Prop(Prop::StyleVariant, PropValue::I32(self.style.0)));
        }
        // Flyout and CommandBarFlyout are compound types not in TOML.
        if let Some(ref fly) = self.flyout {
            out.push(Binding::Prop(
                Prop::FlyoutContent,
                PropValue::FlyoutDef(fly.clone()),
            ));
            if fly.placement != FlyoutPlacementMode::default() {
                out.push(Binding::Prop(
                    Prop::FlyoutPlacement,
                    PropValue::I32(fly.placement.0),
                ));
            }
        }
        if let Some(ref primary) = self.command_bar_flyout_primary {
            out.push(Binding::Prop(
                Prop::CommandBarFlyoutCommands,
                PropValue::CommandBarFlyoutDef {
                    primary: primary.clone(),
                    secondary: self
                        .command_bar_flyout_secondary
                        .clone()
                        .unwrap_or_default(),
                },
            ));
        }
        out
    }
}

impl Button {
    pub fn on_click(mut self, f: impl IntoUnitCallback) -> Self {
        self.on_click = Some(f.into_unit_callback());
        self
    }

    /// Host a rich element tree as the button's content instead of the `content`
    /// text. The button stays a real WinUI `Button`, so it keeps its
    /// `InvokePattern` automation peer, keyboard activation (Space/Enter), and
    /// accessibility — while rendering arbitrary visuals. When set, the text
    /// `content` is ignored. Pair with [`Button::subtle`] (or a chromeless style)
    /// plus zeroed `padding`/`min_width`/`min_height` to use the button as a
    /// tappable, automatable wrapper around custom-drawn or composed visuals.
    pub fn content_element(mut self, element: impl Into<Element>) -> Self {
        self.content_element = Some(Box::new(element.into()));
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.is_enabled = enabled;
        self
    }

    pub fn accent(mut self) -> Self {
        self.style = ButtonStyle::Accent;
        self
    }

    pub fn subtle(mut self) -> Self {
        self.style = ButtonStyle::Subtle;
        self
    }

    pub fn text_link(mut self) -> Self {
        self.style = ButtonStyle::TextLink;
        self
    }

    pub fn icon(mut self, sym: Symbol) -> Self {
        self.icon = Some(sym);
        self
    }

    pub fn flyout(mut self, text: impl Into<String>) -> Self {
        self.flyout = Some(FlyoutDef::text(text));
        self
    }

    pub fn flyout_with_placement(
        mut self,
        text: impl Into<String>,
        placement: FlyoutPlacementMode,
    ) -> Self {
        self.flyout = Some(FlyoutDef::text(text).placement(placement));
        self
    }

    /// Attach a rich element-tree flyout (band panel, color picker, …).
    pub fn flyout_element(mut self, element: impl Into<Element>) -> Self {
        self.flyout = Some(FlyoutDef::rich(element));
        self
    }

    /// Attach a fully-specified [`FlyoutDef`] (text/rich content + placement +
    /// open + on_closed).
    pub fn flyout_def(mut self, def: FlyoutDef) -> Self {
        self.flyout = Some(def);
        self
    }

    pub fn menu_flyout(mut self, items: Vec<MenuItemDef>) -> Self {
        self.menu_flyout_items = Some(items);
        self
    }

    pub fn on_item_clicked(mut self, f: impl IntoCallback<String>) -> Self {
        self.on_item_clicked = Some(f.into_callback());
        self
    }

    pub fn command_bar_flyout(mut self, primary: Vec<CommandBarCommandDef>) -> Self {
        self.command_bar_flyout_primary = Some(primary);
        self
    }

    pub fn command_bar_flyout_secondary(mut self, secondary: Vec<CommandBarCommandDef>) -> Self {
        self.command_bar_flyout_secondary = Some(secondary);
        self
    }

    pub fn on_command_bar_flyout_click(mut self, f: impl IntoCallback<String>) -> Self {
        self.on_command_bar_flyout_click = Some(f.into_callback());
        self
    }
}

pub fn button(content: impl Into<String>) -> Button {
    Button::new(content)
}
