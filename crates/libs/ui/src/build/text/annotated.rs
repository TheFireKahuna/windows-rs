use super::*;
use crate::widget::TextAnnotation;

pub(super) struct Part {
    pub sprite: SpriteId,
    pub run: RunId,
    tag: u32,
}

pub(crate) struct Annotated {
    group: GroupId,
    annotate: fn(&str, &mut Vec<TextAnnotation>),
    spans: Vec<TextAnnotation>,
    inks: Vec<Role>,
    pub(super) parts: Vec<Part>,
}

impl Annotated {
    pub fn new(group: GroupId, annotate: fn(&str, &mut Vec<TextAnnotation>)) -> Self {
        Self {
            group,
            annotate,
            spans: Vec::new(),
            inks: Vec::new(),
            parts: Vec::new(),
        }
    }

    pub fn apply(&mut self, engine: &TextEngine, run: &mut ShapedRun, text: &str) {
        self.spans.clear();
        self.inks.clear();
        (self.annotate)(text, &mut self.spans);
        let (mut byte, mut utf16) = (0, 0);
        for span in &self.spans {
            assert!(
                span.range.start >= byte
                    && span.range.start <= span.range.end
                    && text.is_char_boundary(span.range.start)
                    && text.is_char_boundary(span.range.end),
                "text annotations must be ordered nonoverlapping character ranges"
            );
            let tag = self
                .inks
                .iter()
                .position(|ink| *ink == span.ink)
                .unwrap_or_else(|| {
                    self.inks.push(span.ink);
                    self.inks.len() - 1
                }) as u32
                + 1;
            utf16 += text[byte..span.range.start].encode_utf16().count() as u32;
            let end = utf16 + text[span.range.clone()].encode_utf16().count() as u32;
            engine
                .annotate(run, utf16..end, tag, span.weight)
                .expect(LAYOUT);
            byte = span.range.end;
            utf16 = end;
        }
    }
}

impl Host {
    pub(super) fn paint_annotated(&mut self, at: usize, default: Role, scope: Scope) -> bool {
        let Some(Entry {
            target: Target::Annotated(data),
            ..
        }) = self.text.entries.at(at)
        else {
            return false;
        };
        let count = data.parts.len();
        for i in 0..count {
            let Some(Entry {
                target: Target::Annotated(data),
                ..
            }) = self.text.entries.at(at)
            else {
                unreachable!()
            };
            let part = &data.parts[i];
            let ink = part
                .tag
                .checked_sub(1)
                .map_or(default, |tag| data.inks[tag as usize]);
            self.paint(
                part.sprite,
                Paint::Solid(resolve(ink, scope.for_paint())),
                None,
            );
        }
        true
    }

    pub(super) fn emit_annotated(&mut self, at: usize, lines: usize) -> bool {
        let Some(Entry {
            target: Target::Annotated(data),
            style,
            ..
        }) = self.text.entries.at(at)
        else {
            return false;
        };
        let (group, colors, vertical) = (data.group, data.inks.len() as u32, style.vertical);
        let (mut slot, mut top) = (0, 0.0);
        for line in 0..lines {
            let size = self.text.entries.at(at).unwrap().run.line_ink(line).size;
            for tag in 0..=colors {
                if !self
                    .text
                    .entries
                    .at(at)
                    .unwrap()
                    .run
                    .line_tags(line)
                    .any(|value| value == tag)
                {
                    continue;
                }
                let offset = self
                    .text
                    .entries
                    .at(at)
                    .unwrap()
                    .run
                    .tagged_ink(line, tag)
                    .1;
                let offset = (offset * self.env.scale()).floor() / self.env.scale();
                let (span, ink) = self.coverage(at, line, Some(tag));
                let existing = match &self.text.entries.at(at).unwrap().target {
                    Target::Annotated(data) => data.parts.get(slot).map(|p| (p.sprite, p.run)),
                    _ => unreachable!(),
                };
                let (sprite, run) = existing.unwrap_or_else(|| {
                    let after = match &self.text.entries.at(at).unwrap().target {
                        Target::Annotated(data) => data.parts.last().map(|p| p.sprite.0),
                        _ => unreachable!(),
                    };
                    let sprite = self.visual(group, after);
                    let run = self.run(Span::default(), Ink::default());
                    self.mask(sprite, Mask::Run(run));
                    if vertical {
                        self.write_channel(
                            sprite.0,
                            Prop::RotationAngle,
                            Value::Scalar(core::f32::consts::FRAC_PI_2),
                        );
                    }
                    if let Target::Annotated(data) =
                        &mut self.text.entries.at_mut(at).unwrap().target
                    {
                        data.parts.push(Part { sprite, run, tag });
                    }
                    (sprite, run)
                });
                if let Target::Annotated(data) = &mut self.text.entries.at_mut(at).unwrap().target {
                    data.parts[slot].tag = tag;
                }
                self.set_run(run, span, ink);
                self.visual_rect(
                    sprite,
                    Vector2 {
                        x: if vertical { ink.size.y } else { offset },
                        y: top + if vertical { offset } else { 0.0 },
                    },
                    ink.size,
                );
                slot += 1;
            }
            top += if vertical { size.x } else { size.y };
        }
        loop {
            let Target::Annotated(data) = &mut self.text.entries.at_mut(at).unwrap().target else {
                unreachable!()
            };
            if data.parts.len() <= slot {
                break;
            }
            let part = data.parts.pop().unwrap();
            self.destroy(part.sprite.0, Exit::None);
            self.release(part.run);
        }
        self.paint_run(at);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::role::TypeRole;
    use crate::signal::Cell;
    use crate::widget::reactive;
    use windows_scene::Op;

    fn annotate(source: &str, out: &mut Vec<TextAnnotation>) {
        if let Some(end) = source.find(' ') {
            out.push(TextAnnotation {
                range: 0..end,
                ink: Role::Text(Text::Secondary),
                weight: Some(600),
            });
            out.push(TextAnnotation {
                range: end..source.len(),
                ink: Role::Text(Text::Primary),
                weight: None,
            });
        }
    }

    #[test]
    fn annotations_reuse_parts_preserve_source_reflow_and_retire() {
        for dpi in [96.0, 144.0, 192.0] {
            let mut patch = crate::build::rig::fixture_at(dpi);
            let initial_nodes = Host::with(|host| host.live_nodes());
            let source = Cell::new("日本😀 value with words\nsecond line");
            let mut node = NodeId::default();
            let mounted = Ui::mount_root(|ui| {
                node = ui
                    .annotated_text(
                        TextStyle::new(TypeRole::Body)
                            .flow(Flow::Wrap)
                            .line_height(1.6),
                        reactive(move |out| out.push_str(source.get())),
                        annotate,
                    )
                    .id()
                    .into();
            });
            Host::flush(&mut patch);
            let key = Host::with(|host| host.tree.c.text[node.index()]);
            let inspect = || {
                Host::with(|host| {
                    let entry = host.text.entries.get(key).unwrap();
                    let Target::Annotated(data) = &entry.target else {
                        panic!("annotated target")
                    };
                    assert_eq!(host.text.str_of(key), Some(source.get()));
                    assert_eq!(entry.run.len(), source.get().encode_utf16().count() as u32);
                    assert!(
                        entry
                            .run
                            .lines()
                            .iter()
                            .all(|line| (line.height - entry.font.size * 1.6).abs() < 0.001)
                    );
                    assert!(data.parts.iter().any(|part| part.tag != 0));
                    data.parts
                        .iter()
                        .map(|part| (part.sprite, part.run))
                        .collect::<Vec<_>>()
                })
            };
            let before = inspect();
            Host::with(|host| {
                let mut full = SegBuffers::default();
                let entry = host.text.entries.get(key).unwrap();
                entry.run.segments(0, &mut full);
                let offsets: Vec<_> = (1..=2).map(|tag| entry.run.tagged_ink(0, tag).1).collect();
                for (i, offset) in offsets.into_iter().enumerate() {
                    let placed = (offset * host.env.scale()).floor() / host.env.scale();
                    let (span, _) = host.coverage(key.at as usize, 0, Some(i as u32 + 1));
                    for seg in host.pending.segs(span) {
                        assert!(full.segs.iter().any(|original| {
                            original.face == seg.face && original.bidi == seg.bidi
                                && (original.origin.x - seg.origin.x - placed).abs() < 0.001
                                && original.origin.y == seg.origin.y
                                && original.glyphs.of(&full.glyphs) == host.pending.glyphs(seg.glyphs)
                        }), "cropping shifted the shaped glyph origin");
                    }
                }
            });
            patch.clear();
            Host::flush(&mut patch);
            assert!(patch.ops().is_empty());
            let idle_allocations = crate::counting::allocations();
            for _ in 0..20 {
                Host::flush(&mut patch);
            }
            assert_eq!(crate::counting::allocations(), idle_allocations);
            source.set("日本😀 value with words\nthird line");
            Host::flush(&mut patch);
            assert_eq!(inspect(), before);
            for width in [90.0, 220.0, 90.0] {
                Host::with(|host| host.set_window(Vector2 { x: width, y: 600.0 }));
                patch.clear();
                Host::flush(&mut patch);
                inspect();
                patch.clear();
                Host::flush(&mut patch);
                assert!(patch.ops().is_empty());
            }
            source.set("plain");
            Host::flush(&mut patch);
            Host::with(|host| {
                let Target::Annotated(data) = &host.text.entries.get(key).unwrap().target else {
                    panic!()
                };
                assert_eq!(data.parts.len(), 1);
                assert_eq!(data.parts[0].tag, 0);
            });
            source.set("");
            Host::flush(&mut patch);
            Host::with(|host| {
                let Target::Annotated(data) = &host.text.entries.get(key).unwrap().target else {
                    panic!()
                };
                assert!(data.parts.is_empty());
            });
            drop(mounted);
            patch.clear();
            Host::flush(&mut patch);
            assert!(patch.ops().iter().any(|op| matches!(op, Op::Drop { .. })));
            assert_eq!(Host::with(|host| host.live_nodes()), initial_nodes);
            assert!(Host::with(|host| host.text.entries.get(key).is_none()));
        }
    }
}
