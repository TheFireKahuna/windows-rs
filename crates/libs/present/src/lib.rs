#![doc = include_str!("../readme.md")]

// Every `unsafe` block in this crate calls a generated binding, which COM and the Win32
// wait primitives cannot express as safe in a signature. The invariants that discharge them
// are uniform, so they are stated once here rather than at every call site: each interface
// pointer is owned by the wrapper that calls it and is neither null nor dangling, every
// out-parameter is a stack local that outlives its call, and no method here retains a borrow
// past its return. Handles are kernel objects owned by the region or group that minted them
// and are closed exactly once from that owner. Three obligations fall on a caller instead:
// the surface handle a region hands out must be released before the region drops,
// `CoInitializeEx` must be paired on the present thread, and a texture passed to
// `Gpu::adopt` must come from that `Gpu`.
//
// The list below carries no `dead_code` entry. An unused item in the private generated
// module means a bindings-filter entry this crate does not consume, and `rustc`'s warning is
// the only report of that. The filter names enum members individually rather than their
// types, because naming a type generates every one of its members and buries the warning.
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

// Re-exported crate-wide so every module reaches them through `use super::*`. No generated
// type crosses this crate's boundary; the one object that does — the composition surface
// handle — leaves as a raw pointer, because it is a kernel handle rather than an interface.
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

// The types a consumer names to implement a `Frame`, re-exported so writing one needs no
// direct dependency on the drawing crates. Re-exported rather than re-declared, so a
// consumer's `Rect` and output transform are the same types the drawing crates accept.
pub use windows_color::OutputTransform;
pub use windows_core::Result;
pub use windows_d2d::{Draw, Gpu, Rect};

// Used across this crate's own modules, and by the drawing seam.
pub(crate) use windows_d2d::{Loss, Opacity, Pass, PassError, Target};
pub(crate) use windows_window::Event;
// The compositor clock this thread paces off and the scheduling class it asks for come from
// `windows-window`, which owns both as properties of a window's lifetime. `Watch` is one
// consumer's wake on a window's visibility: the present thread and the window's own pacer
// each hold their own, so a change reaches both.
pub use windows_window::Watch;
pub(crate) use windows_window::clock::{self, Observed};
pub(crate) use windows_window::qos::{self, Speed};

/// Returns the current system-interrupt time, in 100 ns units since the system started.
///
/// This is the clock [`PresentationGroup::present_at`] schedules against, so a slot time is
/// arithmetic on this value and needs no conversion.
#[must_use]
pub fn interrupt_time_now() -> u64 {
    let mut value = 0u64;
    // SAFETY: the out-parameter is a stack local that outlives the call.
    unsafe { QueryInterruptTimePrecise(&mut value) };
    value
}
