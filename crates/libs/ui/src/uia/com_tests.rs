//! The COM surface, driven.
//!
//! [`tests`](super::tests) exercises the data. This exercises what the design actually
//! claims: that a provider answers correctly **from another thread**, with the publishing
//! thread not involved and not even running. A data model that is right and a vtable that
//! deadlocks would pass every test over there.

use super::tests::{Screen, listening};
use crate::bindings::{
    IRawElementProviderFragment, IRawElementProviderFragmentRoot, IRawElementProviderSimple,
    NavigateDirection_FirstChild, NavigateDirection_NextSibling, NavigateDirection_Parent,
    UIA_ControlTypePropertyId, UIA_InvokePatternId, UIA_NamePropertyId, UIA_SliderControlTypeId,
    UIA_TogglePatternId, VARIANT,
};
use crate::widget::{Range, UiaRole};
use windows_core::Interface;
use windows_scene::NO_ENTRY;

/// A provider addressed the way automation addresses one: a bare pointer any apartment may
/// call. Sending it is the test.
struct Agile(*mut core::ffi::c_void);

// SAFETY: this is the claim under test, and it is the platform's own contract for an agile
// object. The referent outlives the thread because the `Uia` that owns it does.
unsafe impl Send for Agile {}

impl Agile {
    fn of(provider: &IRawElementProviderSimple) -> Self {
        Self(provider.as_raw())
    }

    /// SAFETY: the caller keeps the owning `Uia` alive across the borrow.
    unsafe fn simple(&self) -> IRawElementProviderSimple {
        unsafe { IRawElementProviderSimple::from_raw_borrowed(&self.0) }
            .expect("a live provider")
            .clone()
    }
}

// The generated bindings are implement-side, so there are no client wrappers to call. That
// is not a gap here: calling straight through the vtable is exactly what automation does,
// and it is the surface actually under test.

fn navigate(
    element: &IRawElementProviderFragment,
    direction: crate::bindings::NavigateDirection,
) -> Option<IRawElementProviderFragment> {
    let mut out = core::ptr::null_mut();
    // SAFETY: a live provider, the direction it declares, and an out-pointer to a local.
    // A `S_OK` with null is "nothing that way", which is why the pointer is checked.
    unsafe {
        (element.vtable().Navigate)(element.as_raw(), direction, &raw mut out)
            .ok()
            .ok()?;
        IRawElementProviderFragment::from_raw_borrowed(&out).cloned()
    }
}

fn property(element: &IRawElementProviderSimple, id: i32) -> VARIANT {
    let mut out = VARIANT::default();
    // SAFETY: a live provider and an out-pointer to a zeroed variant, which the callee
    // fills and this thread owns from here.
    unsafe {
        _ = (element.vtable().GetPropertyValue)(element.as_raw(), id, &raw mut out);
    }
    out
}

fn supports(element: &IRawElementProviderSimple, pattern: i32) -> bool {
    let mut out = core::ptr::null_mut();
    // SAFETY: as above. A null out with `S_OK` is the documented "not supported".
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
    // SAFETY: a live provider and an out-pointer to a local.
    unsafe {
        (root.vtable().ElementProviderFromPoint)(root.as_raw(), x, y, &raw mut out)
            .ok()
            .ok()?;
        IRawElementProviderFragment::from_raw_borrowed(&out).cloned()
    }
}

fn bounds(element: &IRawElementProviderFragment) -> Option<crate::bindings::UiaRect> {
    let mut out = crate::bindings::UiaRect::default();
    // SAFETY: a live provider and an out-pointer to a local.
    unsafe {
        (element.vtable().get_BoundingRectangle)(element.as_raw(), &raw mut out)
            .ok()
            .ok()?;
    }
    Some(out)
}

/// The tag every variant carries, which is the whole of what "nothing here" is.
fn tag(value: &VARIANT) -> u16 {
    // SAFETY: the tag is present in every arm of the union.
    unsafe { value.Anonymous.Anonymous.vt }
}

fn text(value: &VARIANT) -> String {
    assert_eq!(tag(value), 8, "expected a BSTR");
    // SAFETY: read only after asserting the tag, and `bstrVal` is the arm it names.
    unsafe { String::try_from(&*value.Anonymous.Anonymous.Anonymous.bstrVal).unwrap_or_default() }
}

fn number(value: &VARIANT) -> i32 {
    assert_eq!(tag(value), 3, "expected an I4");
    // SAFETY: as above, for the `VT_I4` arm.
    unsafe { value.Anonymous.Anonymous.Anonymous.lVal }
}

#[test]
fn a_provider_answers_from_a_thread_that_never_published() {
    let mut uia = listening();
    let mut screen = Screen::new();
    let group = screen.add(NO_ENTRY, (0.0, 0.0, 200.0, 80.0), UiaRole::Group, "output");
    screen.add(group, (8.0, 8.0, 80.0, 32.0), UiaRole::Button, "mute");
    screen.slider(group, (96.0, 8.0, 190.0, 32.0), Range::new(-60.0, 0.0));
    screen.publish(&mut uia);

    let root = Agile::of(&uia.root_for_test());
    let walked = std::thread::spawn(move || {
        // SAFETY: `uia` is alive on the parent thread, which joins below.
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

        // Element-from-point, from here, over the same array the pointer scans.
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
    let group = screen.add(NO_ENTRY, (0.0, 0.0, 200.0, 80.0), UiaRole::Group, "output");
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
    let group = screen.add(NO_ENTRY, (0.0, 0.0, 200.0, 80.0), UiaRole::Group, "output");
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
    next.add(NO_ENTRY, (0.0, 0.0, 200.0, 80.0), UiaRole::Group, "input");
    next.add(0, (8.0, 8.0, 80.0, 32.0), UiaRole::Button, "solo");
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
