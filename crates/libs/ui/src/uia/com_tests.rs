//! Tests that drive the provider through its COM vtables from a thread that never
//! published, with the publishing thread parked in a join.
//!
//! [`tests`](super::tests) covers the data model. These calls go through the generated
//! implement-side vtables, which is the surface automation itself calls.

use super::NONE;
use super::tests::{Screen, listening};
use crate::bindings::{
    IRawElementProviderFragment, IRawElementProviderFragmentRoot, IRawElementProviderSimple,
    ITextProvider, ITextProvider2, ITextRangeProvider, NavigateDirection_FirstChild,
    NavigateDirection_NextSibling, NavigateDirection_Parent, SAFEARRAY,
    TextPatternRangeEndpoint_End, TextUnit_Character, UIA_ControlTypePropertyId,
    UIA_InvokePatternId, UIA_NamePropertyId, UIA_SliderControlTypeId, UIA_TextPattern2Id,
    UIA_TextPatternId, UIA_TogglePatternId, VARIANT,
};
use crate::widget::{Range, UiaRole};
use windows_core::Interface;
use windows_numerics::Vector2;

windows_core::link!("oleaut32.dll" "system" fn SafeArrayGetUBound(array: *mut SAFEARRAY, dim: u32, out: *mut i32) -> windows_core::HRESULT);
windows_core::link!("oleaut32.dll" "system" fn SafeArrayGetElement(array: *mut SAFEARRAY, at: *const i32, out: *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("oleaut32.dll" "system" fn SafeArrayDestroy(array: *mut SAFEARRAY) -> windows_core::HRESULT);

/// A raw provider pointer, moved to another thread and called from there the way automation
/// calls one: from any apartment, without marshalling.
struct Agile(*mut core::ffi::c_void);

// SAFETY: the referent is a provider with no apartment affinity — every method reads an
// immutable published snapshot — and the `Uia` that owns it stays alive until the thread
// the pointer was moved to has been joined.
unsafe impl Send for Agile {}

impl Agile {
    fn of(provider: &IRawElementProviderSimple) -> Self {
        Self(provider.as_raw())
    }

    /// Takes a counted reference to the provider the pointer names.
    ///
    /// # Safety
    ///
    /// The `Uia` that owns the provider must outlive both this call and the returned
    /// interface.
    unsafe fn simple(&self) -> IRawElementProviderSimple {
        unsafe { IRawElementProviderSimple::from_raw_borrowed(&self.0) }
            .expect("a live provider")
            .clone()
    }
}

// The generated bindings are implement-side and carry no client wrappers, so the helpers
// below call through the vtable directly, which is the path automation takes.

fn navigate(
    element: &IRawElementProviderFragment,
    direction: crate::bindings::NavigateDirection,
) -> Option<IRawElementProviderFragment> {
    let mut out = core::ptr::null_mut();
    // SAFETY: `element` holds a counted reference for the whole call, and `out` points at a
    // local that outlives it. `Navigate` returns `S_OK` with a null out-pointer where there
    // is no element in that direction, so the pointer is checked before it is cloned.
    unsafe {
        (element.vtable().Navigate)(element.as_raw(), direction, &raw mut out)
            .ok()
            .ok()?;
        IRawElementProviderFragment::from_raw_borrowed(&out).cloned()
    }
}

fn property(element: &IRawElementProviderSimple, id: i32) -> VARIANT {
    let mut out = VARIANT::default();
    // SAFETY: `element` holds a counted reference for the whole call, and `out` points at a
    // local `VARIANT` that outlives it. The callee initialises the variant, and whatever it
    // stores there is owned by this thread from the return onward.
    unsafe {
        _ = (element.vtable().GetPropertyValue)(element.as_raw(), id, &raw mut out);
    }
    out
}

fn supports(element: &IRawElementProviderSimple, pattern: i32) -> bool {
    let mut out = core::ptr::null_mut();
    // SAFETY: `element` holds a counted reference for the whole call, and `out` points at a
    // local that outlives it. `GetPatternProvider` stores null for a pattern the element
    // does not support, so a non-null pointer is an owned reference, released here.
    unsafe {
        _ = (element.vtable().GetPatternProvider)(element.as_raw(), pattern, &raw mut out);
        if out.is_null() {
            return false;
        }
        drop(windows_core::IUnknown::from_raw(out));
        true
    }
}

fn from_point(
    root: &IRawElementProviderFragmentRoot,
    x: f64,
    y: f64,
) -> Option<IRawElementProviderFragment> {
    let mut out = core::ptr::null_mut();
    // SAFETY: `root` holds a counted reference for the whole call, and `out` points at a
    // local that outlives it.
    unsafe {
        (root.vtable().ElementProviderFromPoint)(root.as_raw(), x, y, &raw mut out)
            .ok()
            .ok()?;
        IRawElementProviderFragment::from_raw_borrowed(&out).cloned()
    }
}

fn bounds(element: &IRawElementProviderFragment) -> Option<crate::bindings::UiaRect> {
    let mut out = crate::bindings::UiaRect::default();
    // SAFETY: `element` holds a counted reference for the whole call, and `out` points at a
    // local that outlives it.
    unsafe {
        (element.vtable().get_BoundingRectangle)(element.as_raw(), &raw mut out)
            .ok()
            .ok()?;
    }
    Some(out)
}

/// Returns a variant's type tag, which is `VT_EMPTY` where the provider answered nothing.
fn tag(value: &VARIANT) -> u16 {
    // SAFETY: `vt` sits at the same offset in every arm of the union, so it is initialised
    // whichever arm the callee filled.
    unsafe { value.Anonymous.Anonymous.vt }
}

fn text(value: &VARIANT) -> String {
    assert_eq!(tag(value), 8, "expected a BSTR");
    // SAFETY: the tag asserted above is `VT_BSTR`, so `bstrVal` is the arm the callee
    // filled and it names a live string.
    unsafe { String::try_from(&*value.Anonymous.Anonymous.Anonymous.bstrVal).unwrap_or_default() }
}

fn number(value: &VARIANT) -> i32 {
    assert_eq!(tag(value), 3, "expected an I4");
    // SAFETY: the tag asserted above is `VT_I4`, so `lVal` is the arm the callee filled.
    unsafe { value.Anonymous.Anonymous.Anonymous.lVal }
}

#[test]
fn a_provider_answers_from_a_thread_that_never_published() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let group = screen.add(NONE, (0.0, 0.0, 200.0, 80.0), UiaRole::Group, "output");
    screen.add(group, (8.0, 8.0, 80.0, 32.0), UiaRole::Button, "mute");
    screen.slider(group, (96.0, 8.0, 190.0, 32.0), Range::new(-60.0, 0.0));
    screen.publish(&mut uia);

    let root = Agile::of(&uia.root_for_test());
    let walked = std::thread::spawn(move || {
        // SAFETY: the parent thread owns `uia` and joins this one before dropping it.
        let root = unsafe { root.simple() };
        let root_of_fragments: IRawElementProviderFragmentRoot =
            root.cast().expect("the root is a fragment root");
        let window: IRawElementProviderFragment = root.cast().expect("the root is a fragment");
        let group =
            navigate(&window, NavigateDirection_FirstChild).expect("the window has a child");

        let mut names = Vec::new();
        let mut types = Vec::new();
        let mut child = navigate(&group, NavigateDirection_FirstChild);
        while let Some(element) = child {
            let simple: IRawElementProviderSimple = element.cast().expect("every element is one");
            names.push(text(&property(&simple, UIA_NamePropertyId)));
            types.push(number(&property(&simple, UIA_ControlTypePropertyId)));
            child = navigate(&element, NavigateDirection_NextSibling);
        }

        // Element-from-point, resolved from this thread over the same array the pointer
        // scans.
        let at: IRawElementProviderSimple = from_point(&root_of_fragments, 20.0, 20.0)
            .expect("something is under the point")
            .cast()
            .unwrap();
        let hit = text(&property(&at, UIA_NamePropertyId));

        let first = navigate(&group, NavigateDirection_FirstChild).unwrap();
        let button: IRawElementProviderSimple = first.cast().unwrap();
        let invokes = supports(&button, UIA_InvokePatternId);
        let toggles = supports(&button, UIA_TogglePatternId);

        let up: IRawElementProviderSimple = navigate(&first, NavigateDirection_Parent)
            .unwrap()
            .cast()
            .unwrap();
        let parent = text(&property(&up, UIA_NamePropertyId));

        (names, types, hit, invokes, toggles, parent)
    })
    .join()
    .expect("no provider call panicked or deadlocked");

    let (names, types, hit, invokes, toggles, parent) = walked;
    assert_eq!(names, ["mute", "gain"], "read off another thread entirely");
    assert_eq!(types[1], UIA_SliderControlTypeId);
    assert_eq!(hit, "mute", "element-from-point resolves like the pointer");
    assert!(invokes, "a button invokes");
    assert!(!toggles, "and does not toggle");
    assert_eq!(parent, "output");
}

#[test]
fn one_element_is_one_object_however_it_is_reached() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let group = screen.add(NONE, (0.0, 0.0, 200.0, 80.0), UiaRole::Group, "output");
    screen.add(group, (8.0, 8.0, 80.0, 32.0), UiaRole::Button, "mute");
    screen.publish(&mut uia);

    let root: IRawElementProviderFragment = uia.root_for_test().cast().unwrap();
    let reach = || {
        let group = navigate(&root, NavigateDirection_FirstChild).unwrap();
        navigate(&group, NavigateDirection_FirstChild).unwrap()
    };
    assert_eq!(
        reach().as_raw(),
        reach().as_raw(),
        "automation correlates raised events by object identity, so one element must be \
         one object however a client got to it"
    );
}

#[test]
fn an_unmounted_element_stops_resolving_rather_than_answering_for_its_successor() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let group = screen.add(NONE, (0.0, 0.0, 200.0, 80.0), UiaRole::Group, "output");
    screen.add(group, (8.0, 8.0, 80.0, 32.0), UiaRole::Button, "mute");
    screen.publish(&mut uia);

    let root: IRawElementProviderFragment = uia.root_for_test().cast().unwrap();
    let group = navigate(&root, NavigateDirection_FirstChild).unwrap();
    let stale: IRawElementProviderSimple = navigate(&group, NavigateDirection_FirstChild)
        .unwrap()
        .cast()
        .unwrap();
    assert_eq!(text(&property(&stale, UIA_NamePropertyId)), "mute");

    // The screen is replaced by a different one; the client is still holding the button.
    let mut next = screen.successor();
    next.add(NONE, (0.0, 0.0, 200.0, 80.0), UiaRole::Group, "input");
    next.add(0u16, (8.0, 8.0, 80.0, 32.0), UiaRole::Button, "solo");
    next.publish(&mut uia);

    for id in [
        UIA_NamePropertyId,
        crate::bindings::UIA_IsEnabledPropertyId,
        -1,
    ] {
        let mut out = VARIANT::default();
        // SAFETY: `stale` owns its interface and `out` is writable for the call.
        let status = unsafe { (stale.vtable().GetPropertyValue)(stale.as_raw(), id, &raw mut out) };
        assert_eq!(status, super::provider::gone().code());
    }
    let stale: IRawElementProviderFragment = stale.cast().unwrap();
    assert!(
        bounds(&stale).is_none(),
        "and its geometry is unavailable rather than somebody else's"
    );
}

// ── the text pattern, through its own vtables ───────────────────────────────────

/// Returns the pattern `id` names on `element`, where it supports one.
fn pattern<T: Interface>(element: &IRawElementProviderSimple, id: i32) -> Option<T> {
    let mut out = core::ptr::null_mut();
    // SAFETY: `element` holds a counted reference for the whole call, and `out` points at a
    // local that outlives it. A pattern the element does not support stores null, so the
    // pointer is checked before an owned reference is taken from it.
    unsafe {
        (element.vtable().GetPatternProvider)(element.as_raw(), id, &raw mut out)
            .ok()
            .ok()?;
        let held = windows_core::IUnknown::from_raw_borrowed(&out)?.clone();
        drop(windows_core::IUnknown::from_raw(out));
        held.cast().ok()
    }
}

#[test]
fn a_producer_missing_reading_never_marshals_as_zero_or_a_stale_number() {
    use crate::bindings::{IRangeValueProvider, UIA_RangeValuePatternId};
    use std::sync::{Arc, atomic::{AtomicU64, Ordering::Relaxed}};
    let mut uia = listening();
    let mut screen = Screen::new();
    let at = screen.slider(NONE, (0.0, 0.0, 100.0, 30.0), Range::new(-48.0, 12.0));
    screen.publish(&mut uia);
    let id = screen.control(at);
    let value = Arc::new(AtomicU64::new((-12.0f64).to_bits()));
    uia.shared.regions.bind_value(id, value.clone());
    let provider = super::provider::provider_for(&uia.shared, id).unwrap();
    let range: IRangeValueProvider = pattern(&provider, UIA_RangeValuePatternId).unwrap();
    for bits in [(-12.0f64).to_bits(), super::MISSING_READING, f64::NEG_INFINITY.to_bits(), f64::NAN.to_bits()] {
        value.store(bits, Relaxed);
        let mut out = 42.0;
        // SAFETY: `range` owns the live provider and `out` remains writable for the call.
        unsafe { (range.vtable().Value)(range.as_raw(), &raw mut out).ok().unwrap(); }
        if f64::from_bits(bits).is_finite() {
            assert_eq!(out, -12.0);
        } else {
            assert!(out.is_nan());
        }
    }
}

fn document_range(text: &ITextProvider2) -> ITextRangeProvider {
    let mut out = core::ptr::null_mut();
    // SAFETY: `text` holds a counted reference for the whole call, and `out` points at a local
    // that outlives it.
    unsafe {
        (text.vtable().base__.DocumentRange)(text.as_raw(), &raw mut out)
            .ok()
            .unwrap();
        ITextRangeProvider::from_raw(out)
    }
}

fn range_text(range: &ITextRangeProvider) -> String {
    let mut out: *mut core::ffi::c_void = core::ptr::null_mut();
    // SAFETY: `range` holds a counted reference for the whole call, `out` points at a local
    // that outlives it, and on success the callee hands over one owned string, adopted here.
    unsafe {
        (range.vtable().GetText)(range.as_raw(), -1, &raw mut out)
            .ok()
            .unwrap();
        String::try_from(&windows_core::BSTR::from_raw(out.cast())).unwrap_or_default()
    }
}

fn move_by(range: &ITextRangeProvider, unit: crate::bindings::TextUnit, count: i32) -> i32 {
    let mut moved = 0;
    // SAFETY: as above; `moved` is a local the callee writes one integer into.
    unsafe {
        (range.vtable().Move)(range.as_raw(), unit, count, &raw mut moved)
            .ok()
            .unwrap();
    }
    moved
}

fn expand(range: &ITextRangeProvider, unit: crate::bindings::TextUnit) {
    // SAFETY: `range` holds a counted reference for the whole call.
    unsafe {
        (range.vtable().ExpandToEnclosingUnit)(range.as_raw(), unit)
            .ok()
            .unwrap();
    }
}

fn move_endpoint(
    range: &ITextRangeProvider,
    which: crate::bindings::TextPatternRangeEndpoint,
    unit: crate::bindings::TextUnit,
    count: i32,
) -> i32 {
    let mut moved = 0;
    // SAFETY: as above; `moved` is a local the callee writes one integer into.
    unsafe {
        (range.vtable().MoveEndpointByUnit)(range.as_raw(), which, unit, count, &raw mut moved)
            .ok()
            .unwrap();
    }
    moved
}

/// Returns the flat `left, top, width, height` runs the range reports, and releases the array.
fn rectangles(range: &ITextRangeProvider) -> Vec<f64> {
    let mut array: *mut SAFEARRAY = core::ptr::null_mut();
    // SAFETY: `range` holds a counted reference for the whole call. On success the callee
    // hands over one array of doubles, whose bound is read before any element is, and which is
    // destroyed here.
    unsafe {
        (range.vtable().GetBoundingRectangles)(range.as_raw(), &raw mut array)
            .ok()
            .unwrap();
        if array.is_null() {
            return Vec::new();
        }
        let mut upper = -1;
        _ = SafeArrayGetUBound(array, 1, &raw mut upper);
        let mut out = Vec::new();
        for at in 0..=upper {
            let mut value = 0f64;
            if SafeArrayGetElement(array, &raw const at, (&raw mut value).cast()).is_ok() {
                out.push(value);
            }
        }
        _ = SafeArrayDestroy(array);
        out
    }
}

/// Returns the field's text provider, reached the way a client reaches it.
fn field_text(uia: &super::Uia, id: windows_scene::ControlId) -> ITextProvider2 {
    let element = super::provider::provider_for(uia.shared_for_test(), id).expect("a provider");
    pattern(&element, UIA_TextPattern2Id).expect("an edit publishes TextPattern2")
}

fn caret_range(text: &ITextProvider2) -> windows_core::Result<(bool, Option<ITextRangeProvider>)> {
    let mut active = windows_core::BOOL::from(true);
    let mut out = core::ptr::dangling_mut();
    // SAFETY: the provider is live, both outputs are writable locals, and a non-null
    // returned interface transfers one reference to the caller.
    unsafe {
        (text.vtable().GetCaretRange)(text.as_raw(), &raw mut active, &raw mut out).ok()?;
        Ok((
            active.as_bool(),
            (!out.is_null()).then(|| ITextRangeProvider::from_raw(out)),
        ))
    }
}

#[test]
fn text2_reports_the_selection_caret_and_live_focus_through_com() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let field = screen.field(NONE, (0.0, 0.0, 120.0, 24.0), "a\u{1f600}b", 8.0);
    screen.reselect(field, 3, 1);
    screen.publish(&mut uia);
    let id = screen.control(field);
    let text = field_text(&uia, id);
    let element = super::provider::provider_for(uia.shared_for_test(), id).unwrap();
    let inherited: ITextProvider = pattern(&element, UIA_TextPatternId).unwrap();
    assert_eq!(
        inherited.as_raw(),
        text.as_raw(),
        "COM inheritance uses the same vtable prefix"
    );

    uia.set_focus(Some(id));
    let (active, caret) = caret_range(&text).unwrap();
    assert!(active);
    let caret = caret.unwrap();
    assert_eq!(range_text(&caret), "");
    assert!(
        rectangles(&caret).is_empty(),
        "a degenerate text range contains no visible text"
    );
    expand(&caret, TextUnit_Character);
    assert_eq!(
        range_text(&caret),
        "\u{1f600}",
        "the caret is the moving endpoint, not the anchor"
    );
    assert!(!rectangles(&caret).is_empty());

    screen.reselect(field, 1, 3);
    screen.publish(&mut uia);
    uia.set_focus(None);
    let (active, caret) = caret_range(&text).unwrap();
    assert!(!active);
    let caret = caret.unwrap();
    expand(&caret, TextUnit_Character);
    assert_eq!(
        range_text(&caret),
        "b",
        "a cached provider reads the published caret"
    );

    let mut replacement = screen.successor();
    replacement.publish(&mut uia);
    assert_eq!(
        caret_range(&text).unwrap_err().code(),
        super::provider::gone().code()
    );
}

#[test]
fn text2_exposes_no_caret_for_static_text_or_passwords() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let field = screen.field(NONE, (0.0, 0.0, 120.0, 24.0), "secret", 8.0);
    screen.snapshot.fields[0].password = true;
    let label = screen.add(NONE, (0.0, 30.0, 120.0, 54.0), UiaRole::Text, "label");
    screen.snapshot.entries[label as usize].flags = super::snapshot::ColFlags::BODY;
    let button = screen.add(NONE, (0.0, 60.0, 120.0, 84.0), UiaRole::Button, "button");
    screen.publish(&mut uia);
    for at in [field, label] {
        uia.set_focus(Some(screen.control(at)));
        let text = field_text(&uia, screen.control(at));
        let (active, caret) = caret_range(&text).unwrap();
        assert!(!active);
        assert!(caret.is_none());
        let mut annotation = core::ptr::dangling_mut();
        // SAFETY: the provider is live and the output is writable. A null annotation
        // names no object, so no input pointer is dereferenced by this query.
        unsafe {
            (text.vtable().RangeFromAnnotation)(
                text.as_raw(),
                core::ptr::null_mut(),
                &raw mut annotation,
            )
            .ok()
            .unwrap();
        }
        assert!(annotation.is_null());
    }
    let element =
        super::provider::provider_for(uia.shared_for_test(), screen.control(button)).unwrap();
    assert!(pattern::<ITextProvider2>(&element, UIA_TextPattern2Id).is_none());
}

/// A character of an editable document is a cluster, so one step crosses a supplementary
/// character whole rather than landing between its two code units.
#[test]
fn a_character_walk_through_the_provider_crosses_whole_clusters() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let field = screen.field(NONE, (0.0, 0.0, 120.0, 24.0), "a\u{1f600}b", 8.0);
    screen.publish(&mut uia);

    let text = field_text(&uia, screen.control(field));
    let range = document_range(&text);
    assert_eq!(range_text(&range), "a\u{1f600}b");

    expand(&range, TextUnit_Character);
    assert_eq!(range_text(&range), "a");
    assert_eq!(move_by(&range, TextUnit_Character, 1), 1);
    assert_eq!(
        range_text(&range),
        "\u{1f600}",
        "one step crossed the pair, not half of it"
    );
    assert_eq!(move_by(&range, TextUnit_Character, 1), 1);
    assert_eq!(range_text(&range), "b");
    assert_eq!(
        move_by(&range, TextUnit_Character, 4),
        1,
        "the walk stops at the end and reports how far it got"
    );
    assert_eq!(range_text(&range), "", "and rests degenerate there");
    assert_eq!(move_by(&range, TextUnit_Character, 1), 0);

    let whole = document_range(&text);
    assert_eq!(
        move_endpoint(&whole, TextPatternRangeEndpoint_End, TextUnit_Character, -1),
        -1
    );
    assert_eq!(range_text(&whole), "a\u{1f600}");
}

/// A range's rectangles are cut by every clipping ancestor: a field a scroller has carried
/// past its own edge reports the part that shows, and one carried out of it entirely reports
/// nothing at all.
#[test]
fn a_ranges_rectangles_are_cut_by_a_clipping_ancestor() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let list = screen.add(NONE, (0.0, 0.0, 120.0, 60.0), UiaRole::Group, "list");
    let field = screen.field(list, (0.0, 40.0, 120.0, 64.0), "ab", 8.0);
    let viewport = windows_scene::NodeId::raw(7, 0);
    screen.scrolls(list, viewport, &[field]);
    screen.publish(&mut uia);

    let text = field_text(&uia, screen.control(field));
    assert_eq!(
        rectangles(&document_range(&text)),
        vec![0.0, 40.0, 16.0, 20.0],
        "the 24-DIP run is cut to the 20 the list admits"
    );

    uia.set_scroll(viewport, Vector2 { x: 0.0, y: 40.0 });
    assert_eq!(
        rectangles(&document_range(&text)),
        vec![0.0, 0.0, 16.0, 24.0],
        "scrolled into view, the whole run is reported"
    );

    uia.set_scroll(viewport, Vector2 { x: 0.0, y: -100.0 });
    assert!(
        rectangles(&document_range(&text)).is_empty(),
        "carried past the list's edge, a client is given nothing to draw"
    );
}

/// An element's own bounding rectangle is cut by the same clip chain its text ranges are, so
/// a highlight and the box it sits in cannot disagree about where the element is.
#[test]
fn an_elements_bounding_rectangle_is_cut_by_the_same_clip_its_ranges_are() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let list = screen.add(NONE, (0.0, 0.0, 120.0, 60.0), UiaRole::Group, "list");
    let field = screen.field(list, (0.0, 40.0, 120.0, 64.0), "ab", 8.0);
    let viewport = windows_scene::NodeId::raw(7, 0);
    screen.scrolls(list, viewport, &[field]);
    screen.publish(&mut uia);

    let id = screen.control(field);
    let element: IRawElementProviderFragment =
        super::provider::provider_for(uia.shared_for_test(), id)
            .expect("a provider")
            .cast()
            .unwrap();

    let partial = bounds(&element).expect("a live element");
    assert_eq!(
        (partial.top, partial.height),
        (40.0, 20.0),
        "the 24-DIP field is cut to the 20 the list admits"
    );
    let ranged = rectangles(&document_range(&field_text(&uia, id)));
    assert_eq!(
        ranged[1] + ranged[3],
        partial.top + partial.height,
        "the run and the element are cut at the same line"
    );

    uia.set_scroll(viewport, Vector2 { x: 0.0, y: 40.0 });
    let whole = bounds(&element).expect("a live element");
    assert_eq!((whole.top, whole.height), (0.0, 24.0));

    uia.set_scroll(viewport, Vector2 { x: 0.0, y: -100.0 });
    let gone = bounds(&element).expect("a live element");
    assert_eq!(
        (gone.width, gone.height),
        (0.0, 0.0),
        "carried out of the list, it is nowhere rather than somewhere it cannot be reached"
    );
}

/// Consumes a provider SAFEARRAY, retaining each returned COM interface.
fn providers<T: Interface>(array: *mut SAFEARRAY) -> Vec<T> {
    assert!(!array.is_null());
    let mut high = -1;
    let mut out = Vec::new();
    // SAFETY: the array is an owned VT_UNKNOWN array and each successful read adds a reference.
    unsafe {
        SafeArrayGetUBound(array, 1, &raw mut high).unwrap();
        for index in 0..=high {
            let mut raw: *mut core::ffi::c_void = core::ptr::null_mut();
            SafeArrayGetElement(array, &index, (&raw mut raw).cast()).unwrap();
            out.push(windows_core::IUnknown::from_raw(raw).cast::<T>().unwrap());
        }
        SafeArrayDestroy(array).unwrap();
    }
    out
}

fn selection_items(element: &IRawElementProviderSimple) -> Vec<IRawElementProviderSimple> {
    let selection: crate::bindings::ISelectionProvider = element.cast().unwrap();
    let mut array = core::ptr::null_mut();
    // SAFETY: the counted interface and local out pointer live through the call.
    unsafe {
        (selection.vtable().GetSelection)(selection.as_raw(), &raw mut array).unwrap();
    }
    providers(array)
}

#[test]
fn collapsed_combo_publishes_a_navigable_selected_option_and_retires_replaced_options() {
    use super::{ColFlags, State};
    use crate::bindings::*;
    let mut uia = listening();
    let mut screen = Screen::new();
    let combo = screen.add(NONE, (0.0, 0.0, 100.0, 24.0), UiaRole::ComboBox, "Filter");
    screen.snapshot.entries[combo as usize].flags =
        ColFlags::EXPANDS | ColFlags::SELECTION_REQUIRED;
    let name = screen.snapshot.intern("Low shelf");
    screen.snapshot.choices.push((combo, 7, name));
    screen.publish(&mut uia);
    let owner = uia
        .shared
        .object(screen.control(combo), super::provider::NO_PART);
    let item = selection_items(&owner).pop().unwrap();
    assert_eq!(text(&property(&item, UIA_NamePropertyId)), "Low shelf");
    assert_eq!(
        number(&property(&item, UIA_ControlTypePropertyId)),
        UIA_ListItemControlTypeId
    );
    assert!(supports(&item, UIA_SelectionItemPatternId));
    assert!(!supports(&item, UIA_ExpandCollapsePatternId));
    let fragment: IRawElementProviderFragment = owner.cast().unwrap();
    assert_eq!(
        navigate(&fragment, NavigateDirection_FirstChild)
            .unwrap()
            .cast::<IRawElementProviderSimple>()
            .unwrap(),
        item
    );
    let selected: ISelectionItemProvider = item.cast().unwrap();
    let mut container = core::ptr::null_mut();
    // SAFETY: each interface is counted and all outputs point to live locals.
    unsafe {
        (selected.vtable().SelectionContainer)(selected.as_raw(), &raw mut container).unwrap();
        assert_eq!(IRawElementProviderSimple::from_raw(container), owner);
        (selected.vtable().Select)(selected.as_raw()).unwrap();
        (selected.vtable().AddToSelection)(selected.as_raw()).unwrap();
        assert!((selected.vtable().RemoveFromSelection)(selected.as_raw()).is_err());
    }
    screen.snapshot.choices[0].1 = 8;
    screen.publish(&mut uia);
    assert!(bounds(&item.cast().unwrap()).is_none());
    screen.snapshot.state[combo as usize] = State::ENABLED | State::EXPANDED;
    let list = screen.add(combo, (0.0, 24.0, 100.0, 72.0), UiaRole::List, "Options");
    let option = screen.add(
        list,
        (0.0, 24.0, 100.0, 48.0),
        UiaRole::Button,
        "High shelf",
    );
    screen.snapshot.entries[option as usize].flags = ColFlags::SELECTS;
    screen.snapshot.state[option as usize] = State::ENABLED | State::SELECTED;
    screen.publish(&mut uia);
    assert_eq!(
        text(&property(&selection_items(&owner)[0], UIA_NamePropertyId)),
        "High shelf"
    );
}

#[test]
fn optional_selection_distinguishes_add_remove_and_replace_through_com() {
    use super::action::SelectionChange;
    use super::{ColFlags, State};
    use crate::bindings::*;
    let mut uia = listening();
    let mut screen = Screen::new();
    let group = screen.add(NONE, (0.0, 0.0, 100.0, 80.0), UiaRole::Group, "Choices");
    screen.snapshot.entries[group as usize].flags = ColFlags::SELECTION;
    let first = screen.add(
        group,
        (0.0, 0.0, 100.0, 24.0),
        UiaRole::RadioButton,
        "First",
    );
    let wrapper = screen.add(group, (0.0, 24.0, 100.0, 48.0), UiaRole::Group, "Layout");
    let second = screen.add(
        wrapper,
        (0.0, 24.0, 100.0, 48.0),
        UiaRole::RadioButton,
        "Second",
    );
    screen.snapshot.state[first as usize] = State::ENABLED | State::SELECTED;
    screen.publish(&mut uia);
    let a: ISelectionItemProvider = uia
        .shared
        .object(screen.control(first), super::provider::NO_PART)
        .cast()
        .unwrap();
    let b = uia
        .shared
        .object(screen.control(second), super::provider::NO_PART);
    assert_eq!(number(&property(&b, UIA_PositionInSetPropertyId)), 2);
    assert_eq!(number(&property(&b, UIA_SizeOfSetPropertyId)), 2);
    let b: ISelectionItemProvider = b.cast().unwrap();
    // SAFETY: the interfaces remain counted for all calls.
    unsafe {
        assert!((b.vtable().AddToSelection)(b.as_raw()).is_err());
        (a.vtable().RemoveFromSelection)(a.as_raw()).unwrap();
        (b.vtable().Select)(b.as_raw()).unwrap();
    }
    let mut actions = Vec::new();
    uia.shared.actions.drain(&mut actions);
    assert_eq!(
        actions,
        vec![
            super::Action::Select(screen.control(first), SelectionChange::Remove),
            super::Action::Select(screen.control(second), SelectionChange::Select)
        ]
    );
    screen.snapshot.entries[group as usize].flags =
        ColFlags::SELECTION | ColFlags::SELECTION_REQUIRED;
    screen.publish(&mut uia);
    // SAFETY: the cached provider resolves the newly published required-selection flag.
    unsafe {
        assert!((a.vtable().RemoveFromSelection)(a.as_raw()).is_err());
    }
}

#[test]
fn dialog_window_pattern_closes_without_claiming_resize_or_minimize() {
    use super::ColFlags;
    use crate::bindings::*;
    let mut uia = listening();
    let mut screen = Screen::new();
    let at = screen.add(NONE, (0.0, 0.0, 200.0, 200.0), UiaRole::Group, "Inspector");
    screen.snapshot.entries[at as usize].flags = ColFlags::DIALOG | ColFlags::OVERLAY;
    screen.publish(&mut uia);
    let simple = uia
        .shared
        .object(screen.control(at), super::provider::NO_PART);
    assert!(supports(&simple, UIA_WindowPatternId));
    assert!(supports(&simple, UIA_TransformPatternId));
    let window: IWindowProvider = simple.cast().unwrap();
    let transform: ITransformProvider = simple.cast().unwrap();
    let mut value = windows_core::BOOL::default();
    // SAFETY: both providers are counted and each out parameter points to a local.
    unsafe {
        (window.vtable().IsModal)(window.as_raw(), &raw mut value).unwrap();
        assert!(value.as_bool());
        (transform.vtable().CanResize)(transform.as_raw(), &raw mut value).unwrap();
        assert!(!value.as_bool());
        assert!((transform.vtable().Resize)(transform.as_raw(), 1.0, 1.0).is_err());
        assert!(
            (window.vtable().SetVisualState)(window.as_raw(), WindowVisualState_Minimized).is_err()
        );
        (window.vtable().SetVisualState)(window.as_raw(), WindowVisualState_Normal).unwrap();
        (window.vtable().Close)(window.as_raw()).unwrap();
    }
    let mut actions = Vec::new();
    uia.shared.actions.drain(&mut actions);
    assert_eq!(
        actions,
        vec![super::Action::CloseWindow(screen.control(at))]
    );
    let mut next = screen.successor();
    next.publish(&mut uia);
    // SAFETY: cached providers remain counted after the dialog retires.
    unsafe {
        assert!((window.vtable().Close)(window.as_raw()).is_err());
    }
}

#[test]
fn visible_text_ranges_follow_clusters_clips_and_scroll_and_preserve_reveal_span() {
    use crate::bindings::*;
    let mut uia = listening();
    let mut screen = Screen::new();
    let clip = screen.add(NONE, (0.0, 0.0, 16.0, 24.0), UiaRole::Group, "Clip");
    let field = screen.field(clip, (0.0, 0.0, 80.0, 24.0), "abcdef", 8.0);
    screen.snapshot.entries[field as usize].clip = clip;
    screen.publish(&mut uia);
    let text = field_text(&uia, screen.control(field));
    let mut array = core::ptr::null_mut();
    // SAFETY: the text provider is counted and array is a local out parameter.
    unsafe {
        (text.vtable().base__.GetVisibleRanges)(text.as_raw(), &raw mut array).unwrap();
    }
    let ranges = providers::<ITextRangeProvider>(array);
    assert_eq!(ranges.len(), 1);
    assert_eq!(range_text(&ranges[0]), "ab");
    // SAFETY: the range is counted and TRUE requests top alignment.
    unsafe {
        (ranges[0].vtable().ScrollIntoView)(ranges[0].as_raw(), true.into()).unwrap();
    }
    let mut actions = Vec::new();
    uia.shared.actions.drain(&mut actions);
    assert_eq!(
        actions,
        vec![super::Action::RevealText(
            screen.control(field),
            1,
            0,
            2,
            true
        )]
    );
    screen.snapshot.entries[clip as usize].box_ = [100.0, 0.0, 116.0, 24.0];
    screen.publish(&mut uia);
    // SAFETY: the cached text provider resolves the new clip.
    unsafe {
        (text.vtable().base__.GetVisibleRanges)(text.as_raw(), &raw mut array).unwrap();
    }
    assert_eq!(range_text(&providers::<ITextRangeProvider>(array)[0]), "");
}

#[test]
fn static_text_uses_published_clusters_and_advertises_no_selection() {
    use crate::bindings::*;
    let mut uia = listening();
    let mut screen = Screen::new();
    let row = screen.field(NONE, (0.0, 0.0, 16.0, 24.0), "abcdef", 8.0);
    let field = screen.snapshot.fields.pop().unwrap();
    screen
        .snapshot
        .text_geometry
        .push((row, field.geometry.unwrap()));
    screen.snapshot.entries[row as usize].role = UiaRole::Text;
    screen.snapshot.entries[row as usize].flags = super::ColFlags::BODY;
    screen.snapshot.entries[row as usize].name = screen.snapshot.intern("abcdef");
    screen.publish(&mut uia);
    let provider = field_text(&uia, screen.control(row));
    let mut supported = SupportedTextSelection_Single;
    let mut array = core::ptr::null_mut();
    // SAFETY: the counted provider writes to live local out parameters.
    unsafe {
        (provider.vtable().base__.SupportedTextSelection)(provider.as_raw(), &raw mut supported)
            .unwrap();
        (provider.vtable().base__.GetVisibleRanges)(provider.as_raw(), &raw mut array).unwrap();
    }
    assert_eq!(supported, SupportedTextSelection_None);
    let ranges = providers::<ITextRangeProvider>(array);
    assert_eq!(range_text(&ranges[0]), "ab");
    assert!(!rectangles(&ranges[0]).is_empty());
}

#[test]
fn window_focus_round_trip_updates_cached_provider_and_focus_events() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let field = screen.field(NONE, (0.0, 0.0, 120.0, 24.0), "text", 8.0);
    screen.publish(&mut uia);
    let id = screen.control(field);
    let element = super::provider::provider_for(uia.shared_for_test(), id).unwrap();
    let text = field_text(&uia, id);
    let mut ring = crate::input::FocusRing::default();
    let mut raised = Vec::new();
    ring.focus(Some(id));
    uia.take_pending_for_test(&mut raised);
    for active in [false, true, false, true] {
        raised.clear();
        ring.window_focus(active, &screen.table());
        uia.set_focus(ring.keyboard());
        assert_eq!(caret_range(&text).unwrap().0, active);
        uia.take_pending_for_test(&mut raised);
        assert_eq!(
            raised.iter().filter(|event| **event == super::events::Raise::focus(id)).count(),
            usize::from(active),
        );
        assert_eq!(ring.current(), Some(id));
        let value = property(&element, crate::bindings::UIA_HasKeyboardFocusPropertyId);
        assert_eq!(tag(&value), 11);
        // SAFETY: the checked VT_BOOL tag identifies the initialized boolVal arm.
        assert_eq!(unsafe { value.Anonymous.Anonymous.Anonymous.boolVal } != 0, active);
    }
}
