//! `TextPattern`, over the tree's own string blob.
//!
//! A surface whose text a user reads, selects and copies but never edits publishes this
//! pattern rather than an edit field: an edit field hands ownership to TSF and raises a
//! touch keyboard over a surface with nothing to type into. Nothing here creates a
//! text-services object.
//!
//! The blob is UTF-16 and automation's text offsets are UTF-16 offsets, so a range is two
//! integers into a slice the tree already holds. Storing the strings as `str` would cost an
//! offset table per element to say the same thing.

use super::element::{At, Element_Impl, Shared, gone, none};
use super::tree::{Tree, Value};
use crate::bindings::{
    IRawElementProviderSimple, ITextRangeProvider, SAFEARRAY, SupportedTextSelection,
    SupportedTextSelection_Single, TEXTATTRIBUTEID, TextPatternRangeEndpoint,
    TextPatternRangeEndpoint_Start, TextUnit, TextUnit_Character, TextUnit_Document,
    TextUnit_Format, TextUnit_Line, TextUnit_Page, TextUnit_Paragraph, TextUnit_Word, UiaPoint,
    VARIANT,
};
use core::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Weak};
use windows_core::{BOOL, BSTR, Interface, Ref, Result, implement_decl};
use windows_scene::ControlId;

/// One range over one element's text.
///
/// The endpoints are atomics because automation mutates a range in place — `Move`,
/// `ExpandToEnclosingUnit` and `MoveEndpointByUnit` each edit the object they are called on
/// — and the object is callable from any thread, like every provider here.
pub struct Range {
    shared: Weak<Shared>,
    owner: ControlId,
    start: AtomicU32,
    end: AtomicU32,
}

implement_decl! {
    impl Range as pub Range_Impl: [ITextRangeProvider]
}

impl Range {
    fn new(shared: &Arc<Shared>, owner: ControlId, start: u32, end: u32) -> Self {
        Self {
            shared: Arc::downgrade(shared),
            owner,
            start: AtomicU32::new(start),
            end: AtomicU32::new(end),
        }
    }

    /// Returns the element's whole text, with this range's endpoints clamped into it.
    ///
    /// The clamp runs on every read rather than on every write, because a republish can
    /// replace the text while a client still holds a range over the old one. A stale
    /// endpoint then reads as a short range instead of panicking or slicing another
    /// element's string.
    fn body(&self) -> Result<(Arc<Tree>, Arc<[u16]>, u32, u32)> {
        let shared = self.shared.upgrade().ok_or_else(gone)?;
        let tree = super::element::tree_of(&shared);
        let at = tree.index_of(self.owner).ok_or_else(gone)?;
        let col = tree.col(at).ok_or_else(gone)?;
        let text = tree.field(self.owner).map_or_else(
            || Arc::from(tree.text(col.name)),
            |field| Arc::clone(&field.text),
        );
        let len = text.len() as u32;
        let start = self.start.load(Relaxed).min(len);
        let end = self.end.load(Relaxed).clamp(start, len);
        Ok((tree, text, start, end))
    }

    fn walk(
        &self,
        tree: &Tree,
        text: &[u16],
        from: u32,
        unit: TextUnit,
        count: i32,
    ) -> Result<(u32, i32)> {
        let Some(field) = tree
            .field(self.owner)
            .filter(|_| unit == TextUnit_Character)
        else {
            return Ok(walk(text, from, unit, count));
        };
        let geometry = field
            .geometry
            .as_ref()
            .filter(|g| g.revision == field.revision)
            .ok_or_else(none)?;
        let mut at = from.min(text.len() as u32);
        let mut moved = 0;
        for _ in 0..count.unsigned_abs() {
            let next = if count > 0 {
                geometry.next(at)
            } else {
                geometry.previous(at)
            };
            if next == at {
                break;
            }
            at = next;
            moved += if count > 0 { 1 } else { -1 };
        }
        Ok((at, moved))
    }

    fn span(&self) -> (u32, u32) {
        (self.start.load(Relaxed), self.end.load(Relaxed))
    }

    /// Stores both endpoints, holding `end` at or after `start` so a range cannot invert.
    fn set(&self, start: u32, end: u32) {
        self.start.store(start, Relaxed);
        self.end.store(end.max(start), Relaxed);
    }

    fn object(&self, start: u32, end: u32) -> Result<ITextRangeProvider> {
        let shared = self.shared.upgrade().ok_or_else(gone)?;
        Ok(Self::new(&shared, self.owner, start, end).into())
    }
}

/// Returns the offset `count` unit boundaries from `from`, and how many were crossed.
///
/// A negative `count` walks backward and the crossing count comes back negative to match.
/// The walk stops at either end of the text, so the count is what tells a caller a partial
/// move from a complete one.
///
/// Word boundaries are whitespace transitions and line boundaries are newlines. `Document`,
/// `Page` and `Format` each span the whole text: the text carries one paint, so a format
/// boundary is the document boundary.
fn walk(text: &[u16], from: u32, unit: TextUnit, count: i32) -> (u32, i32) {
    let len = text.len() as u32;
    if count == 0 || len == 0 {
        return (from.min(len), 0);
    }
    let forward = count > 0;
    let mut at = from.min(len);
    let mut moved = 0;
    for _ in 0..count.unsigned_abs() {
        let next = match unit {
            _ if unit == TextUnit_Character => {
                if forward {
                    (at + 1).min(len)
                } else {
                    at.saturating_sub(1)
                }
            }
            _ if unit == TextUnit_Word => boundary(text, at, forward, is_space),
            _ if unit == TextUnit_Line || unit == TextUnit_Paragraph => {
                boundary(text, at, forward, is_break)
            }
            // Document, Page and Format are all "the whole of it".
            _ if unit == TextUnit_Document || unit == TextUnit_Page || unit == TextUnit_Format => {
                if forward {
                    len
                } else {
                    0
                }
            }
            _ => at,
        };
        if next == at {
            break;
        }
        at = next;
        moved += 1;
    }
    (at, if forward { moved } else { -moved })
}

/// Returns the next position across a boundary `at_boundary` defines, in the direction
/// `forward` names.
fn boundary(text: &[u16], from: u32, forward: bool, at_boundary: fn(u16) -> bool) -> u32 {
    let len = text.len() as u32;
    let mut at = from;
    if forward {
        while at < len && !at_boundary(text[at as usize]) {
            at += 1;
        }
        while at < len && at_boundary(text[at as usize]) {
            at += 1;
        }
    } else {
        at = at.saturating_sub(1);
        while at > 0 && at_boundary(text[at as usize]) {
            at -= 1;
        }
        while at > 0 && !at_boundary(text[(at - 1) as usize]) {
            at -= 1;
        }
    }
    at
}

const fn is_space(unit: u16) -> bool {
    matches!(unit, 0x20 | 0x09 | 0x0a | 0x0d)
}

const fn is_break(unit: u16) -> bool {
    matches!(unit, 0x0a | 0x0d)
}

/// Returns the endpoint `which` names, out of a `(start, end)` pair.
fn endpoint(span: (u32, u32), which: TextPatternRangeEndpoint) -> u32 {
    if which == TextPatternRangeEndpoint_Start {
        span.0
    } else {
        span.1
    }
}

/// Returns `span` with the endpoint `which` names moved to `to`, taking the other endpoint
/// along rather than letting the range invert.
fn with_endpoint(span: (u32, u32), which: TextPatternRangeEndpoint, to: u32) -> (u32, u32) {
    if which == TextPatternRangeEndpoint_Start {
        (to, span.1.max(to))
    } else {
        (span.0.min(to), to)
    }
}

/// Returns the span of a range automation handed back, when it is one of ours over the same
/// element, and `None` otherwise.
///
/// Both conditions carry weight. A foreign implementation or a marshalling proxy does not
/// answer the dynamic-cast protocol, so it resolves to `None` rather than to a
/// reinterpretation of another object's memory; and endpoint arithmetic against a range
/// over a different element has no meaning, so it is refused rather than answered.
fn peer(owner: ControlId, range: Ref<ITextRangeProvider>) -> Option<(u32, u32)> {
    let object = range.ok().ok()?;
    let other = object
        .cast_to_any::<Range>()
        .ok()?
        .downcast_ref::<Range_Impl>()?;
    (other.owner == owner).then(|| other.span())
}

impl crate::bindings::ITextRangeProvider_Impl for Range_Impl {
    fn Clone(&self) -> Result<ITextRangeProvider> {
        let (start, end) = self.span();
        self.object(start, end)
    }

    fn Compare(&self, range: Ref<ITextRangeProvider>) -> Result<BOOL> {
        Ok(BOOL::from(peer(self.owner, range) == Some(self.span())))
    }

    fn CompareEndpoints(
        &self,
        which: TextPatternRangeEndpoint,
        target: Ref<ITextRangeProvider>,
        target_which: TextPatternRangeEndpoint,
    ) -> Result<i32> {
        let other = peer(self.owner, target).ok_or_else(none)?;
        let mine = endpoint(self.span(), which) as i64;
        Ok((mine - endpoint(other, target_which) as i64).signum() as i32)
    }

    fn ExpandToEnclosingUnit(&self, unit: TextUnit) -> Result<()> {
        let (tree, text, start, _) = self.body()?;
        if unit == TextUnit_Character {
            if let Some(field) = tree.field(self.owner) {
                let g = field
                    .geometry
                    .as_ref()
                    .filter(|g| g.revision == field.revision)
                    .ok_or_else(none)?;
                let at = start.min(text.len().saturating_sub(1) as u32);
                let from = g
                    .clusters
                    .iter()
                    .find(|c| c.start <= at && at < c.end)
                    .map_or(0, |c| c.start);
                self.set(from, g.next(from));
                return Ok(());
            }
        }
        let (from, _) = self.walk(&tree, &text, start, unit, -1)?;
        let (to, _) = self.walk(&tree, &text, from, unit, 1)?;
        self.set(from, to);
        Ok(())
    }

    fn FindAttribute(
        &self,
        _: TEXTATTRIBUTEID,
        _: &VARIANT,
        _: BOOL,
    ) -> Result<ITextRangeProvider> {
        // The text carries one paint, so no attribute varies across it and no sub-range
        // holds a different one. `S_OK` with a null range is the documented "not found".
        Err(none())
    }

    fn FindText(
        &self,
        needle: &BSTR,
        backward: BOOL,
        ignore_case: BOOL,
    ) -> Result<ITextRangeProvider> {
        let (_, text, start, end) = self.body()?;
        let haystack = &text[start as usize..end as usize];
        let needle: Vec<u16> = if ignore_case.as_bool() {
            fold(&wide_of(needle))
        } else {
            wide_of(needle)
        };
        if needle.is_empty() || needle.len() > haystack.len() {
            return Err(none());
        }
        let folded: Vec<u16> = if ignore_case.as_bool() {
            fold(haystack)
        } else {
            haystack.to_vec()
        };
        let found = if backward.as_bool() {
            folded.windows(needle.len()).rposition(|at| at == needle)
        } else {
            folded.windows(needle.len()).position(|at| at == needle)
        };
        let at = start + found.ok_or_else(none)? as u32;
        self.object(at, at + needle.len() as u32)
    }

    fn GetAttributeValue(&self, _: TEXTATTRIBUTEID) -> Result<VARIANT> {
        // Uniform across the whole range, and this stack publishes none of the attributes
        // automation asks about, so every one of them is "not supported here".
        Ok(super::variant::empty())
    }

    fn GetBoundingRectangles(&self) -> Result<*mut SAFEARRAY> {
        // The element's own box: a single-run surface is one line, one paint, one
        // rectangle. A sub-line rectangle would need cluster geometry from the text
        // engine, which lives on the front thread.
        let shared = self.shared.upgrade().ok_or_else(gone)?;
        let tree = super::element::tree_of(&shared);
        let at = tree.index_of(self.owner).ok_or_else(gone)?;
        let entry = tree.entry(at).ok_or_else(gone)?;
        let (origin, scale) = tree.live.window();
        let offset = tree.live.scroll(entry.scroll_src);
        if let Some(field) = tree.field(self.owner) {
            let g = field
                .geometry
                .as_ref()
                .filter(|g| g.revision == field.revision)
                .ok_or_else(none)?;
            let (start, end) = self.span();
            let mut rects = Vec::new();
            g.rects(start..end, &mut rects);
            if start == end {
                rects.push(g.caret(crate::text_input::Selection {
                    caret: start,
                    ..Default::default()
                }));
            }
            let mut values = Vec::with_capacity(rects.len() * 4);
            for r in rects {
                let left = (r.x + g.origin.x).max(g.viewport.x);
                let right = (r.x + r.w + g.origin.x).min(g.viewport.x + g.viewport.w);
                if right < left {
                    continue;
                }
                values.extend_from_slice(&[
                    f64::from(origin.x + (entry.x0 - offset.x + left) * scale),
                    f64::from(origin.y + (entry.y0 - offset.y + r.y + g.origin.y) * scale),
                    f64::from((right - left) * scale),
                    f64::from(r.h * scale),
                ]);
            }
            return Ok(super::variant::rect_array(&values));
        }
        Ok(super::variant::rect_array(&[
            f64::from(origin.x + (entry.x0 - offset.x) * scale),
            f64::from(origin.y + (entry.y0 - offset.y) * scale),
            f64::from((entry.x1 - entry.x0) * scale),
            f64::from((entry.y1 - entry.y0) * scale),
        ]))
    }

    fn GetEnclosingElement(&self) -> Result<IRawElementProviderSimple> {
        let shared = self.shared.upgrade().ok_or_else(gone)?;
        super::element::provider_for(&shared, self.owner).ok_or_else(gone)
    }

    fn GetText(&self, max: i32) -> Result<BSTR> {
        let (_, text, start, end) = self.body()?;
        let mut span = &text[start as usize..end as usize];
        if max >= 0 && (max as usize) < span.len() {
            span = &span[..max as usize];
        }
        Ok(super::variant::bstr(span))
    }

    fn Move(&self, unit: TextUnit, count: i32) -> Result<i32> {
        let (tree, text, start, _) = self.body()?;
        let (from, moved) = self.walk(&tree, &text, start, unit, count)?;
        // `Move` leaves the range degenerate and then expands it to the unit, which is the
        // documented behaviour and what lets a client page through a word at a time.
        let (to, _) = self.walk(&tree, &text, from, unit, 1)?;
        self.set(from, to);
        Ok(moved)
    }

    fn MoveEndpointByUnit(
        &self,
        which: TextPatternRangeEndpoint,
        unit: TextUnit,
        count: i32,
    ) -> Result<i32> {
        let (tree, text, ..) = self.body()?;
        let span = self.span();
        let (to, moved) = self.walk(&tree, &text, endpoint(span, which), unit, count)?;
        let (start, end) = with_endpoint(span, which, to);
        self.set(start, end);
        Ok(moved)
    }

    fn MoveEndpointByRange(
        &self,
        which: TextPatternRangeEndpoint,
        target: Ref<ITextRangeProvider>,
        target_which: TextPatternRangeEndpoint,
    ) -> Result<()> {
        let other = peer(self.owner, target).ok_or_else(none)?;
        let (start, end) = with_endpoint(self.span(), which, endpoint(other, target_which));
        self.set(start, end);
        Ok(())
    }

    fn Select(&self) -> Result<()> {
        let shared = self.shared.upgrade().ok_or_else(gone)?;
        let tree = super::element::tree_of(&shared);
        let field = tree
            .field(self.owner)
            .filter(|f| !f.password)
            .ok_or_else(none)?;
        let (anchor, caret) = self.span();
        if caret as usize > field.text.len() {
            return Err(none());
        }
        shared.edit(super::action::TextAction::Select(
            self.owner,
            field.revision,
            crate::text_input::Selection {
                anchor,
                caret,
                affinity: Default::default(),
            },
        ));
        Ok(())
    }

    fn AddToSelection(&self) -> Result<()> {
        Err(none())
    }

    fn RemoveFromSelection(&self) -> Result<()> {
        Err(none())
    }

    fn ScrollIntoView(&self, _: BOOL) -> Result<()> {
        let shared = self.shared.upgrade().ok_or_else(gone)?;
        shared.act(super::action::Action::Reveal(self.owner));
        Ok(())
    }

    fn GetChildren(&self) -> Result<*mut SAFEARRAY> {
        // Flat text: no embedded objects, so an empty array rather than a null one, which
        // is what a client iterating without a length check expects.
        Ok(super::variant::provider_array(&[]))
    }
}

/// Returns a `BSTR`'s UTF-16 units, which `FindText` indexes directly.
fn wide_of(text: &BSTR) -> Vec<u16> {
    String::try_from(text)
        .unwrap_or_default()
        .encode_utf16()
        .collect()
}

/// Folds ASCII and Latin-1 upper-case letters to lower case, which covers the alphabet a
/// search over user-visible labels meets. Full Unicode folding needs a table this crate has
/// no other use for.
fn fold(text: &[u16]) -> Vec<u16> {
    text.iter()
        .map(|&unit| match unit {
            0x0041..=0x005a | 0x00c0..=0x00d6 | 0x00d8..=0x00de => unit + 32,
            _ => unit,
        })
        .collect()
}

// ── the element half ────────────────────────────────────────────────────────────

impl crate::bindings::ITextProvider_Impl for Element_Impl {
    fn GetSelection(&self) -> Result<*mut SAFEARRAY> {
        let at = self.at()?;
        if let Some(field) = at.tree.field(self.id()).filter(|f| !f.password) {
            let range = field.selection.range();
            return Ok(super::variant::range_array(&[Range::new(
                &at.shared,
                self.id(),
                range.start,
                range.end,
            )
            .into()]));
        }
        Ok(super::variant::range_array(&[]))
    }

    fn GetVisibleRanges(&self) -> Result<*mut SAFEARRAY> {
        let range = document(self)?;
        Ok(super::variant::range_array(&[range]))
    }

    fn RangeFromChild(&self, _: Ref<IRawElementProviderSimple>) -> Result<ITextRangeProvider> {
        // Flat text has no children, so no child can name a range in it.
        Err(none())
    }

    fn RangeFromPoint(&self, point: &UiaPoint) -> Result<ITextRangeProvider> {
        let at = self.at()?;
        if let Some(field) = at.tree.field(self.id()) {
            let g = field
                .geometry
                .as_ref()
                .filter(|g| g.revision == field.revision)
                .ok_or_else(none)?;
            let entry = at.tree.entry(at.at).ok_or_else(gone)?;
            let (origin, scale) = at.tree.live.window();
            let offset = at.tree.live.scroll(entry.scroll_src);
            let (index, _) =
                g.hit((point.x as f32 - origin.x) / scale - entry.x0 + offset.x - g.origin.x);
            return Ok(Range::new(&at.shared, self.id(), index, index).into());
        }
        // Point-to-offset needs cluster geometry, which belongs to the text engine on the
        // front thread. A degenerate range at the start is the documented fallback, and is
        // what a client anchors a walk on.
        let (shared, owner) = owner(self)?;
        Ok(Range::new(&shared, owner, 0, 0).into())
    }

    fn DocumentRange(&self) -> Result<ITextRangeProvider> {
        document(self)
    }

    fn SupportedTextSelection(&self) -> Result<SupportedTextSelection> {
        Ok(SupportedTextSelection_Single)
    }
}

/// Returns the control a text call is about, and the shared state behind it.
///
/// Only an element whose column carries [`Value::Text`] answers; every other element
/// refuses. The fragment root does not implement the pattern at all, because it is the
/// window and carries no text.
fn owner(this: &Element_Impl) -> Result<(Arc<Shared>, ControlId)> {
    let At {
        shared, tree, at, ..
    } = this.at()?;
    match tree.col(at).map(|col| col.value) {
        Some(Value::Text | Value::EditableText) => Ok((shared, this.id())),
        _ => Err(none()),
    }
}

/// Returns a range spanning the element's whole text.
fn document(this: &Element_Impl) -> Result<ITextRangeProvider> {
    let (shared, id) = owner(this)?;
    let tree = super::element::tree_of(&shared);
    let at = tree.index_of(id).ok_or_else(gone)?;
    let len = tree.field(id).map_or_else(
        || tree.col(at).map_or(0, |col| col.name.len),
        |f| f.text.len() as u32,
    );
    Ok(Range::new(&shared, id, 0, len).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::TextPatternRangeEndpoint_End;

    fn utf16(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn a_word_walk_lands_on_word_starts_in_both_directions() {
        let text = utf16("gain  makeup trim");
        assert_eq!(walk(&text, 0, TextUnit_Word, 1), (6, 1));
        assert_eq!(walk(&text, 6, TextUnit_Word, 1), (13, 1));
        assert_eq!(walk(&text, 13, TextUnit_Word, -1), (6, -1));
        assert_eq!(walk(&text, 6, TextUnit_Word, -1), (0, -1));
    }

    #[test]
    fn a_walk_stops_at_the_ends_and_reports_how_far_it_got() {
        let text = utf16("gain");
        assert_eq!(
            walk(&text, 0, TextUnit_Word, 9),
            (4, 1),
            "one move, then stuck"
        );
        assert_eq!(walk(&text, 0, TextUnit_Character, -3), (0, 0));
        assert_eq!(walk(&text, 0, TextUnit_Document, 1), (4, 1));
    }

    #[test]
    fn an_endpoint_move_cannot_invert_a_range() {
        assert_eq!(
            with_endpoint((4, 8), TextPatternRangeEndpoint_Start, 9),
            (9, 9),
            "dragging the start past the end takes the end with it"
        );
        assert_eq!(
            with_endpoint((4, 8), TextPatternRangeEndpoint_End, 2),
            (2, 2)
        );
    }

    #[test]
    fn folding_is_case_insensitive_over_the_range_it_claims() {
        assert_eq!(fold(&utf16("Gain ÀB")), utf16("gain àb"));
    }
    #[test]
    fn editable_character_walk_uses_published_clusters_and_rejects_stale_layout() {
        use crate::text_input::{Cluster, Geometry};
        let id = ControlId::default();
        let units: Arc<[u16]> = utf16("á😀").into();
        let clusters: Arc<[Cluster]> = [(0, 2), (2, 4)]
            .into_iter()
            .map(|(start, end)| Cluster {
                start,
                end,
                rect: windows_text::Rect {
                    x: start as f32,
                    y: 0.0,
                    w: 2.0,
                    h: 12.0,
                },
                leading: start as f32,
                trailing: end as f32,
            })
            .collect::<Vec<_>>()
            .into();
        let mut seeds = super::super::tree::Seeds::default();
        seeds.fields.push(super::super::tree::FieldText {
            id,
            revision: 1,
            text: units.clone(),
            selection: Default::default(),
            password: false,
            geometry: Some(Arc::new(Geometry {
                revision: 1,
                clusters,
                ..Default::default()
            })),
        });
        let tree = Tree::build(&[], &seeds);
        let range = Range {
            shared: Weak::new(),
            owner: id,
            start: AtomicU32::new(0),
            end: AtomicU32::new(4),
        };
        assert_eq!(
            range.walk(&tree, &units, 0, TextUnit_Character, 1).unwrap(),
            (2, 1)
        );
        assert_eq!(
            range.walk(&tree, &units, 2, TextUnit_Character, 1).unwrap(),
            (4, 1)
        );
        assert_eq!(
            range
                .walk(&tree, &units, 4, TextUnit_Character, i32::MIN)
                .unwrap(),
            (0, -2)
        );
        seeds.fields[0].revision = 2;
        let stale = Tree::build(&[], &seeds);
        assert!(
            range
                .walk(&stale, &units, 0, TextUnit_Character, 1)
                .is_err()
        );
        assert_eq!(walk(&units, 0, TextUnit_Character, i32::MIN), (0, 0));
    }
}
