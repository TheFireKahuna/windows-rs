//! Measured text: the table the solve reads, and the step that turns it into coverage.
//!
//! Three properties of the text engine decide the shape of everything here, and each is a
//! cost this module would otherwise pay per frame:
//!
//! * **A run is kept, not rebuilt.** Re-flowing at a new width is a property set on the
//!   layout it already holds, so a resize costs no shaping. A changed string reshapes *in
//!   place*, keeping the harvest buffers and the line vector.
//! * **Measuring never moves a glyph.** A solve probes one node several times per pass with
//!   different constraints, so measure is a metrics read and the glyphs are placed once,
//!   afterwards, at the width layout actually chose.
//! * **`pin` says whether they moved.** A non-wrapping run laid out leading does not break,
//!   so a window resize re-pins every label and re-rasterizes none of them.
//!
//! The table is a thread-local rather than a field of the host, because measure runs
//! *inside* `Model::flush`'s own solve — where the host is already borrowed. It is also why
//! it is not an `Arc<Mutex<..>>`: `Measure` is `Send`, but a closure that captures nothing
//! is `Send` whatever it reaches through, and a run holds a layout object that is not.

use crate::role::{Scope, TypeRole};
use crate::widget::{Flow, Run, Shaped, TextSource};
use std::cell::RefCell;
use windows_numerics::Vector2;
use windows_scene::{GroupId, Mask, MeasureIn, MeasureKey, Model, RunId, SpriteId, taffy};
use windows_text::FontSpec;

/// Where one entry's lines are drawn.
///
/// A coverage tile is **one line's**, so a run that can break needs one sprite per line.
/// Keeping the two cases apart is what preserves "a static label costs one visual": the
/// wrapping case is the only one that pays for a group and a vector, and whether a widget
/// can wrap is decided by its seed rather than by its content.
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
/// Copying it into a `String` would be one allocation per label per mount — on the path a list
/// row realized during a fling takes, and for a string that already exists in the binary.
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
    /// A static string crosses as a **borrow**; the other two hand over a string that is
    /// already the caller's own, so nothing here copies anything twice.
    fn from(source: &TextSource) -> Self {
        match source {
            TextSource::Static(s) => Self::Static(s),
            TextSource::Owned(s) => Self::Owned(s.clone()),
            TextSource::Dynamic(read) => Self::Owned(read()),
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
            // A static run that acquired a dynamic string: the first change is where it
            // becomes owned, and every one after that reuses this buffer.
            Self::Static(_) => *self = Self::Owned(text.to_owned()),
        }
    }
}

pub(crate) struct Entry {
    run: Box<dyn Run>,
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
    /// The width the glyphs currently stand at. `NaN` until the first pin, so the first
    /// publish always emits.
    pinned: f32,
    /// The string or the font moved, so the run is behind its source.
    stale: bool,
    live: bool,
}

/// What a run needs to exist.
///
/// One argument rather than seven, because every one of them is decided at the same call site
/// and a positional list of that length is a place to transpose two.
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
    entries: Vec<Entry>,
    free: Vec<u32>,
}

thread_local! {
    static TEXT: RefCell<Table> = RefCell::new(Table::default());
}

/// Runs `f` against the thread's text table.
pub(crate) fn with<R>(f: impl FnOnce(&mut Table) -> R) -> R {
    TEXT.with(|table| f(&mut table.borrow_mut()))
}

/// The same, for a caller that may be running while the thread's locals are being
/// destroyed — where reaching one is an error and [`with`] would answer by panicking.
pub(crate) fn try_with<R>(f: impl FnOnce(&mut Table) -> R) -> Option<R> {
    TEXT.try_with(|table| f(&mut table.borrow_mut())).ok()
}

/// What the solve asks of the layer that owns the text engine.
///
/// The class is an **input** rather than something read from ambient state, so a
/// measurement is taken under the width the container actually resolved and not under
/// whatever was current when the node was built.
pub(crate) fn measure(input: MeasureIn) -> Vector2 {
    with(|table| {
        let Some(entry) = table.entry_mut(input.key) else {
            return Vector2 { x: 0.0, y: 0.0 };
        };
        entry.sync(input.class);
        entry.run.measure(input.available.0)
    })
}

impl Table {
    fn entry_mut(&mut self, key: MeasureKey) -> Option<&mut Entry> {
        self.entries
            .get_mut(key.0 as usize)
            .filter(|entry| entry.live)
    }

    /// Registers a run and hands back the key layout will name it by.
    ///
    /// A released slot keeps its laid-out run, so this **reshapes** rather than building one:
    /// the same reason a changed string reshapes in place, extended across the unmount that
    /// recycled the slot. That is what makes a list row realized during a fling cost the walk
    /// and nothing else.
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
        if let Some(at) = self.free.pop() {
            let entry = &mut self.entries[at as usize];
            entry.run.reshape(text.as_str(), font, flow);
            entry.text = text;
            entry.ramp = ramp;
            entry.flow = flow;
            entry.scope = scope;
            entry.font = font;
            entry.ink = ink;
            entry.target = target;
            entry.pinned = f32::NAN;
            entry.stale = false;
            entry.live = true;
            return MeasureKey(u64::from(at));
        }
        self.entries.push(Entry {
            run: crate::widget::shaper().shape(text.as_str(), font, flow),
            text,
            ramp,
            flow,
            scope,
            font,
            ink,
            target,
            pinned: f32::NAN,
            // Fresh, for the same reason the recycled path above is: the run was just laid
            // out from this string under this font. Marking it stale would reshape it again
            // at the first measure. A class that resolves a *different* font still reshapes,
            // because `sync` compares the font rather than trusting this flag.
            stale: false,
            live: true,
        });
        MeasureKey((self.entries.len() - 1) as u64)
    }

    /// Re-points a run's text.
    ///
    /// Event rate by construction: changing a line's string is structural — reshape,
    /// re-rasterize, re-point — and text that changes at display rate belongs in a
    /// presentation region.
    pub(crate) fn set_text(&mut self, key: MeasureKey, text: &str) {
        let Some(entry) = self.entry_mut(key) else {
            return;
        };
        if entry.text.as_str() == text {
            return;
        }
        entry.text.set(text);
        entry.stale = true;
    }

    /// The scope a run resolves its type ramp against, for a subtree that changed class.
    pub(crate) fn set_scope(&mut self, key: MeasureKey, scope: Scope) {
        if let Some(entry) = self.entry_mut(key) {
            entry.scope = scope;
        }
    }

    /// Releases a run and the resource slots it held.
    ///
    /// The line sprites go with the subtree the destroy cascades over, so only the coverage
    /// tiles are named here — and they are refcounted on the far side, which is what makes
    /// dropping the model's claim safe while a sprite is still holding one.
    pub(crate) fn release(&mut self, key: MeasureKey, model: &mut Model) {
        let at = key.0 as u32;
        let Some(entry) = self.entries.get_mut(at as usize).filter(|e| e.live) else {
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
        // The run and its buffers stay: this slot is being recycled, not discarded, and
        // `mint` reshapes what it finds here. Only the coverage the far side holds is
        // named above.
        entry.live = false;
        self.free.push(at);
    }

    /// How many runs are live. What a leak test reads.
    #[cfg(test)]
    pub(crate) fn live(&self) -> usize {
        self.entries.iter().filter(|entry| entry.live).count()
    }

    /// Pins every run at the width the solve gave it, and re-publishes the ones that moved.
    ///
    /// Runs between the solve and the hand-over, which is the whole reason
    /// [`Model::solve`] is separable: a run laid out at the width layout chose has to reach
    /// the *same* patch as that layout, or it arrives a frame after the box it was measured
    /// for.
    ///
    /// Answers whether anything was emitted, so a caller can skip the re-solve when nothing
    /// did.
    ///
    /// Every live run, and **not** a dirty list: a resize moves every label's box without
    /// touching a single string, so the thing that decides whether a run moved is the solved
    /// width and nothing above knows it. What the walk costs is one solved-rect read and a
    /// float compare per run, which is why the expensive halves — reshaping and re-emitting —
    /// are both behind `pin`.
    pub(crate) fn publish(&mut self, model: &mut Model) -> bool {
        let mut emitted = false;
        for entry in &mut self.entries {
            if entry.live {
                emitted |= entry.publish(model);
            }
        }
        emitted
    }
}

impl Entry {
    /// Brings the run up to date with the string and the class before it is measured.
    ///
    /// `reshape` and not a fresh run: it keeps the harvest buffers and the line vector, so
    /// a label bound to a changing value allocates nothing after the first change.
    fn sync(&mut self, class: windows_scene::WidthClass) {
        let font = crate::role::typography(self.ramp, self.scope.at_width(class));
        if !self.stale && font == self.font {
            return;
        }
        self.font = font;
        self.stale = false;
        self.pinned = f32::NAN;
        self.run.reshape(self.text.as_str(), font, self.flow);
    }

    /// Fixes this run at the width it was given and re-emits what moved.
    fn publish(&mut self, model: &mut Model) -> bool {
        // The box layout gave *this* run: the laid-out label sprite, or the column the
        // lines sit in. Both are the node whose style measured it.
        let node = match self.target {
            Target::Line { sprite, .. } => sprite.node(),
            Target::Wrapped { group, .. } => group.node(),
        };
        let width = model.solved(node).size.x;
        if width <= 0.0 {
            return false;
        }
        // `pin` is the single authoritative writer of the width. Whichever probe the solve
        // happened to end on would otherwise decide where the glyphs landed.
        let moved = self.run.pin(width);
        if !moved && self.pinned == width {
            return false;
        }
        self.pinned = width;
        if !moved {
            return false;
        }
        match &mut self.target {
            Target::Line { sprite, run } => {
                let shaped = self.run.emit(0, model.glyphs());
                publish_line(model, *sprite, run, shaped);
                true
            }
            Target::Wrapped { group, lines } => {
                let light =
                    crate::role::resolve(crate::role::Role::Text(self.ink), self.scope.for_paint());
                publish_lines(model, &mut *self.run, *group, lines, light)
            }
        }
    }
}

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
    model: &mut Model,
    run: &mut dyn Run,
    group: GroupId,
    lines: &mut Vec<Line>,
    light: windows_color::Radiance,
) -> bool {
    let count = run.lines();
    while lines.len() > count {
        let line = lines.pop().expect("the vector is longer than the run");
        model.destroy(line.sprite.node(), windows_scene::Exit::None);
        model.release(line.run);
    }
    for index in 0..count {
        let shaped = run.emit(index, model.glyphs());
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

/// One line's box: exactly its coverage tile.
///
/// Built here rather than through the [`Over`](crate::layout::Over) vocabulary because the
/// number is the text engine's rather than an author's — this is the lowering resolving a
/// measurement, not a widget expressing a size, and `Len` deliberately cannot say it.
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
