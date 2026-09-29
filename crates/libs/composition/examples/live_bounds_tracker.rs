use windows_composition::*;
use windows_numerics::{Vector2, Vector3};
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    let _queue = DispatcherQueueController::create_on_current_thread()?;
    let window = windows_window::Window::new("Composition live-bounds probe")
        .size(420, 180).create()?;
    let compositor = Compositor::new()?;
    let start = Instant::now();
    let target = compositor.create_desktop_window_target(&window, false)?;
    let root = compositor.create_container_visual();
    target.set_root(&root);
    let visual = compositor.create_sprite_visual();
    visual.set_size(100.0, 100.0);
    visual.set_brush(&compositor.create_color_brush(Color::rgb(0, 160, 200)));
    root.children().insert_at_top(&visual);
    println!("ms,event,width,height,offset_x");
    let tracker = compositor.create_interaction_tracker_with_owner(move |event| {
        match event {
            TrackerEvent::ValuesChanged { position, .. } => println!("{},value,{},{},{}", start.elapsed().as_millis(), position.x, position.y, position.z),
            other => eprintln!("{} {other:?}", start.elapsed().as_millis()),
        }
    })?;
    tracker.set_position_bounds(Vector3::new(-10000.0, -10000.0, -10000.0), Vector3::new(10000.0, 10000.0, 10000.0));
    let observe = compositor.create_expression_animation("Vector3(source.Size.X, source.Size.Y, source.Offset.X)");
    observe.set_reference_parameter("source", &**visual);
    tracker.try_update_position_with_animation(&observe)?;
    let observed = compositor.create_property_set();
    observed.insert_vector2("Observed", Vector2::new(-1.0, -1.0));
    let expression = compositor.create_expression_animation("source.Size");
    expression.set_reference_parameter("source", &**visual);
    observed.start_animation("Observed", &expression);
    let follower = compositor.create_sprite_visual();
    follower.set_brush(&compositor.create_color_brush(Color::rgb(255, 180, 0)));
    follower.set_offset(0.0, 110.0, 0.0);
    let follow = compositor.create_expression_animation("Vector2(source.Observed.X, 8)");
    follow.set_reference_parameter("source", &observed);
    follower.start_animation("Size", &follow);
    root.children().insert_at_top(&follower);
    drop(compositor.request_commit()?);
    window.show();
    let spring = compositor.create_spring_vector2_animation();
    spring.set_period(Duration::from_millis(650));
    spring.set_damping_ratio(0.5);
    spring.set_final_value(Vector2::new(300.0, 100.0));
    let mut started = false;
    let mut reversed = false;


    while start.elapsed() < Duration::from_secs(8) {
        windows_window::pump();
        let ms = start.elapsed().as_millis();
        if ms >= 2500 && !started {
            visual.start_animation("Size", &spring);
            drop(compositor.request_commit()?);
            started = true;
        }
        if ms >= 3500 && !reversed {
            spring.set_final_value(Vector2::new(180.0, 100.0));
            visual.start_animation("Size", &spring);
            drop(compositor.request_commit()?);
            reversed = true;
        }

        std::thread::sleep(Duration::from_millis(2));
    }
    Ok(())
}



