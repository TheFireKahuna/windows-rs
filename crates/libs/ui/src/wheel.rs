//! A valued control's own wheel source.
//!
//! The compositor gives a wheel to the nearest interaction source under the pointer that admits
//! it, and that source's tracker absorbs it: no window message carries it. A control that takes
//! the wheel ([`Element::on_wheel`](crate::build::Element::on_wheel)) therefore has a source of
//! its own — the vertical wheel alone, on a tracker that moves nothing on screen — and its
//! detents are read from where that tracker comes to rest. One path serves a control inside a
//! scroll container and one outside it alike.

use crate::build::Host;
use crate::build::tree::NONE;
use crate::input::Report;
use crate::seam::WheelOp;
use crate::widget::Front;
use windows_core::Result;
use windows_numerics::Vector2;
use windows_scene::{
    ControlId, GroupId, NodeId, Observed, Paint, Phase, SceneEvent, Source, TrackerId,
    TrackerRequest,
};

/// How far one `WHEEL_DELTA` of wheel moves a tracker per line of the system's lines-per-notch
/// setting, in physical pixels.
///
/// Measured on Windows 11 26200: the same at 96, 192 and 240 DPI and for every source size,
/// proportional to the delta and to the setting, and divided by the scale between the source's
/// visual and the screen.
const PX_PER_LINE: f32 = 94.90;

/// The lines a page-scroll setting moves per notch, by the same measure: 1898 physical pixels.
const PAGE_LINES: f32 = 20.0;

/// `SPI_GETWHEELSCROLLLINES`, and the value it answers for scrolling a page per notch.
const SPI_GETWHEELSCROLLLINES: u32 = 0x0068;
const WHEEL_PAGESCROLL: u32 = u32::MAX;

windows_core::link!("user32.dll" "system" fn SystemParametersInfoW(action: u32, param: u32, value: *mut core::ffi::c_void, flags: u32) -> i32);

/// Returns the system's lines per wheel notch, which is what the compositor scales a
/// redirected wheel by once a change to it has been broadcast.
fn lines_per_notch() -> f32 {
    let mut lines = 3u32;
    // SAFETY: SPI_GETWHEELSCROLLLINES writes one UINT through the pointer, a stack local.
    unsafe { SystemParametersInfoW(SPI_GETWHEELSCROLLLINES, 0, (&raw mut lines).cast(), 0) };
    match lines {
        WHEEL_PAGESCROLL => PAGE_LINES,
        lines => lines as f32,
    }
}

/// The range a wheel tracker rests inside, either side of zero, in DIPs. An idle tracker past
/// half of it is returned to zero, so its position keeps its precision however long the
/// control is wheeled.
const BOUND: f32 = 1.0e5;

/// One wheel control, as the app thread mounts it.
pub(crate) struct WheelRow {
    node: NodeId,
    control: ControlId,
    tracker: TrackerId<Observed>,
    /// Whether the tracker exists: it is created once the node has a solved box, because a
    /// source created on a visual with no size hit-tests nothing while reporting success.
    created: bool,
}

impl Host {
    /// Gives `control`, painted by `node`, a wheel source of its own. A node that has one keeps
    /// it.
    pub(crate) fn mount_wheel(&mut self, node: NodeId, control: ControlId) {
        if self.wheel_row(node) != NONE {
            return;
        }
        // The compositor routes a wheel by its hit test of painted content, so the control
        // takes the wheel over its whole box only where something paints the box: a derived
        // sprite spanning it, carrying the transparent paint.
        let fill = self.chrome_visual(GroupId(node), None);
        self.visual_insets(fill, [0.0; 4]);
        self.paint(fill, Paint::Clear, None);
        let tracker = self.tracker_id();
        let at = self.wheels.place(WheelRow {
            node,
            control,
            tracker,
            created: false,
        });
        self.set_wheel_row(node, at);
    }

    /// Creates the tracker of every wheel control whose node has been solved to a box.
    pub(crate) fn publish_wheels(&mut self) {
        let pending: Vec<u32> = self
            .wheels
            .iter()
            .filter(|(_, row)| !row.created)
            .map(|(at, _)| at)
            .collect();
        for at in pending {
            let Some(row) = self.wheels.get(at) else {
                continue;
            };
            let (node, control, tracker) = (row.node, row.control, row.tracker);
            let size = self.tree.c.geom[node.index()].size;
            if size.x <= 0.0 || size.y <= 0.0 {
                continue;
            }
            self.create_tracker(tracker, GroupId(node), Source::Wheel);
            self.tracker_bounds(
                tracker,
                Vector2 { x: 0.0, y: -BOUND },
                Vector2 { x: 0.0, y: BOUND },
            );
            self.wheel_ops.push(WheelOp::Add { tracker, control });
            if let Some(row) = self.wheels.get_mut(at) {
                row.created = true;
            }
        }
    }

    /// Retires the wheel source of the node whose side row held `at`.
    pub(crate) fn retire_wheel(&mut self, at: u32) {
        if let Some(row) = self.wheels.free(at) {
            if row.created {
                self.wheel_ops.push(WheelOp::Drop {
                    tracker: row.tracker,
                });
            }
            self.drop_tracker(row.tracker);
        }
    }
}

/// One wheel control, as the scene thread reads its tracker.
struct Live {
    tracker: TrackerId<Observed>,
    control: ControlId,
    /// Where the tracker last came to rest. A notch's travel is the distance from here to the
    /// next resting position, which stays exact while notches arrive faster than one settles.
    rest: f32,
}

/// The scene thread's wheel controls.
#[derive(Default)]
pub(crate) struct WheelTable {
    rows: Vec<Live>,
}

impl WheelTable {
    /// Takes the controls the app thread mounted and retired.
    pub(crate) fn apply(&mut self, ops: &mut Vec<WheelOp>) {
        for op in ops.drain(..) {
            match op {
                WheelOp::Add { tracker, control } => self.rows.push(Live {
                    tracker,
                    control,
                    rest: 0.0,
                }),
                WheelOp::Drop { tracker } => {
                    self.rows.retain(|row| row.tracker.id() != tracker.id());
                }
            }
        }
    }

    /// Turns this pass's wheel tracker reports into detents for their controls, appended to
    /// `out`, and returns an idle tracker far from zero to zero.
    ///
    /// `scale` is the window's DIP scale, which is the scale between a control's visual and the
    /// screen.
    ///
    /// # Errors
    ///
    /// The compositor refused a request.
    pub(crate) fn reports(
        &mut self,
        events: &[SceneEvent],
        scale: f32,
        out: &mut Vec<Report>,
        front: &mut Front<'_>,
    ) -> Result<()> {
        if self.rows.is_empty() {
            return Ok(());
        }
        for event in events {
            match *event {
                SceneEvent::InertiaBegan { tracker, rest, .. } => {
                    let Some(row) = self.rows.iter_mut().find(|row| row.tracker.id() == tracker.id())
                    else {
                        continue;
                    };
                    let travel = rest.y - row.rest;
                    row.rest = rest.y;
                    // The tracker's position grows as the wheel turns toward the user, where a
                    // detent is negative.
                    let notches = -travel * scale / (PX_PER_LINE * lines_per_notch());
                    if notches != 0.0 {
                        out.push(Report::Wheel {
                            target: row.control,
                            notches,
                        });
                    }
                }
                SceneEvent::TrackerPhase {
                    tracker,
                    phase: Phase::Idle,
                } => {
                    let Some(row) = self.rows.iter_mut().find(|row| row.tracker.id() == tracker.id())
                    else {
                        continue;
                    };
                    if row.rest.abs() > BOUND * 0.5 {
                        // An idle tracker applies a position request at once, so the next
                        // notch is measured from zero.
                        front
                            .scene
                            .request(row.tracker, TrackerRequest::To(Vector2::zero()))?;
                        row.rest = 0.0;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
