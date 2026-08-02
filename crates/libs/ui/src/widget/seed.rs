//! The widget set: nineteen functions, each a seed with no body.
//!
//! A widget writes a slot and returns an element. It does not call `Model`, resolve a
//! colour or build a style — the lowering does all three — which is what makes nineteen
//! widgets nineteen short functions rather than nineteen implementations, and what makes
//! the twentieth cost the same as the first.
//!
//! A **composition** is a function returning a tree of these. It costs this crate nothing
//! and needs no permission, and it is where `badge`, `nav`, `tabs` and every screen an
//! application assembles for itself live.

use crate::build::{Button, El, Path, View};
use crate::gesture::{DragDecl, GestureDecl};
use crate::layout::Preset;
use crate::role::{Fill, Metric, Role, Text, TypeRole};
use crate::signal::{Cell, Signal};
use crate::widget::{Flow, Interaction, Range, StatePolicy, TextSource, UiaRole, Wash, roles};
use windows_scene::{GeomId, HitFlags};

// ── text ─────────────────────────────────────────────────────────────────────────
//
// Five rungs of one ladder. Each carries its own role and type ramp and **none accepts a
// colour or a size**, which is the whole of why a call site never restates the theme's job.

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

/// A field's or a group's name. Secondary, because a label is not the thing it labels.
#[must_use]
pub fn label(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Label, Text::Secondary, Flow::Line)
}

/// Supporting prose. **Wraps by definition**, which is why it is the one text widget that
/// is a group: a coverage tile is one line's, so a run that can break needs a sprite per
/// line and everything else stays a single visual.
#[must_use]
pub fn caption(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Caption, Text::Tertiary, Flow::Wrap)
}

/// A read-out, in tabular figures, so its digits do not shift width as it changes.
#[must_use]
pub fn mono(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Mono, Text::Primary, Flow::Line)
}

/// The shared body of all five, and of every label inside a control.
///
/// `ink` is stated here and overridden at mount by the enclosing widget's chrome row where
/// there is one, so a button's variant reaches its text without the text knowing there are
/// variants.
fn run(s: impl Into<TextSource>, ramp: TypeRole, ink: Text, flow: Flow) -> View {
    El::seed(Preset::Text).text_seed(s.into(), ramp, Some(ink), flow)
}

/// A label whose colour is the enclosing control's, not its own.
fn inner(s: impl Into<TextSource>, ramp: TypeRole) -> View {
    El::seed(Preset::Text).text_seed(s.into(), ramp, None, Flow::Line)
}

// ── surfaces ─────────────────────────────────────────────────────────────────────
//
// **A surface takes an optional key, never children.** Children arrive through a layout
// modifier — `card().stack((..))`, `panel("effects").row((..))` — because every layout
// class exists as both a free function and a method over one table. That is what keeps four
// surfaces by seven classes from being twenty-eight signatures.

/// A bare filled rectangle. No scope push, so nothing inside it resolves differently.
#[must_use]
pub fn box_() -> View {
    El::seed(Preset::Bare).chrome(roles::SURFACE, roles::SURFACE_PANEL, Metric::Radius)
}

/// The workhorse. A scope push to `Raised`, its own padding, radius and hairline, and
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

/// A region of the window's own plane. No hairline — an outline here would be a box drawn
/// around nothing.
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

/// A value along a track. The thumb moves **front-side** in the tick that saw the contact,
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

/// A value turned rather than dragged.
///
/// The moving part is a **child**, not the control itself: "the part a value moves" is one
/// concept whether it slides or turns, and the router retargets that part. A knob whose
/// rotation sat on its own node would be the one control the front thread could not move.
///
/// Its bed is a chrome row like every other control's, which is also what gives its
/// interaction wash a radius — a wash resolves the surface it covers, and a control with no
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
                // A fraction and not an angle: the sweep is applied by whichever side is
                // moving the part, from one function, so a committed value and a live drag
                // cannot land the knob in two different places.
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
    // The one heap form, and it is visible here: a widget whose child count comes from a
    // slice cannot be a tuple. Static structure elsewhere never touches it.
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
/// A widget rather than a composition for one reason: its automation pattern is `ComboBox`,
/// and that has to be declared somewhere.
#[must_use]
pub fn select(text: impl Into<TextSource>, body: impl Fn() -> View + 'static) -> El<Button> {
    control(UiaRole::ComboBox)
        .chrome(roles::BUTTON, roles::DEFAULT, Metric::Radius)
        .flyout(body)
        .row(inner(text, TypeRole::Body))
}

/// A read-only level.
///
/// **No hit entry at all**, which is what "not interactive" has to mean here: it is the hit
/// entry that mints a control row, a front-side row and a slot in the array every pointer
/// sample is resolved against — and a column of meters is exactly where that adds up. Its
/// level springs, because a meter carries momentum where a value the application wrote must
/// be where it was put.
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

/// What a control whose whole purpose is a value washes with. The accent is already the
/// colour it moves in, so hovering it in ink would be a second hue for one gesture.
const fn accent_wash() -> StatePolicy {
    StatePolicy::Wash {
        hover: Wash::Accent,
        press: Wash::Accent,
    }
}

const _: () = {
    // Every widget above names a table this crate owns, and a variant method addresses a
    // row of one. An empty table would resolve every label through the fallback ink, which
    // is a different colour from the one the variant chose and nothing would say so.
    assert!(!roles::BUTTON.is_empty());
    assert!(!roles::SURFACE.is_empty());
    assert!(!roles::TRACK.is_empty());
    assert!(!roles::FIELD.is_empty());
    assert!(!roles::OPTION.is_empty());
};
