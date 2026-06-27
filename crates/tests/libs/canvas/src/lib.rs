//! Integration tests for windows-canvas.
//!
//! These tests create real D3D11/D2D devices and verify the core API surface.
//! They require a GPU (hardware or WARP) to run.

#[cfg(test)]
mod tests {
    use windows_canvas::*;

    fn ensure_com_initialized() {
        unsafe {
            windows_core::link!("combase.dll" "system" fn CoIncrementMTAUsage(pcookie: *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
            let mut cookie = core::ptr::null_mut();
            let _ = CoIncrementMTAUsage(&mut cookie);
        }
    }

    #[test]
    fn create_device() {
        let device = GpuDevice::new_warp().expect("Failed to create WARP device");
        let _d3d = device.d3d_device();
        let _d2d = device.d2d_device();
        let _factory = device.dxgi_factory();
    }

    #[test]
    fn create_swap_chain() {
        let device = GpuDevice::new_warp().unwrap();
        let chain = device.create_swap_chain(64, 64).unwrap();
        assert_eq!(chain.width(), 64);
        assert_eq!(chain.height(), 64);
    }

    #[test]
    fn draw_and_present() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();

        {
            let session = chain.begin_draw().unwrap();
            session.clear(ColorF::CORNFLOWER_BLUE);
        }

        chain.present().unwrap();
    }

    #[test]
    fn resize_swap_chain() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();
        chain.resize(128, 128).unwrap();
        assert_eq!(chain.width(), 128);
        assert_eq!(chain.height(), 128);
    }

    #[test]
    fn brush_reuse_across_frames() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();

        let brush = chain.create_solid_brush(ColorF::RED).unwrap();

        for _ in 0..3 {
            let session = chain.begin_draw().unwrap();
            session.clear(ColorF::BLACK);
            session.fill_ellipse(&Ellipse::circle(Vector2::new(32.0, 32.0), 16.0), &brush);
            drop(session);
            chain.present().unwrap();
        }
    }

    #[test]
    fn text_format_and_draw_text() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(128, 64).unwrap();

        let format = TextFormat::new("Segoe UI", 16.0)
            .unwrap()
            .with_alignment(TextAlignment::Center);
        let brush = chain.create_solid_brush(ColorF::WHITE).unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        session.draw_text("Hello", &format, &Rect::new(0.0, 0.0, 128.0, 64.0), &brush);
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn path_builder_typestate() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();

        // Build a triangle path.
        let path = PathBuilder::new(&device)
            .unwrap()
            .begin(Vector2::new(32.0, 0.0))
            .line_to(Vector2::new(64.0, 64.0))
            .line_to(Vector2::new(0.0, 64.0))
            .close()
            .build()
            .unwrap();

        let brush = chain.create_solid_brush(ColorF::GREEN).unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        session.fill_path(&path, &brush);
        session.draw_path(&path, &brush, 2.0);
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn present_reports_no_device_lost() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        drop(session);

        // Normal present should return Ok(true) — device is fine.
        let result = chain.present().unwrap();
        assert!(result, "Expected present to succeed (no device lost)");
        assert!(!chain.is_device_lost());
    }

    #[test]
    fn rounded_rect_draw_fill() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();
        let brush = chain.create_solid_brush(ColorF::RED).unwrap();

        let rrect = RoundedRect::uniform(Rect::new(5.0, 5.0, 55.0, 55.0), 8.0);

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        session.fill_rounded_rect(&rrect, &brush);
        session.draw_rounded_rect(&rrect, &brush, 2.0);
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn transform_set_get_restore() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);

        // Default is identity.
        let identity = session.get_transform();
        assert_eq!(identity.m11, 1.0);
        assert_eq!(identity.m22, 1.0);
        assert_eq!(identity.m31, 0.0);

        // Set a translation.
        let translated = Matrix3x2 {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            m31: 10.0,
            m32: 20.0,
        };
        session.set_transform(&translated);
        let got = session.get_transform();
        assert_eq!(got.m31, 10.0);
        assert_eq!(got.m32, 20.0);

        // with_transform restores original.
        let scaled = Matrix3x2 {
            m11: 2.0,
            m12: 0.0,
            m21: 0.0,
            m22: 2.0,
            m31: 0.0,
            m32: 0.0,
        };
        session.with_transform(&scaled, || {
            let inside = session.get_transform();
            assert_eq!(inside.m11, 2.0);
        });
        let after = session.get_transform();
        assert_eq!(after.m31, 10.0); // restored to translated

        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn linear_gradient_brush() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);

        let gradient = session
            .create_linear_gradient(
                Vector2::new(0.0, 0.0),
                Vector2::new(64.0, 0.0),
                &[
                    GradientStop::new(0.0, ColorF::RED),
                    GradientStop::new(1.0, ColorF::BLUE),
                ],
            )
            .unwrap();

        // Use gradient with various draw methods (same as solid brush).
        session.fill_rect(&Rect::new(0.0, 0.0, 64.0, 32.0), &gradient);
        session.fill_ellipse(&Ellipse::circle(Vector2::new(32.0, 48.0), 12.0), &gradient);

        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn radial_gradient_brush() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);

        let gradient = session
            .create_radial_gradient(
                Vector2::new(32.0, 32.0),
                32.0,
                32.0,
                &[
                    GradientStop::new(0.0, ColorF::WHITE),
                    GradientStop::new(1.0, ColorF::BLACK),
                ],
            )
            .unwrap();

        session.fill_rect(&Rect::new(0.0, 0.0, 64.0, 64.0), &gradient);

        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn load_and_draw_bitmap() {
        ensure_com_initialized();
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();

        let bitmap = chain
            .load_bitmap(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test.png"))
            .unwrap();

        assert!(bitmap.width() > 0.0);
        assert!(bitmap.height() > 0.0);

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        session.draw_bitmap(&bitmap, &Rect::new(0.0, 0.0, 64.0, 64.0), 1.0);
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn load_bitmap_nonexistent_file_returns_err() {
        ensure_com_initialized();
        let device = GpuDevice::new_warp().unwrap();
        let chain = device.create_swap_chain(64, 64).unwrap();

        let result = chain.load_bitmap("nonexistent_file_that_does_not_exist.png");
        assert!(result.is_err());
    }

    #[test]
    fn resize_zero_is_noop() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();
        chain.resize(0, 0).unwrap();
        assert_eq!(chain.width(), 64);
        assert_eq!(chain.height(), 64);
    }

    #[test]
    fn path_builder_begin_hollow_and_end_open() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();

        // Build a hollow open path (stroke-only, not closed).
        let path = PathBuilder::new(&device)
            .unwrap()
            .begin_hollow(Vector2::new(0.0, 32.0))
            .line_to(Vector2::new(32.0, 0.0))
            .line_to(Vector2::new(64.0, 32.0))
            .end_open()
            .build()
            .unwrap();

        let brush = chain.create_solid_brush(ColorF::WHITE).unwrap();
        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        session.draw_path(&path, &brush, 2.0);
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn path_builder_multiple_figures() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();

        // Two separate closed triangles in one path.
        let path = PathBuilder::new(&device)
            .unwrap()
            .begin(Vector2::new(0.0, 0.0))
            .line_to(Vector2::new(30.0, 0.0))
            .line_to(Vector2::new(15.0, 30.0))
            .close()
            .begin(Vector2::new(34.0, 34.0))
            .line_to(Vector2::new(64.0, 34.0))
            .line_to(Vector2::new(49.0, 64.0))
            .close()
            .build()
            .unwrap();

        let brush = chain.create_solid_brush(ColorF::GREEN).unwrap();
        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        session.fill_path(&path, &brush);
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn text_format_new_bold() {
        let format = TextFormat::new_bold("Segoe UI", 20.0).unwrap();

        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(128, 64).unwrap();
        let brush = chain.create_solid_brush(ColorF::WHITE).unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        session.draw_text("Bold", &format, &Rect::new(0.0, 0.0, 128.0, 64.0), &brush);
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn radial_gradient_with_fill_ellipse() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);

        let gradient = session
            .create_radial_gradient(
                Vector2::new(32.0, 32.0),
                32.0,
                32.0,
                &[
                    GradientStop::new(0.0, ColorF::WHITE),
                    GradientStop::new(1.0, ColorF::BLACK),
                ],
            )
            .unwrap();

        // Use radial gradient with fill_ellipse (not just fill_rect).
        session.fill_ellipse(&Ellipse::circle(Vector2::new(32.0, 32.0), 30.0), &gradient);

        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn draw_line_and_styled() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();
        let brush = chain.create_solid_brush(ColorF::WHITE).unwrap();

        let style = device
            .create_stroke_style(
                &StrokeStyleBuilder::new()
                    .caps(CapStyle::Round)
                    .line_join(LineJoin::Round),
            )
            .unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        session.draw_line(
            Vector2::new(5.0, 5.0),
            Vector2::new(55.0, 55.0),
            &brush,
            2.0,
        );
        session.draw_line_styled(
            Vector2::new(5.0, 55.0),
            Vector2::new(55.0, 5.0),
            &brush,
            3.0,
            &style,
        );
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn draw_rect_and_styled() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();
        let brush = chain.create_solid_brush(ColorF::RED).unwrap();

        let style = device
            .create_stroke_style(
                &StrokeStyleBuilder::new()
                    .dash_style(DashStyle::Dash)
                    .dash_cap(CapStyle::Square),
            )
            .unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        session.draw_rect(&Rect::new(5.0, 5.0, 30.0, 30.0), &brush, 1.0);
        session.draw_rect_styled(&Rect::new(34.0, 34.0, 59.0, 59.0), &brush, 2.0, &style);
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn draw_ellipse_and_styled() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();
        let brush = chain.create_solid_brush(ColorF::GREEN).unwrap();

        let style = device
            .create_stroke_style(
                &StrokeStyleBuilder::new()
                    .start_cap(CapStyle::Triangle)
                    .end_cap(CapStyle::Flat),
            )
            .unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        session.draw_ellipse(
            &Ellipse::new(Vector2::new(32.0, 32.0), 25.0, 15.0),
            &brush,
            1.0,
        );
        session.draw_ellipse_styled(
            &Ellipse::circle(Vector2::new(32.0, 32.0), 20.0),
            &brush,
            2.0,
            &style,
        );
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn draw_rounded_rect_styled() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();
        let brush = chain.create_solid_brush(ColorF::WHITE).unwrap();

        let style = device
            .create_stroke_style(
                &StrokeStyleBuilder::new()
                    .dash_style(DashStyle::DashDot)
                    .miter_limit(5.0),
            )
            .unwrap();

        let rrect = RoundedRect::new(Rect::new(5.0, 5.0, 55.0, 55.0), 10.0, 5.0);

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        session.draw_rounded_rect_styled(&rrect, &brush, 2.0, &style);
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn draw_path_styled() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();
        let brush = chain.create_solid_brush(ColorF::WHITE).unwrap();

        let style = device
            .create_stroke_style(
                &StrokeStyleBuilder::new()
                    .caps(CapStyle::Round)
                    .dash_style(DashStyle::Dot)
                    .dash_offset(0.5),
            )
            .unwrap();

        let path = PathBuilder::new(&device)
            .unwrap()
            .begin(Vector2::new(10.0, 10.0))
            .line_to(Vector2::new(54.0, 10.0))
            .line_to(Vector2::new(54.0, 54.0))
            .close()
            .build()
            .unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        session.draw_path_styled(&path, &brush, 2.0, &style);
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn stroke_style_builder_all_setters() {
        let device = GpuDevice::new_warp().unwrap();

        let style = device
            .create_stroke_style(
                &StrokeStyleBuilder::new()
                    .start_cap(CapStyle::Round)
                    .end_cap(CapStyle::Square)
                    .dash_cap(CapStyle::Triangle)
                    .line_join(LineJoin::Bevel)
                    .miter_limit(4.0)
                    .dash_style(DashStyle::DashDot)
                    .dash_offset(1.5),
            )
            .unwrap();

        // StrokeStyle implements Clone.
        let _clone = style.clone();
        drop(style);
    }

    #[test]
    fn bitmap_target_shadow_effect() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);

        let target = session.create_bitmap_target().unwrap();
        session.with_target(&target, || {
            let Ok(brush) = session.create_solid_brush(ColorF::WHITE) else {
                return;
            };
            session.clear(ColorF::TRANSPARENT);
            session.fill_ellipse(&Ellipse::circle(Vector2::new(32.0, 32.0), 10.0), &brush);
        });

        let shadow = session.create_shadow(&target).unwrap();
        session.draw_effect(&shadow);
        session.draw_image(&target);

        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn session_load_bitmap() {
        ensure_com_initialized();
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);

        let bitmap = session
            .load_bitmap(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test.png"))
            .unwrap();

        assert!(bitmap.width() > 0.0);
        assert!(bitmap.height() > 0.0);

        session.draw_bitmap(&bitmap, &Rect::new(0.0, 0.0, 64.0, 64.0), 1.0);

        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn rect_from_xywh_and_accessors() {
        let rect = Rect::from_xywh(10.0, 20.0, 30.0, 40.0);
        assert_eq!(rect.left, 10.0);
        assert_eq!(rect.top, 20.0);
        assert_eq!(rect.right, 40.0);
        assert_eq!(rect.bottom, 60.0);
        assert_eq!(rect.width(), 30.0);
        assert_eq!(rect.height(), 40.0);
    }

    #[test]
    fn ellipse_new() {
        let e = Ellipse::new(Vector2::new(10.0, 20.0), 30.0, 40.0);
        assert_eq!(e.center.x, 10.0);
        assert_eq!(e.center.y, 20.0);
        assert_eq!(e.radius_x, 30.0);
        assert_eq!(e.radius_y, 40.0);
    }

    #[test]
    fn rounded_rect_new() {
        let rr = RoundedRect::new(Rect::new(0.0, 0.0, 100.0, 50.0), 10.0, 20.0);
        assert_eq!(rr.radius_x, 10.0);
        assert_eq!(rr.radius_y, 20.0);
        assert_eq!(rr.rect.width(), 100.0);
    }

    #[test]
    fn text_format_with_weight_and_paragraph_alignment() {
        let format = TextFormat::with_weight("Segoe UI", 18.0, FontWeight::BOLD)
            .unwrap()
            .with_paragraph_alignment(ParagraphAlignment::Center);

        let _raw = format.raw();

        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(128, 64).unwrap();
        let brush = chain.create_solid_brush(ColorF::WHITE).unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        session.draw_text(
            "Centered",
            &format,
            &Rect::new(0.0, 0.0, 128.0, 64.0),
            &brush,
        );
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn path_bezier_to() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();
        let brush = chain.create_solid_brush(ColorF::WHITE).unwrap();

        let path = PathBuilder::new(&device)
            .unwrap()
            .begin(Vector2::new(0.0, 32.0))
            .bezier_to(
                Vector2::new(10.0, 0.0),
                Vector2::new(54.0, 0.0),
                Vector2::new(64.0, 32.0),
            )
            .close()
            .build()
            .unwrap();

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        session.fill_path(&path, &brush);
        session.draw_path(&path, &brush, 1.0);
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn color_constructors() {
        let c1 = ColorF::new(0.5, 0.6, 0.7, 0.8);
        assert_eq!(c1.r, 0.5);
        assert_eq!(c1.a, 0.8);

        let c2 = ColorF::rgb(1.0, 0.0, 0.0);
        assert_eq!(c2.a, 1.0);

        let c3 = ColorF::from_rgba8(255, 128, 0, 255);
        assert_eq!(c3.r, 1.0);
        assert!(c3.g > 0.49 && c3.g < 0.51);

        let c4 = ColorF::from_rgb8(0, 0, 0);
        assert_eq!(c4.r, 0.0);
        assert_eq!(c4.a, 1.0);
    }

    #[test]
    fn device_accessors() {
        let device = GpuDevice::new_warp().unwrap();
        let _d2d_factory = device.d2d_factory();
        let _dwrite = device.dwrite_factory();
    }

    #[test]
    fn swap_chain_raw_and_load_bitmap() {
        ensure_com_initialized();
        let device = GpuDevice::new_warp().unwrap();
        let chain = device.create_swap_chain(64, 64).unwrap();

        let _raw = chain.raw_swap_chain();

        let bitmap = chain
            .load_bitmap(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test.png"))
            .unwrap();
        assert!(bitmap.width() > 0.0);
    }

    #[test]
    fn session_raw_and_create_brush() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();

        let session = chain.begin_draw().unwrap();
        let _raw = session.raw();

        // create_solid_brush on session (vs swap chain).
        let brush = session.create_solid_brush(ColorF::RED).unwrap();
        session.fill_rect(&Rect::new(0.0, 0.0, 64.0, 64.0), &brush);

        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn path_raw_accessor() {
        let device = GpuDevice::new_warp().unwrap();
        let path = PathBuilder::new(&device)
            .unwrap()
            .begin(Vector2::new(0.0, 0.0))
            .line_to(Vector2::new(10.0, 0.0))
            .close()
            .build()
            .unwrap();

        let _raw = path.raw();
    }

    #[test]
    fn path_fill_contains_point() {
        let device = GpuDevice::new_warp().unwrap();

        // Triangle: apex at (32, 0), base from (0, 64) to (64, 64).
        let path = PathBuilder::new(&device)
            .unwrap()
            .begin(Vector2::new(32.0, 0.0))
            .line_to(Vector2::new(64.0, 64.0))
            .line_to(Vector2::new(0.0, 64.0))
            .close()
            .build()
            .unwrap();

        assert!(path.fill_contains_point(Vector2::new(32.0, 50.0)));
        assert!(!path.fill_contains_point(Vector2::new(5.0, 5.0)));
    }

    #[test]
    fn path_stroke_contains_point() {
        let device = GpuDevice::new_warp().unwrap();

        // Open horizontal segment from (10, 32) to (54, 32).
        let path = PathBuilder::new(&device)
            .unwrap()
            .begin_hollow(Vector2::new(10.0, 32.0))
            .line_to(Vector2::new(54.0, 32.0))
            .end_open()
            .build()
            .unwrap();

        assert!(path.stroke_contains_point(Vector2::new(32.0, 32.0), 4.0));
        assert!(!path.stroke_contains_point(Vector2::new(32.0, 50.0), 4.0));
    }

    #[test]
    fn path_compute_bounds() {
        let device = GpuDevice::new_warp().unwrap();

        // Axis-aligned rectangle 10,20 -> 40,50.
        let path = PathBuilder::new(&device)
            .unwrap()
            .begin(Vector2::new(10.0, 20.0))
            .line_to(Vector2::new(40.0, 20.0))
            .line_to(Vector2::new(40.0, 50.0))
            .line_to(Vector2::new(10.0, 50.0))
            .close()
            .build()
            .unwrap();

        let bounds = path.compute_bounds();
        assert!((bounds.left - 10.0).abs() < 0.5);
        assert!((bounds.top - 20.0).abs() < 0.5);
        assert!((bounds.right - 40.0).abs() < 0.5);
        assert!((bounds.bottom - 50.0).abs() < 0.5);
    }

    #[test]
    fn path_polygon() {
        let device = GpuDevice::new_warp().unwrap();

        // Same triangle as path_fill_contains_point, built via the polygon helper.
        let path = PathBuilder::new(&device)
            .unwrap()
            .polygon([
                Vector2::new(32.0, 0.0),
                Vector2::new(64.0, 64.0),
                Vector2::new(0.0, 64.0),
            ])
            .unwrap();

        assert!(path.fill_contains_point(Vector2::new(32.0, 50.0)));
        assert!(!path.fill_contains_point(Vector2::new(5.0, 5.0)));
    }

    #[test]
    fn path_polygon_empty_errors() {
        let device = GpuDevice::new_warp().unwrap();
        let result = PathBuilder::new(&device).unwrap().polygon([]);
        assert!(result.is_err());
    }

    // Device-lost classification is pure logic (no GPU needed). The codes below
    // are the canonical DXGI/Direct2D HRESULTs, written independently of the
    // crate's own constants so the test isn't a tautology.
    #[test]
    fn is_device_lost_classifies_known_codes() {
        for code in [
            0x887A0005_u32, // DXGI_ERROR_DEVICE_REMOVED
            0x887A0007,     // DXGI_ERROR_DEVICE_RESET
            0x887A0006,     // DXGI_ERROR_DEVICE_HUNG
            0x887A0020,     // DXGI_ERROR_DRIVER_INTERNAL_ERROR
            0x8899000C,     // D2DERR_RECREATE_TARGET
        ] {
            let hr = windows_core::HRESULT(code as i32);
            assert!(is_device_lost(hr), "{code:#X} should be device-lost");
        }
    }

    #[test]
    fn is_device_lost_rejects_other_codes() {
        for code in [
            0x0000_0000_u32, // S_OK
            0x8007_0057,     // E_INVALIDARG
            0x8000_4005,     // E_FAIL
            0x8007_000E,     // E_OUTOFMEMORY
        ] {
            let hr = windows_core::HRESULT(code as i32);
            assert!(!is_device_lost(hr), "{code:#X} should not be device-lost");
        }
    }

    #[test]
    fn check_device_lost_handles_ok_and_err() {
        let ok: Result<i32> = Ok(7);
        assert!(!check_device_lost(&ok));

        let lost: Result<i32> = Err(windows_core::Error::from_hresult(windows_core::HRESULT(
            0x887A0005_u32 as i32, // DXGI_ERROR_DEVICE_REMOVED
        )));
        assert!(check_device_lost(&lost));

        let other: Result<i32> = Err(windows_core::Error::from_hresult(windows_core::HRESULT(
            0x8007_0057_u32 as i32, // E_INVALIDARG
        )));
        assert!(!check_device_lost(&other));
    }

    #[test]
    fn new_or_warp_produces_working_device() {
        let device = GpuDevice::new_or_warp().unwrap();
        let chain = device.create_swap_chain(64, 64).unwrap();
        let _raw = chain.raw_swap_chain();
    }

    #[test]
    fn set_dpi_recreates_target_and_still_draws() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();
        chain.set_dpi(192.0, 192.0);
        {
            let session = chain.begin_draw().unwrap();
            session.clear(ColorF::WHITE);
        }
        chain.present().unwrap();
    }

    #[test]
    fn set_composition_scale_is_applied() {
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();
        chain.set_composition_scale(2.0, 2.0);
        {
            let session = chain.begin_draw().unwrap();
            session.clear(ColorF::BLACK);
        }
        chain.present().unwrap();
    }

    // --- Waitable swap chain (frame-latency waitable object) ---

    #[test]
    fn normal_swap_chain_is_not_waitable() {
        let device = GpuDevice::new_warp().unwrap();
        let chain = device.create_swap_chain(64, 64).unwrap();
        assert!(
            chain.frame_latency_waitable().is_none(),
            "a non-waitable chain must not expose a frame-latency object"
        );
    }

    #[test]
    fn create_waitable_swap_chain() {
        let device = GpuDevice::new_warp().unwrap();
        let chain = device.create_waitable_swap_chain(64, 64).unwrap();
        assert_eq!((chain.width(), chain.height()), (64, 64));
        let wait = chain
            .frame_latency_waitable()
            .expect("a waitable chain exposes its frame-latency object");
        assert!(
            !wait.0.is_null(),
            "frame-latency waitable handle should be non-null"
        );
    }

    #[test]
    fn waitable_draw_and_present_paces() {
        // The built-in wait must complete each frame, not hang: several frames in
        // a row exercise the wait -> draw -> present -> re-signal cycle. If the
        // wait deadlocked, the 1s timeout in begin_draw would still let it proceed,
        // so a hang here would be a real bug.
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_waitable_swap_chain(64, 64).unwrap();
        for _ in 0..5 {
            let session = chain.begin_draw().unwrap();
            session.clear(ColorF::CORNFLOWER_BLUE);
            drop(session);
            assert!(chain.present().unwrap());
        }
    }

    #[test]
    fn waitable_resize_preserves_flag() {
        // Resizing a waitable chain must replay the waitable flag; passing the
        // wrong flags to ResizeBuffers makes it fail. The object survives the
        // resize and the chain stays usable.
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_waitable_swap_chain(64, 64).unwrap();
        chain.resize(128, 96).unwrap();
        assert_eq!((chain.width(), chain.height()), (128, 96));
        assert!(chain.frame_latency_waitable().is_some());

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        drop(session);
        chain.present().unwrap();
    }

    #[test]
    fn set_wait_object_none_disables_wait() {
        // Clearing the wait lets a consumer pace the frame themselves; begin_draw
        // must not block.
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_waitable_swap_chain(64, 64).unwrap();
        assert!(chain.frame_latency_waitable().is_some());
        chain.set_wait_object(None);
        for _ in 0..3 {
            let session = chain.begin_draw().unwrap();
            session.clear(ColorF::RED);
            drop(session);
            chain.present().unwrap();
        }
    }

    #[test]
    fn set_wait_object_custom_event() {
        // Substitute our own already-signalled manual-reset event: begin_draw must
        // wait on it (and return promptly because it is signalled).
        unsafe {
            windows_core::link!("kernel32.dll" "system" fn CreateEventW(attrs: *const core::ffi::c_void, manual_reset: windows_core::BOOL, initial: windows_core::BOOL, name: *const u16) -> *mut core::ffi::c_void);
            windows_core::link!("kernel32.dll" "system" fn CloseHandle(handle: *mut core::ffi::c_void) -> windows_core::BOOL);

            let event = CreateEventW(
                core::ptr::null(),
                true.into(),
                true.into(),
                core::ptr::null(),
            );
            assert!(!event.is_null(), "CreateEventW failed");

            let device = GpuDevice::new_warp().unwrap();
            let mut chain = device.create_waitable_swap_chain(64, 64).unwrap();
            chain.set_wait_object(Some(WaitObject(event)));

            let session = chain.begin_draw().unwrap();
            session.clear(ColorF::GREEN);
            drop(session);
            chain.present().unwrap();

            let _ = CloseHandle(event);
        }
    }

    #[test]
    fn set_composition_scale_and_dpi() {
        // Exercises the DPI/retarget path and the `IDXGISwapChain2` cast +
        // `SetMatrixTransform` path (composition scale). Both must succeed on a
        // WARP composition chain and leave the chain drawable.
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_swap_chain(64, 64).unwrap();
        chain.set_dpi(192.0, 192.0);
        chain.set_composition_scale(2.0, 2.0);

        let session = chain.begin_draw().unwrap();
        session.clear(ColorF::BLACK);
        drop(session);
        chain.present().unwrap();
    }

    // --- Stress ---

    #[test]
    fn stress_many_swap_chains() {
        // Create, draw, present and drop many swap chains from one device, to
        // shake out resource lifetime / target-recreation issues.
        let device = GpuDevice::new_warp().unwrap();
        for i in 0..64u32 {
            let n = 32 + i % 64;
            let mut chain = device.create_swap_chain(n, n).unwrap();
            let session = chain.begin_draw().unwrap();
            session.clear(ColorF::BLACK);
            drop(session);
            chain.present().unwrap();
        }
    }

    #[test]
    fn stress_waitable_many_frames() {
        // Hammer the wait -> draw -> present -> re-signal cycle for many frames so
        // a deadlock or a leaked/never-signalled waitable object would surface.
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_waitable_swap_chain(64, 64).unwrap();
        for _ in 0..300 {
            let session = chain.begin_draw().unwrap();
            session.clear(ColorF::CORNFLOWER_BLUE);
            drop(session);
            assert!(chain.present().unwrap());
        }
    }

    #[test]
    fn stress_resize_churn() {
        // Rapidly resize a waitable chain between frames: exercises ResizeBuffers
        // + target recreation and the waitable-flag replay under churn.
        let device = GpuDevice::new_warp().unwrap();
        let mut chain = device.create_waitable_swap_chain(64, 64).unwrap();
        for i in 0..64u32 {
            let n = 16 + (i * 7) % 200;
            chain.resize(n, n).unwrap();
            assert_eq!((chain.width(), chain.height()), (n, n));
            let session = chain.begin_draw().unwrap();
            session.clear(ColorF::BLACK);
            drop(session);
            chain.present().unwrap();
        }
        assert!(chain.frame_latency_waitable().is_some());
    }

    // --- Multi-threaded (shared multi-threaded factory device) ---
    //
    // These share one multi-threaded-factory device across threads, each
    // rendering through its own swap chain. The concurrent
    // CreateSwapChainForComposition / GetBuffer / Present / ResizeBuffers calls on
    // the shared Direct3D device are serialized by the crate's internal
    // `ID2D1Multithread` factory lock; without correct locking this races and
    // typically crashes, corrupts, or hangs. WARP keeps them GPU-independent.

    const STRESS_THREADS: usize = 8;
    const STRESS_FRAMES: usize = 200;

    #[test]
    fn multi_threaded_shared_device_stress() {
        let device = GpuDevice::new_warp_multi_threaded().unwrap();
        let mut handles = Vec::with_capacity(STRESS_THREADS);
        for t in 0..STRESS_THREADS {
            let device = device.clone();
            handles.push(std::thread::spawn(move || {
                let mut chain = device.create_swap_chain(64, 64).unwrap();
                for f in 0..STRESS_FRAMES {
                    {
                        let session = chain.begin_draw().unwrap();
                        session.clear(if (t + f) % 2 == 0 {
                            ColorF::RED
                        } else {
                            ColorF::BLUE
                        });
                    }
                    chain.present().unwrap();
                    // Periodically churn the buffers to exercise ResizeBuffers and
                    // GetBuffer on the shared device under contention.
                    if f % 16 == 15 {
                        let n = 48 + (f as u32 % 32);
                        chain.resize(n, n).unwrap();
                    }
                }
            }));
        }
        handles.into_iter().for_each(|h| h.join().unwrap());
    }

    #[test]
    fn multi_threaded_waitable_and_normal_mix() {
        // Mixed workload on the shared device: half the threads use waitable
        // chains (auto-wait pacing), half use normal chains, all presenting
        // concurrently — so the lock is exercised around the waitable setup and
        // the per-frame wait as well as Present.
        const THREADS: usize = 6;
        const FRAMES: usize = 120;
        let device = GpuDevice::new_warp_multi_threaded().unwrap();
        let mut handles = Vec::with_capacity(THREADS);
        for t in 0..THREADS {
            let device = device.clone();
            handles.push(std::thread::spawn(move || {
                let mut chain = if t % 2 == 0 {
                    device.create_waitable_swap_chain(64, 64).unwrap()
                } else {
                    device.create_swap_chain(64, 64).unwrap()
                };
                for _ in 0..FRAMES {
                    {
                        let session = chain.begin_draw().unwrap();
                        session.clear(ColorF::GREEN);
                    }
                    chain.present().unwrap();
                }
            }));
        }
        handles.into_iter().for_each(|h| h.join().unwrap());
    }
}
