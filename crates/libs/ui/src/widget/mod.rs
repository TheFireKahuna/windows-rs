//! Stock retained recipes and the vocabulary shared by custom controls.
//!
//! Application components use the same direct writes, declaring their children through
//! a borrowed `Ui` context.

pub mod roles;

mod kind;
mod recipes;
mod state;
mod text;

pub use kind::{
    Chrome, Interaction, ModelState, Motion, Range, RoleSet, ScalarPart, ScalarValue, StatePolicy,
    TURN_SPAN, TURN_SWEEP, UiaRole, Wash, angle_of, detent_delta, fraction_of, offset_of,
};
pub use recipes::{
    Choice, ChoiceStyle, SliderStyle, TextStyle, box_, button, button_with, caption, card, code,
    control_text, display, edge_button, field, field_with, flyout, icon_button, knob, label, meter,
    micro, mono, note, panel, path, pills, segmented, segmented_with, select, sheet, slider,
    styled_text, text, text_group, title, toggle, vertical_label,
};
// `ChromeRow` is one row of a widget's colour table; `Controls` is the front thread's table
// of live controls.
pub use state::{ChromeRow, Controls, Dragging, Front, Intent, What};
pub use text::{Flow, Shaped, TextSource, Written, reactive, shown};
