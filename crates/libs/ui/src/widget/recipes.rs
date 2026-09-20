//! Stock recipes use the same retained writes as application components.
//!
//! Each states only what it adds to its preset. The presets carry no gap, no padding and no
//! minimum, so a line here that sets one is a line that means it.

use super::roles::{
    self, Chrome, Interaction, ModelState, Range, ScalarPart, ScalarValue, TURN_SWEEP, UiaRole,
};
use super::text::{Flow, TextSource};
use crate::build::{Element, Field, Path, Scalar, Ui};
use crate::layout::{Align, Edge, Len, Preset};
use crate::role::{Elevation, Fill, Metric, Role, Text, TypeRole};
use crate::signal::{Cell, Signal};
use std::rc::Rc;
use windows_numerics::Vector2;
use windows_scene::{GeomId, HitFlags, PathVerb, Prop, RampId};

/// Text appearance chosen by an application recipe. Typography stays unresolved until layout.
#[derive(Copy, Clone, Debug)]
pub struct TextStyle {
    pub typography: TypeRole,
    /// Standalone ink. An enclosing control's chrome governs the label colour instead where this
    /// is absent.
    pub ink: Option<Role>,
    pub flow: Flow,
    pub caps: bool,
    pub vertical: bool,
}

impl TextStyle {
    /// A primary, single-line run with the supplied typography token.
    #[must_use]
    pub const fn new(typography: TypeRole) -> Self {
        Self {
            typography,
            ink: Some(Role::Text(Text::Primary)),
            flow: Flow::Line,
            caps: false,
            vertical: false,
        }
    }

    /// Overrides only the ink; typography remains a solve-time token.
    #[must_use]
    pub const fn ink(self, ink: Role) -> Self {
        Self {
            ink: Some(ink),
            ..self
        }
    }

    #[must_use]
    pub const fn flow(self, flow: Flow) -> Self {
        Self { flow, ..self }
    }

    #[must_use]
    pub const fn caps(self, caps: bool) -> Self {
        Self { caps, ..self }
    }

    #[must_use]
    pub const fn vertical(self, vertical: bool) -> Self {
        Self { vertical, ..self }
    }
}

/// Paint and origin of a slider's retained value stroke.
#[derive(Copy, Clone, Debug)]
pub struct SliderStyle {
    /// Value the fill grows from; the range minimum when absent.
    pub origin: Option<f64>,
    /// A fixed gradient across the whole rail; trimmed rather than rescaled.
    pub ramp: Option<RampId>,
    pub rail: Metric,
    pub thumb: Metric,
    pub height: Metric,
    pub mark_origin: bool,
}

impl Default for SliderStyle {
    fn default() -> Self {
        Self {
            origin: None,
            ramp: None,
            rail: Metric::SliderRailH,
            thumb: Metric::SliderThumb,
            height: Metric::RowH,
            mark_origin: true,
        }
    }
}

/// Construction-time dimensions and typography for a retained choice rail.
#[derive(Copy, Clone, Debug)]
pub struct ChoiceStyle {
    pub radius: Metric,
    pub height: Metric,
    pub inset: Metric,
    pub typography: TypeRole,
}

/// What a choice rail reads and writes.
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

/// Gives a control the room a pressable row needs: a height nobody stated, the inset its label
/// sits in, the space between a glyph and that label, and the centring of both.
///
/// A control is minted as a `Row`, and no preset carries a floor, a padding, a gap or a
/// justification — so a recipe that means one says it. Without this a button is exactly as tall and
/// as wide as its text.
fn room<K>(element: Element<'_, K>) -> Element<'_, K> {
    element
        .layout(|l| l.floor = Len::from(Metric::RowH))
        .padding_xy(Metric::SpaceMd, Metric::SpaceXs)
        .gap(Metric::SpaceSm)
        .justify(Align::Center)
}

/// Restates a control's container preset without adding to its body.
///
/// `Ui::control` and `Ui::scalar` mint the node and its children together and take no preset, so a
/// recipe wanting a different container names it here. The closure is empty because the body was
/// already declared.
fn as_layer<K>(element: Element<'_, K>) -> Element<'_, K> {
    element.layer(|_| {})
}

/// The stock text recipes. They differ in a style and in nothing else, so they are a table.
macro_rules! runs {
    ($($name:ident = $style:expr;)*) => {$(
        pub fn $name<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a> {
            ui.text_owned(None, $style, text)
        }
    )*};
}

runs! {
    text = TextStyle::new(TypeRole::Body);
    title = TextStyle::new(TypeRole::Title);
    label = TextStyle::new(TypeRole::Label).ink(Role::Text(Text::Secondary));
    caption = TextStyle::new(TypeRole::Caption).ink(Role::Text(Text::Tertiary)).flow(Flow::Wrap);
    note = TextStyle::new(TypeRole::Caption).ink(Role::Text(Text::Tertiary));
    micro = TextStyle::new(TypeRole::Micro).ink(Role::Text(Text::Tertiary));
    mono = TextStyle::new(TypeRole::Mono);
    code = TextStyle::new(TypeRole::Mono).flow(Flow::Wrap);
    display = TextStyle::new(TypeRole::Display);
    vertical_label = TextStyle::new(TypeRole::Label)
        .ink(Role::Text(Text::Secondary))
        .vertical(true);
}

/// A run with an application-chosen style, painted as standalone ink.
pub fn styled_text<'a>(
    ui: &'a mut Ui<'_>,
    text: impl Into<TextSource>,
    style: TextStyle,
) -> Element<'a> {
    ui.text_owned(None, style, text)
}

/// A run belonging to the enclosing control, so its chrome governs the colour.
pub fn control_text<'a>(
    ui: &'a mut Ui<'_>,
    text: impl Into<TextSource>,
    style: TextStyle,
) -> Element<'a> {
    ui.text(style, text)
}

/// A container announced as one run, for text an application assembles from several.
pub fn text_group<'a>(ui: &'a mut Ui<'_>) -> Element<'a> {
    ui.node(Preset::Layer).hit(HitFlags::UIA, UiaRole::Text)
}

fn surface<'a>(ui: &'a mut Ui<'_>, elevation: Elevation, variant: u8) -> Element<'a> {
    let roles = roles::SURFACE[variant as usize];
    ui.node(Preset::Stack)
        .elevate(elevation)
        .appearance(Chrome::new(roles, Metric::RadiusSurface))
        .padding(Metric::SpaceLg)
        .gap(Metric::SpaceSm)
}

/// A raised surface with a hairline.
///
/// The height is a floor rather than a stated minimum, so an application that states one keeps it.
/// A floor is the block axis alone, so the inline minimum is stated as a minimum.
pub fn card<'a>(ui: &'a mut Ui<'_>) -> Element<'a> {
    surface(ui, Elevation::Raised, roles::SURFACE_CARD)
        .layout(|l| l.floor = Len::from(Metric::CardMinH))
        .min_width(Metric::CardMinW)
}

pub fn panel<'a>(ui: &'a mut Ui<'_>, key: &'static str) -> Element<'a> {
    surface(ui, Elevation::Base, roles::SURFACE_PANEL).key(key)
}

pub fn flyout<'a>(ui: &'a mut Ui<'_>) -> Element<'a> {
    surface(ui, Elevation::Flyout, roles::SURFACE_FLYOUT)
}

/// A filled container with no padding and no elevation of its own, stacking its children on
/// the block axis.
///
/// Arranged rather than layered: a container that stretched every child over the same box
/// would draw two runs on top of one another, and an author reaching for a plain filled box
/// has said nothing about wanting that. A caller that does states it, with
/// [`Element::layer`](crate::Element::layer) or [`Element::grid`](crate::Element::grid).
pub fn box_<'a>(ui: &'a mut Ui<'_>) -> Element<'a> {
    let roles = roles::SURFACE[roles::SURFACE_PANEL as usize];
    ui.node(Preset::Stack)
        .appearance(Chrome::new(roles, Metric::RadiusSurface))
}

/// A square-edged plate that meets its neighbours, as a docked inspector does.
pub fn sheet<'a>(ui: &'a mut Ui<'_>, key: &'static str) -> Element<'a> {
    ui.node(Preset::Stack)
        .plate(Len::ZERO, Role::Fill(Fill::Surface), 1.0)
        .key(key)
}

pub fn button<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a> {
    button_with(ui, text, TextStyle::new(TypeRole::Body))
}

pub fn button_with<'a>(
    ui: &'a mut Ui<'_>,
    text: impl Into<TextSource>,
    style: TextStyle,
) -> Element<'a> {
    let roles = roles::BUTTON[roles::DEFAULT as usize];
    room(ui.button(Chrome::new(roles, Metric::Radius), style, text))
}

/// A button flush with one edge: that edge has square corners and no border.
pub fn edge_button<'a>(
    ui: &'a mut Ui<'_>,
    text: impl Into<TextSource>,
    edge: Edge,
    radius: Metric,
) -> Element<'a> {
    let roles = roles::BUTTON[roles::DEFAULT as usize];
    let mut chrome = Chrome::new(roles, radius);
    chrome.attached = Some(edge);
    room(ui.button(chrome, TextStyle::new(TypeRole::Body), text))
}

/// A ghost button `side` square with `body` centred in it.
///
/// The body is the mark the caller draws, sized by the caller or filling the box; the button
/// states no inset, so a mark the size of the box is the box. A caller widening one side
/// gets a stadium.
pub fn icon_button<'a>(
    ui: &'a mut Ui<'_>,
    side: impl Into<Len> + Copy,
    body: impl FnOnce(&mut Ui<'_>),
) -> Element<'a> {
    let roles = roles::BUTTON[roles::GHOST as usize];
    let button = ui.control(Some(Chrome::new(roles, Metric::Radius)), UiaRole::Button, body);
    as_layer(button)
        .wash(roles::Wash::Ink)
        .size(side)
        .justify(Align::Center)
        .align(Align::Center)
}

/// A button that opens `body` beneath it, announced as a combo box.
pub fn select<'a>(
    ui: &'a mut Ui<'_>,
    text: impl Into<TextSource>,
    body: impl Fn(&mut Ui<'_>) + 'static,
) -> Element<'a> {
    button(ui, text)
        .hit(HitFlags::INTERACTIVE | HitFlags::GESTURE, UiaRole::ComboBox)
        .flyout(body)
}

pub fn field<'a>(ui: &'a mut Ui<'_>, text: impl Into<TextSource>) -> Element<'a, Field> {
    field_with(ui, text, TextStyle::new(TypeRole::Body))
}

pub fn field_with<'a>(
    ui: &'a mut Ui<'_>,
    text: impl Into<TextSource>,
    style: TextStyle,
) -> Element<'a, Field> {
    // Justified at the start, not centred: a field's text is read and edited from its first
    // character, and the caret has to sit somewhere stable when the field is empty.
    ui.field(Chrome::new(roles::FIELD[0], Metric::Radius), style, text)
        .layout(|l| l.floor = Len::from(Metric::RowH))
        .padding_xy(Metric::SpaceMd, Metric::SpaceXs)
}

pub fn toggle<'a, M>(ui: &'a mut Ui<'_>, on: impl Signal<bool, M> + Copy + 'static) -> Element<'a> {
    ui.toggle(on)
}

/// Declares a two-state control: a track that fills when it is on, and a knob over its travel.
///
/// The on-state fill is the chrome ladder's own selected row rather than a second plate faded over
/// the first, so the state change is the crossfade every other control's is.
///
/// Everything a toggle looks like is here. What is left to [`Ui::toggle`] is binding the value,
/// because reaching a scalar drive from a `bool` is `build`'s own adapter and this layer cannot
/// name it.
pub fn toggle_body<'a, M>(
    ui: &'a mut Ui<'_>,
    on: impl Signal<bool, M> + Copy + 'static,
) -> Element<'a> {
    let chrome = Chrome::new(roles::TRACK[roles::TRACK_OFF as usize], Metric::RadiusPill)
        .when(ModelState::Selected, roles::TRACK[roles::TRACK_ON as usize]);
    ui.control(Some(chrome), UiaRole::CheckBox, |ui| {
        ui.plate(Metric::RadiusPill, Role::Text(Text::Primary), 1.0)
            .size(Len::times(Metric::TrackH, 0.8))
            .scalar_part(ScalarPart::Thumb { vertical: false });
    })
    .selected(on)
    .height(Metric::TrackH)
    .width(Len::times(Metric::TrackH, 1.7))
    // The knob rests this far inside the track at either end, which is the travel the solve
    // measures and the router then moves it over.
    .padding(Len::times(Metric::TrackH, 0.1))
    .justify(Align::Start)
    .align(Align::Center)
}

pub fn path<'a>(ui: &'a mut Ui<'_>, geometry: GeomId) -> Element<'a, Path> {
    ui.path(geometry)
}

const SEGMENTED: ChoiceStyle = ChoiceStyle {
    radius: Metric::Radius,
    height: Metric::RowH,
    inset: Metric::SpaceSm,
    typography: TypeRole::Caption,
};

const PILLS: ChoiceStyle = ChoiceStyle {
    radius: Metric::RadiusPill,
    height: Metric::RowH,
    inset: Metric::SpaceMd,
    typography: TypeRole::Body,
};

pub fn segmented<'a, T: Copy + PartialEq + 'static>(
    ui: &'a mut Ui<'_>,
    choice: impl Choice<T> + 'static,
    options: &'static [(&'static str, T)],
) -> Element<'a> {
    segmented_with(ui, choice, options, SEGMENTED)
}

pub fn pills<'a, T: Copy + PartialEq + 'static>(
    ui: &'a mut Ui<'_>,
    choice: impl Choice<T> + 'static,
    options: &'static [(&'static str, T)],
) -> Element<'a> {
    segmented_with(ui, choice, options, PILLS)
}

/// Builds the stock choice rail with application-owned sizing and text tokens.
///
/// The options touch, so the rail states a zero gap that its preset does not carry, and they fill
/// the groove's height, which a row's centred default does not give.
///
/// One `Choice` reaches every option's own reader and writer, which is what the count is shared
/// for: the rail holds no selection of its own to keep in step with the application's.
pub fn segmented_with<'a, T: Copy + PartialEq + 'static>(
    ui: &'a mut Ui<'_>,
    choice: impl Choice<T> + 'static,
    options: &'static [(&'static str, T)],
    style: ChoiceStyle,
) -> Element<'a> {
    let choice = Rc::new(choice);
    ui.node(Preset::Row)
        .appearance(Chrome::new(roles::GROOVE[0], style.radius))
        .height(style.height)
        .padding(Len::times(Metric::HairlineW, 2.0))
        .gap(Len::ZERO)
        .align(Align::Stretch)
        .hit(HitFlags::NONE, UiaRole::List)
        .children(move |ui| {
            for &(name, option) in options {
                let (reads, writes) = (Rc::clone(&choice), Rc::clone(&choice));
                ui.control(
                    Some(Chrome::new(roles::OPTION[0], style.radius)),
                    UiaRole::RadioButton,
                    |ui| {
                        ui.text(TextStyle::new(style.typography), name);
                    },
                )
                .selected(move || reads.selected() == option)
                .on_click(move || writes.choose(option))
                .padding_xy(style.inset, Len::ZERO)
                .justify(Align::Center);
            }
        })
}

/// Returns an `(x, y)` pair from an extent along a control's own axis and one across it, so a
/// slider is authored once rather than once per orientation.
const fn axes(vertical: bool, main: Len, cross: Len) -> (Len, Len) {
    if vertical {
        (cross, main)
    } else {
        (main, cross)
    }
}

pub fn slider<'a, M>(
    ui: &'a mut Ui<'_>,
    value: impl Signal<f64, M> + Copy + 'static,
    range: Range,
    style: SliderStyle,
) -> Element<'a, Scalar> {
    // One epoch: an application that never replaces the source never supersedes a gesture on it.
    let source = move || ScalarValue {
        value: value.read(),
        epoch: 0,
    };
    slider_source(ui, source, range, style)
}

/// A stock slider with the same epoch-aware source contract as a custom scalar.
pub fn slider_source<'a, M>(
    ui: &'a mut Ui<'_>,
    value: impl Signal<ScalarValue, M> + Copy + 'static,
    range: Range,
    style: SliderStyle,
) -> Element<'a, Scalar> {
    let vertical = range.vertical;
    let extent = crate::layout::probe();
    // The trim runs along the rail, and a vertical value grows against that direction.
    let at_min = range.fraction(style.origin.unwrap_or(range.min));
    let origin = if vertical { 1.0 - at_min } else { at_min };
    let [rail, mark] = ui.geometries(
        crate::build::geometry::Source::Probe(extent),
        [3; 2],
        move |inputs, [rail, mark]| {
            let at = |main, cross| {
                let (x, y) = if vertical {
                    (cross, main)
                } else {
                    (main, cross)
                };
                Vector2::new(x, y)
            };
            let (length, mid) = if vertical {
                (inputs.size.y, inputs.size.x * 0.5)
            } else {
                (inputs.size.x, inputs.size.y * 0.5)
            };
            let half = crate::role::metric(style.thumb, inputs.scope) * 0.55;
            for (out, from, to) in [
                (rail, at(0.0, mid), at(length, mid)),
                (
                    mark,
                    at(length * origin, mid - half),
                    at(length * origin, mid + half),
                ),
            ] {
                out.push(PathVerb::Segment { from, to });
            }
        },
    );
    let (pad_x, pad_y) = axes(vertical, Len::times(style.thumb, 0.5), Len::ZERO);
    let (w, h) = axes(vertical, Len::pct(1.0), style.height.into());
    let (rail_w, rail_h) = axes(vertical, Len::pct(1.0), style.rail.into());
    let slider = ui.scalar(
        Some(Chrome::new(roles::OPTION[0], Metric::RadiusPill)),
        Interaction::Slide(range),
        value,
        move |ui| {
            // A layer gives every child the whole box, so the rail, the trail and the origin mark
            // stack without a grid of one track to hold them.
            ui.node(Preset::Layer).children(move |ui| {
                ui.plate(Metric::Radius, Role::Fill(Fill::Pressed), 1.0)
                    .width(rail_w)
                    .height(rail_h)
                    .align_self(Align::Center);
                let trail = ui.path(rail).probed(extent);
                let trail = match style.ramp {
                    Some(ramp) => trail.stroke_ramp(ramp, style.rail),
                    None => trail.ink_stroke(style.rail),
                };
                trail.scalar_part(ScalarPart::Trail { from: origin });
                // Minted only where it is wanted: a sprite at zero opacity is still a visual on
                // the idle frontier.
                if style.origin.is_some() && style.mark_origin {
                    ui.path(mark).ink_stroke(Metric::HairlineW).strength(0.15);
                }
            });
            ui.plate(Len::times(style.thumb, 0.5), Role::Text(Text::Primary), 1.0)
                .size(style.thumb)
                .scalar_part(ScalarPart::Thumb { vertical });
        },
    );
    as_layer(slider).width(w).height(h).padding_xy(pad_x, pad_y)
}

/// A rotary control: a track, and a needle turned by a single-pointer rotation or a dial detent.
pub fn knob<'a, M>(
    ui: &'a mut Ui<'_>,
    value: impl Signal<f64, M> + Copy + 'static,
    range: Range,
) -> Element<'a, Scalar> {
    let roles = roles::TRACK[roles::TRACK_OFF as usize];
    let source = move || ScalarValue {
        value: value.read(),
        epoch: 0,
    };
    ui.scalar(
        Some(Chrome::new(roles, Metric::RadiusPill)),
        Interaction::Turn(range),
        source,
        |ui| {
            ui.plate(Metric::RadiusPill, Role::Fill(Fill::Accent), 1.0)
                .scalar_part(ScalarPart::Rotation {
                    from: 0.0,
                    to: TURN_SWEEP,
                });
        },
    )
    .aspect(1.0)
}

/// A level bed with an accent fill scaled to `level`. Not a control: nothing points at it.
pub fn meter<'a, M>(ui: &'a mut Ui<'_>, level: impl Signal<f32, M> + 'static) -> Element<'a> {
    let roles = roles::TRACK[roles::TRACK_OFF as usize];
    ui.node(Preset::Layer)
        .appearance(Chrome::new(roles, Metric::Radius))
        .hit(HitFlags::UIA, UiaRole::ProgressBar)
        .children(|ui| {
            ui.plate(Metric::Radius, Role::Fill(Fill::Accent), 1.0)
                .channel(Prop::ScaleX, level);
        })
}
