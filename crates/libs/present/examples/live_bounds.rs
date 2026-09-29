use std::sync::{Arc, Mutex, mpsc::channel};
use std::time::{Duration, Instant};
use windows_color::{DisplayCapability, OutputTransform, Radiance};
use windows_composition::{BatchKind, Clamping, Color, Compositor, ScaleAnimationPolicy, Stretch, TrackerEvent, Vector2, Vector3};
use windows_d2d::Solid;
use windows_present::{Bound, Draw, DrawCtx, Epoch, Extent, Frame, GateCtx, Gpu, Pass, Presenter, Queue, Rect, RegionInput, RegionKey, RegionSpec, Result, Tuning};

#[derive(Clone, Copy)]
struct Sample { seq: u64, at: Instant, width: f32, height: f32, offset: f32 }

fn main() -> Result<()> {
    let window = windows_window::Window::new("Native bounds to presentation probe").size(600, 360).create()?;
    let compositor = Compositor::new()?;
    let target = compositor.create_desktop_window_target(&window, false)?;
    let root = compositor.create_container_visual();
    target.set_root(&root);
    let native = compositor.create_sprite_visual();
    native.set_size(100.0, 60.0);
    native.set_brush(&compositor.create_color_brush(Color::rgb(0, 160, 200)));
    root.children().insert_at_top(&native);
    let surface_visual = compositor.create_sprite_visual();
    surface_visual.set_offset(0.0, 160.0, 0.0);
    surface_visual.set_size(560.0, 140.0);
    let native_translation = compositor.create_expression_animation("Vector3(source.Offset.X, 160, 0)");
    native_translation.set_reference_parameter("source", &**native);
    surface_visual.start_animation("Offset", &native_translation);
    root.children().insert_at_top(&surface_visual);

    let origin = Instant::now();
    let sample = Arc::new(Mutex::new(Sample { seq: 0, at: origin, width: 100.0, height: 60.0, offset: 0.0 }));
    let epoch = Arc::new(Epoch::new()?);
    let (tx, rx) = channel();
    let presenter = Presenter::spawn(Tuning { depth: 1, statistics: false, ..Tuning::default() }, OutputTransform::for_display(DisplayCapability::Sdr, 300.0), Some(window.watch()?), Box::new(move |_, bound| { let _ = tx.send(bound); }))?;
    let frame_sample = sample.clone();
    let stall_ms = std::env::var("PROBE_STALL_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    presenter.mount(RegionSpec { key: RegionKey(1), queue: Queue::Solo, extent: Extent::new(560.0, 140.0, 96.0) }, epoch.clone(), Arc::new(RegionInput::new()), move |_: &Gpu| Ok(Box::new(Probe { sample: frame_sample, current: Sample { seq: u64::MAX, at: origin, width: 100.0, height: 60.0, offset: 0.0 }, ink: None, origin, stall_ms }) as Box<dyn Frame>));
    let Bound::Surface { handle, .. } = rx.recv_timeout(Duration::from_secs(5)).expect("surface binding") else { panic!("surface failed"); };
    // SAFETY: the mounted region owns this handle; presenter outlives the attached visual.
    let surface = unsafe { compositor.create_surface_for_handle(handle as *mut _)? };
    let brush = compositor.create_surface_brush(&surface);
    brush.set_stretch(Stretch::None);
    brush.set_alignment_ratio(0.0, 0.0);
    brush.set_nearest_sampling();
    surface_visual.set_brush(&brush);

    println!("ms,event,seq,width,height,offset,age_ms");
    let tracker = compositor.create_interaction_tracker_with_owner(move |event| {
        match event {
            TrackerEvent::ValuesChanged { position, .. } => {
                let mut value = sample.lock().unwrap();
                *value = Sample { seq: value.seq + 1, at: Instant::now(), width: position.x, height: position.y, offset: position.z };
                println!("{},callback,{},{},{},{},0", origin.elapsed().as_millis(), value.seq, value.width, value.height, value.offset);
                drop(value);
                epoch.invalidate();
            }
            other => eprintln!("{} {other:?}", origin.elapsed().as_millis()),
        }
    })?;
    tracker.set_position_bounds(Vector3::new(-10000.0, -10000.0, -10000.0), Vector3::new(10000.0, 10000.0, 10000.0));
    let observe = compositor.create_expression_animation("Vector3(source.Size.X, source.Size.Y, 0)");
    observe.set_reference_parameter("source", &**native);
    tracker.try_update_position_with_animation(&observe)?;
    drop(compositor.request_commit()?);
    window.show();

    let spring = compositor.create_spring_vector2_animation();
    spring.set_period(Duration::from_millis(120));
    spring.set_damping_ratio(0.7);
    spring.set_final_value(Vector2::new(400.0, 100.0));
    let position = compositor.create_spring_scalar_animation();
    position.set_period(Duration::from_millis(180));
    position.set_damping_ratio(0.8);
    position.set_final_value(70.0);
    let mut step = 0;
    let mut completions = Vec::new();
    while origin.elapsed() < Duration::from_secs(12) && window.is_open() {
        windows_window::pump();
        let ms = origin.elapsed().as_millis();
        if (step == 0 && ms >= 2200) || (step == 1 && ms >= 2350) || (step == 2 && ms >= 2500) {
            let batch = compositor.create_scoped_batch(BatchKind::Animation);
            match step {
                0 => native.start_animation("Size", &spring),
                1 => native.start_animation("Offset.X", &position),
                _ => {
                    spring.set_final_value(Vector2::new(180.0, 60.0));
                    native.start_animation("Size", &spring);
                }
            }
            let token = step;
            let observer = tracker.clone();
            completions.push(batch.on_completed(move || {
                eprintln!("{} completed {token}", origin.elapsed().as_millis());
                if token == 2 {
                    eprintln!("stop observer: {:?}", observer.try_update_position(Vector3::new(180.0, 60.0, 0.0), Clamping::Auto, ScaleAnimationPolicy::Stop));
                }
            })?);
            batch.try_end()?;
            drop(compositor.request_commit()?);
            step += 1;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    eprintln!("{} finished", origin.elapsed().as_millis());
    Ok(())
}

struct Probe { sample: Arc<Mutex<Sample>>, current: Sample, ink: Option<Solid>, origin: Instant, stall_ms: u64 }

impl Frame for Probe {
    fn should_draw(&mut self, _: GateCtx<'_>) -> bool {
        let next = *self.sample.lock().unwrap();
        if self.current.seq == next.seq { return false; }
        self.current = next;
        true
    }
    fn prepare(&mut self, ctx: GateCtx<'_>, _: &mut Pass<'_>) -> Result<()> {
        if self.stall_ms > 0 && self.origin.elapsed() > Duration::from_millis(2450) {
            let ms = std::mem::take(&mut self.stall_ms);
            eprintln!("{} render stall {ms} ms", self.origin.elapsed().as_millis());
            std::thread::sleep(Duration::from_millis(ms));
        }
        if self.ink.is_none() { self.ink = Some(ctx.device.solid(ctx.out.apply(Radiance::new(0.0, 0.0, 0.0, 1.0)))?); }
        Ok(())
    }
    fn draw(&mut self, ctx: DrawCtx<'_>, draw: &Draw<'_>) {
        let s = self.current;
        println!("{},draw,{},{},{},{},{}", self.origin.elapsed().as_millis(), s.seq, s.width, s.height, s.offset, s.at.elapsed().as_secs_f64() * 1000.0);
        draw.clear(ctx.out.apply(Radiance::new(0.0, 0.0, 0.0, 1.0)));
        let ink = self.ink.as_ref().unwrap();
        ink.set(ctx.out.apply(Radiance::new(100.0, 25.0, 70.0, 1.0)));
        draw.fill(Rect::new(s.offset, 0.0, s.offset + s.width, s.height), ink);
        ink.set(ctx.out.apply(Radiance::new(180.0, 180.0, 180.0, 1.0)));
        for fraction in [0.25, 0.5, 0.75] {
            let x = (s.offset + s.width * fraction).round();
            draw.fill(Rect::new(x, 0.0, x + 1.0, s.height), ink);
        }
    }
    fn opaque(&self) -> bool { true }
    fn device_reset(&mut self) { self.ink = None; }
}
