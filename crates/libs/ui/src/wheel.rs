//! A valued control's own wheel source.
//!
//! The compositor gives a wheel to the nearest interaction source under the pointer that admits
//! it, and that source's tracker absorbs it: no window message carries it. A control that takes
//! the wheel — the rotated wheel through [`Element::on_wheel`](crate::build::Element::on_wheel),
//! the tilted wheel as a horizontal slider — therefore has a source of its own, taking only its
//! axes on a tracker that moves nothing on screen, and its detents are read from where that
//! tracker comes to rest. One path serves a control inside a scroll container and one outside it
//! alike.

use crate::build::Host;
use crate::build::tree::NONE;
use crate::input::Report;
use crate::seam::WheelOp;
use crate::widget::{Front, WheelAxis};
use windows_core::Result;
use windows_numerics::Vector2;
use windows_scene::{
    Axes, ControlId, GroupId, NodeId, Observed, Paint, Phase, SceneEvent, Source, TrackerId,
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

/// How far one `WHEEL_DELTA` of tilted wheel moves a tracker per character of the system's
/// horizontal scroll setting, in physical pixels.
///
/// Measured on Windows 11 26200 as [`PX_PER_LINE`] is: the same at 96 and 192 DPI and for every
/// source size, proportional to the delta and to the setting, and independent of the
/// lines-per-notch setting.
const PX_PER_CHAR: f32 = 151.20;

const SPI_GETWHEELSCROLLLINES: u32 = 0x0068;
const SPI_GETWHEELSCROLLCHARS: u32 = 0x006C;
/// The value `SPI_GETWHEELSCROLLLINES` answers for scrolling a page per notch.
const WHEEL_PAGESCROLL: u32 = u32::MAX;

windows_core::link!("user32.dll" "system" fn SystemParametersInfoW(action: u32, param: u32, value: *mut core::ffi::c_void, flags: u32) -> i32);

/// Returns one of the system's wheel settings: a `UINT` the action writes.
fn setting(action: u32) -> u32 {
    let mut value = 3u32;
    // SAFETY: both wheel actions write one UINT through the pointer, a stack local.
    unsafe { SystemParametersInfoW(action, 0, (&raw mut value).cast(), 0) };
    value
}

/// Returns the physical pixels one detent on `axis` moves a tracker, which is what the
/// compositor scales a redirected wheel by once a change to its setting has been broadcast.
///
/// A setting of zero scrolls nothing, and the compositor moves no tracker for it.
pub(crate) fn px_per_notch(axis: WheelAxis) -> f32 {
    match axis {
        WheelAxis::Rotate => match setting(SPI_GETWHEELSCROLLLINES) {
            WHEEL_PAGESCROLL => PX_PER_LINE * PAGE_LINES,
            lines => PX_PER_LINE * lines as f32,
        },
        WheelAxis::Tilt => PX_PER_CHAR * setting(SPI_GETWHEELSCROLLCHARS) as f32,
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
    /// The wheel axes the control declared, which its source takes.
    axes: Axes,
    /// Whether the tracker exists: it is created once the node has a solved box, because a
    /// source created on a visual with no size hit-tests nothing while reporting success.
    created: bool,
}

impl Host {
    /// Gives `control`, painted by `node`, a wheel source of its own that takes the wheel on
    /// `axes`. A node that has one keeps it, and a source missing one of `axes` is replaced by
    /// one that takes it.
    pub(crate) fn mount_wheel(&mut self, node: NodeId, control: ControlId, axes: Axes) {
        let at = self.wheel_row(node);
        if at != NONE {
            let Some(had) = self.wheels.get(at).map(|row| row.axes) else {
                return;
            };
            let wanted = Axes {
                x: had.x || axes.x,
                y: had.y || axes.y,
                scale: false,
            };
            if wanted == had {
                return;
            }
            // A source's axes are fixed when it is created, so a widened one is a new source
            // on a new tracker.
            let fresh = self.tracker_id();
            let Some(row) = self.wheels.get_mut(at) else {
                return;
            };
            let (old, created) = (row.tracker, row.created);
            row.axes = wanted;
            row.tracker = fresh;
            row.created = false;
            if created {
                self.wheel_ops.push(WheelOp::Drop { tracker: old });
            }
            self.drop_tracker(old);
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
            axes,
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
            let (node, control, tracker, axes) = (row.node, row.control, row.tracker, row.axes);
            let size = self.tree.c.geom[node.index()].size;
            if size.x <= 0.0 || size.y <= 0.0 {
                continue;
            }
            self.create_tracker(tracker, GroupId(node), Source::Wheel(axes));
            let reach = Vector2 {
                x: if axes.x { BOUND } else { 0.0 },
                y: if axes.y { BOUND } else { 0.0 },
            };
            self.tracker_bounds(tracker, -reach, reach);
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
    rest: Vector2,
    /// The axis this burst of wheel input has moved, until the tracker is idle again.
    held: Option<WheelAxis>,
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
                    rest: Vector2::zero(),
                    held: None,
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
                    let travel = rest - row.rest;
                    row.rest = rest;
                    // The tracker's position grows as the wheel turns toward the user, where a
                    // detent is negative, and as it tilts right, where a detent is positive.
                    let rotate = -travel.y * scale / px_per_notch(WheelAxis::Rotate);
                    let tilt = travel.x * scale / px_per_notch(WheelAxis::Tilt);
                    let (axis, notches) = match (rotate.is_normal(), tilt.is_normal()) {
                        (false, false) => continue,
                        (true, false) => (WheelAxis::Rotate, rotate),
                        (false, true) => (WheelAxis::Tilt, tilt),
                        (true, true) if rotate.abs() >= tilt.abs() => (WheelAxis::Rotate, rotate),
                        (true, true) => (WheelAxis::Tilt, tilt),
                    };
                    // A touchpad that synthesizes the wheel sends both axes for one diagonal
                    // swipe, so a burst moves the axis it began on.
                    if *row.held.get_or_insert(axis) != axis {
                        continue;
                    }
                    out.push(Report::Wheel {
                        target: row.control,
                        axis,
                        notches,
                    });
                }
                SceneEvent::TrackerPhase {
                    tracker,
                    phase: Phase::Idle,
                } => {
                    let Some(row) = self.rows.iter_mut().find(|row| row.tracker.id() == tracker.id())
                    else {
                        continue;
                    };
                    row.held = None;
                    if row.rest.x.abs().max(row.rest.y.abs()) > BOUND * 0.5 {
                        // An idle tracker applies a position request at once, so the next
                        // notch is measured from zero.
                        front
                            .scene
                            .request(row.tracker, TrackerRequest::To(Vector2::zero()))?;
                        row.rest = Vector2::zero();
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
