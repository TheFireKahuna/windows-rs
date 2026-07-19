use super::*;

/// A display-only horizontal level meter: a groove revealed left-to-right to
/// the current value by a gradient fill, with a position needle riding the
/// fill edge and an optional reference marker hairline (e.g. 0 dBFS).
///
/// The fill/needle motion runs entirely on the system compositor — a value
/// change retargets a compositor spring; the app never ticks while it settles.
/// Colors are authored linear scRGB and display-mapped by the backend at its
/// draw choke like all native chrome.
#[derive(Clone, Debug, PartialEq)]
pub struct Meter {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    /// Current level in `[minimum, maximum]`.
    pub value: f64,
    pub minimum: f64,
    pub maximum: f64,
    /// Reference marker position in value units (`None` = no marker).
    pub marker: Option<f64>,
    /// Marker hairline color (`None` = a neutral tick). Alpha is honored.
    pub marker_color: Option<Color>,
    /// Fill gradient stops `(position 0..1 across the full range, color)`.
    /// Empty = a solid theme-accent fill.
    pub stops: Vec<(f64, Color)>,
}

impl Default for Meter {
    fn default() -> Self {
        Self {
            key: None,
            modifiers: Modifiers::default(),
            value: 0.0,
            minimum: 0.0,
            maximum: 1.0,
            marker: None,
            marker_color: None,
            stops: Vec::new(),
        }
    }
}

impl Meter {
    /// A meter at `value` over the default `0..1` range.
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
    /// Place a reference marker hairline at `v` (value units).
    pub fn marker(mut self, v: f64) -> Self {
        self.marker = Some(v);
        self
    }
    /// Tint the marker hairline (authored linear scRGB; alpha honored).
    pub fn marker_color(mut self, c: Color) -> Self {
        self.marker_color = Some(c);
        self
    }
    /// The fill gradient across the full range: `(position 0..1, color)` stops.
    pub fn stops(mut self, stops: Vec<(f64, Color)>) -> Self {
        self.stops = stops;
        self
    }
}

impl Widget for Meter {
    widget_header!(ControlKind::Meter);
    fn bindings(&self) -> PropBindings {
        let mut out = vec![
            Binding::Prop(Prop::Value, PropValue::F64(self.value)),
            Binding::Prop(Prop::Minimum, PropValue::F64(self.minimum)),
            Binding::Prop(Prop::Maximum, PropValue::F64(self.maximum)),
        ];
        if let Some(v) = self.marker {
            out.push(Binding::Prop(Prop::Marker, PropValue::F64(v)));
        }
        if let Some(c) = self.marker_color {
            out.push(Binding::Prop(Prop::MarkerColor, PropValue::Color(c)));
        }
        if !self.stops.is_empty() {
            out.push(Binding::Prop(
                Prop::GradientStops,
                PropValue::GradientStops(self.stops.clone()),
            ));
        }
        out
    }
}
