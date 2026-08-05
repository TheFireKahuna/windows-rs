//! Measured text: the table the solve reads, and the step that turns it into coverage.
//!
//! Three properties of the text engine shape everything here:
//!
//! * **A run is kept, not rebuilt.** Re-flowing at a new width is a property set on the
//!   layout it already holds, so a resize costs no shaping. A changed string reshapes *in
//!   place*, keeping the harvest buffers and the line vector.
//! * **Measuring never moves a glyph.** A solve probes one node several times per pass with
//!   different constraints, so measure is a metrics read and the glyphs are placed once,
//!   afterwards, at the width layout chose.
//! * **`pin` says whether they moved.** A non-wrapping run laid out leading does not break,
//!   so a window resize re-pins every label and re-rasterizes none of them.
//!
//! The table is a thread-local rather than a field of the host, because measure runs
//! *inside* `Model::flush`'s own solve, where the host is already borrowed. An
//! `Arc<Mutex<..>>` would not do either: a closure that captures nothing is `Send` whatever
//! it reaches through, and a run holds a layout object that is not.

use crate::role::{Scope, TypeRole};
use crate::widget::{Flow, Shaped, TextSource};
use std::cell::RefCell;
use windows_core::Result;
use windows_numerics::Vector2;
use windows_scene::{
    Avail, GroupId, Ids, Mask, MeasureIn, MeasureKey, Measured, Model, NodeId, RunId, Slots,
    SpriteId, taffy,
};
use windows_text::{FontLadder, FontSpec, SegBuffers, ShapedRun, TextEngine};

/// Where one entry's lines are drawn.
///
/// A coverage tile covers **one line**, so a run that can break needs one sprite per line.
/// Only the wrapping case pays for a group and a vector, so a static label costs one visual.
/// Which case an entry takes is decided by the widget's seed rather than by its content.
enum Target {
    /// One line, always: the node is the sprite.
    Line {
        sprite: SpriteId,
        run: Option<RunId>,
    },
    /// One sprite per line, laid out in a column under the node.
    Wrapped { group: GroupId, lines: Vec<Line> },
}

struct Line {
    sprite: SpriteId,
    run: RunId,
    size: Vector2,
}

/// The string a run is laid out from.
///
/// Most of a screen's text is chrome and is `&'static str`, so it is **kept as a borrow**.
/// Copying it into a `String` would be one allocation per label per mount, on the path a list
/// row realized during a fling takes.
///
/// A bound value's string is owned, because it is a closure's result and is kept nowhere else.
/// That one is written **in place**, so a label following a changing number allocates once.
pub(crate) enum Source {
    Static(&'static str),
    Owned(String),
}

impl From<&TextSource> for Source {
    /// Snapshots a text source for the table.
    ///
    /// A static string crosses as a **borrow**; the other two hand over a string of their
    /// own, so nothing is copied twice.
    fn from(source: &TextSource) -> Self {
        match source {
            TextSource::Static(s) => Self::Static(s),
            TextSource::Owned(s) => Self::Owned(s.clone()),
            // The one allocation a reactive run makes at mount, and it is a buffer it keeps:
            // every later change is written into it in place.
            TextSource::Dynamic(_) => {
                let mut owned = String::new();
                source.append(&mut owned);
                Self::Owned(owned)
            }
        }
    }
}

impl Source {
    fn as_str(&self) -> &str {
        match self {
            Self::Static(s) => s,
            Self::Owned(s) => s,
        }
    }

    /// Re-points at `text`, reusing the buffer where there is one.
    fn set(&mut self, text: &str) {
        match self {
            Self::Owned(owned) => {
                owned.clear();
                owned.push_str(text);
            }
            // A static run given a dynamic string: the first change is where it becomes
            // owned, and every one after that reuses this buffer.
            Self::Static(_) => *self = Self::Owned(text.to_owned()),
        }
    }
}

/// One laid-out run, and everything needed to lay it out again.
pub(crate) struct Entry {
    run: ShapedRun,
    /// The current string, needed again whenever the type ramp moves.
    text: Source,
    ramp: TypeRole,
    flow: Flow,
    scope: Scope,
    /// What the run was last laid out under. A width class change moves the type ramp, so
    /// this is what distinguishes a reshape from a re-pin.
    font: FontSpec,
    /// The foreground the lines are painted in. Held because a wrapping run mints its own
    /// sprites as it breaks, and a sprite minted after the walk has no other way to learn
    /// what colour its widget resolved.
    ink: crate::role::Text,
    target: Target,
    /// The width the glyphs stand at. `NaN` until the first pin, so the first
    /// publish always emits.
    pinned: f32,
    /// The inline extent of the coverage last emitted for a single line. `NaN` until there
    /// is one.
    ///
    /// Held rather than read back from the run, because reading a line's ink needs a harvest
    /// and this is compared on **every** flush. The comparison is what re-pins a box that a
    /// class re-lower rebuilt from the recipe: that path reshapes nothing, so it emits
    /// nothing to hang the correction off.
    ink_w: f32,
    /// The string or the font moved, so the run is behind its source.
    stale: bool,
}

/// What a run needs to exist.
///
/// One argument rather than seven positional ones, all decided at the same call site.
pub(crate) struct Mint {
    pub text: Source,
    pub ramp: TypeRole,
    pub flow: Flow,
    pub scope: Scope,
    pub ink: Option<crate::role::Text>,
    /// The sprite a single line draws into. Ignored where `group` says it wraps.
    pub sprite: SpriteId,
    /// The column a wrapping run mints its lines under.
    pub group: Option<GroupId>,
}

#[derive(Default)]
pub(crate) struct Table {
    keys: Ids<Measured>,
    pub(crate) entries: Slots<Measured, Entry>,
    /// Laid-out runs whose key has been released, kept for their buffers.
    ///
    /// A released entry is **parked here rather than left in a vacated slot**. Reshaping a
    /// parked run allocates nothing, where building a fresh one shapes and allocates on the
    /// path a list row realized during a fling takes. Leaving it in the store would need a
    /// second removal verb for a slot that is vacant and still holds something, which is a
    /// live flag beside a generation.
    spare: Vec<Entry>,
    /// The thread's shaping engine.
    ///
    /// Here rather than in [`Host`](super::Host) because of the measure seam:
    /// `Model::on_measure` takes a `Send` closure that captures nothing, so measure can reach
    /// only this table's own thread-local, and a run cannot be reshaped without the engine
    /// that laid it out.
    engine: Option<TextEngine>,
}

thread_local! {
    static TEXT: RefCell<Table> = RefCell::new(Table::default());
}

/// Installs this thread's shaping engine over `fonts`, once, before anything mounts.
///
/// `fonts` must be the ladder the rasterizing half already holds — `Backends::ladder()`. Two
/// ladders interning independently agree on face `0` and disagree on everything after it,
/// which draws a run in the wrong face rather than reporting an error.
///
/// # Errors
///
/// The DirectWrite factory or the font collection failing to open.
///
/// # Panics
///
/// If an engine is already installed on this thread.
pub fn install(fonts: FontLadder) -> Result<()> {
    let engine = TextEngine::new(fonts)?;
    with(|table| {
        assert!(
            table.engine.is_none(),
            "a text engine is already installed on this thread"
        );
        table.engine = Some(engine);
    });
    Ok(())
}

/// Returns whether this thread has a shaping engine installed.
#[must_use]
pub fn installed() -> bool {
    with(|table| table.engine.is_some())
}

/// Runs `f` against the thread's text table.
pub(crate) fn with<R>(f: impl FnOnce(&mut Table) -> R) -> R {
    TEXT.with(|table| f(&mut table.borrow_mut()))
}

/// Runs `f` against the thread's text table, answering `None` where the thread's locals are
/// being destroyed and the table cannot be reached. [`with`] panics there instead.
pub(crate) fn try_with<R>(f: impl FnOnce(&mut Table) -> R) -> Option<R> {
    TEXT.try_with(|table| f(&mut table.borrow_mut())).ok()
}

/// Measures the run `input` names, under the availability and width class the solve is
/// probing with. What the solve asks of the layer that owns the text engine.
///
/// The class is an **input** rather than ambient state, so the measurement is taken under the
/// width the container resolved rather than the one current when the node was built.
///
/// **The three availability states are answered separately**, and for a run that can break
/// they are three different numbers. A min-content probe asks how narrow the run can be,
/// which is its longest unbreakable span; a max-content probe asks its one-line width.
/// Answering the one-line width to both lets flex shrink a paragraph below its own longest
/// word, which breaks a word in the middle.
///
/// # Panics
///
/// If no shaping engine is installed on this thread.
pub(crate) fn measure(input: MeasureIn) -> Vector2 {
    with(|table| {
        let Table {
            entries, engine, ..
        } = table;
        let engine = engine.as_ref().expect(ENGINE);
        let Some(entry) = entries.get_mut(input.key) else {
            return Vector2 { x: 0.0, y: 0.0 };
        };
        entry.sync(engine, input.class);
        match input.available.0 {
            // Unbounded: the run at its natural width. A single-line run answers this to
            // every probe, since it has no break opportunity and its narrowest width is its
            // widest.
            Avail::MaxContent => entry.run.measure(None),
            Avail::Definite(w) => entry.run.measure(Some(w)),
            // Measured at the longest unbreakable span, so the height is the one that span
            // implies rather than the one-line height a bare `min_width` would leave beside
            // it. A wrapping run at min-content is several lines tall, and a caller given
            // the width without the height gets a box that clips its own text.
            Avail::MinContent => entry.run.measure(Some(entry.run.min_width())),
        }
    })
}

impl Table {
    /// Returns the string a run was laid out from.
    ///
    /// What automation derives an accessible name from: a widget's name is its own text
    /// unless it was given one, and this is where its own text is.
    pub(crate) fn str_of(&self, key: MeasureKey) -> Option<&str> {
        self.entries.get(key).map(|entry| entry.text.as_str())
    }

    /// Registers a run and hands back the key layout will name it by.
    ///
    /// A released slot keeps its laid-out run, so this **reshapes** a parked entry rather
    /// than building one — the same reuse a changed string gets, extended across the unmount
    /// that recycled the slot.
    ///
    /// # Panics
    ///
    /// If no shaping engine is installed on this thread, or if DirectWrite cannot lay out the
    /// run.
    pub(crate) fn mint(&mut self, mint: Mint) -> MeasureKey {
        let Mint {
            text,
            ramp,
            flow,
            scope,
            ink,
            sprite,
            group,
        } = mint;
        let font = crate::role::typography(ramp, scope);
        let target = match group {
            Some(group) => Target::Wrapped {
                group,
                lines: Vec::new(),
            },
            None => Target::Line { sprite, run: None },
        };
        let ink = ink.unwrap_or(crate::role::Text::Primary);
        let engine = self.engine.as_ref().expect(ENGINE);
        if let Some(mut entry) = self.spare.pop() {
            let _ = engine.reshape(&mut entry.run, text.as_str(), &font, flow);
            entry.text = text;
            entry.ramp = ramp;
            entry.flow = flow;
            entry.scope = scope;
            entry.font = font;
            entry.ink = ink;
            entry.target = target;
            entry.pinned = f32::NAN;
            entry.stale = false;
            return self.entries.insert(&mut self.keys, entry);
        }
        let entry = Entry {
            // Panics rather than measuring zero: a plausible size with no glyphs behind it
            // lays the screen out around a number nothing produced, and the failure then
            // surfaces as geometry rather than as the layout call that could not run.
            run: engine
                .shape(text.as_str(), &font, flow)
                .expect("DirectWrite could not lay out a run"),
            text,
            ramp,
            flow,
            scope,
            font,
            ink,
            target,
            pinned: f32::NAN,
            ink_w: f32::NAN,
            // Fresh, as on the recycled path above: the run is laid out from this string
            // under this font, and marking it stale would reshape it again at the first
            // measure. A class resolving a *different* font still reshapes, because `sync`
            // compares the font rather than trusting this flag.
            stale: false,
        };
        self.entries.insert(&mut self.keys, entry)
    }

    /// Re-points a run's text, answering the node whose measure has to be re-asked, or `None`
    /// where the string did not move.
    ///
    /// Changing a line's string is structural — reshape, re-rasterize, re-point — so this is
    /// an event-rate call; text that changes at display rate belongs in a presentation
    /// region.
    ///
    /// The node and not a `bool`, because marking the entry stale is only half of it.
    /// `stale` is read by [`Entry::sync`], `sync` runs from the measure function, and the
    /// measure function runs only for a node taffy considers dirty. Re-pointing a string does
    /// not make it dirty, since the measure context is a key that never changes, so the
    /// caller has to mark it — and this is the node to mark.
    pub(crate) fn set_text(&mut self, key: MeasureKey, text: &str) -> Option<NodeId> {
        let entry = self.entries.get_mut(key)?;
        if entry.text.as_str() == text {
            return None;
        }
        entry.text.set(text);
        entry.stale = true;
        Some(entry.node())
    }

    /// Releases a run and the resource slots it held.
    ///
    /// The line sprites go with the subtree the destroy cascades over, so only the coverage
    /// tiles are named here — and they are refcounted on the far side, which is what makes
    /// dropping the model's claim safe while a sprite is still holding one.
    pub(crate) fn release(&mut self, key: MeasureKey, model: &mut Model) {
        let Some(mut entry) = self.entries.remove(&mut self.keys, key) else {
            return;
        };
        match &mut entry.target {
            Target::Line { run, .. } => {
                if let Some(id) = run.take() {
                    model.release(id);
                }
            }
            Target::Wrapped { lines, .. } => {
                for line in lines.drain(..) {
                    model.release(line.run);
                }
            }
        }
        // The run and its buffers stay, parked for the next `mint` to reshape. Only the
        // coverage the far side holds is named above.
        self.spare.push(entry);
    }

    /// Pins every run at the width the solve gave it, re-publishes the ones that moved, and
    /// answers whether anything was emitted so a caller can skip the re-solve.
    ///
    /// Runs between the solve and the hand-over, which is what [`Model::solve`] being
    /// separable buys: a run laid out at the width layout chose reaches the *same* patch as
    /// that layout rather than arriving a frame after the box it was measured for.
    ///
    /// Every live run, and **not** a dirty list: a resize moves every label's box without
    /// touching a string, and the solved width is what decides whether a run moved. The walk
    /// costs one solved-rect read and a float compare per run; reshaping and re-emitting are
    /// both behind `pin`.
    ///
    /// # Panics
    ///
    /// If no shaping engine is installed on this thread.
    pub(crate) fn publish(&mut self, model: &mut Model) -> bool {
        let Table {
            entries, engine, ..
        } = self;
        let engine = engine.as_ref().expect(ENGINE);
        let mut emitted = false;
        for (_, entry) in entries.iter_mut() {
            emitted |= entry.publish(engine, model);
        }
        emitted
    }

    /// Re-sends every run's coverage, whether or not its box moved.
    ///
    /// The response to a pixel grid that moved and to a device that was rebuilt. A coverage
    /// tile is rasterized at **device** resolution and neither event changes a DIP, so
    /// [`publish`](Self::publish), which is gated on the width having moved, answers "nothing
    /// to do" for a display hop that leaves every glyph rasterized for the old grid.
    ///
    /// Shaping is not redone: it is resolution-independent, and the run is already pinned at
    /// the width the last solve gave it. What is stale is the raster, and re-pointing each
    /// run through `Model::set_run` replaces it.
    ///
    /// # Panics
    ///
    /// If no shaping engine is installed on this thread.
    pub(crate) fn reemit(&mut self, model: &mut Model) {
        let Table {
            entries, engine, ..
        } = self;
        let engine = engine.as_ref().expect(ENGINE);
        for (_, entry) in entries.iter_mut() {
            entry.emit(engine, model);
        }
    }
}

impl Entry {
    /// Brings the run up to date with the string and the class before it is measured.
    ///
    /// `reshape` and not a fresh run: it keeps the harvest buffers and the line vector, so
    /// a label bound to a changing value allocates nothing after the first change.
    fn sync(&mut self, engine: &TextEngine, class: windows_scene::WidthClass) {
        let font = crate::role::typography(self.ramp, self.scope.at_width(class));
        if !self.stale && font == self.font {
            return;
        }
        self.font = font;
        self.stale = false;
        self.pinned = f32::NAN;
        let _ = engine.reshape(&mut self.run, self.text.as_str(), &font, self.flow);
    }

    /// Returns the node whose style measured this run: the laid-out label sprite, or the
    /// column the lines sit in.
    fn node(&self) -> NodeId {
        match self.target {
            Target::Line { sprite, .. } => sprite.node(),
            Target::Wrapped { group, .. } => group.node(),
        }
    }

    /// Fixes this run at the width it was given and re-emits what moved.
    fn publish(&mut self, engine: &TextEngine, model: &mut Model) -> bool {
        let node = self.node();
        let width = model.solved(node).size.x;
        if width <= 0.0 {
            return false;
        }
        // `pin` is the single authoritative writer of the width. Whichever probe the solve
        // happened to end on would otherwise decide where the glyphs landed.
        let moved = self.run.pin(width);
        let mut emitted = false;
        if moved || self.pinned != width {
            self.pinned = width;
            if moved {
                emitted = self.emit(engine, model);
            }
        }
        // After the emit, so the extent compared against is this pass's own.
        emitted | self.fit_line_box(model, node)
    }

    /// Makes a single line's box exactly its coverage, and answers whether that moved a box.
    ///
    /// Only [`Target::Line`], because that is the case where **the node is the sprite**: a
    /// wrapping run owns line sprites of its own and sizes each to its own tile
    /// ([`line_style`]), so its node is free to be whatever the container makes it.
    ///
    /// A single line has no such sprite behind it, so a container that stretches its children
    /// stretches the tile, and the tile's brush fills — the glyphs smear across the whole
    /// container. A measurement leaves the cross size `auto`, which is the case stretch
    /// applies to; a definite size is what it yields to.
    ///
    /// **This terminates.** The width written is the ink of a run that cannot wrap, so it
    /// does not depend on the box it is written into: the next solve hands back the width
    /// just written, the comparison holds, and nothing is written again.
    /// [`Host::flush`](super::Host::flush) re-solves once after a publish and needs that of
    /// every publish.
    fn fit_line_box(&mut self, model: &mut Model, node: NodeId) -> bool {
        let Target::Line { .. } = self.target else {
            return false;
        };
        let ink = self.ink_w;
        // Ordered so `NaN` — no coverage emitted yet — takes the same exit a zero does.
        if !(ink > 0.0) {
            return false;
        }
        if (ink - model.solved(node).size.x).abs() < 0.01 {
            return false;
        }
        let class = model.solved(node).class;
        let Some(style) = super::style::pin_width(node, class, ink) else {
            return false;
        };
        model.style(node, &style);
        true
    }

    /// Sends this run's coverage, whatever the layout did. The half of
    /// [`publish`](Self::publish) behind its gate, so a re-emit takes the same path a move
    /// does rather than a second one that has to be kept in step with it.
    fn emit(&mut self, engine: &TextEngine, model: &mut Model) -> bool {
        match &mut self.target {
            Target::Line { sprite, run } => {
                let shaped = emit(engine, &mut self.run, 0, model.glyphs());
                self.ink_w = shaped.ink.size.x;
                publish_line(model, *sprite, run, shaped);
                true
            }
            Target::Wrapped { group, lines } => {
                let light =
                    crate::role::resolve(crate::role::Role::Text(self.ink), self.scope.for_paint());
                publish_lines(engine, model, &mut self.run, *group, lines, light)
            }
        }
    }
}

/// Returns one line's segments and the tile they occupy.
///
/// Harvests first: `pin` marks the layout stale, and reading a line before the walk would
/// answer from the previous width.
fn emit(engine: &TextEngine, run: &mut ShapedRun, line: usize, out: &mut SegBuffers) -> Shaped {
    let _ = engine.harvest(run);
    Shaped {
        segs: run.segments(line, out),
        ink: run.line_ink(line),
    }
}

/// The message every missing-engine panic carries.
const ENGINE: &str = "a text engine must be installed before anything mounts: call                       windows_ui::build::text::install once at start-up";

/// Points one sprite at a line's coverage.
///
/// The id is minted **once** and re-pointed for the life of the node: a resource slot per
/// text change would churn the far side's table for a string that occupies the same sprite
/// throughout.
fn publish_line(model: &mut Model, sprite: SpriteId, run: &mut Option<RunId>, shaped: Shaped) {
    if let Some(id) = *run {
        model.set_run(id, shaped.segs, shaped.ink);
        return;
    }
    let id = model.run(shaped.segs, shaped.ink);
    *run = Some(id);
    model.mask(sprite, Mask::Run(id));
}

/// Reconciles a wrapping run's line sprites, and points each at its own coverage.
///
/// Sprites are minted and destroyed only as the line **count** changes, which for a caption
/// is a resize crossing a break and not a keystroke.
fn publish_lines(
    engine: &TextEngine,
    model: &mut Model,
    run: &mut ShapedRun,
    group: GroupId,
    lines: &mut Vec<Line>,
    light: windows_color::Radiance,
) -> bool {
    // Harvesting fills the line table and pinning makes it stale, so the walk happens once
    // here rather than at each reader below.
    let _ = engine.harvest(run);
    let count = run.lines().len();
    while lines.len() > count {
        let line = lines.pop().expect("the vector is longer than the run");
        model.destroy(line.sprite.node(), windows_scene::Exit::None);
        model.release(line.run);
    }
    for index in 0..count {
        let shaped = emit(engine, run, index, model.glyphs());
        let size = shaped.ink.size;
        if let Some(line) = lines.get_mut(index) {
            model.set_run(line.run, shaped.segs, shaped.ink);
            if line.size != size {
                line.size = size;
                model.style(line.sprite.node(), &line_style(size));
            }
        } else {
            let after = lines.last().map(|line| line.sprite.node());
            let sprite = model.sprite(group, after);
            let id = model.run(shaped.segs, shaped.ink);
            model.mask(sprite, Mask::Run(id));
            model.paint(sprite, windows_scene::Paint::Solid(light));
            model.style(sprite.node(), &line_style(size));
            lines.push(Line {
                sprite,
                run: id,
                size,
            });
        }
    }
    true
}

/// Returns one line's box: exactly its coverage tile.
///
/// Built here rather than through the [`Over`](crate::layout::Over) vocabulary because the
/// number is the text engine's rather than an author's. This is the lowering resolving a
/// measurement rather than a widget expressing a size, and `Len` cannot say it.
fn line_style(size: Vector2) -> taffy::Style {
    taffy::Style {
        size: taffy::Size {
            width: taffy::Dimension::length(size.x),
            height: taffy::Dimension::length(size.y),
        },
        flex_shrink: 0.0,
        ..taffy::Style::DEFAULT
    }
}
