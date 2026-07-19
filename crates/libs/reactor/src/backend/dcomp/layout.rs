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
//!
//! # The two halves of a pass
//!
//! A pass is bisected along the render-thread split's seam ([`compute`]):
//!
//! - **measure + solve** reads and writes *layout inputs* — text layouts,
//!   resolved alignment, the Taffy tree — and records every node's placement
//!   as plain `Send` data ([`Solved`], parked in [`LayoutTree::solved`]). It
//!   is also the **only writer of `TextLayout` state** (construction in
//!   [`rebuild_text`], the wrap pin in [`solve_walk`]): paint and the apply
//!   half only read, so when this half moves to the app thread the layouts
//!   have a single mutating side by construction.
//! - **apply + sync** consumes `Solved` and touches the composition tree —
//!   rect/offset/size pushes, scroll translation, child re-stacking. It never
//!   reads the Taffy tree. This is the half that stays with the compositor.
//!
//! The seam exists so the first half can eventually run on the app thread
//! against its own arena while the second replays `Solved` on the front
//! thread; until then both run back-to-back here and the split is enforced
//! purely by what each half is allowed to touch.

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
    let lt = arena.layout.take();
    let lt = compute_in(lt, arena, root, width, height, scale, false, (0.0, 0.0), (0.0, 0.0));
    arena.layout = Some(lt);
}

/// Lay out the overlay root — a hosted flyout's content — against the space the
/// popup can offer.
///
/// Uses the arena's OWN overlay tree rather than a second root in the window's:
/// a `LayoutTree` assumes one root, and alternating two through it would
/// re-parent the synthetic viewport every pass and sweep every pass. See
/// [`Arena::overlay_layout`](super::node::Arena).
/// `origin` is where the content sits in WINDOW DIPs, and `inset` is where it
/// sits inside the popup's own container. Both are needed and they are not the
/// same number: every node's `rect` must be in window space, because that is
/// what the entire input pipeline hit-tests and scrubs against, while the
/// root's visual offset must be relative to the container it was adopted into.
/// `solve_walk` already carries the two separately, so this is a matter of
/// seeding it correctly rather than rebasing anything afterwards.
pub(crate) fn compute_overlay(
    arena: &mut Arena,
    root: ControlId,
    width: f32,
    height: f32,
    scale: f32,
    origin: (f32, f32),
    inset: (f32, f32),
) {
    // `hug`: a flyout panel is sized BY its content, so `width`/`height` are a
    // ceiling rather than the box to fill.
    let lt = arena.overlay_layout.take();
    let lt = compute_in(lt, arena, root, width, height, scale, true, origin, inset);
    arena.overlay_layout = Some(lt);
}

/// Drop the overlay tree — its root is gone (the flyout closed or unmounted).
///
/// Freeing it rather than keeping it primed is deliberate: a flyout is open for
/// seconds at a time, its Taffy nodes are dead the moment it closes, and
/// leaving them owned would make the next pass's sweep walk them for nothing.
pub(crate) fn drop_overlay(arena: &mut Arena) {
    arena.overlay_layout = None;
}

fn compute_in(
    lt: Option<LayoutTree>,
    arena: &mut Arena,
    root: ControlId,
    width: f32,
    height: f32,
    scale: f32,
    hug: bool,
    origin: (f32, f32),
    inset: (f32, f32),
) -> LayoutTree {
    let scale = scale.max(0.01);
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
    let mut lt = lt.unwrap_or_else(LayoutTree::new);
    measure_solve(arena, &mut lt, root, width, height, scale, hug, origin, inset);
    apply(arena, &lt.solved, root, scale);
    // Re-stack composition children before the tree goes home: `sync` borrows
    // the arena mutably and `lt`'s scratch buffer mutably, which is only two
    // disjoint borrows while `lt` is still a local.
    let mut order = std::mem::take(&mut lt.order);
    sync(arena, root, &mut order);
    lt.order = order;
    lt
}

/// The measure + solve half of a pass (see the module docs): text and
/// alignment inputs, the Taffy solve, and the [`solve_walk`] that turns the
/// computed tree into [`LayoutTree::solved`]. On a failed solve the previous
/// pass's placements are cleared rather than kept — [`apply`] finding no entry
/// leaves the visuals exactly as they are, which is also what the old
/// single-walk did when the root had no Taffy node.
fn measure_solve(
    arena: &mut Arena,
    lt: &mut LayoutTree,
    root: ControlId,
    width: f32,
    height: f32,
    scale: f32,
    hug: bool,
    origin: (f32, f32),
    inset: (f32, f32),
) {
    rebuild_text(arena, root);
    resolve_align(arena, root, false, false);

    lt.solved.clear();
    let Some(root_taffy) = lt.walk_root(arena, root) else {
        return;
    };
    let Some(viewport) = lt.viewport(width, height, root_taffy, hug) else {
        return;
    };
    let tree = &mut lt.tree;

    let arena_ref = &*arena;
    // A hugging viewport is offered MAX-CONTENT: a definite box is what makes
    // Star tracks and stretched children expand to fill, which is exactly what
    // an overlay must not do. Its `max_size` still clamps the result.
    let offered = if hug {
        Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        }
    } else {
        Size {
            width: AvailableSpace::Definite(width),
            height: AvailableSpace::Definite(height),
        }
    };
    let _ = tree.compute_layout_with_measure(
        viewport,
        offered,
        |known, available, _node_id, ctx, _style| {
            if let (Some(w), Some(h)) = (known.width, known.height) {
                return Size { width: w, height: h };
            }
            // Taffy hands the context by `&mut`; copy the id out once so the
            // per-kind arms below can each test it without contending over the
            // borrow.
            let ctx = ctx.map(|id| *id);
            // An InfoBar's height is a function of its width — its paragraph
            // wraps inside a text column narrower than the band — so it
            // answers from its own cached run rather than the generic
            // `text_layout` slot below. Its `None` (max-content) case measures
            // the paragraph on one line, exactly as the generic wrap path's
            // does.
            if let Some(id) = ctx
                && let Some(node) = arena_ref.get(id)
                && node.kind == ControlKind::InfoBar
            {
                let avail = known.width.or(match available.width {
                    AvailableSpace::Definite(w) => Some(w),
                    AvailableSpace::MinContent => Some(0.0),
                    AvailableSpace::MaxContent => None,
                });
                let (w, h) = info_bar::measure(node, avail);
                return Size {
                    width: known.width.unwrap_or(w),
                    height: known.height.unwrap_or(h),
                };
            }
            // An InfoBadge sizes to its count (or to the bare dot). Both come
            // from `info_badge`, which is also what `birth_style` floors it
            // with, so the two cannot disagree.
            if let Some(id) = ctx
                && let Some(node) = arena_ref.get(id)
                && node.kind == ControlKind::InfoBadge
            {
                let (w, h) = info_badge::measure(node);
                return Size {
                    width: known.width.unwrap_or(w),
                    height: known.height.unwrap_or(h),
                };
            }
            // The two per-item kinds size from `item_text`, not `text_layout`:
            // neither owns a single representative run. A SelectorBar's
            // segments each size to their own label; a ToggleSwitch is the
            // track plus the gap plus the WIDER of its two state labels — the
            // wider, not the current one, so flipping it never reflows the row.
            if let Some(id) = ctx
                && let Some(node) = arena_ref.get(id)
                && matches!(node.kind, ControlKind::SelectorBar | ControlKind::ToggleSwitch)
                && let Some(t) = node.item_text.as_ref()
            {
                let (mut widest, mut line_h) = (0.0f32, 0.0f32);
                for l in t.measurable() {
                    if let Ok((w, h)) = l.measure() {
                        widest = widest.max(w);
                        line_h = line_h.max(h);
                    }
                }
                if node.kind == ControlKind::SelectorBar {
                    let m = controls::seg_metrics(node.paint.style_variant, node.paint.font_size);
                    let labels: f32 = node.ctrl().seg_label_w.iter().sum();
                    let n = node.ctrl().seg_label_w.len().max(1) as f32;
                    return Size {
                        width: known.width.unwrap_or(labels + n * 2.0 * m.pad_x + 2.0 * m.tray),
                        height: known.height.unwrap_or(line_h + 2.0 * (m.pad_y + m.tray)),
                    };
                }
                return Size {
                    width: known
                        .width
                        .unwrap_or(parts::TRACK_W + controls::TOGGLE_LABEL_GAP + widest),
                    // Taffy clamps to the birth `min_size` (the 40x20 track), so
                    // the label's line height alone is the right answer here.
                    height: known.height.unwrap_or(line_h),
                };
            }
            if let Some(id) = ctx
                && let Some(node) = arena_ref.get(id)
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
                    // A hyperlink is its words and nothing else — no ornament
                    // to reserve room for, and no border to inset from. The
                    // generic arm below would run `button_palette` on it to
                    // decide the latter, which is a question about a control
                    // this is not.
                    if node.kind == ControlKind::HyperlinkButton {
                        return Size {
                            width: known.width.unwrap_or(tw),
                            height: known.height.unwrap_or(th),
                        };
                    }
                    // The ornaments widen the button by exactly what
                    // `button_boxes` reserves for them — without this an
                    // adorned button sizes to its label alone and the ornament
                    // overlaps the text — and the outline takes its own room on
                    // both axes.
                    let chrome = controls::chrome_inset(node);
                    return Size {
                        width: known
                            .width
                            .unwrap_or(tw + controls::ornament_width(node) + chrome),
                        height: known.height.unwrap_or(th + chrome),
                    };
                }
            }
            Size {
                width: known.width.unwrap_or(0.0),
                height: known.height.unwrap_or(0.0),
            }
        },
    );

    // `ox/oy` accumulate into the absolute rect; `sox/soy` are the snapped
    // parent origin the relative offset is measured from. Seeding the second as
    // `origin - inset` makes the root's `rel` come out as exactly `inset` — the
    // offset it needs inside the container it was adopted into — while its rect
    // lands at `origin` in window space. The window root passes zeros for both
    // and is unaffected.
    solve_walk(
        arena,
        &lt.tree,
        &mut lt.solved,
        root,
        origin.0,
        origin.1,
        origin.0 - inset.0,
        origin.1 - inset.1,
        scale,
    );
}

/// One node's placement, as the measure + solve half hands it to [`apply`]:
/// plain data only, `Send` by construction — this is the payload that crosses
/// the thread boundary once the two halves live on different threads.
#[derive(Clone, Copy)]
pub(crate) struct Solved {
    /// Snapped absolute rect — becomes [`Node::rect`](super::node::Node) for
    /// hit-testing.
    rect: LaidRect,
    /// Offset relative to the parent's snapped origin — what the container
    /// visual is pushed.
    rel: (f32, f32),
    /// The wrap pin re-flowed this node's text: which glyphs land where
    /// changed without necessarily changing the node's box, so apply must
    /// repaint even when `resized` says nothing did.
    reflowed: bool,
    /// Scroll containers only: the content extent below the node's origin,
    /// measured from the children's solved rects. Zero elsewhere.
    content_h: f32,
}

const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<Solved>();
};

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
pub(crate) fn rebuild_text(arena: &mut Arena, id: ControlId) {
    if arena.get(id).is_none() {
        return;
    }
    // The button family shapes two runs BESIDE its label: the leading icon (a
    // different family at a different size) and the badge's count (the badge's
    // own smaller, heavier type). They live in `button_text` because the
    // generic `text_layout` slot below is already the label's.
    //
    // This runs first and deliberately leaves `text_dirty` set: the generic
    // block is what clears it, and the family always reaches that block —
    // `is_text` covers the whole family whether or not it carries words.
    let needs_ornaments = arena
        .get(id)
        .is_some_and(|n| n.text_dirty && super::node::is_button_family(n.kind));
    if needs_ornaments {
        let icon = arena
            .get(id)
            .map(|n| n.extras().icon)
            .filter(|cp| *cp != 0)
            .and_then(controls::glyph_str)
            .and_then(|g| {
                build_text_layout(&g, controls::ICON_SIZE, 400, theme::FONT_ICON, false)
            });
        let badge = arena
            .get(id)
            .and_then(|n| n.extras().badge)
            .and_then(|b| b.count)
            .and_then(|c| {
                build_text_layout(
                    &c.to_string(),
                    info_badge::FONT_SIZE,
                    info_badge::FONT_WEIGHT,
                    "Segoe UI",
                    false,
                )
            });
        if let Some(n) = arena.get_mut(id) {
            let t = n.button_text.get_or_insert_with(Default::default);
            t.icon_layout = icon;
            t.badge_layout = badge;
        }
    }
    // The Expander's two chevrons, shaped together into the same positional
    // `[off, on]` shape the switch uses — index `0` is collapsed, `1` expanded,
    // which is what `glyph_text::expander_sync` indexes with `expanded`.
    // Expanding never sets `text_dirty`, so both have to exist up front.
    //
    // ABOVE the generic run build below, which CLEARS `text_dirty` — an
    // Expander's header now qualifies as text, so anything of its own gated on
    // that flag and placed after it would simply never run.
    let needs_chevron = arena
        .get(id)
        .is_some_and(|n| n.text_dirty && n.kind == ControlKind::Expander);
    if needs_chevron {
        let built: Vec<Option<TextLayout>> = [
            controls::EXPANDER_GLYPH_COLLAPSED,
            controls::EXPANDER_GLYPH_EXPANDED,
        ]
        .into_iter()
        .map(|cp| {
            controls::glyph_str(cp).and_then(|g| {
                build_text_layout(
                    &g,
                    controls::EXPANDER_CHEVRON_SIZE,
                    400,
                    theme::FONT_ICON,
                    false,
                )
            })
        })
        .collect();
        if let Some(n) = arena.get_mut(id) {
            let t = n.item_text.get_or_insert_with(Default::default);
            t.layouts = built;
            t.strong.clear();
        }
    }
    let needs = arena.get(id).is_some_and(|n| n.text_dirty && is_text(n));
    if needs {
        let (text, size, weight, family, wrap) = {
            let n = arena.get(id).unwrap();
            // A button's WEIGHT comes from the palette — an accent button sets
            // at 600 — so reading the raw `paint` value here measured a lighter
            // run than the one that lands, and the label crowded its padding.
            //
            // Its size does not: the family is born at `FONT_SIZE_MD` already,
            // so a floor here could only ever discard a SMALLER size the app
            // asked for, which is how `.font_size(..)` came to be silently
            // inert on the one control most likely to want a small one — a
            // pill, a chip, a compact toolbar button.
            let (size, weight) = if super::node::is_button_family(n.kind) {
                (n.paint.font_size, super::controls::button_palette(n).weight)
            } else if n.kind == ControlKind::HyperlinkButton {
                // The floor the painted link applied. Unlike the button family
                // a hyperlink is NOT born at `FONT_SIZE_MD`, so dropping it
                // here would shrink every default link.
                (n.paint.font_size.max(theme::FONT_SIZE_MD), 400)
            } else {
                (n.paint.font_size, n.paint.font_weight)
            };
            (
                n.paint.text.clone(),
                size,
                weight,
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
    // A SelectorBar shapes every item label TWICE — once at rest weight and
    // once at the weight a selected segment takes — because a weight is baked
    // into a layout and selecting a segment does not set `text_dirty`. See
    // `glyph_text::ItemText`.
    //
    // Widths come from the ACTIVE runs, the wider pair, so a segment does not
    // grow when it is picked.
    // Deliberately NOT gated on there being any items: a bar whose items are
    // taken away must have its shaped runs taken away with them, and a pass that
    // skipped the empty case would leave the departed labels shaped and the sync
    // would keep placing them.
    let needs_seg = arena
        .get(id)
        .is_some_and(|n| n.text_dirty && n.kind == ControlKind::SelectorBar);
    if needs_seg {
        let (items, size, family) = {
            let n = arena.get(id).unwrap();
            (
                n.ctrl().items.clone(),
                controls::seg_font_size(n),
                n.paint.font_family.clone().unwrap_or_else(|| "Segoe UI".to_string()),
            )
        };
        let mut widths = Vec::with_capacity(items.len());
        let mut rest = Vec::with_capacity(items.len());
        let mut strong = Vec::with_capacity(items.len());
        for item in &items {
            let a = build_text_layout(item, size, controls::SEG_WEIGHT_ACTIVE, &family, false);
            widths.push(
                a.as_ref()
                    .and_then(|l| l.measure().ok())
                    .map_or(0.0, |(w, _)| w),
            );
            rest.push(build_text_layout(item, size, controls::SEG_WEIGHT, &family, false));
            strong.push(a);
        }
        if let Some(n) = arena.get_mut(id) {
            n.ctrl_mut().seg_label_w = widths;
            let t = n.item_text.get_or_insert_with(Default::default);
            t.layouts = rest;
            t.strong = strong;
            n.text_dirty = false;
            n.measure_dirty = true;
        }
    }
    // A select trigger draws a label its node's `paint.text` does not hold: a
    // ComboBox shows the selected item (or the placeholder), a DropDownButton
    // its own content. Neither had a measure prep, so `text_layout` stayed
    // empty, the generic callback fell through to 0×0, and both collapsed to
    // the padding alone.
    //
    // The ComboBox measures the WIDEST item, not the selected one, for the same
    // reason the ToggleSwitch measures the wider of its two labels: picking a
    // different item must not reflow the row around the control.
    let needs_select = arena.get(id).is_some_and(|n| {
        n.text_dirty && matches!(n.kind, ControlKind::ComboBox | ControlKind::DropDownButton)
    });
    if needs_select {
        let (candidates, family) = {
            let n = arena.get(id).unwrap();
            let family = n.paint.font_family.clone().unwrap_or_else(|| "Segoe UI".to_string());
            let candidates = if n.kind == ControlKind::ComboBox {
                let mut v = n.ctrl().items.clone();
                v.push(n.ctrl().placeholder.clone());
                v
            } else {
                vec![n.paint.text.clone()]
            };
            (candidates, family)
        };
        // `paint_select` draws at `FONT_SIZE_SM`/400 regardless of the node's
        // own text style, so the measurement matches that and not `paint`.
        let mut widest: Option<TextLayout> = None;
        let mut widest_w = -1.0f32;
        for s in &candidates {
            if let Some(l) = build_text_layout(s, theme::FONT_SIZE_SM, 400, &family, false)
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
    // A ToggleSwitch shapes BOTH state labels: it is sized to the wider of the
    // two so flipping it never reflows the row around it, and drawn with
    // whichever one the state currently names. One cached layout could answer
    // one of those or the other, so both are kept — index `0` is `off`, `1` is
    // `on`, which is what `glyph_text::toggle_sync` indexes with `is_on`.
    //
    // Not gated on either label being set, for the reason the segment pass is
    // not gated on having items: a switch that LOSES its labels must lose its
    // shaped runs, and the empty case already maps to `None` below.
    let needs_toggle = arena
        .get(id)
        .is_some_and(|n| n.text_dirty && n.kind == ControlKind::ToggleSwitch);
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
        // Positional: a switch with only `OnContent` set must still find that
        // label at index 1, so an empty side holds its slot as `None`.
        let built: Vec<Option<TextLayout>> = [&off, &on]
            .into_iter()
            .map(|s| {
                (!s.is_empty())
                    .then(|| build_text_layout(s, size, 400, &family, false))
                    .flatten()
            })
            .collect();
        if let Some(n) = arena.get_mut(id) {
            let t = n.item_text.get_or_insert_with(Default::default);
            t.layouts = built;
            t.strong.clear();
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
    // The nav pane draws a header, a label per item and a settings row, so it
    // lays them out here too — and then re-derives the width that depends on
    // whether there is a header at all. See `apply_nav_metrics`.
    let needs_nav = arena
        .get(id)
        .is_some_and(|n| n.text_dirty && n.kind == ControlKind::NavigationView);
    if needs_nav {
        let built = arena
            .get(id)
            .map(|n| nav::build_runs(n.extras(), &n.ctrl().items, &n.ctrl().icons));
        if let Some(n) = arena.get_mut(id) {
            // Adopted into the live text, never assigned over it: the pane's
            // sprites live beside its runs and own compositor visuals parented
            // into the node, so replacing the whole struct would orphan every
            // glyph already on screen and mint a second set beside it.
            let t = n.nav_text.get_or_insert_with(Default::default);
            t.adopt(built.unwrap_or_default());
            n.text_dirty = false;
            n.measure_dirty = true;
            apply_nav_metrics(n);
        }
    }
    // The InfoBar draws a title + message paragraph of its own, so it lays it
    // out here too. Unlike the caption and the pane this run WRAPS, and its
    // wrapped height is the band's height — so the rebuild also has to
    // invalidate the measure, which `measure_dirty` below does.
    let needs_bar = arena
        .get(id)
        .is_some_and(|n| n.text_dirty && n.kind == ControlKind::InfoBar);
    if needs_bar {
        let built = arena.get(id).map(|n| info_bar::build_text(n.extras()));
        if let Some(n) = arena.get_mut(id) {
            n.bar_text = built.flatten();
            n.text_dirty = false;
            n.measure_dirty = true;
        }
    }
    // A numeric InfoBadge measures its count. The run is a single short label,
    // so it lives in the generic `text_layout` slot — the same place a
    // ToggleSwitch parks the wider of its two state labels, and for the same
    // reason: one cached run is all the control has.
    let needs_badge = arena
        .get(id)
        .is_some_and(|n| n.text_dirty && n.kind == ControlKind::InfoBadge);
    if needs_badge {
        let label = arena.get(id).and_then(info_badge::label);
        let family = arena
            .get(id)
            .and_then(|n| n.paint.font_family.clone())
            .unwrap_or_else(|| "Segoe UI".to_string());
        // Size and weight come from `paint`, not from the badge constants: a
        // node is BORN carrying those constants (`birth_paint`), so an
        // untouched badge is unchanged while `.font_size(..)` / `.bold()` now
        // reach it like any other text-bearing control.
        let (size, weight) = arena
            .get(id)
            .map(|n| (n.paint.font_size, n.paint.font_weight))
            .unwrap_or((info_badge::FONT_SIZE, info_badge::FONT_WEIGHT));
        let built = label.and_then(|s| build_text_layout(&s, size, weight, &family, false));
        if let Some(n) = arena.get_mut(id) {
            n.text_layout = built;
            n.text_dirty = false;
            n.measure_dirty = true;
        }
    }
    let mut i = 0;
    while let Some(c) = arena.get(id).and_then(|n| n.children.get(i).copied()) {
        rebuild_text(arena, c);
        i += 1;
    }
}

/// Re-derive the NavigationView's layout inset from the pane state it now
/// holds: the content child is inset by exactly the width the pane draws
/// across.
///
/// The same rule, and the same single-definition discipline, as
/// [`apply_caption_metrics`]: the width comes straight from `nav`, which is
/// also what `birth_style` builds a virgin NavigationView from — so a node
/// whose pane state is back at its defaults re-derives exactly the style it was
/// born with, and the reset invariant holds without anything remembering a
/// number.
///
/// Reading `rect.w` (last pass's width) is what makes `PaneDisplayMode::Auto`
/// adaptive; see [`nav::resolve`].
pub(crate) fn apply_nav_metrics(n: &mut Node) {
    if n.kind != ControlKind::NavigationView {
        return;
    }
    n.style.padding.left = length(nav::pane_width(n.extras(), n.rect.w));
    n.mark_dirty();
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
        ControlKind::TextBlock => !n.paint.text.is_empty(),
        // A button with only an icon and no label still needs a layout: it is
        // what carries the line height, and without one the measure callback
        // has nothing to key on and the button collapses to 0x0. The empty
        // run measures zero wide, so the icon width is the whole intrinsic
        // size — which is the right answer.
        //
        // The WHOLE family, not `Button` alone: a ToggleButton or RepeatButton
        // that never built a layout measured to nothing and wrapped its label
        // one letter per line inside a collapsed box.
        k if super::node::is_button_family(k) => {
            !n.paint.text.is_empty() || n.extras().icon != 0
        }
        // A hyperlink's words are glyph sprites, and sprites are placed from a
        // shaped run — so it needs a layout where the painted link needed only
        // a string. Without one it renders nothing at all, silently: the sync
        // has no run to walk and simply hides.
        ControlKind::HyperlinkButton => !n.paint.text.is_empty(),
        // Same story as the hyperlink: the trailing label is glyph sprites now,
        // and sprites are placed from a shaped run. A checkbox with no label is
        // just the box, so an empty string needs no layout.
        ControlKind::CheckBox => !n.paint.text.is_empty(),
        // Its header label, likewise. The chevron beside it is not this run —
        // it lives in `item_text`, because there are two of them.
        ControlKind::Expander => !n.paint.text.is_empty(),
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
    /// Whether this tree's viewport sizes to its content (an overlay) rather
    /// than filling the space given (the window). Part of the viewport's style
    /// identity, so a change re-pushes it.
    vp_hug: bool,
    /// Reused stack of child ids, so a pass allocates nothing: each `walk`
    /// frame pushes its children's ids above `base` and truncates back on exit.
    scratch: Vec<NodeId>,
    /// Reused ordering buffer for [`sync`]'s composition re-stack. Parked here
    /// (rather than on the stack) purely so no pass allocates it afresh.
    pub(crate) order: Stack,
    /// The plain-data output of the measure + solve half, keyed by raw
    /// `ControlId`; [`apply`] consumes it. Reused across passes so the steady
    /// state allocates nothing. Post-split this map is what crosses the thread
    /// boundary — everything in it is `Send` by the [`Solved`] assertion.
    solved: rustc_hash::FxHashMap<u32, Solved>,
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
            vp_hug: false,
            scratch: Vec::new(),
            order: Vec::new(),
            solved: rustc_hash::FxHashMap::default(),
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
    fn viewport(&mut self, width: f32, height: f32, root: NodeId, hug: bool) -> Option<NodeId> {
        let vp = match self.viewport {
            Some(v) => v,
            None => {
                let v = self.tree.new_leaf(viewport_style(width, height, hug)).ok()?;
                self.viewport = Some(v);
                self.vp_size = (width, height);
                self.vp_hug = hug;
                v
            }
        };
        if self.vp_size != (width, height) || self.vp_hug != hug {
            let _ = self.tree.set_style(vp, viewport_style(width, height, hug));
            self.vp_size = (width, height);
            self.vp_hug = hug;
        }
        if self.vp_child != Some(root) {
            let _ = self.tree.set_children(vp, &[root]);
            self.vp_child = Some(root);
        }
        Some(vp)
    }
}

fn viewport_style(width: f32, height: f32, hug: bool) -> Style {
    // An OVERLAY viewport hugs: a flyout panel is sized BY its content, so the
    // cell is auto and the given extent is only a ceiling. Filling it — which
    // is exactly what the window viewport must do — gave a 250x90 band panel a
    // 480x560 box, because the root stretched into every pixel it was offered.
    if hug {
        let mut s = Style {
            display: Display::Grid,
            max_size: Size {
                width: length(width),
                height: length(height),
            },
            ..Style::default()
        };
        s.grid_template_columns = vec![auto()];
        s.grid_template_rows = vec![auto()];
        // The cell hugs, so the item must not stretch into it either.
        s.justify_items = Some(AlignItems::START);
        s.align_items = Some(AlignItems::START);
        return s;
    }
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
    // A closed InfoBar reclaims its space rather than merely painting nothing:
    // WinUI's `IsOpen=false` removes the band from layout, and a bar that left
    // a 40-DIP hole behind would push the content it sits above off-centre for
    // the whole time it is dismissed. Applied here, not in `default_style`, so
    // it re-derives on every pass from the live flag — the style comparison in
    // `walk` then picks the flip up as an ordinary style change.
    if node.kind == ControlKind::InfoBar && !node.extras().bar_open {
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

/// Walk the computed Taffy tree and record each node's [`Solved`] placement.
///
/// `(ox, oy)` is the parent's raw (Taffy) absolute origin — children accumulate
/// raw positions so rounding never drifts down the tree — while `(sox, soy)` is
/// the parent's *snapped* absolute origin: the recorded relative offset is the
/// difference of snapped absolutes, so each node lands exactly on its snapped
/// absolute rect on screen. Sizes snap by edge (`right - left`), keeping shared
/// edges between siblings coincident.
///
/// A node whose Taffy node is missing (or whose layout read fails) gets no
/// entry, and neither does its subtree — [`apply`] then leaves those visuals
/// untouched, exactly as the old single walk's early return did.
#[allow(clippy::too_many_arguments)]
fn solve_walk(
    arena: &Arena,
    tree: &TaffyTree<ControlId>,
    solved: &mut rustc_hash::FxHashMap<u32, Solved>,
    id: ControlId,
    ox: f32,
    oy: f32,
    sox: f32,
    soy: f32,
    scale: f32,
) {
    let Some(n) = arena.get(id) else { return };
    let Some(taffy_id) = n.taffy_id.map(|(_, t)| t) else {
        return;
    };
    let l = match tree.layout(taffy_id) {
        Ok(l) => *l,
        Err(_) => return,
    };
    let (ax, ay) = (ox + l.location.x, oy + l.location.y);
    let (aw, ah) = (l.size.width, l.size.height);
    // Snapped absolute rect (edge-snapped so adjacent siblings stay flush).
    let (sx, sy) = (snap(ax, scale), snap(ay, scale));
    let w = snap(ax + aw, scale) - sx;
    let h = snap(ay + ah, scale) - sy;
    // Pin a wrapping run's reflow box to the width it was actually given. This
    // is the single authoritative writer of that width: the measure callback
    // probes a node several times per pass (min-content, max-content, then the
    // definite size), and since the `TextLayout` is shared mutable COM state,
    // whichever probe ran last would otherwise decide how paint reflows. Here
    // the final answer is known, so neither paint nor apply needs constraint
    // logic of its own — and keeping the pin on the solve side keeps every
    // `TextLayout` write in this half (see the module docs).
    let reflowed = n.paint.wrap
        && n.text_layout.as_ref().is_some_and(|layout| {
            if layout.metrics().is_ok_and(|m| m.layout_width != w) {
                let _ = layout.set_max_width(w);
                true
            } else {
                false
            }
        });
    solved.insert(
        id.get(),
        Solved {
            rect: LaidRect { x: sx, y: sy, w, h },
            rel: (sx - sox, sy - soy),
            reflowed,
            content_h: 0.0,
        },
    );
    // Notify any viz host (SurfacePainter / composition surface) bound to this
    // node's container of its laid-out size (the DComp analogue of XAML's
    // FrameworkElement.SizeChanged). Fired every pass — NOT gated on a size
    // change — because a listener may register after this node's size already
    // settled (mount callbacks vs layout ordering); the registry change-gates
    // per listener, so an unchanged size is a cheap no-op and a fresh listener
    // is guaranteed its first delivery on the next pass. Skip the lookup
    // entirely when nothing is subscribed (the common case). The listeners are
    // app closures on the app thread; the fire queues plain triples that
    // `size`'s delivery hook carries over (see `size.rs`), so solving here on
    // the front thread never runs app code.
    if size::has_listeners() {
        size::fire_element_size(id, w, h);
    }
    for &c in &n.children {
        solve_walk(arena, tree, solved, c, ax, ay, sx, sy, scale);
    }
    // Scroll containers: content extent from the children just placed. Pure
    // math here — the clamp against the scroll offset happens in [`apply`],
    // because the offset is input-owned front-thread state.
    if n.is_scroll() {
        let mut content_h = 0.0_f32;
        for &c in &n.children {
            if let Some(cs) = solved.get(&c.get()) {
                content_h = content_h.max(cs.rect.y + cs.rect.h - sy);
            }
        }
        if let Some(s) = solved.get_mut(&id.get()) {
            s.content_h = content_h;
        }
    }
}

/// The apply half: push each solved placement onto its node — the absolute
/// rect for hit-testing, the relative offset + size onto the container visual
/// — then clamp and apply scroll. Consumes [`Solved`] and the arena only;
/// never reads the Taffy tree or writes a `TextLayout`.
fn apply(
    arena: &mut Arena,
    solved: &rustc_hash::FxHashMap<u32, Solved>,
    id: ControlId,
    scale: f32,
) {
    let Some(s) = solved.get(&id.get()).copied() else {
        return;
    };
    let LaidRect { x: sx, y: sy, w, h } = s.rect;
    if let Some(n) = arena.get_mut(id) {
        let resized = (n.rect.w, n.rect.h) != (w, h);
        n.rect = s.rect;
        // The compositor moves/sizes the node — no repaint for a move; a size
        // change does need the node's surface rebuilt at the new pixel extent.
        // Pushes are change-gated (and, when the node declares a layout
        // animation or translation transition, an actual change GLIDES — the
        // implicit animation triggers on the compositor, no ticks here).
        n.push_offset(s.rel.0, s.rel.1);
        n.push_size(w, h);
        // A re-flow (`reflowed`) changes which glyphs land where without
        // necessarily changing the node's box, so it repaints on a width
        // change that `resized` misses.
        if resized || s.reflowed {
            n.mark_dirty();
        }
    }
    // Indexed rather than over a cloned child list — the clone was a heap
    // allocation per node per frame bought purely to dodge `&mut Arena`.
    let mut i = 0;
    while let Some(c) = arena.get(id).and_then(|n| n.children.get(i).copied()) {
        apply(arena, solved, c, scale);
        i += 1;
    }

    // Scroll containers: adopt the solved content extent, clamp the offset, and
    // apply the scroll translation to children (a compositor offset — no
    // repaint).
    let is_scroll = arena.get(id).is_some_and(|n| n.is_scroll());
    if is_scroll {
        let max_scroll = (s.content_h - h).max(0.0);
        // Children are placed UNSCROLLED; the scroll translation lives on the
        // content carrier visual they parent into. Snap it: rects are
        // pixel-snapped, so a fractional offset would push every child back
        // off the grid.
        let scroll = if let Some(n) = arena.get_mut(id) {
            n.ctrl_mut().content_h = s.content_h;
            n.scroll_off = snap(n.scroll_off.clamp(0.0, max_scroll), scale);
            n.scroll_off
        } else {
            0.0
        };
        let mut i = 0;
        while let Some(c) = arena.get(id).and_then(|n| n.children.get(i).copied()) {
            if let Some(cn) = arena.get_mut(c) {
                let (x, y) = (cn.rect.x - sx, cn.rect.y - sy);
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

