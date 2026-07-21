use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Slider {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub value: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub step: Option<f64>,
    pub on_value_changed: Option<Callback<f64>>,
    pub header: Option<String>,
    pub orientation: Orientation,
    pub is_enabled: bool,
    /// Fill origin in value units (`None` = fill from `minimum`). An origin
    /// strictly inside the range gives a bidirectional gain-style fill that
    /// grows out from the origin, marked by a neutral tick notch on the track.
    pub fill_origin: Option<f64>,
    /// Fill color when the value sits at or below the fill origin (`None` =
    /// the theme accent). Authored linear scRGB — the backend display-maps it
    /// at the draw choke like every other chrome color.
    pub fill_color: Option<Color>,
    /// Fill color when the value sits above the fill origin (`None` = same as
    /// [`fill_color`](Self::fill_color)) — the two-tone cut/boost split.
    pub fill_color_alt: Option<Color>,
    /// Fires `true` when a pointer drag captures the slider and `false` on
    /// release — for hosts that highlight *what is being touched* beyond the
    /// per-value `on_value_changed` stream (which cannot see the release).
    pub on_drag_changed: Option<Callback<bool>>,
    /// The value's unit ("dB", "ms", "Hz"), announced after the value by an
    /// assistive client. Not drawn — the slider has no read-out of its own; this
    /// is the dimension a listener cannot infer from the number alone.
    pub unit: Option<String>,
}
impl Default for Slider {
    fn default() -> Self {
        Self {
            key: None,
            modifiers: Modifiers::default(),
            value: 0.0,
            minimum: 0.0,
            maximum: 100.0,
            step: None,
            on_value_changed: None,
            header: None,
            orientation: Orientation::Horizontal,
            is_enabled: true,
            fill_origin: None,
            fill_color: None,
            fill_color_alt: None,
            on_drag_changed: None,
            unit: None,
        }
    }
}
impl Slider {
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
    pub fn on_value_changed(mut self, f: impl IntoCallback<f64>) -> Self {
        self.on_value_changed = Some(f.into_callback());
        self
    }
    pub fn header(mut self, s: impl Into<String>) -> Self {
        self.header = Some(s.into());
        self
    }
    /// Switch to a vertical slider (`ISlider::Orientation = Vertical`).
    pub fn vertical(mut self) -> Self {
        self.orientation = Orientation::Vertical;
        self
    }
    /// Switch to a horizontal slider (default).
    pub fn horizontal(mut self) -> Self {
        self.orientation = Orientation::Horizontal;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.is_enabled = enabled;
        self
    }
    /// Fill from `origin` (value units) instead of from `minimum` — the
    /// bidirectional gain-style fill (e.g. `0.0` on a ±dB slider).
    pub fn fill_from(mut self, origin: f64) -> Self {
        self.fill_origin = Some(origin);
        self
    }
    /// Tint the value fill (authored linear scRGB; display-mapped by the
    /// backend). One color for both sides of the fill origin.
    pub fn fill_color(mut self, c: Color) -> Self {
        self.fill_color = Some(c);
        self
    }
    /// Two-tone fill: `below` when the value sits at or below the fill origin,
    /// `above` when it sits above (e.g. cut vs boost on a gain slider).
    pub fn fill_colors(mut self, below: Color, above: Color) -> Self {
        self.fill_color = Some(below);
        self.fill_color_alt = Some(above);
        self
    }
    /// Observe pointer-drag capture/release (`true` on press, `false` on
    /// release), beyond the per-value `on_value_changed` stream.
    pub fn on_drag_changed(mut self, f: impl IntoCallback<bool>) -> Self {
        self.on_drag_changed = Some(f.into_callback());
        self
    }
    /// The value's unit ("dB", "ms"), announced after the value by an assistive
    /// client. Purely an accessibility affordance: nothing draws it.
    pub fn unit(mut self, s: impl Into<String>) -> Self {
        self.unit = Some(s.into());
        self
    }
}

impl Widget for Slider {
    widget_header!(ControlKind::Slider);
    fn bindings(&self) -> PropBindings {
        // Non-generated extras appended here so they survive a `tool_reactor`
        // regen (same pattern as NumberBox's step/large-change).
        let mut out = generated::slider_bindings(self);
        if let Some(v) = self.step {
            out.push(Binding::Prop(Prop::Step, PropValue::F64(v)));
        }
        if let Some(v) = self.fill_origin {
            out.push(Binding::Prop(Prop::FillOrigin, PropValue::F64(v)));
        }
        if let Some(c) = self.fill_color {
            out.push(Binding::Prop(Prop::FillColor, PropValue::Color(c)));
        }
        if let Some(c) = self.fill_color_alt {
            out.push(Binding::Prop(Prop::FillColorAlt, PropValue::Color(c)));
        }
        if let Some(u) = &self.unit {
            out.push(Binding::Prop(Prop::Unit, PropValue::Str(u.clone())));
        }
        out.push(Binding::Event(
            Event::DragStateChanged,
            self.on_drag_changed
                .as_ref()
                .map(|cb| EventHandler::Bool(cb.clone())),
        ));
        out
    }
}
