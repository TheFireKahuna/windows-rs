//! Measured text: the table the solve reads, and the step that turns it into coverage.
//!
//! Three properties of the text engine shape everything here:
//!
//! * **A run is kept, not rebuilt.** Re-flowing at a new width is a property set on the
//!   layout it already holds, so a resize costs no shaping. A changed string reshapes *in
//!   place*, keeping the harvest buffers and the line vector.
//! * **Measuring never moves a glyph.** The solve asks one node for its widths and then for a
//!   height, so a measure is a metrics read and the glyphs are placed once, afterwards, at
//!   the width layout chose.
//! * **`pin` says whether they moved.** A non-wrapping run laid out leading does not break,
//!   so a window resize re-pins every label and re-rasterizes none of them.

use super::host::Host;
use super::ui::{Element, Ui};
use crate::layout::{Preset, WidthClass};
use crate::role::{Role, Scope, Text, resolve, typography};
use crate::text_input::Cluster;
use crate::widget::{Flow, TextSource, TextStyle};
use windows_core::Result;
use windows_numerics::Vector2;
use windows_scene::{
    ControlId, Exit, GlyphSeg, GroupId, Ink, Mask, NodeId, Paint, Prop, RunId, Span, SpriteId,
    Value,
};
use windows_text::{FontLadder, FontSpec, Rect, SegBuffers, ShapedRun, TextEngine};

mod annotated;
use annotated::Annotated;

/// The message every missing-engine panic carries.
const ENGINE: &str =
    "a text engine must be installed before anything mounts: install it on the host at start-up";
/// The message every failed-layout panic carries.
///
/// Panics rather than measuring zero: a plausible size with no glyphs behind it lays the
/// screen out around a number nothing produced, and the failure then surfaces as geometry
/// rather than as the layout call that could not run.
const LAYOUT: &str = "DirectWrite could not lay out a run";
/// What a masked cluster draws. BMP, so one mask is one UTF-16 unit.
const MASK: char = '\u{25cf}';

/// Names one run in the text table.
///
/// The table's own generational index rather than a scene id family: nothing on the wire
/// names a text measurement, so a key minted here never has to agree with the far side.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub(crate) struct MeasureKey {
    at: u32,
    age: u32,
}

impl MeasureKey {
    /// The `text` column's empty value. A live slot always carries a non-zero age.
    pub(crate) const NONE: Self = Self { at: 0, age: 0 };
}

/// Where one entry's lines are drawn.
///
/// A coverage tile covers **one line**, so a run that can break needs one sprite per line.
/// Wrapping and vertical text use a group; a horizontal static label costs one visual. Which
/// case an entry takes is decided by the widget's text recipe rather than by its content.
pub(crate) enum Target {
    /// One line, always: the node is the sprite.
    Line { sprite: SpriteId, run: RunId },
    /// One derived sprite per line, positioned within the measured owner.
    Wrapped {
        group: GroupId,
        lines: Vec<(SpriteId, RunId)>,
    },
    Annotated(Box<Annotated>),
}

/// How the string a run draws differs from the string automation announces.
///
/// Casing is a typographic treatment and not a rename, and a mask is a display form and not
/// the value, so both are one operation: fold the author's string into what the shaper draws
/// and keep the author's beside it. Without that a reader says a badge one letter at a time,
/// and a password reader says its plaintext.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub(crate) enum Fold {
    #[default]
    None,
    Caps,
    Mask,
}

impl Fold {
    /// Maps logically ordered folded clusters to their enclosing source UTF-16 spans.
    fn remap(self, source: &str, clusters: &mut [Cluster]) {
        let mut chars = source
            .chars()
            .map(|ch| {
                let units = ch.len_utf16() as u32;
                let width = match self {
                    Self::Caps => ch.to_uppercase().map(|c| c.len_utf16() as u32).sum(),
                    Self::Mask => 1,
                    Self::None => units,
                };
                (width, units)
            })
            .peekable();
        let (mut drawn, mut source) = (0, 0);
        let mut offset = |position: u32, trailing: bool| {
            while let Some(&(width, units)) = chars.peek() {
                if position < drawn + width {
                    return source
                        + if trailing && position > drawn {
                            units
                        } else {
                            0
                        };
                }
                drawn += width;
                source += units;
                chars.next();
            }
            source
        };
        for cluster in clusters {
            cluster.start = offset(cluster.start, false);
            cluster.end = offset(cluster.end, true);
        }
    }

    /// Returns whether `held` is already `text` under this fold.
    ///
    /// Compares against the folded stream rather than folding into a buffer first, so the
    /// common answer — a reactive run whose value moved and whose text did not — costs a
    /// comparison and no allocation.
    fn holds(self, held: &str, text: &str) -> bool {
        match self {
            Self::None => held == text,
            Self::Caps => held.chars().eq(text.chars().flat_map(char::to_uppercase)),
            Self::Mask => held.chars().eq(text.chars().map(|_| MASK)),
        }
    }

    fn write(self, out: &mut String, text: &str) {
        match self {
            Self::None => out.push_str(text),
            Self::Caps => out.extend(text.chars().flat_map(char::to_uppercase)),
            Self::Mask => out.extend(text.chars().map(|_| MASK)),
        }
    }
}

/// The string a run is laid out from.
///
/// Most of a screen's text is chrome and is `&'static str`, so it is **kept as a borrow**.
/// Copying it into a `String` would be one allocation per label per mount, on the path a list
/// row realized during a fling takes.
///
/// A bound value's string is owned, because it is a closure's result and is kept nowhere
/// else. That one is written **in place**, so a label following a changing number allocates
/// once.
pub(crate) enum Source {
    Static(&'static str),
    Owned(String),
}

impl Source {
    fn as_str(&self) -> &str {
        match self {
            Self::Static(s) => s,
            Self::Owned(s) => s,
        }
    }

    /// Re-points at `text` folded for `fold`, reusing the buffer where there is one, and
    /// answers whether the drawn string moved.
    fn set(&mut self, text: &str, fold: Fold) -> bool {
        if fold.holds(self.as_str(), text) {
            return false;
        }
        // A static run given a dynamic string: the first change is where it becomes owned,
        // and every one after that reuses this buffer.
        let mut buffer = match core::mem::replace(self, Self::Static("")) {
            Self::Owned(mut buffer) => {
                buffer.clear();
                buffer
            }
            Self::Static(_) => String::new(),
        };
        fold.write(&mut buffer, text);
        *self = Self::Owned(buffer);
        true
    }
}

/// One laid-out run, and everything needed to lay it out again.
pub(crate) struct Entry {
    /// The layout, its harvest buffers and its line vector.
    run: ShapedRun,
    uia: std::cell::OnceCell<std::sync::Arc<crate::text_input::Geometry>>,
    /// The drawn string, needed again whenever the type ramp moves.
    text: Source,
    /// What the author wrote, held only where the fold changed it. `None` on every run whose
    /// two forms are the same string, which is all of them but the capitalised and the masked
    /// ones.
    author: Option<Box<str>>,
    /// The rung, flow and ink the run resolves through. Held rather than the resolved values
    /// because a wrapping run mints its own sprites as it breaks, and a sprite minted after
    /// the walk has no other way to learn what colour its widget resolved.
    style: TextStyle,
    scope: Scope,
    /// What the run is laid out under now. A width class change moves the type ramp, so this
    /// is what distinguishes a reshape from a re-pin.
    font: FontSpec,
    target: Target,
    node: NodeId,
    /// The control whose chrome governs this run's colour and whose accessible name it
    /// supplies, or `ControlId::NONE` for a run painted in its own stated ink.
    owner: ControlId,
    class: WidthClass,
    fold: Fold,
    /// The string, the ramp or the scope moved, so the run is behind its source.
    stale: bool,
    /// The memo the solve reads: (minimum, natural) under `class`. `NaN` where the answer has
    /// to be taken again.
    pair: [f32; 2],
    /// The width the block height was answered at, and the height answered there.
    at_w: f32,
    h: f32,
    /// The width the glyphs stand at. `NaN` until the first pin, so the first publish always
    /// emits.
    pinned: f32,
}

impl Entry {
    /// Brings the run up to date with the string and the class before it is measured.
    ///
    /// `reshape` and not a fresh run: it keeps the harvest buffers and the line vector, so a
    /// label bound to a changing value allocates nothing after the first change.
    fn sync(&mut self, engine: &TextEngine, class: WidthClass) {
        let font = typography(self.style.typography, self.scope.at_width(class));
        if !self.stale && self.class == class && self.font == font {
            return;
        }
        engine
            .reshape(&mut self.run, self.text.as_str(), &font, self.style.flow)
            .expect(LAYOUT);
        if let Target::Annotated(data) = &mut self.target {
            data.apply(engine, &mut self.run, self.text.as_str());
        }
        if self.style.line_height > 0.0 {
            self.run.set_line_height(font.size * self.style.line_height).expect(LAYOUT);
        }
        self.uia.take();
        self.font = font;
        self.class = class;
        self.stale = false;
        self.pair = [f32::NAN; 2];
        self.at_w = f32::NAN;
        self.pinned = f32::NAN;
    }

    /// The coverage slot a single line re-points, or `None` where the run can break.
    const fn line(&self) -> Option<RunId> {
        match self.target {
            Target::Line { run, .. } => Some(run),
            Target::Wrapped { .. } | Target::Annotated(_) => None,
        }
    }
}

/// What a run needs to exist. One argument rather than seven positional ones, all decided at
/// the same call site.
pub(crate) struct Mint {
    pub node: NodeId,
    pub text: Source,
    pub style: TextStyle,
    pub scope: Scope,
    /// Whether the run is capitalised or masked. The widget's choice, not the rung's.
    pub fold: Fold,
    pub owner: ControlId,
    /// The sprite a single line draws into, or the column a wrapping run mints its lines
    /// under.
    pub target: Target,
}

/// Moves initial text into the retained table and defers reactive reads until creation ends.
pub(super) fn install(
    host: &mut Host,
    node: NodeId,
    mut mint: Mint,
    source: TextSource,
) -> MeasureKey {
    let read = match source {
        TextSource::Static(text) => {
            mint.text = Source::Static(text);
            None
        }
        TextSource::Owned(text) => {
            mint.text = Source::Owned(text);
            None
        }
        TextSource::Dynamic(read) => Some(read),
    };
    // A single line's coverage slot is minted with its sprite and re-pointed for the life of
    // the node, so the mask is written here and the emit has no first-time arm.
    let line = match mint.target {
        Target::Line { sprite, run } => Some((sprite, run)),
        Target::Wrapped { .. } | Target::Annotated(_) => None,
    };
    let key = host.text.mint(mint);
    host.tree.c.text[node.index()] = key;
    if let Some((sprite, run)) = line {
        host.mask(sprite, Mask::Run(run));
    }
    if let Some(read) = read {
        // The writer belongs to the signal scope creation installed, so it retires with the
        // subtree that declared it and needs no slot of its own.
        let mut scratch = String::new();
        host.binding(move || {
            scratch.clear();
            read(&mut scratch);
            Host::with(|host| {
                if let Some(node) = host.text.set_text(key, &scratch) {
                    host.tree.mark(node);
                    host.uia_stale.set(true);
                }
            });
        });
    }
    key
}

impl Ui<'_> {
    /// A run belonging to the enclosing control, so that control's chrome governs its colour
    /// and automation derives the control's name from it where it was given none.
    pub fn text(&mut self, style: TextStyle, text: impl Into<TextSource>) -> Element<'_> {
        let owner = self.control;
        self.text_owned(Some(owner), style, text)
    }

    /// A run painted in its own stated ink where `owner` is `None`, and in `owner`'s chrome
    /// ink where one is given. The first run supplies a semantic control's default name.
    ///
    /// Container text remains independently accessible; only semantic controls absorb runs
    /// into their default name.
    ///
    /// A run that can break mints one sprite per line under a column of its own; one that
    /// cannot **is** the sprite, which is the whole difference between the two targets.
    pub fn text_owned(
        &mut self,
        owner: Option<ControlId>,
        style: TextStyle,
        text: impl Into<TextSource>,
    ) -> Element<'_> {
        self.text_inner(owner, style, text.into(), None)
    }

    /// Builds one accessible text run with source-dependent ink and weight ranges.
    /// The annotator writes into a reused buffer when text or typography changes.
    /// Text must not request case folding; annotations address the original UTF-8 bytes.
    pub fn annotated_text(
        &mut self,
        style: TextStyle,
        text: impl Into<TextSource>,
        annotate: fn(&str, &mut Vec<crate::widget::TextAnnotation>),
    ) -> Element<'_> {
        assert!(!style.caps, "annotated text cannot fold source bytes");
        self.text_inner(None, style, text.into(), Some(annotate))
    }

    fn text_inner(
        &mut self,
        owner: Option<ControlId>,
        style: TextStyle,
        text: TextSource,
        annotate: Option<fn(&str, &mut Vec<crate::widget::TextAnnotation>)>,
    ) -> Element<'_> {
        let breaks = annotate.is_some() || style.vertical || !matches!(style.flow, Flow::Line);
        let node = match breaks {
            true => self.node(Preset::Text).node_id(),
            false => self.sprite(Preset::Text).node_id(),
        };
        let target = if let Some(annotate) = annotate {
            Target::Annotated(Box::new(Annotated::new(GroupId(node), annotate)))
        } else {
            match breaks {
                true => Target::Wrapped {
                    group: GroupId(node),
                    lines: Vec::new(),
                },
                false => Target::Line {
                    sprite: SpriteId(node),
                    run: self.host.run(Span::default(), Ink::default()),
                },
            }
        };
        let owner = owner.unwrap_or(ControlId::NONE);
        let mint = Mint {
            node,
            text: Source::Static(""),
            style,
            scope: self.scope(),
            fold: match style.caps {
                true => Fold::Caps,
                false => Fold::None,
            },
            owner,
            target,
        };
        let key = install(self.host, node, mint, text.into());
        if let Some(row) = self.host.control_mut(self.control)
            .filter(|row| !matches!(row.uia, crate::widget::UiaRole::None | crate::widget::UiaRole::Group))
        {
            // The first run a semantic control declares supplies its default name.
            row.text.get_or_insert(key);
        } else {
            // Standalone and container-owned runs publish their own text element.
            // It declares no hit entry, because nothing routes to it — only the control column
            // is written, which is what the automation walk reads.
            let scope = self.scope();
            let id = self
                .host
                .mint_control(super::control::ControlRow::blank(node, scope));
            self.host.tree.c.control[node.index()] = id;
            if let Some(row) = self.host.control_mut(id) {
                row.uia = crate::widget::UiaRole::Text;
                row.text = Some(key);
            }
        }
        self.element(node)
    }
}

/// The retained runs, keyed by [`MeasureKey`].
///
/// Dense and walkable by index: every publication pass visits every live run, and a store
/// reachable only by key would make each pass a lookup per entry.
#[derive(Default)]
struct Store {
    rows: Vec<(u32, Option<Entry>)>,
    free: Vec<u32>,
}

impl Store {
    fn insert(&mut self, entry: Entry) -> MeasureKey {
        let at = self.free.pop().unwrap_or_else(|| {
            self.rows.push((0, None));
            self.rows.len() as u32 - 1
        });
        let row = &mut self.rows[at as usize];
        row.0 += 1;
        row.1 = Some(entry);
        MeasureKey { at, age: row.0 }
    }

    fn get(&self, key: MeasureKey) -> Option<&Entry> {
        match self.rows.get(key.at as usize) {
            Some((age, entry)) if *age == key.age => entry.as_ref(),
            _ => None,
        }
    }

    fn get_mut(&mut self, key: MeasureKey) -> Option<&mut Entry> {
        match self.rows.get_mut(key.at as usize) {
            Some((age, entry)) if *age == key.age => entry.as_mut(),
            _ => None,
        }
    }

    fn remove(&mut self, key: MeasureKey) -> Option<Entry> {
        let (age, entry) = self.rows.get_mut(key.at as usize)?;
        if *age != key.age {
            return None;
        }
        // The age moves on release, so a key held past the unmount reads stale from here on.
        *age += 1;
        self.free.push(key.at);
        entry.take()
    }

    fn at(&self, at: usize) -> Option<&Entry> {
        self.rows.get(at)?.1.as_ref()
    }

    fn at_mut(&mut self, at: usize) -> Option<&mut Entry> {
        self.rows.get_mut(at)?.1.as_mut()
    }
}

#[derive(Default)]
pub(crate) struct Table {
    entries: Store,
    /// Layouts whose key has been released, kept for their harvest buffers and line vector.
    ///
    /// Reshaping a parked layout allocates nothing, where building a fresh one shapes and
    /// allocates on the path a list row realized during a fling takes.
    spare: Vec<ShapedRun>,
    /// Owned by this host and borrowed during measurement.
    engine: Option<TextEngine>,
    /// Where a line's segments are harvested before they are copied into the patch.
    segs: SegBuffers,
    /// Runs minted, or whose string or fold moved, since the text pass last read them: a
    /// run can reshape to the box it already had, so its node need not appear in the change
    /// set.
    restated: Vec<usize>,
}

impl Table {
    pub(crate) fn install(&mut self, fonts: FontLadder) -> Result<()> {
        self.engine = Some(TextEngine::new(fonts)?);
        Ok(())
    }

    /// Answers this run's (minimum, natural) inline widths under `class`.
    ///
    /// **The two are different numbers** for a run that can break: the minimum is its longest
    /// unbreakable span and the natural is its one-line width. Answering the one-line width
    /// to both lets a row shrink a paragraph below its own longest word, which breaks a word
    /// in the middle.
    ///
    /// The class is an **input** rather than ambient state, so the measurement is taken under
    /// the width the container resolved rather than the one current when the node was built.
    ///
    /// # Panics
    ///
    /// If no shaping engine is installed on this thread.
    pub(crate) fn pair(&mut self, key: MeasureKey, class: WidthClass) -> [f32; 2] {
        let engine = self.engine.as_ref().expect(ENGINE);
        let Some(entry) = self.entries.get_mut(key) else {
            return [0.0; 2];
        };
        entry.sync(engine, class);
        if entry.pair[1].is_nan() {
            let natural = entry.run.measure(None);
            // A vertical run is laid out horizontally and rotated a quarter turn, so the
            // extent the container receives has its axes swapped.
            entry.pair = if entry.style.vertical {
                [natural.y, natural.y]
            } else {
                [entry.run.min_width(), natural.x]
            };
        }
        entry.pair
    }

    /// Answers this run's height at inline width `w`.
    ///
    /// # Panics
    ///
    /// If no shaping engine is installed on this thread.
    pub(crate) fn height_at(&mut self, key: MeasureKey, class: WidthClass, w: f32) -> f32 {
        let engine = self.engine.as_ref().expect(ENGINE);
        let Some(entry) = self.entries.get_mut(key) else {
            return 0.0;
        };
        entry.sync(engine, class);
        if entry.at_w != w {
            entry.h = if entry.style.vertical {
                entry.run.measure(None).x
            } else {
                entry.run.measure(Some(w)).y
            };
            entry.at_w = w;
        }
        entry.h
    }

    /// Registers a run and hands back the key layout will name it by.
    ///
    /// A released slot's layout is parked, so this **reshapes** one rather than building it —
    /// the same reuse a changed string gets, extended across the unmount that recycled it.
    ///
    /// # Panics
    ///
    /// If no shaping engine is installed on this thread, or if DirectWrite cannot lay out the
    /// run.
    pub(crate) fn mint(&mut self, mint: Mint) -> MeasureKey {
        let engine = self.engine.as_ref().expect(ENGINE);
        let font = typography(mint.style.typography, mint.scope);
        let author = (mint.fold != Fold::None).then(|| Box::from(mint.text.as_str()));
        let mut text = mint.text;
        if let Some(author) = &author {
            text.set(author, mint.fold);
        }
        let mut run = match self.spare.pop() {
            Some(mut run) => {
                engine
                    .reshape(&mut run, text.as_str(), &font, mint.style.flow)
                    .expect(LAYOUT);
                run
            }
            None => engine
                .shape(text.as_str(), &font, mint.style.flow)
                .expect(LAYOUT),
        };
        if mint.style.line_height > 0.0 {
            run.set_line_height(font.size * mint.style.line_height).expect(LAYOUT);
        }
        let stale = matches!(&mint.target, Target::Annotated(_));
        let key = self.entries.insert(Entry {
            uia: std::cell::OnceCell::new(),
            run,
            text,
            author,
            style: mint.style,
            scope: mint.scope,
            font,
            target: mint.target,
            node: mint.node,
            owner: mint.owner,
            class: mint.scope.width,
            fold: mint.fold,
            // Fresh, not stale: the run is laid out from this string under this font. A class
            // resolving a different font still reshapes, because `sync` compares the font
            // rather than trusting this flag.
            stale,
            pair: [f32::NAN; 2],
            at_w: f32::NAN,
            h: f32::NAN,
            pinned: f32::NAN,
        });
        self.restated.push(key.at as usize);
        key
    }

    /// Re-points a run's text, answering the node whose measure has to be re-asked, or `None`
    /// where the string did not move.
    ///
    /// The node and not a `bool`: re-pointing a string does not make the node dirty, since
    /// its measure key never changes, so the caller has to mark it — and this is the node to
    /// mark.
    ///
    /// Changing a line's string is structural — reshape, re-rasterize, re-point — so this is
    /// an event-rate call; text that changes at display rate belongs in a presentation
    /// region.
    pub(crate) fn set_text(&mut self, key: MeasureKey, text: &str) -> Option<NodeId> {
        let entry = self.entries.get_mut(key)?;
        if !entry.text.set(text, entry.fold) {
            return None;
        }
        // Written on the same terms as the shaped form: a folded run that follows a value
        // reshapes and re-announces together.
        if let Some(author) = &mut entry.author {
            *author = Box::from(text);
        }
        entry.stale = true;
        let node = entry.node;
        self.restated.push(key.at as usize);
        Some(node)
    }

    /// Returns the string a run was laid out from.
    ///
    /// What automation derives an accessible name from where a widget was given none.
    pub(crate) fn str_of(&self, key: MeasureKey) -> Option<&str> {
        let entry = self.entries.get(key)?;
        Some(
            entry
                .author
                .as_deref()
                .unwrap_or_else(|| entry.text.as_str()),
        )
    }

    /// Publishes a field's shaped view into caller-owned buffers: the node it draws in, its
    /// resolved font, and one cluster per selectable position with the rect and the leading
    /// and trailing carets DirectWrite hit-tested for it.
    ///
    /// One call rather than one per answer, because input asks all of it at the same moment
    /// and each entry point is another generation check on the same slot. The edges are the
    /// layout's own hit test, so they are the positions input places a caret at rather than a
    /// re-derivation of them; a masked run's clusters are remapped to source positions by the
    /// field row, which holds both strings.
    pub(crate) fn field_view(
        &self,
        key: MeasureKey,
        out: &mut Vec<Cluster>,
    ) -> Option<(NodeId, FontSpec, Rect)> {
        let entry = self.entries.get(key)?;
        let run = &entry.run;
        out.clear();
        let mut at = 0;
        while at < run.len() {
            let (leading, hit) = run.caret(at, false);
            let end = (hit.position + hit.length).max(at + 1).min(run.len());
            let (trailing, _) = run.caret(end - 1, true);
            out.push(Cluster {
                start: at,
                end,
                rect: hit.rect,
                leading: leading.x,
                trailing: trailing.x,
            });
            at = end;
        }
        let (end, hit) = run.caret(run.len(), false);
        Some((
            entry.node,
            entry.font,
            Rect {
                x: end.x,
                y: end.y,
                w: 0.0,
                h: hit.rect.h,
            },
        ))
    }

    /// How many slots a walk visits, vacated ones included.
    fn slots(&self) -> usize {
        self.entries.rows.len()
    }

    /// The node whose solved box decides where a run's glyphs stand, and whether it is
    /// rotated a quarter turn.
    fn placement(&self, at: usize) -> Option<(NodeId, bool)> {
        let entry = self.entries.at(at)?;
        Some((entry.node, entry.style.vertical))
    }

    /// Shares the pinned run's hit-test geometry with static text providers.
    pub(crate) fn uia_geometry(
        &self,
        key: MeasureKey,
    ) -> Option<std::sync::Arc<crate::text_input::Geometry>> {
        let entry = self.entries.get(key)?;
        Some(std::sync::Arc::clone(entry.uia.get_or_init(|| {
            let mut clusters = Vec::new();
            let (_, _, end) = self
                .field_view(key, &mut clusters)
                .expect("the text entry is live");
            if let Some(author) = entry.author.as_deref() {
                entry.fold.remap(author, &mut clusters);
            }
            std::sync::Arc::new(crate::text_input::Geometry {
                clusters: clusters.into(),
                end,
                ..Default::default()
            })
        })))
    }

    /// Brings a run up to date under `class`, fixes it at `w`, and answers whether the glyphs
    /// moved and its coverage is owed.
    ///
    /// The pin is the single authoritative writer of the width. Whichever probe the solve
    /// happened to end on would otherwise decide where the glyphs landed.
    ///
    /// # Panics
    ///
    /// If no shaping engine is installed on this thread.
    /// The slot a live key names, which is what the text pass addresses a run by.
    fn slot_of(&self, key: MeasureKey) -> Option<usize> {
        self.entries.get(key).map(|_| key.at as usize)
    }

    /// Whether [`Table::pin`] would reshape or re-pin the run at `at`: what a pass that
    /// skipped it has to be sure of.
    #[cfg(debug_assertions)]
    fn due(&self, at: usize, class: WidthClass, w: f32) -> bool {
        let Some(entry) = self.entries.at(at) else {
            return false;
        };
        let font = typography(entry.style.typography, entry.scope.at_width(class));
        let reshapes = entry.stale || entry.class != class || entry.font != font;
        let held = w <= 0.0 && !entry.text.as_str().is_empty();
        reshapes || (!held && entry.pinned != w)
    }

    fn pin(&mut self, at: usize, class: WidthClass, w: f32) -> bool {
        let engine = self.engine.as_ref().expect(ENGINE);
        let Some(entry) = self.entries.at_mut(at) else {
            return false;
        };
        entry.sync(engine, class);
        if (w <= 0.0 && !entry.text.as_str().is_empty()) || (entry.pinned == w && !entry.stale) {
            return false;
        }
        let moved = entry.run.pin(w);
        if moved {
            entry.uia.take();
        }
        entry.pinned = w;
        moved
    }

    /// Harvests a run's glyph data and answers how many lines it broke into.
    ///
    /// Harvests first: the pin marks the layout stale, and reading a line before the walk
    /// would answer from the previous width. The walk happens once here rather than at each
    /// reader below.
    ///
    /// # Panics
    ///
    /// If no shaping engine is installed on this thread.
    fn harvest(&mut self, at: usize) -> Option<usize> {
        let engine = self.engine.as_ref().expect(ENGINE);
        let entry = self.entries.at_mut(at)?;
        engine.harvest(&mut entry.run).expect(LAYOUT);
        Some(entry.run.lines().len())
    }

    /// What a run's colour resolves from: its own stated ink, the control that owns it, and
    /// the scope both resolve through.
    fn lighting(&self, at: usize) -> Option<(Option<Role>, ControlId, Scope)> {
        let entry = self.entries.at(at)?;
        Some((entry.style.ink, entry.owner, entry.scope))
    }

    /// The control a run's colour and accessible name belong to.
    fn owner(&self, at: usize) -> ControlId {
        self.entries
            .at(at)
            .map_or(ControlId::NONE, |entry| entry.owner)
    }

    /// The sprite a single-line run draws into, or `None` where the run can break.
    fn line_sprite(&self, at: usize) -> Option<SpriteId> {
        match self.entries.at(at)?.target {
            Target::Line { sprite, .. } => Some(sprite),
            Target::Wrapped { .. } | Target::Annotated(_) => None,
        }
    }

    /// How many lines a wrapping run currently draws.
    fn lines(&self, at: usize) -> usize {
        match self.entries.at(at).map(|entry| &entry.target) {
            Some(Target::Wrapped { lines, .. }) => lines.len(),
            _ => 0,
        }
    }

    /// Re-points a run's fold, answering the node whose measure has to be re-asked.
    ///
    /// What a field states when its scope becomes a masked one: the fold is the run's, so the
    /// plaintext never reaches the shaper, the coverage or the automation snapshot.
    pub(crate) fn set_fold(&mut self, key: MeasureKey, fold: Fold) -> Option<NodeId> {
        let entry = self.entries.get_mut(key)?;
        if entry.fold == fold {
            return None;
        }
        let author = entry
            .author
            .clone()
            .unwrap_or_else(|| Box::from(entry.text.as_str()));
        entry.fold = fold;
        entry.author = (fold != Fold::None).then(|| author.clone());
        entry.text.set(&author, fold);
        entry.stale = true;
        let node = entry.node;
        self.restated.push(key.at as usize);
        Some(node)
    }

    /// Appends one line's segments to the harvest buffers and answers where they landed and
    /// the tile they occupy.
    fn line(&mut self, at: usize, line: usize, tag: Option<u32>) -> (windows_text::Span, Ink) {
        self.segs.clear();
        let Self { entries, segs, .. } = self;
        let Some(entry) = entries.at_mut(at) else {
            return (windows_text::Span::EMPTY, Ink::default());
        };
        let span = match tag {
            Some(tag) => entry.run.tagged_segments(line, tag, segs),
            None => entry.run.segments(line, segs),
        };
        (span, tag.map_or_else(|| entry.run.line_ink(line), |tag| entry.run.tagged_ink(line, tag).0))
    }

    /// Drops the sprites a shorter run no longer breaks onto, newest first.
    fn shed_line(&mut self, at: usize, count: usize) -> Option<(SpriteId, RunId)> {
        let Target::Wrapped { lines, .. } = &mut self.entries.at_mut(at)?.target else {
            return None;
        };
        (lines.len() > count).then(|| lines.pop()).flatten()
    }

    /// The sprite and coverage slot a wrapped line already owns.
    fn line_slot(&self, at: usize, line: usize) -> Option<(SpriteId, RunId)> {
        let Target::Wrapped { lines, .. } = &self.entries.at(at)?.target else {
            return None;
        };
        lines.get(line).copied()
    }

    /// Where a fresh line sprite is mounted: its column, and the line it sits above.
    fn line_mount(&self, at: usize) -> Option<(GroupId, Option<NodeId>)> {
        let Target::Wrapped { group, lines } = &self.entries.at(at)?.target else {
            return None;
        };
        Some((*group, lines.last().map(|(sprite, _)| sprite.0)))
    }

    fn push_line(&mut self, at: usize, sprite: SpriteId, run: RunId) {
        if let Some(entry) = self.entries.at_mut(at)
            && let Target::Wrapped { lines, .. } = &mut entry.target
        {
            lines.push((sprite, run));
        }
    }
}

impl Host {
    /// Pins each run whose box, class or string may have moved at the width the solve gave
    /// it, and re-publishes the ones that moved.
    ///
    /// Runs between the solve and the hand-over, so a run laid out at the width layout chose
    /// reaches the *same* patch as that layout rather than arriving a frame after the box it
    /// was measured for.
    ///
    /// A run is pinned where its node is in the change set — a resize moves every label's
    /// box, and each one lands there — where its own string moved, and on a sweep, which a
    /// theme or scale change owes. A flush that moved one label reshapes and resolves one
    /// run, not every run in the window.
    pub(crate) fn publish_text(&mut self) {
        if self.changes.sweeping() {
            self.text.restated.clear();
            for at in 0..self.text.slots() {
                self.publish_run(at);
            }
        } else {
            let restated = core::mem::take(&mut self.text.restated);
            for &at in &restated {
                self.publish_run(at);
            }
            self.text.restated = restated;
            self.text.restated.clear();
            for i in 0..self.tree.moved.len() {
                let node = self.tree.moved[i];
                if !self.tree.is_live(node) {
                    continue;
                }
                if let Some(at) = self.text.slot_of(self.tree.c.text[node.index()]) {
                    self.publish_run(at);
                }
            }
        }
        #[cfg(debug_assertions)]
        for at in 0..self.text.slots() {
            if let Some((node, class, w)) = self.run_width(at) {
                debug_assert!(!self.text.due(at, class, w), "the change set missed a run: {node:?}");
            }
        }
    }

    /// The node a run stands in, the class it resolves at and the inline extent it is
    /// pinned to.
    fn run_width(&self, at: usize) -> Option<(NodeId, WidthClass, f32)> {
        let (node, vertical) = self.text.placement(at)?;
        let solved = self.geom(node);
        // A vertical run is rotated, so its inline extent is the box's cross axis.
        let w = if vertical { solved.size.y } else { solved.size.x };
        Some((node, self.tree.class(node), w))
    }

    /// Pins one run at its solved width, and re-emits it where its glyphs moved.
    fn publish_run(&mut self, at: usize) {
        let Some((_, class, w)) = self.run_width(at) else {
            return;
        };
        if self.text.pin(at, class, w) {
            self.emit_run(at);
        }
    }

    /// Re-sends the coverage of every live text run.
    ///
    /// Answers a rebuilt device and a changed pixel grid, which the ordinary publish cannot: a
    /// coverage tile is rasterized at **device** resolution and neither event moves a DIP, so
    /// the width gate reports nothing moved for exactly the case where every raster is wrong.
    /// Shaping is not redone: it is resolution-independent, and the run is already pinned at
    /// the width the last solve gave it.
    pub fn reemit_text(&mut self) {
        for at in 0..self.text.slots() {
            self.emit_run(at);
        }
    }

    /// Rebases every run onto `root` and marks it behind its source.
    ///
    /// The other half of a theme flip: it moves the type ramp and the ink, so the next solve
    /// reshapes each run. There is nothing to emit here.
    pub(crate) fn retheme_text(&mut self, root: Scope) {
        for at in 0..self.text.slots() {
            let Some(entry) = self.text.entries.at_mut(at) else {
                continue;
            };
            entry.scope = entry.scope.in_theme(root);
            entry.stale = true;
        }
    }

    /// Re-resolves the colour of every run one control owns.
    ///
    /// A run is painted where it is emitted rather than through a paint row of its own, so a
    /// model state that changes a control's foreground reaches its label here.
    pub(crate) fn relight_runs(&mut self, owner: ControlId) {
        if owner.is_none() {
            return;
        }
        for at in 0..self.text.slots() {
            if self.text.owner(at) == owner {
                self.paint_run(at);
            }
        }
    }

    /// Paints every sprite a run draws into, in the light its ink resolves to.
    fn paint_run(&mut self, at: usize) {
        let Some((ink, owner, scope)) = self.text.lighting(at) else {
            return;
        };
        // The owning control's chrome governs its label, state by state; a run with no
        // chrome above it paints the ink its style states.
        let role = self
            .owner_ink(owner)
            .or(ink)
            .unwrap_or(Role::Text(Text::Primary));
        let light = resolve(role, scope.for_paint());
        if self.paint_annotated(at, role, scope) { return; }
        if let Some(sprite) = self.text.line_sprite(at) {
            self.paint(sprite, Paint::Solid(light), None);
            return;
        }
        for line in 0..self.text.lines(at) {
            let Some((sprite, _)) = self.text.line_slot(at, line) else {
                continue;
            };
            self.paint(sprite, Paint::Solid(light), None);
        }
    }

    /// Releases a run and the resource slots it held.
    ///
    /// The line sprites go with the subtree the destroy cascades over, so only the coverage
    /// tiles are named here — and they are refcounted on the far side, which is what makes
    /// dropping this side's claim safe while a sprite is still holding one.
    pub(crate) fn release_text(&mut self, key: MeasureKey) {
        let Some(mut entry) = self.text.entries.remove(key) else {
            return;
        };
        match &mut entry.target {
            Target::Line { run, .. } => self.release(*run),
            Target::Wrapped { lines, .. } => {
                for (_, run) in lines.drain(..) {
                    self.release(run);
                }
            }
            Target::Annotated(data) => {
                for part in data.parts.drain(..) { self.release(part.run); }
            }
        }
        self.text.spare.push(entry.run);
    }

    /// Sends this run's coverage, whatever the layout did.
    ///
    /// A line's `RunId` is minted **once** and re-pointed for the life of the node: a
    /// resource slot per text change would churn the far side's table for a string that
    /// occupies the same sprite throughout. Sprites are minted and destroyed only as the line
    /// **count** changes, which for a caption is a resize crossing a break and not a
    /// keystroke.
    fn emit_run(&mut self, at: usize) {
        let Some(count) = self.text.harvest(at) else {
            return;
        };
        if self.emit_annotated(at, count) { return; }
        if let Some(run) = self.text.entries.at(at).and_then(Entry::line) {
            let (span, ink) = self.coverage(at, 0, None);
            self.set_run(run, span, ink);
            self.paint_run(at);
            return;
        }
        while let Some((sprite, run)) = self.text.shed_line(at, count) {
            self.destroy(sprite.0, Exit::None);
            self.release(run);
        }
        let vertical = self
            .text
            .placement(at)
            .is_some_and(|(_, vertical)| vertical);
        let mut top = 0.0;
        for line in 0..count {
            let (span, ink) = self.coverage(at, line, None);
            match self.text.line_slot(at, line) {
                Some((_, run)) => self.set_run(run, span, ink),
                None => self.mint_line(at, span, ink, vertical),
            }
            let Some((sprite, _)) = self.text.line_slot(at, line) else {
                continue;
            };
            // Rotation is about the origin: one line-height of rightward translation keeps
            // vertical coverage inside the owner's axis-swapped extent.
            let origin = Vector2 {
                x: if vertical { ink.size.y } else { 0.0 },
                y: top,
            };
            self.visual_rect(sprite, origin, ink.size);
            top += if vertical { ink.size.x } else { ink.size.y };
        }
        self.paint_run(at);
    }

    /// Mints one wrapped line's sprite and coverage slot above the lines already placed.
    fn mint_line(&mut self, at: usize, span: Span, ink: Ink, vertical: bool) {
        let Some((group, after)) = self.text.line_mount(at) else {
            return;
        };
        let sprite = self.visual(group, after);
        let run = self.run(span, ink);
        self.mask(sprite, Mask::Run(run));
        if vertical {
            self.write_channel(
                sprite.0,
                Prop::RotationAngle,
                Value::Scalar(core::f32::consts::FRAC_PI_2),
            );
        }
        self.text.push_line(at, sprite, run);
    }

    /// Copies one line's segments into the patch and answers the span naming them.
    ///
    /// The shaper's spans address the table's own buffers and the wire's address the patch's
    /// pools, so the glyphs, advances and offsets are appended here and each segment is
    /// re-pointed at where they landed. A segment names the face fallback chose, which a
    /// family index cannot.
    fn coverage(&mut self, at: usize, line: usize, tag: Option<u32>) -> (Span, Ink) {
        let (local, mut ink) = self.text.line(at, line, tag);
        let phase = tag.map_or(0.0, |tag| {
            let offset = self.text.entries.at(at).unwrap().run.tagged_ink(line, tag).1;
            offset - (offset * self.env.scale()).floor() / self.env.scale()
        });
        // Cropped tiles start on the full line's pixel grid; the fractional remainder
        // stays in the glyph origins so a color boundary does not change glyph spacing.
        ink.size.x += phase;
        let Self { text, pending, .. } = self;
        let mut span = Span { off: 0, len: 0 };
        for seg in local.of(&text.segs.segs) {
            let glyphs = pending.push_glyphs(seg.glyphs.of(&text.segs.glyphs));
            let advances = pending.push_floats(seg.advances.of(&text.segs.advances));
            // Two floats per glyph on the wire, where the shaper holds one pair per glyph.
            let offsets = pending.push_floats(seg.offsets.of(&text.segs.offsets).as_flattened());
            let placed = pending.push_segs(&[GlyphSeg {
                face: seg.face,
                em: seg.em,
                bidi: seg.bidi,
                origin: Vector2 { x: seg.origin.x + phase, y: seg.origin.y },
                glyphs,
                advances,
                offsets,
            }]);
            if span.len == 0 {
                span.off = placed.off;
            }
            span.len += 1;
        }
        (span, ink)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folded_clusters_preserve_expansions_surrogates_and_combined_spans_without_allocation() {
        for (fold, text, spans, expected) in [
            (
                Fold::Caps,
                "aß\u{1f600}",
                [(0, 1), (1, 2), (2, 3), (3, 5)],
                [(0, 1), (1, 2), (1, 2), (2, 4)],
            ),
            (
                Fold::Mask,
                "a\u{1f600}bc",
                [(0, 1), (1, 2), (2, 3), (3, 4)],
                [(0, 1), (1, 3), (3, 4), (4, 5)],
            ),
            (
                Fold::Caps,
                "fißxy",
                [(0, 2), (2, 4), (4, 5), (5, 6)],
                [(0, 2), (2, 3), (3, 4), (4, 5)],
            ),
        ] {
            let mut clusters = spans.map(|(start, end)| Cluster {
                start,
                end,
                ..Default::default()
            });
            let before = crate::counting::allocations();
            fold.remap(text, &mut clusters);
            assert_eq!(crate::counting::allocations(), before);
            assert_eq!(clusters.map(|c| (c.start, c.end)), expected);
        }
    }

    fn folded(fold: Fold, text: &str) -> String {
        let mut out = String::new();
        fold.write(&mut out, text);
        out
    }

    #[test]
    fn a_fold_recognises_its_own_output_without_writing_a_buffer() {
        assert!(Fold::Caps.holds("ABC", "abc"));
        assert!(Fold::Mask.holds("\u{25cf}\u{25cf}", "ab"));
        assert!(Fold::None.holds("ab", "ab"));
        assert!(!Fold::Caps.holds("ABC", "abd"));
    }

    /// One mask per character, so a supplementary character draws one dot and not two.
    #[test]
    fn a_mask_draws_one_cluster_per_character() {
        assert_eq!(folded(Fold::Mask, "a\u{1f600}"), "\u{25cf}\u{25cf}");
        assert_eq!(folded(Fold::Caps, "straße"), "STRASSE");
    }

    #[test]
    fn a_static_source_becomes_owned_at_its_first_change_and_keeps_the_buffer() {
        let mut source = Source::Static("one");
        assert!(
            !source.set("one", Fold::None),
            "an equal string never moves"
        );
        assert!(source.set("two", Fold::None));
        let Source::Owned(owned) = &source else {
            panic!("a changed run owns its string");
        };
        let at = owned.as_ptr();
        assert!(source.set("three", Fold::None));
        let Source::Owned(owned) = &source else {
            unreachable!()
        };
        assert_eq!(at, owned.as_ptr(), "the buffer is written in place");
    }

    /// A key held past its unmount names nothing, so a late reader cannot address the run
    /// that recycled its slot.
    #[test]
    fn a_released_key_reads_stale_once_its_slot_is_taken_again() {
        let mut store = Store::default();
        store.rows.push((1, None));
        let stale = MeasureKey { at: 0, age: 1 };
        store.rows[0].0 = 2;
        assert!(store.get(stale).is_none());
    }
}
