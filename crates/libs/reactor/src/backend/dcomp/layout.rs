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
use windows_numerics::{Vector2, Vector3};

/// Compute layout for the tree rooted at `root` into a `width` x `height` (DIP)
/// box, pushing each node's offset/size onto its container and recording its
/// absolute [`LaidRect`](super::node::LaidRect) for hit-testing.
pub(crate) fn compute(arena: &mut Arena, root: ControlId, width: f32, height: f32) {
    rebuild_text(arena, root);
    resolve_align(arena, root, false, false);

    let mut tree: TaffyTree<ControlId> = TaffyTree::new();
    let root_taffy = build(arena, &mut tree, root);

    let _ = tree.compute_layout_with_measure(
        root_taffy,
        Size {
            width: AvailableSpace::Definite(width),
            height: AvailableSpace::Definite(height),
        },
        |known, _available, _node_id, ctx, _style| {
            if let (Some(w), Some(h)) = (known.width, known.height) {
                return Size { width: w, height: h };
            }
            if let Some(id) = ctx
                && let Some(node) = arena.get(*id)
                && let Some(layout) = &node.text_layout
                && let Ok((tw, th)) = layout.measure()
            {
                return Size {
                    width: known.width.unwrap_or(tw),
                    height: known.height.unwrap_or(th),
                };
            }
            Size {
                width: known.width.unwrap_or(0.0),
                height: known.height.unwrap_or(0.0),
            }
        },
    );

    assign(arena, &tree, root, 0.0, 0.0);
    sync(arena, root);
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
        let is_grid = n.kind == ControlKind::Grid;
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
    // Single line, generous box; intrinsic width comes from `measure()`.
    let layout = TextLayout::new(text, &fmt, 100_000.0, 100_000.0).ok()?;
    let _ = layout.set_word_wrap(wrap);
    Some(layout)
}

/// Build a Taffy node (and its subtree) from the arena, recording the mapping in
/// each node's `taffy_id` and applying grid templates to grid containers.
fn build(arena: &mut Arena, tree: &mut TaffyTree<ControlId>, id: ControlId) -> NodeId {
    let children = arena.get(id).map(|n| n.children.clone()).unwrap_or_default();
    let child_ids: Vec<NodeId> = children.iter().map(|c| build(arena, tree, *c)).collect();

    let style = finalize_style(arena.get(id).unwrap());
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
        GridLength::Star(f) => fr(*f as f32),
    })
}

/// Walk the computed tree: write each node's absolute (window-relative) rect for
/// hit-testing and push its relative offset + size onto its container visual.
fn assign(arena: &mut Arena, tree: &TaffyTree<ControlId>, id: ControlId, ox: f32, oy: f32) {
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
    let (w, h) = (l.size.width, l.size.height);
    if let Some(n) = arena.get_mut(id) {
        let resized = (n.rect.w, n.rect.h) != (w, h);
        n.rect = LaidRect { x: ax, y: ay, w, h };
        // The compositor moves/sizes the node — no repaint for a move; a size
        // change does need the node's surface rebuilt at the new pixel extent.
        let _ = n.vis.SetOffset(Vector3::new(rel.0, rel.1, 0.0));
        let _ = n.vis.SetSize(Vector2::new(w, h));
        if resized {
            n.mark_dirty();
            // Notify any viz host (SurfacePainter / composition surface) bound to
            // this node's container that its size changed (the DComp analogue of
            // XAML's FrameworkElement.SizeChanged). Skip the identity cast entirely
            // when nothing is subscribed (the common case).
            if size::has_listeners()
                && let Some(key) = size::container_key(&n.container)
            {
                size::fire_element_size(key, w, h);
            }
        }
    }
    for c in &children {
        assign(arena, tree, *c, ax, ay);
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
        if let Some(n) = arena.get_mut(id) {
            n.ctrl.content_h = content_h;
            n.anim.target = n.anim.target.clamp(0.0, max_scroll);
            n.anim.x = n.anim.x.clamp(0.0, max_scroll);
        }
        let scroll = arena.get(id).map(|n| n.anim.x).unwrap_or(0.0);
        for c in &children {
            if let Some(cn) = arena.get_mut(*c) {
                let _ = cn.vis.SetOffset(Vector3::new(
                    cn.rect.x - nx,
                    cn.rect.y - ny - scroll,
                    0.0,
                ));
            }
        }
    }
}

/// Re-sync the composition child order for any node whose children list or a
/// child's Z-order changed: rebuild the collection as `[own surface (bottom),
/// children sorted by (z, doc order)]`. Retained visuals are merely re-parented,
/// not recreated.
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

        if let Some(coll) = arena.get(id).and_then(|n| n.container.Children().ok()) {
            let _ = coll.RemoveAll();
            if let Some(sp) = &surf_sprite {
                let _ = coll.InsertAtBottom(sp);
            }
            for (_, _, v) in &kids {
                let _ = coll.InsertAtTop(v);
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
