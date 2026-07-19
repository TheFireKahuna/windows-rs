use super::*;

#[derive(Clone, Default, Debug, PartialEq)]
pub struct InfoBadge {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub value: Option<i32>,
    /// Count weight override; `None` leaves the badge at the weight its kind
    /// is born with. `Modifiers` carries no weight (only family and size), so
    /// this is the one text axis a badge has to expose itself.
    pub font_weight: Option<u16>,
}
impl InfoBadge {
    pub fn dot() -> Self {
        Self::default()
    }
    pub fn numeric(v: i32) -> Self {
        Self {
            value: Some(v),
            ..Default::default()
        }
    }
    pub fn font_weight(mut self, w: u16) -> Self {
        self.font_weight = Some(w);
        self
    }
    /// Shortcut for `.font_weight(700)`, matching `TextBlock::bold`.
    pub fn bold(self) -> Self {
        self.font_weight(700)
    }
}

impl Widget for InfoBadge {
    widget_header!(ControlKind::InfoBadge);
    fn bindings(&self) -> PropBindings {
        let mut out = generated::info_badge_bindings(self);
        if let Some(v) = self.value {
            out.push(Binding::Prop(Prop::Value, PropValue::I32(v)));
        }
        if let Some(w) = self.font_weight {
            out.push(Binding::Prop(Prop::FontWeight, PropValue::U16(w)));
        }
        out
    }
}
