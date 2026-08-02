#![doc = include_str!("../readme.md")]

// Every `unsafe` block in this crate is a call into a generated binding, which COM and
// the Win32 wait primitives cannot express as safe in a signature. What discharges it is
// uniform enough to state once rather than at every call site: each interface pointer is
// owned by the wrapper that calls it and can be neither null nor dangling, every
// out-parameter is a stack local that outlives its call, and no method here retains a
// borrow past its return. The handles are kernel objects owned by the region or group
// that minted them, and are closed exactly once from that owner. The three obligations
// that genuinely fall on a *caller* are marked where they arise: the surface handle a
// region hands out must be released before the region drops, `CoInitializeEx` is paired
// on the present thread, and a texture handed to `Gpu::adopt` must come from that `Gpu`.
// There is deliberately no `dead_code` expectation here. Every filter entry this crate
// does not consume is exactly what `rustc`'s own warning over the private generated module
// exists to report, so the enum *members* are named individually rather than their types —
// naming a type generates all of its members and drowns that signal. If this ever needs
// widening, trim the filter instead.
#[expect(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::missing_transmute_annotations,
    clippy::upper_case_acronyms
)]
mod bindings;

mod device;
mod frame;
mod group;
mod pump;
mod region;

// Re-exported crate-wide so every module can `use super::*` rather than naming what it
// touches. None of the generated half is public: no generated type crosses this crate's
// boundary, and the one object that must — the composition surface handle — leaves as a
// raw pointer, because it is a kernel handle and not an interface.
pub(crate) use bindings::*;
pub(crate) use std::rc::Rc;
pub(crate) use windows_core::Interface;

pub use device::{Flushed, PresentationDevice};
pub use frame::{Epoch, Frame, FrameCtx, Part, RegionInput, RegionParts, SubId};
pub use group::{
    Instance, Interrupt, Outcome, PresentStatistic, PresentTally, PresentationGroup, Queue,
};
pub use pump::{Bound, Presenter, Tuning};
pub use region::{Extent, PresentationRegion, RegionKey, RegionSpec};

// The types a consumer names to implement a `Frame`, so it does not have to depend on the
// drawing crates to write one. Re-exported rather than re-declared: there is one `Rect`
// and one draw choke in this stack, and a parallel pair here would be two of each.
pub use windows_color::OutputTransform;
pub use windows_core::Result;
pub use windows_d2d::{Draw, Gpu, Rect};

// Used across this crate's own modules, and by the drawing seam.
pub(crate) use windows_d2d::{Loss, Opacity, Pass, PassError, Target};
pub(crate) use windows_window::Event;
// The clock this thread paces off and the scheduling class it asks for are both
// `windows-window`'s: thread properties a window's lifetime implies, consumed here rather than
// re-derived. `Watch` is one consumer's wake on a window's visibility — the present thread and
// the window's own pacer each hold their own, so a change reaches both.
pub use windows_window::Watch;
pub(crate) use windows_window::clock::{self, Observed};
pub(crate) use windows_window::qos::{self, Speed};

/// The system-interrupt clock a present is scheduled against: 100 ns units since the
/// system started.
///
/// The same clock `present_at` takes, which is why a slot time is arithmetic on this value
/// and needs no conversion.
#[must_use]
pub fn interrupt_time_now() -> u64 {
    let mut value = 0u64;
    // SAFETY: the out-parameter is a stack local that outlives the call.
    unsafe { QueryInterruptTimePrecise(&mut value) };
    value
}
