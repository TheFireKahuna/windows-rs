//! The per-node sync walk: the pass that reconciles every dirty node's
//! **retained** appearance against the state the reconciler just wrote.
//!
//! Nothing here draws. Every control's chrome is compositor objects — nine-grid
//! parts ([`parts`](super::parts)), vector sprite shapes
//! ([`path_shape`](super::path_shape)) and glyph sprites
//! ([`glyph_text`](super::glyph_text)) — so this walk's whole job is to visit
//! the nodes that changed and let each of those modules self-gate. A node that
//! is dirty but genuinely unchanged issues no COM calls at all.
//!
//! The module is the descendant of a per-node painter: every node with chrome
//! used to own an FP16 `CompositionDrawingSurface` sized to its rect, which this
//! walk minted, resized and re-entered `BeginDraw` on. The surfaces are gone and
//! so is the drawing; what survives is the traversal and the dirty gate.

use super::bootstrap::Compositing;
use super::node::Arena;
use super::*;
use crate::backend::ControlKind;

/// The shape kinds whose appearance is retained vector layers
/// ([`super::path_shape`]) rather than a plan of nine-grid parts.
///
/// A `Rectangle` is NOT one of them: it derives a box, and a box is what the
/// atlas hands out. See [`super::path_shape::derived_geometry`].
fn is_shape(kind: ControlKind) -> bool {
    matches!(kind, ControlKind::Path | ControlKind::Ellipse | ControlKind::Line)
}

/// Walk the tree, reconciling each dirty node's retained chrome.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_tree(
    comp: &Compositing,
    atlas: &mut parts::Atlas,
    glyphs: &mut glyph_text::Atlases,
    arena: &mut Arena,
    root: ControlId,
    scale: f32,
    scrubbing: bool,
) {
    sync_node(comp, atlas, glyphs, arena, root, scale, scrubbing);
}

#[allow(clippy::too_many_arguments)]
fn sync_node(
    comp: &Compositing,
    atlas: &mut parts::Atlas,
    glyphs: &mut glyph_text::Atlases,
    arena: &mut Arena,
    id: ControlId,
    scale: f32,
    scrubbing: bool,
) {
    // Which nodes have retained chrome to reconcile. `converted` covers every
    // plan-driven and plan-less kind; the three named below render themselves
    // through a module of their own and would otherwise never be visited at
    // all — silently, with no compiler error, rendering nothing.
    let needs = arena.get(id).is_some_and(|n| {
        parts::converted(n.kind)
            // Prose: retained glyph sprites (`glyph_text::text_sync`).
            || n.kind == ControlKind::TextBlock
            // Geometry: retained vector layers (`path_shape::sync_path`).
            || is_shape(n.kind)
            // Dial: groove, ticks, hub, arc and needle (`knob::sync_knob`),
            // plus four runs of its own.
            || n.kind == ControlKind::Knob
    });
    if needs {
        let (w, h) = arena.get(id).map_or((0.0, 0.0), |n| (n.rect.w, n.rect.h));

        // Where a caption band's Content slot landed, in the band's own space —
        // read BEFORE the mutable borrow below, because it lives on a different
        // node. It is the far edge of the title's grid track, so it is what the
        // drawn title ellipsizes against; re-deriving the track here instead
        // would be this module second-guessing the layout that just ran.
        let caption_content_left = arena
            .get(id)
            .filter(|n| n.kind == ControlKind::TitleBar)
            .and_then(|n| n.header_slot.map(|c| (n.rect.x, c)))
            .and_then(|(bx, c)| arena.get(c).map(|c| c.rect.x - bx));

        let dirty = arena.get(id).is_some_and(|n| n.dirty);
        if dirty && w > 0.0 && h > 0.0
            && let Some(n) = arena.get_mut(id) {
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
                glyph_text::button_sync(comp, glyphs, n, scale);
                // The same, for the one control that is only text.
                glyph_text::text_sync(comp, glyphs, n, scale);
                // …and for the one that is only text plus a focus ring.
                glyph_text::hyperlink_sync(comp, glyphs, n, scale);
                // The two controls whose words come one PER ITEM rather than
                // one per node: a switch's state label, a bar's segment labels.
                glyph_text::toggle_sync(comp, glyphs, n, scale);
                glyph_text::segmented_sync(comp, glyphs, n, scale);
                // …and the count on a badge's plate.
                glyph_text::info_badge_sync(comp, glyphs, n, scale);
                // …and the pane's two chrome glyphs, its header, and the glyph
                // plus label on every row it has room for.
                glyph_text::nav_sync(comp, glyphs, n, scale);
                // The bar's severity glyph, its wrapped paragraph and its close
                // glyph — the paragraph re-pinned to its column first.
                glyph_text::info_bar_sync(comp, glyphs, n, scale);
                // The caption's coupled title/subtitle pair and its four button
                // glyphs.
                glyph_text::caption_sync(comp, glyphs, n, scale, caption_content_left);
                // …and a checkbox's trailing label.
                glyph_text::check_sync(comp, glyphs, n, scale);
                // …and an expander's header label plus its chevron.
                glyph_text::expander_sync(comp, glyphs, n, scale);
                // …and a select trigger's current label plus its chevron, above
                // the box and border it keeps on its surface.
                glyph_text::select_sync(comp, glyphs, n, scale);
                // Editors: the text run, its selection and its composition rule
                // as sprites, then the caret sprite. Both are placed from the
                // same `editor::TextBand`, so they cannot disagree.
                if n.editor.is_some() {
                    glyph_text::editor_sync(comp, glyphs, n, scale);
                    parts::sync_caret(comp, atlas, n, scale);
                }
                // Knob: reconcile the value-arc shape + needle (its own retained
                // vector chrome, outside the flat `Part` model), then its dial
                // text — which shapes here rather than in the layout pass,
                // because its type sizes come from the radius the solve decided.
                if n.kind == crate::backend::ControlKind::Knob {
                    knob::sync_knob(comp, n, atlas.epoch(), scale, scrubbing);
                    glyph_text::knob_sync(comp, glyphs, n, scale);
                }
                // Shapes: reconcile the retained curve layers. Same shape as
                // the knob's — vector chrome outside the flat `Part` model.
                if is_shape(n.kind) {
                    path_shape::sync_path(comp, n, atlas.epoch(), scale);
                }
            }
    }

    // Indexed rather than over a cloned child list — the clone was a heap
    // allocation per node per frame bought purely to dodge `&mut Arena`.
    let mut i = 0;
    while let Some(c) = arena.get(id).and_then(|n| n.children.get(i).copied()) {
        sync_node(comp, atlas, glyphs, arena, c, scale, scrubbing);
        i += 1;
    }

    // Overlay scrollbar thumb (above the scrolled children) for scroll containers.
    if arena.get(id).is_some_and(|n| n.is_scroll()) {
        update_scroll_thumb(comp, atlas, arena, id, scale);
    }
}

/// Resolve the auto-hiding overlay scrollbar thumb of a scroll container: its
/// geometry from the viewport and content, its reveal from the policy and the
/// tick loop's hover edge. The sprite itself is `parts::sync_scroll_thumb`'s.
fn update_scroll_thumb(
    comp: &Compositing,
    atlas: &mut parts::Atlas,
    arena: &mut Arena,
    id: ControlId,
    scale: f32,
) {
    use scroll::thumb_geom;
    let (vh, content_h, sc, shown) = match arena.get(id) {
        Some(n) => (n.rect.h, n.ctrl().content_h, n.scroll_off, n.thumb_shown),
        None => return,
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
            if let Some(t) = &mut n.scroll_thumb {
                t.set_opacity(0.0);
            }
        }
        return;
    }

    // An always-visible policy has no hover edge to ride, so overflow itself is
    // the reveal; a never-visible one conceals for the same reason. Both only
    // resolve the flag — the fade is `sync_scroll_thumb`'s, which is the single
    // writer of the thumb's opacity.
    let shown = match arena
        .get(id)
        .map(|n| scroll::reveal_policy(n.extras().v_scrollbar))
    {
        Some(scroll::Reveal::Always) => true,
        Some(scroll::Reveal::Never) => false,
        _ => shown,
    };
    if let Some(n) = arena.get_mut(id) {
        n.thumb_shown = shown;
        parts::sync_scroll_thumb(comp, atlas, n, scale, &g, shown);
    }
}
