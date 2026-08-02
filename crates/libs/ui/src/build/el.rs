//! `El<K>` — a node under construction, and the whole modifier surface.
//!
//! The bloat risk in a widget set is not the widgets; it is every modifier written once per
//! widget. `.tip()`, `.key()`, `.grow()` and the rest apply to everything, so they live once
//! in `impl<K> El<K>`.
//!
//! `K` is a zero-sized marker with one job: a `Box` has no `trim` and a card has no
//! `.accent()`, and neither should compile. Only [`Path`] and [`Button`] carry one; everything
//! else is `El<Any>`. A kind × property table answers the same question at runtime, by
//! ignoring the property — or, worse, by clamping it into a row that renders as some other
//! widget.

use super::arena::{
    Act, Build, ChanSource, HitSeed, MaskSeed, Part, Slot, SpriteSeed, TextSeed, Unit,
};
use crate::gesture::{DragDecl, GestureDecl};
use crate::layout::{Align, Len, Over, Preset, Track};
use crate::role::{DataRole, Elevation, Metric, Role, Text, TypeRole};
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
/// A kind exists where a method would otherwise be offered on elements that cannot honour
/// it: `.accent()` on a card would rewrite an index into the *surface* table and silently
/// render a panel. A missing method says so; a clamped index does not.
#[derive(Copy, Clone, Debug)]
pub struct Button;

/// A node under construction: an index into the thread's build arena.
///
/// `Copy`, and eight bytes. There is no `Rc` to clone, so a `move ||` capture is trivial —
/// the same property that makes [`Cell`](crate::signal::Cell) cheap, extended to elements.
pub struct El<K = Any> {
    pub(crate) at: u32,
    /// `fn() -> K` rather than `K`: a marker must not lend the element its auto-traits and
    /// must not make it invariant.
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

/// What a channel accepts.
///
/// `pub(crate)`: every channel is reached through a named method, so this constrains the
/// lowering rather than the authoring surface and nothing outside can usefully implement it.
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

    /// Forgets the kind. What every container does to its children.
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
        Build::with(|b| b.push_over(self.at, over));
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

    /// The one reactive lowering's build half: a constant is stored inline and produces no
    /// graph node; anything else becomes an `Effect` at mount.
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

    /// The run this node draws. `ink` of `None` takes the enclosing widget's chrome row, so
    /// a variant that moves the text colour does not have to reach into the text seed.
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

    /// The table row this node's surface comes from.
    pub(crate) fn chrome(self, roles: &'static [RoleSet], variant: u8, radius: Metric) -> Self {
        self.slot_mut(|s| {
            s.chrome = Some(Chrome {
                roles,
                variant,
                radius,
            });
        })
    }

    /// Rewrites which row of its own table this widget reads.
    ///
    /// One byte. Everything a variant costs — a fill that is not minted, a stroke that is —
    /// is decided at mount from the row, so this can run in any order with anything else.
    ///
    /// Reachable only from a kind that carries a table ([`Button`]), so "this element has no
    /// variants" is a missing method rather than a write that goes nowhere.
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

    /// The part a value moves: a toggle's knob, a slider's thumb, a meter's level.
    ///
    /// Marked as the thumb rather than as another fill, so the router can move exactly it
    /// and the state driver can leave the rest of the control alone.
    pub(crate) fn thumb(self, radius: Metric, role: Role) -> Self {
        self.sprite(
            MaskSeed::Box {
                radius: Some(Len::Metric(radius)),
            },
            role,
            Part::Thumb,
        )
    }

    /// The geometry this node's shape sprites draw.
    pub(crate) fn geom(self, geom: GeomId) -> Self {
        self.slot_mut(|s| s.geom = Some(geom))
    }

    /// A height of `n` row heights. The count is reactive; the height is the palette's.
    ///
    /// `pub(crate)`: its one caller is the virtualized list's spacers, where the number is a
    /// **count** of unrealized rows. Offered publicly it would be the only value-taking
    /// modifier that is not a [`Signal`], for a case an author does not have.
    pub(crate) fn height_rows(
        self,
        row: impl Fn() -> Metric + 'static,
        n: impl Fn() -> f32 + 'static,
    ) -> Self {
        self.act(Act::Restyle(Box::new(move || {
            Over::Height(Len::Times(row(), n().max(0.0)))
        })))
    }

    /// Makes this container scroll: a tracker on its own box, and the content bound to it.
    pub(crate) fn scrolls(self, reveal: crate::layout::Reveal) -> Self {
        Build::with(|b| {
            // Touch is handed to the tracker rather than to a recogniser, which is what
            // keeps a fling running while the front thread is busy. A knob must never have
            // this; a scroll surface must always.
            b.gesture_mut(self.at, |decl| decl.redirect = true);
            let slot = &mut b.nodes[self.at as usize];
            slot.scroll = Some(reveal);
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

    /// A column. Children stretch.
    #[must_use]
    pub fn stack(self, children: impl super::IntoChildren) -> Self {
        self.contain(Preset::Stack, children)
    }

    /// A row. Children centre.
    #[must_use]
    pub fn row(self, children: impl super::IntoChildren) -> Self {
        self.contain(Preset::Row, children)
    }

    /// A row that wraps.
    #[must_use]
    pub fn wrap(self, children: impl super::IntoChildren) -> Self {
        self.contain(Preset::Wrap, children)
    }

    /// An explicit grid. Children are auto-placed unless the container places them.
    #[must_use]
    pub fn grid(self, children: impl super::IntoChildren) -> Self {
        self.contain(Preset::Grid, children)
    }

    /// Responsive tiles: `repeat(auto-fill, minmax(min, 1fr))`.
    #[must_use]
    pub fn tiles(self, min: impl Into<Len>, children: impl super::IntoChildren) -> Self {
        self.over(Over::TileMin(min.into()))
            .contain(Preset::Tiles, children)
    }

    /// Adopts a layout class and takes a child list.
    ///
    /// **The class is always set**, unconditionally — chrome is an override
    /// ([`surface`](Self::surface), [`control`](Self::control)), so there is nothing on the
    /// slot for it to displace.
    pub(crate) fn contain(self, preset: Preset, children: impl super::IntoChildren) -> Self {
        // Collected onto the arena's own stack, so a screen of nested containers allocates
        // once at high-water mark rather than once per container per mount.
        let mark = Build::with(|b| b.mark());
        children.append(&mut super::Slots::new());
        Build::with(|b| {
            b.nodes[self.at as usize].preset = preset;
            b.take_kids(self.at, mark);
        });
        self
    }

    /// A surface: a scope push, its own chrome row, and the padding that goes with the rung.
    ///
    /// What `card`, `panel` and `flyout` are between them, stated once. It sets no layout
    /// class, so `card().stack(..)` and `card().row(..)` are both a card. The padding is an
    /// override, so a call site that states its own afterwards wins — overrides apply in
    /// chain order and the last one is the answer.
    pub(crate) fn surface(self, elevation: Elevation, variant: u8, radius: Metric) -> Self {
        self.elevate(elevation)
            .chrome(crate::widget::roles::SURFACE, variant, radius)
            .over(Over::Padding(Len::Metric(Metric::SpaceLg)))
    }

    /// A control: the palette's row height as a floor, a tighter gap, and control padding.
    ///
    /// Overrides too, so a control that wraps its label is still a control.
    pub(crate) fn control(self) -> Self {
        self.over(Over::MinHeight(Len::Metric(Metric::RowH)))
            .over(Over::Padding(Len::Metric(Metric::SpaceMd)))
            .over(Over::Gap(Len::Metric(Metric::SpaceSm)))
            .over(Over::Justify(Align::Center))
    }

    /// Explicit placement, stated by the **container** on the child's behalf.
    ///
    /// The child has no such property to set, so placing one that cannot be placed is a
    /// missing method rather than a silent no-op.
    #[must_use]
    pub fn at<C>(self, row: u16, column: u16, child: El<C>) -> Self {
        self.place_child(row, column, 1, 1, child.erase())
    }

    /// Placement with a span.
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

    /// A grid row track.
    #[must_use]
    pub fn rows(self, tracks: impl IntoIterator<Item = Track>) -> Self {
        for t in tracks {
            self.over(Over::Row(t));
        }
        self
    }

    /// A grid column track.
    #[must_use]
    pub fn cols(self, tracks: impl IntoIterator<Item = Track>) -> Self {
        for t in tracks {
            self.over(Over::Column(t));
        }
        self
    }

    // ── layout: container properties ─────────────────────────────────────────────

    /// Absorb the slack.
    #[must_use]
    pub fn grow(self) -> Self {
        self.over(Over::Grow)
    }

    #[must_use]
    pub fn width(self, l: impl Into<Len>) -> Self {
        self.over(Over::Width(l.into()))
    }

    #[must_use]
    pub fn height(self, l: impl Into<Len>) -> Self {
        self.over(Over::Height(l.into()))
    }

    #[must_use]
    pub fn min_width(self, l: impl Into<Len>) -> Self {
        self.over(Over::MinWidth(l.into()))
    }

    #[must_use]
    pub fn min_height(self, l: impl Into<Len>) -> Self {
        self.over(Over::MinHeight(l.into()))
    }

    #[must_use]
    pub fn max_width(self, l: impl Into<Len>) -> Self {
        self.over(Over::MaxWidth(l.into()))
    }

    #[must_use]
    pub fn padding(self, l: impl Into<Len>) -> Self {
        self.over(Over::Padding(l.into()))
    }

    #[must_use]
    pub fn gap(self, l: impl Into<Len>) -> Self {
        self.over(Over::Gap(l.into()))
    }

    /// How this container aligns **all** of its children, on the cross axis.
    #[must_use]
    pub fn align(self, a: Align) -> Self {
        self.over(Over::Align(a))
    }

    /// How this container distributes its children along the main axis.
    #[must_use]
    pub fn justify(self, a: Align) -> Self {
        self.over(Over::Justify(a))
    }

    /// The rare per-child escape, and the **only** one.
    ///
    /// It is here rather than on the container — where placement went — because it is the
    /// one per-child layout property that cannot silently do nothing: cross-axis alignment
    /// is honoured under every class this crate produces, where `grid_row` on a flex child
    /// is a write that goes nowhere. That distinction is what `no_child_layout` enforces.
    #[must_use]
    pub fn align_self(self, a: Align) -> Self {
        self.over(Over::AlignSelf(a))
    }

    /// Classify this container's own inline size for its subtree.
    ///
    /// A caller never passes a width down. A width variant changes **styles, never
    /// structure**, so nothing unmounts while a window is being dragged across a
    /// threshold.
    #[must_use]
    pub fn responsive(self, narrow_max: f32, medium_max: f32) -> Self {
        self.slot_mut(|s| s.responsive = Some(Bounds([narrow_max, medium_max])))
    }

    // ── presence ──────────────────────────────────────────────────────────────────

    /// Contribute nothing when `cond` is false: no node, no layout participation.
    ///
    /// A **constant** condition is resolved at build time and never reaches the graph. It
    /// marks the slot absent rather than hiding it, so the mount never sees it: no visual,
    /// no style, no shaped run, no mount row. That is what makes the common case — a badge
    /// that a build flag turns off — cost the arena slot and nothing else.
    ///
    /// A **varying** condition is `Display::None`, because the alternative is unmounting a
    /// subtree whose state the user is in the middle of — a half-typed field must survive a
    /// window edge being dragged across a breakpoint.
    ///
    /// There is no `hide_when` twin: a closure is a [`Signal`], so `when(move || !hidden())`
    /// is the same call and the negation reads at the call site rather than in two method
    /// names that have to stay opposite.
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

    /// How this subtree leaves when it is destroyed.
    #[must_use]
    pub fn exit(self, exit: Exit) -> Self {
        self.slot_mut(|s| s.exit = exit)
    }

    // ── identity and assistive technology ─────────────────────────────────────────

    /// The automation-id segment. `&'static str`, so nothing is built at mount: the path is
    /// materialized only if UI Automation asks.
    #[must_use]
    pub fn key(self, key: &'static str) -> Self {
        self.slot_mut(|s| s.key = Some(key))
    }

    /// The accessible name, where it is not derivable from the widget's own text.
    #[must_use]
    pub fn name(self, name: &'static str) -> Self {
        self.slot_mut(|s| s.name = Some(name))
    }

    // ── attachments ───────────────────────────────────────────────────────────────

    /// A hover description. One tooltip exists at a time and the overlay layer owns it; a
    /// widget only declares interest.
    ///
    /// A tip is a hover target, so this declares one. Without that the node has no hit entry,
    /// the mount has nowhere to move the handler to, and the tip is dropped in silence — on
    /// exactly the elements that most want one, since a caption is not otherwise interactive.
    #[must_use]
    pub fn tip(self, tip: impl Into<TextSource>) -> Self {
        self.act(Act::Tip(tip.into()))
            .slot_mut(|s| add_flags(s, HitFlags::INTERACTIVE))
    }

    /// An anchored, light-dismissed surface.
    #[must_use]
    pub fn flyout(self, body: impl Fn() -> View + 'static) -> Self {
        self.act(Act::Flyout(Box::new(body)))
            .slot_mut(|s| add_flags(s, HitFlags::GESTURE))
    }

    /// A drag whose meaning depends on its axis: threshold, then first-past lock.
    #[must_use]
    pub fn drag(self, drag: DragDecl) -> Self {
        Build::with(|b| {
            b.gesture_mut(self.at, |decl| *decl = decl.with_drag(drag));
            add_flags(&mut b.nodes[self.at as usize], HitFlags::GESTURE);
        });
        self
    }

    /// Model state, not interaction chrome: it swaps base roles and drops the hit flags.
    #[must_use]
    pub fn disabled<M>(self, cond: impl Signal<bool, M> + 'static) -> Self {
        self.act(Act::DisabledWhen(Box::new(move || cond.read())))
    }

    /// Model state as well: a discrete paint swap at event rate, and not a wash.
    #[must_use]
    pub fn selected<M>(self, cond: impl Signal<bool, M> + 'static) -> Self {
        self.act(Act::SelectedWhen(Box::new(move || cond.read())))
    }

    #[must_use]
    pub fn on_click(self, f: impl Fn() + 'static) -> Self {
        self.act(Act::Click(Box::new(f)))
            .slot_mut(|s| add_flags(s, HitFlags::GESTURE | HitFlags::INTERACTIVE))
    }

    /// Every value the control produced while it was being moved.
    ///
    /// Declares a target, for the reason [`tip`](Self::tip) does: a handler on a node with
    /// no hit entry has nothing to route to it, and the mount would drop it in silence.
    #[must_use]
    pub fn on_change(self, f: impl Fn(f64) + 'static) -> Self {
        self.act(Act::ChangeF64(Box::new(f)))
            .slot_mut(|s| add_flags(s, HitFlags::GESTURE | HitFlags::INTERACTIVE))
    }

    /// The value it settled on. A canceled contact restores what it was and commits
    /// nothing, which is the drag policy's rule rather than this method's.
    #[must_use]
    pub fn on_commit(self, f: impl Fn(f64) + 'static) -> Self {
        self.act(Act::CommitF64(Box::new(f)))
            .slot_mut(|s| add_flags(s, HitFlags::GESTURE | HitFlags::INTERACTIVE))
    }

    // ── channels ──────────────────────────────────────────────────────────────────

    /// A constant produces no `Cell` and no `Effect`; anything else becomes one `Effect`.
    #[must_use]
    pub fn opacity<M>(self, v: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::Opacity, Motion::Chrome, Unit::Direct, v)
    }

    /// An angle, in **radians**. There is deliberately no degrees twin: a 57× error reads as
    /// a broken control rather than as a unit bug.
    ///
    /// For a part a *pointer* turns, use [`turns`](Self::turns): a raw angle is a property
    /// this thread owns outright, and writing one the router is also driving is the one
    /// consistency hazard the retarget seam leaves open.
    #[must_use]
    pub fn rotation<M>(self, radians: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::RotationAngle, Motion::Chrome, Unit::Direct, radians)
    }

    /// How far a turned part is through its sweep, `0..=1`.
    ///
    /// The twin of [`along`](Self::along), and typed for the same reason: it opens a value
    /// row, so exactly one of this thread and the router moves the part and the sweep is
    /// applied by whichever one does — from the same [`angle_of`](crate::widget::angle_of).
    pub(crate) fn turns<M>(self, v: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::RotationAngle, Motion::Chrome, Unit::Turn, v)
    }

    /// Where a moving part sits along its track, `0..=1` of the room it has.
    ///
    /// The **room is not known here** — it is the enclosing control's extent less this
    /// part's own, which layout decides — so this records a fraction and the post-solve step
    /// multiplies it out. Binding the fraction straight to an offset would move the thumb by
    /// one DIP and look like a control that does not work.
    ///
    /// Typed rather than reached through the generic channel, and that is not stylistic: a
    /// closure is itself a value, so `T` left to inference is ambiguous between "a signal
    /// of `f32`" and "a constant whose value is this closure". Naming the channel names the
    /// type, and the second reading stops existing.
    pub(crate) fn along<M>(self, vertical: bool, v: impl Signal<f32, M> + 'static) -> Self {
        let prop = if vertical {
            Prop::OffsetY
        } else {
            Prop::OffsetX
        };
        self.channel(prop, Motion::Chrome, Unit::Travel, v)
    }

    /// How far a level fills its bed, `0..=1`.
    ///
    /// A **scale** and not an offset, so it is already in the property's own unit: the bed is
    /// the node's own box and a fraction of it needs nothing from layout.
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

    /// Opt out of touch inflation. For a dense field of targets, where inflating past the
    /// drawn rect makes two neighbours both claim one point.
    ///
    /// Recorded on the slot rather than folded into the hit entry, so it does **not** depend
    /// on being called after whatever declared that entry. The mount applies it only where
    /// there is one: declining an inflation must not create a target, which would give a
    /// non-interactive node a control row and a slot in the array every pointer sample is
    /// resolved against.
    #[must_use]
    pub fn no_inflate(self) -> Self {
        self.slot_mut(|s| s.no_inflate = true)
    }
}

impl El<Any> {
    /// A bare node, for a caller that only wants somewhere to hang overrides.
    ///
    /// On [`Any`] and not on `El<K>`: it ignores the kind and answers a [`View`], so offering
    /// it generically would let `El::<Path>::seed_bare()` compile and hand back something
    /// with no `trim`.
    pub(crate) fn seed_bare() -> Self {
        Self::seed(Preset::Bare)
    }

    /// The viewport of a scroll container: it clips, it does not move, and the tracker is
    /// sourced from it.
    pub(crate) fn viewport(reveal: crate::layout::Reveal, content: Self) -> Self {
        Self::seed(Preset::Bare)
            .scrolls(reveal)
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

/// Which row of its own table a widget reads.
///
/// On [`El<Button>`](Button) alone, because those are the indices the button table has.
/// A surface's "variants" are `card`, `panel` and `flyout` — separate functions over
/// separate rows — so there is nothing here for them to get wrong.
impl El<Button> {
    /// The accent fill, with text that reads on it.
    #[must_use]
    pub fn accent(self) -> Self {
        self.variant(crate::widget::roles::ACCENT)
    }

    /// A tinted fill with accent text and an accent hairline. What a call to action is.
    #[must_use]
    pub fn accent_subtle(self) -> Self {
        self.variant(crate::widget::roles::ACCENT_SUBTLE)
    }

    /// No fill and no stroke, so it costs the sprites it does not have.
    #[must_use]
    pub fn ghost(self) -> Self {
        self.variant(crate::widget::roles::GHOST)
    }
}

impl El<Path> {
    /// Fill the geometry with a chromatic, application-defined role.
    #[must_use]
    pub fn fill(self, role: DataRole) -> Self {
        self.sprite(
            MaskSeed::Shape { stroke: None },
            Role::Data(role),
            Part::Fill,
        )
    }

    /// Outline it. The width is a `Metric`, because a hairline is the palette's business.
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

    /// Fill it in the **enclosing widget's** own foreground rather than in a data role.
    ///
    /// What an icon is: a glyph inside a button takes that button's text colour, so a
    /// variant that moves the text moves the icon with it and neither has to know about the
    /// other.
    #[must_use]
    pub fn ink(self) -> Self {
        self.sprite(
            MaskSeed::Shape { stroke: None },
            Role::Text(Text::Primary),
            Part::Label,
        )
    }

    /// The draw-on window. A channel and not a field, because it animates.
    #[must_use]
    pub fn trim<M>(self, end: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::TrimEnd, Motion::Chrome, Unit::Direct, end)
    }

    /// Stroke width, animated.
    #[must_use]
    pub fn stroke_width<M>(self, w: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::StrokeThickness, Motion::Chrome, Unit::Direct, w)
    }
}
