//! Layout: reactor props (already folded into each [`Node`]'s `taffy::Style`)
//! -> a **persistent** Taffy tree kept in lock-step with the arena -> per-node
//! composition offset/size and an absolute DIP rect for hit-testing. Alignment
//! is resolved against each node's parent (Grid vs Flex axis) before the tree is
//! synced, and after layout the composition child order is re-stacked for any
//! node whose children changed. Text intrinsic sizing is fed to Taffy by a
//! measure callback reading each text node's cached DirectWrite [`TextLayout`].
//!
//! The Taffy tree lives in [`Arena::layout`](super::node::Arena) and survives
//! across passes: [`LayoutTree::walk`] creates and destroys Taffy nodes only as
//! arena nodes appear and disappear, pushes a style only when it actually
//! changed, and otherwise leaves Taffy's own dirty propagation to decide what
//! needs recomputing. Rebuilding the tree per pass — which is what this used to
//! do — threw that away and allocated a Taffy node plus a cloned `Style` per
//! arena node per frame.

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

    // The persistent tree is moved OUT of the arena for the pass: the measure
    // callback below needs `&Arena` while Taffy holds `&mut TaffyTree`, which it
    // could not have if the tree were still a field of the arena. It is put back
    // unconditionally at the end.
    //
    // If a panic unwound between the take and the restore the arena would be
    // left holding `None` while live nodes still carry a `taffy_id` into the
    // lost tree. That is survivable *because* the id is generation-stamped: a
    // fresh `LayoutTree` mints a new generation, every stale stamp mismatches,
    // and the whole tree rebuilds. Without the stamp a stale `NodeId` would
    // index a fresh Taffy slotmap and panic (or, worse, alias a live node).
    let mut lt = arena.layout.take().unwrap_or_else(LayoutTree::new);
    lay_out(arena, &mut lt, root, width, height, scale);
    // Re-stack composition children before the tree goes home: `sync` borrows
    // the arena mutably and `lt`'s scratch buffer mutably, which is only two
    // disjoint borrows while `lt` is still a local.
    let mut order = std::mem::take(&mut lt.order);
    sync(arena, root, &mut order);
    lt.order = order;
    arena.layout = Some(lt);
}

/// The Taffy half of a pass, with the tree already extracted from the arena.
fn lay_out(
    arena: &mut Arena,
    lt: &mut LayoutTree,
    root: ControlId,
    width: f32,
    height: f32,
    scale: f32,
) {
    let Some(root_taffy) = lt.walk_root(arena, root) else {
        return;
    };
    let Some(viewport) = lt.viewport(width, height, root_taffy) else {
        return;
    };
    let tree = &mut lt.tree;

    let arena_ref = &*arena;
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
                && let Some(node) = arena_ref.get(*id)
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
                        let labels: f32 = node.ctrl().seg_label_w.iter().sum();
                        let n = node.ctrl().seg_label_w.len().max(1) as f32;
                        return Size {
                            width: known
                                .width
                                .unwrap_or(labels + n * 2.0 * m.pad_x + 2.0 * m.tray),
                            height: known.height.unwrap_or(th + 2.0 * (m.pad_y + m.tray)),
                        };
                    }
                    // A ToggleSwitch's label sits AFTER the track, so its
                    // intrinsic width is the track plus the gap plus the
                    // (wider) label — without this the row clips the text.
                    if node.kind == ControlKind::ToggleSwitch {
                        return Size {
                            width: known.width.unwrap_or(
                                parts::TRACK_W + controls::TOGGLE_LABEL_GAP + tw,
                            ),
                            // Taffy clamps to the birth `min_size` (the 40x20
                            // track), so the label's line height alone is the
                            // right answer here.
                            height: known.height.unwrap_or(th),
                        };
                    }
                    // A leading icon widens the button by its box plus the gap
                    // — without this an icon button sizes to its label alone
                    // and the glyph overlaps the text.
                    let icon_w = if node.extras().icon != 0 {
                        controls::ICON_SIZE
                            + if node.paint.text.is_empty() {
                                0.0
                            } else {
                                controls::ICON_GAP
                            }
                    } else {
                        0.0
                    };
                    return Size {
                        width: known.width.unwrap_or(tw + icon_w),
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

    assign(arena, &lt.tree, root, 0.0, 0.0, 0.0, 0.0, scale.max(0.01));
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
    let (is_grid, is_row) = {
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
        (is_grid, is_row)
    };
    let mut i = 0;
    while let Some(c) = arena.get(id).and_then(|n| n.children.get(i).copied()) {
        resolve_align(arena, c, is_grid, is_row);
        i += 1;
    }
}

/// (Re)build the DirectWrite layout for any text-bearing node flagged dirty.
fn rebuild_text(arena: &mut Arena, id: ControlId) {
    if arena.get(id).is_none() {
        return;
    }
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
            // Taffy's measure cache is keyed on constraints only; a new layout
            // silently changes the answer, so the node has to be re-measured.
            n.measure_dirty = true;
        }
    }
    // A SelectorBar measures every item label (each segment sizes to its own
    // label) and caches one layout as `text_layout` so the measure callback has
    // the line height. Measured at the active weight (600) so widths hold when
    // any segment becomes active.
    let needs_seg = arena.get(id).is_some_and(|n| {
        n.text_dirty && n.kind == ControlKind::SelectorBar && !n.ctrl().items.is_empty()
    });
    if needs_seg {
        let (items, size, family) = {
            let n = arena.get(id).unwrap();
            (
                n.ctrl().items.clone(),
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
            n.ctrl_mut().seg_label_w = widths;
            n.text_layout = keep;
            n.text_dirty = false;
            n.measure_dirty = true;
        }
    }
    // A ToggleSwitch sizes to its track PLUS the wider of its two state
    // labels. The *wider*, not the current one, so flipping the switch never
    // reflows the row around it — and so one cached layout is enough.
    let needs_toggle = arena.get(id).is_some_and(|n| {
        n.text_dirty
            && n.kind == ControlKind::ToggleSwitch
            && !(n.extras().on_content.is_empty() && n.extras().off_content.is_empty())
    });
    if needs_toggle {
        let (on, off, size, family) = {
            let n = arena.get(id).unwrap();
            (
                n.extras().on_content.clone(),
                n.extras().off_content.clone(),
                n.paint.font_size.max(theme::FONT_SIZE_MD),
                n.paint.font_family.clone().unwrap_or_else(|| "Segoe UI".to_string()),
            )
        };
        let mut widest: Option<TextLayout> = None;
        let mut widest_w = -1.0f32;
        for s in [&on, &off] {
            if s.is_empty() {
                continue;
            }
            if let Some(l) = build_text_layout(s, size, 400, &family, false)
                && let Ok((w, _)) = l.measure()
                && w > widest_w
            {
                widest_w = w;
                widest = Some(l);
            }
        }
        if let Some(n) = arena.get_mut(id) {
            n.text_layout = widest;
            n.text_dirty = false;
            n.measure_dirty = true;
        }
    }
    // The caption band draws its own title/subtitle, so it lays them out here
    // too — and then re-derives the geometry that depends on their measured
    // width. See `apply_caption_metrics`.
    let needs_caption = arena
        .get(id)
        .is_some_and(|n| n.text_dirty && n.kind == ControlKind::TitleBar);
    if needs_caption {
        let built = arena.get(id).map(|n| caption::build_text(n.extras()));
        if let Some(n) = arena.get_mut(id) {
            n.caption_text = built.flatten();
            n.text_dirty = false;
            n.measure_dirty = true;
            apply_caption_metrics(n);
        }
    }
    let mut i = 0;
    while let Some(c) = arena.get(id).and_then(|n| n.children.get(i).copied()) {
        rebuild_text(arena, c);
        i += 1;
    }
}

/// Re-derive the TitleBar band's layout metrics from the caption state it now
/// holds: the `tall` band height, and the left padding that reserves the drawn
/// back button + title block.
///
/// Both are chrome geometry, not app style — the band's *right* padding has
/// always reserved the drawn window-button cluster the same way (see
/// `birth_style`), and this is the leading half of that same rule. A `Padding`
/// write from the app is therefore combined with, not replaced by, the inset:
/// the base comes from `birth_style()` so this stays idempotent no matter how
/// many times a title changes.
pub(crate) fn apply_caption_metrics(n: &mut Node) {
    if n.kind != ControlKind::TitleBar {
        return;
    }
    // Both come straight from `caption`, which is also what `birth_style`
    // builds a virgin TitleBar from — so a node whose caption state is back at
    // its defaults re-derives exactly the style it was born with.
    let pad = caption::pad_left(n.extras())
        + caption::title_block(n.caption_text.as_deref(), n.rect.w);
    n.style.min_size.height = length(caption::band_height(n.extras()));
    n.style.padding.left = length(pad);
    n.mark_dirty();
}

fn is_text(n: &Node) -> bool {
    match n.kind {
        // A Button with only an icon and no label still needs a layout: it is
        // what carries the line height, and without one the measure callback
        // has nothing to key on and the button collapses to 0x0. The empty
        // run measures zero wide, so the icon width is the whole intrinsic
        // size — which is the right answer.
        ControlKind::Button => !n.paint.text.is_empty() || n.extras().icon != 0,
        ControlKind::TextBlock => !n.paint.text.is_empty(),
        _ => false,
    }
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

/// Generation source for [`LayoutTree`]. Every tree ever built gets a distinct
/// stamp, so a [`Node::taffy_id`](super::node::Node::taffy_id) minted by one can
/// never be mistaken for a live id in another.
static LAYOUT_GEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

/// The persistent Taffy tree plus the bookkeeping that keeps it in lock-step
/// with the arena. Owned by [`Arena::layout`](super::node::Arena) so it lives
/// across layout passes; taken out for the duration of one (see [`compute`]).
pub(crate) struct LayoutTree {
    tree: TaffyTree<ControlId>,
    /// Stamp minted for this tree; mirrored into every `Node::taffy_id` it
    /// hands out. Taffy indexes its slotmap unchecked, so dereferencing an id
    /// from a *different* tree is a panic at best — the stamp makes that
    /// unrepresentable: a mismatched id is simply treated as "no node yet".
    generation: u32,
    /// Every Taffy node this tree owns, keyed by the raw `ControlId` that owns
    /// it. This is the sweep list: an entry whose id has left the arena is a
    /// dead node and its Taffy node is removed.
    owned: rustc_hash::FxHashMap<u32, NodeId>,
    /// The synthetic viewport wrapper and the two things ever pushed to it.
    viewport: Option<NodeId>,
    vp_size: (f32, f32),
    vp_child: Option<NodeId>,
    /// Reused stack of child ids, so a pass allocates nothing: each `walk`
    /// frame pushes its children's ids above `base` and truncates back on exit.
    scratch: Vec<NodeId>,
    /// Reused ordering buffer for [`sync`]'s composition re-stack. Parked here
    /// (rather than on the stack) purely so no pass allocates it afresh.
    pub(crate) order: Stack,
}

impl LayoutTree {
    fn new() -> Self {
        Self {
            tree: TaffyTree::new(),
            generation: LAYOUT_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            owned: rustc_hash::FxHashMap::default(),
            viewport: None,
            vp_size: (f32::NAN, f32::NAN),
            vp_child: None,
            scratch: Vec::new(),
            order: Vec::new(),
        }
    }

    /// Sync the whole arena subtree under `root` into the Taffy tree and sweep
    /// away the Taffy nodes of arena nodes that have since died. Returns the
    /// root's Taffy node.
    fn walk_root(&mut self, arena: &mut Arena, root: ControlId) -> Option<NodeId> {
        let mut visited = 0usize;
        let id = self.walk(arena, root, false, &mut visited);
        // A node can be alive in the arena yet unreachable from the root (the
        // reconciler legitimately parks a subtree mid-diff), so "not visited"
        // does NOT mean "dead" — the sweep re-checks the arena and keeps those.
        // The count only decides whether a sweep is worth running at all.
        if visited != self.owned.len() {
            self.sweep(arena);
        }
        id
    }

    /// Reconcile one arena node (and its subtree) with its Taffy node: create
    /// the Taffy node if it has none, push its style only if that style
    /// actually changed, re-parent its children only if the child list changed,
    /// and mark it dirty when something Taffy cannot see (a rebuilt text
    /// layout) invalidated its cached measurement.
    ///
    /// `hidden` is set for the body subtree of a collapsed [`Expander`]: such
    /// nodes map to `Display::None` so the body reclaims its layout space
    /// (height 0) while staying mounted (its visuals collapse to 0×0). The flag
    /// is sticky — it propagates to the whole subtree so every descendant
    /// collapses with it.
    fn walk(
        &mut self,
        arena: &mut Arena,
        id: ControlId,
        hidden: bool,
        visited: &mut usize,
    ) -> Option<NodeId> {
        // A collapsed Expander hides its body children (the header is drawn on
        // the node's own surface, so every child is body content).
        let collapse = arena
            .get(id)
            .is_some_and(|n| n.kind == ControlKind::Expander && !n.ctrl().expanded);
        let child_hidden = hidden || collapse;

        // Children first — their Taffy nodes must exist before `set_children`.
        // Indexed rather than over a cloned child list: the clone was a heap
        // allocation per node per frame bought purely to dodge `&mut Arena`.
        let base = self.scratch.len();
        let mut i = 0;
        while let Some(c) = arena.get(id).and_then(|n| n.children.get(i).copied()) {
            if let Some(cid) = self.walk(arena, c, child_hidden, visited) {
                self.scratch.push(cid);
            }
            i += 1;
        }

        let Some(n) = arena.get(id) else {
            self.scratch.truncate(base);
            return None;
        };
        let existing = n.taffy_id.and_then(|(g, t)| (g == self.generation).then_some(t));
        // Building the finalized style to compare it is a stack copy (plus a
        // vec clone only for a Grid that declares tracks) — cheap next to the
        // full subtree invalidation `set_style` would otherwise trigger every
        // pass. Comparing the WHOLE style with `==` rather than field by field
        // is deliberate: a future Taffy field cannot be forgotten here.
        let want = finalize_style(n, hidden);
        let remeasure = n.measure_dirty
            // `seg_metrics` reads the style variant, which is not a text prop
            // and so does not route through `text_dirty` — a SelectorBar whose
            // chrome changed re-measures with its repaint.
            || (n.kind == ControlKind::SelectorBar && n.dirty);

        let tid = match existing {
            Some(t) => {
                if self.tree.style(t).is_ok_and(|cur| *cur != want) {
                    let _ = self.tree.set_style(t, want);
                }
                t
            }
            None => {
                let Ok(t) = self.tree.new_leaf_with_context(want, id) else {
                    self.scratch.truncate(base);
                    return None;
                };
                self.owned.insert(id.get(), t);
                t
            }
        };
        if let Some(n) = arena.get_mut(id) {
            n.taffy_id = Some((self.generation, tid));
            n.measure_dirty = false;
        }
        // Taffy caches a measured leaf by its constraints alone. A rebuilt
        // DirectWrite layout changes the ANSWER for constraints it has already
        // seen, which no style edit reflects — so the cache has to be dropped
        // by hand or a relabelled node keeps its old intrinsic size forever.
        if remeasure {
            let _ = self.tree.mark_dirty(tid);
        }

        // Re-parent only on an actual change: `set_children` marks the parent
        // (and its ancestors) dirty. Compared without `TaffyTree::children`,
        // which hands back a freshly allocated Vec.
        let kids = &self.scratch[base..];
        let same = kids
            .iter()
            .enumerate()
            .all(|(i, k)| self.tree.child_at_index(tid, i).is_ok_and(|c| c == *k))
            && self.tree.child_at_index(tid, kids.len()).is_err();
        if !same {
            let _ = self.tree.set_children(tid, kids);
        }
        self.scratch.truncate(base);
        *visited += 1;
        Some(tid)
    }

    /// Drop the Taffy node of every tracked id that has left the arena.
    fn sweep(&mut self, arena: &Arena) {
        let tree = &mut self.tree;
        let mut vp_child = self.vp_child;
        self.owned.retain(|raw, nid| {
            if arena.get(ControlId::new(*raw)).is_some() {
                return true;
            }
            if vp_child == Some(*nid) {
                vp_child = None;
            }
            let _ = tree.remove(*nid);
            false
        });
        self.vp_child = vp_child;
    }

    /// The synthetic viewport wrapping the real root, created once and resized
    /// / re-parented only when the window size or the root's Taffy node change.
    ///
    /// Taffy sizes a *root* node with `size: auto` to its content, not to the
    /// available space — so a full-bleed reactor root (default alignment
    /// Stretch, no explicit size) would collapse to 0 on both axes and drag the
    /// whole tree (every Star track resolves against 0) down with it. Mirror
    /// WinUI's "root fills the window" by wrapping the real root in a synthetic
    /// 1×1 Star×Star grid cell sized to the viewport: a stretch grid item with
    /// `size: auto` fills the cell, while an item with a fixed size or an
    /// explicit non-stretch alignment is honoured — and its margin insets it
    /// correctly (`percent(1.0)` would overflow by the margin and ignore the
    /// offset).
    ///
    /// The cell uses `flex(1.0)` (`minmax(0, 1fr)`), **not** bare `fr(1.0)`: an
    /// `fr` track has a min-content floor, so a root whose content (a long
    /// scrollable chain) exceeds the window would inflate the cell to its
    /// content height instead of clamping to the viewport — every descendant
    /// Star then resolves against the inflated height and inner ScrollViewers
    /// never receive a bounded extent. The zero-floor cell clamps the root to
    /// the window; overflow scrolls.
    fn viewport(&mut self, width: f32, height: f32, root: NodeId) -> Option<NodeId> {
        let vp = match self.viewport {
            Some(v) => v,
            None => {
                let v = self.tree.new_leaf(viewport_style(width, height)).ok()?;
                self.viewport = Some(v);
                self.vp_size = (width, height);
                v
            }
        };
        if self.vp_size != (width, height) {
            let _ = self.tree.set_style(vp, viewport_style(width, height));
            self.vp_size = (width, height);
        }
        if self.vp_child != Some(root) {
            let _ = self.tree.set_children(vp, &[root]);
            self.vp_child = Some(root);
        }
        Some(vp)
    }
}

fn viewport_style(width: f32, height: f32) -> Style {
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
    s
}

/// Produce the final Taffy style for a node: its accumulated `style`, the
/// collapsed-subtree `Display::None` override, plus grid track templates for a
/// `Grid` (the only style derived lazily from props).
fn finalize_style(node: &Node, hidden: bool) -> Style {
    let mut s = node.style.clone();
    if hidden {
        s.display = Display::None;
    }
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
    let Some(taffy_id) = arena.get(id).and_then(|n| n.taffy_id).map(|(_, t)| t) else {
        return;
    };
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
    // Indexed rather than over a cloned child list — the clone was a heap
    // allocation per node per frame bought purely to dodge `&mut Arena`.
    let mut i = 0;
    while let Some(c) = arena.get(id).and_then(|n| n.children.get(i).copied()) {
        assign(arena, tree, c, ax, ay, sx, sy, scale);
        i += 1;
    }

    // Scroll containers: measure content extent, clamp the offset, and apply the
    // scroll translation to children (a compositor offset — no repaint).
    let is_scroll = arena.get(id).is_some_and(|n| n.is_scroll());
    if is_scroll {
        let (nx, ny, vh) = arena.get(id).map(|n| (n.rect.x, n.rect.y, n.rect.h)).unwrap();
        let mut content_h = 0.0_f32;
        let mut i = 0;
        while let Some(c) = arena.get(id).and_then(|n| n.children.get(i).copied()) {
            if let Some(cn) = arena.get(c) {
                content_h = content_h.max(cn.rect.y + cn.rect.h - ny);
            }
            i += 1;
        }
        let max_scroll = (content_h - vh).max(0.0);
        // Children are placed UNSCROLLED; the scroll translation lives on the
        // content carrier visual they parent into. Snap it: rects are
        // pixel-snapped, so a fractional offset would push every child back
        // off the grid.
        let scroll = if let Some(n) = arena.get_mut(id) {
            n.ctrl_mut().content_h = content_h;
            n.scroll_off = snap(n.scroll_off.clamp(0.0, max_scroll), scale);
            n.scroll_off
        } else {
            0.0
        };
        let mut i = 0;
        while let Some(c) = arena.get(id).and_then(|n| n.children.get(i).copied()) {
            if let Some(cn) = arena.get_mut(c) {
                let (x, y) = (cn.rect.x - nx, cn.rect.y - ny);
                cn.push_offset(x, y);
            }
            i += 1;
        }
        // Layout is placement, not motion: snap the carrier (gated inside, so
        // an unchanged pass costs nothing and a glide already heading to this
        // same target keeps flying).
        if let Some(n) = arena.get_mut(id) {
            n.scroll_snap(scroll);
        }
    }
}

/// Where a visual sits in a node's owned stack, bottom → top. The variant
/// ORDER *is* the z-order: adding a band is a one-line edit here plus wherever
/// it is collected, and it slots in at the right depth by construction.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Band {
    /// Chrome parts painted under the node's own surface (ink washes, tracks).
    BelowChrome,
    /// The node's own painted-chrome surface.
    Surface,
    /// Arena children — or, for a scroll container, the carrier they ride in.
    /// Ordered among themselves by `(z_index, document order)`.
    Content,
    /// Chrome parts painted over the surface (pills, thumbs, indicators).
    AboveChrome,
    /// The auto-hiding overlay scrollbar thumb.
    Overlay,
}

/// Which collection under a node a visual is parented into. A node's carrier is
/// created with the node and never appears or disappears, so a visual's slot is
/// fixed for its whole life — which is what lets [`restack`] detach the
/// previous set without having to guess where each visual ended up.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Slot {
    /// The node's own `container.Children()`.
    Container,
    /// A scroll container's content-carrier children.
    Carrier,
}

/// A visual's place in its node's owned stack. The whole ordering policy is
/// this struct's **field order** plus `derive(Ord)`: collection first (the two
/// are independent stacks), then band, then a child's `z_index`, then its
/// position in the child list. Nothing else sorts the stack, so there is one
/// place to read — and one place to change — what "z-order" means here.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct StackKey {
    pub slot: Slot,
    pub band: Band,
    pub z: i32,
    pub doc: usize,
}

/// One visual in a node's owned stack, with the key it sorts by.
type Stack = Vec<(StackKey, Visual)>;

/// Re-stack the composition children of any node whose child list or a child's
/// Z-order changed, as `[below-band chrome parts, own surface, children sorted
/// by (z, doc order), above-band chrome parts, scroll thumb]`. Retained visuals
/// are merely re-parented, not recreated.
///
/// # Why this detaches by name instead of calling `RemoveAll`
///
/// This used to open with `Children().RemoveAll()` and then re-insert the five
/// categories it knows about. That is a *total* teardown of a collection it
/// only *partly* owns: a Knob parents its value-arc and needle sprites straight
/// into the node container ([`knob::KnobParts`](super::knob::KnobParts)) and a
/// focused editor parents its caret there ([`parts::Caret`](super::parts)), and
/// neither was ever re-inserted. A Knob or editor that once took this path lost
/// its arc, needle or caret permanently — latent only because the path needs
/// `children_dirty` or a `z_dirty` child, and both controls are leaves today.
/// Give a Knob one child and it breaks.
///
/// So the rule is not "re-insert everything sync knows about" — that is exactly
/// the assumption that failed — but **detach only what sync itself attached**:
/// each node records the visuals it was last stacked with ([`Node::stacked`]),
/// and a re-stack removes precisely that set before laying the current one back
/// down. A visual sync has never heard of is never removed, so a future sprite
/// category cannot be lost by being forgotten here; the worst it can do is keep
/// the position its creator chose, which for both of today's strays
/// (`InsertAtTop`, i.e. above everything) is already where they want to be.
fn sync(arena: &mut Arena, id: ControlId, order: &mut Stack) {
    if arena.get(id).is_none() {
        return;
    }
    let mut any_z = false;
    let mut i = 0;
    while let Some(c) = arena.get(id).and_then(|n| n.children.get(i).copied()) {
        any_z |= arena.get(c).is_some_and(|cn| cn.z_dirty);
        i += 1;
    }
    let need = arena.get(id).is_some_and(|n| n.children_dirty) || any_z;

    if need {
        order.clear();
        collect(arena, id, order);
        // Stable, so equal keys keep collection order within a band.
        order.sort_by_key(|(k, _)| *k);
        restack(arena, id, order);

        if let Some(n) = arena.get_mut(id) {
            n.children_dirty = false;
        }
        let mut i = 0;
        while let Some(c) = arena.get(id).and_then(|n| n.children.get(i).copied()) {
            if let Some(cn) = arena.get_mut(c) {
                cn.z_dirty = false;
            }
            i += 1;
        }
    }

    // Indexed rather than over a cloned child list — the clone was a heap
    // allocation per node per frame bought purely to dodge `&mut Arena`.
    let mut i = 0;
    while let Some(c) = arena.get(id).and_then(|n| n.children.get(i).copied()) {
        sync(arena, c, order);
        i += 1;
    }
}

/// Gather every visual `sync` owns under `id`, tagged with its slot and band.
/// This is the ONE enumeration of the owned set: [`restack`] both detaches the
/// previous one and attaches this one from it, so the two can never disagree
/// about what sync is responsible for.
fn collect(arena: &Arena, id: ControlId, out: &mut Stack) {
    use Band::*;
    use Slot::Container;
    let Some(n) = arena.get(id) else { return };
    // Chrome bands hold at most a handful of parts each and carry no z of their
    // own, so their key is the band plus the order they were collected in.
    let push = |out: &mut Stack, slot, band, v| {
        let doc = out.len();
        out.push((StackKey { slot, band, z: 0, doc }, v));
    };

    if let Some(parts) = n.parts.as_deref() {
        for v in parts.below_visuals() {
            push(out, Container, BelowChrome, v);
        }
    }
    if let Some(v) = n.surf.as_ref().and_then(|s| s.sprite.cast::<Visual>().ok()) {
        push(out, Container, Surface, v);
    }
    // A scroll container's children ride the content CARRIER visual (whose
    // Offset is the animated scroll translation), so the carrier is what
    // occupies the content band and the children stack inside it; every other
    // node stacks its children directly.
    let carrier = n.scroll_content.as_ref().and_then(|c| c.cast::<Visual>().ok());
    let child_slot = if carrier.is_some() { Slot::Carrier } else { Container };
    if let Some(carrier) = carrier {
        push(out, Container, Content, carrier);
    }
    for (i, c) in n.children.iter().enumerate() {
        if let Some(cn) = arena.get(*c)
            && let Ok(v) = cn.container.cast::<Visual>()
        {
            let key = StackKey { slot: child_slot, band: Content, z: cn.z_index, doc: i };
            out.push((key, v));
        }
    }
    if let Some(parts) = n.parts.as_deref() {
        for v in parts.above_visuals() {
            push(out, Container, AboveChrome, v);
        }
    }
    if let Some(v) = n
        .scroll_thumb
        .as_ref()
        .and_then(|s| s.sprite.cast::<Visual>().ok())
    {
        push(out, Container, Overlay, v);
    }
}

/// Detach exactly the visuals this node was last stacked with, then lay the
/// current banded set back down *beneath* anything else parented into the same
/// collections. `order` must already be sorted bottom → top.
///
/// Walking top → bottom and pushing each visual to the BOTTOM is what leaves
/// the owned stack in exact order underneath every stray — the mirror image of
/// the `InsertAtTop` sequence this replaces, and the reason a stray keeps the
/// topmost position its creator gave it instead of being buried.
fn restack(arena: &mut Arena, id: ControlId, order: &Stack) {
    let Some(n) = arena.get(id) else { return };
    let Ok(coll) = n.container.Children() else { return };
    let carrier = n.scroll_content.as_ref().and_then(|c| c.Children().ok());
    let pick = |slot: Slot| match slot {
        Slot::Carrier => carrier.as_ref().unwrap_or(&coll),
        Slot::Container => &coll,
    };

    // Detach the previous set — and only it. Taken out of the node so the
    // buffer's allocation is reused rather than reallocated per re-stack, and
    // handed back BEFORE the inserts so the node's registry always describes
    // what sync believes it owns.
    let mut prev = arena
        .get_mut(id)
        .map(|n| std::mem::take(&mut n.stacked))
        .unwrap_or_default();
    for (slot, v) in prev.drain(..) {
        let _ = pick(slot).Remove(&v);
    }
    prev.extend(order.iter().map(|(k, v)| (k.slot, v.clone())));
    if let Some(n) = arena.get_mut(id) {
        n.stacked = prev;
    }

    for (k, v) in order.iter().rev() {
        let _ = pick(k.slot).InsertAtBottom(v);
    }
}

