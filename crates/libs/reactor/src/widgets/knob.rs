use super::*;

/// 7 o'clock — the default sweep start (`0.75·π`), a 270° sweep to `2.25·π`.
pub const KNOB_DEFAULT_START: f64 = std::f64::consts::PI * 0.75;
/// 5 o'clock — the default sweep end (`2.25·π`).
pub const KNOB_DEFAULT_END: f64 = std::f64::consts::PI * 2.25;

/// A rotary gauge: a background track ring, a gradient value arc, tick marks +
/// numeric labels, a needle, and a center readout. The needle and value arc
/// ease toward the committed value on the system compositor (a retargeted
/// spring drives both through one expression — no app frame while it settles);
/// the ring, ticks, labels, and readout paint only on a value commit or resize.
///
/// The readout text, tick labels, and per-value accent color are supplied by
/// the app (this widget holds no formatting logic), so a domain control formats
/// them however it likes and passes plain strings/colors each render.
#[derive(Clone, Debug, PartialEq)]
pub struct Knob {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub value: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub step: Option<f64>,
    pub start_angle: f64,
    pub end_angle: f64,
    /// Value-arc gradient stops `(position 0..1 across the sweep, color)`.
    pub stops: Vec<(f64, Color)>,
    /// Tick-mark positions (value units).
    pub ticks: Vec<f64>,
    /// `(value, label)` scale labels (formatted by the app).
    pub tick_labels: Vec<(f64, String)>,
    /// Ticks whose value is an exact multiple draw longer/brighter.
    pub major_every: Option<f64>,
    /// Per-value accent for the arc / needle glow (`None` = theme accent).
    pub accent: Option<Color>,
    /// Center readout (large), formatted by the app.
    pub text: Option<String>,
    /// Small unit line under the readout (e.g. `"dB"`).
    pub unit: Option<String>,
    /// Optional sub-line under the unit (e.g. a linear multiplier).
    pub sub_text: Option<String>,
    pub is_enabled: bool,
    pub on_value_changed: Option<Callback<f64>>,
    /// `true` on pointer-drag capture, `false` on release.
    pub on_drag_changed: Option<Callback<bool>>,
}

impl Default for Knob {
    fn default() -> Self {
        Self {
            key: None,
            modifiers: Modifiers::default(),
            value: 0.0,
            minimum: 0.0,
            maximum: 1.0,
            step: None,
            start_angle: KNOB_DEFAULT_START,
            end_angle: KNOB_DEFAULT_END,
            stops: Vec::new(),
            ticks: Vec::new(),
            tick_labels: Vec::new(),
            major_every: None,
            accent: None,
            text: None,
            unit: None,
            sub_text: None,
            is_enabled: true,
            on_value_changed: None,
            on_drag_changed: None,
        }
    }
}

impl Knob {
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
    pub fn step(mut self, v: f64) -> Self {
        self.step = Some(v);
        self
    }
    /// Override the sweep (radians; canvas convention, 0 = east, clockwise).
    pub fn sweep(mut self, start: f64, end: f64) -> Self {
        self.start_angle = start;
        self.end_angle = end;
        self
    }
    /// The value-arc gradient `(position 0..1, color)` stops.
    pub fn stops(mut self, stops: Vec<(f64, Color)>) -> Self {
        self.stops = stops;
        self
    }
    pub fn ticks(mut self, ticks: Vec<f64>) -> Self {
        self.ticks = ticks;
        self
    }
    pub fn tick_labels(mut self, labels: Vec<(f64, String)>) -> Self {
        self.tick_labels = labels;
        self
    }
    pub fn major_every(mut self, m: f64) -> Self {
        self.major_every = Some(m);
        self
    }
    pub fn accent(mut self, c: Color) -> Self {
        self.accent = Some(c);
        self
    }
    /// The large center readout (app-formatted).
    pub fn text(mut self, s: impl Into<String>) -> Self {
        self.text = Some(s.into());
        self
    }
    pub fn unit(mut self, s: impl Into<String>) -> Self {
        self.unit = Some(s.into());
        self
    }
    pub fn sub_text(mut self, s: impl Into<String>) -> Self {
        self.sub_text = Some(s.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.is_enabled = enabled;
        self
    }
    pub fn on_value_changed(mut self, f: impl IntoCallback<f64>) -> Self {
        self.on_value_changed = Some(f.into_callback());
        self
    }
    pub fn on_drag_changed(mut self, f: impl IntoCallback<bool>) -> Self {
        self.on_drag_changed = Some(f.into_callback());
        self
    }
}

impl Widget for Knob {
    widget_header!(ControlKind::Knob);
    fn bindings(&self) -> PropBindings {
        let mut out = vec![
            Binding::Event(
                Event::ValueChanged,
                self.on_value_changed
                    .as_ref()
                    .map(|cb| EventHandler::F64(cb.clone())),
            ),
            Binding::Event(
                Event::DragStateChanged,
                self.on_drag_changed
                    .as_ref()
                    .map(|cb| EventHandler::Bool(cb.clone())),
            ),
            Binding::Prop(Prop::IsEnabled, PropValue::Bool(self.is_enabled)),
            Binding::Prop(Prop::Value, PropValue::F64(self.value)),
            Binding::Prop(Prop::Minimum, PropValue::F64(self.minimum)),
            Binding::Prop(Prop::Maximum, PropValue::F64(self.maximum)),
            Binding::Prop(Prop::StartAngle, PropValue::F64(self.start_angle)),
            Binding::Prop(Prop::EndAngle, PropValue::F64(self.end_angle)),
        ];
        if let Some(v) = self.step {
            out.push(Binding::Prop(Prop::Step, PropValue::F64(v)));
        }
        if !self.stops.is_empty() {
            out.push(Binding::Prop(
                Prop::GradientStops,
                PropValue::GradientStops(self.stops.clone()),
            ));
        }
        if !self.ticks.is_empty() {
            out.push(Binding::Prop(
                Prop::Ticks,
                PropValue::F64List(self.ticks.clone()),
            ));
        }
        if !self.tick_labels.is_empty() {
            out.push(Binding::Prop(
                Prop::TickLabels,
                PropValue::ValueLabels(self.tick_labels.clone()),
            ));
        }
        if let Some(m) = self.major_every {
            out.push(Binding::Prop(Prop::MajorEvery, PropValue::F64(m)));
        }
        if let Some(c) = self.accent {
            out.push(Binding::Prop(Prop::Accent, PropValue::Color(c)));
        }
        if let Some(t) = &self.text {
            out.push(Binding::Prop(Prop::Text, PropValue::Str(t.clone())));
        }
        if let Some(u) = &self.unit {
            out.push(Binding::Prop(Prop::Unit, PropValue::Str(u.clone())));
        }
        if let Some(s) = &self.sub_text {
            out.push(Binding::Prop(Prop::SubText, PropValue::Str(s.clone())));
        }
        out
    }
}
