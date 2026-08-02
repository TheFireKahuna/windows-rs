//! Text: where a naive authoring layer allocates, and how this one does not.
//!
//! Most strings on a screen are chrome and are `&'static str`, so they must cost nothing.
//! The rest are the application's own data, which it has already paid for.

use crate::signal::Signal;
use std::cell::RefCell;
use std::rc::Rc;
use windows_numerics::Vector2;
use windows_scene::Span;
use windows_text::{FontSpec, Ink, SegBuffers};

/// How a run occupies the width it is given.
pub use windows_text::Flow;

/// What a text-taking method accepts.
///
/// `&'static str` — the common case — costs **no allocation, ever**. An owned string costs
/// the application's own clone and no copy of ours: the shaped result goes into the patch's
/// glyph buffers and the source is dropped.
pub enum TextSource {
    Static(&'static str),
    Owned(String),
    Dynamic(Box<dyn Fn() -> String>),
}

impl TextSource {
    /// Whether reading can ever answer differently — the same gate every other value goes
    /// through, so a static label produces no `Effect`.
    #[must_use]
    pub fn is_constant(&self) -> bool {
        !matches!(self, Self::Dynamic(_))
    }

    /// Reads the current text, registering a dependency where there is one.
    pub fn read<R>(&self, f: impl FnOnce(&str) -> R) -> R {
        match self {
            Self::Static(s) => f(s),
            Self::Owned(s) => f(s),
            Self::Dynamic(g) => f(&g()),
        }
    }
}

impl core::fmt::Debug for TextSource {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Static(s) => out.debug_tuple("Static").field(s).finish(),
            Self::Owned(s) => out.debug_tuple("Owned").field(s).finish(),
            Self::Dynamic(_) => out.write_str("Dynamic(..)"),
        }
    }
}

impl From<&'static str> for TextSource {
    fn from(s: &'static str) -> Self {
        Self::Static(s)
    }
}

impl From<String> for TextSource {
    fn from(s: String) -> Self {
        Self::Owned(s)
    }
}

impl From<&String> for TextSource {
    fn from(s: &String) -> Self {
        Self::Owned(s.clone())
    }
}

/// A closure is reactive; anything else is not.
///
/// Deliberately **not** a blanket `impl<F: Fn() -> String>`: that would collide with the
/// two owned forms above, and Rust answers an overlap by refusing to compile it. The marker
/// trick [`Signal`] uses is unnecessary here because there are only two cases and one of
/// them is a function.
pub fn reactive(f: impl Fn() -> String + 'static) -> TextSource {
    TextSource::Dynamic(Box::new(f))
}

/// Text bound to any readable thing — a cell, a memo, a closure or a constant.
pub fn bound<M>(v: impl Signal<String, M> + 'static) -> TextSource {
    if v.is_constant() {
        TextSource::Owned(v.read())
    } else {
        TextSource::Dynamic(Box::new(move || v.read()))
    }
}

// ── the shaping seam ─────────────────────────────────────────────────────────────

/// What one line left in the buffers it was appended to.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Shaped {
    /// The segments, spanning the buffer that was passed in. A **list**, because font
    /// fallback splits a line: a label mixing scripts resolves to two faces, and a segment
    /// that could only name one would draw the second face's glyph ids through the first.
    pub segs: Span,
    /// The tile the line occupies, and where its baseline sits in it.
    pub ink: Ink,
}

/// One laid-out run, kept rather than rebuilt.
///
/// Kept, because a layout pass probes a node several times with different constraints
/// before deciding one, and glyph positions are not knowable until the last of those. So
/// measuring and pinning are separate: [`measure`](Self::measure) is called freely and
/// moves nothing, [`pin`](Self::pin) is the single authoritative writer of the width, and
/// only what `pin` says moved is re-emitted.
///
/// A run is thread-affine and does not need to be otherwise: what crosses the seam is
/// `GlyphSeg` over `SegBuffers`, plain `Copy` data.
pub trait Run: 'static {
    /// What this run occupies at `available` — `None` for an indefinite width — without
    /// moving a glyph.
    fn measure(&mut self, available: Option<f32>) -> Vector2;

    /// Fixes the width the glyphs are laid out at, and answers whether that moved them.
    ///
    /// A non-wrapping run is laid out leading and does not break, so its box decides only
    /// what hangs outside it and this is usually `false` — which is what keeps a resize
    /// from re-rasterizing every label on the screen.
    fn pin(&mut self, width: f32) -> bool;

    /// Re-lays for new text or a new font, keeping the buffers.
    fn reshape(&mut self, text: &str, font: FontSpec, flow: Flow);

    /// How many lines it broke into. One, for everything that does not wrap.
    fn lines(&mut self) -> usize;

    /// Appends `line`'s segments to `out` and names what it wrote.
    fn emit(&mut self, line: usize, out: &mut SegBuffers) -> Shaped;
}

/// What the widget layer needs from the text engine, and nothing more.
pub trait Shaper: 'static {
    /// Lays `text` out under `font`.
    fn shape(&self, text: &str, font: FontSpec, flow: Flow) -> Box<dyn Run>;
}

thread_local! {
    /// The installed engine.
    ///
    /// Thread-local and not a process-wide `OnceLock`, because a text engine is
    /// thread-affine — a `ShapedRun` holds its layout object — and demanding `Send + Sync`
    /// here would mean no real engine could be installed without a wrapper whose only job
    /// is to claim otherwise. Measure runs inside the model's own solve and shaping runs at
    /// mount, both on this thread, so nothing needs it anywhere else.
    static SHAPER: RefCell<Option<Rc<dyn Shaper>>> = const { RefCell::new(None) };
}

/// Installs the text engine. Once per thread, before anything with text mounts.
///
/// # Panics
///
/// If one is already installed. A screen measured by one engine and drawn by another is a
/// defect whose only symptom is text in the wrong place.
pub fn install_shaper(shaper: impl Shaper) {
    SHAPER.with(|slot| {
        let mut slot = slot.borrow_mut();
        assert!(slot.is_none(), "a shaper is already installed");
        *slot = Some(Rc::new(shaper));
    });
}

/// Whether a shaper has been installed on this thread.
#[must_use]
pub fn shaper_installed() -> bool {
    SHAPER.with(|slot| slot.borrow().is_some())
}

/// The installed text engine.
///
/// **Panics rather than measuring zero.** A widget layer that answered a plausible size
/// with no engine behind it would lay a screen out around a lie, and the failure would
/// surface as mysterious geometry rather than as a missing dependency.
///
/// # Panics
///
/// If no shaper is installed.
#[must_use]
pub fn shaper() -> Rc<dyn Shaper> {
    SHAPER.with(|slot| {
        slot.borrow().clone().expect(
            "a shaper must be installed before text mounts: call \
             windows_ui::widget::install_shaper once at start-up",
        )
    })
}
