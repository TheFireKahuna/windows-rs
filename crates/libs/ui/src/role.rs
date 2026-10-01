//! Roles: what a widget names instead of a colour, a size, a spacing or an alignment.
//!
//! A widget names a [`Role`], the enclosing [`Scope`] says what that role means there, and
//! [`resolve`] turns the pair into light. A call site states neither a colour nor a size.
//!
//! 1. **One level.** A role resolves against the enclosing scope rather than an inheritance
//!    chain, so nothing resolves at a distance.
//! 2. **Total.** Every `(role, scope)` pair has a value. [`resolve`] returns a
//!    [`Radiance`], not an `Option`, so a missing token is unrepresentable.
//! 3. **Scopes nest by construction.** A card *is* a scope push, so nesting is lexical and
//!    there is no ambient inheritance to fall into.
//! 4. **[`Data`](Role::Data) roles carry no polarity.** Band hues, series colours and the
//!    spectrum ramp are chromatic and shared between light and dark.
//!
//! # Everything here is authored light
//!
//! [`resolve`] returns [`Radiance`]: scene-referred, linear Rec.2020, absolute cd/m²,
//! unbounded. Nothing at this layer has met a display, and there is no way to ask it for
//! anything else. The display transform runs once, at the draw choke, on the way to the
//! compositor.

use windows_color::{Gamut, Radiance};
use windows_text::FontSpec;

pub use crate::layout::WidthClass;

/// How far a surface sits off the window's own plane.
///
/// Selects which rung of the surface ladder a scope's fills come from. The palette authors
/// the ladder; this is neither a shadow depth nor a z-index.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Elevation {
    /// The window's own plane. Panels.
    Base,
    /// A card.
    Raised,
    /// A drawer or a sheet — attached, and above the content it covers.
    Overlay,
    /// A flyout, menu or tooltip — detached, and above everything.
    Flyout,
}

/// Which way round this window's palette runs.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Polarity {
    Dark,
    Light,
}

/// How tight the layout is: the user's preference, not the container's situation.
///
/// It applies to every scope within its host, so no call site branches on it.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Density {
    Comfortable,
    Compact,
}

/// Which accent family. The application names them; this crate only carries the choice.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct AccentId(pub u8);

/// Immutable palette identity carried by a window's scopes. No process-global install.
#[derive(Copy, Clone)]
pub struct PaletteRef(pub &'static dyn Palette);

impl PartialEq for PaletteRef {
    /// Identity, not content: a palette is a `&'static` table, and two scopes carry the same
    /// palette when they point at the same one.
    fn eq(&self, other: &Self) -> bool {
        core::ptr::addr_eq(self.0, other.0)
    }
}

impl core::fmt::Debug for PaletteRef {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Palette")
            .field(&(self.0 as *const dyn Palette))
            .finish()
    }
}

/// Everything a role resolves against.
///
/// The scope axes answer different questions. [`Density`] is what the user asked for and
/// applies throughout its host; [`WidthClass`] is how much room this container got, so one
/// card is `Wide` in a full-width row and `Narrow` in a detail pane of the same window at the
/// same instant. The palette resolves both in one function, so a rule such as "compact and
/// narrow takes the tightest gap, but never below the touch floor" is stated once rather than
/// as two conditionals per call site.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Scope {
    pub palette: PaletteRef,
    /// Which rung of the surface ladder fills come from.
    pub elevation: Elevation,
    /// Which way round the palette runs.
    pub polarity: Polarity,
    /// Which accent family the palette draws from.
    pub accent: AccentId,
    /// How tight the layout is.
    pub density: Density,
    /// How much room this container got.
    pub width: WidthClass,
}

/// The width class every colour resolves against, whatever the container's actual extent.
///
/// Its value is arbitrary; only its constancy matters. Applied by [`Scope::for_paint`].
pub const PAINT_WIDTH: WidthClass = WidthClass::Wide;

impl Scope {
    /// Returns a dark root scope with the given palette, accent and density.
    ///
    /// `width` starts at [`WidthClass::Wide`] and is replaced by the first responsive
    /// container that classifies itself.
    #[must_use]
    pub fn root(palette: &'static dyn Palette, accent: AccentId, density: Density) -> Self {
        Self {
            palette: PaletteRef(palette),
            elevation: Elevation::Base,
            polarity: Polarity::Dark,
            accent,
            density,
            width: WidthClass::Wide,
        }
    }

    /// Returns the same scope at `elevation`. What `card` and `flyout` push.
    #[must_use]
    pub const fn elevate(self, elevation: Elevation) -> Self {
        Self { elevation, ..self }
    }

    /// Returns the same scope at a classified width. What a responsive container applies to
    /// its subtree.
    #[must_use]
    pub const fn at_width(self, width: WidthClass) -> Self {
        Self { width, ..self }
    }

    /// Returns the same scope at `density`.
    #[must_use]
    pub const fn at_density(self, density: Density) -> Self {
        Self { density, ..self }
    }

    /// Rebases window-owned axes, preserving lexical elevation and solved width.
    #[must_use]
    pub const fn in_theme(self, root: Self) -> Self {
        Self {
            elevation: self.elevation,
            width: self.width,
            ..root
        }
    }

    /// Returns this scope with [`width`](Self::width) pinned to [`PAINT_WIDTH`].
    ///
    /// The paint path's only entry. Only [`Palette::metric`] and [`Palette::typography`] read
    /// the width class; a colour never does, and pinning the axis here is what holds even for
    /// a palette whose colour method reads `scope.width`.
    ///
    /// The class is not known at mount: a responsive container resolves it inside the solve
    /// and re-resolves it whenever a window crosses a threshold. A width-dependent colour
    /// would therefore make dragging a window edge re-key every rasterized cell and re-source
    /// every mask brush in the subtree.
    #[must_use]
    pub const fn for_paint(self) -> Self {
        Self {
            width: PAINT_WIDTH,
            ..self
        }
    }
}

/// Foreground roles.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Text {
    Primary,
    Secondary,
    Tertiary,
    Disabled,
    Accent,
    /// On top of an [`Fill::Accent`] surface.
    OnAccent,
}

/// Surface roles.
///
/// [`Hover`](Self::Hover), [`Pressed`](Self::Pressed) and [`Selected`](Self::Selected) are the
/// same surface resolved in a different interaction state rather than extra colour
/// parameters. The scene ramps between the two resolutions on the event; the application never
/// writes a hover colour.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Fill {
    Surface,
    /// Fills recessed fields and tracks with an opaque surface.
    Sunken,
    /// Supplies the foreground wash and its hover opacity.
    Hover,
    /// Supplies the foreground wash and its pressed opacity.
    Pressed,
    Selected,
    Accent,
    AccentSubtle,
}

/// Line roles: hairlines, dividers, focus rings.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Stroke {
    Subtle,
    Default,
    Focus,
    Accent,
}

/// A chromatic, application-defined role.
///
/// The chromatic extension, carrying no [`Polarity`]: a band hue, a series colour and the
/// spectrum ramp mean the same thing in light and dark. This crate never interprets the
/// number.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct DataRole(pub u16);

/// A rung of the type ramp.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum TypeRole {
    Display,
    Title,
    BodyStrong,
    Body,
    Label,
    Caption,
    /// Annotation on a data surface: a unit beside a figure, an index on a tile, a channel
    /// name on a wire, a coefficient on a crossing.
    ///
    /// The rung below [`Caption`](Self::Caption), and the smallest the ramp offers. It exists
    /// because a plot's own labelling competes with the plot for room, and setting it at the
    /// caption size is what makes a dense surface read as crowded.
    Micro,
    /// Tabular figures. What a read-out is set in, so its digits do not shift width as it
    /// changes.
    Mono,
    /// An application-owned token, resolved in the same scope as built-in roles.
    Custom(&'static ScopedToken<FontSpec>),
}

/// A scalar the palette owns, in DIPs unless the name says otherwise.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Metric {
    /// The spacing scale, narrowest first. A rung between two named ones carries both
    /// names.
    Space3xs,
    Space2xs,
    SpaceXs,
    SpaceXsSm,
    SpaceSm,
    SpaceSmMd,
    SpaceMd,
    SpaceMdLg,
    SpaceLg,
    SpaceXl,
    Space2xl,
    /// The radius of a control: a button, a field, a segmented option, a menu option.
    Radius,
    /// The radius of a surface: a card, a panel, a flyout, a plate.
    ///
    /// Separate from [`Radius`](Self::Radius) because a surface and the controls sitting on it
    /// round by different amounts. A scope cannot carry that difference: a control pushes no
    /// elevation, so a control on a card resolves at the card's own scope and one value
    /// answers both.
    RadiusSurface,
    /// The radius of a pill: a switch's track or a segment rail. The palette supplies a
    /// radius; geometry caps it at half the box.
    RadiusPill,
    /// A control's row height, and the floor a touch target is inflated to.
    RowH,
    /// A switch's track: the capsule its knob rides in.
    ///
    /// Under [`RowH`](Self::RowH). A switch marks what a row already says rather than being
    /// what the row is sized for, and one as tall as the row reads as a second button beside
    /// whatever else the row carries.
    ///
    /// It is the switch's one free variable. How long the track is and how much of it the knob
    /// fills are proportions of this — a switch is one shape, and its inset is both where the
    /// knob rests and what its travel is measured between, so a caller free to set them
    /// separately can put a knob outside the track it rides in. A slider's groove is a
    /// different shape and does not read this rung.
    TrackH,
    /// The narrowest a card is authored to hold its content.
    CardMinW,
    /// The shortest a card is authored to hold its content.
    CardMinH,
    /// Thickness of a slider's rail, independent of its pointer target.
    SliderRailH,
    /// Diameter of a slider's thumb.
    SliderThumb,
    /// One device pixel at the current scale, expressed in DIPs by the palette.
    HairlineW,
    /// A drawn border, which is a design decision rather than a device property.
    BorderW,
    /// An application-owned token, resolved in the same scope as built-in roles.
    Custom(&'static ScopedToken<f32>),
}

/// How many metrics the palette owns outright, which is every variant but
/// [`Custom`](Metric::Custom).
pub const BUILTIN_METRICS: usize = 22;

impl Metric {
    /// Every palette-owned metric, in the order a cache row indexes them.
    ///
    /// The solve caches exactly these per width class, so the array's order is the cache's
    /// layout and [`row`](Self::row) is its index.
    pub const BUILTIN: [Self; BUILTIN_METRICS] = [
        Self::Space3xs,
        Self::Space2xs,
        Self::SpaceXs,
        Self::SpaceXsSm,
        Self::SpaceSm,
        Self::SpaceSmMd,
        Self::SpaceMd,
        Self::SpaceMdLg,
        Self::SpaceLg,
        Self::SpaceXl,
        Self::Space2xl,
        Self::Radius,
        Self::RadiusSurface,
        Self::RadiusPill,
        Self::RowH,
        Self::TrackH,
        Self::CardMinW,
        Self::CardMinH,
        Self::SliderRailH,
        Self::SliderThumb,
        Self::HairlineW,
        Self::BorderW,
    ];

    /// This metric's row in [`BUILTIN`](Self::BUILTIN), or `None` for a custom token.
    ///
    /// A custom token has no row: it resolves through its own `&'static` resolver rather than
    /// out of the cache.
    #[must_use]
    pub const fn row(self) -> Option<usize> {
        Some(match self {
            Self::Space3xs => 0,
            Self::Space2xs => 1,
            Self::SpaceXs => 2,
            Self::SpaceXsSm => 3,
            Self::SpaceSm => 4,
            Self::SpaceSmMd => 5,
            Self::SpaceMd => 6,
            Self::SpaceMdLg => 7,
            Self::SpaceLg => 8,
            Self::SpaceXl => 9,
            Self::Space2xl => 10,
            Self::Radius => 11,
            Self::RadiusSurface => 12,
            Self::RadiusPill => 13,
            Self::RowH => 14,
            Self::TrackH => 15,
            Self::CardMinW => 16,
            Self::CardMinH => 17,
            Self::SliderRailH => 18,
            Self::SliderThumb => 19,
            Self::HairlineW => 20,
            Self::BorderW => 21,
            Self::Custom(_) => return None,
        })
    }
}

/// What a widget names.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Role {
    Text(Text),
    Fill(Fill),
    Stroke(Stroke),
    Data(DataRole),
    /// Application-owned semantic paint and emission, resolved at width-independent scope.
    Custom(&'static ScopedToken<(Radiance, Emission)>),
}

impl From<Text> for Role {
    fn from(role: Text) -> Self {
        Self::Text(role)
    }
}

impl From<Fill> for Role {
    fn from(role: Fill) -> Self {
        Self::Fill(role)
    }
}

impl From<Stroke> for Role {
    fn from(role: Stroke) -> Self {
        Self::Stroke(role)
    }
}

impl From<DataRole> for Role {
    fn from(role: DataRole) -> Self {
        Self::Data(role)
    }
}

/// One silhouette's worth of a role's light.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Light {
    /// The Gaussian's σ, in DIPs.
    pub sigma: f32,
    /// How much of the role's own alpha the light carries, in `0.0..=1.0`.
    pub strength: f32,
}

impl Light {
    /// No light at all.
    pub const NONE: Self = Self::new(0.0, 0.0);

    /// Returns `sigma` DIPs of light at `strength` of the role's alpha.
    #[must_use]
    pub const fn new(sigma: f32, strength: f32) -> Self {
        Self { sigma, strength }
    }

    /// Returns whether this puts anything on screen.
    ///
    /// Either term at zero draws nothing, and a halo declared for it would be a composition
    /// object per sprite rendering no pixels.
    #[must_use]
    pub fn is_lit(self) -> bool {
        self.sigma > 0.0 && self.strength > 0.0
    }
}

/// What a halo is blurring, which decides which of a role's two lights it spends.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Silhouette {
    /// Glyphs, a hairline, a stroked path — thin, and mostly edge.
    Ink,
    /// A card, a plate, a button — solid, and mostly interior.
    Area,
}

/// How far a role's colour reaches past the silhouette it paints.
///
/// The **emissive tier**, distinct from the surface and ink tiers because it says nothing
/// about what colour a thing is — only how much light that colour spends outside itself.
/// Authored per role in the palette rather than per draw site, so a processor kind's badge and
/// the same kind's card cannot disagree about what that kind's light is.
///
/// A consumer with a reason it can name from *data* — a routed crossing's coefficient, a
/// selected row — scales what the palette authored. One that merely wants more light does not.
///
/// # Why two lights and not one
///
/// A drop shadow carries **no spread**, and a role's light lands differently on ink than on a
/// filled area because of it. A design system with spread states one blur for both and shrinks
/// the area's silhouette before blurring; without it, an area needs a tighter, dimmer blur to
/// avoid a hard rim, and ink needs a wider one to spread past its own stems. The difference is
/// the platform's rather than the designer's, which is why it is stated here once instead of
/// being compensated for at every call site.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Emission {
    /// Light where the silhouette is **ink**: glyphs, a hairline, a stroked path.
    pub ink: Light,
    /// Light where the silhouette is a **filled area**: a card, a plate, a button.
    pub area: Light,
}

impl Emission {
    /// A role that spends no light. What every role answers until the palette says otherwise.
    pub const NONE: Self = Self::new(Light::NONE, Light::NONE);

    /// Returns a role's light over each silhouette.
    #[must_use]
    pub const fn new(ink: Light, area: Light) -> Self {
        Self { ink, area }
    }

    /// Returns the light this role spends over `of`.
    #[must_use]
    pub const fn of(self, of: Silhouette) -> Light {
        match of {
            Silhouette::Ink => self.ink,
            Silhouette::Area => self.area,
        }
    }
}

/// A detached surface's occlusion, resolved once through the palette. The edge supplies the
/// direction; neither blur nor offset animates.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Shadow {
    /// Gaussian sigma in DIPs.
    pub sigma: f32,
    /// Distance cast outward from the named edge, in DIPs.
    pub offset: f32,
    /// Scene light and opacity of the shadow.
    pub light: Radiance,
}

/// A typed resolver with stable identity. Declare each token as a `static`.
///
/// The resolver must be pure, bounded and allocation-free. It may read only its scope and
/// immutable application data: layout calls it while solving, including after a container
/// changes width class. It must not access the build arena or install effects. A name is
/// diagnostic; two statics with the same name remain distinct tokens.
pub struct ScopedToken<T> {
    name: &'static str,
    resolve: fn(Scope) -> T,
}

impl<T> ScopedToken<T> {
    /// Defines a token that has an answer for every scope, without registration.
    pub const fn new(name: &'static str, resolve: fn(Scope) -> T) -> Self {
        Self { name, resolve }
    }

    /// Resolves at the caller's current scope, without caching a width class.
    pub fn resolve(&self, scope: Scope) -> T {
        (self.resolve)(scope)
    }

    /// Returns the diagnostic name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

// Identity, not content: a token is named by the `static` that holds it, so two resolvers
// returning the same value at every scope remain two tokens. `Metric`, `TypeRole` and `Role`
// derive `Eq` and `Hash` over a `&'static ScopedToken`, which is what these serve.
impl<T> PartialEq for ScopedToken<T> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self, other)
    }
}

impl<T> Eq for ScopedToken<T> {}

impl<T> core::hash::Hash for ScopedToken<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        core::ptr::hash(self, state);
    }
}

impl<T> core::fmt::Debug for ScopedToken<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("ScopedToken").field(&self.name).finish()
    }
}

/// What the application supplies. This crate never interprets a value it returns.
///
/// Every method is total: it returns a value rather than an `Option`, so a missing token is
/// unrepresentable. A palette that has not decided what a `(role, scope)` pair means still
/// answers.
///
/// `Send + Sync`: a palette is a lookup table over authored constants, and the present thread
/// resolves data roles for a region's own drawing. A derived shade — an accent ramp, a wash —
/// is a function over a base rather than stored state, so there is nothing to synchronize.
///
/// # The three colour methods may not read `scope.width`
///
/// Only [`metric`](Self::metric) and [`typography`](Self::typography) may. A width class is
/// resolved inside the solve and changes whenever a window crosses a threshold, so a colour
/// that depended on it would make a resize invalidate every rasterized cell in the subtree.
/// [`Scope::for_paint`] pins the axis on the way in, so a palette reading `scope.width` in a
/// colour method still cannot produce a width-dependent colour.
pub trait Palette: core::any::Any + Send + Sync + 'static {
    /// Returns the typography and dimensions of a transient text description.
    fn tooltip(&self, _scope: Scope) -> TooltipStyle {
        TooltipStyle::default()
    }
    /// Returns the light a foreground role resolves to in `scope`.
    fn text(&self, role: Text, scope: Scope) -> Radiance;

    /// Returns the light a surface role resolves to in `scope`.
    fn fill(&self, role: Fill, scope: Scope) -> Radiance;

    /// Returns the light a line role resolves to in `scope`.
    fn stroke(&self, role: Stroke, scope: Scope) -> Radiance;

    /// Returns the light an application-defined chromatic role resolves to.
    ///
    /// No [`Scope`]: a data role is chromatic and shared between polarities.
    fn data(&self, role: DataRole) -> Radiance;

    /// Returns how much light `role` spends outside the silhouette it paints.
    ///
    /// The whole [`Role`] rather than one of the four sub-enums, because emission crosses
    /// them: a processor kind's colour emits wherever it is painted, and the accent under a
    /// call to action emits as a fill. Total like the rest: a role that carries no light
    /// answers [`Emission::NONE`] rather than nothing. May not read `scope.width`.
    fn emission(&self, role: Role, scope: Scope) -> Emission;

    /// Returns the font a rung of the type ramp resolves to in `scope`.
    fn typography(&self, role: TypeRole, scope: Scope) -> FontSpec;

    /// Returns a detached surface's shadow. Width is pinned as for surface colours.
    fn shadow(&self, scope: Scope) -> Shadow;

    /// Returns a scalar the palette owns, in DIPs unless the name says otherwise.
    fn metric(&self, metric: Metric, scope: Scope) -> f32;

    /// Returns the brightest channel this palette authors in `gamut`'s primaries, in cd/m².
    ///
    /// The output transform's shoulder is built to reach this value, and anything authored
    /// above it clips. Taken in the primaries of the display the transform is for, because a
    /// channel's height depends on the primaries it is expressed in.
    fn content_peak_nits(&self, gamut: &Gamut, scope: Scope) -> f32;
}

/// Styles a non-interactive tooltip. Dimensions are in DIPs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TooltipStyle {
    pub typography: TypeRole,
    pub max_width: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub radius: Metric,
}

impl Default for TooltipStyle {
    fn default() -> Self {
        Self { typography: TypeRole::Caption, max_width: 320.0, padding_x: 10.0,
            padding_y: 7.0, radius: Metric::Radius }
    }
}

/// Resolves tooltip typography and dimensions from the owning palette.
pub fn tooltip(scope: Scope) -> TooltipStyle {
    scope.palette.0.tooltip(scope)
}

/// Returns the root scope a window resolves against.
///
/// Nothing process-wide is written: a palette is a `&'static` table and the scope carries the
/// pointer, so two windows can run two palettes at once and a test needs no teardown.
#[must_use]
pub fn install(palette: &'static dyn Palette, accent: AccentId, density: Density) -> Scope {
    Scope::root(palette, accent, density)
}

/// Returns the light `role` resolves to in `scope`.
///
/// Total: every pair has a value. The result is authored light — scene-referred, absolute
/// cd/m² — and no display transform has run on it.
#[must_use]
pub fn resolve(role: Role, scope: Scope) -> Radiance {
    let scope = scope.for_paint();
    match role {
        Role::Text(role) => scope.palette.0.text(role, scope),
        Role::Fill(role) => scope.palette.0.fill(role, scope),
        Role::Stroke(role) => scope.palette.0.stroke(role, scope),
        Role::Data(role) => scope.palette.0.data(role),
        Role::Custom(token) => token.resolve(scope).0,
    }
}

/// Returns how much light `role` spends past its own silhouette.
///
/// The emissive tier's one reader. A surface that draws its own pixels — a presentation
/// region, which has no [`Scope`] on the thread it draws on — reads this where it *can*
/// resolve a scope and pins the answer, the way it already pins a type rung.
#[must_use]
pub fn emission(role: Role, scope: Scope) -> Emission {
    let scope = scope.for_paint();
    match role {
        Role::Custom(token) => token.resolve(scope).1,
        role => scope.palette.0.emission(role, scope),
    }
}

/// Returns the font for a rung of the type ramp, resolved through the same scope the colours
/// use.
#[must_use]
pub fn typography(role: TypeRole, scope: Scope) -> FontSpec {
    match role {
        TypeRole::Custom(token) => token.resolve(scope),
        role => scope.palette.0.typography(role, scope),
    }
}

/// Returns a spacing, radius, row height or border width, in DIPs.
#[must_use]
pub fn metric(metric: Metric, scope: Scope) -> f32 {
    match metric {
        Metric::Custom(token) => token.resolve(scope),
        metric => scope.palette.0.metric(metric, scope),
    }
}

/// Returns the light a chromatic role resolves to.
///
/// No [`Scope`] axis is read: a data role is chromatic and shared between polarities, so it is
/// the one role that resolves the same everywhere. That is what lets a gradient be minted
/// where there is no scope to resolve against — a resource, rather than a sprite inside a
/// tree.
#[must_use]
pub fn data(role: DataRole, scope: Scope) -> Radiance {
    scope.palette.0.data(role)
}

/// Returns the palette's detached-surface shadow, with the paint width pinned.
#[must_use]
pub fn shadow(scope: Scope) -> Shadow {
    scope.palette.0.shadow(scope.for_paint())
}

/// Returns the brightest value the palette authors in `gamut`'s primaries, in cd/m².
///
/// Scope-free in effect: it is a property of the authored table and of the display's primaries
/// rather than of any site that uses it, and it is what the output transform's shoulder is
/// built to reach. Read from the palette rather than passed in, so the transform a window
/// builds and the values the palette authors answer to one peak.
#[must_use]
pub fn content_peak_nits(gamut: &Gamut, scope: Scope) -> f32 {
    scope.palette.0.content_peak_nits(gamut, scope.for_paint())
}

/// Returns the foreground wash: [`Text::Primary`] in `scope`, at `alpha`.
///
/// Hairlines, dividers and hover tints. It follows polarity because the foreground it is
/// derived from does.
#[must_use]
pub fn ink(alpha: f32, scope: Scope) -> Radiance {
    resolve(Role::Text(Text::Primary), scope).with_alpha(alpha)
}

/// Returns the background wash: the window's own base surface at `alpha`.
///
/// Scrims behind a modal, and overlays over content. Resolved at [`Elevation::Base`] rather
/// than at the caller's rung, because a scrim belongs to the window it dims and not to the
/// card that raised it.
#[must_use]
pub fn veil(alpha: f32, scope: Scope) -> Radiance {
    resolve(Role::Fill(Fill::Surface), scope.elevate(Elevation::Base)).with_alpha(alpha)
}

/// Returns the accent fill in `scope` at `alpha`: a selection tint, a subtle accent fill, a
/// focus glow.
#[must_use]
pub fn accent_wash(alpha: f32, scope: Scope) -> Radiance {
    resolve(Role::Fill(Fill::Accent), scope).with_alpha(alpha)
}

// A reference palette for production lowering tests, shared with the modules that resolve
// through it.
#[cfg(test)]
pub(crate) mod tests;
