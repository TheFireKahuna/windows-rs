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
