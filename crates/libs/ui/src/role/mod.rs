//! Roles: what a widget names instead of a colour, a size, a spacing or an alignment.
//!
//! A widget names a [`Role`], the enclosing [`Scope`] says what that role means there, and
//! [`resolve`] turns the pair into light. A call site states neither a colour nor a size.
//!
//! # Four properties of the resolution
//!
//! 1. **One level.** A role resolves against the enclosing scope rather than an inheritance
//!    chain, so nothing resolves at a distance.
//! 2. **Total.** Every `(role, scope)` pair has a value. [`resolve`] returns a
//!    [`Radiance`](windows_color::Radiance), not an `Option`, so a missing token is
//!    unrepresentable.
//! 3. **Scopes nest by construction.** A card *is* a scope push, so nesting is lexical and
//!    there is no ambient inheritance to fall into.
//! 4. **[`Data`](Role::Data) roles carry no polarity.** Band hues, series colours and
//!    the spectrum ramp are chromatic and shared between light and dark.
//!
//! # Everything here is authored light
//!
//! [`resolve`] returns [`Radiance`](windows_color::Radiance): scene-referred, linear
//! Rec.2020, absolute cd/m², unbounded. Nothing at this layer has met a display, and there is
//! no way to ask it for anything else. The display transform runs once, at the draw choke, on
//! the way to the compositor.

mod palette;
// The reference palette installs into a process-wide `OnceLock`, so the lowering's tests
// reach this module rather than install a second palette that would lose the race.
#[cfg(test)]
pub(crate) mod tests;

pub use palette::{
    Palette, accent_wash, content_peak_nits, ink, install, installed, metric, resolve, typography,
    veil,
};

use core::sync::atomic::{AtomicU8, Ordering};

pub use windows_scene::WidthClass;

/// How far a surface sits off the window's own plane.
///
/// Selects which rung of the surface ladder a scope's fills come from. The palette authors
/// the ladder; this is neither a shadow depth nor a z-index.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum Elevation {
    /// The window's own plane. Panels.
    #[default]
    Base,
    /// A card.
    Raised,
    /// A drawer or a sheet — attached, and above the content it covers.
    Overlay,
    /// A flyout, menu or tooltip — detached, and above everything.
    Flyout,
}

/// Which way round the palette runs. Process-global; see [`polarity`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum Polarity {
    #[default]
    Dark,
    Light,
}

/// How tight the layout is: the user's preference, not the container's situation.
///
/// It applies to every scope at once, so no call site branches on it.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

/// Which accent family. The application names them; this crate only carries the choice.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct AccentId(pub u8);

/// Everything a role resolves against.
///
/// The five axes answer different questions. [`Density`] is what the user asked for and
/// applies to every scope at once; [`WidthClass`] is how much room this container got, so one
/// card is `Wide` in a full-width row and `Narrow` in a detail pane of the same window at the
/// same instant. The palette resolves both in one function, so a rule such as "compact and
/// narrow takes the tightest gap, but never below the touch floor" is stated once rather than
/// as two conditionals per call site.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Scope {
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

impl Scope {
    /// Returns the window's own scope: [`Elevation::Base`], the current process
    /// [`polarity`], and the given accent and density.
    ///
    /// `width` starts at [`WidthClass::Wide`] and is replaced by the first responsive
    /// container that classifies itself.
    #[must_use]
    pub fn root(accent: AccentId, density: Density) -> Self {
        Self {
            elevation: Elevation::Base,
            polarity: polarity(),
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

    /// Returns the same scope with the process polarity re-read. Step one of a theme flip.
    #[must_use]
    pub fn repolarized(self) -> Self {
        Self {
            polarity: polarity(),
            ..self
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

/// The width class every colour resolves against, whatever the container's actual extent.
///
/// Its value is arbitrary; only its constancy matters. Applied by [`Scope::for_paint`].
pub const PAINT_WIDTH: WidthClass = WidthClass::Wide;

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
/// [`Hover`](Self::Hover), [`Pressed`](Self::Pressed) and [`Selected`](Self::Selected) are
/// the same surface resolved in a different interaction state rather than extra colour
/// parameters. The scene ramps between the two resolutions on the event; the application
/// never writes a hover colour.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Fill {
    Surface,
    Hover,
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
/// The one extensible arm, and the only one carrying no [`Polarity`]: a band hue, a series
/// colour and the spectrum ramp mean the same thing in light and dark. This crate never
/// interprets the number.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct DataRole(pub u16);

/// What a widget names.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Role {
    Text(Text),
    Fill(Fill),
    Stroke(Stroke),
    Data(DataRole),
}

/// A rung of the type ramp.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum TypeRole {
    Display,
    Title,
    Body,
    BodyStrong,
    Caption,
    Label,
    /// Tabular figures. What a read-out is set in, so its digits do not shift width as it
    /// changes.
    Mono,
}

/// A scalar the palette owns, in DIPs unless the name says otherwise.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Metric {
    SpaceXs,
    SpaceSm,
    SpaceMd,
    SpaceLg,
    /// The radius of a control: a button, a field, a segmented option, a menu option.
    Radius,
    /// The radius of a surface: a card, a panel, a flyout, a plate.
    ///
    /// Separate from [`Metric::Radius`] because a surface and the controls sitting on it
    /// round by different amounts. A scope cannot carry that difference: a control pushes
    /// no elevation, so a control on a card resolves at the card's own scope and one value
    /// answers both.
    RadiusSurface,
    /// The radius of a fully rounded control.
    ///
    /// A real value in the scale, and a small one. A composition corner radius caps at half
    /// the box, so a pill authored as a large sentinel renders a tall box as a football
    /// rather than as a stadium.
    RadiusPill,
    /// A control's row height, and the floor a touch target is inflated to.
    RowH,
    // ── the band ladder: the horizontal strips a shell is built from ─────────────
    /// A strip carrying text and no control: a status bar, a footnote rule.
    BandSm,
    /// A strip carrying controls: a toolbar, a section header, a card's own bar.
    BandMd,
    /// The window's own caption band.
    BandLg,
    /// A window command's width — the minimize, maximize and close controls of a custom
    /// caption band.
    CommandW,
    BorderW,
    CardMinW,
    CardMinH,
    /// One device pixel at the current scale, expressed in DIPs by the palette.
    HairlineW,
}

/// The process polarity. Read at every [`Scope::root`] and every [`Scope::repolarized`].
static POLARITY: AtomicU8 = AtomicU8::new(0);

/// The current process polarity.
#[must_use]
pub fn polarity() -> Polarity {
    // Relaxed: the byte publishes nothing but itself, and a flip reaches the interface
    // through the re-resolve and re-emit its caller performs rather than through this load.
    match POLARITY.load(Ordering::Relaxed) {
        0 => Polarity::Dark,
        _ => Polarity::Light,
    }
}

/// Sets the process polarity, and returns whether it moved.
///
/// Step one of four. The caller completes the flip, and skipping any of the rest leaves half
/// the interface in the other polarity: repolarize every live [`Scope`] so [`resolve`]
/// answers from the other palette, bump the colour generation so rasterized cells are
/// rebuilt, re-emit the whole patch so sprites rebind their paints, and invalidate the window
/// backdrop, which is not in the retained tree.
pub fn set_polarity(polarity: Polarity) -> bool {
    let next = u8::from(polarity == Polarity::Light);
    // Relaxed: the swap orders nothing but itself. What follows a flip is the caller's
    // re-resolve and re-emit, not a read of this byte.
    POLARITY.swap(next, Ordering::Relaxed) != next
}
