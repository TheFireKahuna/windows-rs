//! App half of fields: callbacks and shaping. The working document remains input-owned.

use super::{Host, text};
use crate::role::{Fill, Role, Scope, Text};
use crate::text_input::{Commit, Geometry, InputScope, Layout, Selection, Source, Update};
use std::{rc::Rc, sync::Arc};
use windows_numerics::Vector2;
use windows_scene::{
    Bind, Clip, ControlId, Corners, Easing, GroupId, Iterations, Mask, MeasureKey, Paint, Prop,
    SpriteId, Value, taffy,
};

pub(crate) struct Row {
    key: MeasureKey,
    group: GroupId,
    pub scope: InputScope,
    style: Scope,
    pub callback: Option<Rc<dyn Fn(&str)>>,
    pub revision: u64,
    layout_revision: u64,
    pub callback_revision: Option<u64>,
    pub delivered_revision: Option<u64>,
    pub text: Arc<[u16]>,
    pub selection: Selection,
    composition: Option<core::ops::Range<u32>>,
    focused: bool,
    dirty: bool,
    scroll: f32,
    last_size: Vector2,
    last_origin: Vector2,
    font: Option<windows_text::FontSpec>,
    caret: SpriteId,
    selections: Vec<SpriteId>,
    underlines: Vec<SpriteId>,
    pub geometry: Option<Arc<Geometry>>,
    scratch: Vec<windows_text::Rect>,
    password_run: Option<windows_text::ShapedRun>,
    password_map: Vec<(u32, u32)>,
}

impl Host {
    pub(crate) fn install_field(
        &mut self,
        id: ControlId,
        group: GroupId,
        key: MeasureKey,
        scope: InputScope,
        style: Scope,
        callback: Option<Rc<dyn Fn(&str)>>,
    ) {
        let caret = self.model().sprite(group, None);
        self.model().style(
            caret.node(),
            &taffy::Style {
                position: taffy::Position::Absolute,
                ..taffy::Style::DEFAULT
            },
        );
        self.model().mask(
            caret,
            Mask::Box {
                radius: Corners::default(),
            },
        );
        self.model()
            .bind(caret.node(), Prop::Opacity, Bind::Set(Value::Scalar(0.0)));
        self.model().clip(
            group.node(),
            Clip::Rect {
                l: 0.0,
                t: 0.0,
                r: 0.0,
                b: 0.0,
                radius: Corners::default(),
            },
        );
        // Text is absolute: editing a long value cannot widen its field or its neighbours.
        if let Some((node, _)) = text::with(|t| t.field_geometry(key, 0)) {
            let mut style = taffy::Style::DEFAULT;
            style.position = taffy::Position::Absolute;
            self.model().style(node, &style);
        }
        self.fields.place(
            id,
            Row {
                key,
                group,
                scope,
                style,
                callback,
                revision: 0,
                layout_revision: 0,
                callback_revision: None,
                delivered_revision: None,
                text: Arc::from([]),
                selection: Selection::default(),
                composition: None,
                focused: false,
                dirty: true,
                scroll: 0.0,
                last_size: Vector2::default(),
                last_origin: Vector2::default(),
                font: None,
                caret,
                selections: Vec::new(),
                underlines: Vec::new(),
                geometry: None,
                scratch: Vec::new(),
                password_run: None,
                password_map: Vec::new(),
            },
        );
    }

    pub(crate) fn field_source(&mut self, id: ControlId, source: &str) {
        let Some(row) = self.fields.get(id) else {
            return;
        };
        let units = || {
            source
                .encode_utf16()
                .filter(|u| !matches!(u, 10 | 13 | 0x0085 | 0x2028 | 0x2029))
        };
        let value: Arc<[u16]> = if units().eq(row.text.iter().copied()) {
            row.text.clone()
        } else {
            units().collect::<Vec<_>>().into()
        };
        if let Some(old) = self.field_sources.iter_mut().find(|s| s.id == id) {
            old.text = value;
            old.based_on = row.callback_revision.unwrap_or(row.revision);
        } else {
            self.field_sources.push(Source {
                id,
                scope: row.scope,
                based_on: row.callback_revision.unwrap_or(row.revision),
                text: value,
            });
        }
    }

    pub(crate) fn field_update(&mut self, update: &Update) {
        let Some(row) = self.fields.get_mut(update.id) else {
            return;
        };
        if update.revision < row.revision {
            return;
        }
        row.revision = update.revision;
        row.selection = update.selection;
        row.composition.clone_from(&update.composition);
        row.focused = update.focused;
        row.dirty = true;
        if let Some(value) = &update.text {
            row.text = Arc::clone(value);
            row.geometry = None;
            let original = String::from_utf16_lossy(value);
            let display = if row.scope == InputScope::Password {
                text::with(|t| {
                    t.password_display(
                        row.key,
                        &original,
                        &mut row.password_run,
                        &mut row.password_map,
                    )
                })
            } else {
                original
            };
            let node = text::with(|t| t.set_text(row.key, &display));
            if let Some(node) = node {
                self.model().remeasure(node);
            }
        }
        if let Some(value) = &update.commit {
            self.field_commits.push(Commit {
                id: update.id,
                revision: update.revision,
                text: Arc::clone(value),
            });
        }
        self.uia_restale();
    }

    pub(crate) fn retheme_fields(&mut self, root: Scope) {
        for (_, row) in self.fields.iter_mut() {
            row.style = row.style.in_theme(root);
            row.dirty = true;
        }
    }

    pub(crate) fn publish_fields(&mut self) {
        // Split ownership instead of re-entering Host while shaping publishes geometry.
        let mut fields = core::mem::take(&mut self.fields);
        for (id, row) in fields.iter_mut() {
            let solved = self.model().solved(row.group.node());
            let Some((node, font)) = text::with(|t| t.field_font(row.key)) else {
                continue;
            };
            if !row.dirty
                && row.last_size == solved.size
                && row.last_origin == solved.local
                && row.font == Some(font)
            {
                continue;
            }
            let mut geometry = if row.font == Some(font)
                && row
                    .geometry
                    .as_ref()
                    .is_some_and(|g| g.revision == row.revision)
            {
                (**row.geometry.as_ref().unwrap()).clone()
            } else {
                let Some((_, mut geometry)) =
                    text::with(|t| t.field_geometry(row.key, row.revision))
                else {
                    continue;
                };
                if row.scope == InputScope::Password {
                    for cluster in Arc::make_mut(&mut geometry.clusters) {
                        if let Some(&(start, _)) = row.password_map.get(cluster.start as usize) {
                            let end = row
                                .password_map
                                .get(cluster.end.saturating_sub(1) as usize)
                                .map_or(start, |r| r.1);
                            cluster.start = start;
                            cluster.end = end;
                        }
                    }
                }
                geometry
            };
            row.font = Some(font);
            let line = self.model().solved(node);
            let inset = crate::layout::Len::Metric(crate::role::Metric::SpaceSm)
                .dips(row.style)
                .unwrap_or(0.0);
            let width = (solved.size.x - inset * 2.0).max(1.0);
            let caret = geometry.caret(row.selection);
            // Shorter source text or a wider field must release obsolete scroll.
            row.scroll = row
                .scroll
                .min(caret.x)
                .max(caret.x - width)
                .max(0.0)
                .min((line.size.x - width).max(0.0));
            geometry.origin = Vector2 {
                x: inset - row.scroll,
                y: (solved.size.y - line.size.y) * 0.5,
            };
            geometry.viewport = windows_text::Rect {
                x: inset,
                y: 0.0,
                w: width,
                h: solved.size.y,
            };
            self.model()
                .bind(node, Prop::Offset, Bind::Set(Value::Vec2(geometry.origin)));
            self.model().paint(
                row.caret,
                Paint::Solid(crate::role::resolve(
                    Role::Text(Text::Primary),
                    row.style.for_paint(),
                )),
            );
            let (caret_width, blink) = crate::text_input::settings::caret();
            let caret_width = (caret_width as f32 / self.env.scale()).max(crate::role::metric(
                crate::role::Metric::HairlineW,
                row.style,
            ));
            place(
                self.model(),
                row.caret,
                windows_text::Rect {
                    x: geometry.origin.x + caret.x,
                    y: geometry.origin.y + caret.y,
                    w: caret_width,
                    h: caret.h.max(line.size.y),
                },
            );
            if row.focused && row.selection.range().is_empty() && blink != 0 && blink != u32::MAX {
                let anim = self.model().frames(
                    &[
                        (0.0, Value::Scalar(1.0), Easing::Linear),
                        (0.499, Value::Scalar(1.0), Easing::Linear),
                        (0.5, Value::Scalar(0.0), Easing::Linear),
                        (1.0, Value::Scalar(0.0), Easing::Linear),
                    ],
                    blink.saturating_mul(2),
                    Iterations::Forever,
                );
                self.model()
                    .bind(row.caret.node(), Prop::Opacity, Bind::Animate(anim));
            } else {
                self.model().bind(
                    row.caret.node(),
                    Prop::Opacity,
                    Bind::Set(Value::Scalar(
                        if row.focused && row.selection.range().is_empty() {
                            1.0
                        } else {
                            0.0
                        },
                    )),
                );
            }
            geometry.rects(row.selection.range(), &mut row.scratch);
            if !row.focused {
                row.scratch.clear();
            }
            decoration(
                self.model(),
                row.group,
                &mut row.selections,
                &row.scratch,
                geometry.origin,
                row.style,
                false,
            );
            geometry.rects(row.composition.clone().unwrap_or(0..0), &mut row.scratch);
            if !row.focused {
                row.scratch.clear();
            }
            decoration(
                self.model(),
                row.group,
                &mut row.underlines,
                &row.scratch,
                geometry.origin,
                row.style,
                true,
            );
            row.layout_revision += 1;
            geometry.layout_revision = row.layout_revision;
            let geometry = Arc::new(geometry);
            row.geometry = Some(Arc::clone(&geometry));
            self.field_layouts.push(Layout { id, geometry });
            row.dirty = false;
            row.last_size = solved.size;
            row.last_origin = solved.local;
        }
        self.fields = fields;
    }
}

fn place(model: &mut windows_scene::Model, sprite: SpriteId, rect: windows_text::Rect) {
    model.bind(
        sprite.node(),
        Prop::Offset,
        Bind::Set(Value::Vec2(Vector2 {
            x: rect.x,
            y: rect.y,
        })),
    );
    model.bind(
        sprite.node(),
        Prop::Size,
        Bind::Set(Value::Vec2(Vector2 {
            x: rect.w,
            y: rect.h,
        })),
    );
}

fn decoration(
    model: &mut windows_scene::Model,
    group: GroupId,
    sprites: &mut Vec<SpriteId>,
    rects: &[windows_text::Rect],
    origin: Vector2,
    scope: Scope,
    underline: bool,
) {
    let role = if underline {
        Role::Text(Text::Primary)
    } else {
        Role::Fill(Fill::Selected)
    };
    while sprites.len() < rects.len() {
        let sprite = model.sprite(group, None);
        model.style(
            sprite.node(),
            &taffy::Style {
                position: taffy::Position::Absolute,
                ..taffy::Style::DEFAULT
            },
        );
        model.mask(
            sprite,
            Mask::Box {
                radius: Corners::default(),
            },
        );
        sprites.push(sprite);
    }
    for (i, &sprite) in sprites.iter().enumerate() {
        let shown = i < rects.len();
        model.bind(
            sprite.node(),
            Prop::Opacity,
            Bind::Set(Value::Scalar(if shown { 1.0 } else { 0.0 })),
        );
        if let Some(rect) = rects.get(i) {
            let mut rect = *rect;
            rect.x += origin.x;
            rect.y += origin.y;
            if underline {
                let thickness = crate::role::metric(crate::role::Metric::BorderW, scope);
                rect.y += rect.h - thickness;
                rect.h = thickness;
            }
            place(model, sprite, rect);
            model.paint(
                sprite,
                Paint::Solid(crate::role::resolve(role, scope.for_paint())),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{mount, tests::fixture};
    use crate::text_input::{Affinity, Command, Editor};

    #[test]
    fn field_shapes_on_app_reuses_clusters_for_selection_and_releases_on_unmount() {
        let mut patch = fixture();
        let root = Host::with(|h| h.model().root());
        let mounted = mount(crate::widget::field("á😀ffi العربية"), root);
        Host::flush(&mut patch);
        let source = Host::with(|h| h.field_sources[0].clone());
        let mut editor = Editor::new(source.id, source.scope, &source.text);
        editor.publish(true, false);
        Host::with(|h| {
            for update in editor.updates.drain(..) {
                h.field_update(&update);
            }
        });
        Host::flush(&mut patch);
        let first = Host::with(|h| h.fields.get(source.id).unwrap().geometry.clone().unwrap());
        assert!(!first.clusters.is_empty());
        assert!(
            first.clusters.iter().all(|c| c.start != 1 && c.start != 3),
            "combining marks and surrogate pairs stay together"
        );
        editor.layout(first.clone());
        editor.command(Command::End { select: false });
        Host::with(|h| {
            for update in editor.updates.drain(..) {
                h.field_update(&update);
            }
        });
        Host::flush(&mut patch);
        let second = Host::with(|h| h.fields.get(source.id).unwrap().geometry.clone().unwrap());
        assert!(Arc::ptr_eq(&first.clusters, &second.clusters));
        assert_eq!(editor.selection().affinity, Affinity::Upstream);
        drop(mounted);
        assert!(Host::with(|h| h.fields.get(source.id).is_none()));
    }

    #[test]
    fn shorter_source_on_blur_releases_horizontal_scroll() {
        let mut patch = fixture();
        let root = Host::with(|h| h.model().root());
        let _mounted = mount(
            crate::widget::field("1234.56789123456789").width(crate::layout::Len::Pct(0.1)),
            root,
        );
        Host::flush(&mut patch);
        let source = Host::with(|h| h.field_sources[0].clone());
        let mut editor = Editor::new(source.id, source.scope, &source.text);
        editor.publish(true, false);
        editor.focus(true);
        Host::with(|h| {
            for update in editor.updates.drain(..) {
                h.field_update(&update);
            }
        });
        Host::flush(&mut patch);
        editor.layout(Host::with(|h| {
            h.fields.get(source.id).unwrap().geometry.clone().unwrap()
        }));
        editor.command(Command::End { select: false });
        Host::with(|h| {
            for update in editor.updates.drain(..) {
                h.field_update(&update);
            }
        });
        Host::flush(&mut patch);
        Host::with(|h| {
            assert!(h.fields.get(source.id).unwrap().scroll > 0.0);
        });
        editor.source(
            editor.revision,
            "1235".encode_utf16().collect::<Vec<_>>().into(),
        );
        assert!(
            editor.updates.is_empty(),
            "source stays deferred while editing"
        );
        editor.focus(false);
        Host::with(|h| {
            for update in editor.updates.drain(..) {
                assert!(
                    update.commit.is_none(),
                    "formatting does not commit an edit"
                );
                h.field_update(&update);
            }
        });
        Host::flush(&mut patch);
        Host::with(|h| {
            let row = h.fields.get(source.id).unwrap();
            assert_eq!(String::from_utf16_lossy(&row.text), "1235");
            assert_eq!(row.scroll, 0.0, "short text starts at the field inset");
            let geometry = row.geometry.as_ref().unwrap();
            assert_eq!(geometry.origin.x, geometry.viewport.x);
        });
    }

    #[test]
    fn password_geometry_keeps_original_acp_and_uia_never_contains_plaintext() {
        let mut patch = fixture();
        let root = Host::with(|h| h.model().root());
        let _mounted = mount(
            crate::widget::field("á😀")
                .scope(InputScope::Password)
                .name("Password"),
            root,
        );
        Host::flush(&mut patch);
        let source = Host::with(|h| h.field_sources[0].clone());
        let mut editor = Editor::new(source.id, source.scope, &source.text);
        editor.publish(true, false);
        let mut seeds = crate::uia::Seeds::default();
        Host::with(|h| {
            for update in editor.updates.drain(..) {
                h.field_update(&update);
            }
        });
        Host::flush(&mut patch);
        Host::with(|h| {
            h.uia_seeds(&mut seeds);
        });
        let field = &seeds.fields[0];
        assert!(field.password);
        assert!(field.text.is_empty());
        assert!(field.geometry.is_none());
        assert!(!String::from_utf16_lossy(&seeds.blob).contains("á"));
        let geometry = Host::with(|h| h.fields.get(source.id).unwrap().geometry.clone().unwrap());
        assert_eq!(geometry.clusters.last().unwrap().end, 4);
    }
}
