//! The preorder walk that fills the hit array, its id index and the accessible tree.
//!
//! Three stacks carry the ancestry — every entry by index, clipping entries by index,
//! scrolling nodes by id — so `parent`, `clip_parent` and `scroll_src` are filled during the
//! walk rather than by a second pass. The automation rows ride the same walk, because the two
//! describe the same layout and a second walk could describe a different one.

use super::binding::HandlerTable;
use super::control::ControlRow;
use super::field;
use super::text::Table;
use super::tree::{self, Geom, Tree};
use crate::uia::snapshot::FieldText;
use crate::uia::{ColFlags, Entry, Snapshot, State};
use crate::widget::{ModelState, Range, UiaRole, flag};
use std::sync::Arc;
use windows_scene::{
    CONTROL, ControlId, HitDecl, HitEntry, HitFlags, NO_ENTRY, NodeId, SinkPatch, Slots,
};

fn display_only(row: &ControlRow, handlers: Option<&super::control::Handlers>) -> bool {
    matches!(row.uia, UiaRole::ProgressBar | UiaRole::Graph)
        && !handlers.is_some_and(|h| {
            h.click.is_some() || h.scalar.is_some() || h.drag.is_some() || h.flyout.is_some()
        })
}

/// What the walk reads. Disjoint borrows of the host, so the walk holds no `&mut Host`.
pub(crate) struct Walk<'a> {
    pub tree: &'a Tree,
    pub controls: &'a Slots<CONTROL, ControlRow>,
    pub text: &'a Table,
    pub handlers: &'a HandlerTable,
    pub fields: &'a Slots<CONTROL, field::Row>,
    /// The overlays standing, so a control with one open on it reports itself expanded.
    pub overlays: &'a [super::host::Placement],
}

/// What the walk writes. The automation half is absent on a pass no provider asked for.
///
/// The array is built into the patch's own two buffers rather than into one of ours, so the
/// largest payload on the wire is written once and read once.
pub(crate) struct Out<'a> {
    pub hits: &'a mut HitBuilder,
    pub patch: &'a mut SinkPatch,
    pub uia: Option<&'a mut Snapshot>,
    /// Where a help string is read before it is interned. Held by the caller, so a screen of
    /// tooltips costs one allocation.
    pub scratch: &'a mut String,
    /// [`Tree::bears`], written by this walk.
    pub bears: &'a mut Vec<bool>,
}

/// The ancestry one preorder walk carries, kept across rebuilds for its capacity.
#[derive(Default)]
pub(crate) struct HitBuilder {
    /// Every emitted entry, as `(depth, index)`.
    entries: Vec<(usize, u32)>,
    /// The clipping ones.
    clips: Vec<(usize, u32)>,
    /// The scrolling nodes, with the row each one occupies in [`Snapshot::scrolls`].
    scrolls: Vec<(usize, NodeId, u16)>,
    /// The emitted automation elements, and the clipping ones among them.
    uia: Vec<(usize, u16)>,
    uia_clips: Vec<(usize, u16)>,
}

impl HitBuilder {
    fn unwind(&mut self, depth: usize) {
        self.entries.retain(|&(at, _)| at < depth);
        self.clips.retain(|&(at, _)| at < depth);
        self.scrolls.retain(|&(at, ..)| at < depth);
        self.uia.retain(|&(at, _)| at < depth);
        self.uia_clips.retain(|&(at, _)| at < depth);
    }
}

/// Fills the array, and the automation tree where `out` carries one, from the window root
/// and every overlay above it, in z-order.
///
/// Slot roots append after the window subtree, in the order they opened, each
/// light-dismissing overlay preceded by its full-window blocker: the array is the z-order and
/// the scan takes the first hit from the back.
pub(crate) fn fill(walk: &Walk<'_>, out: &mut Out<'_>, root: NodeId) {
    let window = walk.tree.c.geom[root.index()].size;
    out.bears.clear();
    out.bears.resize(walk.tree.c.flags.len(), true);
    begin(out);
    self::walk(walk, out, root, 0);
    for placement in walk.overlays {
        if let Some(id) = placement.blocker {
            blocker(out, id, (window.x, window.y));
        }
        self::walk(walk, out, placement.root, 0);
        // A blocker spans the window, so the overlay's root answers for it whatever its
        // subtree holds.
        out.bears[placement.root.index()] = true;
    }
    // The blockers read the window root's extent.
    out.bears[root.index()] = true;
    // Sorted on the way out, so `HitTable::replace` is two copies and never a sort.
    out.patch.index_mut().sort_unstable_by_key(|&(id, _)| id);
}

/// Clears the ancestry stacks and every output, starting a fresh array.
pub(crate) fn begin(out: &mut Out<'_>) {
    out.hits.unwind(0);
    out.patch.hits_mut().clear();
    out.patch.index_mut().clear();
    if let Some(uia) = out.uia.as_deref_mut() {
        uia.clear();
    }
}

/// Appends a full-window blocker ahead of an overlay that dismisses on a press outside, so a
/// dismissing press never also reaches the content underneath.
///
/// A slot root is not inside the window subtree, so the ancestry is cut here: it inherits
/// neither the window's clips, nor its scroll offsets, nor its entries.
pub(crate) fn blocker(out: &mut Out<'_>, id: ControlId, window: (f32, f32)) {
    out.hits.unwind(0);
    push_entry(
        out,
        HitEntry {
            x0: 0.0,
            y0: 0.0,
            x1: window.0,
            y1: window.1,
            touch_inflate: 0.0,
            clip_parent: NO_ENTRY,
            parent: NO_ENTRY,
            flags: HitFlags::BLOCKER | HitFlags::INTERACTIVE,
            scroll_src: NodeId::NONE,
            id,
        },
    );
}

/// Appends one entry and its id-index row.
fn push_entry(out: &mut Out<'_>, entry: HitEntry) -> u32 {
    let id = entry.id;
    let entries = out.patch.hits_mut();
    let at = entries.len() as u32;
    entries.push(entry);
    if !id.is_none() {
        out.patch.index_mut().push((id, at));
    }
    at
}

/// Walks a root in paint order, emitting the entry of every node that declared one.
///
/// Suspended subtrees and derived sprites are skipped: a thumb's entry would name a rect the
/// solve fixed and the tracker then moved away from.
///
/// Answers whether the subtree put anything in the array, and records it in
/// [`Out::bears`]: an entry, a scrolling node its descendants resolve through, or a clip
/// collapsed to nothing, whose growing back would reveal what it hid.
pub(crate) fn walk(walk: &Walk<'_>, out: &mut Out<'_>, node: NodeId, depth: usize) -> bool {
    let flags = walk.tree.c.flags[node.index()];
    if flags & (tree::HIDDEN | tree::SUSPENDED | tree::DERIVED) != 0 {
        // Showing, resuming or re-linking one rebuilds the array on its own.
        out.bears[node.index()] = false;
        return false;
    }
    let translation_start = out.uia.as_ref().map(|uia| uia.entries.len());
    out.hits.unwind(depth);
    let control = walk.tree.c.control[node.index()];
    if let Some(row) = walk.controls.get(control) {
        let selects = matches!(row.uia, UiaRole::RadioButton | UiaRole::TabItem)
            || (row.selectable && row.uia != UiaRole::CheckBox);
        if selects
            && !walk
                .handlers
                .get(row.handlers)
                .is_some_and(|h| h.select.is_some())
        {
            let mut parent = walk.tree.parent(node);
            while !parent.is_none() {
                if let Some(group) = walk.controls.get(walk.tree.c.control[parent.index()]) {
                    assert!(
                        group.selection != Some(false),
                        "optional selection items require on_select(bool)"
                    );
                    if group.selection.is_some()
                        || matches!(group.uia, UiaRole::ComboBox | UiaRole::List | UiaRole::Tab)
                    {
                        break;
                    }
                }
                parent = walk.tree.parent(parent);
            }
        }
    }
    let bounded = flags & tree::CLIP != 0;
    let geom = walk.tree.c.geom[node.index()];
    if bounded && (geom.size.x <= 0.0 || geom.size.y <= 0.0) {
        out.bears[node.index()] = true;
        return true;
    }
    let mut bears = flags & (tree::HIT | tree::SCROLL) != 0;
    if flags & tree::HIT != 0 {
        let mut decl = HitDecl {
            flags: HitFlags::from_bits(tree::unpack_decl(flags)),
            id: control,
            touch_inflate: Some(walk.tree.c.inflate[node.index()]).filter(|v| !v.is_nan()),
        };
        if walk
            .controls
            .get(control)
            .is_some_and(|row| display_only(row, walk.handlers.get(row.handlers)))
        {
            decl.flags = HitFlags::from_bits(decl.flags.bits() & !HitFlags::INTERACTIVE.bits());
        }
        emit_hit(out, depth, &geom, bounded, flags, decl);
    }
    if out.uia.is_some() {
        emit_uia(walk, out, depth, &geom, bounded, flags, control);
    }
    // Pushed after this node's own rows: a viewport's own box is not resolved through its own
    // offset, only its descendants' are.
    if flags & tree::SCROLL != 0 {
        // The container's own entry, where it published one: `emit_uia` ran a moment ago and
        // pushed it at this depth, so the scroll pattern hangs on the element the viewport is
        // rather than on a second one invented for it.
        let owner = out
            .hits
            .uia
            .last()
            .filter(|&&(held, _)| held == depth)
            .map_or(crate::uia::NONE, |&(_, at)| at);
        let at = match out.uia.as_deref_mut() {
            Some(uia) => {
                let at = uia.scrolls.len() as u16;
                uia.scrolls.push(crate::uia::ScrollView::new(node, owner));
                at
            }
            None => crate::uia::NONE,
        };
        out.hits.scrolls.push((depth, node, at));
    }
    for child in walk.tree.children(node) {
        bears |= self::walk(walk, out, child, depth + 1);
    }
    out.bears[node.index()] = bears;
    if let (Some(start), Some(uia), Some(state)) = (
        translation_start, out.uia.as_deref_mut(),
        walk.controls.get(control).and_then(|row| row.translation.as_ref()),
    ) {
        uia.translations.push(windows_scene::TranslationRange {
            owner: control, start, end: uia.entries.len(), state: state.clone(),
        });
    }
    bears
}

/// Emits one node's hit entry and pushes it onto the ancestry stacks.
///
/// A node that declares nothing appends no entry, and its children take the nearest emitted
/// ancestor as their `parent`.
fn emit_hit(
    out: &mut Out<'_>,
    depth: usize,
    geom: &Geom,
    bounded: bool,
    node_flags: tree::Bits,
    decl: HitDecl,
) {
    let mut flags = decl.flags;
    if bounded {
        flags = flags | HitFlags::CLIP;
    }
    if node_flags & tree::SCROLL != 0 {
        flags = flags | HitFlags::SCROLL;
    }
    // Chrome pinned to a viewport states that its rect does not resolve through that
    // viewport's offset, so a rail does not slide off its own track.
    let scroll_src = if flags.contains(HitFlags::UNSCROLLED) {
        NodeId::NONE
    } else {
        out.hits
            .scrolls
            .last()
            .map_or(NodeId::NONE, |&(_, id, _)| id)
    };
    let at = push_entry(
        out,
        HitEntry {
            x0: geom.rect.x0,
            y0: geom.rect.y0,
            x1: geom.rect.x1,
            y1: geom.rect.y1,
            touch_inflate: decl
                .touch_inflate
                .unwrap_or_else(|| windows_scene::default_inflation(geom.size.x, geom.size.y)),
            clip_parent: out.hits.clips.last().map_or(NO_ENTRY, |&(_, at)| at),
            parent: out.hits.entries.last().map_or(NO_ENTRY, |&(_, at)| at),
            flags,
            scroll_src,
            id: decl.id,
        },
    );
    out.hits.entries.push((depth, at));
    if flags.contains(HitFlags::CLIP) {
        out.hits.clips.push((depth, at));
    }
}

/// Emits one node's automation element.
///
/// A node whose control carries neither a role nor a name is skipped and its children reparent
/// past it. A named one with no role is a group, so a name an author wrote is never dropped
/// on its way to a client.
fn emit_uia(
    walk: &Walk<'_>,
    out: &mut Out<'_>,
    depth: usize,
    geom: &Geom,
    bounded: bool,
    node_flags: tree::Bits,
    control: ControlId,
) {
    let Some(row) = walk.controls.get(control) else {
        return;
    };
    let role = match row.uia {
        UiaRole::None if row.name.is_some() => UiaRole::Group,
        UiaRole::None => return,
        role => role,
    };
    let decl = HitFlags::from_bits(tree::unpack_decl(node_flags));
    let run = row.text.and_then(|key| walk.text.str_of(key));
    let name = row.name.as_deref().or(run).unwrap_or_default();
    // A control that is labelled and also shows a run reports that run as its value: a combo
    // box is named "Output endpoint" and shows the endpoint. Only where the role carries the
    // value pattern, so a named button interns nothing.
    let shown = run.filter(|run| {
        row.name.is_some()
            && !run.is_empty()
            && *run != name
            && crate::uia::roles::row(role)
                .patterns
                .has(crate::uia::Patterns::VALUE)
    });
    // Read untracked: this runs inside a flush, and subscribing whatever effect is on the
    // stack would rebuild a screen when a tip changed.
    // A validation message is the help while it stands; the tip is the help otherwise.
    out.scratch.clear();
    let has_help = match row.validation {
        Some(text) => {
            out.scratch.push_str(text);
            true
        }
        None => walk.handlers.get(row.handlers).is_some_and(|handlers| {
            handlers.tip.as_ref().is_some_and(|(text, _)| {
                crate::signal::untracked(|| text.append(out.scratch));
                true
            })
        }),
    };
    // The editable body is a row of its own rather than a run in the pool, because the input
    // stack owns the text and automation reads the same UTF-16 buffer it does.
    let field = walk.fields.get(control);
    // A control with no span carries no number: every driven control holds a value row, and a
    // toggle's is the degenerate one its fraction is published through. Reporting that as a
    // range would advertise `RangeValue` over nothing and announce a switch as a number.
    let range = row
        .value
        .filter(|value| value.span != 0.0)
        .map(|value| Range {
            min: value.min,
            max: value.min + value.span,
            step: value.step,
            vertical: row.front.flags & flag::VERTICAL != 0,
        });
    let mut flags = ColFlags::NONE;
    if let Some(required) = row.selection {
        flags = flags | ColFlags::SELECTION;
        if required {
            flags = flags | ColFlags::SELECTION_REQUIRED;
        }
    }
    if role == UiaRole::ComboBox || (role == UiaRole::List && row.overlay.is_some()) {
        flags = flags | ColFlags::SELECTION_REQUIRED;
    }
    let handlers = walk.handlers.get(row.handlers);
    let read_only = field.is_none() && !handlers.is_some_and(|h| h.scalar.is_some());
    if read_only {
        flags = flags | ColFlags::READ_ONLY;
    }
    if !row.disabled && decl.contains(HitFlags::INTERACTIVE) && !display_only(row, handlers) {
        flags = flags | ColFlags::FOCUSABLE;
    }
    if field.is_some() {
        flags = flags | ColFlags::FIELD;
    }
    // The overlay itself, not the control it opened from: a dialog is what a reader announces
    // title-first, and announcing the button that opens one that way names the wrong thing.
    if let Some(kind) = row.overlay {
        flags = flags | ColFlags::OVERLAY;
        if kind == crate::overlay::Kind::Popup {
            flags = flags | ColFlags::DIALOG;
        }
    }
    if node_flags & tree::SCROLL != 0 {
        flags = flags | ColFlags::SCROLLS;
    }
    if field.map_or(role == UiaRole::Text, |f| {
        f.scope != crate::text_input::InputScope::Password
    }) {
        flags = flags | ColFlags::BODY;
    }
    if range.is_some() {
        flags = flags | ColFlags::RANGED;
    }
    if shown.is_some() {
        flags = flags | ColFlags::SHOWN;
    }
    // Selection the role does not already imply: a list row and a tab answer `SelectionItem`,
    // and a check box and a radio button answer it through their role instead.
    if row.selectable && !matches!(role, UiaRole::CheckBox | UiaRole::RadioButton) {
        flags = flags | ColFlags::SELECTS;
    }
    match walk.tree.live_bits(node_flags) {
        tree::LIVE_POLITE => flags = flags | ColFlags::LIVE_POLITE,
        tree::LIVE_ASSERTIVE => flags = flags | ColFlags::LIVE_ASSERTIVE,
        _ => {}
    }
    if handlers.is_some_and(|h| h.flyout.is_some() || h.expand.is_some()) {
        flags = flags | ColFlags::EXPANDS;
    }

    let mut parent = out.hits.uia.last().map_or(crate::uia::NONE, |&(_, at)| at);
    if row.overlay == Some(crate::overlay::Kind::Flyout) {
        if let Some(invoker) = walk
            .overlays
            .iter()
            .find(|p| p.root == row.node)
            .and_then(|p| p.invoker)
        {
            if let Some(at) = out
                .uia
                .as_deref()
                .and_then(|uia| uia.entries.iter().position(|e| e.id == invoker))
            {
                parent = at as u16;
            }
        }
    }
    let clip = out
        .hits
        .uia_clips
        .last()
        .map_or(crate::uia::NONE, |&(_, at)| at);
    let scroll = out
        .hits
        .scrolls
        .last()
        .map_or(crate::uia::NONE, |&(_, _, at)| at);
    let Out { uia, scratch, .. } = out;
    let uia = uia
        .as_deref_mut()
        .expect("the caller asked for automation rows");
    let name = uia.intern(name);
    let shown = shown.map(|run| uia.intern(run));
    let help = has_help.then(|| uia.intern(scratch));
    let at = uia.entries.len() as u16;
    uia.entries.push(Entry {
        id: control,
        box_: [geom.rect.x0, geom.rect.y0, geom.rect.x1, geom.rect.y1],
        name,
        parent,
        child: crate::uia::NONE,
        next: crate::uia::NONE,
        clip,
        scroll,
        flags,
        role,
    });
    // A check box reports the same fact as a toggle and every other role as a selection, so a
    // reader hears "checked" or "3 of 5" rather than silence.
    let selected = row.selected;
    let chosen = match (selected, role) {
        (false, _) => State::default(),
        (true, UiaRole::CheckBox) => State::TOGGLED,
        (true, _) => State::SELECTED,
    };
    // A control with its flyout standing is expanded, read from the overlay stack rather than
    // recorded when it opened: the two would otherwise have to be kept in step, and a menu
    // dismissed by a press outside closes without telling the control it opened from.
    let expanded = walk
        .overlays
        .iter()
        .any(|placement| placement.invoker == Some(control))
        || row.expanded;
    uia.state.push(
        match row.state {
            ModelState::Disabled => chosen,
            _ => State::ENABLED | chosen,
        } | if expanded {
            State::EXPANDED
        } else {
            State::default()
        },
    );
    if role == UiaRole::ComboBox {
        if let Some((key, name)) = handlers.and_then(|h| h.choice.as_ref()) {
            let name = uia.intern(name);
            uia.choices.push((at, *key, name));
        }
    }
    if let Some(help) = help {
        uia.helps.push((at, help));
    }
    if let Some(shown) = shown {
        uia.shown.push((at, shown));
    }
    if let Some(key) = row.key.as_deref() {
        let key = uia.intern(key);
        uia.keys.push((at, key));
    }
    if let Some(range) = range {
        uia.ranges.push((at, range));
        // Stated at every publish, so a client reads where the control stands rather than
        // whatever number the tree before it last announced. A control whose number is written
        // elsewhere states none here, and a `0.0` would be a number rather than its absence.
        if let Some(number) = row.number {
            uia.values.push((at, number));
        }
    }
    if role == UiaRole::Text && row.name.as_deref().is_none_or(|name| run == Some(name)) {
        if let Some(geometry) = row.text.and_then(|key| walk.text.uia_geometry(key)) {
            uia.text_geometry.push((at, geometry));
        }
    }
    if let Some(field) = field {
        uia.fields.push(FieldText {
            id: control,
            revision: field.revision,
            text: Arc::clone(&field.text),
            selection: field.selection,
            geometry: field.geometry.clone(),
            password: field.scope == crate::text_input::InputScope::Password,
        });
    }
    out.hits.uia.push((depth, at));
    if bounded {
        out.hits.uia_clips.push((depth, at));
    }
}
