//! Stock retained recipes and the vocabulary shared by custom controls.
//!
//! Application components use the same direct writes, declaring their children through a borrowed
//! `Ui` context.

pub mod roles;

mod recipes;
mod state;
#[cfg(test)]
mod tests;
mod text;

pub use recipes::{
    Choice, ChoiceStyle, SliderStyle, TextStyle, box_, button, button_with, caption, card, code,
    control_text, display, edge_button, field, field_with, flyout, icon_button, knob, label, meter,
    micro, mono, note, panel, path, pills, segmented, segmented_with, select, sheet, slider,
    slider_source, styled_text, text, text_group, title, toggle, toggle_body, vertical_label,
};
pub use roles::{
    Chrome, Gesturing, Interaction, ModelState, Motion, Range, RoleSet, ScalarPart, ScalarValue,
    UiaRole, Wash, fraction_of, offset_of,
};
// `ChromeRow` and `ValueRow` are the two halves of a control the front thread reads; `Controls` is
// its table of live controls.
pub use state::{ChromeRow, Controls, Front, Intent, ValueRow, What, flag};
pub use text::{Flow, Shaped, TextAnnotation, TextSource, Written, reactive, shown};
