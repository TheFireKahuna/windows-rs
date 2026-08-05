//! Text: where a naive authoring layer allocates, and how this one does not.
//!
//! Most strings on a screen are chrome and are `&'static str`, so they must cost nothing.
//! The rest are the application's own data, which it has already paid for.
//!
//! # A changing string is written, never returned
//!
//! [`Table::set_text`](crate::build::text) already compares before it reshapes, because the
//! case that matters is **a value moving at display rate whose text does not**: a gain
//! readout dragged from `-6.031` to `-6.028` formats to `-6.0 dB` both times. A source that
//! answered with a `String` defeated half of what that comparison bought — the frame still
//! allocated, formatted, copied and freed to discover nothing had changed.
//!
//! So a dynamic source **writes into a buffer the caller owns**. One scratch buffer per
//! reactive run reaches its high-water mark once and allocates nothing afterwards, and the
//! call site's `push_str` reaches it directly rather than through a `String` built to be
//! thrown away.

use core::fmt::{Display, Write};
use windows_scene::Span;
use windows_text::Ink;

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
    /// Appends the current text to the buffer it is given. The buffer is the caller's and is
    /// cleared before each read, so nothing here allocates once it has grown.
    Dynamic(Box<dyn Fn(&mut String)>),
}

impl TextSource {
    /// Whether reading can ever answer differently — the same gate every other value goes
    /// through, so a static label produces no `Effect`.
    #[must_use]
    pub fn is_constant(&self) -> bool {
        !matches!(self, Self::Dynamic(_))
    }

    /// Appends the current text to `out`, registering a dependency where there is one.
    ///
    /// The **only** reader, so the three cases are answered in one place and a caller that
    /// wants an owned snapshot writes into a `String` of its own rather than being handed one
    /// this crate built. That is what keeps the eager callers — a tooltip resolved when it
    /// opens, an accessible name interned into the published blob — from being a second path
    /// with its own allocation policy.
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

impl From<&String> for TextSource {
    fn from(s: &String) -> Self {
        Self::Owned(s.clone())
    }
}

/// What a text-writing closure may answer.
///
/// Nothing, which is what a `push_str` body ends in; or the [`core::fmt::Result`] a `write!`
/// produces, which is discarded — a `String` does not fail to grow, it aborts, so there is
/// never anything to propagate. Accepting both is what removes the `let _ =` that otherwise
/// stands in front of every formatted binding in an application.
pub trait Written {}
impl Written for () {}
impl Written for core::fmt::Result {}

/// Text a closure writes.
///
/// The workhorse, and the one that costs nothing: `push_str` and `write!` reach the run's
/// own buffer, so a binding that follows a value allocates only while that buffer grows.
///
/// Writing **nothing** is how a binding says there is no text — an absent subject, a figure
/// the engine has not answered — and it is both cheaper and more direct than returning an
/// empty `String`.
pub fn reactive<R: Written>(f: impl Fn(&mut String) -> R + 'static) -> TextSource {
    TextSource::Dynamic(Box::new(move |out| {
        f(out);
    }))
}

/// A readout: a value that can show itself.
///
/// The [`Display`] impl writes straight into the run's buffer, so a scalar following a meter
/// costs a format and no allocation at all.
///
/// A closure and **not** an `impl Signal`, which is the one place that trick does not reach.
/// [`Signal`]'s marker disambiguates a closure from a value only when the method fixes the
/// value type — `opacity` knows it wants an `f32`. Here the whole point is that the type is
/// the caller's, so `Signal<T, IsFn>` and `Signal<Self, IsValue>` both apply and nothing
/// pins `T`. A cell reads as `shown(move || cell.get())`, which is what every other binding
/// in an application already looks like.
pub fn shown<T: Display>(f: impl Fn() -> T + 'static) -> TextSource {
    TextSource::Dynamic(Box::new(move |out| {
        // Infallible: the only error `write!` can answer with here is a `String`'s, and a
        // `String` does not fail to grow — it aborts. There is nothing to propagate.
        let _ = write!(out, "{}", f());
    }))
}

// ── text about a subject that may not be there ───────────────────────────────────
//
// A surface bound to a selection asks the same question at every one of its bindings: is
// there a subject, is it the kind I describe, and if so write this about it. Written out, it
// is a `with`, an `as_ref`, an `and_then` and an `if let` per binding — four lines of
// scaffolding around one `push_str`, repeated once per field of the panel.
//
// It is worth a combinator rather than a per-application helper for one reason: the answer to
// "there is nothing here" is **write nothing**, and that is a rule of this text layer rather
// than a convention an application is free to get wrong. A binding that returns an empty
// string instead is still a run, and a run still occupies a line.

/// Text about a subject that may not be there.
///
/// `project` reaches from whatever the signal holds to the thing this binding describes, and
/// may answer `None` a second time — a row that is not a processor, a processor with no
/// figure of this kind. Where the signal's value is already the subject, it is `|it|
/// Some(it)`.
///
/// Either absence writes **nothing at all**: no run, no line, and no empty string standing in
/// for one.
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
            #[doc = "Text about this subject, or nothing at all when there is none."]
            #[doc = ""]
            #[doc = "See [the module's note](self) on why absence writes nothing."]
            #[must_use]
            pub fn about<U, R: Written>(
                self,
                project: impl for<'a> Fn(&'a T) -> Option<&'a U> + 'static,
                write: impl Fn(&mut String, &U) -> R + 'static,
            ) -> TextSource {
                TextSource::Dynamic(Box::new(move |out| {
                    // The borrow is held across the write, which is the whole reason this is
                    // a combinator and not a projection an application could write with
                    // `get`: taking the subject out would clone it, once per binding, once
                    // per read.
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
// A memo's read is gated on its own cutoff, which is where the extra bound comes from — it
// is the same `PartialEq` that stops a write propagating past a derivation it did not change.
about!(Memo, PartialEq);

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
