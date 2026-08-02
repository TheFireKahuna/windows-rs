//! The widget set: data seeds, and the shared vocabulary they name.
//!
//! A widget owns sprites, a hit entry and a role, and adding one is a framework change. A
//! **composition** is a function returning a tree of widgets: it costs this crate nothing
//! and needs no permission, and it belongs to the application.

pub mod roles;

pub(crate) mod id;

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
// `Controls` and not a second `Chrome`: one is a widget's colour table row, the other is the
// front thread's control table, and naming both after the chrome they serve meant the crate
// carried two types of one name and an alias to tell them apart.
pub use state::{ChromeRow, Controls, Front, Intent, What};
pub use text::{
    Flow, Run, Shaped, Shaper, TextSource, bound, install_shaper, reactive, shaper,
    shaper_installed,
};
