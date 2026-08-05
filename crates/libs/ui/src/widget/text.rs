//! Text sources: what a text-taking widget accepts, and how a changing string reaches the
//! shaper without allocating.
//!
//! Chrome strings are `&'static str` and cost nothing. The rest are the application's own
//! data, which it has already allocated.
//!
//! # A changing string is written, never returned
//!
//! [`Table::set_text`](crate::build::text) compares before it reshapes, and the case it
//! exists for is a value moving at display rate whose text does not: a gain readout dragged
//! from `-6.031` to `-6.028` formats to `-6.0 dB` both times. A source answering with a
//! `String` would allocate, format, copy and free on every one of those frames to discover
//! nothing had changed.
//!
//! So a dynamic source writes into a buffer the caller owns. One scratch buffer per reactive
//! run reaches its high-water mark once and allocates nothing afterwards, and the call site's
//! `push_str` reaches it directly.

use core::fmt::{Display, Write};
use windows_scene::Span;
use windows_text::Ink;

/// How a run occupies the width it is given.
pub use windows_text::Flow;

/// What a text-taking method accepts.
///
/// `&'static str` allocates nothing. An owned string costs the application's own allocation
/// and no copy here: the shaped result goes into the patch's glyph buffers and the source is
/// dropped.
pub enum TextSource {
    /// A string with the program's lifetime.
    Static(&'static str),
    /// A string this source owns.
    Owned(String),
    /// Appends the current text to the buffer it is given. The buffer is the caller's and is
    /// cleared before each read, so nothing here allocates once it has grown.
    Dynamic(Box<dyn Fn(&mut String)>),
}

impl TextSource {
    /// Returns whether reading this source can ever answer differently. A constant source
    /// raises no effect, so a static label registers no dependency.
    #[must_use]
    pub fn is_constant(&self) -> bool {
        !matches!(self, Self::Dynamic(_))
    }

    /// Appends the current text to `out`, registering a signal dependency where the source
    /// has one.
    ///
    /// The only reader of a [`TextSource`]. A caller wanting an owned snapshot appends into a
    /// `String` of its own, so the eager callers — a tooltip resolved when it opens, an
    /// accessible name interned into the published blob — share this path and its allocation
    /// policy.
    pub fn append(&self, out: &mut String) {
        match self {
            Self::Static(s) => out.push_str(s),
            Self::Owned(s) => out.push_str(s),
            Self::Dynamic(read) => read(out),
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

/// What a text-writing closure may answer.
///
/// `()`, which a `push_str` body ends in, or the [`core::fmt::Result`] a `write!` produces.
/// The result is discarded: a `String` aborts rather than failing to grow, so there is
/// nothing to propagate and no `let _ =` in front of a formatted binding.
pub trait Written {}
impl Written for () {}
impl Written for core::fmt::Result {}

/// Returns a source whose text `f` writes.
///
/// `push_str` and `write!` inside `f` reach the run's own buffer, so a binding that follows a
/// value allocates only while that buffer grows.
///
/// Writing nothing is how a binding says there is no text: an absent subject, or a figure the
/// engine has not answered.
pub fn reactive<R: Written>(f: impl Fn(&mut String) -> R + 'static) -> TextSource {
    TextSource::Dynamic(Box::new(move |out| {
        f(out);
    }))
}

/// Returns a source that shows what `f` answers, through [`Display`].
///
/// The [`Display`] impl writes straight into the run's buffer, so a readout following a meter
/// costs a format and no allocation.
///
/// Takes a closure rather than an `impl Signal`: [`Signal`](crate::signal::Signal)'s marker
/// separates a closure from a value only where the method fixes the value type, and here the
/// value type is the caller's, so `Signal<T, IsFn>` and `Signal<Self, IsValue>` both apply
/// and nothing pins `T`. A cell reads as `shown(move || cell.get())`.
pub fn shown<T: Display>(f: impl Fn() -> T + 'static) -> TextSource {
    TextSource::Dynamic(Box::new(move |out| {
        // The only error `write!` can answer with here is a `String`'s, and a `String`
        // aborts rather than failing to grow.
        let _ = write!(out, "{}", f());
    }))
}

// ── text about a subject that may not be there ───────────────────────────────────
//
// A surface bound to a selection asks the same question at every binding: is there a subject,
// is it the kind this binding describes, and if so write about it. Written out, that is a
// `with`, an `as_ref`, an `and_then` and an `if let` per binding, around one `push_str`.
//
// `about` answers all three in one place, and its answer to "there is nothing here" is to
// write nothing: an empty string is still a run, and a run still occupies a line.

/// Text about a subject that may not be there.
///
/// `project` reaches from whatever the signal holds to the subject this binding describes,
/// and may answer `None` a second time — a row that is not a processor, or a processor with
/// no figure of this kind. Where the signal's value is already the subject, it is `|it|
/// Some(it)`.
///
/// Either absence writes nothing: no run, no line, and no empty string standing in for one.
///
/// ```no_run
/// # use windows_ui::signal::Memo;
/// # use windows_ui::widget::label;
/// # #[derive(PartialEq)] struct Block { name: String }
/// # #[derive(PartialEq)] enum Row { Processor(Block), Other }
/// # impl Row { fn processor(&self) -> Option<&Block> { match self { Self::Processor(p) => Some(p), _ => None } } }
/// # fn f(selected: Memo<Option<Row>>) {
/// label(selected.about(Row::processor, |out, p| out.push_str(&p.name)));
/// # }
/// ```
macro_rules! about {
    ($holder:ident $(, $bound:path)*) => {
        impl<T: 'static $(+ $bound)*> crate::signal::$holder<Option<T>> {
            #[doc = "Returns a source describing this subject, or nothing where there is none."]
            #[doc = ""]
            #[doc = "Where the held value or `project` answers `None`, this writes nothing: no run, and no empty string standing in for one."]
            #[must_use]
            pub fn about<U, R: Written>(
                self,
                project: impl for<'a> Fn(&'a T) -> Option<&'a U> + 'static,
                write: impl Fn(&mut String, &U) -> R + 'static,
            ) -> TextSource {
                TextSource::Dynamic(Box::new(move |out| {
                    // The borrow is held across the write, so the subject is never taken out
                    // of the holder: a `get` would clone it once per binding, once per
                    // read.
                    self.with(|held| {
                        if let Some(subject) = held.as_ref().and_then(&project) {
                            write(out, subject);
                        }
                    });
                }))
            }
        }
    };
}

about!(Cell);
// A memo's read is gated on its own cutoff, which is where the extra bound comes from: the
// same `PartialEq` that stops a write propagating past a derivation it did not change.
about!(Memo, PartialEq);

// ── the shaping seam ─────────────────────────────────────────────────────────────

/// What one line left in the buffers it was appended to.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Shaped {
    /// The segments this line left, spanning the buffer that was passed in. Font fallback
    /// splits a line, so a label mixing scripts resolves to more than one face and more than
    /// one segment; a single segment would draw the second face's glyph ids through the
    /// first.
    pub segs: Span,
    /// The tile the line occupies, and where its baseline sits in it.
    pub ink: Ink,
}
