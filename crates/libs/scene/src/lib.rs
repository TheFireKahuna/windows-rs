#![doc = include_str!("../readme.md")]

// ── app half · Send · no COM ────────────────────────────────────────────────────
mod env;
mod hit_build;
mod id;
mod layout;
mod model;
mod patch;
mod quant;
mod responsive;
mod sink;

// ── both halves ─────────────────────────────────────────────────────────────────
mod tree;

// ── front half · !Send · owns every composition object ──────────────────────────
mod anim;
mod apply;
mod backends;
mod bind;
mod cache;
mod census;
mod hit;
mod node;
mod prop;
mod res;
mod tracker;

pub use anim::{CHROME_DAMPING, CHROME_PERIOD, SCROLL_DAMPING, SCROLL_PERIOD};
pub use backends::Backends;
pub use cache::{BoxKey, Cache, Cell, Gen, GenMask, SolidKey};
pub use census::{Audit, Census};
pub use env::Env;
pub use hit::{ContactKind, Hit, HitTable};
pub use hit_build::{
    ControlId, HitBuilder, HitDecl, HitEntry, HitFlags, NO_ENTRY, TOUCH_TARGET_DIPS,
    default_inflation,
};
pub use id::{Id, Ids};
pub use layout::{
    LayoutKind, LayoutTree, Measure, MeasureCtx, MeasureIn, MeasureKey, Rect, Solved, snap,
};
pub use model::{Model, SlotRoot};
pub use patch::{Op, PatchPool, SinkPatch, Span};
pub use quant::{Q, quant_stop};
pub use responsive::{Bounds, WidthClass};
pub use sink::*;
pub use tracker::Phase;

pub use windows_core::Result;

/// Layout styles are `taffy`'s, undecorated — re-exported so a consumer cannot end up
/// version-skewed against the `Style` this crate's signatures name.
pub use taffy;

use crate::anim::Motion;
use crate::cache::Cells;
use crate::node::{Painted, Slots};
use crate::res::Resources;
use crate::tracker::{EventQueue, TrackerState};
use core::marker::PhantomData;
use std::cell::RefCell;
use std::rc::Rc;
use windows_composition::{DesktopWindowTarget, Stretch, Visual, VisualInteractionSource};
use windows_numerics::{Vector2, Vector3};
use windows_window::{Wake, Window};

/// What the front half reports upward.
///
/// One channel down and one up, and nothing beside them. Solved layout crosses in neither
/// direction: it becomes bind and hit ops inside the patch.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SceneEvent {
    /// A tracker moved. **The only trustworthy read of one**: it runs in another process,
    /// and its getter answers with what was last set, not what the compositor is
    /// evaluating.
    TrackerValues {
        tracker: Id<Tracker>,
        position: Vector2,
        scale: f32,
    },
    /// A tracker changed phase.
    TrackerPhase { tracker: Id<Tracker>, phase: Phase },
    /// Inertia began, and where it will land is already known — which is what makes
    /// destination prefetch possible while the compositor animates.
    InertiaStarting {
        tracker: Id<Tracker>,
        natural: Vector2,
        /// Where it will actually rest once snap points are applied.
        modified: Vector2,
        /// Whether the motion came from a wheel notch rather than a fling, which is worth a
        /// shorter decay.
        from_impulse: bool,
    },
    /// A request was dropped. Not an error — a position update arriving while the user is
    /// manipulating is documented to be ignored. Drop it and reconcile against the next
    /// values change; **never re-apply it blindly**, or a user whose manipulation ends gets
    /// a double jump.
    RequestIgnored { tracker: Id<Tracker>, request: i32 },
    /// An exit transition finished and its ghost was released.
    GhostReleased,
    /// A timed reveal reached its deadline — a submenu's hover-open, a tooltip's show.
    ///
    /// Raised on the first frame at or past that deadline, never by a timer firing: the delay
    /// is a comparison the scene makes while it is already awake. A cancelled delay never
    /// arrives here.
    DelayElapsed { delay: DelayId },
    /// The device was lost and everything under it has been rebuilt.
    DeviceRebuilt,
    /// The pixel grid moved, so the resources rasterized *at device resolution* are at the
    /// wrong one.
    ///
    /// That is coverage tiles and nothing else: a gradient is a fixed strip stretched to
    /// fill, geometry is vector, and a colour cell is four texels of one value. The
    /// response is to re-emit every run through [`Model::set_run`](crate::Model::set_run),
    /// which re-points the brush each sprite already holds.
    ///
    /// Same rule as [`DeviceRebuilt`](Self::DeviceRebuilt): **the surfaces behind shared
    /// resources are the model's to re-emit.** Keeping a copy of every run's glyphs here to
    /// redraw from would be a second lifetime mechanism for data the model holds.
    ScaleChanged { scale: f32 },
}

/// The front thread's half of the scene.
///
/// `!Send` by construction, making the ownership rules a compile error.
///
/// # Owners, not a field list
///
/// State is grouped so the code below borrows *what it needs*: realizing a sprite takes a
/// node, the resources, the cells and the backends, and its signature says so. Behind one
/// `&mut self` no function can hold a node across a call, which shows up as the same id
/// looked up three times in one body.
///
/// # There is no commit
///
/// `Windows.UI.Composition` has none — the vtable carries no such method. Changes publish
/// when the thread's dispatcher queue finishes the current work item, so **one tick is one
/// publish** and an idle window publishes nothing because it never ticks. Hence the rule:
/// composition objects are touched *inside* the tick and nowhere else.
pub struct Scene {
    #[expect(
        dead_code,
        reason = "held to keep the visual tree attached to the window"
    )]
    target: DesktopWindowTarget,
    /// What has been invalidated since the tree was realized.
    pub(crate) generation: Gen,
    /// The environment the tree was last realized under.
    ///
    /// A **watermark**, not an authority: its only reader is the comparison that decides
    /// what a move invalidated. Nothing asks this scene what the DPI is. `None` until the
    /// first operation states one.
    env: Option<Env>,
    pub(crate) nodes: Slots<node::Node>,
    root: NodeId,
    /// Id-keyed and shared between sprites.
    pub(crate) res: Resources,
    /// Value-keyed and evicted.
    pub(crate) cells: Cells,
    pub(crate) motion: Motion,
    pub(crate) trackers: Slots<TrackerState>,
    pub(crate) hits: HitTable,
    events: EventQueue,
    /// Held, unlike the [`Backends`] and the [`Env`], because an exit animation's `Tick`
    /// outlives the call that started it. **Store what must outlive a call; pass what
    /// needn't** — that rule is the whole of why this is a field and they are arguments.
    pub(crate) wake: Wake,
    pub(crate) census: Census,
    /// Channels [`retarget`](Scene::retarget) has claimed for the front thread.
    ///
    /// The one hazard a front-side write leaves open is *semantic*: the app writing a
    /// channel the router is driving — a thumb offset set mid-drag. Consistency is not at
    /// risk, because there is one shadow and one setter and both are here, so this exists
    /// only to name the two writers at the moment they fight. Zero release cost.
    #[cfg(debug_assertions)]
    pub(crate) front_owned: rustc_hash::FxHashSet<(NodeId, Prop)>,
    _not_send: PhantomData<*const ()>,
}

impl Scene {
    /// Builds a scene hosted on `window`, drawn with `back`.
    ///
    /// `back` is borrowed to mint the target and the root and is **not** stored: every
    /// later operation states it again. The calling thread must pump `window`'s messages —
    /// that is where every tracker callback lands and where every publish happens. `wake`
    /// is the window's own frame clock, which exit animations hold open while they play.
    pub fn new(window: &Window, back: &Backends, wake: Wake) -> Result<Self> {
        let target = back
            .compositor
            .create_desktop_window_target(window, false)?;
        let motion = Motion::new(&back.compositor);

        let root_visual = back.compositor.create_container_visual();
        target.set_root(&root_visual);

        let mut nodes = Slots::default();
        nodes.insert(
            NodeId::ROOT,
            node::Node::new(base_of_group(&root_visual), None, NodeKind::Group),
        );

        Ok(Self {
            target,
            generation: Gen::default(),
            env: None,
            nodes,
            root: NodeId::ROOT,
            res: Resources::default(),
            cells: Cells::default(),
            motion,
            trackers: Slots::default(),
            hits: HitTable::default(),
            events: Rc::new(RefCell::new(Vec::new())),
            wake,
            census: Census::default(),
            #[cfg(debug_assertions)]
            front_owned: rustc_hash::FxHashSet::default(),
            _not_send: PhantomData,
        })
    }

    /// The window subtree's root.
    #[must_use]
    pub const fn root(&self) -> GroupId {
        GroupId(self.root)
    }

    /// The hit array — the single authority every consumer resolves through.
    #[must_use]
    pub fn hits(&self) -> &HitTable {
        &self.hits
    }

    /// What is under `p`.
    pub fn hit(&self, p: Point, contact: ContactKind) -> Option<Hit> {
        self.hits.hit(p, contact)
    }

    /// What this crate has done since it started.
    #[must_use]
    pub const fn census(&self) -> &Census {
        &self.census
    }

    /// Walks the tree and reports what it actually holds.
    ///
    /// Visual count is the compositor's frontier at idle, so it is the one tally worth
    /// corroborating: `visuals_live` can only be wrong if a life event is, and no rendered
    /// frame would show that.
    #[must_use]
    pub fn audit(&self) -> Audit {
        fn walk(nodes: &Slots<node::Node>, at: NodeId, reached: &mut u32) {
            *reached += 1;
            for child in tree::children(nodes, at) {
                walk(nodes, child, reached);
            }
        }
        let mut reached = 0;
        walk(&self.nodes, self.root, &mut reached);
        Audit {
            reached,
            held: self.nodes.len() as u32,
        }
    }

    /// Takes everything the trackers and the batches have reported.
    ///
    /// Both reconciliations are folded in here because both are contracts: a reported
    /// position is the only trustworthy read of a tracker and is what the hit query resolves
    /// a scroll ancestry through, and an ignored request must be dropped, never re-applied.
    pub fn drain_events(&mut self, out: &mut Vec<SceneEvent>) {
        // Only what this call appends is reconciled. `out` is the caller's buffer and may
        // still hold a previous drain's events; applying a tracker position twice is not
        // idempotent, so the range is taken rather than the vector.
        let appended = out.len();
        // Delays that came due. Swept here rather than in `apply`, because a delay elapsing
        // is not a patch arriving: the tick it lands on may carry nothing at all, and this is
        // the call whose job is to say what the scene has to report. Each one releases its
        // own `Tick` as it goes, so the clock parks when the last expires.
        //
        // The clock is read once and only when something is waiting on it, so the ordinary
        // frame — nothing pending — does not read it at all, and two delays that came due
        // together report together rather than one frame apart.
        if !self.motion.delays.is_empty() {
            let now = std::time::Instant::now();
            self.motion.delays.retain(|delay| {
                let elapsed = delay.elapsed(now);
                if elapsed {
                    out.push(SceneEvent::DelayElapsed { delay: delay.id });
                }
                !elapsed
            });
        }
        out.append(&mut self.events.borrow_mut());
        for event in &out[appended..] {
            match *event {
                SceneEvent::TrackerValues {
                    tracker,
                    position,
                    scale,
                } => {
                    let viewport = self.trackers.get_mut(tracker).and_then(|state| {
                        state.values_changed(
                            Vector3 {
                                x: position.x,
                                y: position.y,
                                z: 0.0,
                            },
                            scale,
                        );
                        state.viewport
                    });
                    if let Some(node) = viewport {
                        self.hits.set_scroll(node, position);
                    }
                }
                SceneEvent::RequestIgnored { tracker, request } => {
                    if let Some(state) = self.trackers.get_mut(tracker) {
                        state.ignored(request);
                    }
                }
                SceneEvent::TrackerPhase { tracker, phase } => {
                    if let Some(state) = self.trackers.get_mut(tracker) {
                        state.phase = phase;
                    }
                }
                _ => {}
            }
        }
    }

    // ── the environment ───────────────────────────────────────────────────────────

    /// Brings the tree up to date with `env`, rebinding whatever the move invalidated.
    ///
    /// Every operation that can rasterize goes through here first, which is what makes a
    /// stale environment unrepresentable rather than a caller's responsibility to remember.
    pub(crate) fn sync(&mut self, back: &Backends, env: Env) -> Result<()> {
        let Some(last) = self.env.replace(env) else {
            // The first environment is not a change: nothing has been realized under an
            // older one.
            return Ok(());
        };
        if last == env {
            return Ok(());
        }
        let grid_moved = last.geometry_moved(env);
        self.generation.dpi = self.generation.dpi.wrapping_add(u32::from(grid_moved));
        self.generation.color = self
            .generation
            .color
            .wrapping_add(u32::from(last.light_moved(env)));
        // Cache-backed cells re-rasterize themselves from their keys; a coverage tile is
        // keyed by nothing this side holds, so the model is told and re-emits it.
        if grid_moved {
            self.events
                .borrow_mut()
                .push(SceneEvent::ScaleChanged { scale: env.scale() });
        }
        self.refresh(back, env)
    }

    /// Rebuilds everything under a lost device.
    ///
    /// **No per-kind recovery code.** Every brush is a pure function of a cache key or a
    /// resource id, so recovery is "bump the generation, drop the cells, refresh" — the path
    /// a DPI change and a first bind both take. Shadowed values, tracker positions and the
    /// hit table live in Rust and need no recovery.
    ///
    /// The surfaces *behind* shared resources are the model's to re-emit, which is what
    /// [`SceneEvent::DeviceRebuilt`] is for. Their brushes survive and re-point in place.
    ///
    /// Repairing the device itself belongs to whoever owns it: call [`Backends::adopt`]
    /// with the replacement GPU first, then this to re-realize everything drawn with it.
    pub fn device_lost(&mut self, back: &Backends, env: Env) -> Result<()> {
        self.generation.device = self.generation.device.wrapping_add(1);
        self.cells.clear();
        self.env = Some(env);
        self.refresh(back, env)?;
        self.events.borrow_mut().push(SceneEvent::DeviceRebuilt);
        Ok(())
    }

    /// Rebinds every sprite whose realized chain reads a generation that has moved.
    ///
    /// The **one** response to an invalidation, whichever generation moved. A sprite reads
    /// only what its own mask and paint read, so a theme flip leaves shapes alone and a
    /// monitor change leaves solid fills alone — the selectivity is in the declaration.
    fn refresh(&mut self, back: &Backends, env: Env) -> Result<()> {
        let now = self.generation;
        for id in self.sprites_where(|painted| !painted.fresh(now)) {
            self.rebind(SpriteId(id), back, env)?;
        }
        Ok(())
    }

    /// The sprites whose declaration satisfies `predicate`, as a snapshot.
    ///
    /// Allocating, and only reached from a display event or a presented buffer handing
    /// over — both already rebuilding brushes.
    fn sprites_where(&self, predicate: impl Fn(&Painted) -> bool) -> Vec<NodeId> {
        self.nodes
            .iter_ids()
            .filter(|id| {
                self.nodes
                    .get(*id)
                    .and_then(|n| n.painted.as_ref())
                    .is_some_and(&predicate)
            })
            .collect()
    }

    // ── presented regions ─────────────────────────────────────────────────────────

    /// Points a region's slot at a buffer the producer presents into.
    ///
    /// # Safety
    ///
    /// `handle` must be a live composition surface handle that outlives the binding. The
    /// compositor does not take ownership.
    pub unsafe fn set_region(
        &mut self,
        region: RegionId,
        handle: *mut core::ffi::c_void,
        back: &Backends,
        env: Env,
    ) -> Result<()> {
        self.sync(back, env)?;
        // SAFETY: the caller's obligation, restated.
        let surface = unsafe { back.compositor.create_surface_for_handle(handle)? };
        // The buffer is already at device resolution, so it samples one-to-one and is never
        // stretched — every pixel guarantee a presented region makes depends on that.
        let brush = back.brush(&surface, Stretch::None);
        if let Some(res) = self.res.regions.get_mut(region) {
            res.value = Some(brush);
        }
        self.rebind_region(region, back, env)
    }

    /// Releases a region's buffer.
    ///
    /// Cleared, not merely dropped: the compositor holds a reference to whatever a visual
    /// paints with, so a brush over a handle the producer is about to close must leave the
    /// tree first.
    pub fn clear_region(&mut self, region: RegionId, back: &Backends, env: Env) -> Result<()> {
        self.sync(back, env)?;
        if let Some(res) = self.res.regions.get_mut(region) {
            res.value = None;
        }
        self.rebind_region(region, back, env)
    }

    fn rebind_region(&mut self, region: RegionId, back: &Backends, env: Env) -> Result<()> {
        for id in self.sprites_where(|painted| painted.paint == Paint::Presented(region)) {
            self.rebind(SpriteId(id), back, env)?;
        }
        Ok(())
    }

    // ── trackers ──────────────────────────────────────────────────────────────────

    /// Creates a tracker on `viewport`.
    ///
    /// The marker decides whether an owner is attached, and it is not a tuning knob: an
    /// owner is supplied at construction with no per-callback subscription, so a tracker
    /// needing one event pays for all six. [`request`](Self::request) will not accept a
    /// passive id, so a surface cannot be given callbacks it does not read.
    pub fn tracker<O: Observe>(
        &mut self,
        id: TrackerId<O>,
        viewport: GroupId,
        axes: Axes,
        back: &Backends,
    ) -> Result<()> {
        let visual = self
            .nodes
            .get(viewport.node())
            .map(|n| n.visual.clone())
            .ok_or_else(invalid_arg)?;

        let tracker = if O::OWNED {
            let queue = Rc::clone(&self.events);
            let raw = id.raw;
            back.compositor
                .create_interaction_tracker_with_owner(move |event| {
                    queue.borrow_mut().push(tracker::translate(raw, event));
                })?
        } else {
            back.compositor.create_interaction_tracker()?
        };

        let source = VisualInteractionSource::for_visual(&visual)?;
        tracker::configure_source(&source, axes)?;
        tracker.add_source(&source)?;

        let mut state = TrackerState::new(tracker);
        state.source = Some(source);
        state.viewport = Some(viewport.node());
        self.trackers.insert(id.raw, state);
        Ok(())
    }

    /// Asks an observed tracker to move.
    pub fn request(&mut self, id: TrackerId<Observed>, request: TrackerRequest) -> Result<i32> {
        let state = self.trackers.get_mut(id.raw).ok_or_else(invalid_arg)?;
        state.request(request).map(|r| r.0)
    }
}

/// The base visual of a container, which is what a node stores.
///
/// Named because the deref chain resolves `clone` to the derived type's own.
pub(crate) fn base_of_group(group: &windows_composition::ContainerVisual) -> Visual {
    (**group).clone()
}

/// The base visual of a sprite. Two derefs: a sprite visual *is* a container visual, the
/// same fact that lets one arena hold both kinds of node.
pub(crate) fn base_of_sprite(sprite: &windows_composition::SpriteVisual) -> Visual {
    (***sprite).clone()
}

/// The base visual of a shape host, for a capture that takes the base type.
pub(crate) fn base_of_shape(shape: &windows_composition::ShapeVisual) -> Visual {
    (**shape).clone()
}

pub(crate) fn invalid_arg() -> windows_core::Error {
    windows_core::Error::from(windows_core::HRESULT(-2147024809))
}
