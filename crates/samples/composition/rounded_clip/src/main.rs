//! Clip sample — rounding a sprite whose one brush slot is already spent.
//!
//! A visual takes exactly one brush, and a [`CompositionMaskBrush`] is that one
//! brush: a wide-gamut source revealed by a separate mask. So a card painted
//! that way has nothing left to round its corners with — a mask cannot nest
//! (`set_source` rejects another mask), and capturing the card through a
//! visual surface to round it costs a surface per card and distorts the mask's
//! alpha on the way through.
//!
//! A clip is not a brush. `RectangleClip` and `CompositionGeometricClip` round
//! the same card with no second visual, no surface and no capture, and every
//! side and radius stays animatable — see `RectangleClip` for the per-channel
//! names a radius animates under.
//!
//! The four cards below are the proof, left to right, top to bottom:
//!
//! 1. mask brush + `RectangleClip` — rounded, and the ramp still reads;
//! 2. the same mask brush with no clip — the square-cornered control;
//! 3. `CompositionGeometricClip` over a rounded-rectangle geometry;
//! 4. `RectangleClip` with the four corner radii set independently.

#![windows_subsystem = "windows"]

use windows_composition::*;
use windows_window::*;

const CARD_W: f32 = 300.0;
const CARD_H: f32 = 200.0;
const RADIUS: f32 = 28.0;

fn main() -> Result<()> {
    // A dispatcher queue must exist on the thread before creating a compositor,
    // and both must outlive every composition object and the window — see the
    // standalone sample for why the drop order matters.
    let _queue = DispatcherQueueController::create_on_current_thread()?;
    let compositor = Compositor::new()?;

    let window = Window::new("Composition Rounded Clip")
        .size(800, 600)
        .create()?;

    let target = compositor.create_desktop_window_target(&window, false)?;
    let root = compositor.create_container_visual();
    target.set_root(&root);

    let (width, height) = window.client_size();
    let background = compositor.create_sprite_visual();
    background.set_size(width as f32, height as f32);
    let background_brush = compositor.create_color_brush(Color::rgb(18, 18, 28));
    background.set_brush(&background_brush);
    root.children().insert_at_top(&background);

    // ── 1: mask brush + rectangle clip ──────────────────────────────────────
    // The case with no other cheap route. If the corners come back rounded AND
    // the mask's ramp still fades across the card, the clip composed with the
    // mask rather than replacing it.
    let card = masked_card(&compositor, Color::rgb(0, 120, 215));
    card.set_offset(60.0, 60.0, 0.0);
    let clip = compositor.create_rectangle_clip();
    // A rectangle clip's sides are absolute in the clipped visual's own space,
    // not insets from its edges: a fresh clip is 0,0,0,0, which hides the visual
    // entirely rather than leaving it whole.
    clip.set_sides(0.0, 0.0, CARD_W, CARD_H);
    clip.set_corner_radius(Vector2::new(RADIUS, RADIUS));
    card.set_clip(Some(&clip));
    root.children().insert_at_top(&card);

    // ── 2: the same card, unclipped ─────────────────────────────────────────
    let control = masked_card(&compositor, Color::rgb(0, 120, 215));
    control.set_offset(440.0, 60.0, 0.0);
    // Explicitly cleared rather than never set, so this also exercises the
    // type-free path off `set_clip`'s inferred `Some` arm.
    control.clear_clip();
    root.children().insert_at_top(&control);

    // ── 3: geometric clip over a rounded-rectangle geometry ─────────────────
    let geometric = compositor.create_sprite_visual();
    geometric.set_size(CARD_W, CARD_H);
    geometric.set_offset(60.0, 320.0, 0.0);
    let solid = compositor.create_color_brush(Color::rgb(216, 59, 1));
    geometric.set_brush(&solid);
    let geometry = compositor.create_rounded_rectangle_geometry();
    geometry.set_size(Vector2::new(CARD_W, CARD_H));
    geometry.set_corner_radius(Vector2::new(CARD_H / 2.0, CARD_H / 2.0));
    geometric.set_clip(Some(&compositor.create_geometric_clip(&geometry)));
    root.children().insert_at_top(&geometric);

    // ── 4: four independent corner radii ────────────────────────────────────
    let corners = compositor.create_sprite_visual();
    corners.set_size(CARD_W, CARD_H);
    corners.set_offset(440.0, 320.0, 0.0);
    let green = compositor.create_color_brush(Color::rgb(16, 137, 62));
    corners.set_brush(&green);
    let asymmetric = compositor.create_rectangle_clip();
    asymmetric.set_sides(0.0, 0.0, CARD_W, CARD_H);
    asymmetric.set_corner_radii(
        Vector2::new(0.0, 0.0),
        Vector2::new(64.0, 64.0),
        Vector2::new(0.0, 0.0),
        Vector2::new(64.0, 24.0),
    );
    corners.set_clip(Some(&asymmetric));
    root.children().insert_at_top(&corners);

    run();
    Ok(())
}

/// A card painted by a mask brush: a solid `color` source revealed by a
/// left-to-right alpha ramp, so the visual's single brush slot is spent and
/// the ramp makes it obvious whether a clip replaced the paint or composed
/// with it.
fn masked_card(compositor: &Compositor, color: Color) -> SpriteVisual {
    let card = compositor.create_sprite_visual();
    card.set_size(CARD_W, CARD_H);

    let ramp = compositor.create_linear_gradient_brush();
    ramp.set_mapping_mode(MappingMode::Relative);
    ramp.set_line(Vector2::new(0.0, 0.0), Vector2::new(1.0, 0.0));
    ramp.set_alpha_stops(compositor, &[(0.0, 1.0), (1.0, 0.15)]);

    let mask = compositor.create_mask_brush();
    mask.set_mask(&ramp);
    mask.set_source(&compositor.create_color_brush(color));
    card.set_brush(&mask);
    card
}
