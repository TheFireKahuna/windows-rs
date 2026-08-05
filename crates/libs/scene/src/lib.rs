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
mod backdrop;
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
pub use backdrop::{BackdropSpec, Glow};
pub use backends::Backends;
pub use cache::{BoxKey, Cache, Cell, Gen, GenMask, SolidKey};
pub use census::{Audit, Census};
pub use env::Env;
pub use hit::{ContactKind, Hit, HitTable, scan};
pub use hit_build::{
    ControlId, HitBuilder, HitDecl, HitEntry, HitFlags, NO_ENTRY, TOUCH_TARGET_DIPS,
    default_inflation,
};
pub use id::{Id, Ids, Slots};
pub use layout::{
    Avail, LayoutKind, LayoutTree, Measure, MeasureCtx, MeasureIn, MeasureKey, Rect, Restyle,
    Solved, snap,
};
pub use model::{Model, SlotRoot};
pub use patch::{Attach, Op, PatchPool, SinkPatch, Span};
pub use quant::{Q, quant_stop};
pub use responsive::{Bounds, WidthClass};
pub use sink::*;
pub use tracker::Phase;

pub use windows_core::Result;

/// Re-exports `taffy`, whose undecorated `Style` this crate's signatures name, so a consumer
/// binds against the same version.
pub use taffy;

use crate::anim::Motion;
use crate::cache::Cells;
use crate::node::Painted;
use crate::res::Resources;
use crate::tracker::{EventQueue, Events, TrackerState};
use core::marker::PhantomData;
use std::cell::RefCell;
use std::rc::Rc;
use windows_composition::{
    ContainerVisual, DesktopWindowTarget, Stretch, Visual, VisualInteractionSource,
};
use windows_numerics::{Vector2, Vector3};
use windows_window::{Wake, Window};

/// Carries what the front half reports upward.
///
/// This enum is the only channel up, as [`SinkPatch`] is the only channel down. Solved
/// layout crosses in neither direction: it becomes bind and hit ops inside the patch.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SceneEvent {
    /// Reports a tracker's position and scale.
    ///
    /// The only accurate read of a tracker: it evaluates in another process, and its getter
    /// answers with the value last set rather than the value being evaluated.
    TrackerValues {
        tracker: Id<Tracker>,
        position: Vector2,
        scale: f32,
    },
    /// Reports a tracker's new phase.
    TrackerPhase { tracker: Id<Tracker>, phase: Phase },
    /// Reports that inertia began, with the resting position already known — so a consumer
    /// can prefetch the destination while the compositor animates toward it.
    InertiaStarting {
        tracker: Id<Tracker>,
        natural: Vector2,
        /// Where the motion rests once snap points are applied.
        modified: Vector2,
        /// Whether the motion came from a wheel notch rather than a fling, which a caller
        /// can decay faster.
        from_impulse: bool,
    },
    /// Reports that a tracker dropped a request.
    ///
    /// Not an error: a position update that arrives while the user is manipulating is
    /// ignored by the tracker. A caller drops the request and reconciles against the next
    /// [`TrackerValues`](Self::TrackerValues); re-applying it moves the tracker twice once
    /// the manipulation ends.
    RequestIgnored { tracker: Id<Tracker>, request: i32 },
    /// Reports that a timed reveal reached its deadline, such as a submenu's hover-open or
    /// a tooltip's show.
    ///
    /// Raised on the first frame at or past the deadline; no timer fires, because the
    /// deadline is compared while the scene is already awake. A cancelled delay is never
    /// reported.
    DelayElapsed { delay: DelayId },
    /// Reports that the device was lost and everything under it has been rebuilt.
    DeviceRebuilt,
    /// Reports that the pixel grid moved, leaving the resources rasterized *at device
    /// resolution* built for the wrong one.
    ///
    /// Coverage tiles are the only such resource: a gradient is a fixed strip stretched to
    /// fill, geometry is vector, and a colour cell is four texels of one value. The consumer
    /// re-emits every run through [`Model::set_run`](crate::Model::set_run), which re-points
    /// the brush each sprite already holds.
    ///
    /// As with [`DeviceRebuilt`](Self::DeviceRebuilt), the surfaces behind shared resources
    /// are the model's to re-emit; this crate keeps no copy of a run's glyphs.
    ScaleChanged { scale: f32 },
}

/// Owns the front thread's half of the scene: every composition object, the node arena, the
/// resource tables and the hit array.
///
/// `!Send` by construction, so the thread rule is a compile error rather than a convention.
///
/// # Publishing
///
/// `Windows.UI.Composition` has no commit method. Changes publish when the thread's
/// dispatcher queue finishes the current work item, so one tick is one publish and an idle
/// window publishes nothing because it never ticks. Composition objects are therefore touched
/// only inside a tick.
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
    /// Read only by the comparison that decides what a display move invalidated; no caller
    /// asks the scene for the DPI. `None` until the first operation states one.
    env: Option<Env>,
    pub(crate) nodes: Slots<Node, node::Node>,
    /// The target's root, and the one visual carrying the DIP-to-pixel scale.
    ///
    /// Not an arena node, and neither are the three bands under it: the arena holds only
    /// the nodes the model named.
    window: ContainerVisual,
    /// [`Attach::Window`]. Above the ground, below every overlay.
    content: ContainerVisual,
    /// [`Attach::Detached`] — slot roots — and the ghosts an exit transition leaves
    /// behind. A band, so an overlay sits above content by its position in the tree
    /// rather than by an ordering every caller has to keep.
    overlays: ContainerVisual,
    /// The window's ground: under every root, out of the arena, and not in the hit
    /// array. Held because a display change re-lights it and a resize re-places it.
    backdrop: backdrop::Backdrop,
    /// Every node with no parent node, in attachment order.
    ///
    /// [`audit`](Scene::audit) walks from these. A forest and not a tree: a slot root is a
    /// second root rather than a child placed oddly.
    roots: Vec<NodeId>,
    /// Id-keyed and shared between sprites.
    pub(crate) res: Resources,
    /// Value-keyed and evicted.
    pub(crate) cells: Cells,
    pub(crate) motion: Motion,
    pub(crate) trackers: Slots<Tracker, TrackerState>,
    pub(crate) hits: HitTable,
    events: EventQueue,
    /// Held, unlike the [`Backends`] and the [`Env`], because an exit animation's `Tick`
    /// outlives the call that started it.
    pub(crate) wake: Wake,
    pub(crate) census: Census,
    /// Channels [`retarget`](Scene::retarget) has claimed for the front thread.
    ///
    /// A front-side write cannot tear the shadow — there is one shadow and one setter, both
    /// on this thread — so the set catches the semantic conflict instead: the app writing a
    /// channel the front thread is driving, such as a thumb offset set mid-drag. Debug
    /// builds only.
    #[cfg(debug_assertions)]
    pub(crate) front_owned: rustc_hash::FxHashSet<(NodeId, Prop)>,
    _not_send: PhantomData<*const ()>,
}

impl Scene {
    /// Builds a scene hosted on `window`, drawn with `back`.
    ///
    /// `back` is borrowed to mint the target, the root and the backdrop, and is not stored:
    /// every later operation states it again. `wake` is the window's own frame clock, which
    /// exit animations hold open while they play. `env` paints the backdrop, whose colours
    /// are the display's, before the window is shown; nothing caches it, and
    /// [`apply`](Self::apply) states it again.
    ///
    /// The calling thread must pump `window`'s messages: every tracker callback lands there,
    /// and that is where changes publish.
    ///
    /// # Errors
    ///
    /// Fails when the compositor cannot mint the window target or the backdrop's surfaces.
    pub fn new(
        window: &Window,
        back: &Backends,
        wake: Wake,
        env: Env,
        backdrop: BackdropSpec,
    ) -> Result<Self> {
        let target = back
            .compositor
            .create_desktop_window_target(window, false)?;
        let motion = Motion::new(&back.compositor);

        let root_visual = back.compositor.create_container_visual();
        target.set_root(&root_visual);
        // The one place DIPs become pixels. Everything below this visual is authored in DIPs
        // and snapped onto the pixel grid by `quant` at this same scale; a different number
        // there lands geometry between pixels at every scale but 100%.
        set_dip_space(&root_visual, env);

        // Three bands, bottom to top: the ground, the content, the overlays. Their order is
        // fixed by the tree, so `after: None` means the bottom of one band's collection
        // rather than the bottom of the window.
        let ground = back.compositor.create_container_visual();
        let content = back.compositor.create_container_visual();
        let overlays = back.compositor.create_container_visual();
        let bands = root_visual.children();
        bands.insert_at_top(&ground);
        bands.insert_at_top(&content);
        bands.insert_at_top(&overlays);
        // Each band is the window, stated once as a fraction of the root, so a window resize
        // writes no property here: the compositor re-derives every band from the root's own
        // extent. A band's extent clips nothing on its own — only a `Clip` does — so giving
        // one a size costs the content band nothing.
        for band in [&ground, &content, &overlays] {
            band.set_relative_size_adjustment(Vector2 { x: 1.0, y: 1.0 });
        }

        let backdrop = backdrop::Backdrop::new(backdrop, back, env)?;
        let layers = ground.children();
        for sprite in backdrop.sprites() {
            layers.insert_at_top(&**sprite);
        }

        Ok(Self {
            backdrop,
            content,
            overlays,
            target,
            generation: Gen::default(),
            env: None,
            nodes: Slots::default(),
            window: root_visual,
            roots: Vec::new(),
            res: Resources::default(),
            cells: Cells::default(),
            motion,
            trackers: Slots::default(),
            hits: HitTable::default(),
            events: Rc::new(RefCell::new(Events::default())),
            wake,
            census: Census::default(),
            #[cfg(debug_assertions)]
            front_owned: rustc_hash::FxHashSet::default(),
            _not_send: PhantomData,
        })
    }

    /// Moves one glow's centre, as a fraction of the window.
    ///
    /// `index` is into the [`BackdropSpec::glows`] the scene was built with. A front-side
    /// write, like [`retarget`](Self::retarget): the backdrop carries no patch ops, so an
    /// application driving a glow from a `Cell` calls this from its effect.
    pub fn move_glow(&mut self, index: usize, at: Vector2) {
        self.backdrop.move_glow(index, at);
    }

    /// Returns the overlay band's children, where a ghost outlives the subtree it was
    /// captured from.
    pub(crate) fn overlay_children(&self) -> windows_composition::VisualCollection {
        self.overlays.children()
    }

    /// Returns the hit array, which every consumer resolves a contact through.
    #[must_use]
    pub fn hits(&self) -> &HitTable {
        &self.hits
    }

    /// Returns what is under `p` for `contact`, or `None` if nothing is.
    pub fn hit(&self, p: Point, contact: ContactKind) -> Option<Hit> {
        self.hits.hit(p, contact)
    }

    /// Returns the running tallies of what this scene has realized.
    #[must_use]
    pub const fn census(&self) -> &Census {
        &self.census
    }

    /// Walks the forest and reports how many nodes it reaches against how many the arena
    /// holds.
    ///
    /// The two diverge when a node is in the arena but off the tree, which renders nothing
    /// and reports no error. Recurses over every node, so it is O(nodes).
    #[must_use]
    pub fn audit(&self) -> Audit {
        fn walk(nodes: &Slots<Node, node::Node>, at: NodeId, reached: &mut u32) {
            *reached += 1;
            for child in tree::children(nodes, at) {
                walk(nodes, child, reached);
            }
        }
        let mut reached = 0;
        for root in &self.roots {
            walk(&self.nodes, *root, &mut reached);
        }
        Audit {
            reached,
            held: self.nodes.len() as u32,
        }
    }

    /// Appends everything the trackers and the batches have reported to `out`.
    ///
    /// Reconciles as it drains: a reported position updates the tracker's shadow and the hit
    /// array's scroll offset, and an ignored request is recorded against its id.
    pub fn drain_events(&mut self, out: &mut Vec<SceneEvent>) {
        // `out` is the caller's buffer and may still hold a previous drain's events. Applying
        // a tracker position twice is not idempotent, so only the range appended here is
        // reconciled.
        let appended = out.len();
        // Delays that came due. Swept here rather than in `apply`, because the tick a delay
        // lands on may carry no patch at all. Each expiry drops its own `Tick`, so the frame
        // clock parks when the last one expires. The clock is read once per sweep and only
        // while a delay is pending, so two delays due together report on the same frame.
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
        self.events.borrow_mut().drain(out);
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

    /// Brings the tree up to date with `env`, rebinding whatever a display move invalidated.
    ///
    /// Every operation that can rasterize calls this first, so no raster is built against an
    /// environment the scene has not synced to.
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
            set_dip_space(&self.window, env);
            self.events
                .borrow_mut()
                .push(SceneEvent::ScaleChanged { scale: env.scale() }, &self.wake);
        }
        // The backdrop is outside the cell cache, so no generation reaches it: the same
        // authored light lands on a different display as a different value, and only a
        // re-raster corrects that.
        if last.light_moved(env) {
            self.backdrop.relight(back, env)?;
        }
        self.refresh(back, env)
    }

    /// Rebuilds everything under a lost device.
    ///
    /// Every brush is a pure function of a cache key or a resource id, so recovery bumps the
    /// device generation, drops the cells and refreshes — the path a DPI change and a first
    /// bind both take, with no per-kind recovery code. Shadowed values, tracker positions and
    /// the hit table live in Rust and need no recovery.
    ///
    /// The surfaces *behind* shared resources are the model's to re-emit, which is what
    /// [`SceneEvent::DeviceRebuilt`] reports; their brushes survive and re-point in place.
    ///
    /// The device itself is repaired by whoever owns it: the caller passes the replacement
    /// GPU to [`Backends::adopt`] before calling [`device_lost`](Self::device_lost).
    pub fn device_lost(&mut self, back: &Backends, env: Env) -> Result<()> {
        self.generation.device = self.generation.device.wrapping_add(1);
        self.cells.clear();
        self.env = Some(env);
        self.refresh(back, env)?;
        self.events
            .borrow_mut()
            .push(SceneEvent::DeviceRebuilt, &self.wake);
        Ok(())
    }

    /// Rebinds every sprite whose realized chain reads a generation that has moved.
    ///
    /// The one response to an invalidation, whichever generation moved. A sprite reads only
    /// the generations its own mask and paint declare, so a light change leaves shapes alone
    /// and a grid change leaves solid fills alone.
    fn refresh(&mut self, back: &Backends, env: Env) -> Result<()> {
        let now = self.generation;
        for id in self.sprites_where(|painted| !painted.fresh(now)) {
            self.rebind(SpriteId(id), back, env)?;
        }
        Ok(())
    }

    /// Collects the sprites whose declaration satisfies `predicate`, as a snapshot.
    ///
    /// Allocates, and is reached only from a display event or a presented buffer handing
    /// over, both of which are already rebuilding brushes.
    fn sprites_where(&self, predicate: impl Fn(&Painted) -> bool) -> Vec<NodeId> {
        self.nodes
            .iter()
            .map(|(id, _)| id)
            .filter(|id| {
                self.nodes
                    .get(*id)
                    .and_then(|n| n.painted.as_ref())
                    .is_some_and(&predicate)
            })
            .collect()
    }

    // ── presented regions ─────────────────────────────────────────────────────────

    /// Points a region's slot at a buffer the producer presents into, and rebinds every
    /// sprite painting with it.
    ///
    /// # Safety
    ///
    /// Behavior is undefined if any of the following conditions are violated:
    ///
    /// - `handle` must be a composition surface handle.
    /// - `handle` must stay live for as long as the binding does; the compositor does not
    ///   take ownership of it.
    ///
    /// # Errors
    ///
    /// Fails when the compositor rejects `handle` or a dependent sprite cannot be rebound.
    pub unsafe fn set_region(
        &mut self,
        region: RegionId,
        handle: *mut core::ffi::c_void,
        back: &Backends,
        env: Env,
    ) -> Result<()> {
        self.sync(back, env)?;
        // SAFETY: `handle` is a composition surface handle that stays live for as long as
        // the binding does, which is this function's obligation on its caller.
        let surface = unsafe { back.compositor.create_surface_for_handle(handle)? };
        // The buffer is already at device resolution, so `Stretch::None` samples it one texel
        // per physical pixel — every pixel guarantee a presented region makes rests on that.
        let brush = back.brush(&surface, Stretch::None);
        if let Some(res) = self.res.regions.get_mut(region) {
            res.value = Some(brush);
        }
        self.rebind_region(region, back, env)
    }

    /// Releases a region's buffer and rebinds every sprite painting with it.
    ///
    /// The compositor holds a reference to whatever a visual paints with, so a brush over a
    /// handle the producer is about to close must leave the tree before the handle closes.
    ///
    /// # Errors
    ///
    /// Fails when a dependent sprite cannot be rebound.
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

    /// Creates a tracker on `viewport`, from [`TrackerOp::Create`].
    ///
    /// `owned` attaches an owner. An owner is supplied at construction with no per-callback
    /// subscription, so a tracker that needs one event pays for all six.
    /// [`request`](Self::request) takes only an observed id, so a passive tracker cannot be
    /// asked to move.
    pub(crate) fn tracker(
        &mut self,
        id: Id<Tracker>,
        viewport: NodeId,
        axes: Axes,
        owned: bool,
        back: &Backends,
    ) -> Result<()> {
        let node = self.nodes.get(viewport).ok_or_else(invalid_arg)?;
        let visual = node.visual.clone();
        // The source takes its hit region from this size, and a zero-size source hit-tests
        // nothing while returning success — a scroll surface that ignores every wheel notch
        // rather than anything reported as a failure.
        debug_assert!(
            node.size().x > 0.0 && node.size().y > 0.0,
            "a tracker's viewport must be sized before its source is created"
        );

        let tracker = if owned {
            let queue = Rc::clone(&self.events);
            let wake = self.wake.clone();
            back.compositor
                .create_interaction_tracker_with_owner(move |event| {
                    queue
                        .borrow_mut()
                        .push(tracker::translate(id, event), &wake);
                })?
        } else {
            back.compositor.create_interaction_tracker()?
        };

        let source = VisualInteractionSource::for_visual(&visual)?;
        tracker::configure_source(&source, axes)?;
        tracker.add_source(&source)?;

        let mut state = TrackerState::new(tracker);
        state.source = Some(source);
        state.viewport = Some(viewport);
        self.trackers.place(id, state);
        self.census.trackers_live += 1;
        Ok(())
    }

    /// Asks an observed tracker to move, returning the request's id.
    ///
    /// The tracker may drop the request, which arrives back as a
    /// [`SceneEvent::RequestIgnored`] naming that id.
    ///
    /// # Errors
    ///
    /// Fails when `id` names no live tracker, or when the compositor rejects the request.
    pub fn request(&mut self, id: TrackerId<Observed>, request: TrackerRequest) -> Result<i32> {
        let state = self.trackers.get_mut(id.raw).ok_or_else(invalid_arg)?;
        state.request(request).map(|r| r.0)
    }
}

/// Returns the base visual of a container, which is what a node stores.
///
/// A named function, because the deref chain resolves `clone` to the derived type's own.
pub(crate) fn base_of_group(group: &ContainerVisual) -> Visual {
    (**group).clone()
}

/// Returns the base visual of a sprite. Two derefs, because a sprite visual *is* a container
/// visual — the same relation that lets one arena hold both kinds of node.
pub(crate) fn base_of_sprite(sprite: &windows_composition::SpriteVisual) -> Visual {
    (***sprite).clone()
}

/// Returns the base visual of a shape host, for a capture that takes the base type.
pub(crate) fn base_of_shape(shape: &windows_composition::ShapeVisual) -> Visual {
    (**shape).clone()
}

/// Establishes the tree's DIP space: the factor that makes every DIP below the root mean what
/// it says, and the extent those DIPs are measured against.
///
/// The extent is stated relative to the composition target, not written by this side, so the
/// compositor re-derives it as the window changes — including through a drag-resize, where
/// the system's modal loop owns the thread and this side may not publish at all.
///
/// The adjustment is the reciprocal of the scale: a relative adjustment multiplies the
/// parent's own extent, the target's extent is in *physical pixels*, and everything below
/// this root is in DIPs. Both halves change on a DPI change and on nothing else, so they are
/// set together and only here.
fn set_dip_space(root: &ContainerVisual, env: Env) {
    let scale = env.scale();
    root.set_scale(Vector3 {
        x: scale,
        y: scale,
        z: 1.0,
    });
    let dips = 1.0 / scale;
    root.set_relative_size_adjustment(Vector2 { x: dips, y: dips });
}

/// Returns the `E_INVALIDARG` error, which is how this crate refuses an id or a binding it
/// cannot serve.
pub(crate) fn invalid_arg() -> windows_core::Error {
    windows_core::Error::from(windows_core::HRESULT(-2147024809))
}
