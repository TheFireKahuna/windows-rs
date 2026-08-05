//! The widget set: the seed functions an application calls, and the vocabulary they name.
//!
//! A widget owns sprites, a hit entry and a role, so adding one changes this crate. A
//! composition is an application-side function returning a tree of widgets, and it adds
//! nothing here.

pub mod roles;

mod kind;
mod seed;
mod state;
mod text;

pub use kind::{
    Chrome, Interaction, ModelState, Motion, Range, RoleSet, StatePolicy, TURN_SPAN, TURN_SWEEP,
    UiaRole, Wash, angle_of, detent_delta, fraction_of, offset_of,
};
pub use seed::{
    box_, button, caption, card, field, flyout, icon_button, knob, label, meter, mono, panel, path,
    segmented, select, slider, text, title, toggle,
};
// `ChromeRow` is one row of a widget's colour table; `Controls` is the front thread's table
// of live controls.
pub use state::{ChromeRow, Controls, Front, Intent, What};
pub use text::{Flow, Shaped, TextSource, Written, reactive, shown};
