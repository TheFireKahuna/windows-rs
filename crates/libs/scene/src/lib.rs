#![doc = include_str!("../readme.md")]

// ── the seam · plain `Send` data both halves name ───────────────────────────────────
mod hit_entry;
mod patch;
mod sink;

// ── the query · no composition object, two threads ──────────────────────────────────
mod hit;

// ── the scene half · `!Send` · owns every composition object ────────────────────────
mod arena;
mod realize;
mod scene;

/// The child splice, which both halves run over their own storage.
///
/// A store implements [`Forest`] over its own rows and calls the same [`link`], [`unlink`]
/// and [`children`]. Two hand-written splices put the same subtle bug in two places, only
/// one of which is covered.
pub use arena::{Forest, Links, NO_LINK, children, link, unlink};
pub use hit::{HitTable, scan};
pub use hit_entry::{
    ContactKind, Hit, HitDecl, HitEntry, HitFlags, NO_ENTRY, TOUCH_TARGET_DIPS, default_inflation,
    pack_offset, unpack_offset,
};
pub use patch::{Attach, Op, PatchPool, SinkPatch, Span};
pub use realize::{Backends, BoxKey, CACHE_CAP, Cache, CellKey, fit, nine_slice};
pub use scene::{
    Audit, BackdropSpec, CHROME_DAMPING, CHROME_PERIOD, Census, Glow, SCROLL_DAMPING,
    SCROLL_PERIOD, Scene, SceneEvent, invalid_arg,
};
pub use sink::*;

pub use windows_composition::ManipulationPointer;
pub use windows_core::Result;
