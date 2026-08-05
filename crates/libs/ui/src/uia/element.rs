//! The provider object, and the state every provider reads.
//!
//! One object per element identity, carrying a weak reference to [`Shared`] and two ids. It
//! holds no tree pointer and caches nothing: every call reads the currently published tree,
//! from whatever thread automation chose to call on. With nothing cached there is nothing
//! to invalidate, and no call hops onto the window's pump to read a field.
//!
//! Providers built by `implement_decl!` are agile, so a client may call one from any
//! apartment. Everything reachable from here is either immutable or an atomic.

use super::action::{Action, Queue};
use super::live::State;
use super::roles::{self, Patterns};
use super::slot::{Regions, Slot};
use super::tree::{ColFlags, Part, Tree};
use super::variant;
use crate::bindings::{
    HWND, IExpandCollapseProvider, IInvokeProvider, IRangeValueProvider,
    IRawElementProviderFragment, IRawElementProviderFragmentRoot, IRawElementProviderSimple,
    LPARAM, LRESULT, NavigateDirection, NavigateDirection_FirstChild, NavigateDirection_LastChild,
    NavigateDirection_NextSibling, NavigateDirection_Parent, NavigateDirection_PreviousSibling,
    PATTERNID, PROPERTYID, PostMessageW, ProviderOptions, ProviderOptions_RefuseNonClientSupport,
    ProviderOptions_ServerSideProvider, SAFEARRAY, UIA_AutomationIdPropertyId,
    UIA_ControlTypePropertyId, UIA_E_ELEMENTNOTAVAILABLE, UIA_FrameworkIdPropertyId,
    UIA_HasKeyboardFocusPropertyId, UIA_HelpTextPropertyId, UIA_IsContentElementPropertyId,
    UIA_IsControlElementPropertyId, UIA_IsDialogPropertyId, UIA_IsEnabledPropertyId,
    UIA_IsKeyboardFocusablePropertyId, UIA_IsOffscreenPropertyId, UIA_LabeledByPropertyId,
    UIA_LiveSettingPropertyId, UIA_LocalizedControlTypePropertyId, UIA_NamePropertyId,
    UiaHostProviderFromHwnd, UiaRect, UiaReturnRawElementProvider, VARIANT, WPARAM,
};
use crate::bindings::{
    IScrollItemProvider, ISelectionItemProvider, ISelectionProvider, ITextProvider,
    IToggleProvider, IValueProvider,
};
use crate::widget::UiaRole;
use core::sync::atomic::{AtomicBool, AtomicIsize, Ordering::Relaxed};
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::sync::{Arc, Mutex, Weak};
use windows_core::{Error, HRESULT, IUnknown, Interface, Result, implement_decl};
use windows_scene::{ContactKind, ControlId, NO_ENTRY, Point};

/// The provider options this stack advertises.
///
/// `ServerSideProvider`, because the control implements its own provider. Automation
/// broadcasts events raised from a server-side provider and keeps a client-side one inside
/// the client process, so claiming `ClientSideProvider` here would lose every raised event.
///
/// `UseComThreading` is not set. It spares a provider from being thread-safe by funnelling
/// every query through the window's own pump, which is the path along which a slow provider
/// blocks a screen reader. Every provider here is thread-safe already.
///
/// `RefuseNonClientSupport`, because the window draws its own caption and publishes those
/// buttons as ordinary elements. Without it the system contributes a second set.
const OPTIONS: ProviderOptions =
    ProviderOptions_ServerSideProvider | ProviderOptions_RefuseNonClientSupport;

const NOT_AVAILABLE: HRESULT = HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32);
const OUT_OF_MEMORY: HRESULT = HRESULT(0x8007_000Eu32 as i32);
/// `UiaRootObjectId`. A `WM_GETOBJECT` naming any other object id is not ours to answer.
const ROOT_OBJECT_ID: i32 = -25;

/// The part id of an element that is a real entry rather than one of a region's parts.
pub const NO_PART: u32 = u32::MAX;

/// Everything a provider can reach, and the only state shared across threads.
///
/// Held by the front thread's [`Uia`](super::Uia) as an `Arc` and by every provider as a
/// `Weak`, so objects a client still holds cannot keep a closed window's state alive. They
/// stop resolving instead, which is what `UIA_E_ELEMENTNOTAVAILABLE` reports.
#[derive(Debug, Default)]
pub struct Shared {
    pub slot: Slot,
    /// What presentation regions declare — parts and producer-owned values. Held beside
    /// the tree rather than in it, so a band drag republishes no element.
    pub regions: Regions,
    pub actions: Queue,
    /// Latched by the first `WM_GETOBJECT` and cleared only by [`disconnect`].
    /// `UiaClientsAreListening` is a hint; having been asked for a provider is not.
    pub queried: AtomicBool,
    hwnd: AtomicIsize,
    /// One object per element identity, so an element always answers as the same object.
    /// Automation matches a raised event to a listener by object identity.
    providers: Mutex<FxHashMap<(ControlId, u32), SendProvider>>,
}

/// An agile provider: callable and reference-countable from any apartment.
#[derive(Debug)]
struct SendProvider(IRawElementProviderSimple);

// SAFETY: `implement_decl!` objects answer `IAgileObject`/`IMarshal`, so a single instance
// is safely shared across automation's worker threads.
unsafe impl Send for SendProvider {}

impl Shared {
    /// Records the window every provider answers for.
    pub fn attach(&self, hwnd: HWND) {
        self.hwnd.store(hwnd as isize, Relaxed);
    }

    /// Returns the attached window, or null before [`attach`](Self::attach) has run.
    #[must_use]
    pub fn window(&self) -> HWND {
        self.hwnd.load(Relaxed) as HWND
    }

    /// Queues an action and asks the front thread for a tick.
    ///
    /// Posts the pacer's own message, so the tick that runs the action is the tick that
    /// services input, draining in the same order and publishing the same way. Only the
    /// first action of a batch posts, because the drain takes the whole queue.
    pub fn act(&self, action: Action) {
        if self.actions.push(action) {
            self.wake();
        }
    }

    /// Asks the front thread for a tick.
    pub fn wake(&self) {
        let hwnd = self.window();
        if hwnd.is_null() {
            return;
        }
        // SAFETY: `PostMessageW` is callable from any thread and validates the handle
        // itself, so a handle whose window has gone fails the call rather than faulting —
        // which is why the result is dropped.
        unsafe {
            _ = PostMessageW(hwnd, windows_window::WM_FRAME, 0, 0);
        }
    }

    /// Returns the stable object for one element identity, minting it on the first ask.
    fn provider(self: &Arc<Self>, id: ControlId, part: u32) -> IRawElementProviderSimple {
        let mut providers = self.providers.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(found) = providers.get(&(id, part)) {
            return found.0.clone();
        }
        let element = Element {
            shared: Arc::downgrade(self),
            id,
            part,
        };
        let object: IRawElementProviderSimple = if id.is_none() {
            Root(element).into()
        } else {
            element.into()
        };
        providers.insert((id, part), SendProvider(object.clone()));
        object
    }

    /// Drops the objects for elements the new tree no longer has.
    ///
    /// A stale provider is already correct without this, because an id that no longer
    /// resolves answers `UIA_E_ELEMENTNOTAVAILABLE`. What this bounds is the size of the
    /// map across a session that mounts and unmounts.
    pub fn evict(&self, tree: &Tree) {
        self.providers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|&(id, _), _| id.is_none() || tree.index_of(id).is_some());
    }

    /// Drops every provider object minted for this window.
    pub fn forget(&self) {
        self.providers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
}

thread_local! {
    /// This thread's own reference to the published tree, refreshed only when it moves.
    static SEEN: RefCell<Option<(u64, Arc<Tree>)>> = const { RefCell::new(None) };
}

/// One element: a weak reference to [`Shared`], and the two ids that name it.
pub struct Element {
    shared: Weak<Shared>,
    /// [`ControlId::NONE`] is the fragment root, which is the window and is not in the
    /// array. No minted control can be it: ids start at generation one.
    id: ControlId,
    /// Which part of a presentation region, or [`NO_PART`].
    part: u32,
}

/// The same element with the fragment-root interfaces added. A separate type because only
/// the root answers element-from-point and focus.
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

/// A resolved element: the tree it lives in, and where in it.
pub struct At {
    pub shared: Arc<Shared>,
    pub tree: Arc<Tree>,
    pub at: usize,
    pub part: u32,
}

impl Element {
    pub fn id(&self) -> ControlId {
        self.id
    }

    pub fn shared(&self) -> Result<Arc<Shared>> {
        self.shared.upgrade().ok_or_else(gone)
    }

    /// Returns the shared state and the tree as this thread currently sees it.
    ///
    /// What the root reads: it is the window, so it is not in the entry array and has no
    /// index to resolve.
    pub fn window(&self) -> Result<(Arc<Shared>, Arc<Tree>)> {
        let shared = self.shared()?;
        let tree = SEEN.with(|seen| shared.slot.read(seen));
        Ok((shared, tree))
    }

    /// Resolves the element against the current tree.
    ///
    /// # Errors
    ///
    /// `UIA_E_ELEMENTNOTAVAILABLE` once the element has unmounted or the window has gone.
    /// The identity is a generational id rather than a cached node pointer, so an unmounted
    /// element has nothing to dangle and no registration to forget.
    pub fn at(&self) -> Result<At> {
        let (shared, tree) = self.window()?;
        let at = tree.index_of(self.id).ok_or_else(gone)?;
        Ok(At {
            shared,
            tree,
            at,
            part: self.part,
        })
    }

    fn is_root(&self) -> bool {
        self.id.is_none()
    }

    /// Returns the element's own box in screen pixels, with its scroll ancestry applied.
    ///
    /// Automation reports screen pixels and everything above reports DIPs, so the window's
    /// origin and scale are read from the live half here. The root has no entry of its own
    /// and reports the extent of every published entry instead.
    fn bounds(&self) -> Result<UiaRect> {
        let (tree, rect) = if self.is_root() {
            let (_, tree) = self.window()?;
            let width = tree.entries().iter().fold(0.0f32, |w, e| w.max(e.x1));
            let height = tree.entries().iter().fold(0.0f32, |h, e| h.max(e.y1));
            (tree, (0.0, 0.0, width, height))
        } else {
            let At {
                shared,
                tree,
                at,
                part,
            } = self.at()?;
            let entry = tree.entry(at).ok_or_else(gone)?;
            let offset = tree.live.scroll(entry.scroll_src);
            let (x0, y0) = (entry.x0 - offset.x, entry.y0 - offset.y);
            // A part is region-local, so it is placed inside its region's own box.
            let rect = with_part(&shared, self.id, part, |part| {
                (
                    x0 + part.rect.0,
                    y0 + part.rect.1,
                    x0 + part.rect.2,
                    y0 + part.rect.3,
                )
            })
            .unwrap_or((x0, y0, entry.x1 - offset.x, entry.y1 - offset.y));
            (tree, rect)
        };
        let (origin, scale) = tree.live.window();
        Ok(UiaRect {
            left: f64::from(origin.x + rect.0 * scale),
            top: f64::from(origin.y + rect.1 * scale),
            width: f64::from((rect.2 - rect.0) * scale),
            height: f64::from((rect.3 - rect.1) * scale),
        })
    }

    /// Returns whether a clipping ancestor excludes the element's own box.
    ///
    /// Walks the clip chain rather than testing against the window: an element scrolled out
    /// of a list is offscreen, and one below the fold of a window is not.
    fn offscreen(&self, tree: &Tree, at: usize) -> bool {
        let Some(entry) = tree.entry(at) else {
            return true;
        };
        let offset = tree.live.scroll(entry.scroll_src);
        let (x0, y0) = (entry.x0 - offset.x, entry.y0 - offset.y);
        let (x1, y1) = (entry.x1 - offset.x, entry.y1 - offset.y);
        let mut clip = entry.clip_parent;
        let mut guard = tree.len();
        while clip != NO_ENTRY && guard > 0 {
            let Some(bound) = tree.entry(clip as usize) else {
                break;
            };
            let by = tree.live.scroll(bound.scroll_src);
            let (bx0, by0) = (bound.x0 - by.x, bound.y0 - by.y);
            let (bx1, by1) = (bound.x1 - by.x, bound.y1 - by.y);
            if x1 <= bx0 || x0 >= bx1 || y1 <= by0 || y0 >= by1 {
                return true;
            }
            clip = bound.clip_parent;
            guard -= 1;
        }
        false
    }

    /// Answers every property this stack publishes.
    ///
    /// A property the element does not carry answers `VT_EMPTY` rather than an error: a
    /// client walks every element asking for the same twenty properties, and failing the
    /// ones that do not apply turns an ordinary walk into a log of failures.
    fn property(&self, id: PROPERTYID) -> VARIANT {
        if self.is_root() {
            return match id {
                _ if id == UIA_ControlTypePropertyId => variant::i4(roles::DIALOG_CONTROL_TYPE),
                _ if id == UIA_IsControlElementPropertyId
                    || id == UIA_IsContentElementPropertyId
                    || id == UIA_IsEnabledPropertyId =>
                {
                    variant::bool(true)
                }
                _ if id == UIA_FrameworkIdPropertyId => variant::wide(&wide(FRAMEWORK)),
                _ => variant::empty(),
            };
        }
        let Ok(At {
            shared,
            tree,
            at,
            part,
        }) = self.at()
        else {
            return variant::empty();
        };
        // A part carries a name and a role only, so it answers no structural property.
        if let Some(answer) = with_part(&shared, self.id, part, |part| {
            let row = roles::row(part.role);
            match id {
                _ if id == UIA_NamePropertyId => variant::wide(&wide(part.name)),
                _ if id == UIA_ControlTypePropertyId => variant::i4(row.control_type),
                _ if id == UIA_LocalizedControlTypePropertyId => {
                    variant::wide(&wide(row.localized))
                }
                _ if id == UIA_IsControlElementPropertyId
                    || id == UIA_IsContentElementPropertyId
                    || id == UIA_IsEnabledPropertyId
                    || id == UIA_IsKeyboardFocusablePropertyId =>
                {
                    variant::bool(true)
                }
                _ => variant::empty(),
            }
        }) {
            return answer;
        }
        let Some(col) = tree.col(at) else {
            return variant::empty();
        };
        let parent = tree
            .col(col.parent as usize)
            .map_or(UiaRole::None, |up| up.role);
        match id {
            _ if id == UIA_NamePropertyId => variant::wide(tree.text(col.name)),
            _ if id == UIA_HelpTextPropertyId => variant::wide(tree.text(col.help)),
            _ if id == UIA_LabeledByPropertyId => match tree.entry(col.labelled_by as usize) {
                Some(label) => variant::provider(&shared.provider(label.id, NO_PART)),
                None => variant::empty(),
            },
            _ if id == UIA_ControlTypePropertyId => {
                variant::i4(roles::control_type_in(col.role, parent))
            }
            _ if id == UIA_LocalizedControlTypePropertyId => {
                variant::wide(&wide(roles::row(col.role).localized))
            }
            _ if id == UIA_AutomationIdPropertyId => {
                // The wide string is built here and nowhere else: the column keeps a
                // `&'static str`, and this runs only when a client asks for the id.
                col.key
                    .map_or_else(variant::empty, |key| variant::wide(&wide(key)))
            }
            _ if id == UIA_IsControlElementPropertyId => variant::bool(true),
            _ if id == UIA_IsContentElementPropertyId => {
                variant::bool(roles::row(col.role).content)
            }
            _ if id == UIA_IsEnabledPropertyId => {
                variant::bool(tree.live.state(at).has(State::ENABLED))
            }
            _ if id == UIA_IsKeyboardFocusablePropertyId => {
                variant::bool(col.flags.has(ColFlags::FOCUSABLE))
            }
            _ if id == UIA_HasKeyboardFocusPropertyId => {
                variant::bool(tree.live.focused() == packed(self.id))
            }
            _ if id == UIA_IsOffscreenPropertyId => variant::bool(self.offscreen(&tree, at)),
            _ if id == UIA_IsDialogPropertyId => variant::bool(col.flags.has(ColFlags::DIALOG)),
            _ if id == UIA_LiveSettingPropertyId => variant::i4(live_setting(col.flags)),
            _ if id == UIA_FrameworkIdPropertyId => variant::wide(&wide(FRAMEWORK)),
            _ => variant::empty(),
        }
    }

    /// Returns the patterns this element answers.
    fn patterns(&self) -> Patterns {
        let Ok(At {
            shared,
            tree,
            at,
            part,
        }) = self.at()
        else {
            return Patterns::NONE;
        };
        // A part answers its own role's patterns, not its region's.
        with_part(&shared, self.id, part, |part| {
            roles::row(part.role).patterns
        })
        .unwrap_or_else(|| tree.patterns(at))
    }

    fn pattern(&self, id: PATTERNID) -> Result<IUnknown> {
        let wanted = roles::pattern_of(id);
        if wanted != Patterns::NONE && self.patterns().has(wanted) {
            // One object implements every pattern, so the provider for a supported pattern
            // is this element itself.
            return self.object()?.cast();
        }
        // `none()` marshals as `S_OK` with a null interface, which reports the pattern as
        // absent; a real error would reach the client as a failed call.
        Err(none())
    }

    /// Returns this element's own stable provider object.
    fn object(&self) -> Result<IRawElementProviderSimple> {
        Ok(self.shared()?.provider(self.id, self.part))
    }

    fn runtime_id(&self) -> Result<*mut SAFEARRAY> {
        let array = variant::runtime_id(self.id.index() as u32, self.part);
        if array.is_null() {
            return Err(Error::from_hresult(OUT_OF_MEMORY));
        }
        Ok(array)
    }

    fn host(&self) -> Result<IRawElementProviderSimple> {
        // Only the fragment root has a host: it is the window, and a child claiming one
        // would be announced as a second window.
        if !self.is_root() {
            return Err(none());
        }
        let hwnd = self.shared()?.window();
        let mut provider = core::ptr::null_mut();
        // SAFETY: the handle names a window this process owns and the out-pointer is a
        // local. The call transfers one reference, which `from_raw` takes ownership of.
        unsafe {
            UiaHostProviderFromHwnd(hwnd, &raw mut provider).ok()?;
            IRawElementProviderSimple::from_raw(provider)
                .cast()
                .or(Err(none()))
        }
    }

    fn set_focus(&self) -> Result<()> {
        self.shared()?.act(Action::Focus(self.id));
        Ok(())
    }

    fn fragment_root(&self) -> Result<IRawElementProviderFragmentRoot> {
        self.shared()?
            .provider(ControlId::NONE, NO_PART)
            .cast()
            .or(Err(gone()))
    }

    /// Returns the element one step from here in the given direction.
    ///
    /// Reads the columns the build pass filled, so every direction but `PreviousSibling` is
    /// a field read. A region's parts extend the tree: they are the region's children and
    /// it is their parent, so a band handle is an element without being an entry.
    fn navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        let (shared, tree) = self.window()?;
        if self.is_root() {
            // The root's children are the elements with no ancestry of their own, which is
            // where overlays land: an overlay is not inside the window's subtree, so it
            // becomes a child of the fragment root — the position it already holds in the
            // hit array.
            let (first, last) = tree.roots();
            let to = match direction {
                _ if direction == NavigateDirection_FirstChild => index(first),
                _ if direction == NavigateDirection_LastChild => index(last),
                _ => None,
            };
            return fragment(&shared, &tree, to);
        }

        let at = tree.index_of(self.id).ok_or_else(gone)?;

        if self.part != NO_PART {
            // A part's parent is its region and its siblings are the region's other parts.
            // It has no children of its own.
            let step = shared.regions.with_parts(self.id, |parts| {
                let found = parts.iter().position(|part| part.sub == self.part);
                let sibling = |to: Option<usize>| Some(parts.get(to?)?.sub);
                match direction {
                    _ if direction == NavigateDirection_Parent => Some(NO_PART),
                    _ if direction == NavigateDirection_NextSibling => {
                        sibling(found.map(|f| f + 1))
                    }
                    _ if direction == NavigateDirection_PreviousSibling => {
                        sibling(found.and_then(|f| f.checked_sub(1)))
                    }
                    _ => None,
                }
            });
            return shared
                .provider(self.id, step.ok_or_else(none)?)
                .cast()
                .or(Err(none()));
        }

        let col = tree.col(at).ok_or_else(gone)?;
        // A region with parts has them as children rather than nothing.
        let child = shared.regions.with_parts(self.id, |parts| match direction {
            _ if direction == NavigateDirection_FirstChild => parts.first().map(|part| part.sub),
            _ if direction == NavigateDirection_LastChild => parts.last().map(|part| part.sub),
            _ => None,
        });
        if let Some(sub) = child {
            return shared.provider(self.id, sub).cast().or(Err(none()));
        }

        let to = match direction {
            _ if direction == NavigateDirection_Parent => {
                if col.parent == NO_ENTRY {
                    // The window is the parent of a top-level element, and it is the one
                    // element that is not in the array.
                    return shared
                        .provider(ControlId::NONE, NO_PART)
                        .cast()
                        .or(Err(none()));
                }
                index(col.parent)
            }
            _ if direction == NavigateDirection_FirstChild => index(col.first_child),
            _ if direction == NavigateDirection_LastChild => index(col.last_child),
            _ if direction == NavigateDirection_NextSibling => index(col.next_sibling),
            _ if direction == NavigateDirection_PreviousSibling => previous(&tree, col.parent, at),
            _ => None,
        };
        fragment(&shared, &tree, to)
    }
}

impl Root {
    /// Returns the element at a screen point.
    ///
    /// Runs the same scan pointer routing runs, over the same entries, from automation's
    /// own thread. Hit-testing has one implementation, so the two cannot disagree.
    fn element_at(&self, x: f64, y: f64) -> Result<IRawElementProviderFragment> {
        let (shared, tree) = self.window()?;
        let (origin, scale) = tree.live.window();
        if scale <= 0.0 {
            return Err(none());
        }
        let p = Point {
            x: (x as f32 - origin.x) / scale,
            y: (y as f32 - origin.y) / scale,
        };
        let found = windows_scene::scan(
            tree.entries(),
            |node| tree.live.scroll(node),
            0,
            p,
            ContactKind::Mouse,
        );
        let Some((at, local)) = found else {
            // Inside the window but on nothing: the fragment root is the answer, and a
            // failure here would make a client believe the window is not ours.
            return shared
                .provider(ControlId::NONE, NO_PART)
                .cast()
                .or(Err(none()));
        };
        let entry = tree.entry(at).ok_or_else(gone)?;
        // A region's parts extend the scan rather than forking it, exactly as pointer
        // routing does: the region's entry wins first, then the part inside it.
        let (px, py) = (local.x - entry.x0, local.y - entry.y0);
        let part = shared.regions.with_parts(entry.id, |parts| {
            parts
                .iter()
                .find(|part| {
                    px >= part.rect.0 && px <= part.rect.2 && py >= part.rect.1 && py <= part.rect.3
                })
                .map_or(NO_PART, |part| part.sub)
        });
        shared.provider(entry.id, part).cast().or(Err(none()))
    }

    fn focused(&self) -> Result<IRawElementProviderFragment> {
        let (shared, tree) = self.window()?;
        let focused = tree.live.focused();
        let at = tree
            .entries()
            .iter()
            .position(|entry| packed(entry.id) == focused)
            .ok_or_else(none)?;
        fragment(&shared, &tree, Some(at))
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────────

const FRAMEWORK: &str = "windows-ui";

/// Packs a [`ControlId`] into one `u64`, index above generation.
///
/// Focus is a single atomic word and still names a generational id, so it cannot be
/// mistaken for a reused index after a republish.
#[must_use]
pub fn packed(id: ControlId) -> u64 {
    (id.index() as u64) << 32 | u64::from(id.generation())
}

fn fragment(
    shared: &Arc<Shared>,
    tree: &Tree,
    at: Option<usize>,
) -> Result<IRawElementProviderFragment> {
    let entry = at.and_then(|at| tree.entry(at)).ok_or_else(none)?;
    shared.provider(entry.id, NO_PART).cast().or(Err(none()))
}

/// Returns the sibling before `at`, by walking its parent's child list.
///
/// The one direction the columns do not carry: storing it would cost four bytes an element
/// to save a walk over one parent's list.
fn previous(tree: &Tree, parent: u32, at: usize) -> Option<usize> {
    // The parentless elements are the window's children and their list head is the tree's
    // own, so a top-level element needs no separate case here.
    let mut next = match tree.col(parent as usize) {
        Some(col) => col.first_child,
        None => tree.roots().0,
    };
    let mut before = None;
    while let Some(step) = index(next) {
        if step == at {
            return before;
        }
        before = Some(step);
        next = tree.col(step).map_or(NO_ENTRY, |col| col.next_sibling);
    }
    None
}

const fn index(raw: u32) -> Option<usize> {
    if raw == NO_ENTRY {
        None
    } else {
        Some(raw as usize)
    }
}

/// Calls `f` with the part `part` names under `id`, or returns `None` when `part` is
/// [`NO_PART`] or names no declared part.
///
/// Takes a closure rather than returning a borrow: the parts live behind a versioned table
/// each reader refreshes into a thread-local, so what `f` receives borrows this thread's
/// own copy and must not outlive the call.
fn with_part<R>(
    shared: &Shared,
    id: ControlId,
    part: u32,
    f: impl FnOnce(&Part) -> R,
) -> Option<R> {
    if part == NO_PART {
        return None;
    }
    shared.regions.with_parts(id, |parts| {
        parts.iter().find(|candidate| candidate.sub == part).map(f)
    })
}

/// Returns the `LiveSetting` value for a column's flags: 0 off, 1 polite, 2 assertive.
const fn live_setting(flags: ColFlags) -> i32 {
    if flags.has(ColFlags::LIVE_ASSERTIVE) {
        2
    } else if flags.has(ColFlags::LIVE_POLITE) {
        1
    } else {
        0
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// Returns `UIA_E_ELEMENTNOTAVAILABLE`, the answer for an element that has unmounted or a
/// window that has gone.
pub fn gone() -> Error {
    Error::from_hresult(NOT_AVAILABLE)
}

/// Returns the empty error, which marshals as `S_OK` with a null out-parameter: nothing is
/// there, and the call did not fail.
pub fn none() -> Error {
    Error::empty()
}

// ── the two objects' shared vtables ─────────────────────────────────────────────

macro_rules! provider {
    ($imp:ident) => {
        impl crate::bindings::IRawElementProviderSimple_Impl for $imp {
            fn ProviderOptions(&self) -> Result<ProviderOptions> {
                Ok(OPTIONS)
            }
            fn GetPatternProvider(&self, id: PATTERNID) -> Result<IUnknown> {
                self.pattern(id)
            }
            fn GetPropertyValue(&self, id: PROPERTYID) -> Result<VARIANT> {
                Ok(self.property(id))
            }
            fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
                self.host()
            }
        }

        impl crate::bindings::IRawElementProviderFragment_Impl for $imp {
            fn Navigate(&self, d: NavigateDirection) -> Result<IRawElementProviderFragment> {
                self.navigate(d)
            }
            fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
                self.runtime_id()
            }
            fn get_BoundingRectangle(&self) -> Result<UiaRect> {
                self.bounds()
            }
            fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
                // Nothing here hosts a foreign fragment root. A null array is the
                // documented answer and is not an error.
                Ok(core::ptr::null_mut())
            }
            fn SetFocus(&self) -> Result<()> {
                self.set_focus()
            }
            fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
                self.fragment_root()
            }
        }
    };
}

provider!(Element_Impl);
provider!(Root_Impl);

impl crate::bindings::IRawElementProviderFragmentRoot_Impl for Root_Impl {
    fn ElementProviderFromPoint(&self, x: f64, y: f64) -> Result<IRawElementProviderFragment> {
        self.this.element_at(x, y)
    }

    fn GetFocus(&self) -> Result<IRawElementProviderFragment> {
        self.this.focused()
    }
}

/// Returns the published tree, refreshing this thread's own reference if it has moved.
pub fn tree_of(shared: &Arc<Shared>) -> Arc<Tree> {
    SEEN.with(|seen| shared.slot.read(seen))
}

/// Returns the object a raised event names, or `None` when the element is not in the
/// published tree.
pub fn provider_for(shared: &Arc<Shared>, id: ControlId) -> Option<IRawElementProviderSimple> {
    if !id.is_none() {
        let tree = SEEN.with(|seen| shared.slot.read(seen));
        tree.index_of(id)?;
    }
    Some(shared.provider(id, NO_PART))
}

/// Tells automation the window's providers are finished with, and drops every minted
/// object.
///
/// `UiaReturnRawElementProvider(hwnd, 0, 0, NULL)` is the documented way to say it, and it
/// is not the same as our own references going away: automation caches per window, so
/// without this it keeps that cache — and a client keeps a window element — for a window
/// that no longer exists.
///
/// Must be called while the window handle is still valid, which puts it on `WM_DESTROY`
/// rather than on a drop.
pub fn disconnect(shared: &Arc<Shared>) {
    let hwnd = shared.window();
    if !hwnd.is_null() && shared.queried.swap(false, Relaxed) {
        // SAFETY: the handle names a window this process owns and has not yet destroyed,
        // and a null provider is the documented argument for releasing its cache.
        unsafe {
            _ = UiaReturnRawElementProvider(hwnd, 0, 0, core::ptr::null_mut());
        }
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
    // The latch transition is what asks for a tick: a window that is not laid out again
    // never publishes on its own, and the client that just attached would walk nothing.
    if !shared.queried.swap(true, Relaxed) {
        shared.wake();
    }
    let object = shared.provider(ControlId::NONE, NO_PART);
    // SAFETY: the handle names a window this process owns, and `object` holds a reference
    // for the whole call — `UiaReturnRawElementProvider` takes its own.
    Some(unsafe { UiaReturnRawElementProvider(shared.window(), w, l, object.as_raw()) })
}
