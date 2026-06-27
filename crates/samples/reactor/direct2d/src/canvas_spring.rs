//! Demo of the windows-canvas `surface_painter` for an *interactive* control: a
//! slider you drag, which springs to the nearest detent when you let go.
//!
//! It shows the two halves of a custom-drawn control on a `SurfaceImageSource`:
//!  - **value tracking during a drag** — a capture-capable `PointerSurface`
//!    (opened from the painter's mount hook) follows the pointer past the element
//!    bounds, calling `invalidate()` per move for a coalesced, vsync-aligned redraw;
//!  - **a settling animation on release** — `animate()` steps a spring each frame
//!    and stops itself when settled, with the painter idle (no frame subscription,
//!    no work) the rest of the time.

use windows_canvas::{ColorF, Ellipse, PumpHold, Rect, RoundedRect, Step, Vector2};
use windows_reactor::*;

/// A small critically-damped-style spring (FabFilter-style post-release settle).
#[derive(Clone, Copy)]
struct Spring {
    value: f32,
    velocity: f32,
    target: f32,
}

impl Spring {
    fn new(value: f32) -> Self {
        Self {
            value,
            velocity: 0.0,
            target: value,
        }
    }

    fn step(&mut self, dt: f32) {
        const STIFFNESS: f32 = 140.0;
        const DAMPING: f32 = 22.0;
        let dt = dt.min(1.0 / 30.0);
        let accel = STIFFNESS * (self.target - self.value) - DAMPING * self.velocity;
        self.velocity += accel * dt;
        self.value += self.velocity * dt;
    }

    fn settled(&self) -> bool {
        (self.target - self.value).abs() < 0.001 && self.velocity.abs() < 0.01
    }
}

const PAD: f32 = 24.0;
const DETENTS: [f32; 3] = [0.0, 0.5, 1.0];

/// Map a pointer x (element-relative DIPs) to a `0.0..=1.0` track value.
fn value_at(x: f64, width: f32) -> f32 {
    let track_w = (width - 2.0 * PAD).max(1.0);
    ((x as f32 - PAD) / track_w).clamp(0.0, 1.0)
}

/// Sample page: a draggable slider drawn with `surface_painter` that springs to
/// the nearest detent on release.
pub fn canvas_spring_sample(_: &(), cx: &mut RenderCx) -> Element {
    let spring = cx.use_ref(Spring::new(0.5));
    let dragging = cx.use_ref(false);
    // Kept alive so the pointer subscriptions / the in-flight drag hold survive.
    let pointer = cx.use_ref::<Option<PointerSurface>>(None);
    let hold = cx.use_ref::<Option<PumpHold>>(None);

    // The draw fills the whole surface every frame, so skip the auto-clear to
    // avoid a redundant second Clear per repaint.
    let painter = windows_canvas::surface_painter(cx).clear_color(None).draw({
        let spring = spring.clone();
        move |ctx| {
            ctx.clear(ColorF::rgb(0.10, 0.10, 0.12));
            let value = spring.borrow().value.clamp(0.0, 1.0);

            let track_h = 10.0;
            let track_y = ctx.height / 2.0 - track_h / 2.0;
            let track_w = (ctx.width - 2.0 * PAD).max(1.0);
            let radius = track_h / 2.0;

            if let Ok(track) = ctx.create_solid_brush(ColorF::rgb(0.25, 0.25, 0.30)) {
                let r = Rect::from_xywh(PAD, track_y, track_w, track_h);
                ctx.fill_rounded_rect(&RoundedRect::new(r, radius, radius), &track);
            }

            let thumb_x = PAD + value * track_w;
            if let Ok(fill) = ctx.create_solid_brush(ColorF::rgb(0.30, 0.66, 1.0)) {
                let r = Rect::from_xywh(PAD, track_y, (thumb_x - PAD).max(0.0), track_h);
                ctx.fill_rounded_rect(&RoundedRect::new(r, radius, radius), &fill);
                let center = Vector2::new(thumb_x, ctx.height / 2.0);
                ctx.fill_ellipse(&Ellipse::new(center, 14.0, 14.0), &fill);
            }
        }
    });

    // Open a capture-capable pointer surface from the mounted Image and wire the
    // drag to the painter. Size tracking is still handled by the painter itself.
    painter.on_mounted({
        let painter = painter.clone();
        move |handle| {
            let Ok(surface) = handle.pointer_surface() else {
                return;
            };

            // Press: capture the pointer, keep the pump warm, jump to the value.
            let _ = surface.on_down_capture({
                let painter = painter.clone();
                let spring = spring.clone();
                let dragging = dragging.clone();
                let hold = hold.clone();
                move |e| {
                    dragging.set(true);
                    hold.set(Some(painter.hold()));
                    let v = value_at(e.x, painter.size().0);
                    let mut s = spring.borrow_mut();
                    s.value = v;
                    s.target = v;
                    s.velocity = 0.0;
                    drop(s);
                    painter.invalidate();
                }
            });

            // Drag: track the pointer with a coalesced redraw per frame.
            let _ = surface.on_move({
                let painter = painter.clone();
                let spring = spring.clone();
                let dragging = dragging.clone();
                move |e| {
                    if !dragging.get_cloned() {
                        return;
                    }
                    let v = value_at(e.x, painter.size().0);
                    {
                        let mut s = spring.borrow_mut();
                        s.value = v;
                        s.target = v;
                    }
                    painter.invalidate();
                }
            });

            // Release: drop the hold and spring to the nearest detent.
            let _ = surface.on_up({
                let painter = painter.clone();
                let spring = spring.clone();
                let dragging = dragging.clone();
                let hold = hold.clone();
                move |_e| {
                    dragging.set(false);
                    hold.set(None);
                    let value = spring.borrow().value;
                    let target = DETENTS
                        .into_iter()
                        .min_by(|a, b| (a - value).abs().total_cmp(&(b - value).abs()))
                        .unwrap_or(value);
                    spring.borrow_mut().target = target;
                    painter.animate({
                        let spring = spring.clone();
                        move |t| {
                            let mut s = spring.borrow_mut();
                            s.step(t.delta.as_secs_f32());
                            if s.settled() {
                                Step::Done
                            } else {
                                Step::Redraw
                            }
                        }
                    });
                }
            });

            *pointer.borrow_mut() = Some(surface);
        }
    });

    grid((
        Element::from(
            text_block("surface_painter: drag the slider — it springs to a detent on release")
                .grid_row(0),
        ),
        Element::from(
            border(painter.element())
                .border_thickness(Thickness::uniform(1.0))
                .margin(Thickness {
                    left: 0.0,
                    top: 8.0,
                    right: 0.0,
                    bottom: 0.0,
                })
                .grid_row(1),
        ),
    ))
    .rows([GridLength::Auto, GridLength::STAR])
    .columns([GridLength::STAR])
    .margin(Thickness::uniform(16.0))
    .into()
}
