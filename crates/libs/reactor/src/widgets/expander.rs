use super::*;

/// Content for an [`Expander`] header — either plain text or an arbitrary
/// element tree.
#[derive(Clone, Debug, PartialEq)]
pub enum ExpanderHeader {
    Text(String),
    Element(Box<Element>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expander {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub header: Option<ExpanderHeader>,
    pub child: Box<Element>,
    pub is_expanded: bool,
    pub on_expanding: Option<Callback<bool>>,
    /// Outline drawn around the header strip, overriding the theme's. Carried as
    /// the widget's own field for the reason [`Button`]'s is: `background` is a
    /// universal [`ElementExt`] modifier but an outline is only meaningful on the
    /// kinds that draw one, so each states it.
    ///
    /// A fully transparent colour is how a host that draws its own boundary — a
    /// card enclosing the expander — suppresses the strip's, rather than living
    /// with two rules a hairline apart.
    pub border_brush: Option<Color>,
}
impl Default for Expander {
    fn default() -> Self {
        Self {
            key: None,
            modifiers: Modifiers::default(),
            header: None,
            child: Box::new(Element::Empty),
            is_expanded: false,
            on_expanding: None,
            border_brush: None,
        }
    }
}
impl Expander {
    pub fn new(child: impl Into<Element>) -> Self {
        Self {
            child: Box::new(child.into()),
            ..Default::default()
        }
    }
    /// Set a plain-text header.
    pub fn header(mut self, s: impl Into<String>) -> Self {
        self.header = Some(ExpanderHeader::Text(s.into()));
        self
    }
    /// Set an element tree as the header content (complex headers).
    pub fn header_content(mut self, el: impl Into<Element>) -> Self {
        self.header = Some(ExpanderHeader::Element(Box::new(el.into())));
        self
    }
    pub fn expanded(mut self, v: bool) -> Self {
        self.is_expanded = v;
        self
    }
    pub fn on_expanding(mut self, f: impl IntoCallback<bool>) -> Self {
        self.on_expanding = Some(f.into_callback());
        self
    }
    /// Outline the header strip in `c` instead of leaving it to the theme.
    pub fn border_brush(mut self, c: Color) -> Self {
        self.border_brush = Some(c);
        self
    }
}

impl Widget for Expander {
    widget_header!(ControlKind::Expander);
    fn bindings(&self) -> PropBindings {
        let mut out = generated::expander_bindings(self);
        // Header text is a compound type not expressible in TOML.
        if let Some(ExpanderHeader::Text(s)) = &self.header {
            out.push(Binding::Prop(Prop::Header, PropValue::Str(s.clone())));
        }
        if let Some(c) = self.border_brush {
            out.push(Binding::Prop(Prop::BorderBrush, PropValue::Color(c)));
        }
        out
    }
    fn children(&self) -> Children<'_> {
        Children::PositionalSingle(&self.child)
    }
    fn header_element(&self) -> Option<&Element> {
        if let Some(ExpanderHeader::Element(el)) = &self.header {
            Some(el)
        } else {
            None
        }
    }
}
