//! Exposes `TextPattern2` over published text, selection and cluster geometry.
//!
//! A surface whose text a user reads, selects and copies but never edits publishes this
//! pattern rather than an edit field: an edit field hands ownership to TSF and raises a touch
//! keyboard over a surface with nothing to type into. Nothing here creates a text-services
//! object.
//!
//! The pool is UTF-16 and automation's text offsets are UTF-16 offsets, so a range is two
//! integers into a slice the snapshot already holds. Storing the strings as `str` would cost
//! an offset table per element to say the same thing.

use super::action::{Action, TextAction};
use super::provider::{At, Element, Element_Impl, Shared, gone, none, tree_of, vtable};
use super::snapshot::{ColFlags, Tree};
use super::variant;
use crate::bindings::*;
use crate::text_input::{Geometry, Selection};
use core::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Weak};
use windows_core::{BOOL, BSTR, Interface, Ref, Result, implement_decl};
use windows_scene::ControlId;

/// One range over one element's text.
///
/// The endpoints are atomics because automation mutates a range in place — `Move`,
/// `ExpandToEnclosingUnit` and `MoveEndpointByUnit` each edit the object they are called on —
/// and the object is callable from any thread, like every provider here.
pub struct Range {
    shared: Weak<Shared>,
    owner: ControlId,
    start: AtomicU32,
    end: AtomicU32,
}

implement_decl! {
    impl Range as pub Range_Impl: [ITextRangeProvider]
}

/// What a text call resolves to: the body, the endpoints clamped into it, and the clusters the
/// shaper published for it.
struct Body {
    shared: Arc<Shared>,
    tree: Arc<Tree>,
    text: Arc<[u16]>,
    /// Whether the body is an editable document, whose characters the shaper's clusters
    /// define. A stale or absent geometry over one is a refusal rather than a code-unit walk.
    editable: bool,
    clusters: Option<Arc<Geometry>>,
    span: (u32, u32),
}

impl Range {
    fn new(shared: &Arc<Shared>, owner: ControlId, span: (u32, u32)) -> Self {
        Self {
            shared: Arc::downgrade(shared),
            owner,
            start: AtomicU32::new(span.0),
            end: AtomicU32::new(span.1),
        }
    }

    fn span(&self) -> (u32, u32) {
        (self.start.load(Relaxed), self.end.load(Relaxed))
    }

    /// Stores both endpoints, holding `end` at or after `start` so a range cannot invert.
    fn set(&self, span: (u32, u32)) {
        self.start.store(span.0, Relaxed);
        self.end.store(span.1.max(span.0), Relaxed);
    }

    /// Returns the element's whole text, with this range's endpoints clamped into it.
    ///
    /// The clamp runs on every read rather than on every write, because a republish can replace
    /// the text while a client still holds a range over the old one. A stale endpoint then
    /// reads as a short range instead of panicking or slicing another element's string.
    fn body(&self) -> Result<Body> {
        let shared = self.shared.upgrade().ok_or_else(gone)?;
        let tree = tree_of(&shared);
        let mut body = body_of(&shared, &tree, self.owner)?;
        let len = body.text.len() as u32;
        let start = self.start.load(Relaxed).min(len);
        body.span = (start, self.end.load(Relaxed).clamp(start, len));
        Ok(body)
    }

    /// Returns a new range over the same element.
    fn over(&self, span: (u32, u32)) -> Result<ITextRangeProvider> {
        let shared = self.shared.upgrade().ok_or_else(gone)?;
        Ok(Self::new(&shared, self.owner, span).into())
    }

    /// Returns the offset `count` units from `from`, and how many were crossed.
    ///
    /// A character of an editable document is a grapheme cluster, so it steps through the
    /// shaper's clusters: stepping by code unit would land on half a surrogate pair or on a
    /// combining mark alone, which is not a character a user can see. A document whose
    /// geometry is absent or describes an earlier revision has no characters to step through
    /// and refuses. Every other unit is a code-unit boundary search.
    ///
    /// # Errors
    ///
    /// The empty error where a character walk over an editable document has no current
    /// cluster geometry.
    fn walk(&self, body: &Body, from: u32, unit: TextUnit, count: i32) -> Result<(u32, i32)> {
        if unit != TextUnit_Character || !body.editable {
            return Ok(walk(&body.text, from, unit, count));
        }
        let clusters = body.clusters.as_ref().ok_or_else(none)?;
        let (mut at, mut moved) = (from.min(body.text.len() as u32), 0);
        for _ in 0..count.unsigned_abs() {
            let next = if count > 0 {
                clusters.next(at)
            } else {
                clusters.previous(at)
            };
            if next == at {
                break;
            }
            at = next;
            moved += if count > 0 { 1 } else { -1 };
        }
        Ok((at, moved))
    }
}

/// Returns the body `id` publishes, or the empty error where it publishes none.
///
/// Only an element whose entry carries [`ColFlags::BODY`] answers; every other element
/// refuses. The fragment root does not implement the pattern at all, because it is the window
/// and carries no text.
fn body_of(shared: &Arc<Shared>, tree: &Arc<Tree>, id: ControlId) -> Result<Body> {
    let at = tree.index_of(id).ok_or_else(gone)?;
    let entry = *tree.at(at).ok_or_else(gone)?;
    if !entry.flags.has(ColFlags::BODY) {
        return Err(none());
    }
    let field = tree.field(id);
    Ok(Body {
        shared: Arc::clone(shared),
        tree: Arc::clone(tree),
        text: field.map_or_else(|| Arc::from(tree.text(entry.name)), |f| Arc::clone(&f.text)),
        editable: field.is_some(),
        clusters: field
            .and_then(|f| {
                f.geometry
                    .as_ref()
                    .filter(|g| g.revision == f.revision)
                    .cloned()
            })
            .or_else(|| tree.text_geometry(at).cloned()),
        span: (0, 0),
    })
}

/// Returns the offset `count` unit boundaries from `from`, and how many were crossed.
///
/// A negative `count` walks backward and the crossing count comes back negative to match. The
/// walk stops at either end of the text, so the count is what tells a caller a partial move
/// from a complete one.
///
/// Word boundaries are whitespace transitions and line boundaries are newlines. `Document`,
/// `Page` and `Format` each span the whole text: the text carries one paint, so a format
/// boundary is the document boundary.
fn walk(text: &[u16], from: u32, unit: TextUnit, count: i32) -> (u32, i32) {
    let len = text.len() as u32;
    let forward = count > 0;
    let (mut at, mut moved) = (from.min(len), 0);
    for _ in 0..count.unsigned_abs() {
        let next = match unit {
            TextUnit_Character if forward => (at + 1).min(len),
            TextUnit_Character => at.saturating_sub(1),
            TextUnit_Word => boundary(text, at, forward, is_space),
            TextUnit_Line | TextUnit_Paragraph => boundary(text, at, forward, is_break),
            // Document, Page and Format are all "the whole of it".
            _ if forward => len,
            _ => 0,
        };
        if next == at {
            break;
        }
        at = next;
        moved += if forward { 1 } else { -1 };
    }
    (at, moved)
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
        return at;
    }
    at = at.saturating_sub(1);
    while at > 0 && at_boundary(text[at as usize]) {
        at -= 1;
    }
    while at > 0 && !at_boundary(text[(at - 1) as usize]) {
        at -= 1;
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
const fn endpoint(span: (u32, u32), which: TextPatternRangeEndpoint) -> u32 {
    if which == TextPatternRangeEndpoint_Start {
        span.0
    } else {
        span.1
    }
}

/// Returns `span` with the endpoint `which` names moved to `to`, taking the other endpoint
/// along rather than letting the range invert.
const fn with_endpoint(span: (u32, u32), which: TextPatternRangeEndpoint, to: u32) -> (u32, u32) {
    if which == TextPatternRangeEndpoint_Start {
        (to, if span.1 > to { span.1 } else { to })
    } else {
        (if span.0 < to { span.0 } else { to }, to)
    }
}

/// Returns the span of a range automation handed back, when it is one of ours over the same
/// element, and `None` otherwise.
///
/// Both conditions carry weight. A foreign implementation or a marshalling proxy does not
/// answer the dynamic-cast protocol, so it resolves to `None` rather than to a reinterpretation
/// of another object's memory; and endpoint arithmetic against a range over a different element
/// has no meaning, so it is refused rather than answered.
fn peer(owner: ControlId, range: Ref<ITextRangeProvider>) -> Option<(u32, u32)> {
    let object = range.ok().ok()?;
    let other = object
        .cast_to_any::<Range>()
        .ok()?
        .downcast_ref::<Range_Impl>()?;
    (other.owner == owner).then(|| other.span())
}

/// Folds ASCII and Latin-1 upper-case letters to lower case, which covers the alphabet a search
/// over user-visible labels meets. Full Unicode folding needs a table this crate has no other
/// use for.
fn fold(text: &[u16], on: bool) -> Vec<u16> {
    text.iter()
        .map(|&unit| match unit {
            0x0041..=0x005a | 0x00c0..=0x00d6 | 0x00d8..=0x00de if on => unit + 32,
            _ => unit,
        })
        .collect()
}

// ── the element half ────────────────────────────────────────────────────────────

vtable! {
    ITextProvider_Impl for Element_Impl {
        fn DocumentRange(&self) -> Result<ITextRangeProvider> { document_range(&self.this) }
        fn SupportedTextSelection(&self) -> Result<SupportedTextSelection> {
            let at = self.this.at()?;
            Ok(if at.tree.field(self.this.id).is_some_and(|field| !field.password) {
                SupportedTextSelection_Single
            } else { SupportedTextSelection_None })
        }
        fn GetSelection(&self) -> Result<*mut SAFEARRAY> { selection_of(&self.this) }
        fn GetVisibleRanges(&self) -> Result<*mut SAFEARRAY> { visible_ranges(&self.this) }
        // Flat text has no children, so no child can name a range in it.
        fn RangeFromChild(&self, _: Ref<IRawElementProviderSimple>) -> Result<ITextRangeProvider> { Err(none()) }
        fn RangeFromPoint(&self, point: &UiaPoint) -> Result<ITextRangeProvider> { range_from_point(&self.this, point) }
    }
    ITextProvider2_Impl for Element_Impl {
        // No element in this text model owns annotations.
        fn RangeFromAnnotation(&self, _: Ref<IRawElementProviderSimple>) -> Result<ITextRangeProvider> {
            self.this.at()?;
            Err(none())
        }
        fn GetCaretRange(&self, active: *mut BOOL) -> Result<ITextRangeProvider> {
            if active.is_null() {
                return Err(windows_core::Error::from_hresult(windows_core::HRESULT(0x80004003u32 as i32)));
            }
            // SAFETY: COM supplies writable storage for the BOOL out-parameter; null is rejected above.
            unsafe { *active = BOOL::from(false); }
            let at = self.this.at()?;
            let field = at.tree.field(self.this.id).filter(|field| !field.password).ok_or_else(none)?;
            let caret = field.selection.caret.min(field.text.len() as u32);
            // SAFETY: `active` is the same checked COM out-parameter.
            unsafe { *active = BOOL::from(at.focused()); }
            Ok(Range::new(&at.shared, self.this.id, (caret, caret)).into())
        }
    }
}

/// Returns a range spanning the element's whole text.
fn document_range(element: &Element) -> Result<ITextRangeProvider> {
    let at = element.at()?;
    let body = body_of(&at.shared, &at.tree, element.id)?;
    Ok(Range::new(&at.shared, element.id, (0, body.text.len() as u32)).into())
}

/// Returns contiguous logical ranges whose clusters intersect the field and ancestor clips.
fn visible_ranges(element: &Element) -> Result<*mut SAFEARRAY> {
    let at = element.at()?;
    let body = body_of(&at.shared, &at.tree, element.id)?;
    let own = at.unclipped_box();
    let mut out = Vec::new();
    if let Some(geometry) = body.clusters.as_ref() {
        let view = geometry.viewport;
        let clip = at.clipped(if body.editable {
            [
                own[0] + view.x,
                own[1] + view.y,
                own[0] + view.x + view.w,
                own[1] + view.y + view.h,
            ]
        } else {
            own
        });
        let mut span: Option<(u32, u32)> = None;
        for cluster in geometry.clusters.iter() {
            let x = own[0] + geometry.origin.x + cluster.rect.x;
            let y = own[1] + geometry.origin.y + cluster.rect.y;
            let visible = clip[2] > clip[0]
                && clip[3] > clip[1]
                && x < clip[2]
                && x + cluster.rect.w > clip[0]
                && y < clip[3]
                && y + cluster.rect.h > clip[1];
            if visible {
                match span {
                    Some((start, end)) if end >= cluster.start => {
                        span = Some((start, end.max(cluster.end)))
                    }
                    held => {
                        if let Some(held) = held {
                            out.push(Range::new(&at.shared, element.id, held).into());
                        }
                        span = Some((cluster.start, cluster.end));
                    }
                }
            } else if let Some(held) = span.take() {
                out.push(Range::new(&at.shared, element.id, held).into());
            }
        }
        if let Some(span) = span {
            out.push(Range::new(&at.shared, element.id, span).into());
        }
    } else if body.editable {
        return Err(gone());
    } else {
        let rect = at.rect();
        if rect.width > 0.0 && rect.height > 0.0 {
            out.push(Range::new(&at.shared, element.id, (0, body.text.len() as u32)).into());
        }
    }
    if out.is_empty() {
        out.push(Range::new(&at.shared, element.id, (0, 0)).into());
    }
    Ok(variant::range_array(&out))
}

/// Returns the element's selection, which a password field publishes no part of.
fn selection_of(element: &Element) -> Result<*mut SAFEARRAY> {
    let at = element.at()?;
    let Some(field) = at.tree.field(element.id).filter(|field| !field.password) else {
        return Ok(variant::range_array(&[]));
    };
    let span = field.selection.range();
    let one: ITextRangeProvider = Range::new(&at.shared, element.id, (span.start, span.end)).into();
    Ok(variant::range_array(&[one]))
}

/// Returns a degenerate range at the offset under `point`.
///
/// Uses the published cluster geometry. Static text without geometry returns its start;
/// an editable document requires geometry matching its text revision.
fn range_from_point(element: &Element, point: &UiaPoint) -> Result<ITextRangeProvider> {
    let at = element.at()?;
    let body = body_of(&at.shared, &at.tree, element.id)?;
    let Some(clusters) = body.clusters.as_ref() else {
        if body.editable {
            return Err(none());
        }
        return Ok(Range::new(&at.shared, element.id, (0, 0)).into());
    };
    let (origin, scale) = at.tree.window();
    let entry = at.tree.at(at.at).copied().ok_or_else(gone)?;
    let by = at.tree.scroll(at.at);
    let local = (point.x as f32 - origin.x) / scale - entry.box_[0] + by.x - clusters.origin.x;
    let (found, _) = clusters.hit(local);
    Ok(Range::new(&at.shared, element.id, (found, found)).into())
}

// ── the range half ──────────────────────────────────────────────────────────────

vtable! {
    ITextRangeProvider_Impl for Range_Impl {
        fn Clone(&self) -> Result<ITextRangeProvider> { self.over(self.span()) }
        fn Compare(&self, range: Ref<ITextRangeProvider>) -> Result<BOOL> { Ok(BOOL::from(peer(self.owner, range) == Some(self.span()))) }
        fn CompareEndpoints(&self, which: TextPatternRangeEndpoint, target: Ref<ITextRangeProvider>, target_which: TextPatternRangeEndpoint) -> Result<i32> { compare_endpoints(&self.this, which, target, target_which) }
        fn ExpandToEnclosingUnit(&self, unit: TextUnit) -> Result<()> { expand(&self.this, unit) }
        // The text carries one paint, so no attribute varies across it and no sub-range holds
        // a different one. `S_OK` with a null range is the documented "not found".
        fn FindAttribute(&self, _: TEXTATTRIBUTEID, _: &VARIANT, _: BOOL) -> Result<ITextRangeProvider> { Err(none()) }
        fn FindText(&self, text: &BSTR, backward: BOOL, ignore: BOOL) -> Result<ITextRangeProvider> { find_text(&self.this, text, backward, ignore) }
        fn GetAttributeValue(&self, _: TEXTATTRIBUTEID) -> Result<VARIANT> { Ok(variant::empty()) }
        fn GetBoundingRectangles(&self) -> Result<*mut SAFEARRAY> { rectangles(&self.this) }
        fn GetEnclosingElement(&self) -> Result<IRawElementProviderSimple> { enclosing(&self.this) }
        fn GetText(&self, max: i32) -> Result<BSTR> { text_of(&self.this, max) }
        fn Move(&self, unit: TextUnit, count: i32) -> Result<i32> { move_by(&self.this, unit, count) }
        fn MoveEndpointByUnit(&self, which: TextPatternRangeEndpoint, unit: TextUnit, count: i32) -> Result<i32> { move_endpoint(&self.this, which, unit, count) }
        fn MoveEndpointByRange(&self, which: TextPatternRangeEndpoint, target: Ref<ITextRangeProvider>, target_which: TextPatternRangeEndpoint) -> Result<()> { move_to_range(&self.this, which, target, target_which) }
        fn Select(&self) -> Result<()> { select(&self.this) }
        // There is one selection per document, so there is nothing to add to and nothing to
        // take away.
        fn AddToSelection(&self) -> Result<()> { Err(none()) }
        fn RemoveFromSelection(&self) -> Result<()> { Err(none()) }
        fn ScrollIntoView(&self, align_top: BOOL) -> Result<()> { reveal(&self.this, align_top.as_bool()) }
        // Flat text has no embedded objects, so an empty array rather than a null one, which
        // is what a client iterating without a length check expects.
        fn GetChildren(&self) -> Result<*mut SAFEARRAY> { Ok(variant::provider_array(&[])) }
    }
}

/// Returns where this range's endpoint sits relative to another range's.
fn compare_endpoints(
    range: &Range,
    which: TextPatternRangeEndpoint,
    target: Ref<ITextRangeProvider>,
    target_which: TextPatternRangeEndpoint,
) -> Result<i32> {
    let other = peer(range.owner, target).ok_or_else(none)?;
    let mine = i64::from(endpoint(range.span(), which));
    Ok((mine - i64::from(endpoint(other, target_which))).signum() as i32)
}

/// Grows the range to the whole of the unit its start sits in.
fn expand(range: &Range, unit: TextUnit) -> Result<()> {
    let body = range.body()?;
    if unit == TextUnit_Character
        && let Some(clusters) = body.clusters.as_ref()
    {
        let at = body.span.0.min(body.text.len().saturating_sub(1) as u32);
        let from = clusters
            .clusters
            .iter()
            .find(|cluster| cluster.start <= at && at < cluster.end)
            .map_or(0, |cluster| cluster.start);
        range.set((from, clusters.next(from)));
        return Ok(());
    }
    let from = range.walk(&body, body.span.0, unit, -1)?.0;
    let to = range.walk(&body, from, unit, 1)?.0;
    range.set((from, to));
    Ok(())
}

/// Returns a range over the first occurrence of `text` inside this one.
fn find_text(
    range: &Range,
    text: &BSTR,
    backward: BOOL,
    ignore: BOOL,
) -> Result<ITextRangeProvider> {
    let body = range.body()?;
    // A `BSTR` derefs to its own UTF-16 units, so the search string is compared as it arrived:
    // a round trip through `String` drops an unpaired surrogate a client can legitimately send.
    let needle = fold(text, ignore.as_bool());
    let hay = fold(
        &body.text[body.span.0 as usize..body.span.1 as usize],
        ignore.as_bool(),
    );
    if needle.is_empty() || needle.len() > hay.len() {
        return Err(none());
    }
    let mut hits = hay.windows(needle.len());
    let found = if backward.as_bool() {
        hits.rposition(|at| at == needle)
    } else {
        hits.position(|at| at == needle)
    };
    let at = body.span.0 + found.ok_or_else(none)? as u32;
    range.over((at, at + needle.len() as u32))
}

/// Returns the rectangles the range covers, in screen pixels.
///
/// A static run is one line and one paint, so it is its own element's box. A document reports
/// the clusters the shaper published, cut to the field's own reveal viewport: a scrolled-out
/// run is behind the edit's edge, and a client drawing an unclipped rect would highlight the
/// chrome.
///
/// Both are then cut by every clipping ancestor, through the same rule the element's own
/// bounding rectangle is cut by, so a highlight and the box it sits in agree. A run a list has
/// carried past its edge contributes no rectangle rather than one over whatever the list sits
/// under.
fn rectangles(range: &Range) -> Result<*mut SAFEARRAY> {
    let body = range.body()?;
    let at = body.tree.index_of(range.owner).ok_or_else(gone)?;
    let element = At::of(&body.shared, &body.tree, at);
    let own = element.unclipped_box();
    // Every box below is stated against the element's own uncut corner, so one closure carries
    // it into the tree's space, cuts it and converts it once.
    let emit = |x0: f32, y0: f32, x1: f32, y1: f32, out: &mut Vec<f64>| {
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let box_ = element.clipped([own[0] + x0, own[1] + y0, own[0] + x1, own[1] + y1]);
        if box_[2] <= box_[0] || box_[3] <= box_[1] {
            return;
        }
        let r = element.screen(box_);
        out.extend_from_slice(&[r.left, r.top, r.width, r.height]);
    };
    let mut out = Vec::new();
    if body.clusters.is_none() && !body.editable {
        emit(0.0, 0.0, own[2] - own[0], own[3] - own[1], &mut out);
        return Ok(variant::rect_array(&out));
    }
    let clusters = body.clusters.as_ref().ok_or_else(none)?;
    let mut boxes = Vec::new();
    clusters.rects(body.span.0..body.span.1, &mut boxes);
    if body.span.0 == body.span.1 {
        boxes.push(clusters.caret(Selection::at(body.span.0)));
    }
    out.reserve(boxes.len() * 4);
    for box_ in boxes {
        let view = if body.editable {
            (
                clusters.viewport.x,
                clusters.viewport.x + clusters.viewport.w,
            )
        } else {
            (0.0, own[2] - own[0])
        };
        let left = (box_.x + clusters.origin.x).max(view.0);
        let right = (box_.x + box_.w + clusters.origin.x).min(view.1);
        let top = box_.y + clusters.origin.y;
        emit(left, top, right, top + box_.h, &mut out);
    }
    Ok(variant::rect_array(&out))
}

/// Returns the element whose text this range is over.
fn enclosing(range: &Range) -> Result<IRawElementProviderSimple> {
    let shared = range.shared.upgrade().ok_or_else(gone)?;
    super::provider::provider_for(&shared, range.owner).ok_or_else(gone)
}

/// Returns the range's text, truncated to `max` units where `max` is not negative.
fn text_of(range: &Range, max: i32) -> Result<BSTR> {
    let body = range.body()?;
    let mut end = body.span.1 as usize;
    if max >= 0 {
        end = end.min(body.span.0 as usize + max as usize);
    }
    Ok(variant::bstr(&body.text[body.span.0 as usize..end]))
}

/// Moves the range by `count` units and returns how many it crossed.
fn move_by(range: &Range, unit: TextUnit, count: i32) -> Result<i32> {
    let body = range.body()?;
    let (from, moved) = range.walk(&body, body.span.0, unit, count)?;
    // `Move` leaves the range degenerate and then expands it to the unit, which is the
    // documented behaviour and what lets a client page through a word at a time.
    let to = range.walk(&body, from, unit, 1)?.0;
    range.set((from, to));
    Ok(moved)
}

/// Moves one endpoint by `count` units and returns how many it crossed.
fn move_endpoint(
    range: &Range,
    which: TextPatternRangeEndpoint,
    unit: TextUnit,
    count: i32,
) -> Result<i32> {
    let body = range.body()?;
    let (to, moved) = range.walk(&body, endpoint(body.span, which), unit, count)?;
    range.set(with_endpoint(body.span, which, to));
    Ok(moved)
}

/// Moves one endpoint onto an endpoint of another range over the same element.
fn move_to_range(
    range: &Range,
    which: TextPatternRangeEndpoint,
    target: Ref<ITextRangeProvider>,
    target_which: TextPatternRangeEndpoint,
) -> Result<()> {
    let other = peer(range.owner, target).ok_or_else(none)?;
    range.set(with_endpoint(
        range.span(),
        which,
        endpoint(other, target_which),
    ));
    Ok(())
}

/// Queues the range as the document's selection. A password field publishes no caret.
fn select(range: &Range) -> Result<()> {
    let body = range.body()?;
    let field = body
        .tree
        .field(range.owner)
        .filter(|field| !field.password)
        .ok_or_else(none)?;
    let selection = Selection {
        anchor: body.span.0,
        caret: body.span.1,
        ..Selection::default()
    };
    body.shared
        .edit(TextAction::Select(range.owner, field.revision, selection));
    Ok(())
}

/// Queues revealing this range without changing the document's selection.
fn reveal(range: &Range, align_top: bool) -> Result<()> {
    let body = range.body()?;
    if let Some(field) = body.tree.field(range.owner) {
        if !field.password {
            body.shared.act(Action::RevealText(
                range.owner,
                field.revision,
                body.span.0,
                body.span.1,
                align_top,
            ));
        }
    } else {
        body.shared.act(Action::RevealText(
            range.owner,
            0,
            body.span.0,
            body.span.1,
            align_top,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::snapshot::{Entry, FieldText, NONE, Snapshot, State};
    use super::*;
    use crate::bindings::TextPatternRangeEndpoint_End;
    use crate::text_input::Cluster;
    use crate::widget::UiaRole;

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
        assert_eq!(fold(&utf16("Gain ÀB"), true), utf16("gain àb"));
        assert_eq!(fold(&utf16("Gain ÀB"), false), utf16("Gain ÀB"));
    }

    /// A character of an editable document is a cluster, so the walk steps through the
    /// shaper's, and a geometry describing an earlier revision describes different text.
    #[test]
    fn editable_character_walk_uses_published_clusters_and_rejects_stale_layout() {
        let id = ControlId::default();
        let units: Arc<[u16]> = utf16("á😀").into();
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

        let mut snapshot = Snapshot::default();
        snapshot.entries.push(Entry {
            id,
            flags: ColFlags::BODY | ColFlags::FIELD,
            role: UiaRole::Edit,
            parent: NONE,
            clip: NONE,
            scroll: NONE,
            ..Entry::default()
        });
        snapshot.state.push(State::ENABLED);
        snapshot.fields.push(FieldText {
            id,
            revision: 1,
            text: Arc::clone(&units),
            selection: Selection::default(),
            password: false,
            geometry: Some(Arc::new(Geometry {
                revision: 1,
                clusters,
                ..Geometry::default()
            })),
        });

        let current = Arc::new(Tree::adopt(&snapshot, &[]));
        let shared = Arc::new(Shared::default());
        shared.tree.write(|held| *held = Arc::clone(&current));
        let range = Range::new(&shared, id, (0, 4));

        let body = range.body().expect("the field publishes a body");
        assert_eq!(range.walk(&body, 0, TextUnit_Character, 1).unwrap(), (2, 1));
        assert_eq!(range.walk(&body, 2, TextUnit_Character, 1).unwrap(), (4, 1));
        assert_eq!(
            range.walk(&body, 4, TextUnit_Character, i32::MIN).unwrap(),
            (0, -2)
        );

        snapshot.fields[0].revision = 2;
        shared
            .tree
            .write(|held| *held = Arc::new(Tree::adopt(&snapshot, &[])));
        let stale = range.body().expect("the field is still published");
        assert!(
            range.walk(&stale, 0, TextUnit_Character, 1).is_err(),
            "a geometry from an earlier revision describes different text"
        );
        assert_eq!(walk(&units, 0, TextUnit_Character, i32::MIN), (0, 0));
    }
}
