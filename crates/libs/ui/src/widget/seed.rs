//! The widget seeds: each writes a slot and returns an element.
//!
//! A seed does not call `Model`, resolve a colour or build a style. The lowering does all
//! three, which is what keeps a widget to one short function.
//!
//! A composition is a function returning a tree of these. It is where `badge`, `nav`, `tabs`
//! and every screen an application assembles for itself live, and it adds nothing here.

use crate::build::{Button, El, Path, View};
use crate::gesture::{DragDecl, GestureDecl};
use crate::layout::Preset;
use crate::role::{Fill, Metric, Role, Text, TypeRole};
use crate::signal::{Cell, Signal};
use crate::widget::{Flow, Interaction, Range, StatePolicy, TextSource, UiaRole, Wash, roles};
use windows_scene::{GeomId, HitFlags};

// ── text ─────────────────────────────────────────────────────────────────────────
//
// Five rungs of one ladder. Each carries its own role and type ramp, and none takes a colour
// or a size.

/// Body copy.
#[must_use]
pub fn text(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Body, Text::Primary, Flow::Line)
}

/// A heading.
#[must_use]
pub fn title(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Title, Text::Primary, Flow::Line)
}

/// A field's or a group's name, set secondary to the thing it labels.
#[must_use]
pub fn label(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Label, Text::Secondary, Flow::Line)
}

/// Supporting prose. It wraps, so it is the one text widget that mounts as a group: a
/// coverage tile covers one line, and a run that can break needs a sprite per line.
#[must_use]
pub fn caption(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Caption, Text::Tertiary, Flow::Wrap)
}

/// A read-out, in tabular figures, so its digits do not shift width as it changes.
#[must_use]
pub fn mono(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Mono, Text::Primary, Flow::Line)
}

/// Builds a text run: the shared body of the five text widgets and of every label inside a
/// control.
///
/// `ink` is stated here and overridden at mount by the enclosing widget's chrome row where
/// there is one, so a button's variant reaches its text without the text naming a variant.
fn run(s: impl Into<TextSource>, ramp: TypeRole, ink: Text, flow: Flow) -> View {
    El::seed(Preset::Text)
        .text_seed(s.into(), ramp, Some(ink), flow)
        // `UIA` and nothing else: a run has no gesture, takes no focus and routes no
        // pointer, so the hit scan skips it on one flags test. With no entry at all it
        // would have no automation peer, and a screen of text would read as empty.
        .hit(HitFlags::UIA, UiaRole::Text)
}

/// A label whose colour is the enclosing control's rather than its own.
///
/// It mints no automation peer: the control it sits in derives its accessible name from this
/// text, so a peer would have a reader announce the control's name twice.
fn inner(s: impl Into<TextSource>, ramp: TypeRole) -> View {
    El::seed(Preset::Text).text_seed(s.into(), ramp, None, Flow::Line)
}

// ── surfaces ─────────────────────────────────────────────────────────────────────
//
// A surface takes an optional key and never children. Children arrive through a layout
// modifier — `card().stack((..))`, `panel("effects").row((..))` — since every layout class
// exists as both a free function and a method over one table, so four surfaces and seven
// classes are not twenty-eight signatures.

/// A bare filled rectangle. No scope push, so nothing inside it resolves differently.
#[must_use]
pub fn box_() -> View {
    El::seed(Preset::Bare).chrome(roles::SURFACE, roles::SURFACE_PANEL, Metric::Radius)
}

/// A raised surface: a scope push to `Raised`, its own padding, radius and hairline, and
/// minimum metrics from the palette.
#[must_use]
pub fn card() -> View {
    El::seed(Preset::Bare)
        .surface(
            crate::role::Elevation::Raised,
            roles::SURFACE_CARD,
            Metric::Radius,
        )
        .min_width(Metric::CardMinW)
        .min_height(Metric::CardMinH)
}

/// A region of the window's own plane, with no hairline.
#[must_use]
pub fn panel(key: &'static str) -> View {
    El::seed(Preset::Bare)
        .surface(
            crate::role::Elevation::Base,
            roles::SURFACE_PANEL,
            Metric::Radius,
        )
        .key(key)
}

/// A detached surface above everything. The overlay layer anchors and dismisses it; this is
/// only what it looks like.
#[must_use]
pub fn flyout() -> View {
    El::seed(Preset::Bare).surface(
        crate::role::Elevation::Flyout,
        roles::SURFACE_FLYOUT,
        Metric::Radius,
    )
}

// ── interactive ──────────────────────────────────────────────────────────────────

/// A press. Four variants over one table, chosen on [`El<Button>`](Button).
#[must_use]
pub fn button(text: impl Into<TextSource>) -> El<Button> {
    control(UiaRole::Button)
        .chrome(roles::BUTTON, roles::DEFAULT, Metric::Radius)
        .row(inner(text, TypeRole::Body))
}

/// A press with no text, so [`name`](El::name) is required: there is nothing to derive an
/// accessible name from.
#[must_use]
pub fn icon_button(icon: GeomId) -> El<Button> {
    control(UiaRole::Button)
        .chrome(roles::BUTTON, roles::GHOST, Metric::RadiusPill)
        .row(path(icon).ink())
}

/// A two-state switch. The knob is a sprite sprung between the ends of its track, so the
/// transition is a compositor animation and costs no frame after the one that started it.
#[must_use]
pub fn toggle<M>(on: impl Signal<bool, M> + Copy + 'static) -> View {
    control::<crate::build::Any>(UiaRole::CheckBox)
        .chrome(roles::TRACK, roles::TRACK_OFF, Metric::RadiusPill)
        .selected(on)
        .interaction(Interaction::Press)
        .row(knob_sprite().along(false, move || f32::from(u8::from(on.read()))))
}

/// A value along a track. The thumb moves front-side in the tick that saw the contact,
/// and the number reaches the application afterwards.
#[must_use]
pub fn slider<M>(value: impl Signal<f64, M> + Copy + 'static, range: Range) -> View {
    control::<crate::build::Any>(UiaRole::Slider)
        .chrome(roles::TRACK, roles::TRACK_OFF, Metric::RadiusPill)
        .interaction(Interaction::Slide(range))
        .gesture(GestureDecl::slider(range.vertical))
        .state(accent_wash())
        .row(knob_sprite().along(range.vertical, move || range.fraction(value.read())))
}

/// A value turned rather than slid.
///
/// The moving part is a child node rather than the control itself, so the router retargets
/// the same kind of part it retargets for a slider.
///
/// Its bed is a chrome row like every other control's, which is what gives the interaction
/// wash its radius: a wash takes the shape of the surface it covers, and a control with no
/// row would be washed as a square.
#[must_use]
pub fn knob<M>(value: impl Signal<f64, M> + Copy + 'static, range: Range) -> View {
    control::<crate::build::Any>(UiaRole::Slider)
        .chrome(roles::TRACK, roles::TRACK_OFF, Metric::RadiusPill)
        .interaction(Interaction::Turn(range))
        .drag(DragDecl::turn())
        .state(accent_wash())
        .stack(
            El::<crate::build::Any>::seed(Preset::Bare)
                .thumb(Metric::RadiusPill, Role::Fill(Fill::Accent))
                // A fraction and not an angle: whichever side is moving the part applies
                // the sweep, through `angle_of`, so a committed value and a live drag land
                // the knob in the same place.
                .turns(move || range.fraction(value.read())),
        )
}

/// One choice of several, laid out as a row.
///
/// Selection is [`ModelState`](super::ModelState) — a discrete paint swap at event rate —
/// rather than a variant, because it is state any control can be in and not something only
/// this widget has.
#[must_use]
pub fn segmented<T>(value: Cell<T>, options: &'static [(&'static str, T)]) -> View
where
    T: Copy + PartialEq + 'static,
{
    // The one allocation in the widget set: a child count that comes from a slice cannot be
    // a tuple, which is what static structure elsewhere uses.
    let kids: Vec<View> = options
        .iter()
        .map(|&(name, option)| {
            control::<crate::build::Any>(UiaRole::Button)
                .chrome(roles::OPTION, 0, Metric::Radius)
                .selected(move || value.get() == option)
                .on_click(move || value.set(option))
                .row(inner(name, TypeRole::Label))
        })
        .collect();
    El::seed(Preset::Bare)
        .row(kids)
        // An automation container and nothing else: the options route the pointer.
        .hit(HitFlags::NONE, UiaRole::List)
}

/// A text-editable field. Text services own the caret; this declares the target.
#[must_use]
pub fn field(value: impl Into<TextSource>) -> View {
    El::seed(Preset::Bare)
        .chrome(roles::FIELD, 0, Metric::Radius)
        .control()
        .hit(
            HitFlags::INTERACTIVE | HitFlags::GESTURE | HitFlags::TEXT,
            UiaRole::Edit,
        )
        .state(ink_wash())
        .row(inner(value, TypeRole::Body))
}

/// A button that opens a list of options.
///
/// A widget rather than a composition, because its automation pattern is `ComboBox` and only
/// a widget declares one.
#[must_use]
pub fn select(text: impl Into<TextSource>, body: impl Fn() -> View + 'static) -> El<Button> {
    control(UiaRole::ComboBox)
        .chrome(roles::BUTTON, roles::DEFAULT, Metric::Radius)
        .flyout(body)
        .row(inner(text, TypeRole::Body))
}

/// A read-only level.
///
/// It mints no hit entry, so it costs no control row, no front-side row and no slot in the
/// array every pointer sample is resolved against — which is what a column of meters would
/// otherwise add up to. Its level springs, because a meter carries momentum.
#[must_use]
pub fn meter<M>(level: impl Signal<f32, M> + 'static) -> View {
    El::seed(Preset::Bare)
        .chrome(roles::TRACK, roles::TRACK_OFF, Metric::Radius)
        .stack(
            El::<crate::build::Any>::seed(Preset::Bare)
                .thumb(Metric::Radius, Role::Fill(Fill::Accent))
                // A scale and not an offset: the bed is this node's own box, so a fraction of
                // it needs nothing from layout.
                .scale_x(level),
        )
}

/// Arbitrary geometry, in sprite-local DIPs. The one kind-marked builder.
#[must_use]
pub fn path(geom: GeomId) -> El<Path> {
    El::<Path>::seed(Preset::Bare).geom(geom)
}

/// The shape every interactive widget starts from: the palette's row height as a floor, a
/// tighter gap, control padding, and the flags that route a pointer to it.
fn control<K>(uia: UiaRole) -> El<K> {
    El::seed(Preset::Bare)
        .control()
        .hit(HitFlags::INTERACTIVE | HitFlags::GESTURE, uia)
        .state(ink_wash())
}

/// The moving part of a toggle or a slider.
fn knob_sprite() -> View {
    El::<crate::build::Any>::seed(Preset::Bare).thumb(Metric::RadiusPill, Role::Fill(Fill::Surface))
}

const fn ink_wash() -> StatePolicy {
    StatePolicy::Wash {
        hover: Wash::Ink,
        press: Wash::Ink,
    }
}

/// The wash for a control whose moving part is drawn in the accent, so hover and press stay
/// in the one hue.
const fn accent_wash() -> StatePolicy {
    StatePolicy::Wash {
        hover: Wash::Accent,
        press: Wash::Accent,
    }
}

const _: () = {
    // Every widget above names one of these tables, and a variant method addresses a row of
    // it. `Chrome::roles` clamps to the last row, so an empty table would index out of
    // bounds.
    assert!(!roles::BUTTON.is_empty());
    assert!(!roles::SURFACE.is_empty());
    assert!(!roles::TRACK.is_empty());
    assert!(!roles::FIELD.is_empty());
    assert!(!roles::OPTION.is_empty());
};
