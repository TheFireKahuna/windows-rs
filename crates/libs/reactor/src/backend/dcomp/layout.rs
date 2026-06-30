//! Layout: reactor props (already folded into each [`Node`]'s `taffy::Style`)
//! -> a Taffy tree rebuilt from the arena -> absolute DIP rects written back to
//! each node. Text intrinsic sizing is fed to Taffy by a measure callback that
//! reads each text node's cached DirectWrite [`TextLayout`].

use super::node::{Arena, LaidRect, Node};
use super::*;
use crate::backend::ControlKind;
use crate::style::GridLength;
use taffy::prelude::*;
use windows_canvas_core::{TextFormat, TextLayout};

/// Compute layout for the tree rooted at `root` into a `width` x `height` (DIP)
/// box, writing each node's absolute [`LaidRect`](super::node::LaidRect).
pub(crate) fn compute(arena: &mut Arena, root: ControlId, width: f32, height: f32) {
    rebuild_text(arena, root);

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
}

/// (Re)build the DirectWrite layout for any text-bearing node flagged dirty.
fn rebuild_text(arena: &mut Arena, id: ControlId) {
    let children = match arena.get(id) {
        Some(n) => n.children.clone(),
        None => return,
    };
    let needs = arena.get(id).is_some_and(|n| n.text_dirty && is_text(n));
    if needs {
        let (text, size, weight, family) = {
            let n = arena.get(id).unwrap();
            (
                n.paint.text.clone(),
                n.paint.font_size,
                n.paint.font_weight,
                n.paint.font_family.clone().unwrap_or_else(|| "Segoe UI".to_string()),
            )
        };
        let layout = build_text_layout(&text, size, weight, &family);
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

fn build_text_layout(text: &str, size: f32, weight: u16, family: &str) -> Option<TextLayout> {
    let fmt = TextFormat::with_weight(
        family,
        size,
        windows_canvas_core::FontWeight(weight as i32),
    )
    .ok()?;
    // Single line, generous box; intrinsic width comes from `measure()`.
    let layout = TextLayout::new(text, &fmt, 100_000.0, 100_000.0).ok()?;
    let _ = layout.set_word_wrap(false);
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

/// Walk the computed tree, writing each node's absolute (window-relative) rect.
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
    let ax = ox + l.location.x;
    let ay = oy + l.location.y;
    if let Some(n) = arena.get_mut(id) {
        n.rect = LaidRect {
            x: ax,
            y: ay,
            w: l.size.width,
            h: l.size.height,
        };
    }
    for c in children {
        assign(arena, tree, c, ax, ay);
    }
}
