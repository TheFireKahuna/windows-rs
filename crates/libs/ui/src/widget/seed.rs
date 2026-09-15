//! The widget seeds: each writes a slot and returns an element.
//!
//! A seed does not call `Model`, resolve a colour or build a style. The lowering does all
//! three, which is what keeps a widget to one short function.
//!
//! A composition is a function returning a tree of these. It is where `badge`, `nav`, `tabs`
//! and every screen an application assembles for itself live, and it adds nothing here.

use crate::build::arena::{FULL, MaskSeed, Part};
use crate::build::{Button, El, Path, View};
use crate::layout::{Align, Len, Over, Preset};
use crate::role::{Fill, Metric, Role, Text, TypeRole};
use crate::signal::{Cell, Signal};
use crate::widget::{Flow, Interaction, Range, StatePolicy, TextSource, UiaRole, Wash, roles};
use windows_scene::{GeomId, HitFlags};

/// Text appearance chosen by an application recipe. Typography remains unresolved until layout.
#[derive(Copy, Clone, Debug)]
pub struct TextStyle {
    pub typography: TypeRole,
    /// Standalone ink. An enclosing control's chrome governs the label colour;
    /// `None` otherwise uses primary text.
    pub ink: Option<Role>,
    pub flow: Flow,
    pub caps: bool,
}

impl TextStyle {
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
        }
    }
}

/// A compound text element with one automation peer. Add its runs with `control_text`;
/// the group derives one accessible name from those runs.
#[must_use]
pub fn text_group() -> View {
    El::seed(Preset::Bare).hit(HitFlags::UIA, UiaRole::Text)
}

/// A text element with its own accessible name, using an application-owned recipe.
#[must_use]
pub fn styled_text(s: impl Into<TextSource>, style: TextStyle) -> View {
    control_text(s, style).hit(HitFlags::UIA, UiaRole::Text)
}

/// Text inside a control or compound text element. Its enclosing element owns the
/// automation peer and derives its accessible name from this run.
#[must_use]
pub fn control_text(s: impl Into<TextSource>, style: TextStyle) -> View {
    El::seed(Preset::Text).text_seed(
        s.into(),
        style.typography,
        style.ink,
        style.flow,
        style.caps,
    )
}

// ── text ─────────────────────────────────────────────────────────────────────────
//
// Each text seed carries its own role and type ramp, and none takes a colour
// or a size.

/// Body copy.
#[must_use]
pub fn text(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Body, Text::Primary, Flow::Line, false)
}

/// A heading.
#[must_use]
pub fn title(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Title, Text::Primary, Flow::Line, false)
}

/// A field's or a group's name, set secondary to the thing it labels.
///
#[must_use]
pub fn label(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Label, Text::Secondary, Flow::Line, false)
}

/// A single label read from top to bottom, with its measured axes exchanged.
#[must_use]
pub fn vertical_label(s: impl Into<TextSource>) -> View {
    label(s).vertical_text()
}

/// Supporting prose. A coverage tile covers one line, so wrapping mounts a group with
/// one sprite per line.
#[must_use]
pub fn caption(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Caption, Text::Tertiary, Flow::Wrap, false)
}

/// A short statement set beside something else: a figure's name on a band, a unit after a
/// read-out, the word a state is reported in.
///
/// The [`caption`] rung on one line. Every other rung is a line and only that one wraps, so
/// this is what the reading rung offers a row whose items are laid out across it: a wrapping
/// run in a row measures at its longest word and breaks inside it, which is a name split over
/// two lines in a band one line deep.
#[must_use]
pub fn note(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Caption, Text::Tertiary, Flow::Line, false)
}

/// Annotation on a data surface: a unit, an index, a channel name, a coefficient.
///
/// Tertiary, because it names something that is itself on the surface and must not
/// outweigh it.
#[must_use]
pub fn micro(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Micro, Text::Tertiary, Flow::Line, false)
}

/// A read-out, in tabular figures, so its digits do not shift width as it changes.
#[must_use]
pub fn mono(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Mono, Text::Primary, Flow::Line, false)
}

/// Read-only source text: monospaced, preserving line breaks and wrapping to its container.
#[must_use]
pub fn code(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Mono, Text::Primary, Flow::Wrap, false)
}

/// A prominent instrument readout, using the display rung of the type ramp.
#[must_use]
pub fn display(s: impl Into<TextSource>) -> View {
    run(s, TypeRole::Display, Text::Primary, Flow::Line, false)
}

/// Builds a text run: the shared body of the text widgets and of every label inside a
/// control.
///
/// `ink` is stated here and overridden at mount by the enclosing widget's chrome row where
/// there is one, so a button's variant reaches its text without the text naming a variant.
fn run(s: impl Into<TextSource>, ramp: TypeRole, ink: Text, flow: Flow, caps: bool) -> View {
    El::seed(Preset::Text)
        .text_seed(s.into(), ramp, Some(Role::Text(ink)), flow, caps)
        // `UIA` and nothing else: a run has no gesture, takes no focus and routes no
        // pointer, so the hit scan skips it on one flags test. With no entry at all it
        // would have no automation peer, and a screen of text would read as empty.
        .hit(HitFlags::UIA, UiaRole::Text)
}

/// A label whose colour is the enclosing control's rather than its own.
///
/// It mints no automation peer: the control it sits in derives its accessible name from this
/// text, so a peer would have a reader announce the control's name twice.
fn inner(s: impl Into<TextSource>, ramp: TypeRole, caps: bool) -> View {
    El::seed(Preset::Text).text_seed(s.into(), ramp, None, Flow::Line, caps)
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

/// A square-edged surface that meets adjoining regions without rounded gaps.
#[must_use]
pub fn sheet(key: &'static str) -> View {
    El::seed(Preset::Bare)
        .sprite(
            MaskSeed::Box { radius: None },
            Role::Fill(Fill::Surface),
            Part::Fill,
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
        Metric::RadiusSurface,
    )
}

// ── interactive ──────────────────────────────────────────────────────────────────

/// A press. Four variants over one table, chosen on [`El<Button>`](Button).
#[must_use]
pub fn button(text: impl Into<TextSource>) -> El<Button> {
    button_with(text, TextStyle::new(TypeRole::Body))
}

/// A button using application-owned typography and casing, with one label run.
#[must_use]
pub fn button_with(text: impl Into<TextSource>, style: TextStyle) -> El<Button> {
    control(UiaRole::Button)
        .chrome(roles::BUTTON, roles::DEFAULT, Metric::Radius)
        .row(control_text(text, style))
}

/// A button joined flush to a containing edge. Placement remains the caller's job;
/// its fill, border and interaction wash share the two exposed corners.
#[must_use]
pub fn edge_button(
    text: impl Into<TextSource>,
    edge: crate::layout::Edge,
    radius: Metric,
) -> El<Button> {
    control(UiaRole::Button)
        .chrome(roles::BUTTON, roles::DEFAULT, radius)
        .attached(edge)
        .row(inner(text, TypeRole::Body, false))
}

/// A press with no text, so [`name`](El::name) is required: there is nothing to derive an
/// accessible name from.
#[must_use]
pub fn icon_button(icon: GeomId) -> El<Button> {
    control(UiaRole::Button)
        .chrome(roles::BUTTON, roles::GHOST, Metric::RadiusPill)
        .row(path(icon).ink())
}

/// How long a switch's track is, as a multiple of [`Metric::TrackH`].
///
/// Enough for the knob and most of a knob's width of travel, which is what reads as a switch
/// rather than as an indicator dot. A proportion and not a rung: the palette says how big a
/// switch is, and this says what shape one is.
pub(crate) const TRACK_ASPECT: f32 = 1.7;

/// The knob's diameter on a switch, as a fraction of the track's height.
///
/// Above [`KNOB_OF_TRACK`], which is a groove's. A groove is a line the thumb rides along and
/// the track either side of it is the value; a switch is a capsule the knob nearly fills, and
/// the sliver of track left at the ends is what says which end it is at.
pub(crate) const TOGGLE_KNOB_OF_TRACK: f32 = 0.8;

/// The fill a switch takes when it is on.
///
/// Read out of [`roles::TRACK_ON`] rather than named again here, so the row that says what an
/// on switch is is the row this paints.
const TRACK_ON_FILL: Role = match roles::TRACK[roles::TRACK_ON as usize].fill {
    Some(fill) => Role::Fill(fill),
    // The row is authored with a fill. Stated so the constant is total.
    None => Role::Fill(Fill::Accent),
};

/// A two-state switch. The knob is a sprite sprung between the ends of its track, so the
/// transition is a compositor animation and costs no frame after the one that started it.
///
/// It states its whole box, which is the one control here that has to. A control's defaults
/// are a row's: a floor of one row height, a label's padding, and its content centred. A
/// switch is shorter than a row, the knob is wider than what that padding leaves — the layout
/// shrinks it into a lens — and it rests at the start of a track its travel is measured from
/// the start of.
///
/// Every one of those numbers comes off [`Metric::TrackH`]. A caller restating the box
/// therefore moves the track and leaves the knob where the palette put it, which is why the
/// whole shape is stated here rather than left to a call site.
#[must_use]
pub fn toggle<M>(on: impl Signal<bool, M> + Copy + 'static) -> View {
    control::<crate::build::Any>(UiaRole::CheckBox)
        .chrome(roles::TRACK, roles::TRACK_OFF, Metric::RadiusPill)
        .selected(on)
        .interaction(Interaction::Press)
        .act(crate::build::arena::Act::ScalarSource(Box::new(
            move || (f32::from(on.read()), 0),
        )))
        .row((
            // The on state's fill, as a covering plate whose opacity carries the state. A
            // variant is resolved at mount, so the alternative is rebuilding the control on
            // every press; and the state swap `selected` performs resolves a *wash*, which
            // is what a selected row takes and not what a switch that is on is.
            //
            // It is out of flow, so it neither takes the track's padding nor displaces the
            // knob beside it. The interaction wash is emitted below a node's children, so an
            // on switch does not lighten under the pointer — which is what the design
            // reference does too: the switch reports its own state and nothing else.
            El::<crate::build::Any>::seed(Preset::Bare)
                .plate(Metric::RadiusPill, TRACK_ON_FILL, FULL)
                .cover()
                .opacity(move || f32::from(u8::from(on.read()))),
            knob_sprite(1.0, TOGGLE_KNOB_OF_TRACK),
        ))
        .min_height(Len::Zero)
        .height(Metric::TrackH)
        .width(Len::Times(Metric::TrackH, TRACK_ASPECT))
        .padding(Len::Times(
            Metric::TrackH,
            knob_inset_of(TOGGLE_KNOB_OF_TRACK),
        ))
        .justify(Align::Start)
        .align(Align::Center)
        // A switch is one fixed shape. Shrinkable, a tight row takes the width off the track
        // first, and a track under twice its own radius renders as a lens.
        .no_shrink()
}

/// Paint and origin of a slider's retained value stroke.
#[derive(Copy, Clone, Debug, Default)]
pub struct SliderStyle {
    /// Value the fill grows from; the range minimum when absent.
    pub origin: Option<f64>,
    /// A fixed gradient across the whole rail; trimmed rather than rescaled.
    pub ramp: Option<windows_scene::RampId>,
}

/// A value along a track. The thumb moves front-side in the tick that saw the contact,
/// and the number reaches the application afterwards.
#[must_use]
pub fn slider<M>(
    value: impl Signal<f64, M> + Copy + 'static,
    range: Range,
    style: SliderStyle,
) -> View {
    let extent = crate::layout::probe();
    let fraction = range.fraction(style.origin.unwrap_or(range.min));
    let origin = if range.vertical {
        1.0 - fraction
    } else {
        fraction
    };
    let [geometry, marker] =
        crate::build::local_geometries(extent, [3; 2], move |[rail, marker], size, scope| {
            use windows_numerics::Vector2;
            use windows_scene::PathVerb;
            let at = |along, cross| {
                if range.vertical {
                    Vector2 { x: cross, y: along }
                } else {
                    Vector2 { x: along, y: cross }
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
                out.push(PathVerb::Segment { from: a, to: b });
            }
        });
    let trail = path(geometry)
        .slider_trail(origin, style.ramp)
        .probed(extent)
        .width(Len::Pct(1.0))
        .height(Len::Pct(1.0))
        .erase();

    let tick = path(marker)
        .ink_stroke(Metric::HairlineW)
        .opacity(if style.origin.is_some() { 0.15 } else { 0.0 })
        .cover();
    let rail = El::<crate::build::Any>::seed(Preset::Bare).sprite(
        MaskSeed::Box {
            radius: Some(Metric::Radius.into()),
        },
        Role::Fill(Fill::Pressed),
        Part::Static,
    );
    let rail = if range.vertical {
        crate::layout::row(
            crate::layout::grid((
                rail.width(Metric::SliderRailH).height(Len::Pct(1.0)),
                trail.cover(),
                tick,
            ))
            .cols([crate::layout::Track::Fr(1.0)])
            .rows([crate::layout::Track::Fr(1.0)])
            .justify(Align::Center)
            .width(Metric::RowH)
            .height(Len::Pct(1.0)),
        )
        .justify(Align::Center)
        .padding_xy(Len::Zero, Metric::SliderThumb)
    } else {
        crate::layout::row(
            crate::layout::grid((
                rail.height(Metric::SliderRailH).width(Len::Pct(1.0)),
                trail.cover(),
                tick,
            ))
            .cols([crate::layout::Track::Fr(1.0)])
            .rows([crate::layout::Track::Fr(1.0)])
            .align(Align::Center)
            .height(Metric::RowH)
            .width(Len::Pct(1.0)),
        )
        .align(Align::Center)
        .padding_xy(Metric::SliderThumb, Len::Zero)
    }
    .cover();
    let thumb = El::<crate::build::Any>::seed(Preset::Bare)
        .thumb(
            Len::Times(Metric::SliderThumb, 0.5),
            Role::Text(Text::Primary),
        )
        .width(Metric::SliderThumb)
        .height(Metric::SliderThumb)
        .no_shrink();
    let control = control::<crate::build::Any>(UiaRole::Slider)
        .chrome(roles::OPTION, 0, Metric::RadiusPill)
        .slide(value, range)
        .state(accent_wash());
    let inset = Len::Times(Metric::SliderThumb, 0.5);
    let control = if range.vertical {
        control
            .stack((rail, thumb))
            .width(Metric::RowH)
            .padding_xy(Len::Zero, inset)
    } else {
        control
            .row((rail, thumb))
            .height(Metric::RowH)
            .padding_xy(inset, Len::Zero)
    };
    control
        .gap(Len::Zero)
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
        .turn(value, range)
        .state(accent_wash())
        .stack(
            El::<crate::build::Any>::seed(Preset::Bare)
                .thumb(Metric::RadiusPill, Role::Fill(Fill::Accent))
                // A fraction and not an angle: whichever side is moving the part applies
                // the sweep, through `angle_of`, so a committed value and a live drag land
                // the knob in the same place.
                .scalar_part(super::ScalarPart::Rotation {
                    from: 0.0,
                    to: super::TURN_SWEEP,
                }),
        )
}

/// How far the track insets its options, in device pixels.
///
/// Two, which is the smallest inset that reads as a rail holding the selected option rather
/// than as a fill flush against it. Below the spacing scale on purpose: [`Metric::SpaceXs`]
/// is the tightest gap between two separate things, and this is the seam inside one control.
const GROOVE_INSET_PX: f32 = 2.0;

/// A choice reads canonical state and dispatches a user selection. A Cell or a
/// `(read, dispatch)` pair implements it; reducers need no shadow selection Cell.
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

/// One choice of several, laid out as a row inside a groove.
///
/// Selection is [`ModelState`](super::ModelState) — a discrete paint swap at event rate —
/// rather than a variant, because it is state any control can be in and not something only
/// this widget has.
///
/// The options sit at zero gap in a track of their own. A picker is one control naming one
/// value, and options separated by the row gap read as that many buttons; the track is what
/// says the choice is exclusive, and it is what the selected option's fill slides within.
#[must_use]
pub fn segmented<T>(
    value: impl Choice<T> + Copy + 'static,
    options: &'static [(&'static str, T)],
) -> View
where
    T: Copy + PartialEq + 'static,
{
    rail(
        value,
        options,
        Metric::Radius,
        Metric::SpaceSm,
        TypeRole::Caption,
    )
}

/// The same choice, drawn as a stadium: a rounded rail with a rounded slab inside it.
///
/// The shape is the difference and it carries a meaning. A [`segmented`] names a value on a
/// surface already full of controls, so it takes the control corner every button beside it
/// takes. A pill rail names *which of two things the window is*, sits alone in a band, and is
/// read at a glance — which is what the roomier option inset and the fully round ends are
/// for. [`Metric::RadiusPill`] is a palette rung and names a segment rail among its
/// consumers, so this reaches an authored value rather than a shape stated here.
#[must_use]
pub fn pills<T>(
    value: impl Choice<T> + Copy + 'static,
    options: &'static [(&'static str, T)],
) -> View
where
    T: Copy + PartialEq + 'static,
{
    rail(
        value,
        options,
        Metric::RadiusPill,
        Metric::SpaceMd,
        TypeRole::Body,
    )
}

/// Builds a rail of exclusive options at `radius`, with `inset` either side of each label,
/// naming them on `ramp`.
///
/// An option names a value and is set in mixed case, so the rung is a reading rung and never
/// [`TypeRole::Label`]: a rail of capitals reads as a row of headings over an empty surface.
/// The two rails differ by one rung, which is the same difference their insets and their
/// corners carry.
fn rail<T>(
    value: impl Choice<T> + Copy + 'static,
    options: &'static [(&'static str, T)],
    radius: Metric,
    inset: Metric,
    ramp: TypeRole,
) -> View
where
    T: Copy + PartialEq + 'static,
{
    // The one allocation in the widget set: a child count that comes from a slice cannot be
    // a tuple, which is what static structure elsewhere uses.
    let kids: Vec<View> = options
        .iter()
        .map(|&(name, option)| {
            control::<crate::build::Any>(UiaRole::RadioButton)
                .chrome(roles::OPTION, 0, radius)
                .selected(move || value.selected() == option)
                .on_click(move || value.choose(option))
                // The group is one row tall and stretches its options, so an option that
                // also carried the row height as a floor would push the track past it by
                // twice the inset.
                .min_height(Len::Zero)
                // Horizontal only. The option's height is the track's, so vertical padding
                // would be a second claim on it; the label is centred in what it gets.
                .over(Over::PaddingXY(Len::Metric(inset), Len::Zero))
                .row(inner(name, ramp, false))
        })
        .collect();
    El::seed(Preset::Bare)
        .chrome(roles::GROOVE, 0, radius)
        .row(kids)
        .height(Metric::RowH)
        .padding(Len::Times(Metric::HairlineW, GROOVE_INSET_PX))
        .gap(Len::Zero)
        // Stretch, so every option is the track's height and the selected fill is a band
        // across it rather than a chip floating inside it.
        .align(Align::Stretch)
        // An automation container and nothing else: the options route the pointer.
        .hit(HitFlags::NONE, UiaRole::List)
}

/// A text-editable field. Text services own the caret; this declares the target.
#[must_use]
pub fn field(value: impl Into<TextSource>) -> El<crate::build::Field> {
    field_with(value, TextStyle::new(TypeRole::Body))
}

/// A text field with an application-owned text recipe; draft ownership stays in TSF.
#[must_use]
pub fn field_with(value: impl Into<TextSource>, style: TextStyle) -> El<crate::build::Field> {
    El::<crate::build::Field>::seed(Preset::Bare)
        .height(Metric::RowH)
        .min_width(Len::Times(Metric::RowH, 4.0))
        .chrome(roles::FIELD, 0, Metric::Radius)
        .control()
        .hit(
            HitFlags::INTERACTIVE | HitFlags::GESTURE | HitFlags::TEXT,
            UiaRole::Edit,
        )
        .state(ink_wash())
        .row(control_text("", style))
        .field_source(value.into())
}

/// A button that opens a list of options.
///
/// A widget rather than a composition, because its automation pattern is `ComboBox` and only
/// a widget declares one.
#[must_use]
pub fn select(text: impl Into<TextSource>, body: impl Fn() -> View + 'static) -> El<Button> {
    control(UiaRole::ComboBox)
        .chrome(roles::BUTTON, roles::DEFAULT, Metric::Radius)
        // The plate is the picker's, not the body's. A menu is always a detached surface, and
        // a body handed in without one draws its text straight over whatever it opened above.
        // `El::flyout` stays bare for the cases that want to state their own.
        .flyout(move || flyout().stack(body()).erase())
        .row(inner(text, TypeRole::Body, false))
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

/// Returns how far a knob of diameter `of_track` sits from its track's edge, as a fraction of
/// the track's height.
///
/// Half of what that height leaves once the knob has taken its share, so the gap is the same
/// on all four sides. The track states it as padding, which is both what the knob rests at
/// and what its travel is measured between, so one call sets both.
const fn knob_inset_of(of_track: f32) -> f32 {
    (1.0 - of_track) * 0.5
}

/// The moving part of a track `track_rows` row heights tall, at `of_track` of its height.
///
/// `track_rows` is in row heights because a groove is a row tall. A switch states its own box
/// off [`Metric::TrackH`] and passes one, so the fraction it hands in is of that box.
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
fn knob_sprite(track_rows: f32, of_track: f32) -> View {
    let rows = track_rows * of_track;
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
