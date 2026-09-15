//! Stock recipes use the same retained writes as application components.
use super::{Chrome, Flow, Range, TextSource, UiaRole, Wash, roles};
use crate::build::{Element, Field, Path, Scalar, Ui};
use crate::layout::{Align, Len, Preset};
use crate::role::{Elevation, Fill, Metric, Role, Text, TypeRole};
use crate::signal::{Cell, Signal};
/// Text appearance chosen by an application recipe. Typography remains unresolved until layout.
#[derive(Copy, Clone, Debug)]
pub struct TextStyle {
    pub typography: TypeRole,
    /// Standalone ink. An enclosing control's chrome governs the label colour;
    /// `None` otherwise uses primary text.
    pub ink: Option<Role>,
    pub flow: Flow,
    pub caps: bool,
    pub vertical: bool,
}

impl TextStyle {
    pub const fn vertical(mut self, vertical: bool) -> Self {
        self.vertical = vertical;
        self
    }
    /// Overrides only the ink; typography remains a solve-time token.
    pub const fn ink(mut self, ink: Role) -> Self {
        self.ink = Some(ink);
        self
    }
    pub const fn flow(mut self, flow: Flow) -> Self {
        self.flow = flow;
        self
    }
    pub const fn caps(mut self, caps: bool) -> Self {
        self.caps = caps;
        self
    }
    /// A primary, single-line run with the supplied typography token.
    pub const fn new(typography: TypeRole) -> Self {
        Self {
            typography,
            ink: Some(Role::Text(Text::Primary)),
            flow: Flow::Line,
            caps: false,
            vertical: false,
        }
    }
}

/// Paint and origin of a slider's retained value stroke.
#[derive(Copy, Clone, Debug, Default)]
pub struct SliderStyle {
    /// Value the fill grows from; the range minimum when absent.
    pub origin: Option<f64>,
    /// A fixed gradient across the whole rail; trimmed rather than rescaled.
    pub ramp: Option<windows_scene::RampId>,
}

pub trait Choice<T> {
    fn selected(&self) -> T;
    fn choose(&self, value: T);
}
impl<T: Clone + PartialEq + 'static> Choice<T> for Cell<T> {
    fn selected(&self) -> T {
        self.get()
    }
    fn choose(&self, value: T) {
        self.set(value);
    }
}
impl<T, R: Fn() -> T, W: Fn(T)> Choice<T> for (R, W) {
    fn selected(&self) -> T {
        self.0()
    }
    fn choose(&self, value: T) {
        self.1(value);
    }
}

pub fn text<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a> {
    ui.text_owned(
        None,
        TextStyle::new(TypeRole::Body)
            .ink(Role::Text(Text::Primary))
            .flow(Flow::Line),
        text,
    )
}
pub fn title<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a> {
    ui.text_owned(
        None,
        TextStyle::new(TypeRole::Title)
            .ink(Role::Text(Text::Primary))
            .flow(Flow::Line),
        text,
    )
}
pub fn label<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a> {
    ui.text_owned(
        None,
        TextStyle::new(TypeRole::Label)
            .ink(Role::Text(Text::Secondary))
            .flow(Flow::Line),
        text,
    )
}
pub fn caption<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a> {
    ui.text_owned(
        None,
        TextStyle::new(TypeRole::Caption)
            .ink(Role::Text(Text::Tertiary))
            .flow(Flow::Wrap),
        text,
    )
}
pub fn note<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a> {
    ui.text_owned(
        None,
        TextStyle::new(TypeRole::Caption)
            .ink(Role::Text(Text::Tertiary))
            .flow(Flow::Line),
        text,
    )
}
pub fn micro<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a> {
    ui.text_owned(
        None,
        TextStyle::new(TypeRole::Micro)
            .ink(Role::Text(Text::Tertiary))
            .flow(Flow::Line),
        text,
    )
}
pub fn mono<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a> {
    ui.text_owned(
        None,
        TextStyle::new(TypeRole::Mono)
            .ink(Role::Text(Text::Primary))
            .flow(Flow::Line),
        text,
    )
}
pub fn code<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a> {
    ui.text_owned(
        None,
        TextStyle::new(TypeRole::Mono)
            .ink(Role::Text(Text::Primary))
            .flow(Flow::Wrap),
        text,
    )
}
pub fn display<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a> {
    ui.text_owned(
        None,
        TextStyle::new(TypeRole::Display)
            .ink(Role::Text(Text::Primary))
            .flow(Flow::Line),
        text,
    )
}

pub fn vertical_label<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a> {
    ui.text_owned(
        None,
        TextStyle::new(TypeRole::Label)
            .ink(Role::Text(Text::Secondary))
            .vertical(true),
        text,
    )
}
pub fn styled_text<'a>(
    ui: &'a mut Ui<'_>,
    text: impl Into<TextSource>,
    style: TextStyle,
) -> Element<'a> {
    ui.text_owned(None, style, text)
}
pub fn control_text<'a>(
    ui: &'a mut Ui<'_>,
    text: impl Into<TextSource>,
    style: TextStyle,
) -> Element<'a> {
    ui.text(style, text)
}
pub fn text_group<'a>(ui: &'a mut Ui<'_>) -> Element<'a> {
    ui.node(Preset::Bare)
        .hit(windows_scene::HitFlags::UIA, UiaRole::Text)
}
pub fn button<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a> {
    button_with(ui, text, TextStyle::new(TypeRole::Body))
}
pub fn button_with<'a>(
    ui: &'a mut Ui<'_>,
    text: impl Into<TextSource>,
    style: TextStyle,
) -> Element<'a> {
    ui.button(
        Chrome::new(roles::BUTTON[roles::DEFAULT as usize], Metric::Radius),
        style,
        text,
    )
}
pub fn edge_button<'a>(
    ui: &'a mut Ui<'_>,
    text: impl Into<TextSource>,
    edge: crate::layout::Edge,
    radius: Metric,
) -> Element<'a> {
    let chrome = Chrome {
        attached: Some(edge),
        ..Chrome::new(roles::BUTTON[roles::DEFAULT as usize], radius)
    };
    ui.button(chrome, TextStyle::new(TypeRole::Body), text)
}
pub fn icon_button<'a>(ui: &'a mut Ui<'_>, icon: windows_scene::GeomId) -> Element<'a> {
    ui.control(
        Some(Chrome::new(
            roles::BUTTON[roles::GHOST as usize],
            Metric::RadiusPill,
        )),
        UiaRole::Button,
        |ui| {
            ui.path(icon).ink();
        },
    )
}
pub fn card<'a>(ui: &'a mut Ui<'_>) -> Element<'a> {
    surface(ui, Elevation::Raised, roles::SURFACE_CARD)
        .min_width(Metric::CardMinW)
        .min_height(Metric::CardMinH)
}
pub fn panel<'a>(ui: &'a mut Ui<'_>, key: &'static str) -> Element<'a> {
    surface(ui, Elevation::Base, roles::SURFACE_PANEL).key(key)
}
pub fn flyout<'a>(ui: &'a mut Ui<'_>) -> Element<'a> {
    surface(ui, Elevation::Flyout, roles::SURFACE_FLYOUT)
}
fn surface<'a>(ui: &'a mut Ui<'_>, elevation: Elevation, variant: u8) -> Element<'a> {
    ui.node(Preset::Bare)
        .elevate(elevation)
        .appearance(Chrome::new(
            roles::SURFACE[variant as usize],
            Metric::RadiusSurface,
        ))
        .padding(Metric::SpaceLg)
}
pub fn box_<'a>(ui: &'a mut Ui<'_>) -> Element<'a> {
    ui.node(Preset::Bare).appearance(Chrome::new(
        roles::SURFACE[roles::SURFACE_PANEL as usize],
        Metric::RadiusSurface,
    ))
}
pub fn sheet<'a>(ui: &'a mut Ui<'_>, key: &'static str) -> Element<'a> {
    ui.node(Preset::Bare)
        .plate(Len::Zero, Role::Fill(Fill::Surface), 1.0)
        .key(key)
}
pub fn field<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a, Field> {
    field_with(ui, text, TextStyle::new(TypeRole::Body))
}
pub fn field_with<'a>(
    ui: &'a mut Ui<'_>,
    text: impl Into<TextSource>,
    style: TextStyle,
) -> Element<'a, Field> {
    ui.field(Chrome::new(roles::FIELD[0], Metric::Radius), style, text)
}
pub fn toggle<'a, M>(ui: &'a mut Ui<'_>, on: impl Signal<bool, M> + Copy + 'static) -> Element<'a> {
    ui.toggle(on)
}
pub fn select<'a>(
    ui: &'a mut Ui<'_>,
    text: impl Into<TextSource>,
    body: impl Fn(&mut Ui<'_>) + 'static,
) -> Element<'a> {
    button(ui, text)
        .hit(
            windows_scene::HitFlags::INTERACTIVE | windows_scene::HitFlags::GESTURE,
            UiaRole::ComboBox,
        )
        .flyout(body)
}
pub fn path<'a>(ui: &'a mut Ui<'_>, geometry: windows_scene::GeomId) -> Element<'a, Path> {
    ui.path(geometry)
}
pub fn segmented<'a, T: Copy + PartialEq + 'static>(
    ui: &'a mut Ui<'_>,
    value: impl Choice<T> + Copy + 'static,
    options: &'static [(&'static str, T)],
) -> Element<'a> {
    rail(
        ui,
        value,
        options,
        Metric::Radius,
        Metric::SpaceSm,
        TypeRole::Caption,
    )
}
pub fn pills<'a, T: Copy + PartialEq + 'static>(
    ui: &'a mut Ui<'_>,
    value: impl Choice<T> + Copy + 'static,
    options: &'static [(&'static str, T)],
) -> Element<'a> {
    rail(
        ui,
        value,
        options,
        Metric::RadiusPill,
        Metric::SpaceMd,
        TypeRole::Body,
    )
}
fn rail<'a, T: Copy + PartialEq + 'static>(
    ui: &'a mut Ui<'_>,
    value: impl Choice<T> + Copy + 'static,
    options: &'static [(&'static str, T)],
    radius: Metric,
    inset: Metric,
    ramp: TypeRole,
) -> Element<'a> {
    ui.node(Preset::Row)
        .appearance(Chrome::new(roles::GROOVE[0], radius))
        .height(Metric::RowH)
        .padding(Len::Times(Metric::HairlineW, 2.0))
        .gap(Len::Zero)
        .align(Align::Stretch)
        .hit(windows_scene::HitFlags::NONE, UiaRole::List)
        .children(|ui| {
            for &(name, option) in options {
                ui.control(
                    Some(Chrome::new(roles::OPTION[0], radius)),
                    UiaRole::RadioButton,
                    |ui| {
                        ui.text(TextStyle::new(ramp), name);
                    },
                )
                .selected(move || value.selected() == option)
                .on_click(move || value.choose(option))
                .min_height(Len::Zero)
                .padding_xy(inset, Len::Zero);
            }
        })
}
pub fn slider<'a, M>(
    ui: &'a mut Ui<'_>,
    value: impl Signal<f64, M> + Copy + 'static,
    range: Range,
    style: SliderStyle,
) -> Element<'a, Scalar> {
    let extent = crate::layout::probe();
    let fraction = range.fraction(style.origin.unwrap_or(range.min));
    let origin = if range.vertical {
        1.0 - fraction
    } else {
        fraction
    };
    let [geometry, marker] =
        ui.local_geometries(extent, [3; 2], move |[rail, marker], size, scope| {
            use windows_numerics::Vector2;
            let at = |along, cross| {
                if range.vertical {
                    Vector2::new(cross, along)
                } else {
                    Vector2::new(along, cross)
                }
            };
            let (length, cross) = if range.vertical {
                (size.y, size.x * 0.5)
            } else {
                (size.x, size.y * 0.5)
            };
            let half = crate::role::metric(Metric::SliderThumb, scope) * 0.55;
            for (out, a, b) in [
                (rail, at(0.0, cross), at(length, cross)),
                (
                    marker,
                    at(length * origin, cross - half),
                    at(length * origin, cross + half),
                ),
            ] {
                out.push(windows_scene::PathVerb::Segment { from: a, to: b });
            }
        });
    let inset = Len::Times(Metric::SliderThumb, 0.5);
    ui.control(
        Some(Chrome::new(roles::OPTION[0], Metric::RadiusPill)),
        UiaRole::Slider,
        |_| {},
    )
    .slide(value, range)
    .wash(Wash::Accent)
    .layout(|l| {
        l.flow = Some(if range.vertical {
            Preset::Stack
        } else {
            Preset::Row
        });
        if range.vertical {
            l.width = Some(Metric::RowH.into());
            l.padding = Some([Len::Zero, inset]);
        } else {
            l.height = Some(Metric::RowH.into());
            l.padding = Some([inset, Len::Zero]);
        }
    })
    .gap(Len::Zero)
    .justify(Align::Start)
    .align(Align::Center)
    .children(|ui| {
        ui.node(Preset::Row)
            .cover()
            .justify(Align::Center)
            .align(Align::Center)
            .padding_xy(
                if range.vertical {
                    Len::Zero
                } else {
                    Metric::SliderThumb.into()
                },
                if range.vertical {
                    Metric::SliderThumb.into()
                } else {
                    Len::Zero
                },
            )
            .children(|ui| {
                ui.node(Preset::Grid)
                    .cols([crate::layout::Track::Fr(1.0)])
                    .rows([crate::layout::Track::Fr(1.0)])
                    .width(if range.vertical {
                        Metric::RowH.into()
                    } else {
                        Len::Pct(1.0)
                    })
                    .height(if range.vertical {
                        Len::Pct(1.0)
                    } else {
                        Metric::RowH.into()
                    })
                    .align(Align::Center)
                    .justify(Align::Center)
                    .children(|ui| {
                        ui.plate(Metric::Radius, Role::Fill(Fill::Pressed), 1.0)
                            .width(if range.vertical {
                                Metric::SliderRailH.into()
                            } else {
                                Len::Pct(1.0)
                            })
                            .height(if range.vertical {
                                Len::Pct(1.0)
                            } else {
                                Metric::SliderRailH.into()
                            });
                        ui.path(geometry)
                            .slider_trail(origin, style.ramp)
                            .probed(extent)
                            .width(Len::Pct(1.0))
                            .height(Len::Pct(1.0))
                            .cover();
                        ui.path(marker)
                            .ink_stroke(Metric::HairlineW)
                            .opacity(if style.origin.is_some() { 0.15 } else { 0.0 })
                            .cover();
                    });
            });
        ui.plate(
            Len::Times(Metric::SliderThumb, 0.5),
            Role::Text(Text::Primary),
            1.0,
        )
        .width(Metric::SliderThumb)
        .height(Metric::SliderThumb)
        .no_shrink()
        .thumb();
    })
}
pub fn knob<'a, M>(
    ui: &'a mut Ui<'_>,
    value: impl Signal<f64, M> + Copy + 'static,
    range: Range,
) -> Element<'a, Scalar> {
    ui.control(
        Some(Chrome::new(
            roles::TRACK[roles::TRACK_OFF as usize],
            Metric::RadiusPill,
        )),
        UiaRole::Slider,
        |_| {},
    )
    .turn(value, range)
    .wash(Wash::Accent)
    .stack(|ui| {
        ui.plate(Metric::RadiusPill, Role::Fill(Fill::Accent), 1.0)
            .thumb()
            .scalar_part(super::ScalarPart::Rotation {
                from: 0.0,
                to: super::TURN_SWEEP,
            });
    })
}
pub fn meter<'a, M>(ui: &'a mut Ui<'_>, level: impl Signal<f32, M> + 'static) -> Element<'a> {
    ui.node(Preset::Stack)
        .appearance(Chrome::new(
            roles::TRACK[roles::TRACK_OFF as usize],
            Metric::Radius,
        ))
        .children(|ui| {
            ui.plate(Metric::Radius, Role::Fill(Fill::Accent), 1.0)
                .channel(
                    windows_scene::Prop::ScaleX,
                    super::Motion::Chrome,
                    level,
                    windows_scene::Value::Scalar,
                );
        })
}
