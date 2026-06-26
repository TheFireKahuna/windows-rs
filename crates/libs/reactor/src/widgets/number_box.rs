use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct NumberBox {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub value: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub on_value_changed: Option<Callback<f64>>,
    pub header: Option<String>,
    pub is_enabled: bool,
    /// Increment applied by the spin buttons / arrow keys (`SmallChange`).
    pub small_change: Option<f64>,
    /// Increment applied by PageUp/PageDown (`LargeChange`).
    pub large_change: Option<f64>,
    /// Number of fraction digits shown (drives a `DecimalFormatter`).
    pub precision: Option<i32>,
    /// Horizontal alignment of the entered text within the box.
    pub text_alignment: Option<HorizontalAlignment>,
}
impl Default for NumberBox {
    fn default() -> Self {
        Self {
            key: None,
            modifiers: Modifiers::default(),
            value: 0.0,
            minimum: f64::MIN,
            maximum: f64::MAX,
            on_value_changed: None,
            header: None,
            is_enabled: true,
            small_change: None,
            large_change: None,
            precision: None,
            text_alignment: None,
        }
    }
}
impl NumberBox {
    pub fn new(value: f64) -> Self {
        Self {
            value,
            ..Default::default()
        }
    }
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.minimum = min;
        self.maximum = max;
        self
    }
    pub fn on_value_changed(mut self, f: impl IntoCallback<f64>) -> Self {
        self.on_value_changed = Some(f.into_callback());
        self
    }
    pub fn header(mut self, s: impl Into<String>) -> Self {
        self.header = Some(s.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.is_enabled = enabled;
        self
    }

    /// Spin-button / arrow-key increment (`SmallChange`).
    pub fn step(mut self, step: f64) -> Self {
        self.small_change = Some(step);
        self
    }

    /// PageUp / PageDown increment (`LargeChange`).
    pub fn large_change(mut self, step: f64) -> Self {
        self.large_change = Some(step);
        self
    }

    /// Number of fraction digits to display (e.g. `1` → `3.0`, `2` → `3.00`).
    /// Installs a `DecimalFormatter` with that many fraction digits.
    pub fn precision(mut self, fraction_digits: i32) -> Self {
        self.precision = Some(fraction_digits);
        self
    }

    /// Horizontal alignment of the entered text inside the box.
    pub fn text_alignment(mut self, align: HorizontalAlignment) -> Self {
        self.text_alignment = Some(align);
        self
    }
}

impl Widget for NumberBox {
    widget_header!(ControlKind::NumberBox);
    fn bindings(&self) -> PropBindings {
        // Base props come from the generated table; the fork's step / precision /
        // alignment extras are appended here (not in the generated file) so they
        // survive a `tool_reactor` regen.
        let mut out = generated::number_box_bindings(self);
        if let Some(v) = self.small_change {
            out.push(Binding::Prop(Prop::Step, PropValue::F64(v)));
        }
        if let Some(v) = self.large_change {
            out.push(Binding::Prop(Prop::LargeChange, PropValue::F64(v)));
        }
        if let Some(v) = self.precision {
            out.push(Binding::Prop(Prop::Precision, PropValue::I32(v)));
        }
        if let Some(v) = self.text_alignment {
            out.push(Binding::Prop(
                Prop::HorizontalContentAlignment,
                PropValue::I32(v.0),
            ));
        }
        out
    }
}
