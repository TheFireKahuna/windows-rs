//! App half of fields: callbacks and shaping. The working document remains input-owned.

use super::host::Host;
use super::text::{Fold, MeasureKey};
use crate::layout::Align;
use crate::role::{Fill, Metric, Role, Scope, Text, metric, resolve};
use crate::text_input::{
    Cluster, Commit, Geometry, InputScope, Layout, Selection, Source, Update, system,
};
use crate::widget::TextSource;
use std::borrow::Cow;
use std::sync::Arc;
use windows_numerics::Vector2;
use windows_scene::{
    Anim, Bind, ControlId, Corners, Easing, GroupId, Iterations, Mask, NodeId, Paint, Prop,
    SpriteId, Value,
};
use windows_text::Rect;

/// One field's app-side row.
///
/// The input STA owns the editable UTF-16 buffer, the private selection and affinity, and the
/// composition lifetime; nothing here mirrors them.
pub(crate) struct Row {
    /// The run this field shapes through. Password masking also shapes on this thread, as the
    /// fold the run was minted with.
    key: MeasureKey,
    group: GroupId,
    /// The published immutable text, in UTF-16 because the positions input names are UTF-16
    /// offsets and converting at every caret query would make each one a scan.
    pub text: Arc<[u16]>,
    pub selection: Selection,
    composition: Option<core::ops::Range<u32>>,
    pub geometry: Option<Arc<Geometry>>,
    /// The user revision `text` was written against, and the layout revision published beside
    /// it. A source record carries the revision it was based on, and later typing supersedes
    /// it; a key waiting for geometry completes before blur applies a pending replacement.
    pub revision: u64,
    layout_revision: u64,
    pub callback_revision: Option<u64>,
    pub delivered_revision: Option<u64>,
    /// Where the text stands inside the clip. Text is absolute inside the field, so a long
    /// edit does not continually resize its neighbours, and reveal moves this rather than the
    /// box.
    scroll: f32,
    align: Align,
    reveal: Option<(u32, u32)>,
    /// One caret, one sprite per selection rect, one per composition underline. Pooled, so a
    /// keystroke inside a selection retargets rather than mints.
    caret: SpriteId,
    selections: Vec<SpriteId>,
    underlines: Vec<SpriteId>,
    /// Where this field's shaped clusters and highlight boxes are staged. Per field rather
    /// than per host, so publishing one field borrows nothing another owns.
    clusters: Vec<Cluster>,
    rects: Vec<Rect>,
    style: Scope,
    pub scope: InputScope,
    focused: bool,
    dirty: bool,
}

impl Row {
    fn new(key: MeasureKey, group: GroupId, style: Scope, caret: SpriteId) -> Self {
        Self {
            key,
            group,
            text: Arc::from([]),
            selection: Selection::default(),
            composition: None,
            geometry: None,
            revision: 0,
            layout_revision: 0,
            callback_revision: None,
            delivered_revision: None,
            scroll: 0.0,
            align: Align::Start,
            reveal: None,
            caret,
            selections: Vec::new(),
            underlines: Vec::new(),
            clusters: Vec::new(),
            rects: Vec::new(),
            style,
            scope: InputScope::Default,
            focused: false,
            dirty: true,
        }
    }
}

/// Remaps a masked run's cluster boundaries onto the source's own positions.
///
/// The mask is one dot per source character and not one code unit per code unit, so a
/// supplementary character is one dot whose span is two. Input names positions in the
/// source's units, so the boundaries are translated here rather than the mask being made to
/// match by length.
fn remap<'a>(text: &[u16], scope: InputScope, clusters: &'a [Cluster]) -> Cow<'a, [Cluster]> {
    if scope != InputScope::Password {
        return Cow::Borrowed(clusters);
    }
    let mut out = clusters.to_vec();
    let mut at = 0;
    for cluster in &mut out {
        let end = at + source_units(text, at);
        (cluster.start, cluster.end) = (at, end);
        at = end;
    }
    Cow::Owned(out)
}

/// The range one decoration covers: the selection, the composition, or nothing at all where
/// the field does not hold focus.
fn decorated(
    focused: bool,
    selection: Selection,
    composition: Option<&core::ops::Range<u32>>,
    underline: bool,
) -> core::ops::Range<u32> {
    match (focused, underline) {
        (false, _) => 0..0,
        (true, false) => selection.range(),
        (true, true) => composition.cloned().unwrap_or(0..0),
    }
}

/// What one plan writes, computed under a read of the table and applied through the host.
struct Plan {
    /// The run's own node, which the origin is written onto.
    node: NodeId,
    geometry: Geometry,
    caret: Rect,
    size: Vector2,
}

/// Removes the separators a single-line field cannot hold: CR, LF, NEL, LINE SEPARATOR and
/// PARAGRAPH SEPARATOR.
///
/// Applied to paste and to source values. Enter claimed by the text service belongs to the
/// composition; otherwise it inserts no newline, so this is the only way one reaches the
/// buffer.
fn units(text: &str) -> impl Iterator<Item = u16> + '_ {
    text.encode_utf16()
        .filter(|u| !matches!(u, 10 | 13 | 0x0085 | 0x2028 | 0x2029))
}

/// The code units the source character at `at` occupies: two for a surrogate pair, one
/// otherwise.
fn source_units(text: &[u16], at: u32) -> u32 {
    match text.get(at as usize) {
        Some(0xd800..=0xdbff) => 2,
        _ => 1,
    }
}

/// The pool a decoration draws from: selection fills, or composition underlines.
fn pool(row: &Row, underline: bool) -> &Vec<SpriteId> {
    match underline {
        true => &row.underlines,
        false => &row.selections,
    }
}

fn pool_mut(row: &mut Row, underline: bool) -> &mut Vec<SpriteId> {
    match underline {
        true => &mut row.underlines,
        false => &mut row.selections,
    }
}

/// Moves `rect` into the run's space and, for an underline, reduces it to a rule on its
/// baseline edge.
fn decoration(rect: Rect, origin: Vector2, thickness: f32) -> Rect {
    let rect = Rect {
        x: rect.x + origin.x,
        y: rect.y + origin.y,
        ..rect
    };
    match thickness > 0.0 {
        true => Rect {
            y: rect.y + rect.h - thickness,
            h: thickness,
            ..rect
        },
        false => rect,
    }
}

impl Host {
    /// Mints a field row for `id` and installs its application source.
    ///
    /// Field storage is released on unmount and the slot is generation checked, so a late
    /// callback cannot address a reused control slot.
    pub(crate) fn install_field(&mut self, id: ControlId, source: TextSource) {
        let Some(control) = self.control(id) else {
            return;
        };
        let (node, style) = (control.node, control.scope);
        let Some(key) = control.text else {
            return;
        };
        let group = GroupId(node);
        let caret = self.visual(group, None);
        self.mask(
            caret,
            Mask::Box {
                radius: Corners::default(),
            },
        );
        self.write_channel(caret.0, Prop::Opacity, Value::Scalar(0.0));
        self.fields.place(id, Row::new(key, group, style, caret));
        match source {
            TextSource::Static(text) => self.field_source(id, text),
            TextSource::Owned(text) => self.field_source(id, &text),
            TextSource::Dynamic(read) => {
                // The writer belongs to the signal scope creation installed, so it retires
                // with the subtree that declared this field.
                let mut scratch = String::new();
                self.binding(move || {
                    scratch.clear();
                    read(&mut scratch);
                    Host::with(|host| host.field_source(id, &scratch));
                });
            }
        }
    }

    /// States a field's text-service context, and the fold its run draws under.
    ///
    /// The mask is the run's own fold rather than a second shaped string, so a masked field's
    /// plaintext never reaches the shaper, the coverage or the automation snapshot.
    pub(crate) fn set_field_scope(&mut self, id: ControlId, scope: InputScope) {
        let Some(row) = self.fields.get_mut(id) else {
            return;
        };
        row.scope = scope;
        row.dirty = true;
        let (key, fold) = (
            row.key,
            match scope {
                InputScope::Password => Fold::Mask,
                _ => Fold::None,
            },
        );
        if let Some(node) = self.text.set_fold(key, fold) {
            self.tree.mark(node);
        }
    }

    /// Adopts an application-authored value.
    ///
    /// Source records carry the user revision they were based on, so later typing supersedes
    /// them; an equal echo therefore reaches input as the same bytes and acknowledges without
    /// resetting selection.
    pub(crate) fn field_source(&mut self, id: ControlId, source: &str) {
        let Some(row) = self.fields.get(id) else {
            return;
        };
        let text: Arc<[u16]> = match units(source).eq(row.text.iter().copied()) {
            true => Arc::clone(&row.text),
            false => units(source).collect::<Vec<_>>().into(),
        };
        let based_on = row.callback_revision.unwrap_or(row.revision);
        let scope = row.scope;
        // Before first publication input has no working document for this node. Both
        // halves receive the same sanitized text and initial selection at revision zero.
        if self.tree.c.flags[row.group.0.index()] & super::tree::INITIAL != 0 {
            self.fields.get_mut(id).unwrap().selection = Selection::at(text.len() as u32);
            self.field_text(id, Arc::clone(&text));
        }
        match self.field_sources.iter_mut().find(|held| held.id == id) {
            Some(held) => (held.text, held.based_on, held.scope) = (text, based_on, scope),
            None => self.field_sources.push(Source {
                id,
                scope,
                based_on,
                text,
            }),
        }
    }

    /// Adopts a change the input STA completed.
    ///
    /// Selection and intermediate composition changes are visual updates and carry no commit;
    /// typing, deletion, paste and the end of a composition are completed edits, and Enter and
    /// blur do not duplicate one. A stale revision is a superseded echo and is dropped rather
    /// than applied.
    pub(crate) fn field_update(&mut self, update: &Update) {
        let Some(row) = self.fields.get_mut(update.id) else {
            return;
        };
        if update.revision < row.revision {
            return;
        }
        if update.revision != row.revision || update.selection != row.selection {
            row.reveal = None;
        }
        row.revision = update.revision;
        row.selection = update.selection;
        row.composition.clone_from(&update.composition);
        row.focused = update.focused;
        row.dirty = true;
        if let Some(value) = &update.text {
            self.field_text(update.id, Arc::clone(value));
        }
        if let Some(value) = &update.commit {
            // Coalescing visuals must preserve all completed-edit callbacks in order, so the
            // edit is queued and delivered after its pixels are applied rather than here.
            self.field_commits.push(Commit {
                id: update.id,
                revision: update.revision,
                text: Arc::clone(value),
            });
        }
        self.uia_stale.set(true);
    }

    fn field_text(&mut self, id: ControlId, text: Arc<[u16]>) {
        let Some(row) = self.fields.get_mut(id) else { return; };
        row.text = text;
        row.geometry = None;
        row.dirty = true;
        // The run's fold masks passwords before shaping, coverage and automation.
        let display = String::from_utf16_lossy(&row.text);
        if let Some(node) = self.text.set_text(row.key, &display) {
            self.tree.mark(node);
        }
    }

    pub(crate) fn field_reveal(&mut self, id: ControlId, revision: u64, start: u32, end: u32) {
        let Some(row) = self.fields.get_mut(id) else {
            return;
        };
        if row.revision != revision
            || row.scope == InputScope::Password
            || row.reveal == Some((start, end))
        {
            return;
        }
        row.reveal = Some((start, end));
        row.dirty = true;
        self.uia_stale.set(true);
    }

    /// Publishes each changed field's shaped geometry, decorations and caret.
    ///
    /// Missing geometry makes the text service answer `TS_E_NOLAYOUT`; this publication is
    /// what causes the layout notification that lets input ask again. Geometry-dependent
    /// navigation, deletion and pointer placement queue in order until the matching text
    /// revision arrives, so the revision travels with the geometry.
    ///
    /// Coordinate conversion is input's: this side publishes an origin and a viewport, and
    /// input combines them with the hit table, the live scroll shadow, ancestor clipping, the
    /// window origin and the current scale.
    pub(crate) fn publish_fields(&mut self) {
        // The ids first, then the publications: a publication borrows the host, so the walk
        // cannot hold the table's iterator across one, and finding each next id by a search
        // from the start would make the pass quadratic in the fields mounted.
        let mut ids = core::mem::take(&mut self.field_ids);
        ids.extend(self.fields.iter().map(|(id, _)| id));
        for &id in &ids {
            self.publish_field(id);
        }
        ids.clear();
        self.field_ids = ids;
    }

    fn publish_field(&mut self, id: ControlId) {
        let Some(plan) = self.plan_field(id) else {
            return;
        };
        self.tree.c.driven[plan.node.index()] |=
            (1 << Prop::OffsetX as u32) | (1 << Prop::OffsetY as u32);
        self.write_channel(plan.node, Prop::Offset, Value::Vec2(plan.geometry.origin));
        // The parent clips the viewport; the scrolling sprite must cover the whole run.
        self.write_channel(plan.node, Prop::Size, Value::Vec2(plan.size));
        self.place_caret(id, plan.caret, plan.size.y, plan.geometry.origin);
        self.decorate(id, &plan.geometry, false);
        self.decorate(id, &plan.geometry, true);
        let Self {
            fields,
            field_layouts,
            ..
        } = self;
        let Some(row) = fields.get_mut(id) else {
            return;
        };
        row.layout_revision += 1;
        let mut geometry = plan.geometry;
        geometry.layout_revision = row.layout_revision;
        let geometry = Arc::new(geometry);
        row.geometry = Some(Arc::clone(&geometry));
        field_layouts.push(Layout { id, geometry });
    }

    /// Shapes one field's view and resolves where its text stands, without emitting.
    ///
    /// Every borrow here is short and disjoint: the table's rows, the text table and the geom
    /// column are three fields of the host and are never live together.
    fn plan_field(&mut self, id: ControlId) -> Option<Plan> {
        let (group, style) = {
            let row = self.fields.get(id)?;
            (row.group, row.style)
        };
        let box_ = self.geom(group.0).size;
        let layout = self.tree.c.layout[group.0.index()];
        let inset = layout.padding[0].resolve(
            &self.metrics, self.tree.class(group.0), style, box_.x, self.env.scale(),
        ).unwrap_or(0.0).max(0.0);
        let width = (box_.x - inset * 2.0).max(1.0);
        let (node, end) = {
            let Self { fields, text, .. } = self;
            let row = fields.get_mut(id)?;
            let resized = row.geometry.as_ref().is_none_or(|g| {
                g.viewport.x != inset || g.viewport.w != width || g.viewport.h != box_.y
            }) || row.align != layout.justify;
            if !core::mem::take(&mut row.dirty) && !resized {
                return None;
            }
            row.align = layout.justify;
            let (node, _, end) = text.field_view(row.key, &mut row.clusters)?;
            (node, end)
        };
        let line = self.geom(node).size;
        let row = self.fields.get_mut(id)?;
        // The cluster table is shared with the geometry published against the same text
        // revision, so a caret move re-publishes the boxes it already handed out.
        let clusters = match row
            .geometry
            .as_ref()
            .filter(|held| held.revision == row.revision)
        {
            Some(held) => Arc::clone(&held.clusters),
            None => Arc::from(remap(&row.text, row.scope, &row.clusters).into_owned()),
        };
        let mut geometry = Geometry {
            revision: row.revision,
            clusters,
            end,
            ..Geometry::default()
        };
        let caret = geometry.caret(row.selection);
        let target = row.reveal.map_or((caret.x, caret.x), |(start, end)| {
            if start == end {
                let x = geometry.caret(Selection::at(start)).x;
                return (x, x);
            }
            geometry.rects(start..end, &mut row.rects);
            row.rects
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), r| {
                    (lo.min(r.x), hi.max(r.x + r.w))
                })
        });
        let target = if target.0.is_finite() {
            target
        } else {
            (caret.x, caret.x)
        };
        let right = geometry
            .clusters
            .iter()
            .fold(geometry.end.x, |right, cluster| {
                right.max(cluster.rect.x + cluster.rect.w)
            });
        // Horizontal reveal stays inside the field clip, and shorter text or a wider box must
        // release scroll it no longer needs.
        row.scroll = row
            .scroll
            .max(target.1 - width)
            .min(target.0)
            .clamp(0.0, (right - width).max(0.0));
        geometry.origin = Vector2 {
            x: inset - row.scroll + match row.align {
                Align::Center => (width - right).max(0.0) * 0.5,
                Align::End => (width - right).max(0.0),
                _ => 0.0,
            },
            y: (box_.y - line.y) * 0.5,
        };
        geometry.viewport = Rect {
            x: inset,
            y: 0.0,
            w: width,
            h: box_.y,
        };
        Some(Plan {
            node,
            geometry,
            caret,
            size: Vector2 { x: right.max(line.x), y: line.y },
        })
    }

    /// Places the caret and retargets its blink.
    ///
    /// Width and interval come from the system caret settings, and the blink is a compositor
    /// keyframe animation, so it plays with zero frames on any thread of ours and never
    /// requests an application tick. There is no text timer, polling loop, debounce or
    /// independent clock here. A zero or `u32::MAX` interval is the system asking for no
    /// blink at all.
    fn place_caret(&mut self, id: ControlId, caret: Rect, line_h: f32, origin: Vector2) {
        let Some(row) = self.fields.get(id) else {
            return;
        };
        let (sprite, style) = (row.caret, row.style);
        let shown = row.focused && row.selection.range().is_empty();
        let (width, blink) = system::caret();
        let width = (width as f32 / self.env.scale()).max(metric(Metric::HairlineW, style));
        self.paint(
            sprite,
            Paint::Solid(resolve(Role::Text(Text::Primary), style.for_paint())),
            None,
        );
        self.visual_rect(
            sprite,
            Vector2 {
                x: origin.x + caret.x,
                y: origin.y + caret.y,
            },
            Vector2 {
                x: width,
                y: caret.h.max(line_h),
            },
        );
        let bind = match (shown, blink) {
            (true, 1..=0xFFFF_FFFE) => {
                let frames = self.frames(&[
                    (0.0, Value::Scalar(1.0), Easing::Linear),
                    (0.499, Value::Scalar(1.0), Easing::Linear),
                    (0.5, Value::Scalar(0.0), Easing::Linear),
                    (1.0, Value::Scalar(0.0), Easing::Linear),
                ]);
                Bind::Animate(Anim::Frames {
                    frames,
                    duration_ms: blink.saturating_mul(2),
                    iterations: Iterations::Forever,
                })
            }
            _ => Bind::Set(Value::Scalar(f32::from(shown))),
        };
        self.bind(sprite.0, Prop::Opacity, bind);
    }

    /// Points this field's selection fills or composition underlines at their rects.
    ///
    /// Sprites are pooled and parked at opacity zero rather than destroyed: a selection grows
    /// and shrinks with every arrow key, and minting on that path would churn the far side's
    /// table for a box that returns a keystroke later.
    fn decorate(&mut self, id: ControlId, geometry: &Geometry, underline: bool) {
        let Some(row) = self.fields.get(id) else {
            return;
        };
        let (group, style) = (row.group, row.style);
        let range = decorated(
            row.focused,
            row.selection,
            row.composition.as_ref(),
            underline,
        );
        let wanted = {
            let Self { fields, .. } = self;
            let Some(row) = fields.get_mut(id) else {
                return;
            };
            geometry.rects(range, &mut row.rects);
            row.rects.len()
        };
        let (role, thickness) = match underline {
            true => (Role::Text(Text::Primary), metric(Metric::BorderW, style)),
            false => (Role::Fill(Fill::Selected), 0.0),
        };
        let light = resolve(role, style.for_paint());
        while self
            .fields
            .get(id)
            .is_some_and(|row| pool(row, underline).len() < wanted)
        {
            let sprite = self.visual(group, None);
            self.mask(
                sprite,
                Mask::Box {
                    radius: Corners::default(),
                },
            );
            if let Some(row) = self.fields.get_mut(id) {
                pool_mut(row, underline).push(sprite);
            }
        }
        let held = self
            .fields
            .get(id)
            .map_or(0, |row| pool(row, underline).len());
        for at in 0..held {
            let Some((sprite, rect)) = self
                .fields
                .get(id)
                .map(|row| (pool(row, underline)[at], row.rects.get(at).copied()))
            else {
                return;
            };
            self.write_channel(
                sprite.0,
                Prop::Opacity,
                Value::Scalar(f32::from(rect.is_some())),
            );
            let Some(rect) = rect else { continue };
            let rect = decoration(rect, geometry.origin, thickness);
            self.visual_rect(
                sprite,
                Vector2 {
                    x: rect.x,
                    y: rect.y,
                },
                Vector2 {
                    x: rect.w,
                    y: rect.h,
                },
            );
            self.paint(sprite, Paint::Solid(light), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_publication_shapes_the_source_before_input_echo_and_later_sources_wait() {
        use crate::build::rig::Rig;
        use crate::layout::Len;
        use crate::signal::Cell;
        let mut rig = Rig::new();
        let source = Cell::new("20\n000");
        let mut target = ControlId::NONE;
        let frame = rig.mount(|ui| {
            target = crate::widget::field(ui, crate::widget::reactive(move |out| out.push_str(source.get())))
                .width(Len::dip(120.0)).height(Len::dip(24.0)).control_id();
        });
        let node = Host::with(|host| {
            let row = host.fields.get(target).unwrap();
            assert_eq!(String::from_utf16_lossy(&row.text), "20000");
            assert_eq!(row.revision, 0);
            let geometry = row.geometry.as_ref().unwrap();
            assert_eq!(geometry.revision, 0);
            assert!(geometry.end.x > 0.0);
            assert_eq!(row.selection, Selection::at(5));
            host.text.field_view(row.key, &mut Vec::new()).unwrap().0
        });
        assert!(matches!(frame.bound(node, Prop::Size), Some(Value::Vec2(size)) if size.x > 0.0));
        rig.set(source, "15000");
        Host::with(|host| {
            assert_eq!(String::from_utf16_lossy(&host.fields.get(target).unwrap().text), "20000");
            host.field_update(&Update { id: target, revision: 1, selection: Selection::at(3),
                text: Some("123".encode_utf16().collect::<Vec<_>>().into()), composition: None,
                focused: true, commit: None });
        });
        rig.flush();
        rig.set(source, "9000");
        Host::with(|host| {
            let row = host.fields.get(target).unwrap();
            assert_eq!(String::from_utf16_lossy(&row.text), "123");
            assert_eq!(row.revision, 1);
            assert_eq!(row.selection, Selection::at(3));
        });
    }

    #[test]
    fn revealed_text_covers_the_field_after_edit_and_resize() {
        use crate::build::rig::Rig;
        use crate::layout::Len;
        use crate::signal::Cell;
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let mut rig = Rig::at(400.0, 160.0, scale);
            let width = Cell::new(120.0);
            let mut target = ControlId::NONE;
            rig.mount(|ui| {
                target = crate::widget::field(ui, "")
                    .height(Len::dip(24.0))
                    .layout_from(move |l| l.width = Len::dip(width.get()))
                    .control_id();
            });
            for (revision, value) in [(1, "C:/a/long/path/whose/file/name/must/remain/visible.toml"), (2, "short")] {
                let text: Arc<[u16]> = value.encode_utf16().collect::<Vec<_>>().into();
                Host::with(|host| host.field_update(&Update {
                    id: target,
                    revision,
                    selection: Selection::at(text.len() as u32),
                    text: Some(text),
                    composition: None,
                    focused: true,
                    commit: None,
                }));
                let frame = rig.flush();
                let (node, right) = Host::with(|host| {
                    let row = host.fields.get(target).unwrap();
                    let geometry = row.geometry.as_ref().unwrap();
                    assert_eq!(host.geom(row.group.0).size.x, width.get());
                    let right = geometry.clusters.iter().fold(geometry.end.x, |r, c| r.max(c.rect.x + c.rect.w));
                    assert!(geometry.origin.x + right <= width.get() + 0.01);
                    (host.text.field_view(row.key, &mut Vec::new()).unwrap().0, right)
                });
                assert!(matches!(frame.bound(node, Prop::Size), Some(Value::Vec2(size)) if size.x >= right));
                let resized = rig.set(width, if revision == 1 { 160.0 } else { 80.0 });
                assert!(matches!(resized.bound(node, Prop::Size), Some(Value::Vec2(size)) if size.x >= right));
                Host::with(|host| {
                    let row = host.fields.get(target).unwrap();
                    let geometry = row.geometry.as_ref().unwrap();
                    assert_eq!(host.geom(row.group.0).size.x, width.get());
                    assert!(geometry.origin.x + right <= width.get() + 0.01);
                    assert!(host.plan_field(target).is_none());
                });
                assert!(rig.flush().patch().ops().is_empty());
            }
        }
    }

    #[test]
    fn fields_honor_authored_padding_and_alignment_without_reshaping() {
        use crate::build::rig::Rig;
        use crate::layout::Len;
        use crate::signal::Cell;
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let mut rig = Rig::at(400.0, 160.0, scale);
            let align = Cell::new(Align::Start);
            let inset = Cell::new(4.0);
            let mut target = ControlId::NONE;
            rig.mount(|ui| {
                target = crate::widget::field(ui, "20000")
                    .width(Len::dip(120.0))
                    .height(Len::dip(24.0))
                    .layout_from(move |l| {
                        l.padding[0] = Len::dip(inset.get());
                        l.justify = align.get();
                    })
                    .control_id();
            });
            rig.flush();
            let clusters = Host::with(|host| Arc::clone(
                &host.fields.get(target).unwrap().geometry.as_ref().unwrap().clusters,
            ));
            for (alignment, padding) in [(Align::Start, 4.0), (Align::Center, 4.0), (Align::End, 9.0)] {
                rig.set(align, alignment);
                rig.set(inset, padding);
                rig.flush();
                Host::with(|host| {
                    let row = host.fields.get(target).unwrap();
                    let g = row.geometry.as_ref().unwrap();
                    assert!(Arc::ptr_eq(&clusters, &g.clusters));
                    assert_eq!(g.viewport.x, padding);
                    assert_eq!(g.viewport.w, 120.0 - 2.0 * padding);
                    let right = g.clusters.iter().fold(g.end.x, |r, c| r.max(c.rect.x + c.rect.w));
                    let fraction = match alignment { Align::Center => 0.5, Align::End => 1.0, _ => 0.0 };
                    assert!((g.origin.x - padding - (g.viewport.w - right) * fraction).abs() < 0.01);
                    let previous = Arc::clone(g);
                    assert!(host.plan_field(target).is_none());
                    assert!(Arc::ptr_eq(&previous.clusters, &host.fields.get(target).unwrap().geometry.as_ref().unwrap().clusters));
                });
            }
        }
    }

    /// A masked run draws one dot per character, so the span a dot stands for is two units
    /// where that character is a surrogate pair.
    #[test]
    fn a_masked_cluster_keeps_the_span_of_the_character_it_hides() {
        let text: Vec<u16> = "a\u{1f600}".encode_utf16().collect();
        assert_eq!(source_units(&text, 0), 1);
        assert_eq!(source_units(&text, 1), 2);
        assert_eq!(source_units(&text, 9), 1, "past the end names one unit");
    }

    /// The whole of a password field's cluster table is the source's own positions, so a
    /// client that places a range reads back masked text at the offsets it named.
    #[test]
    fn a_password_remap_covers_the_source_and_a_plain_field_borrows() {
        let text: Vec<u16> = "a\u{1f600}".encode_utf16().collect();
        let shaped = [Cluster::default(), Cluster::default()];
        assert!(matches!(
            remap(&text, InputScope::Default, &shaped),
            Cow::Borrowed(_)
        ));
        let masked = remap(&text, InputScope::Password, &shaped);
        assert_eq!(
            masked[0],
            Cluster {
                start: 0,
                end: 1,
                ..Cluster::default()
            }
        );
        assert_eq!(masked.last().unwrap().end, 3, "the mask covers the pair");
    }

    /// A newline never reaches a single-line buffer, whichever separator it arrives as.
    #[test]
    fn a_source_value_loses_every_line_separator() {
        let held: Vec<u16> = units("a\r\nb\u{2028}c").collect();
        assert_eq!(String::from_utf16_lossy(&held), "abc");
    }

    /// An underline is a rule on the box's baseline edge; a selection fill is the whole box.
    #[test]
    fn an_underline_is_a_rule_on_the_edge_its_fill_would_cover() {
        let box_ = Rect {
            x: 1.0,
            y: 2.0,
            w: 10.0,
            h: 20.0,
        };
        let origin = Vector2 { x: 3.0, y: 4.0 };
        assert_eq!(
            decoration(box_, origin, 0.0),
            Rect {
                x: 4.0,
                y: 6.0,
                w: 10.0,
                h: 20.0
            }
        );
        assert_eq!(
            decoration(box_, origin, 2.0),
            Rect {
                x: 4.0,
                y: 24.0,
                w: 10.0,
                h: 2.0
            }
        );
    }

    /// Nothing is drawn for an unfocused field, whichever range it holds.
    #[test]
    fn an_unfocused_field_draws_no_selection_and_no_underline() {
        let selection = Selection {
            anchor: 0,
            caret: 3,
            ..Selection::default()
        };
        let composition = 0..2;
        assert!(decorated(false, selection, Some(&composition), false).is_empty());
        assert!(decorated(false, selection, Some(&composition), true).is_empty());
        assert_eq!(decorated(true, selection, Some(&composition), false), 0..3);
        assert_eq!(decorated(true, selection, Some(&composition), true), 0..2);
        assert!(decorated(true, selection, None, true).is_empty());
    }
}
