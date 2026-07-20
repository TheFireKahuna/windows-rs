//! Per-node surface painting. Each node with chrome owns a `SpriteVisual` backed
//! by an FP16 `CompositionDrawingSurface` sized to its rect; this module ensures
//! that surface exists, resizes it when the node's size changes, and redraws it
//! **only when the node's own content/size/state changed** (its `dirty` flag).
//! Pure layout containers never get a surface. A small [`PaintCache`] holds a
//! single recolorable solid brush, shared across every node's surface (all
//! surfaces derive from one Direct2D device), so painting allocates nothing per
//! frame.

use super::bootstrap::Compositing;
use super::node::{linear, Arena, Node};
use super::*;
use crate::backend::ControlKind;
use windows_canvas_core::{
    bindings::ID2D1DeviceContext, ColorF, DrawingSession, Ellipse, Rect, RoundedRect, Vector2,
};
use windows_numerics::Matrix3x2;

/// Per-device paint resources. One recolorable brush, reused every frame across
/// all node surfaces (Direct2D brushes are shareable across device contexts of
/// the same device).
#[derive(Default)]
pub(crate) struct PaintCache {
    brush: Option<windows_canvas_core::Brush>,
}

impl PaintCache {
    /// Drop cached GPU resources (e.g. after device loss).
    pub fn invalidate(&mut self) {
        self.brush = None;
    }
}

/// Transparent clear color for premultiplied node surfaces.
const CLEAR: ColorF = ColorF::new(0.0, 0.0, 0.0, 0.0);

/// Walk the tree, repainting each dirty node's own surface. Returns `Err` on
/// device loss so the caller can drop and rebuild cached resources.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint(
    comp: &Compositing,
    cache: &mut PaintCache,
    atlas: &mut parts::Atlas,
    glyphs: &mut super::glyph_atlas::GlyphAtlas,
    arena: &mut Arena,
    root: ControlId,
    scale: f32,
    scrubbing: bool,
) -> windows_core::Result<()> {
    paint_node(comp, cache, atlas, glyphs, arena, root, scale, scrubbing)
}

#[allow(clippy::too_many_arguments)]
fn paint_node(
    comp: &Compositing,
    cache: &mut PaintCache,
    atlas: &mut parts::Atlas,
    glyphs: &mut super::glyph_atlas::GlyphAtlas,
    arena: &mut Arena,
    id: ControlId,
    scale: f32,
    scrubbing: bool,
) -> windows_core::Result<()> {
    // A node reaches this block either because it draws (and so wants a
    // surface) or because it owns retained chrome to reconcile. The two are no
    // longer the same set: the button family draws nothing at all, and its
    // parts and glyph sprites ARE its appearance.
    let needs = arena.get(id).is_some_and(|n| {
        n.has_chrome()
            || n.surf.is_some()
            || parts::converted(n.kind)
            // A TextBlock now reports no chrome and owns no surface, so without
            // this it would never be visited and its glyph sprites would never
            // be placed — the node would simply render nothing.
            || n.kind == ControlKind::TextBlock
            // Same for a path: its curve is retained sprite shapes, so it owns
            // no surface and must be visited on its own account.
            || n.kind == ControlKind::Path
            // And a knob: groove, ticks, hub, arc and needle are all retained
            // compositor chrome and its words are sprites, so an UNFOCUSED one
            // reports no chrome and owns no surface. Without this it would never
            // be visited, `sync_knob` and `knob_sync` would never run, and the
            // dial would render nothing at all.
            || n.kind == ControlKind::Knob
    });
    if needs {
        let (w, h) = arena.get(id).map(|n| (n.rect.w, n.rect.h)).unwrap_or((0.0, 0.0));
        // Layout rects are pixel-snapped, so `w * scale` is integral (mod FP
        // noise); `round` keeps the surface exactly 1:1 with the visual where
        // `ceil` could add a phantom pixel and force a resample.
        let pw = (w * scale).round() as i32;
        let ph = (h * scale).round() as i32;

        let draws = arena
            .get(id)
            .is_some_and(|n| n.has_chrome() || n.surf.is_some());
        if draws {
            let has_surface = arena.get(id).is_some_and(|n| n.surf.is_some());
            if !has_surface {
                let container = arena.get(id).unwrap().container.clone();
                let surf = comp.new_surface(&container, pw, ph)?;
                if let Some(n) = arena.get_mut(id) {
                    n.surf = Some(surf);
                }
            }
            // Both writes self-gate, so a surface just minted at (pw, ph) skips
            // the resize and takes the DIP push once.
            if let Some(n) = arena.get_mut(id)
                && let Some(s) = &mut n.surf
            {
                let _ = s.resize(pw, ph);
                s.set_dip_size(w, h);
            }
        }

        // Where a caption band's Content slot landed, in the band's own space —
        // read BEFORE the mutable borrow below, because it lives on a different
        // node. It is the far edge of the title's grid track, so it is what the
        // drawn title ellipsizes against; re-deriving the track here instead
        // would be this module second-guessing the layout that just ran.
        let caption_content_left = arena
            .get(id)
            .filter(|n| n.kind == ControlKind::TitleBar)
            .and_then(|n| n.title_content.map(|c| (n.rect.x, c)))
            .and_then(|(bx, c)| arena.get(c).map(|c| c.rect.x - bx));

        let dirty = arena.get(id).is_some_and(|n| n.dirty);
        if dirty && w > 0.0 && h > 0.0 {
            if draws {
                draw_surface(comp, cache, arena, id, scale)?;
            }
            if let Some(n) = arena.get_mut(id) {
                n.dirty = false;
                // Reconcile the converted kinds' retained chrome parts (pill /
                // knob / fill / ink) against the state just painted: state
                // changes glide on the compositor, first placement snaps.
                if parts::converted(n.kind) {
                    parts::sync(comp, atlas, n, scale, scrubbing);
                }
                // The button family's label, icon and badge count: retained
                // glyph sprites, placed AFTER the parts sync so their hosts land
                // above the ink that sync creates on first use.
                super::glyph_text::button_sync(comp, glyphs, n, scale);
                // The same, for the one control that is only text.
                super::glyph_text::text_sync(comp, glyphs, n, scale);
                // …and for the one that is only text plus a focus ring.
                super::glyph_text::hyperlink_sync(comp, glyphs, n, scale);
                // The two controls whose words come one PER ITEM rather than
                // one per node: a switch's state label, a bar's segment labels.
                super::glyph_text::toggle_sync(comp, glyphs, n, scale);
                super::glyph_text::segmented_sync(comp, glyphs, n, scale);
                // …and the count on a badge's plate.
                super::glyph_text::info_badge_sync(comp, glyphs, n, scale);
                // …and the pane's two chrome glyphs, its header, and the glyph
                // plus label on every row it has room for.
                super::glyph_text::nav_sync(comp, glyphs, n, scale);
                // The bar's severity glyph, its wrapped paragraph and its close
                // glyph — the paragraph re-pinned to its column first.
                super::glyph_text::info_bar_sync(comp, glyphs, n, scale);
                // The caption's coupled title/subtitle pair and its four button
                // glyphs.
                super::glyph_text::caption_sync(comp, glyphs, n, scale, caption_content_left);
                // …and a checkbox's trailing label.
                super::glyph_text::check_sync(comp, glyphs, n, scale);
                // …and an expander's header label plus its chevron.
                super::glyph_text::expander_sync(comp, glyphs, n, scale);
                // …and a select trigger's current label plus its chevron, above
                // the box and border it keeps on its surface.
                super::glyph_text::select_sync(comp, glyphs, n, scale);
                // Editors: the text run, its selection and its composition rule
                // as sprites, then the caret sprite. Both are placed from the
                // same `editor::TextBand`, so they cannot disagree.
                if n.editor.is_some() {
                    super::glyph_text::editor_sync(comp, glyphs, n, scale);
                    parts::sync_caret(comp, atlas, n, scale);
                }
                // Knob: reconcile the value-arc shape + needle (its own retained
                // vector chrome, outside the flat `Part` model), then its dial
                // text — which shapes here rather than in the layout pass,
                // because its type sizes come from the radius the solve decided.
                if n.kind == crate::backend::ControlKind::Knob {
                    super::knob::sync_knob(comp, n, atlas.epoch(), scale, scrubbing);
                    super::glyph_text::knob_sync(comp, glyphs, n, scale);
                }
                // Path: reconcile the retained curve layers. Same shape as the
                // knob's — vector chrome outside the flat `Part` model.
                if n.kind == crate::backend::ControlKind::Path {
                    super::path_shape::sync_path(comp, n, atlas.epoch(), scale);
                }
            }
        }
    }

    // Indexed rather than over a cloned child list — the clone was a heap
    // allocation per node per frame bought purely to dodge `&mut Arena`. An
    // index is also the only shape that stays correct here: this walk can
    // propagate `?` mid-iteration on device loss, so a `mem::take` of the
    // children would leave the node permanently childless on the way out.
    let mut i = 0;
    while let Some(c) = arena.get(id).and_then(|n| n.children.get(i).copied()) {
        paint_node(comp, cache, atlas, glyphs, arena, c, scale, scrubbing)?;
        i += 1;
    }

    // Overlay scrollbar thumb (above the scrolled children) for scroll containers.
    if arena.get(id).is_some_and(|n| n.is_scroll()) {
        update_scroll_thumb(comp, cache, arena, id, scale)?;
    }
    Ok(())
}

/// Ensure / size / position the auto-hiding overlay scrollbar thumb of a
/// scroll container. The thumb is a top child sprite (drawn over the content);
/// its height tracks viewport/content and its offset tracks the scroll
/// fraction. Its show/hide fade plays on the system compositor
/// ([`animate::fade_thumb`](super::animate::fade_thumb)), edge-triggered from
/// the tick loop's `thumb_shown` flip — never written per frame here.
fn update_scroll_thumb(
    comp: &Compositing,
    cache: &mut PaintCache,
    arena: &mut Arena,
    id: ControlId,
    scale: f32,
) -> windows_core::Result<()> {
    use scroll::{thumb_geom, THUMB_MARGIN, THUMB_W};
    let (vh, content_h, sc, shown) = match arena.get(id) {
        Some(n) => (n.rect.h, n.ctrl().content_h, n.scroll_off, n.thumb_shown),
        None => return Ok(()),
    };
    let g = thumb_geom(vh, content_h, sc);
    if !g.overflow {
        // Content no longer overflows: hide a revealed thumb NOW (no fade — it
        // has nothing to indicate) and reset the reveal state so a future
        // overflow fades in from hidden. Gated on the flag so steady paints of
        // a non-overflowing container cost nothing.
        if let Some(n) = arena.get_mut(id)
            && n.thumb_shown
        {
            n.thumb_shown = false;
            if let Some(t) = &n.scroll_thumb {
                t.snap_opacity(0.0);
            }
        }
        return Ok(());
    }

    // An always-visible bar has no hover edge to ride, so overflow itself is
    // its reveal: the moment the content outgrows the viewport, show it. A
    // never-visible one is concealed here for the same reason — the policy may
    // have changed while the pointer was nowhere near.
    let shown = match arena
        .get(id)
        .map(|n| scroll::reveal_policy(n.extras().v_scrollbar))
    {
        Some(scroll::Reveal::Always) if !shown => {
            if let Some(n) = arena.get_mut(id) {
                n.thumb_shown = true;
                if let Some(t) = &n.scroll_thumb {
                    animate::fade_thumb(comp.compositor(), t, true);
                }
            }
            true
        }
        Some(scroll::Reveal::Never) if shown => {
            if let Some(n) = arena.get_mut(id) {
                n.thumb_shown = false;
                if let Some(t) = &n.scroll_thumb {
                    animate::fade_thumb(comp.compositor(), t, false);
                }
            }
            false
        }
        _ => shown,
    };

    let pw = (THUMB_W * scale).ceil() as i32;
    let ph = (g.thumb_h * scale).ceil() as i32;
    if arena.get(id).is_some_and(|n| n.scroll_thumb.is_none()) {
        let container = arena.get(id).unwrap().container.clone();
        let surf = comp.new_surface_at(&container, pw, ph, true)?;
        // Born hidden; if the scroll is already active (the tick's edge fired
        // before the sprite existed) the reveal fade plays from zero.
        surf.set_opacity(0.0);
        if shown {
            animate::fade_thumb(comp.compositor(), &surf, true);
        }
        if let Some(n) = arena.get_mut(id) {
            n.scroll_thumb = Some(surf);
        }
    }

    // Re-rasterize the bar only when its height (hence corner geometry) changed.
    if arena.get(id).is_some_and(|n| (n.thumb_drawn_h - g.thumb_h).abs() > 0.5) {
        if let Some(n) = arena.get_mut(id)
            && let Some(s) = &mut n.scroll_thumb
        {
            let _ = s.resize(pw, ph);
            s.set_dip_size(THUMB_W, g.thumb_h);
        }
        draw_thumb(comp, cache, arena, id, scale, g.thumb_h)?;
        if let Some(n) = arena.get_mut(id) {
            n.thumb_drawn_h = g.thumb_h;
        }
    }

    if let Some(n) = arena.get(id)
        && let Some(s) = &n.scroll_thumb
    {
        s.set_offset(n.rect.w - THUMB_W - THUMB_MARGIN, g.thumb_y);
    }
    Ok(())
}

/// Draw the thumb bar (a rounded, stroke-strong pill) into its own surface.
fn draw_thumb(
    comp: &Compositing,
    cache: &mut PaintCache,
    arena: &mut Arena,
    id: ControlId,
    scale: f32,
    thumb_h: f32,
) -> windows_core::Result<()> {
    let interop = arena.get(id).unwrap().scroll_thumb.as_ref().unwrap().interop.clone();
    let mut offset = crate::system_bindings::POINT::default();
    comp.device_lost.set(false);
    let ctx: ID2D1DeviceContext = unsafe { interop.BeginDraw(None, &mut offset)? };
    let session = DrawingSession::from_borrowed_context(
        &ctx,
        Matrix3x2::translation(offset.x as f32, offset.y as f32),
    );
    session.set_transform(&Matrix3x2 {
        m11: scale,
        m12: 0.0,
        m21: 0.0,
        m22: scale,
        m31: 0.0,
        m32: 0.0,
    });
    session.clear(CLEAR);
    if cache.brush.is_none() {
        cache.brush = session.create_solid_brush(ColorF::BLACK).ok();
    }
    if let Some(brush) = &cache.brush {
        brush.set_color(linear(theme::stroke_strong()));
        let r = Rect::from_xywh(0.0, 0.0, scroll::THUMB_W, thumb_h);
        session.fill_rounded_rect(&RoundedRect::uniform(r, scroll::THUMB_W / 2.0), brush);
    }
    unsafe { interop.EndDraw().ok()? };
    Ok(())
}

/// Redraw one node's surface: its own chrome at local (0,0)-origin DIP coords.
fn draw_surface(
    comp: &Compositing,
    cache: &mut PaintCache,
    arena: &mut Arena,
    id: ControlId,
    scale: f32,
) -> windows_core::Result<()> {
    let interop = arena.get(id).unwrap().surf.as_ref().unwrap().interop.clone();

    let mut offset = crate::system_bindings::POINT::default();
    comp.device_lost.set(false);
    let ctx: ID2D1DeviceContext = unsafe { interop.BeginDraw(None, &mut offset)? };

    let session = DrawingSession::from_borrowed_context(
        &ctx,
        Matrix3x2::translation(offset.x as f32, offset.y as f32),
    );
    // Grayscale text AA is mandatory on premultiplied/transparent surfaces.
    session.set_grayscale_text_antialiasing();
    // pixel = dip * scale + atlas offset (uniform scale keeps strokes/text crisp).
    session.set_transform(&Matrix3x2 {
        m11: scale,
        m12: 0.0,
        m21: 0.0,
        m22: scale,
        m31: 0.0,
        m32: 0.0,
    });
    session.clear(CLEAR);

    if cache.brush.is_none() {
        cache.brush = session.create_solid_brush(ColorF::BLACK).ok();
    }
    if let Some(brush) = &cache.brush {
        let node = arena.get(id).unwrap();
        // Local rect: the node's own box at the surface origin.
        let local = Rect::from_xywh(0.0, 0.0, node.rect.w, node.rect.h);
        paint_chrome(&session, brush, node, local);
    }

    unsafe { interop.EndDraw().ok()? };
    Ok(())
}

/// Draw a node's own chrome into its surface (no recursion — children own their
/// own surfaces).
fn paint_chrome(
    session: &DrawingSession,
    brush: &windows_canvas_core::Brush,
    node: &Node,
    rect: Rect,
) {
    // The drawn control library handles its own kinds (button, toggle, slider,
    // segmented, select, nav, progress, …) including ink + focus ring.
    if controls::paint(session, brush, node, rect) {
        return;
    }
    match node.kind {
        // Nothing: a TextBlock's prose is retained glyph sprites, so it owns no
        // surface at all and only reaches here on one left over from before the
        // cutover. Falling through to `fill_and_stroke` would paint a
        // background a TextBlock has never had.
        ControlKind::TextBlock => {}
        // Likewise nothing: a Border's fill and outline are retained parts
        // (`parts::box_plan`). Falling through to `fill_and_stroke` would paint
        // a second copy of the box directly under the sprites drawing it.
        ControlKind::Border => {}
        ControlKind::Rectangle => {
            fill_and_stroke(session, brush, node, rect, node.paint.corner_radius)
        }
        ControlKind::Ellipse => paint_ellipse(session, brush, node, rect),
        ControlKind::Line => paint_line(session, brush, node, rect),
        // Nothing: a path's geometry is a retained compositor sprite shape
        // (`CompositionPath` + `CompositionSpriteShape` masked over an FP16
        // source), never rasterized here. It owns no surface at all.
        ControlKind::Path => {}
        // Reached only by a `ControlKind` not yet listed in `parts::plain_box`.
        // Every container that IS listed draws its fill and outline as retained
        // parts and owns no surface, so painting here would put a second copy of
        // the box directly under the sprites drawing it — the same reason the
        // `Border` arm above is empty. An unlisted kind still gets a surface
        // from `has_chrome`'s backstop, and this draws the plain box it is
        // probably meant to be.
        _ => fill_and_stroke(session, brush, node, rect, node.paint.corner_radius),
    }
}

/// Fill a node's background and stroke its border, honouring corner radius.
fn fill_and_stroke(
    session: &DrawingSession,
    brush: &windows_canvas_core::Brush,
    node: &Node,
    rect: Rect,
    radius: f32,
) {
    if let Some(bg) = node.paint.background {
        brush.set_color(linear(bg));
        if radius > 0.0 {
            session.fill_rounded_rect(&RoundedRect::uniform(rect, radius), brush);
        } else {
            session.fill_rect(&rect, brush);
        }
    }
    if let (Some(bc), t) = (node.paint.border_brush, node.paint.border_thickness)
        && t > 0.0
    {
        brush.set_color(linear(bc));
        let inset = inset_rect(rect, t / 2.0);
        if radius > 0.0 {
            session.draw_rounded_rect(&RoundedRect::uniform(inset, radius), brush, t);
        } else {
            session.draw_rect(&inset, brush, t);
        }
    }
}

fn paint_ellipse(
    session: &DrawingSession,
    brush: &windows_canvas_core::Brush,
    node: &Node,
    rect: Rect,
) {
    let e = Ellipse::new(
        Vector2::new((rect.left + rect.right) / 2.0, (rect.top + rect.bottom) / 2.0),
        rect.width() / 2.0,
        rect.height() / 2.0,
    );
    if let Some(fill) = node.paint.fill {
        brush.set_color(linear(fill));
        session.fill_ellipse(&e, brush);
    }
    if let (Some(stroke), t) = (node.paint.stroke, node.paint.stroke_thickness)
        && t > 0.0
    {
        brush.set_color(linear(stroke));
        session.draw_ellipse(&e, brush, t);
    }
}

fn paint_line(
    session: &DrawingSession,
    brush: &windows_canvas_core::Brush,
    node: &Node,
    rect: Rect,
) {
    let l = node.paint.line;
    let t = if node.paint.stroke_thickness > 0.0 {
        node.paint.stroke_thickness
    } else {
        1.0
    };
    // Un-styled lines take the themable strong stroke (the nearest Fluent role
    // to the old mid-grey literal), keeping every default color host-restylable.
    let color = node.paint.stroke.unwrap_or_else(theme::stroke_strong);
    brush.set_color(linear(color));
    session.draw_line(
        Vector2::new(rect.left + l.x1 as f32, rect.top + l.y1 as f32),
        Vector2::new(rect.left + l.x2 as f32, rect.top + l.y2 as f32),
        brush,
        t,
    );
}

fn inset_rect(r: Rect, by: f32) -> Rect {
    Rect::new(r.left + by, r.top + by, r.right - by, r.bottom - by)
}
