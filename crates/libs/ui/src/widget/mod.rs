//! The widget set: the seed functions an application calls, and the vocabulary they name.
//!
//! A widget owns sprites, a hit entry and a role, so adding one changes this crate. A
//! composition is an application-side function returning a tree of widgets, and it adds
//! nothing here.

pub mod roles;

mod kind;
pub(crate) mod seed;
mod state;
mod text;

pub use kind::{
    Chrome, Interaction, ModelState, Motion, Range, RoleSet, StatePolicy, TURN_SPAN, TURN_SWEEP,
    UiaRole, Wash, angle_of, detent_delta, fraction_of, offset_of,
};
pub use seed::{
    CHIP_PLATE, SliderStyle, box_, button, caption, card, chip, display, edge_button, field,
    flyout, icon_button, knob, label, meter, micro, mono, note, panel, path, pills, segmented,
    select, sheet, slider, text, title, toggle, vertical_label, window_title,
};
// `ChromeRow` is one row of a widget's colour table; `Controls` is the front thread's table
// of live controls.
pub use state::{ChromeRow, Controls, Dragging, Front, Intent, What};
pub use text::{Flow, Shaped, TextSource, Written, reactive, shown};
