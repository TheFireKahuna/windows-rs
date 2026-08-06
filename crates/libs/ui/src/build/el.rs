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
use crate::role::{DataRole, Elevation, Metric, Role, Text, TypeRole, WidthClass};
use crate::signal::Signal;
use crate::widget::{Chrome, Flow, Interaction, Motion, RoleSet, StatePolicy, TextSource, UiaRole};
use core::marker::PhantomData;
use windows_numerics::Vector2;
use windows_scene::{Bounds, Exit, GeomId, HitFlags, Prop, Value};

/// The default kind: no methods beyond the universal surface.
#[derive(Copy, Clone, Debug)]
pub struct Any;
/// A geometry sprite. Owns `fill` / `stroke` / `ink` / `trim`.
#[derive(Copy, Clone, Debug)]
pub struct Path;
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
        Build::with(|b| {
            b.push_seed(
                self.at,
                SpriteSeed {
                    mask,
                    role,
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
    ) -> Self {
        let text = Build::with(|b| {
            b.push_text(TextSeed {
                source: Some(source),
                ramp,
                ink,
                flow,
            })
        });
        self.sprite(
            MaskSeed::Run { text },
            Role::Text(ink.unwrap_or(Text::Primary)),
            Part::Label,
        )
    }

    /// Records the role table, variant index and corner radius this node's surface resolves
    /// from.
    pub(crate) fn chrome(self, roles: &'static [RoleSet], variant: u8, radius: Metric) -> Self {
        self.slot_mut(|s| {
            s.chrome = Some(Chrome {
                roles,
                variant,
                radius,
            });
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
    pub(crate) fn thumb(self, radius: Metric, role: Role) -> Self {
        self.sprite(
            MaskSeed::Box {
                radius: Some(Len::Metric(radius)),
            },
            role,
            Part::Thumb,
        )
    }

    /// Names the geometry this node's shape sprites draw.
    pub(crate) fn geom(self, geom: GeomId) -> Self {
        self.slot_mut(|s| s.geom = Some(geom))
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
            out.push(Over::Height(Len::Times(row(), n().max(0.0))));
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
            out.push(Over::Band {
                at: Len::Times(row, index),
                height: Len::Metric(row),
            });
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
            out.push(Over::ClearColumns);
            out.extend(tracks.iter().copied().map(Over::Column));
        })))
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

    /// Runs `f` for every value the control produces while it is being moved.
    ///
    /// Declares a hit entry, for the reason [`tip`](Self::tip) does: a handler on a node with
    /// no hit entry has nothing routing to it, and the mount would drop it in silence.
    #[must_use]
    pub fn on_change(self, f: impl Fn(f64) + 'static) -> Self {
        self.act(Act::ChangeF64(Box::new(f)))
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

    pub(crate) fn elevate(self, elevation: Elevation) -> Self {
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
