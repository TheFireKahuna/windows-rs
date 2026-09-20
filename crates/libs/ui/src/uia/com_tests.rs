//! Tests that drive the provider through its COM vtables from a thread that never
//! published, with the publishing thread parked in a join.
//!
//! [`tests`](super::tests) covers the data model. These calls go through the generated
//! implement-side vtables, which is the surface automation itself calls.

use super::NONE;
use super::tests::{Screen, listening};
use crate::bindings::{
    IRawElementProviderFragment, IRawElementProviderFragmentRoot, IRawElementProviderSimple,
    ITextProvider, ITextRangeProvider, NavigateDirection_FirstChild, NavigateDirection_NextSibling,
    NavigateDirection_Parent, SAFEARRAY, TextPatternRangeEndpoint_End, TextUnit_Character,
    UIA_ControlTypePropertyId, UIA_InvokePatternId, UIA_NamePropertyId, UIA_SliderControlTypeId,
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

    assert_eq!(
        tag(&property(&stale, UIA_NamePropertyId)),
        0,
        "an element that has gone answers nothing, not the one that took its slot"
    );
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

fn document_range(text: &ITextProvider) -> ITextRangeProvider {
    let mut out = core::ptr::null_mut();
    // SAFETY: `text` holds a counted reference for the whole call, and `out` points at a local
    // that outlives it.
    unsafe {
        (text.vtable().DocumentRange)(text.as_raw(), &raw mut out)
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
fn field_text(uia: &super::Uia, id: windows_scene::ControlId) -> ITextProvider {
    let element = super::provider::provider_for(uia.shared_for_test(), id).expect("a provider");
    pattern(&element, UIA_TextPatternId).expect("an edit publishes the text pattern")
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

    uia.set_scroll(
        viewport,
        Vector2 {
            x: 0.0,
            y: -100.0,
        },
    );
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

    uia.set_scroll(
        viewport,
        Vector2 {
            x: 0.0,
            y: -100.0,
        },
    );
    let gone = bounds(&element).expect("a live element");
    assert_eq!(
        (gone.width, gone.height),
        (0.0, 0.0),
        "carried out of the list, it is nowhere rather than somewhere it cannot be reached"
    );
}
