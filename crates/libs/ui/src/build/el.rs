//! `El<K>` — a node under construction, and the whole modifier surface.
//!
//! Modifiers that apply to every element — `.tip()`, `.key()`, `.grow()` and the rest — are
//! written once in `impl<K> El<K>` rather than once per widget.
//!
//! `K` is a zero-sized marker gating the kind-specific methods, so a card has no `.accent()`
//! and a box has no `.trim()`. Only [`Path`] and [`Button`] carry one; everything else is
//! `El<Any>`. A method a kind cannot honour is absent rather than accepted and ignored, or
//! clamped into a table row that renders as some other widget.

use super::arena::{
    Act, Build, ChanSource, HitSeed, MaskSeed, Part, Slot, SpriteSeed, TextSeed, Unit,
};
use crate::gesture::{DragDecl, GestureDecl};
use crate::layout::{Align, Edge, Len, Over, Preset, Rule, Track};
use crate::role::{DataRole, Elevation, Fill, Metric, Role, Text, TypeRole, WidthClass};
use crate::signal::Signal;
use crate::widget::{Chrome, Flow, Interaction, Motion, RoleSet, StatePolicy, TextSource, UiaRole};
use core::marker::PhantomData;
use windows_numerics::Vector2;
use windows_scene::{Bounds, Exit, GeomId, HitFlags, Prop, RampId, Value};

/// The default kind: no methods beyond the universal surface.
#[derive(Copy, Clone, Debug)]
pub struct Any;
/// A geometry sprite. Owns `fill` / `stroke` / `ink` / `trim`.
#[derive(Copy, Clone, Debug)]
pub struct Path;
/// A presentation region: one sprite painting a buffer the present thread draws. Owns
/// `radius`, and no colour method at all — nothing on this side says what is in the pixels.
#[derive(Copy, Clone, Debug)]
pub struct Region;
/// A widget reading the button role table. Owns the variant methods.
///
/// The kind restricts those methods to elements whose chrome row comes from the button
/// table. On a card the same index would select a row of the surface table and render a
/// panel.
#[derive(Copy, Clone, Debug)]
pub struct Button;

/// A node under construction: an index into the thread's build arena.
///
/// `Copy`, with no refcount behind it, so a `move ||` closure captures one without cloning —
/// the same property that makes [`Cell`](crate::signal::Cell) cheap, extended to elements.
pub struct El<K = Any> {
    pub(crate) at: u32,
    /// `fn() -> K` rather than `K`, so the marker lends the element none of its auto-traits
    /// and leaves `K` covariant.
    kind: PhantomData<fn() -> K>,
}

impl<K> Clone for El<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for El<K> {}

impl<K> core::fmt::Debug for El<K> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("El").field(&self.at).finish()
    }
}

/// The application-facing element type.
pub type View = El<Any>;

/// Converts a channel argument into the scene's [`Value`].
///
/// Crate-private: every channel is reached through a named method, so the set of types a
/// channel accepts is closed here rather than at the authoring surface.
pub(crate) trait IntoValue: Copy + 'static {
    fn value(self) -> Value;
}

impl IntoValue for f32 {
    fn value(self) -> Value {
        Value::Scalar(self)
    }
}

impl IntoValue for Vector2 {
    fn value(self) -> Value {
        Value::Vec2(self)
    }
}

impl<K> El<K> {
    pub(crate) const fn at_index(at: u32) -> Self {
        Self {
            at,
            kind: PhantomData,
        }
    }

    /// Discards the kind marker, answering a [`View`]. Every container does this to its
    /// children.
    #[must_use]
    pub const fn erase(self) -> View {
        El::at_index(self.at)
    }

    pub(crate) fn seed(preset: Preset) -> Self {
        Self::at_index(Build::with(|b| {
            b.push_slot(Slot {
                preset,
                ..Slot::default()
            })
        }))
    }

    pub(crate) fn over(self, over: Over) -> Self {
        Build::with(|b| b.push_over(self.at, Rule::always(over)));
        self
    }

    /// Pushes `over` as a rule that applies at `class` only.
    pub(crate) fn over_at(self, class: WidthClass, over: Over) -> Self {
        Build::with(|b| b.push_over(self.at, Rule::at(class, over)));
        self
    }

    pub(crate) fn sprite(self, mask: MaskSeed, role: Role, part: Part) -> Self {
        self.sprite_at(mask, role, part, super::arena::FULL)
    }

    /// Adds a sprite painting `strength` of `role`.
    ///
    /// `strength` must be in `0.0..=1.0`. It is the sprite's own, not a second role: a plate
    /// under text of the same hue is that hue at a fraction of it, and a palette answers one
    /// value per role.
    pub(crate) fn sprite_at(self, mask: MaskSeed, role: Role, part: Part, strength: f32) -> Self {
        Build::with(|b| {
            b.push_seed(
                self.at,
                SpriteSeed {
                    mask,
                    role,
                    strength,
                    ramp: None,
                    region: None,
                    part,
                    next: super::arena::NIL,
                },
            );
        });
        self
    }

    pub(crate) fn slot_mut(self, f: impl FnOnce(&mut Slot)) -> Self {
        Build::with(|b| f(&mut b.nodes[self.at as usize]));
        self
    }

    pub(crate) fn act(self, act: Act) -> Self {
        Build::with(|b| b.push_act(self.at, act));
        self
    }

    /// Records a reactive property on this node.
    ///
    /// A constant is stored inline and produces no graph node; anything else is boxed and
    /// becomes one `Effect` at mount.
    pub(crate) fn channel<T, M>(
        self,
        prop: Prop,
        motion: Motion,
        unit: Unit,
        v: impl Signal<T, M> + 'static,
    ) -> Self
    where
        T: IntoValue,
    {
        let source = if v.is_constant() {
            ChanSource::Const(v.read().value())
        } else {
            ChanSource::Dynamic(Box::new(move || v.read().value()))
        };
        Build::with(|b| b.push_chan(self.at, prop, motion, unit, source));
        self
    }

    /// Records the shaped run this node draws.
    ///
    /// `ink` of `None` takes the enclosing widget's chrome row, so a variant that moves the
    /// text colour leaves the text seed untouched.
    pub(crate) fn text_seed(
        self,
        source: TextSource,
        ramp: TypeRole,
        ink: Option<Text>,
        flow: Flow,
        caps: bool,
    ) -> Self {
        let text = Build::with(|b| {
            b.push_text(TextSeed {
                source: Some(source),
                ramp,
                ink,
                flow,
                vertical: false,
                caps,
            })
        });
        self.sprite(
            MaskSeed::Run { text },
            Role::Text(ink.unwrap_or(Text::Primary)),
            Part::Label,
        )
    }

    pub(crate) fn vertical_text(self) -> Self {
        Build::with(|b| {
            let text = b
                .chain_seeds(b.nodes[self.at as usize].seeds)
                .find_map(|s| {
                    if let MaskSeed::Run { text } = s.mask {
                        Some(text)
                    } else {
                        None
                    }
                })
                .expect("vertical text requires a run");
            b.texts[text as usize].vertical = true;
        });
        self
    }

    /// Records a shaped run painted in `role` rather than in a foreground rung.
    ///
    /// The node carries no chrome row of its own, which is what lets the role stated here
    /// reach the sprite: a label takes its enclosing row's text colour only where there is
    /// one.
    pub(crate) fn text_seed_in(
        self,
        source: TextSource,
        ramp: TypeRole,
        role: Role,
        caps: bool,
    ) -> Self {
        let text = Build::with(|b| {
            b.push_text(TextSeed {
                source: Some(source),
                ramp,
                ink: None,
                flow: Flow::Line,
                vertical: false,
                caps,
            })
        });
        self.sprite(MaskSeed::Run { text }, role, Part::Label)
    }

    /// Records the role table, variant index and corner radius this node's surface resolves
    /// from.
    pub(crate) fn chrome(self, roles: &'static [RoleSet], variant: u8, radius: Metric) -> Self {
        self.slot_mut(|s| {
            s.chrome = Some(Chrome {
                roles,
                variant,
                radius,
                attached: None,
            });
        })
    }

    /// Joins this surface to an edge without changing its layout or hit box.
    pub(crate) fn attached(self, edge: Edge) -> Self {
        self.slot_mut(|s| {
            s.chrome
                .as_mut()
                .expect("attached surface has chrome")
                .attached = Some(edge)
        })
    }

    /// Selects which row of its own role table this widget reads.
    ///
    /// Writes one byte on the slot. Which sprites the row implies — a fill that is minted, a
    /// stroke that is not — is decided at mount, so this may run in any order relative to the
    /// other modifiers.
    ///
    /// # Panics
    ///
    /// If the node carries no role table. Reachable only from a kind that carries one
    /// ([`Button`]). In a debug build, also if `at` is past the end of that table.
    pub(crate) fn variant(self, at: u8) -> Self {
        self.slot_mut(|s| {
            let chrome = s
                .chrome
                .as_mut()
                .expect("a variant belongs to a kind that carries a role table");
            debug_assert!(
                (at as usize) < chrome.roles.len(),
                "variant {at} is past the end of this widget's own table"
            );
            chrome.variant = at;
        })
    }

    pub(crate) fn interaction(self, interaction: Interaction) -> Self {
        self.slot_mut(|s| s.interaction = Some(interaction))
    }

    /// Adds the sprite a value moves: a toggle's knob, a slider's thumb, a meter's level.
    ///
    /// Marked [`Part::Thumb`] rather than a fill, so the router moves exactly this sprite and
    /// the state driver re-resolves the rest of the control without it.
    ///
    /// `radius` is a [`Len`] and not a [`Metric`], because a round part's radius is half its
    /// own box rather than a rung of the radius scale. A metric above half is capped per axis
    /// by the platform, and the four corners then meet in the middle.
    pub(crate) fn thumb(self, radius: impl Into<Len>, role: Role) -> Self {
        self.sprite(
            MaskSeed::Box {
                radius: Some(radius.into()),
            },
            role,
            Part::Thumb,
        )
    }

    /// Washes this node in the gradient `id` names, inside a `radius` corner.
    ///
    /// [`Part::Static`] rather than a fill: the wash sits over the surface's own fill and an
    /// interaction state must not re-resolve it into a flat colour. It is emitted after the
    /// chrome, so it lands above that fill and below anything the node contains.
    ///
    /// The node keeps whatever fill it already had. A wash is a tint over a surface, not the
    /// surface — which is what lets it fade out across a card instead of ending at a seam.
    #[must_use]
    pub fn washed(self, id: RampId, radius: Metric) -> Self {
        Build::with(|b| {
            b.push_seed(
                self.at,
                SpriteSeed {
                    mask: MaskSeed::Box {
                        radius: Some(Len::Metric(radius)),
                    },
                    // Unread while `ramp` is set, and stated rather than left arbitrary so the
                    // seed is meaningful if a ramp is ever cleared from one.
                    role: Role::Fill(Fill::Surface),
                    strength: super::arena::FULL,
                    ramp: Some(id),
                    region: None,
                    part: Part::Static,
                    next: super::arena::NIL,
                },
            );
        });
        self
    }

    /// Casts a halo behind this node in `role`: its own silhouette, blurred to whatever
    /// light the palette says that role spends.
    ///
    /// **For a surface casting light in a role it does not itself paint** — a card whose fill
    /// is a surface and whose light is its processor kind's, a call to action whose light is
    /// the accent's. A sprite that paints a role *as ink* is lit where the role is resolved
    /// and needs nothing here, which is why a label and a badge never name one.
    ///
    /// It takes a role and nothing else: how far a role's light reaches is the palette's to
    /// author, and a σ passed in here would be exactly the ad-hoc brightening at draw sites
    /// the emissive tier exists to replace. A node whose role spends no light is an authoring
    /// mistake rather than a no-op.
    ///
    /// The compositor derives the shape from the alpha the node already paints, so a halo
    /// costs **no visual and no capture** — which is what makes one affordable on every card
    /// in a chain. It casts from the node's fill, or from its glyphs where it paints no fill.
    ///
    /// A halo escapes the node's own box but not an explicit clip on an ancestor, so a halo
    /// inside a container that rounds its own corners is cut at that container.
    ///
    /// **The blur is fixed once and never animates.** Re-blurring is the one per-frame cost
    /// the platform charges for a shadow, so a state that widened the light would pay it on
    /// every frame of every transition, on every element the transition touched. A state
    /// spends more of the light instead, through [`halo_lit`](Self::halo_lit).
    #[must_use]
    pub fn halo<M>(self, role: impl Signal<Role, M> + 'static) -> Self {
        let fixed = role.is_constant().then(|| role.read());
        Build::with(|b| {
            let seed = if let Some(role) = fixed {
                super::arena::HaloSeed::Glow(role)
            } else {
                let index = b.halo_roles.len() as u32;
                b.halo_roles.push(Some(Box::new(move || role.read())));
                super::arena::HaloSeed::Reactive(index)
            };
            b.nodes[self.at as usize].halo = Some(seed);
        });
        self
    }

    /// Casts the palette's occlusion outward from `edge`, using this surface's fill.
    /// Shares the halo slot with emissive light; the last declaration owns it. Its fixed
    /// blur and offset are resolved once, and require no capture or per-frame app work.
    #[must_use]
    pub fn shadowed(self, edge: Edge) -> Self {
        self.slot_mut(|s| s.halo = Some(super::arena::HaloSeed::Shadow(edge)))
    }

    /// Binds how much of its light the halo is currently spending, in `0.0..=1.0`.
    ///
    /// The one channel a state may move. The palette says how far a role's light reaches and
    /// how much of the role's alpha it carries; a state says how much of that is switched on.
    /// Opacity is a compositor property over an already-blurred silhouette, so a transition
    /// re-rasterizes nothing.
    ///
    /// Means nothing on a node with no [`halo`](Self::halo): the channel belongs to the
    /// shadow, and the scene refuses a property whose owner the node does not carry.
    #[must_use]
    pub fn halo_lit<M>(self, lit: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::ShadowOpacity, Motion::Chrome, Unit::Direct, lit)
    }

    /// Adds the plate a chromatic value sits on: a rounded box painting `strength` of `role`.
    ///
    /// [`Part::Static`] rather than [`Part::Fill`]: the plate is the value's own colour and an
    /// interaction state must not re-resolve it into a surface.
    pub(crate) fn plate(self, radius: Metric, role: Role, strength: f32) -> Self {
        self.sprite_at(
            MaskSeed::Box {
                radius: Some(Len::Metric(radius)),
            },
            role,
            Part::Static,
            strength,
        )
    }

    /// Names the geometry this node's shape sprites draw.
    pub(crate) fn geom(self, geom: GeomId) -> Self {
        self.slot_mut(|s| s.geom = Some(geom))
    }

    /// Re-rounds the region sprite this node already carries.
    ///
    /// The sprite is the node's only one, so the mask is rewritten in place rather than a
    /// second one added over it: two boxes at different radii leave the wider one's corners
    /// showing behind the narrower one's.
    pub(crate) fn region_radius(self, radius: Len) -> Self {
        Build::with(|b| {
            let head = b.nodes[self.at as usize].seeds.head;
            if head != super::arena::NIL {
                b.seeds[head as usize].mask = MaskSeed::Box {
                    radius: Some(radius),
                };
            }
        });
        self
    }

    /// Sets the height to `n` of the row height `row` names, both re-read on every restyle.
    ///
    /// Crate-private: its caller is the virtualized list's spacers, where `n` is a count of
    /// unrealized rows. Both arguments are plain closures rather than [`Signal`]s, unlike
    /// every value-taking modifier on the authoring surface.
    pub(crate) fn height_rows(
        self,
        row: impl Fn() -> Metric + 'static,
        n: impl Fn() -> f32 + 'static,
    ) -> Self {
        self.act(Act::Restyle(Box::new(move |out| {
            out.push(Rule::always(Over::Height(Len::Times(row(), n().max(0.0)))));
        })))
    }

    /// Places this node as row `index` of a uniform list: out of flow, one row tall, `index`
    /// row heights down the container.
    ///
    /// Stated by the list on the row's behalf, as a grid container states a placement.
    /// `index` is fixed for the row's life, since a keyed reconcile moves a row's position in
    /// the list and never its key, so this re-lowers only when `row` answers differently.
    pub(crate) fn band_rows(self, index: f32, row: impl Fn() -> Metric + 'static) -> Self {
        self.act(Act::Restyle(Box::new(move |out| {
            let row = row();
            out.push(Rule::always(Over::Band {
                at: Len::Times(row, index),
                height: Len::Metric(row),
            }));
        })))
    }

    /// Makes this container scroll: a tracker on its own box, and the content bound to it.
    pub(crate) fn scrolls(self, decl: crate::layout::ScrollDecl) -> Self {
        Build::with(|b| {
            // Redirect hands touch to the tracker rather than to a recogniser, so a fling
            // keeps running while the front thread is busy. Only a scroll surface sets it.
            b.gesture_mut(self.at, |decl| decl.redirect = true);
            let slot = &mut b.nodes[self.at as usize];
            slot.scroll = Some(decl);
            add_flags(
                slot,
                HitFlags::SCROLL | HitFlags::INTERACTIVE | HitFlags::WHEEL,
            );
        });
        self
    }

    pub(crate) fn gesture(self, decl: GestureDecl) -> Self {
        Build::with(|b| {
            b.gesture_mut(self.at, |slot| *slot = decl);
            add_flags(&mut b.nodes[self.at as usize], HitFlags::GESTURE);
        });
        self
    }

    // ── structure ─────────────────────────────────────────────────────────────────

    /// Lays `children` out in a column. They stretch across it.
    #[must_use]
    pub fn stack(self, children: impl super::IntoChildren) -> Self {
        self.contain(Preset::Stack, children)
    }

    /// Lays `children` out in a row. They centre on the cross axis.
    #[must_use]
    pub fn row(self, children: impl super::IntoChildren) -> Self {
        self.contain(Preset::Row, children)
    }

    /// Lays `children` out in a row that wraps.
    #[must_use]
    pub fn wrap(self, children: impl super::IntoChildren) -> Self {
        self.contain(Preset::Wrap, children)
    }

    /// Lays `children` out in an explicit grid, auto-placing each one the container does not
    /// place.
    #[must_use]
    pub fn grid(self, children: impl super::IntoChildren) -> Self {
        self.contain(Preset::Grid, children)
    }

    /// Lays `children` out as responsive tiles: `repeat(auto-fill, minmax(min, 1fr))`.
    #[must_use]
    pub fn tiles(self, min: impl Into<Len>, children: impl super::IntoChildren) -> Self {
        self.over(Over::TileMin(min.into()))
            .contain(Preset::Tiles, children)
    }

    /// Sets this node's layout class and collects `children` into its child list.
    ///
    /// The class is written unconditionally. Chrome is carried as overrides
    /// ([`surface`](Self::surface), [`control`](Self::control)), so there is nothing on the
    /// slot for it to displace.
    pub(crate) fn contain(self, preset: Preset, children: impl super::IntoChildren) -> Self {
        // Collected onto the arena's own stack, so a screen of nested containers allocates
        // once at high-water mark rather than once per container per mount.
        let mark = Build::with(|b| b.mark());
        children.append(&mut super::Children::new());
        Build::with(|b| {
            b.nodes[self.at as usize].preset = preset;
            b.take_kids(self.at, mark);
        });
        self
    }

    /// Applies a surface: an elevation push, a chrome row from the surface table, and the
    /// padding for that rung.
    ///
    /// Shared by `card`, `panel` and `flyout`. It sets no layout class, so `card().stack(..)`
    /// and `card().row(..)` are both cards. The padding is an override, and overrides apply
    /// in chain order, so a call site stating its own afterwards wins.
    pub(crate) fn surface(self, elevation: Elevation, variant: u8, radius: Metric) -> Self {
        self.elevate(elevation)
            .chrome(crate::widget::roles::SURFACE, variant, radius)
            .over(Over::Padding(Len::Metric(Metric::SpaceLg)))
    }

    /// Applies control metrics: the palette's row height as a floor, control padding, a
    /// tighter gap, and centred main-axis alignment.
    ///
    /// All four are overrides, so a call site restating any of them afterwards wins.
    pub(crate) fn control(self) -> Self {
        // The two axes differ: the row height is what sets a control's height, so the
        // vertical padding only has to clear the text inside it, while the horizontal one
        // is what separates a label from the control's own edge.
        self.over(Over::MinHeight(Len::Metric(Metric::RowH)))
            .over(Over::PaddingXY(
                Len::Metric(Metric::SpaceMd),
                Len::Metric(Metric::SpaceXs),
            ))
            .over(Over::Gap(Len::Metric(Metric::SpaceSm)))
            .over(Over::Justify(Align::Center))
    }

    /// Places `child` at grid `row` and `column`, and appends it to this node's children.
    ///
    /// Stated by the container: the child carries no placement modifier of its own, so a
    /// placement can only be written where the container is able to honour it.
    #[must_use]
    pub fn at<C>(self, row: u16, column: u16, child: El<C>) -> Self {
        self.place_child(row, column, 1, 1, child.erase())
    }

    /// Places `child` at grid `row` and `column`, spanning `row_span` rows and `column_span`
    /// columns.
    #[must_use]
    pub fn span<C>(
        self,
        row: u16,
        column: u16,
        row_span: u16,
        column_span: u16,
        child: El<C>,
    ) -> Self {
        self.place_child(row, column, row_span, column_span, child.erase())
    }

    fn place_child(self, row: u16, column: u16, dr: u16, dc: u16, child: View) -> Self {
        child.over(Over::Place {
            row,
            column,
            row_span: dr,
            column_span: dc,
        });
        Build::with(|b| b.push_kid(self.at, child.at));
        self
    }

    /// Appends `tracks` to this grid's row template.
    #[must_use]
    pub fn rows(self, tracks: impl IntoIterator<Item = Track>) -> Self {
        for t in tracks {
            self.over(Over::Row(t));
        }
        self
    }

    /// Appends `tracks` to this grid's column template.
    #[must_use]
    pub fn cols(self, tracks: impl IntoIterator<Item = Track>) -> Self {
        for t in tracks {
            self.over(Over::Column(t));
        }
        self
    }

    // ── layout: width variants ───────────────────────────────────────────────────
    //
    // Padding, gap, type size, radius and control sizes follow the width class through
    // `Scope` with nothing declared at the call site. These three methods are the whole of
    // what a call site states per class.
    //
    // None of them mounts or unmounts: crossing a threshold changes styles and never
    // structure, so nothing is dropped and no cell is disposed while a resize drag crosses a
    // boundary, and state inside the narrow arrangement survives. A pane that restructures —
    // docked column to overlay drawer — is a `switch` over the window size.

    /// Lays this container out as a column at `class`, whatever class it carries otherwise.
    ///
    /// The whole preset swaps rather than the direction alone: a row centres its children and
    /// gaps them along the inline axis, where a column stretches them and gaps them along the
    /// block axis.
    #[must_use]
    pub fn stack_when(self, class: WidthClass) -> Self {
        self.over_at(class, Over::Class(Preset::Stack))
    }

    /// Sets the column template at `class`, clearing whatever was stated for every class.
    ///
    /// `tracks` is the whole template at that class rather than an addition to it. The other
    /// classes need no declaration: a grid with no template auto-places into a single column.
    #[must_use]
    pub fn cols_when(self, class: WidthClass, tracks: impl IntoIterator<Item = Track>) -> Self {
        self.over_at(class, Over::ClearColumns);
        for t in tracks {
            self.over_at(class, Over::Column(t));
        }
        self
    }

    /// Sets the column template while `cond` holds, clearing whatever was stated otherwise.
    ///
    /// [`cols_when`](Self::cols_when) keys the same statement on the window's width; this one
    /// keys it on application state — a pane the user collapsed, a gutter they turned off.
    /// Each clears the template and states its tracks, and the lowering resolves them in the
    /// order they were written.
    ///
    /// This changes styles and never structure: the track that goes away drops no owner, so
    /// state inside the collapsed column is still there when it comes back.
    ///
    /// The class rules are the recipe and this is a bound override on top of it, so where
    /// both apply this one wins while `cond` holds.
    #[must_use]
    pub fn cols_if<M>(
        self,
        cond: impl Signal<bool, M> + 'static,
        tracks: impl IntoIterator<Item = Track>,
    ) -> Self {
        let tracks: Vec<Track> = tracks.into_iter().collect();
        self.act(Act::Restyle(Box::new(move |out| {
            if !cond.read() {
                return;
            }
            out.push(Rule::always(Over::ClearColumns));
            out.extend(tracks.iter().copied().map(Over::Column).map(Rule::always));
        })))
    }

    /// States the column template from whatever `tracks` writes, re-read whenever a signal
    /// it reads changes.
    ///
    /// [`cols_if`](Self::cols_if) keys a template it was handed on a condition; this one
    /// **computes** the template. What needs it is a track whose extent is a value rather
    /// than a case — a graph column sized from a channel count, a rail sized from what it
    /// holds — where enumerating one arm per value is the whole domain of the value.
    ///
    /// `tracks` fills a buffer this element keeps, so a re-read allocates nothing after the
    /// first. It clears the template accumulated below it and states its own, exactly as
    /// `cols_if` does, and like every restyle it changes styles and never structure: a track
    /// that resizes drops no owner, so state in the column it sizes is untouched.
    #[must_use]
    pub fn cols_from(self, tracks: impl Fn(&mut Vec<Track>) + 'static) -> Self {
        let buf = core::cell::RefCell::new(Vec::new());
        self.act(Act::Restyle(Box::new(move |out| {
            let mut buf = buf.borrow_mut();
            buf.clear();
            tracks(&mut buf);
            out.push(Rule::always(Over::ClearColumns));
            out.extend(buf.iter().copied().map(Over::Column).map(Rule::always));
        })))
    }

    /// Clips pixels and hit-testing to this node's solved box, without a scroll tracker.
    #[must_use]
    pub fn clip(self) -> Self {
        self.over(Over::Clip)
    }

    /// Hides this subtree at `class`: not laid out, and not drawn.
    ///
    /// `Display::None` rather than [`when`](Self::when), so the subtree stays mounted and its
    /// state is still there when the class moves back.
    ///
    /// Exactly one class. A subtree hidden below a threshold takes
    /// [`hide_below`](Self::hide_below).
    #[must_use]
    pub fn hide_when(self, class: WidthClass) -> Self {
        self.over_at(class, Over::Hidden)
    }

    /// Hides this subtree at every class narrower than `class`: not laid out, and not drawn.
    ///
    /// `Display::None` on the terms [`hide_when`](Self::hide_when) states.
    #[must_use]
    pub fn hide_below(self, class: WidthClass) -> Self {
        for narrower in class.below() {
            self.over_at(narrower, Over::Hidden);
        }
        self
    }

    /// Floats this subtree at `class`: out of flow, pinned to `edge` of its container and
    /// stretched across the other axis.
    ///
    /// The twin of [`hide_when`](Self::hide_when), and the same mechanism: a style, so the
    /// subtree stays mounted and nothing inside it is disposed when the class moves. The node
    /// keeps its own width or height for the axis it pins along, and takes its container's
    /// padding box for the other.
    ///
    /// A float paints over its siblings rather than beside them, and the tree's order is its
    /// z-order, so a floating child declared last covers the ones before it — in the hit
    /// array as well as on screen.
    ///
    /// Exactly one class. A subtree that floats below a threshold takes
    /// [`float_below`](Self::float_below).
    #[must_use]
    pub fn float_when(self, class: WidthClass, edge: Edge) -> Self {
        self.over_at(class, Over::Edge(edge))
    }

    /// Floats this subtree at every class narrower than `class`.
    ///
    /// Out of flow on the terms [`float_when`](Self::float_when) states.
    #[must_use]
    pub fn float_below(self, class: WidthClass, edge: Edge) -> Self {
        for narrower in class.below() {
            self.over_at(narrower, Over::Edge(edge));
        }
        self
    }

    /// Hides this subtree while `cond` holds: not laid out, and not drawn.
    ///
    /// `Display::None` on the terms [`hide_when`](Self::hide_when) states: the subtree stays
    /// mounted and nothing is disposed. [`when`](Self::when) is the same mechanism with the
    /// condition read the other way round, so neither sense needs a `!` at the call site.
    #[must_use]
    pub fn hide_if<M>(self, cond: impl Signal<bool, M> + 'static) -> Self {
        self.act(Act::HideWhen(Box::new(move || cond.read())))
    }

    /// Hides this retained subtree only at `class` while `cond` holds.
    /// The solve retains the class gate across resizes; no width enters the signal graph.
    #[must_use]
    pub fn hide_if_when<M>(self, class: WidthClass, cond: impl Signal<bool, M> + 'static) -> Self {
        self.act(Act::Restyle(Box::new(move |out| {
            if cond.read() {
                out.push(Rule::at(class, Over::Hidden));
            }
        })))
    }

    /// Opens a popup while `shown` holds, owned by this mounted node. User dismissal
    /// calls `closed`; a false condition or an unmount closes without changing app intent.
    /// Content is rebuilt under an overlay owner on each open and disposed on close.
    #[must_use]
    pub fn popup_when<M>(
        self,
        shown: impl Signal<bool, M> + 'static,
        spec: crate::overlay::Spec,
        closed: impl Fn() + 'static,
        body: impl Fn() -> View + 'static,
    ) -> Self {
        self.act(Act::Popup {
            shown: Box::new(move || shown.read()),
            spec,
            body: std::rc::Rc::new(body),
            closed: std::rc::Rc::new(closed),
        })
    }

    /// Sets the inline size at one width class.
    #[must_use]
    pub fn width_when(self, class: WidthClass, width: impl Into<Len>) -> Self {
        self.over_at(class, Over::Width(width.into()))
    }

    /// Sets the inline size floor at one width class.
    #[must_use]
    pub fn min_width_when(self, class: WidthClass, width: impl Into<Len>) -> Self {
        self.over_at(class, Over::MinWidth(width.into()))
    }

    /// Sets the inline size ceiling at one width class.
    #[must_use]
    pub fn max_width_when(self, class: WidthClass, width: impl Into<Len>) -> Self {
        self.over_at(class, Over::MaxWidth(width.into()))
    }

    // ── layout: container properties ─────────────────────────────────────────────

    /// Absorbs the slack left along the container's main axis.
    #[must_use]
    pub fn grow(self) -> Self {
        self.over(Over::Grow)
    }

    /// Keeps the stated size in a box too small for it.
    #[must_use]
    pub fn no_shrink(self) -> Self {
        self.over(Over::NoShrink)
    }

    /// Sets a definite inline size.
    #[must_use]
    pub fn width(self, l: impl Into<Len>) -> Self {
        self.over(Over::Width(l.into()))
    }

    /// Sets a definite block size.
    #[must_use]
    pub fn height(self, l: impl Into<Len>) -> Self {
        self.over(Over::Height(l.into()))
    }

    /// Sets a floor on the inline size.
    #[must_use]
    pub fn min_width(self, l: impl Into<Len>) -> Self {
        self.over(Over::MinWidth(l.into()))
    }

    /// Sets a floor on the block size.
    #[must_use]
    pub fn min_height(self, l: impl Into<Len>) -> Self {
        self.over(Over::MinHeight(l.into()))
    }

    /// Sets a ceiling on the inline size.
    #[must_use]
    pub fn max_width(self, l: impl Into<Len>) -> Self {
        self.over(Over::MaxWidth(l.into()))
    }

    /// Insets this container's content on every side.
    #[must_use]
    pub fn padding(self, l: impl Into<Len>) -> Self {
        self.over(Over::Padding(l.into()))
    }

    /// Insets this container's content by `x` either side and `y` above and below.
    ///
    /// What a run of text read across takes: the inset that ends the line has to clear the
    /// surface's corner while the one above it only has to separate the content from the
    /// edge, and a uniform inset large enough for the first makes the box taller than its
    /// content asked for.
    #[must_use]
    pub fn padding_xy(self, x: impl Into<Len>, y: impl Into<Len>) -> Self {
        self.over(Over::PaddingXY(x.into(), y.into()))
    }

    /// Sets the space between adjacent children.
    #[must_use]
    pub fn gap(self, l: impl Into<Len>) -> Self {
        self.over(Over::Gap(l.into()))
    }

    /// Aligns **all** of this container's children on the cross axis.
    #[must_use]
    pub fn align(self, a: Align) -> Self {
        self.over(Over::Align(a))
    }

    /// Distributes this container's children along the main axis.
    #[must_use]
    pub fn justify(self, a: Align) -> Self {
        self.over(Over::Justify(a))
    }

    /// Aligns this child on its container's cross axis, overriding what the container states
    /// for all of them.
    ///
    /// The one per-child layout property in the surface. Cross-axis alignment is honoured
    /// under every layout class this crate produces, where a grid placement on a flex child
    /// would be a write that goes nowhere — so placement is stated by the container
    /// ([`at`](Self::at)) instead.
    #[must_use]
    pub fn align_self(self, a: Align) -> Self {
        self.over(Over::AlignSelf(a))
    }

    /// Classifies this container's own inline size for its subtree: narrow at or below
    /// `narrow_max` DIPs, medium at or below `medium_max`, wide above it.
    ///
    /// The class is resolved inside the solve, so no caller passes a width down. Crossing a
    /// threshold changes styles and never structure, so nothing unmounts while a window is
    /// dragged across one.
    #[must_use]
    pub fn responsive(self, narrow_max: f32, medium_max: f32) -> Self {
        self.slot_mut(|s| s.responsive = Some(Bounds([narrow_max, medium_max])))
    }

    // ── presence ──────────────────────────────────────────────────────────────────

    /// Contributes nothing while `cond` is false: no node, no layout participation.
    ///
    /// A **constant** condition is resolved at build time and never reaches the graph. It
    /// marks the slot absent rather than hiding it, so the mount never sees it: no visual, no
    /// style, no shaped run, no mount row. A badge a build flag turns off costs the arena
    /// slot and nothing else.
    ///
    /// A **varying** condition is `Display::None`, so the subtree stays mounted and state
    /// inside it survives the condition flipping.
    ///
    /// There is no negated twin: a closure is a [`Signal`], so `when(move || !hidden())` is
    /// the same statement, with the negation at the call site.
    ///
    /// [`hide_when`](Self::hide_when) is not that twin. It takes a width class, which is
    /// resolved inside the solve and which no closure can read.
    #[must_use]
    pub fn when<M>(self, cond: impl Signal<bool, M> + 'static) -> Self {
        if cond.is_constant() {
            if !cond.read() {
                return self.slot_mut(|s| s.present = false);
            }
            return self;
        }
        self.act(Act::HideWhen(Box::new(move || !cond.read())))
    }

    /// Sets how this subtree leaves when it is destroyed.
    #[must_use]
    pub fn exit(self, exit: Exit) -> Self {
        self.slot_mut(|s| s.exit = exit)
    }

    // ── identity and assistive technology ─────────────────────────────────────────

    /// Names this node's automation-id segment. `&'static str`, so nothing is built at mount:
    /// the path is materialized only if UI Automation asks.
    #[must_use]
    pub fn key(self, key: &'static str) -> Self {
        self.slot_mut(|s| s.key = Some(key))
    }

    /// Sets the accessible name, for a widget whose own text does not supply one.
    #[must_use]
    pub fn name(self, name: &'static str) -> Self {
        self.slot_mut(|s| s.name = Some(name))
    }

    /// Declares this control to be one of the window's own commands.
    ///
    /// It stays an ordinary control: it draws, hovers and presses like any other and sits in
    /// the one hit array. What this adds is identity, so the caption band can name which
    /// command a point is over, the window answers `HTMINBUTTON` / `HTMAXBUTTON` / `HTCLOSE`
    /// there, and the drag strip is whatever the bar's controls leave over.
    ///
    /// The control must carry no click handler. The press is the system's from the moment the
    /// hit test names it and the window issues the `SC_*` itself, so a handler here would act
    /// a second time on one click. See [`caption`](crate::caption).
    #[must_use]
    pub fn caption(self, button: windows_window::CaptionButton) -> Self {
        self.slot_mut(|s| s.caption = Some(button))
    }

    // ── attachments ───────────────────────────────────────────────────────────────

    /// Attaches a hover description below the control.
    ///
    /// One tooltip exists at a time and the overlay layer owns it; a widget only declares
    /// interest. This also declares the node interactive, without which it would have no hit
    /// entry, the mount would have nothing to move the handler to, and the tip would be
    /// dropped in silence.
    #[must_use]
    pub fn tip(self, tip: impl Into<TextSource>) -> Self {
        self.tip_at(crate::overlay::Side::Bottom, tip)
    }

    /// Attaches a hover description on `side`.
    ///
    /// The side to pick is the one the control's siblings do not run along. Below suits a
    /// toolbar, where the neighbours are left and right of the button, and lands on top of
    /// the next item in a vertical rail. The placer flips and clamps against the window's
    /// edges only: it places against one box and does not see the ones beside it.
    #[must_use]
    pub fn tip_at(self, side: crate::overlay::Side, tip: impl Into<TextSource>) -> Self {
        self.act(Act::Tip(tip.into(), side))
            .slot_mut(|s| add_flags(s, HitFlags::INTERACTIVE))
    }

    /// Attaches an anchored, light-dismissed surface whose contents `body` builds.
    #[must_use]
    pub fn flyout(self, body: impl Fn() -> View + 'static) -> Self {
        self.act(Act::Flyout(std::rc::Rc::new(body)))
            .slot_mut(|s| add_flags(s, HitFlags::GESTURE))
    }

    /// Declares a drag: a movement threshold, then a lock onto the first axis past it.
    #[must_use]
    pub fn drag(self, drag: DragDecl) -> Self {
        Build::with(|b| {
            b.gesture_mut(self.at, |decl| *decl = decl.with_drag(drag));
            add_flags(&mut b.nodes[self.at as usize], HitFlags::GESTURE);
        });
        self
    }

    /// Disables this control while `cond` holds, swapping its base roles and dropping its hit
    /// flags. Model state rather than interaction chrome.
    #[must_use]
    pub fn disabled<M>(self, cond: impl Signal<bool, M> + 'static) -> Self {
        self.act(Act::DisabledWhen(Box::new(move || cond.read())))
    }

    /// Marks this control selected while `cond` holds. Model state as well: a discrete paint
    /// swap at event rate, not a wash.
    #[must_use]
    pub fn selected<M>(self, cond: impl Signal<bool, M> + 'static) -> Self {
        self.act(Act::SelectedWhen(Box::new(move || cond.read())))
    }

    /// Runs `f` when this control is clicked, and declares the hit entry the click routes
    /// through.
    #[must_use]
    pub fn on_click(self, f: impl Fn() + 'static) -> Self {
        self.act(Act::Click(Box::new(f)))
            .slot_mut(|s| add_flags(s, HitFlags::GESTURE | HitFlags::INTERACTIVE))
    }

    /// Handles Escape left unclaimed by the focus ring's overlay scopes.
    /// Declare once on the active screen root. Hidden and unmounted screens do not
    /// receive it; the callback runs outside the host borrow and adds no focus stop.
    #[must_use]
    pub fn on_unhandled_escape(self, f: impl Fn() + 'static) -> Self {
        self.act(Act::Escape(std::rc::Rc::new(f)))
    }

    /// Runs `f` for every value the control produces while it is being moved.
    ///
    /// Declares a hit entry, for the reason [`tip`](Self::tip) does: a handler on a node with
    /// no hit entry has nothing routing to it, and the mount would drop it in silence.
    #[must_use]
    pub fn on_change(self, f: impl Fn(f64) + 'static) -> Self {
        self.act(Act::ChangeF64(Box::new(f)))
            .slot_mut(|s| add_flags(s, HitFlags::GESTURE | HitFlags::INTERACTIVE))
    }

    /// Declares a two-axis drag and the handler it reports to.
    ///
    /// One call and not two: a handler on a node that declared no policy would never be
    /// reached, and a policy with no handler moves nothing. `f` receives every sample of the
    /// drag and then exactly one end — [`Dragging::Committed`](crate::widget::Dragging) on a
    /// release, [`Dragging::Canceled`](crate::widget::Dragging) on a contact taken away.
    ///
    /// A contact that never passes `decl`'s threshold ends as a click, so a node carrying
    /// both this and [`on_click`](Self::on_click) runs one of them and never both.
    #[must_use]
    pub fn on_drag(self, decl: DragDecl, f: impl Fn(crate::widget::Dragging) + 'static) -> Self {
        self.drag(decl)
            .act(Act::Drag(Box::new(f)))
            .slot_mut(|s| add_flags(s, HitFlags::GESTURE | HitFlags::INTERACTIVE))
    }

    /// Runs `f` with the value the control settled on.
    ///
    /// A canceled contact restores the value it had and commits nothing, which is the drag
    /// policy's rule rather than this method's.
    #[must_use]
    pub fn on_commit(self, f: impl Fn(f64) + 'static) -> Self {
        self.act(Act::CommitF64(Box::new(f)))
            .slot_mut(|s| add_flags(s, HitFlags::GESTURE | HitFlags::INTERACTIVE))
    }

    // ── channels ──────────────────────────────────────────────────────────────────

    /// Binds this node's alpha to `v`.
    ///
    /// A constant produces no `Cell` and no `Effect`; anything else becomes one `Effect`.
    #[must_use]
    pub fn opacity<M>(self, v: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::Opacity, Motion::Chrome, Unit::Direct, v)
    }

    /// Binds this node's rotation to `radians`.
    ///
    /// The surface takes radians only; there is no degrees twin.
    ///
    /// A raw angle is a property this thread owns outright. For a part a pointer turns, use
    /// [`turns`](Self::turns), so this thread and the router do not both drive it.
    #[must_use]
    pub fn rotation<M>(self, radians: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::RotationAngle, Motion::Chrome, Unit::Direct, radians)
    }

    /// Sets the rotation centre in local DIPs. Layout changes snap the pivot.
    #[must_use]
    pub fn pivot<M>(self, point: impl Signal<Vector2, M> + 'static) -> Self {
        self.channel(Prop::Center, Motion::Snap, Unit::Direct, point)
    }

    /// Binds how far a turned part is through its sweep, `0..=1`.
    ///
    /// The twin of [`along`](Self::along), and typed for the same reason. It opens a value
    /// row, so exactly one of this thread and the router moves the part, and whichever does
    /// applies the sweep through the same [`angle_of`](crate::widget::angle_of).
    pub(crate) fn turns<M>(self, v: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::RotationAngle, Motion::Chrome, Unit::Turn, v)
    }

    /// Binds where a moving part sits along its track, `0..=1` of the room it has.
    ///
    /// The room is a layout output — the enclosing control's extent less this part's own — so
    /// the channel records the fraction in [`Unit::Travel`] and the post-solve step
    /// multiplies it out. Bound straight to an offset, the fraction would move the thumb by
    /// one DIP.
    ///
    /// Typed rather than reached through [`channel`](Self::channel): a closure is itself a
    /// value, so an inferred `T` is ambiguous between a signal of `f32` and a constant whose
    /// value is that closure.
    pub(crate) fn along<M>(self, vertical: bool, v: impl Signal<f32, M> + 'static) -> Self {
        let prop = if vertical {
            Prop::OffsetY
        } else {
            Prop::OffsetX
        };
        self.channel(prop, Motion::Chrome, Unit::Travel, v)
    }

    /// Binds how far a level fills its bed, `0..=1`.
    ///
    /// A scale and not an offset, so the fraction is already in the property's own unit: the
    /// bed is the node's own box, and a fraction of it needs nothing from layout.
    pub(crate) fn scale_x<M>(self, v: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::ScaleX, Motion::Chrome, Unit::Direct, v)
    }

    pub(crate) fn hit(self, flags: HitFlags, uia: UiaRole) -> Self {
        self.slot_mut(|s| {
            add_flags(s, flags);
            s.uia = uia;
        })
    }

    pub(crate) fn state(self, policy: StatePolicy) -> Self {
        self.slot_mut(|s| s.state = policy)
    }

    /// Pushes a rung of the surface ladder for this node and everything inside it.
    ///
    /// A composition names it the way it names a [`Role`] or a [`Metric`]: which rung a
    /// surface sits on is a fact about the arrangement, and the palette is still the only
    /// thing that says what the rung *is*. A chassis panel and the plane it sits on are the
    /// same widget at two rungs, and nothing here states a colour to tell them apart.
    #[must_use]
    pub fn elevate(self, elevation: Elevation) -> Self {
        self.slot_mut(|s| s.elevate = Some(elevation))
    }

    /// Reports this node's solved box through `probe`.
    ///
    /// For geometry that has to agree with a layout it is not inside: a gutter's wires
    /// meeting independently-sized rows, a connector between two cards. Where one container
    /// can hold both halves, it places its children instead.
    ///
    /// The value arrives one tick later — see [`Probe`](crate::layout::Probe).
    ///
    /// Two probes on one node keep the last, in the order the modifiers were written. One
    /// probe on two nodes goes undetected here: each node writes the same cell, and the
    /// reader sees whichever solved last.
    #[must_use]
    pub fn probed(self, probe: crate::layout::Probe) -> Self {
        self.slot_mut(|s| s.probe = Some(probe.cell()))
    }

    /// Takes this node out of flow and covers its container's box with it.
    ///
    /// The one way an author states "under everything else here": the covering node paints
    /// first because it is declared first, and the siblings after it are laid out over it as
    /// though it were not there. What a presentation region takes, so the chrome its own
    /// region owns sits over its pixels rather than beside them.
    ///
    /// Absolute at inset zero, which is exactly the pair the chrome sprites already use.
    /// `border` is never set on a style this crate produces, so the container's padding box
    /// is its border box and a zero inset covers the node rather than the space inside its
    /// padding — a covering child therefore ignores the padding that insets its siblings,
    /// which is the point of it.
    #[must_use]
    pub fn cover(self) -> Self {
        self.over(Over::Absolute).over(Over::Inset(Len::Zero))
    }

    /// Opts this node out of touch inflation, for a dense field of targets where inflating
    /// past the drawn rect makes two neighbours both claim one point.
    ///
    /// Recorded on the slot rather than folded into the hit entry, so it holds whatever order
    /// it is called in relative to the modifier that declares the entry. The mount applies it
    /// only where an entry exists: declining an inflation never creates a target, which would
    /// give a non-interactive node a control row and a slot in the array every pointer sample
    /// is resolved against.
    #[must_use]
    pub fn no_inflate(self) -> Self {
        self.slot_mut(|s| s.no_inflate = true)
    }
}

impl El<Any> {
    /// Seeds a bare node, for a caller that wants somewhere to hang overrides.
    ///
    /// On [`Any`] and not on `El<K>`: it answers a [`View`] whatever the kind, so offered
    /// generically `El::<Path>::seed_bare()` would compile and hand back an element with no
    /// `trim`.
    pub(crate) fn seed_bare() -> Self {
        Self::seed(Preset::Bare)
    }

    /// Seeds the viewport of a scroll container: it clips, it does not move, and the tracker
    /// is sourced from it.
    pub(crate) fn viewport(decl: crate::layout::ScrollDecl, content: Self) -> Self {
        Self::seed(Preset::Bare)
            .scrolls(decl)
            .contain(Preset::Scroll, content)
    }
}

impl El<Region> {
    /// Seeds a presentation region: the node that carries the declaration, and the one
    /// sprite that paints its buffer.
    ///
    /// The sprite is minted here rather than by a modifier, because a region with no sprite
    /// is a hole in the layout that draws nothing and reports no error. The role it carries
    /// is never resolved — a presented paint replaces it — and is stated only so the seed is
    /// meaningful to read.
    pub(crate) fn region_seed(seed: crate::present::RegionSeed) -> Self {
        let sink = seed.sink;
        let at = Build::with(|b| b.push_region(seed));
        Self::seed(crate::present::PRESET)
            .slot_mut(|s| s.region = Some(at))
            // One hit entry for the whole region, declared like any control's, so capture,
            // cancel, the recogniser pool and inertia are unchanged and only *which part* of
            // it a contact landed on is new. `Graph` because what is inside is data a client
            // reads rather than a container it walks.
            .hit(HitFlags::INTERACTIVE | HitFlags::GESTURE, UiaRole::Graph)
            .region_sprite(sink)
    }

    fn region_sprite(self, sink: windows_scene::RegionId) -> Self {
        Build::with(|b| {
            b.push_seed(
                self.at,
                SpriteSeed {
                    mask: MaskSeed::Box { radius: None },
                    role: Role::Fill(Fill::Surface),
                    strength: super::arena::FULL,
                    ramp: None,
                    region: Some(sink),
                    // Not interaction-sensitive: a region's pixels are the present thread's,
                    // and a hover inside one is picked against its parts rather than by
                    // re-resolving a colour here.
                    part: Part::Static,
                    next: super::arena::NIL,
                },
            );
        });
        self
    }
}

/// Declares a hit entry, or widens the one already there.
fn add_flags(slot: &mut Slot, flags: HitFlags) {
    slot.hit = Some(match slot.hit {
        Some(hit) => HitSeed {
            flags: hit.flags | flags,
            ..hit
        },
        None => HitSeed {
            flags,
            inflate: None,
        },
    });
}

// ── kind-specific surfaces ───────────────────────────────────────────────────────

/// Selects which row of the button role table a widget reads.
///
/// On [`El<Button>`](Button) alone: these are the indices the button table has. A surface's
/// variants are `card`, `panel` and `flyout`, separate functions over separate rows.
impl El<Button> {
    /// Selects the accent fill, with text that reads on it.
    #[must_use]
    pub fn accent(self) -> Self {
        self.variant(crate::widget::roles::ACCENT)
    }

    /// Selects a tinted fill with accent text and an accent hairline, for a call to action.
    #[must_use]
    pub fn accent_subtle(self) -> Self {
        self.variant(crate::widget::roles::ACCENT_SUBTLE)
    }

    /// Selects a row with no fill and no stroke, so neither sprite is minted.
    #[must_use]
    pub fn ghost(self) -> Self {
        self.variant(crate::widget::roles::GHOST)
    }
}

impl El<Path> {
    /// Outlines the path with a retained HDR gradient.
    #[must_use]
    pub fn stroke_ramp(self, id: RampId, width: impl Into<Len>) -> Self {
        Build::with(|b| {
            b.push_seed(
                self.at,
                SpriteSeed {
                    mask: MaskSeed::Shape {
                        stroke: Some(width.into()),
                    },
                    role: Role::Fill(Fill::Surface),
                    strength: super::arena::FULL,
                    ramp: Some(id),
                    region: None,
                    part: Part::Border,
                    next: super::arena::NIL,
                },
            );
        });
        self
    }
    /// Fills this node's geometry in a chromatic, application-defined role.
    #[must_use]
    pub fn fill(self, role: DataRole) -> Self {
        self.sprite(
            MaskSeed::Shape { stroke: None },
            Role::Data(role),
            Part::Fill,
        )
    }

    /// Outlines this node's geometry in `role`, `width` wide.
    #[must_use]
    pub fn stroke(self, role: DataRole, width: impl Into<Len>) -> Self {
        self.sprite(
            MaskSeed::Shape {
                stroke: Some(width.into()),
            },
            Role::Data(role),
            Part::Border,
        )
    }

    /// Fills this node's geometry in one of the palette's line roles.
    ///
    /// The third of the three things a path can be painted with, and the one a surface's own
    /// edge takes. [`fill`](Self::fill) is a data role and states what the geometry *means*;
    /// [`ink`](Self::ink) is the enclosing widget's foreground and reads as its content; a
    /// hairline is neither — the light falling on a surface belongs to the palette, and an
    /// application that had to reach for a data role to draw one would be naming a colour.
    #[must_use]
    pub fn line(self, role: crate::role::Stroke) -> Self {
        self.sprite(
            MaskSeed::Shape { stroke: None },
            Role::Stroke(role),
            Part::Border,
        )
    }

    /// Outlines this node's geometry in one of the palette's line roles, `width` wide.
    ///
    /// [`line`](Self::line)'s outline form, and what a hairline actually takes: a rule one
    /// device pixel across is a stroke rather than a filled sliver, and building it as a
    /// sliver puts a shape a pixel tall through the geometry snap, which is where it goes.
    #[must_use]
    pub fn line_stroke(self, role: crate::role::Stroke, width: impl Into<Len>) -> Self {
        self.sprite(
            MaskSeed::Shape {
                stroke: Some(width.into()),
            },
            Role::Stroke(role),
            Part::Border,
        )
    }

    /// Fills this node's geometry in the **enclosing widget's** foreground rather than in a
    /// data role.
    ///
    /// What an icon takes: a glyph inside a button is painted in that button's text colour,
    /// so a variant that moves the text moves the icon with it.
    #[must_use]
    pub fn ink(self) -> Self {
        self.sprite(
            MaskSeed::Shape { stroke: None },
            Role::Text(Text::Primary),
            Part::Label,
        )
    }

    /// Outlines this node's geometry in the **enclosing widget's** foreground, `width` wide.
    ///
    /// [`ink`](Self::ink)'s outline form, and what an outline icon takes: a glyph inside a
    /// button follows that button's text colour, so a variant that moves the text moves the
    /// icon with it. A filled silhouette cannot express a waveform or a crossover.
    #[must_use]
    pub fn ink_stroke(self, width: impl Into<Len>) -> Self {
        self.sprite(
            MaskSeed::Shape {
                stroke: Some(width.into()),
            },
            Role::Text(Text::Primary),
            Part::Label,
        )
    }

    /// Binds the end of the draw-on window. A channel and not a field, so it animates.
    #[must_use]
    pub fn trim<M>(self, end: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::TrimEnd, Motion::Chrome, Unit::Direct, end)
    }

    /// Binds the stroke width.
    #[must_use]
    pub fn stroke_width<M>(self, w: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::StrokeThickness, Motion::Chrome, Unit::Direct, w)
    }
}
