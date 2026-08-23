//! The widget seeds: each writes a slot and returns an element.
//!
//! A seed does not call `Model`, resolve a colour or build a style. The lowering does all
//! three, which is what keeps a widget to one short function.
//!
//! A composition is a function returning a tree of these. It is where `badge`, `nav`, `tabs`
//! and every screen an application assembles for itself live, and it adds nothing here.

use crate::build::{Button, El, Path, View};
use crate::gesture::{DragDecl, GestureDecl};
use crate::layout::{Align, Len, Preset};
use crate::role::{DataRole, Fill, Metric, Role, Text, TypeRole};
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

/// Annotation on a data surface: a unit, an index, a channel name, a coefficient.
///
/// Tertiary, because it names something that is itself on the surface and must not
/// outweigh it.
#[must_use]
pub fn micro(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Micro, Text::Tertiary, Flow::Line)
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
    El::seed(Preset::Bare).chrome(roles::SURFACE, roles::SURFACE_PANEL, Metric::RadiusSurface)
}

/// A raised surface: a scope push to `Raised`, its own padding, radius and hairline, and
/// minimum metrics from the palette.
#[must_use]
pub fn card() -> View {
    El::seed(Preset::Bare)
        .surface(
            crate::role::Elevation::Raised,
            roles::SURFACE_CARD,
            Metric::RadiusSurface,
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
            Metric::RadiusSurface,
        )
        .key(key)
}

/// A value in a chromatic role: a plate in that role, under text in it.
///
/// What names a thing by its own colour — a processor kind's badge, a band's index, a
/// channel's name on its wire. It takes a [`DataRole`] and not a colour, so the authored
/// triple behind it stays in the application's palette table.
///
/// The plate is the role at [`CHIP_PLATE`] and the text is the role as resolved. One role and
/// two strengths, rather than a second token per kind: a plate is the hue at a fraction of
/// itself by construction, so a kind cannot carry a badge from one row and a plate from
/// another.
///
/// It pushes no scope, so a chip on a card resolves against the card.
#[must_use]
pub fn chip(role: DataRole, s: impl Into<TextSource>) -> View {
    El::<crate::build::Any>::seed(Preset::Bare)
        .plate(Metric::Radius, Role::Data(role), CHIP_PLATE)
        // A container and not a run: a run's box is its coverage tile, so padding one leaves
        // the glyphs drawn against a tile sized without it and the plate hugs them.
        .row(ink_run(s, TypeRole::Label, Role::Data(role)))
        // `UIA` and nothing else, as a text run takes: a chip names something, and routes no
        // pointer of its own.
        .hit(HitFlags::UIA, UiaRole::Text)
}

/// A text run painted in `role`, for a widget whose text colour is chromatic.
fn ink_run(s: impl Into<TextSource>, ramp: TypeRole, role: Role) -> View {
    El::seed(Preset::Text).text_seed_in(s.into(), ramp, role)
}

/// How much of its role a [`chip`]'s plate paints.
///
/// Low enough that the text over it, which is the same hue at full strength, still separates
/// from it.
pub const CHIP_PLATE: f32 = 0.15;

/// A detached surface above everything. The overlay layer anchors and dismisses it; this is
/// only what it looks like.
#[must_use]
pub fn flyout() -> View {
    El::seed(Preset::Bare).surface(
        crate::role::Elevation::Flyout,
        roles::SURFACE_FLYOUT,
        Metric::RadiusSurface,
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

/// A switch's height, in row heights.
///
/// Under a row. A switch marks what a row already says rather than being what the row is
/// sized for, and one as tall as the row reads as a second button beside the disclosure.
const TOGGLE_ROWS: f32 = 0.8;

/// How long a track is, as a multiple of its own height.
///
/// Enough for the knob and most of a knob's width of travel, which is what reads as a switch
/// rather than as an indicator dot.
const TRACK_ASPECT: f32 = 2.0;

/// A two-state switch. The knob is a sprite sprung between the ends of its track, so the
/// transition is a compositor animation and costs no frame after the one that started it.
///
/// It states its whole box, which is the one control here that has to. A control's defaults
/// are a row's: a floor of one row height, a label's padding, and its content centred. A
/// switch is shorter than a row, the knob is wider than what that padding leaves — the layout
/// shrinks it into a lens — and it rests at the start of a track its travel is measured from
/// the start of.
#[must_use]
pub fn toggle<M>(on: impl Signal<bool, M> + Copy + 'static) -> View {
    control::<crate::build::Any>(UiaRole::CheckBox)
        .chrome(roles::TRACK, roles::TRACK_OFF, Metric::RadiusPill)
        .selected(on)
        .interaction(Interaction::Press)
        .row(knob_sprite(TOGGLE_ROWS).along(false, move || f32::from(u8::from(on.read()))))
        .min_height(Len::Zero)
        .height(Len::Times(Metric::RowH, TOGGLE_ROWS))
        .width(Len::Times(Metric::RowH, TOGGLE_ROWS * TRACK_ASPECT))
        .padding(Len::Times(Metric::RowH, TOGGLE_ROWS * KNOB_INSET_OF_TRACK))
        .justify(Align::Start)
        .align(Align::Center)
        // A switch is one fixed shape. Shrinkable, a tight row takes the width off the track
        // first, and a track under twice its own radius renders as a lens.
        .no_shrink()
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
        .row(knob_sprite(1.0).along(range.vertical, move || range.fraction(value.read())))
        // The same inset and the same justification a toggle states, and for the same
        // reason: the thumb rests at the near inset and travels to the far one. A groove is
        // a row tall, so the fractions here are of the row rather than of a shorter track.
        .padding(Len::Times(Metric::RowH, KNOB_INSET_OF_TRACK))
        .justify(Align::Start)
        .align(Align::Center)
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

/// The knob's diameter, as a fraction of the track's height.
///
/// Under the whole of it, so the knob is inset from the track on the cross axis at every
/// density rather than at one.
const KNOB_OF_TRACK: f32 = 0.6;

/// How far the knob sits from the track's edge, as a fraction of the track's height.
///
/// Half of what that height leaves once the knob has taken its share, so the gap is the same
/// on all four sides. The track states it as padding, which is both what the knob rests at
/// and what its travel is measured between, so one constant sets both.
const KNOB_INSET_OF_TRACK: f32 = (1.0 - KNOB_OF_TRACK) * 0.5;

/// The moving part of a track `track_rows` row heights tall.
///
/// It states a definite square box. Without one it solves to nothing: a bare node has no
/// intrinsic size, so the knob is invisible and the travel `along` computes — the room the
/// track's insets leave, less the knob's own box — is the whole track.
///
/// Its radius is half that box, from the same constant, so the knob is a circle by
/// construction rather than by a radius stated beside a size that could drift from it.
///
/// It paints in [`Text::Primary`] and not in a surface: the track under it is the accent once
/// the control is on, and a knob in the surface colour reads as a hole punched through it
/// rather than as the part that moves. Not [`Text::OnAccent`] either — that is the ink a
/// palette picks to *read on* the accent, which in a dark scheme is the dark end.
fn knob_sprite(track_rows: f32) -> View {
    let rows = track_rows * KNOB_OF_TRACK;
    let side = Len::Times(Metric::RowH, rows);
    El::<crate::build::Any>::seed(Preset::Bare)
        .thumb(
            Len::Times(Metric::RowH, rows * 0.5),
            Role::Text(Text::Primary),
        )
        .width(side)
        .height(side)
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
