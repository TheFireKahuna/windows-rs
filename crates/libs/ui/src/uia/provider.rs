//! The two provider objects, and every interface they answer.
//!
//! UI Automation lets one COM object implement every pattern interface and decide in
//! `GetPatternProvider` which of them an element admits to, so there are two object shapes
//! here and not one per pattern: [`Element`] over `(Weak<Shared>, id, part)`, and [`Root`]
//! for the window, which is the only element that answers element-from-point and focus.
//!
//! Nothing is cached. Every call reads the currently published snapshot, from whatever thread
//! automation chose to call on, so there is nothing to invalidate and no call hops onto the
//! window's pump to read a field.
//!
//! Providers built by `implement_decl!` are agile, so a client may call one from any
//! apartment. Everything reachable from here is either immutable or an atomic.

use super::action::{Action, Queue, TextAction};
use super::regions::Regions;
use super::roles;
use super::snapshot::{ColFlags as F, Entry, NONE, Part, State as S, Tree, Versioned};
use super::snapshot::{ColFlags, State};
use super::variant::{self, bool as vb, i4 as vi, wide as vw};
use crate::bindings::*;
use crate::widget::{Range, UiaRole};
use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicIsize, Ordering::Relaxed};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use windows_core::{BOOL, BSTR, Error, IUnknown, Interface, PCWSTR, Result, implement_decl};
use windows_scene::{ControlId, Point};

/// The provider options this stack advertises.
///
/// `ServerSideProvider`, because the control implements its own provider. Automation
/// broadcasts events raised from a server-side provider and keeps a client-side one inside the
/// client process, so claiming `ClientSideProvider` here would lose every raised event.
///
/// `UseComThreading` is not set. It spares a provider from being thread-safe by funnelling
/// every query through the window's own pump, which is the path along which a slow provider
/// blocks a screen reader. Every provider here is thread-safe already.
///
/// `RefuseNonClientSupport`, because the window draws its own caption and publishes those
/// buttons as ordinary elements. Without it the system contributes a second set.
const OPTIONS: ProviderOptions =
    ProviderOptions_ServerSideProvider | ProviderOptions_RefuseNonClientSupport;

/// `UiaRootObjectId`. A `WM_GETOBJECT` naming any other object id is not ours to answer.
const ROOT_OBJECT_ID: i32 = -25;

/// The part id of an element that is a real entry rather than one of a region's parts.
pub use super::regions::NO_PART;

/// Fraction of a range's span reported as `LargeChange`, one page of movement.
/// [`SMALL_FRACTION`] is what `SmallChange` reports where the range declares no step of its
/// own. A client offers both as keyboard increments, so they are the steps the control itself
/// moves in.
const LARGE_FRACTION: f64 = 0.1;
const SMALL_FRACTION: f64 = 0.01;

const FRAMEWORK: &str = "windows-ui";

/// Everything a provider can reach, and the only state shared across threads.
///
/// Held by the front thread's [`Uia`](super::Uia) as an `Arc` and by every provider as a
/// `Weak`, so objects a client still holds cannot keep a closed window's state alive. They
/// stop resolving instead, which is what `UIA_E_ELEMENTNOTAVAILABLE` reports.
#[derive(Default)]
pub struct Shared {
    pub tree: Versioned<Arc<Tree>>,
    /// What presentation regions declare. Held beside the snapshot rather than in it, so a
    /// band drag republishes no element.
    pub regions: Regions,
    pub(crate) actions: Queue<Action>,
    pub(crate) edits: Queue<TextAction>,
    /// Latched by the first `WM_GETOBJECT` and cleared only by [`disconnect`].
    /// `UiaClientsAreListening` is a hint; having been asked for a provider is not.
    pub asked: AtomicBool,
    hwnd: AtomicIsize,
    /// One object per element identity, sorted by it, so an element always answers as the same
    /// object. Automation matches a raised event to a listener by object identity.
    objects: Mutex<Vec<((ControlId, u32), Agile)>>,
}

/// An agile provider: callable and reference-countable from any apartment.
struct Agile(IRawElementProviderSimple);

// SAFETY: `implement_decl!` objects answer `IAgileObject`/`IMarshal`, so a single instance is
// safely shared across automation's worker threads.
unsafe impl Send for Agile {}

impl Shared {
    /// Records the window every provider answers for.
    pub fn attach(&self, hwnd: HWND) {
        self.hwnd.store(hwnd as isize, Relaxed);
    }

    /// Returns the attached window, or null before [`attach`](Self::attach) has run.
    pub fn window(&self) -> HWND {
        self.hwnd.load(Relaxed) as HWND
    }

    /// Queues an action and asks the front thread for a tick.
    ///
    /// Posts the pacer's own message, so the tick that runs the action is the tick that
    /// services input, draining in the same order and publishing the same way. Only the first
    /// action of a batch posts, because the drain takes the whole queue.
    pub fn act(&self, action: Action) {
        if self.actions.push(action) {
            self.wake();
        }
    }

    /// Queues an editing request on the same terms as [`act`](Self::act).
    pub(crate) fn edit(&self, action: TextAction) {
        if self.edits.push(action) {
            self.wake();
        }
    }

    /// Asks the front thread for a tick.
    pub fn wake(&self) {
        let hwnd = self.window();
        if hwnd.is_null() {
            return;
        }
        // SAFETY: `PostMessageW` is callable from any thread and validates the handle itself,
        // so a handle whose window has gone fails the call rather than faulting — which is why
        // the result is dropped.
        unsafe { _ = PostMessageW(hwnd, windows_window::WM_FRAME, 0, 0) }
    }

    /// Returns the stable object for one element identity, minting it on the first ask.
    pub fn object(self: &Arc<Self>, id: ControlId, part: u32) -> IRawElementProviderSimple {
        let mut held = self.objects.lock().unwrap_or_else(PoisonError::into_inner);
        let key = (id, part);
        match held.binary_search_by_key(&key, |&(held, _)| held) {
            Ok(found) => held[found].1.0.clone(),
            Err(found) => {
                let element = Element {
                    shared: Arc::downgrade(self),
                    id,
                    part,
                };
                let made: IRawElementProviderSimple = if id.is_none() {
                    Root(element).into()
                } else {
                    element.into()
                };
                held.insert(found, (key, Agile(made.clone())));
                made
            }
        }
    }

    /// Drops the objects for elements the new tree no longer has.
    ///
    /// A stale provider is already correct without this, because an id that no longer resolves
    /// answers `UIA_E_ELEMENTNOTAVAILABLE`. What this bounds is the size of the table across a
    /// session that mounts and unmounts.
    pub fn evict(&self, tree: &Tree) {
        self.objects
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|&((id, _), _)| id.is_none() || tree.index_of(id).is_some());
    }

    /// Drops every provider object minted for this window.
    pub fn forget(&self) {
        self.objects
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

thread_local! {
    /// This thread's own reference to the published snapshot, refreshed only when it moves.
    static SEEN: RefCell<Option<(u64, Arc<Tree>)>> = const { RefCell::new(None) };
}

/// Returns the published snapshot, refreshing this thread's own reference if it has moved.
pub fn tree_of(shared: &Arc<Shared>) -> Arc<Tree> {
    SEEN.with(|seen| shared.tree.tree(seen))
}

/// One element: a weak reference to [`Shared`], and the two ids that name it.
pub struct Element {
    shared: Weak<Shared>,
    /// [`ControlId::NONE`] is the fragment root, which is the window and is not in the table.
    /// No minted control can be it: ids start at generation one.
    pub id: ControlId,
    /// Which part of a presentation region, or [`NO_PART`].
    pub part: u32,
}

/// The same element with the fragment-root interfaces added. A separate type because only the
/// root answers element-from-point and focus.
pub struct Root(Element);

impl core::ops::Deref for Root {
    type Target = Element;
    fn deref(&self) -> &Element {
        &self.0
    }
}

implement_decl! {
    impl Element as pub Element_Impl: [
        IRawElementProviderSimple,
        IRawElementProviderFragment,
        IInvokeProvider,
        IToggleProvider,
        IValueProvider,
        IRangeValueProvider,
        ISelectionProvider,
        ISelectionItemProvider,
        IExpandCollapseProvider,
        IScrollItemProvider,
        ITextProvider
    ]
}

implement_decl! {
    impl Root as pub Root_Impl: [
        IRawElementProviderSimple,
        IRawElementProviderFragment,
        IRawElementProviderFragmentRoot
    ]
}

/// A resolved element: the snapshot it lives in, where in it, and which part of it.
pub struct At {
    pub shared: Arc<Shared>,
    pub tree: Arc<Tree>,
    pub at: u16,
    pub part: u32,
}

impl Element {
    /// Returns the shared state, or `UIA_E_ELEMENTNOTAVAILABLE` once the window has gone.
    pub fn shared(&self) -> Result<Arc<Shared>> {
        self.shared.upgrade().ok_or_else(gone)
    }

    /// Resolves the element against the current snapshot.
    ///
    /// # Errors
    ///
    /// `UIA_E_ELEMENTNOTAVAILABLE` once the element has unmounted or the window has gone. The
    /// identity is a generational id rather than a cached node pointer, so an unmounted
    /// element has nothing to dangle and no registration to forget.
    pub fn at(&self) -> Result<At> {
        let shared = self.shared()?;
        let tree = tree_of(&shared);
        let at = tree.index_of(self.id).ok_or_else(gone)?;
        Ok(At {
            shared,
            tree,
            at,
            part: self.part,
        })
    }

    /// Returns the shared state and the snapshot without resolving an index.
    ///
    /// What the root reads: it is the window, so it is not in the table.
    pub fn window(&self) -> Result<(Arc<Shared>, Arc<Tree>)> {
        let shared = self.shared()?;
        let tree = tree_of(&shared);
        Ok((shared, tree))
    }

    /// Queues the action `make` names, once the element resolves and is enabled.
    ///
    /// Every command goes through here, so the enabled gate is stated once rather than at
    /// seven call sites that can disagree about whether to state it.
    fn command(&self, make: impl FnOnce(ControlId) -> Action) -> Result<()> {
        let at = self.at()?;
        if !at.tree.state(at.at).has(State::ENABLED) {
            return Err(disabled());
        }
        at.shared.act(make(self.id));
        Ok(())
    }
}

impl At {
    /// Returns a resolved element at `at` in `tree`, addressing the whole element.
    pub(super) fn of(shared: &Arc<Shared>, tree: &Arc<Tree>, at: u16) -> Self {
        Self {
            shared: Arc::clone(shared),
            tree: Arc::clone(tree),
            at,
            part: NO_PART,
        }
    }

    /// Returns the entry this resolved to.
    fn entry(&self) -> Entry {
        self.tree.at(self.at).copied().unwrap_or_default()
    }

    /// Returns the element's own box in screen pixels, with a region part placed inside it.
    pub(super) fn rect(&self) -> UiaRect {
        self.box_of(self.at, self.part())
    }

    /// Returns the box of the entry at `at` in screen pixels, with its scroll ancestry applied
    /// and `part` placed inside it.
    ///
    /// Automation reports screen pixels and everything above reports DIPs, so the window's
    /// origin and scale convert between them here.
    fn box_of(&self, at: u16, part: Option<Part>) -> UiaRect {
        let mut box_ = self.tree.shifted(at);
        if let Some(part) = part {
            box_ = [
                box_[0] + part.rect[0],
                box_[1] + part.rect[1],
                box_[0] + part.rect[2],
                box_[1] + part.rect[3],
            ];
        }
        let (origin, scale) = self.tree.window();
        UiaRect {
            left: f64::from(origin.x + box_[0] * scale),
            top: f64::from(origin.y + box_[1] * scale),
            width: f64::from((box_[2] - box_[0]) * scale),
            height: f64::from((box_[3] - box_[1]) * scale),
        }
    }

    /// Returns the region part this element addresses, where it addresses one.
    fn part(&self) -> Option<Part> {
        (self.part != NO_PART)
            .then(|| self.shared.regions.part(self.entry().id, self.part))
            .flatten()
    }

    /// Returns the element's accessible name, which for a part is the part's own.
    fn name(&self) -> Vec<u16> {
        match self.part() {
            Some(part) => wide(part.name),
            None => self.tree.text(self.entry().name).to_vec(),
        }
    }

    /// Returns whether a flag of the element's live state is set.
    fn flag(&self, flag: State) -> bool {
        self.tree.state(self.at).has(flag)
    }

    /// Returns whether a structural bit of the element's entry is set.
    fn bit(&self, bit: ColFlags) -> bool {
        self.entry().flags.has(bit)
    }

    /// Returns whether this element holds keyboard focus.
    fn focused(&self) -> bool {
        self.tree.focused() == packed(self.entry().id)
    }

    /// Returns whether this element's body is masked, and so publishes nothing.
    fn password(&self) -> bool {
        self.tree
            .field(self.entry().id)
            .is_some_and(|field| field.password)
    }

    /// Returns whether this element is in the content view as well as the control view.
    fn in_content(&self) -> bool {
        roles::row(self.role()).content
    }

    /// Returns the spoken name of this element's control type.
    fn localized(&self) -> &'static str {
        roles::row(self.role()).localized
    }

    /// Returns the role automation is told about, which for a part is the part's own.
    fn role(&self) -> UiaRole {
        self.part()
            .map_or_else(|| self.entry().role, |part| part.role)
    }

    /// Returns the live number for this element, or for the region part it addresses.
    ///
    /// A part reads its own producer slot, and a region prefers its producer cell over the
    /// snapshot's live word, because the producer is the thread that drew the pixels the
    /// number describes.
    fn number(&self) -> Result<f64> {
        let id = self.entry().id;
        if self.part != NO_PART {
            return self.shared.regions.value(id, self.part).ok_or_else(none);
        }
        self.shared
            .regions
            .value(id, NO_PART)
            .or_else(|| self.tree.value(self.at))
            .ok_or_else(none)
    }
}

/// Every property this stack publishes, as one row each.
///
/// A property the element does not carry answers `VT_EMPTY` rather than an error: a client
/// walks every element asking for the same twenty properties, and failing the ones that do not
/// apply turns an ordinary walk into a log of failures.
///
/// A region part answers from the same rows: its name and role come from the part, and every
/// structural property reads the region's entry, which is where a part sits.
const PROPERTIES: [(i32, fn(&At) -> VARIANT); 17] = [
    (UIA_NamePropertyId, |a| vw(&a.name())),
    (UIA_HelpTextPropertyId, |a| vw(a.tree.help(a.at))),
    (UIA_ControlTypePropertyId, |a| vi(control_type(a))),
    (UIA_LocalizedControlTypePropertyId, |a| {
        vw(&wide(a.localized()))
    }),
    (UIA_AutomationIdPropertyId, automation_id),
    (UIA_IsControlElementPropertyId, |_| vb(true)),
    (UIA_IsContentElementPropertyId, |a| vb(a.in_content())),
    (UIA_IsEnabledPropertyId, |a| vb(a.flag(S::ENABLED))),
    (UIA_IsKeyboardFocusablePropertyId, |a| {
        vb(a.bit(F::FOCUSABLE))
    }),
    (UIA_HasKeyboardFocusPropertyId, |a| vb(a.focused())),
    (UIA_IsOffscreenPropertyId, |a| vb(a.tree.clipped(a.at))),
    (UIA_IsDialogPropertyId, |a| vb(a.bit(F::DIALOG))),
    (UIA_IsPasswordPropertyId, |a| vb(a.password())),
    (UIA_LiveSettingPropertyId, |a| {
        vi(live_setting(a.entry().flags))
    }),
    (UIA_LabeledByPropertyId, labelled_by),
    (UIA_BoundingRectanglePropertyId, bounding_rectangle),
    (UIA_FrameworkIdPropertyId, |_| vw(&wide(FRAMEWORK))),
];

/// Returns the control type this element reports, resolved against its parent's role.
///
/// A button is a menu item inside a menu and a list item inside a list, because the same widget
/// is authored for either container. A popup reports a window, which makes a reader announce
/// its title before its content.
fn control_type(a: &At) -> i32 {
    let entry = a.entry();
    if a.part == NO_PART && entry.flags.has(ColFlags::DIALOG) {
        return roles::DIALOG_CONTROL_TYPE;
    }
    let parent = a
        .tree
        .at(entry.parent)
        .map_or(UiaRole::None, |parent| parent.role);
    roles::control_type_in(a.role(), parent)
}

/// Returns the automation-id segment, widened only where a client asks for it.
fn automation_id(a: &At) -> VARIANT {
    a.tree
        .key(a.at)
        .map_or_else(variant::empty, |key| variant::wide(&wide(key)))
}

/// Returns the element's bounding rectangle as a property, which is the same rectangle
/// `get_BoundingRectangle` answers with.
fn bounding_rectangle(a: &At) -> VARIANT {
    let rect = a.rect();
    variant::rect_property(&[rect.left, rect.top, rect.width, rect.height])
}

/// Returns the run this element took its name from, where it took one.
///
/// Both halves are published: `Name`, which most clients read directly, and `LabeledBy`, which
/// says where the name came from, so a reader that navigates to the label does not announce it
/// a second time as the control's own.
fn labelled_by(a: &At) -> VARIANT {
    if !a.entry().flags.has(ColFlags::LABELLED) {
        return variant::empty();
    }
    let before = a.tree.previous(a.at);
    match a.tree.at(before) {
        Some(entry) => variant::provider(&a.shared.object(entry.id, NO_PART)),
        None => variant::empty(),
    }
}

/// Returns the `LiveSetting` value for an entry's flags: 0 off, 1 polite, 2 assertive.
const fn live_setting(flags: ColFlags) -> i32 {
    if flags.has(ColFlags::LIVE_ASSERTIVE) {
        2
    } else if flags.has(ColFlags::LIVE_POLITE) {
        1
    } else {
        0
    }
}

/// Answers `id` from the property table, or `VT_EMPTY` for one this stack does not publish.
fn property(element: &Element, id: PROPERTYID) -> VARIANT {
    let (Ok(at), Some(&(_, answer))) = (
        element.at(),
        PROPERTIES.iter().find(|&&(held, _)| held == id),
    ) else {
        return variant::empty();
    };
    answer(&at)
}

/// Answers the properties the window itself carries. The root is not in the table, so it
/// shares no entry with the elements below it.
fn root_property(id: PROPERTYID) -> VARIANT {
    match id {
        UIA_ControlTypePropertyId => variant::i4(roles::DIALOG_CONTROL_TYPE),
        UIA_IsControlElementPropertyId
        | UIA_IsContentElementPropertyId
        | UIA_IsEnabledPropertyId => variant::bool(true),
        UIA_FrameworkIdPropertyId => variant::wide(&wide(FRAMEWORK)),
        _ => variant::empty(),
    }
}

/// Returns this element as the provider for `id`, where its role admits that pattern.
///
/// One object implements every pattern, so the provider for a supported pattern is the element
/// itself. `none()` marshals as `S_OK` with a null interface, which reports the pattern as
/// absent; a real error would reach the client as a failed call.
fn pattern(element: &Element, id: PATTERNID) -> Result<IUnknown> {
    let at = element.at()?;
    let wanted = roles::pattern_of(id);
    let held = match at.part() {
        // A part answers its own role's patterns, not its region's.
        Some(part) => roles::row(part.role).patterns,
        None => at.tree.patterns(at.at),
    };
    if wanted == roles::Patterns::NONE || !held.has(wanted) {
        return Err(none());
    }
    at.shared.object(at.entry().id, at.part).cast()
}

/// Returns the element one step from here in the given direction.
///
/// Index arithmetic over the published table, so every direction but `PreviousSibling` and
/// `LastChild` is a field read and those two are a walk of one parent's list. A region's parts
/// extend the table: they are the region's children and it is their parent, so a band handle is
/// an element without being an entry.
fn navigate(element: &Element, direction: NavigateDirection) -> Result<IRawElementProviderSimple> {
    let at = element.at()?;
    let entry = at.entry();
    if at.part != NO_PART {
        // A part's parent is its region and its siblings are the region's other parts; it has
        // no children of its own, because its contents are pixels.
        if direction == NavigateDirection_Parent {
            return Ok(at.shared.object(entry.id, NO_PART));
        }
        let step = at.shared.regions.with_subs(entry.id, |parts| {
            let here = parts.iter().position(|part| part.sub == at.part);
            match direction {
                NavigateDirection_NextSibling => parts.get(here? + 1).map(|part| part.sub),
                NavigateDirection_PreviousSibling => {
                    parts.get(here?.checked_sub(1)?).map(|part| part.sub)
                }
                _ => None,
            }
        });
        return Ok(at.shared.object(entry.id, step.ok_or_else(none)?));
    }
    // A region with parts has them as children rather than nothing.
    let child = at
        .shared
        .regions
        .with_subs(entry.id, |parts| match direction {
            NavigateDirection_FirstChild => parts.first().map(|part| part.sub),
            NavigateDirection_LastChild => parts.last().map(|part| part.sub),
            _ => None,
        });
    if let Some(sub) = child {
        return Ok(at.shared.object(entry.id, sub));
    }
    let step = match direction {
        // The window is the parent of a top-level element, and it is the one element that is
        // not in the table.
        NavigateDirection_Parent if entry.parent == NONE => {
            return Ok(at.shared.object(ControlId::NONE, NO_PART));
        }
        NavigateDirection_Parent => entry.parent,
        NavigateDirection_FirstChild => entry.child,
        NavigateDirection_LastChild => at.tree.last_child(at.at),
        NavigateDirection_NextSibling => entry.next,
        _ => at.tree.previous(at.at),
    };
    let found = at.tree.at(step).ok_or_else(none)?;
    Ok(at.shared.object(found.id, NO_PART))
}

/// Returns the root's own navigation: its children are the elements with no ancestry of their
/// own, which is where overlays land. An overlay is not inside the window's subtree, so it
/// becomes a child of the fragment root — the position it already holds in the hit array.
fn root_navigate(root: &Root, direction: NavigateDirection) -> Result<IRawElementProviderSimple> {
    let (shared, tree) = root.window()?;
    let step = match direction {
        NavigateDirection_FirstChild if !tree.is_empty() => 0,
        NavigateDirection_LastChild => tree.last_child(NONE),
        // The window has no parent and no sibling inside this tree.
        _ => NONE,
    };
    let found = tree.at(step).ok_or_else(none)?;
    Ok(shared.object(found.id, NO_PART))
}

/// Returns the fragment root every element answers under.
fn root_of(element: &Element) -> Result<IRawElementProviderFragmentRoot> {
    element
        .shared()?
        .object(ControlId::NONE, NO_PART)
        .cast()
        .or(Err(gone()))
}

impl Root {
    /// Returns the element at a screen point.
    ///
    /// Scans the published table, which the same walk filled as the pointer's own array, from
    /// automation's own thread.
    fn element_at(&self, x: f64, y: f64) -> Result<IRawElementProviderSimple> {
        let (shared, tree) = self.window()?;
        let (origin, scale) = tree.window();
        if scale <= 0.0 {
            return Err(none());
        }
        let point = Point {
            x: (x as f32 - origin.x) / scale,
            y: (y as f32 - origin.y) / scale,
        };
        let Some(at) = tree.hit(point) else {
            // Inside the window but on nothing: the fragment root is the answer, and a failure
            // here would make a client believe the window is not ours.
            return Ok(shared.object(ControlId::NONE, NO_PART));
        };
        // A region's parts extend the scan rather than forking it, exactly as pointer routing
        // does: the region's entry wins first, then the part inside it.
        let entry = tree.entries()[at as usize];
        let by = tree.scroll(at);
        let inside = Point {
            x: point.x + by.x - entry.box_[0],
            y: point.y + by.y - entry.box_[1],
        };
        Ok(shared.object(entry.id, shared.regions.pick(entry.id, inside)))
    }

    /// Returns the element holding keyboard focus.
    fn focused(&self) -> Result<IRawElementProviderSimple> {
        let (shared, tree) = self.window()?;
        let held = tree.focused();
        let found = tree
            .entries()
            .iter()
            .find(|entry| packed(entry.id) == held)
            .ok_or_else(none)?;
        Ok(shared.object(found.id, NO_PART))
    }

    /// Returns the window's own box, which is the extent of every published entry.
    ///
    /// The root has no entry of its own, so it reports what it contains.
    fn bounds(&self) -> Result<UiaRect> {
        let (_, tree) = self.window()?;
        let (origin, scale) = tree.window();
        let (width, height) = tree.entries().iter().fold((0.0f32, 0.0f32), |(w, h), e| {
            (w.max(e.box_[2]), h.max(e.box_[3]))
        });
        Ok(UiaRect {
            left: f64::from(origin.x),
            top: f64::from(origin.y),
            width: f64::from(width * scale),
            height: f64::from(height * scale),
        })
    }

    /// Returns the window's host provider, which is what carries the window's own properties.
    fn host(&self) -> Result<IRawElementProviderSimple> {
        let hwnd = self.shared()?.window();
        let mut provider = core::ptr::null_mut();
        // SAFETY: the handle names a window this process owns and the out-pointer is a local.
        // The call transfers one reference, which `from_raw` takes ownership of.
        unsafe {
            UiaHostProviderFromHwnd(hwnd, &raw mut provider).ok()?;
            IRawElementProviderSimple::from_raw(provider)
                .cast()
                .or(Err(none()))
        }
    }
}

// ── the vtables ─────────────────────────────────────────────────────────────────
//
// Every interface method is one line, delegating to the readers above: a vtable is a fixed
// list of names and what varies between them is which reader answers. The macro is what keeps
// each of the seventy-three to the one line it is worth, because a formatter does not reach
// inside one.

/// Emits one interface implementation per arm, verbatim.
macro_rules! vtable {
    ($($iface:ident for $on:ty { $($body:tt)* })*) => {
        $(impl crate::bindings::$iface for $on { $($body)* })*
    };
}

pub(super) use vtable;

vtable! {
    IRawElementProviderSimple_Impl for Element_Impl {
        fn ProviderOptions(&self) -> Result<ProviderOptions> { Ok(OPTIONS) }
        fn GetPatternProvider(&self, id: PATTERNID) -> Result<IUnknown> { pattern(&self.this, id) }
        fn GetPropertyValue(&self, id: PROPERTYID) -> Result<VARIANT> { Ok(property(&self.this, id)) }
        // Only the fragment root has a host: it is the window, and a child claiming one would
        // be announced as a second window.
        fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> { Err(none()) }
    }
    IRawElementProviderFragment_Impl for Element_Impl {
        fn Navigate(&self, d: NavigateDirection) -> Result<IRawElementProviderFragment> { navigate(&self.this, d)?.cast() }
        fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> { runtime_id(self.id, self.part) }
        fn get_BoundingRectangle(&self) -> Result<UiaRect> { Ok(self.at()?.rect()) }
        // Nothing here hosts a foreign fragment root, and a null array is the documented
        // answer rather than an error.
        fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> { Ok(core::ptr::null_mut()) }
        fn SetFocus(&self) -> Result<()> { self.command(Action::Focus) }
        fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> { root_of(&self.this) }
    }
    IRawElementProviderSimple_Impl for Root_Impl {
        fn ProviderOptions(&self) -> Result<ProviderOptions> { Ok(OPTIONS) }
        // The root is the window: it holds no value, no text and nothing to invoke.
        fn GetPatternProvider(&self, _: PATTERNID) -> Result<IUnknown> { Err(none()) }
        fn GetPropertyValue(&self, id: PROPERTYID) -> Result<VARIANT> { Ok(root_property(id)) }
        fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> { self.this.host() }
    }
    IRawElementProviderFragment_Impl for Root_Impl {
        fn Navigate(&self, d: NavigateDirection) -> Result<IRawElementProviderFragment> { root_navigate(&self.this, d)?.cast() }
        // The root's runtime id is the host's, which automation supplies.
        fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> { Ok(core::ptr::null_mut()) }
        fn get_BoundingRectangle(&self) -> Result<UiaRect> { self.this.bounds() }
        fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> { Ok(core::ptr::null_mut()) }
        // The window takes focus through the system, not through a queued command.
        fn SetFocus(&self) -> Result<()> { Ok(()) }
        fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> { root_of(&self.this) }
    }
    IRawElementProviderFragmentRoot_Impl for Root_Impl {
        fn ElementProviderFromPoint(&self, x: f64, y: f64) -> Result<IRawElementProviderFragment> { self.this.element_at(x, y)?.cast() }
        fn GetFocus(&self) -> Result<IRawElementProviderFragment> { self.this.focused()?.cast() }
    }
    IInvokeProvider_Impl for Element_Impl {
        fn Invoke(&self) -> Result<()> { self.command(Action::Invoke) }
    }
    IScrollItemProvider_Impl for Element_Impl {
        fn ScrollIntoView(&self) -> Result<()> { self.command(Action::Reveal) }
    }
    IToggleProvider_Impl for Element_Impl {
        fn Toggle(&self) -> Result<()> { self.command(Action::Toggle) }
        fn ToggleState(&self) -> Result<ToggleState> { toggle_state(&self.this) }
    }
    IExpandCollapseProvider_Impl for Element_Impl {
        fn Expand(&self) -> Result<()> { self.command(|id| Action::Expand(id, true)) }
        fn Collapse(&self) -> Result<()> { self.command(|id| Action::Expand(id, false)) }
        fn ExpandCollapseState(&self) -> Result<ExpandCollapseState> { expand_state(&self.this) }
    }
    ISelectionItemProvider_Impl for Element_Impl {
        fn Select(&self) -> Result<()> { self.command(Action::Select) }
        // Selection here is single: adding to it is selecting, and no state has nothing
        // selected, so there is nothing to remove.
        fn AddToSelection(&self) -> Result<()> { self.command(Action::Select) }
        fn RemoveFromSelection(&self) -> Result<()> { Err(invalid()) }
        fn IsSelected(&self) -> Result<BOOL> { Ok(BOOL::from(self.at()?.flag(S::SELECTED))) }
        fn SelectionContainer(&self) -> Result<IRawElementProviderSimple> { container(&self.this) }
    }
    ISelectionProvider_Impl for Element_Impl {
        fn GetSelection(&self) -> Result<*mut SAFEARRAY> { selection(&self.this) }
        // A container never holds more than one selected item.
        fn CanSelectMultiple(&self) -> Result<BOOL> { Ok(BOOL::from(false)) }
        fn IsSelectionRequired(&self) -> Result<BOOL> { Ok(BOOL::from(false)) }
    }
    IRangeValueProvider_Impl for Element_Impl {
        fn SetValue(&self, value: f64) -> Result<()> { set_number(&self.this, value) }
        fn Value(&self) -> Result<f64> { self.at()?.number() }
        fn IsReadOnly(&self) -> Result<BOOL> { Ok(BOOL::from(!self.at()?.flag(S::ENABLED))) }
        fn Maximum(&self) -> Result<f64> { Ok(bounds(&self.this)?.max) }
        fn Minimum(&self) -> Result<f64> { Ok(bounds(&self.this)?.min) }
        fn LargeChange(&self) -> Result<f64> { let r = bounds(&self.this)?; Ok((r.max - r.min) * LARGE_FRACTION) }
        fn SmallChange(&self) -> Result<f64> { let r = bounds(&self.this)?; Ok(small_change(r)) }
    }
    IValueProvider_Impl for Element_Impl {
        fn SetValue(&self, value: &PCWSTR) -> Result<()> { set_text(&self.this, value) }
        fn Value(&self) -> Result<BSTR> { value_text(&self.this) }
        fn IsReadOnly(&self) -> Result<BOOL> { Ok(BOOL::from(read_only(&self.this)?)) }
    }
}

// ── the readers the vtables delegate to ─────────────────────────────────────────

/// Returns whether the element reports itself toggled.
fn toggle_state(element: &Element) -> Result<ToggleState> {
    let on = element.at()?.flag(S::TOGGLED);
    Ok(if on { ToggleState_On } else { ToggleState_Off })
}

/// Returns whether the element's flyout is open, or that it owns none.
fn expand_state(element: &Element) -> Result<ExpandCollapseState> {
    let at = element.at()?;
    Ok(if !at.bit(F::EXPANDS) {
        ExpandCollapseState_LeafNode
    } else if at.flag(S::EXPANDED) {
        ExpandCollapseState_Expanded
    } else {
        ExpandCollapseState_Collapsed
    })
}

/// Returns the container whose selection this element is one of.
fn container(element: &Element) -> Result<IRawElementProviderSimple> {
    let at = element.at()?;
    let parent = at.tree.at(at.entry().parent).ok_or_else(none)?;
    Ok(at.shared.object(parent.id, NO_PART))
}

/// Returns the container's selected children, read from the live column so a selection change
/// needs no republish.
fn selection(element: &Element) -> Result<*mut SAFEARRAY> {
    let at = element.at()?;
    let selected: Vec<_> = children(&at.tree, at.at)
        .filter(|&child| at.tree.state(child).has(State::SELECTED))
        .map(|child| {
            at.shared
                .object(at.tree.entries()[child as usize].id, NO_PART)
        })
        .collect();
    Ok(variant::provider_array(&selected))
}

/// Returns each child index of the entry at `at`, in sibling order.
fn children(tree: &Tree, at: u16) -> impl Iterator<Item = u16> + '_ {
    let mut next = tree.at(at).map_or(NONE, |entry| entry.child);
    core::iter::from_fn(move || {
        let out = (next != NONE).then_some(next)?;
        next = tree.at(out).map_or(NONE, |entry| entry.next);
        Some(out)
    })
}

/// Returns the element's numeric bounds, or the empty error where it carries none.
fn bounds(element: &Element) -> Result<Range> {
    let at = element.at()?;
    at.tree.range(at.at).ok_or_else(none)
}

/// Returns the keyboard increment a client offers, which is the control's own step where it
/// declares one.
fn small_change(range: Range) -> f64 {
    if range.step > 0.0 {
        range.step
    } else {
        (range.max - range.min) * SMALL_FRACTION
    }
}

/// Queues a numeric write, refusing one outside the element's own bounds.
fn set_number(element: &Element, value: f64) -> Result<()> {
    let at = element.at()?;
    let range = at.tree.range(at.at).ok_or_else(invalid)?;
    if !value.is_finite() || value < range.min || value > range.max {
        return Err(invalid());
    }
    element.command(|id| Action::SetValue(id, value))
}

/// Queues a string write: a document edit for an editable body, a parsed number otherwise.
///
/// A write the control cannot hold is refused rather than accepted and dropped, so a client
/// cannot read the old value back after an `S_OK`; a text-valued element is read-only through
/// this pattern and publishes `TextPattern` instead.
fn set_text(element: &Element, value: &PCWSTR) -> Result<()> {
    let at = element.at()?;
    if !at.flag(S::ENABLED) {
        return Err(disabled());
    }
    if value.is_null() {
        return Err(invalid());
    }
    // SAFETY: the `IValueProvider::SetValue` ABI passes a null-terminated wide string owned by
    // automation and valid for the duration of the call.
    let text = unsafe { value.to_string() }.map_err(|_| invalid())?;
    if let Some(field) = at.tree.field(element.id) {
        let edit = TextAction::Replace(element.id, field.revision, text.encode_utf16().collect());
        at.shared.edit(edit);
        return Ok(());
    }
    let range = at.tree.range(at.at).ok_or_else(invalid)?;
    let parsed: f64 = text.trim().parse().map_err(|_| invalid())?;
    if !parsed.is_finite() || parsed < range.min || parsed > range.max {
        return Err(invalid());
    }
    at.shared.act(Action::SetValue(element.id, parsed));
    Ok(())
}

/// Returns the element's value as a string.
///
/// A password field publishes no body, so its value reads as absent rather than as the
/// characters behind the mask. A text element answers with its own body, and a numeric one
/// formats to the precision its step implies, so the announced value carries no float noise.
fn value_text(element: &Element) -> Result<BSTR> {
    let at = element.at()?;
    if let Some(field) = at.tree.field(element.id) {
        if field.password {
            return Err(invalid());
        }
        return Ok(variant::bstr(&field.text));
    }
    if let Some(range) = at.tree.range(at.at) {
        return Ok(BSTR::from(formatted(at.number()?, range.step)));
    }
    Ok(variant::bstr(&at.name()))
}

/// Returns whether the element refuses a write through the value pattern.
fn read_only(element: &Element) -> Result<bool> {
    let at = element.at()?;
    let editable = at.bit(F::RANGED) || at.bit(F::FIELD);
    Ok(!editable || !at.flag(S::ENABLED))
}

// ── helpers ─────────────────────────────────────────────────────────────────────

/// Returns a runtime id for one element identity, or a failure where the allocation failed.
fn runtime_id(id: ControlId, part: u32) -> Result<*mut SAFEARRAY> {
    let array = variant::runtime_id(id.index() as u32, part);
    if array.is_null() {
        return Err(Error::from_hresult(windows_core::HRESULT(
            0x8007_000Eu32 as i32,
        )));
    }
    Ok(array)
}

/// Packs a [`ControlId`] into one `u64`, index above generation.
///
/// Focus is a single atomic word and still names a generational id, so it cannot be mistaken
/// for a reused index after a republish.
#[must_use]
pub fn packed(id: ControlId) -> u64 {
    ((id.index() as u64) << 32) | u64::from(id.generation())
}

/// Formats `value` at the number of decimal places `step` implies.
///
/// A `step` of zero or less marks a continuous range and formats to two places. The rounding a
/// control applies when drawing does not reach the announced string, so it is applied here.
fn formatted(value: f64, step: f64) -> String {
    // The fewest decimal places that express `step` exactly, capped at six where the comparison
    // stops discriminating in an f64.
    let places = if step <= 0.0 {
        2
    } else {
        (0..=6)
            .find(|places| {
                let scaled = step * 10f64.powi(*places);
                (scaled - scaled.round()).abs() < 1.0e-6
            })
            .unwrap_or(6) as usize
    };
    format!("{value:.places$}")
}

/// Returns `text`'s UTF-16 units, which is what every automation string is.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// Returns `UIA_E_ELEMENTNOTAVAILABLE`, the answer for an element that has unmounted or a
/// window that has gone.
pub fn gone() -> Error {
    Error::from_hresult(windows_core::HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32))
}

/// Returns the empty error, which marshals as `S_OK` with a null out-parameter: nothing is
/// there, and the call did not fail.
pub fn none() -> Error {
    Error::empty()
}

/// Returns `UIA_E_ELEMENTNOTENABLED`, the answer to a command on a disabled element.
fn disabled() -> Error {
    Error::from_hresult(windows_core::HRESULT(UIA_E_ELEMENTNOTENABLED as i32))
}

/// Returns `UIA_E_INVALIDOPERATION`, the answer to a value or operation the control cannot
/// hold, and to a write on an element that accepts none.
fn invalid() -> Error {
    Error::from_hresult(windows_core::HRESULT(UIA_E_INVALIDOPERATION as i32))
}

/// Returns the object a raised event names, or `None` when the element is not in the published
/// snapshot.
pub fn provider_for(shared: &Arc<Shared>, id: ControlId) -> Option<IRawElementProviderSimple> {
    if !id.is_none() {
        tree_of(shared).index_of(id)?;
    }
    Some(shared.object(id, NO_PART))
}

/// Tells automation the window's providers are finished with, and drops every minted object.
///
/// `UiaReturnRawElementProvider(hwnd, 0, 0, NULL)` is the documented way to say it, and it is
/// not the same as our own references going away: automation caches per window, so without
/// this it keeps that cache — and a client keeps a window element — for a window that no
/// longer exists.
///
/// Must be called while the window handle is still valid, which puts it on `WM_DESTROY` rather
/// than on a drop.
pub fn disconnect(shared: &Arc<Shared>) {
    let hwnd = shared.window();
    if !hwnd.is_null() && shared.asked.swap(false, Relaxed) {
        // SAFETY: the handle names a window this process owns and has not yet destroyed, and a
        // null provider is the documented argument for releasing its cache.
        unsafe { _ = UiaReturnRawElementProvider(hwnd, 0, 0, core::ptr::null_mut()) }
    }
    shared.forget();
}

/// Answers `WM_GETOBJECT` with the fragment root, or `None` when `l` names another object.
///
/// The only automation call that arrives on the pump, and it does nothing but hand back an
/// object. Everything a client asks afterwards is answered off this thread.
pub fn get_object(shared: &Arc<Shared>, w: WPARAM, l: LPARAM) -> Option<LRESULT> {
    if l as i32 != ROOT_OBJECT_ID {
        return None;
    }
    // The latch transition is what asks for a tick: a window that is not laid out again never
    // publishes on its own, and the client that just attached would walk nothing.
    if !shared.asked.swap(true, Relaxed) {
        shared.wake();
    }
    let object = shared.object(ControlId::NONE, NO_PART);
    // SAFETY: the handle names a window this process owns, and `object` holds a reference for
    // the whole call — `UiaReturnRawElementProvider` takes its own.
    Some(unsafe { UiaReturnRawElementProvider(shared.window(), w, l, object.as_raw()) })
}
