use windows_composition::*;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    let _queue = DispatcherQueueController::create_on_current_thread()?;
    let compositor = Compositor::new()?;
    let visual = compositor.create_sprite_visual();
    visual.set_size(100.0, 100.0);
    let tracker = compositor.create_interaction_tracker_with_owner(|event| {
        if let TrackerEvent::ValuesChanged { position, scale, request, .. } = event {
            println!("{},{},{},{},{}", request.0, position.x, position.y, position.z, scale);
        }
    })?;
    tracker.set_position_bounds(Vector3::new(-10000.0,-10000.0,-10000.0), Vector3::new(10000.0,10000.0,10000.0));
    tracker.set_scale_bounds(0.001,10000.0);
    let position = compositor.create_expression_animation("Vector3(source.Offset.X, source.Offset.Y, source.Size.X)");
    position.set_reference_parameter("source", &**visual);
    let height = compositor.create_expression_animation("source.Size.Y + 1");
    height.set_reference_parameter("source", &**visual);
    eprintln!("position {:?}",tracker.try_update_position_with_animation(&position)?);
    eprintln!("scale {:?}",tracker.try_update_scale_with_animation(&height, Vector3::new(0.0,0.0,0.0))?);
    let size = compositor.create_spring_vector2_animation();
    size.set_period(Duration::from_millis(500));
    size.set_damping_ratio(0.8);
    size.set_final_value(Vector2::new(300.0,300.0));
    visual.start_animation("Size",&size);
    drop(compositor.request_commit()?);
    let at=Instant::now();
    while at.elapsed()<Duration::from_secs(3) {
        windows_window::pump();
        std::thread::sleep(Duration::from_millis(2));
    }
    tracker.try_update_position(Vector3::new(0.0,0.0,300.0),Clamping::Disabled,ScaleAnimationPolicy::Stop)?;
    Ok(())
}
