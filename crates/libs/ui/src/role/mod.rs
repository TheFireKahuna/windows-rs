//! Roles: what a widget names instead of a colour, a size, a spacing or an alignment.
//!
//! The layer this replaces is one every application layer grows and none of them wants.
//! Its symptom is a call site restating the theme's job:
//!
//! ```text
//! caption(desc)
//!     .foreground(theme::text_tertiary())   // the application doing the theme's job
//!     .font_size(desc_size)                 // and the type ramp's job
//!     .wrap()
//! ```
//!
//! The answer is neither a cascade nor nothing. It is **one level of scope, resolved by a
//! total function**: a widget names a [`Role`], the enclosing [`Scope`] says what that
//! means here, and [`resolve`] always has an answer.
//!
//! # The four properties that make this safe where CSS is not
//!
//! 1. **One level.** A role resolves against *the enclosing scope*, not an inheritance
//!    chain. No specificity, no `!important`, no action at a distance.
//! 2. **Total.** Every `(role, scope)` pair has a value. [`resolve`] returns a
//!    [`Radiance`], not an `Option`. A missing token is unrepresentable.
//! 3. **Scopes nest by construction.** A card *is* a scope push. There is no ambient
//!    inheritance to fall into by accident, only lexical nesting.
//! 4. **[`Data`](Role::Data) roles carry no polarity.** Band hues, series colours and
//!    the spectrum ramp are chromatic and shared between light and dark.
//!
//! # Everything here is authored light
//!
//! [`resolve`] returns [`Radiance`] — scene-referred, linear Rec.2020, absolute cd/m²,
//! unbounded. Nothing at this layer has met a display, and there is no way to ask it for
//! anything else. The display transform runs once, at the draw choke, on the way to the
//! compositor.

mod palette;
#[cfg(test)]
mod tests;

pub use palette::{
    Palette, accent_wash, ink, install, installed, metric, resolve, typography, veil,
};

use core::sync::atomic::{AtomicU8, Ordering};

pub use windows_scene::WidthClass;

/// How far a surface sits off the window's own plane.
///
/// Not a shadow depth and not a z-index: it selects which rung of the surface ladder a
/// scope's fills come from, and the ladder is a design decision the palette owns.
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

/// How tight the layout is. **The user's preference**, not the container's situation.
///
/// It applies everywhere at once, which is what deletes the
/// `if simple { FONT_LG } else { FONT_MD }` conditional from every card.
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
/// Five axes, and they are separate because they answer different questions.
/// [`Density`] is what the *user* asked for and applies everywhere at once;
/// [`WidthClass`] is how much room *this container* got, so the same card is `Wide` in a
/// full-width row and `Narrow` in a detail pane, in the same window, at the same instant.
/// Resolving both in one function is what lets a palette say "a compact card that is also
/// narrow drops to the tightest gap, but never below the touch floor" in one place,
/// instead of every card multiplying two conditionals at its call site.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Scope {
    pub elevation: Elevation,
    pub polarity: Polarity,
    pub accent: AccentId,
    pub density: Density,
    pub width: WidthClass,
}

impl Scope {
    /// The window's own scope: [`Elevation::Base`], the current process [`polarity`], and
    /// the given accent and density.
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

    /// The same scope one rung up the surface ladder. What `card` and `flyout` do.
    #[must_use]
    pub const fn elevate(self, elevation: Elevation) -> Self {
        Self { elevation, ..self }
    }

    /// The same scope at a classified width. What a responsive container does to its
    /// subtree.
    #[must_use]
    pub const fn at_width(self, width: WidthClass) -> Self {
        Self { width, ..self }
    }

    /// The same scope at a different density.
    #[must_use]
    pub const fn at_density(self, density: Density) -> Self {
        Self { density, ..self }
    }

    /// The same scope with the current process polarity re-read. Step one of a theme flip.
    #[must_use]
    pub fn repolarized(self) -> Self {
        Self {
            polarity: polarity(),
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
/// [`Hover`](Self::Hover), [`Pressed`](Self::Pressed) and [`Selected`](Self::Selected) are
/// **not** extra colour parameters. They are the same surface resolved in a different
/// interaction state, and the scene ramps between the two resolutions on the event. The
/// application never writes a hover colour.
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
/// The one extensible arm, and the only one carrying no [`Polarity`]: a band hue, a
/// series colour and the spectrum ramp mean the same thing in light and dark. This
/// crate never interprets the number.
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
    Radius,
    /// The radius of a fully rounded control.
    ///
    /// **A real value in the scale, and a small one.** A composition corner radius caps at
    /// half the box, so a pill authored as some large sentinel renders as a football on a
    /// tall box rather than as a stadium.
    RadiusPill,
    /// A control's row height, and the floor a touch target is inflated to.
    RowH,
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
    match POLARITY.load(Ordering::Relaxed) {
        0 => Polarity::Dark,
        _ => Polarity::Light,
    }
}

/// Flips the process polarity, and answers whether it moved.
///
/// This is **step one of four**, and missing any of the others leaves half the interface
/// in the other polarity: every live [`Scope`] must be repolarized so `resolve` returns the
/// other palette, the colour generation must bump so rasterized cells are rebuilt, the
/// whole patch must be re-emitted so sprites rebind their paints, and the window backdrop
/// — which is not in the retained tree — must be invalidated on its own.
pub fn set_polarity(polarity: Polarity) -> bool {
    let next = u8::from(polarity == Polarity::Light);
    POLARITY.swap(next, Ordering::Relaxed) != next
}
