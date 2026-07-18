//! Layout: reactor props (already folded into each [`Node`]'s `taffy::Style`)
//! -> a Taffy tree rebuilt from the arena -> per-node composition offset/size and
//! an absolute DIP rect for hit-testing. Alignment is resolved against each
//! node's parent (Grid vs Flex axis) before the tree is built, and after layout
//! the composition child order is re-synced for any node whose children changed.
//! Text intrinsic sizing is fed to Taffy by a measure callback reading each text
//! node's cached DirectWrite [`TextLayout`].

use super::node::{Arena, LaidRect, Node};
use super::*;
use crate::backend::ControlKind;
use crate::style::GridLength;
use taffy::prelude::*;
use windows_canvas_core::{TextFormat, TextLayout};
use windows_core::Interface;

/// Compute layout for the tree rooted at `root` into a `width` x `height` (DIP)
/// box, pushing each node's offset/size onto its container and recording its
/// absolute [`LaidRect`](super::node::LaidRect) for hit-testing.
///
/// `scale` is the DIP→physical-pixel factor (dpi/96): every assigned rect is
/// snapped to the physical pixel grid (WinUI's `UseLayoutRounding`). The root
/// visual carries a `SetScale(scale)`, so a fractional DIP offset lands between
/// physical pixels and the compositor bilinear-resamples the whole surface —
/// text and hairlines blur. Snapping in DIP space by the physical grid keeps
/// every surface on integer pixels.
pub(crate) fn compute(arena: &mut Arena, root: ControlId, width: f32, height: f32, scale: f32) {
    rebuild_text(arena, root);
    resolve_align(arena, root, false, false);

    let mut tree: TaffyTree<ControlId> = TaffyTree::new();
    let root_taffy = build(arena, &mut tree, root, false);

    // Taffy sizes a *root* node with `size: auto` to its content, not to the
    // available space — so a full-bleed reactor root (default alignment Stretch,
    // no explicit size) would collapse to 0 on both axes and drag the whole tree
    // (every Star track resolves against 0) down with it. Mirror WinUI's
    // "root fills the window" by wrapping the real root in a synthetic 1×1
    // Star×Star grid cell sized to the viewport: a stretch grid item with
    // `size: auto` fills the cell, while an item with a fixed size or an explicit
    // non-stretch alignment is honoured — and its margin insets it correctly
    // (`percent(1.0)` would overflow by the margin and ignore the offset).
    //
    // The cell uses `flex(1.0)` (`minmax(0, 1fr)`), **not** bare `fr(1.0)`: an `fr`
    // track has a min-content floor, so a root whose content (a long scrollable
    // chain) exceeds the window would inflate the cell to its content height
    // instead of clamping to the viewport — every descendant Star then resolves
    // against the inflated height and inner ScrollViewers never receive a bounded
    // extent. The zero-floor cell clamps the root to the window; overflow scrolls.
    let viewport = {
        let mut s = Style {
            display: Display::Grid,
            size: Size {
                width: length(width),
                height: length(height),
            },
            ..Style::default()
        };
        s.grid_template_columns = vec![flex(1.0)];
        s.grid_template_rows = vec![flex(1.0)];
        tree.new_with_children(s, &[root_taffy]).unwrap()
    };

    let _ = tree.compute_layout_with_measure(
        viewport,
        Size {
            width: AvailableSpace::Definite(width),
            height: AvailableSpace::Definite(height),
        },
        |known, available, _node_id, ctx, _style| {
            if let (Some(w), Some(h)) = (known.width, known.height) {
                return Size { width: w, height: h };
            }
            if let Some(id) = ctx
                && let Some(node) = arena.get(*id)
                && let Some(layout) = &node.text_layout
            {
                // A wrapping run reflows against whatever width Taffy is asking
                // about, so `measure()` below reports the WRAPPED height instead of
                // the one-line height of the layout's construction box. Without
                // this the box stays as `build_text_layout` made it and
                // `set_word_wrap` is inert — a `.wrap()` paragraph measures (and so
                // paints) as one long line and overflows its parent.
                //
                // The min-content mapping is exact rather than approximate: wrapping
                // into a zero-width box breaks at every opportunity, so the reported
                // width is the longest single word — which is min-content's
                // definition. Max-content is the unconstrained single line.
                //
                // Non-wrapping runs keep the construction box untouched: their
                // intrinsic width IS the measurement, and pills/labels size to it.
                if node.paint.wrap {
                    let constraint = known.width.or(match available.width {
                        AvailableSpace::Definite(w) => Some(w),
                        AvailableSpace::MinContent => Some(0.0),
                        AvailableSpace::MaxContent => None,
                    });
                    let _ = layout.set_max_width(constraint.unwrap_or(f32::INFINITY));
                }
                if let Ok((tw, th)) = layout.measure() {
                    // A SelectorBar's intrinsic size: each segment is its own
                    // measured label + padding, side by side inside the tray
                    // inset. The cached layout supplies the line height.
                    if node.kind == ControlKind::SelectorBar {
                        let m = controls::seg_metrics(node.paint.style_variant, node.paint.font_size);
                        let labels: f32 = node.ctrl.seg_label_w.iter().sum();
                        let n = node.ctrl.seg_label_w.len().max(1) as f32;
                        return Size {
                            width: known
                                .width
                                .unwrap_or(labels + n * 2.0 * m.pad_x + 2.0 * m.tray),
                            height: known.height.unwrap_or(th + 2.0 * (m.pad_y + m.tray)),
                        };
                    }
                    return Size {
                        width: known.width.unwrap_or(tw),
                        height: known.height.unwrap_or(th),
                    };
                }
            }
            Size {
                width: known.width.unwrap_or(0.0),
                height: known.height.unwrap_or(0.0),
            }
        },
    );

    assign(arena, &tree, root, 0.0, 0.0, 0.0, 0.0, scale.max(0.01));
    sync(arena, root);
}

/// Snap a DIP coordinate to the physical pixel grid.
#[inline]
pub(crate) fn snap(v: f32, scale: f32) -> f32 {
    (v * scale).round() / scale
}

/// WinRT alignment (`Left/Top`=0, `Center`=1, `Right/Bottom`=2, `Stretch`=3) to a
/// Taffy align value. `-1` (unset) yields `None` (inherit container default).
fn align(v: i32) -> Option<AlignItems> {
    match v {
        0 => Some(AlignItems::START),
        1 => Some(AlignItems::CENTER),
        2 => Some(AlignItems::END),
        3 => Some(AlignItems::STRETCH),
        _ => None,
    }
}

/// Resolve each node's `align_self`/`justify_self` from its requested H/V
/// alignment and its parent's layout kind: in a Grid both axes are per-child; in
/// a Flex row the cross axis is vertical (V→align_self); in a Flex column the
/// cross axis is horizontal (H→align_self). Main-axis flex alignment is the
/// parent's `justify_content` and is not expressible per child here.
fn resolve_align(arena: &mut Arena, id: ControlId, parent_grid: bool, parent_row: bool) {
    let (children, is_grid, is_row) = {
        let Some(n) = arena.get_mut(id) else { return };
        if parent_grid {
            if let Some(a) = align(n.h_align) {
                n.style.justify_self = Some(a);
            }
            if let Some(a) = align(n.v_align) {
                n.style.align_self = Some(a);
            }
        } else if parent_row {
            if let Some(a) = align(n.v_align) {
                n.style.align_self = Some(a);
            }
        } else if let Some(a) = align(n.h_align) {
            n.style.align_self = Some(a);
        }
        // Grid *alignment* semantics (h→justify_self, v→align_self) apply to any
        // node laid out as a Taffy grid — that includes `Border` (a one-cell grid
        // host), not just `ControlKind::Grid`. Keying on ControlKind here left a
        // Border's child on flex-column semantics (h→align_self), which in a grid
        // is the vertical axis — so a max-width Center child centered vertically
        // but left-aligned horizontally.
        let is_grid = matches!(n.style.display, Display::Grid);
        let is_row = matches!(n.style.display, Display::Flex)
            && matches!(
                n.style.flex_direction,
                FlexDirection::Row | FlexDirection::RowReverse
            );
        (n.children.clone(), is_grid, is_row)
    };
    for c in children {
        resolve_align(arena, c, is_grid, is_row);
    }
}

/// (Re)build the DirectWrite layout for any text-bearing node flagged dirty.
fn rebuild_text(arena: &mut Arena, id: ControlId) {
    let children = match arena.get(id) {
        Some(n) => n.children.clone(),
        None => return,
    };
    let needs = arena.get(id).is_some_and(|n| n.text_dirty && is_text(n));
    if needs {
        let (text, size, weight, family, wrap) = {
            let n = arena.get(id).unwrap();
            (
                n.paint.text.clone(),
                n.paint.font_size,
                n.paint.font_weight,
                n.paint.font_family.clone().unwrap_or_else(|| "Segoe UI".to_string()),
                n.paint.wrap,
            )
        };
        let layout = build_text_layout(&text, size, weight, &family, wrap);
        if let Some(n) = arena.get_mut(id) {
            n.text_layout = layout;
            n.text_dirty = false;
        }
    }
    // A SelectorBar measures every item label (each segment sizes to its own
    // label) and caches one layout as `text_layout` so the measure callback has
    // the line height. Measured at the active weight (600) so widths hold when
    // any segment becomes active.
    let needs_seg = arena.get(id).is_some_and(|n| {
        n.text_dirty && n.kind == ControlKind::SelectorBar && !n.ctrl.items.is_empty()
    });
    if needs_seg {
        let (items, size, family) = {
            let n = arena.get(id).unwrap();
            (
                n.ctrl.items.clone(),
                n.paint.font_size,
                n.paint.font_family.clone().unwrap_or_else(|| "Segoe UI".to_string()),
            )
        };
        let mut widths = Vec::with_capacity(items.len());
        let mut keep: Option<TextLayout> = None;
        for item in &items {
            let mut w = 0.0f32;
            if let Some(l) = build_text_layout(item, size, 600, &family, false) {
                if let Ok((lw, _)) = l.measure() {
                    w = lw;
                }
                keep = Some(l);
            }
            widths.push(w);
        }
        if let Some(n) = arena.get_mut(id) {
            n.ctrl.seg_label_w = widths;
            n.text_layout = keep;
            n.text_dirty = false;
        }
    }
    for c in children {
        rebuild_text(arena, c);
    }
}

fn is_text(n: &Node) -> bool {
    matches!(n.kind, ControlKind::TextBlock | ControlKind::Button) && !n.paint.text.is_empty()
}

fn build_text_layout(
    text: &str,
    size: f32,
    weight: u16,
    family: &str,
    wrap: bool,
) -> Option<TextLayout> {
    let fmt = TextFormat::with_weight(family, size, windows_canvas_core::FontWeight(weight as i32))
        .ok()?;
    // A generous construction box, i.e. the run's max-content state: unconstrained,
    // so `measure()` reports the intrinsic single-line size. A non-wrapping run is
    // measured and painted in exactly this state. A wrapping one is re-flowed
    // against a real width by the measure callback, and pinned to its final laid
    // width by `assign` — see both for why the box does not start at the constraint.
    let layout = TextLayout::new(text, &fmt, 100_000.0, 100_000.0).ok()?;
    let _ = layout.set_word_wrap(wrap);
    Some(layout)
}

/// Build a Taffy node (and its subtree) from the arena, recording the mapping in
/// each node's `taffy_id` and applying grid templates to grid containers.
///
/// `hidden` is set for the body subtree of a collapsed [`Expander`]: such nodes
/// map to `Display::None` so the body reclaims its layout space (height 0) while
/// staying mounted (its visuals collapse to 0×0). The flag is sticky — it
/// propagates to the whole subtree so every descendant collapses with it.
fn build(arena: &mut Arena, tree: &mut TaffyTree<ControlId>, id: ControlId, hidden: bool) -> NodeId {
    let children = arena.get(id).map(|n| n.children.clone()).unwrap_or_default();
    // A collapsed Expander hides its body children (the header is drawn on the
    // node's own surface, so every child is body content).
    let collapse_children = arena
        .get(id)
        .is_some_and(|n| n.kind == ControlKind::Expander && !n.ctrl.expanded);
    let child_ids: Vec<NodeId> = children
        .iter()
        .map(|c| build(arena, tree, *c, hidden || collapse_children))
        .collect();

    let mut style = finalize_style(arena.get(id).unwrap());
    if hidden {
        style.display = Display::None;
    }
    let taffy_id = if child_ids.is_empty() {
        tree.new_leaf_with_context(style, id).unwrap()
    } else {
        let nid = tree.new_with_children(style, &child_ids).unwrap();
        tree.set_node_context(nid, Some(id)).unwrap();
        nid
    };
    if let Some(n) = arena.get_mut(id) {
        n.taffy_id = Some(taffy_id);
    }
    taffy_id
}

/// Produce the final Taffy style for a node: its accumulated `style` plus grid
/// track templates for a `Grid` (the only style derived lazily from props).
fn finalize_style(node: &Node) -> Style {
    let mut s = node.style.clone();
    if node.kind == ControlKind::Grid {
        if !node.grid_rows.is_empty() {
            s.grid_template_rows = node.grid_rows.iter().map(track).collect();
        }
        if !node.grid_cols.is_empty() {
            s.grid_template_columns = node.grid_cols.iter().map(track).collect();
        }
    }
    s
}

fn track(g: &GridLength) -> GridTemplateComponent<String> {
    GridTemplateComponent::Single(match g {
        GridLength::Auto => auto(),
        GridLength::Pixel(p) => length(*p as f32),
        // WinUI `Star` (`*`) divides the *available* track space with a **zero**
        // minimum: a tall child overflows (and an inner ScrollViewer clips/scrolls)
        // rather than inflating the track. Taffy's bare `fr()` is CSS `minmax(auto,
        // 1fr)` — a min-content floor that lets a tall child (a long processor
        // chain) inflate its row and starve the sibling Star row to its min-content
        // (collapsing the analyzer/viz panel above it). `flex()` is `minmax(0, Nfr)`,
        // the exact Star semantics, so equal Star tracks stay equal under overflow.
        GridLength::Star(f) => flex(*f as f32),
    })
}

/// Walk the computed tree: write each node's absolute (window-relative) rect for
/// hit-testing and push its relative offset + size onto its container visual.
///
/// `(ox, oy)` is the parent's raw (Taffy) absolute origin — children accumulate
/// raw positions so rounding never drifts down the tree — while `(sox, soy)` is
/// the parent's *snapped* absolute origin: the visual's relative offset is the
/// difference of snapped absolutes, so each node lands exactly on its snapped
/// absolute rect on screen. Sizes snap by edge (`right - left`), keeping shared
/// edges between siblings coincident.
#[allow(clippy::too_many_arguments)]
fn assign(
    arena: &mut Arena,
    tree: &TaffyTree<ControlId>,
    id: ControlId,
    ox: f32,
    oy: f32,
    sox: f32,
    soy: f32,
    scale: f32,
) {
    let (taffy_id, children) = match arena.get(id) {
        Some(n) => (n.taffy_id, n.children.clone()),
        None => return,
    };
    let Some(taffy_id) = taffy_id else { return };
    let l = match tree.layout(taffy_id) {
        Ok(l) => *l,
        Err(_) => return,
    };
    let rel = (l.location.x, l.location.y);
    let (ax, ay) = (ox + rel.0, oy + rel.1);
    let (aw, ah) = (l.size.width, l.size.height);
    // Snapped absolute rect (edge-snapped so adjacent siblings stay flush).
    let (sx, sy) = (snap(ax, scale), snap(ay, scale));
    let w = snap(ax + aw, scale) - sx;
    let h = snap(ay + ah, scale) - sy;
    if let Some(n) = arena.get_mut(id) {
        let resized = (n.rect.w, n.rect.h) != (w, h);
        n.rect = LaidRect { x: sx, y: sy, w, h };
        // The compositor moves/sizes the node — no repaint for a move; a size
        // change does need the node's surface rebuilt at the new pixel extent.
        // Pushes are change-gated (and, when the node declares a layout
        // animation or translation transition, an actual change GLIDES — the
        // implicit animation triggers on the compositor, no ticks here).
        n.push_offset(sx - sox, sy - soy);
        n.push_size(w, h);
        if resized {
            n.mark_dirty();
        }
        // Pin a wrapping run's reflow box to the width it was actually given. This
        // is the single authoritative writer of that width: the measure callback
        // probes a node several times per pass (min-content, max-content, then the
        // definite size), and since the `TextLayout` is shared mutable COM state,
        // whichever probe ran last would otherwise decide how paint reflows. Here
        // the final answer is known, so paint needs no constraint logic of its own.
        // Re-flowing changes which glyphs land where without necessarily changing
        // the node's box, so this repaints on a width change that `resized` misses.
        if n.paint.wrap
            && let Some(layout) = &n.text_layout
            && layout.metrics().is_ok_and(|m| m.layout_width != w)
        {
            let _ = layout.set_max_width(w);
            n.mark_dirty();
        }
        // Notify any viz host (SurfacePainter / composition surface) bound to
        // this node's container of its laid-out size (the DComp analogue of
        // XAML's FrameworkElement.SizeChanged). Fired every pass — NOT gated on
        // `resized` — because a listener may register after this node's size
        // already settled (mount callbacks vs layout ordering); the registry
        // change-gates per listener, so an unchanged size is a cheap no-op and a
        // fresh listener is guaranteed its first delivery on the next pass. Skip
        // the lookup entirely when nothing is subscribed (the common case).
        if size::has_listeners() {
            size::fire_element_size(id, w, h);
        }
    }
    for c in &children {
        assign(arena, tree, *c, ax, ay, sx, sy, scale);
    }

    // Scroll containers: measure content extent, clamp the offset, and apply the
    // scroll translation to children (a compositor offset — no repaint).
    let is_scroll = arena.get(id).is_some_and(|n| n.is_scroll());
    if is_scroll {
        let (nx, ny, vh) = arena.get(id).map(|n| (n.rect.x, n.rect.y, n.rect.h)).unwrap();
        let mut content_h = 0.0_f32;
        for c in &children {
            if let Some(cn) = arena.get(*c) {
                content_h = content_h.max(cn.rect.y + cn.rect.h - ny);
            }
        }
        let max_scroll = (content_h - vh).max(0.0);
        // Children are placed UNSCROLLED; the scroll translation lives on the
        // content carrier visual they parent into. Snap it: rects are
        // pixel-snapped, so a fractional offset would push every child back
        // off the grid.
        let scroll = if let Some(n) = arena.get_mut(id) {
            n.ctrl.content_h = content_h;
            n.scroll_off = snap(n.scroll_off.clamp(0.0, max_scroll), scale);
            n.scroll_off
        } else {
            0.0
        };
        for c in &children {
            if let Some(cn) = arena.get_mut(*c) {
                let (x, y) = (cn.rect.x - nx, cn.rect.y - ny);
                cn.push_offset(x, y);
            }
        }
        // Layout is placement, not motion: snap the carrier (gated inside, so
        // an unchanged pass costs nothing and a glide already heading to this
        // same target keeps flying).
        if let Some(n) = arena.get_mut(id) {
            n.scroll_snap(scroll);
        }
    }
}

/// Re-sync the composition child order for any node whose children list or a
/// child's Z-order changed: rebuild the collection as `[below-band chrome
/// parts, own surface, children sorted by (z, doc order), above-band chrome
/// parts, scroll thumb]`. Retained visuals are merely re-parented, not
/// recreated.
fn sync(arena: &mut Arena, id: ControlId) {
    let children = match arena.get(id) {
        Some(n) => n.children.clone(),
        None => return,
    };
    let any_z = children
        .iter()
        .any(|c| arena.get(*c).is_some_and(|cn| cn.z_dirty));
    let need = arena.get(id).is_some_and(|n| n.children_dirty) || any_z;

    if need {
        let mut kids: Vec<(i32, usize, crate::system_bindings::Visual)> = Vec::new();
        for (i, c) in children.iter().enumerate() {
            if let Some(cn) = arena.get(*c)
                && let Ok(v) = cn.container.cast::<crate::system_bindings::Visual>()
            {
                kids.push((cn.z_index, i, v));
            }
        }
        kids.sort_by_key(|(z, i, _)| (*z, *i));

        let surf_sprite = arena
            .get(id)
            .and_then(|n| n.surf.as_ref())
            .and_then(|s| s.sprite.cast::<crate::system_bindings::Visual>().ok());

        // The scroll thumb is an overlay sprite (a top child not tracked in the
        // arena children); preserve it above the re-synced content.
        let thumb_sprite = arena
            .get(id)
            .and_then(|n| n.scroll_thumb.as_ref())
            .and_then(|s| s.sprite.cast::<crate::system_bindings::Visual>().ok());

        // Scroll containers parent their children into the content CARRIER
        // visual (whose Offset is the animated scroll translation); everyone
        // else parents children directly.
        let carrier = arena.get(id).and_then(|n| n.scroll_content.clone());

        if let Some(coll) = arena.get(id).and_then(|n| n.container.Children().ok()) {
            let _ = coll.RemoveAll();
            // Sequential InsertAtTop stacks in call order, bottom → top.
            if let Some(n) = arena.get(id)
                && let Some(parts) = n.parts.as_deref()
            {
                for v in parts.below_visuals() {
                    let _ = coll.InsertAtTop(&v);
                }
            }
            if let Some(sp) = &surf_sprite {
                let _ = coll.InsertAtTop(sp);
            }
            match &carrier {
                Some(content) => {
                    if let Ok(cv) = content.cast::<Visual>() {
                        let _ = coll.InsertAtTop(&cv);
                    }
                    if let Ok(cc) = content.Children() {
                        let _ = cc.RemoveAll();
                        for (_, _, v) in &kids {
                            let _ = cc.InsertAtTop(v);
                        }
                    }
                }
                None => {
                    for (_, _, v) in &kids {
                        let _ = coll.InsertAtTop(v);
                    }
                }
            }
            if let Some(n) = arena.get(id)
                && let Some(parts) = n.parts.as_deref()
            {
                for v in parts.above_visuals() {
                    let _ = coll.InsertAtTop(&v);
                }
            }
            if let Some(tp) = &thumb_sprite {
                let _ = coll.InsertAtTop(tp);
            }
        }

        if let Some(n) = arena.get_mut(id) {
            n.children_dirty = false;
        }
        for c in &children {
            if let Some(cn) = arena.get_mut(*c) {
                cn.z_dirty = false;
            }
        }
    }

    for c in children {
        sync(arena, c);
    }
}
