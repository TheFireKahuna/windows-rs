//! The window-procedure half of the pointer stack: it records what a message says and leaves
//! every decision to the frame-clock half.
//!
//! A pointer message signals that samples are available; it does not carry them. The samples
//! live in a system-side history ring addressed by pointer id, so a consumer that reads the
//! ring once per frame keeps the intermediate samples legacy coalescing discards.
//!
//! Every arm here writes a ring slot or a bit and returns. It performs no hit test, touches no
//! tree state and mutates no interaction state.
//!
//! Discrete transitions retain their Win32 position and, for down, up and wheel, a WinRT
//! point. The system's message data may be gone after the pump retrieves another message, so
//! keeping only an id makes a short touch impossible to finish. The ring storage is fixed; the
//! platform owns each point's allocation, released when its ring record is consumed.

use super::{Coords, PointerSpace, Service};
use crate::bindings::*;
use crate::gesture::SLOTS;
use core::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use windows_core::{ComObject, Result};
use windows_window::{Tick, Wake, Window};

/// How many discrete transitions one frame may carry.
///
/// Sized for the worst frame: ten contacts lifting together with their button changes, plus a
/// burst of wheel notches. Overflow is a violated invariant rather than a lossy path.
const RING: usize = 64;

/// Names which transition a ring record carries. Motion has no variant: it sets a per-contact
/// bit instead.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    Down,
    Up,
    /// A button changed while the contact stayed down — the second mouse button pressed during
    /// a drag, or a pen's barrel button.
    Button,
    /// Ends one contact without an up: the gesture aborts and no value is committed. An id of
    /// zero names whatever holds the window's explicit capture.
    Cancel,
    /// The window lost every contact at once, which is what losing focus does.
    CaptureLost,
    Wheel,
}

/// Names the device a contact came from.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PointerType {
    #[default]
    Mouse,
    Touch,
    Pen,
    /// A precision touchpad, reporting real contacts rather than a synthesized wheel.
    Touchpad,
}

impl PointerType {
    /// Maps a `POINTER_INPUT_TYPE` onto this enum. An unknown type reads as a mouse, which is
    /// the one classification that routes through every path unchanged.
    #[must_use]
    pub(crate) const fn from_raw(raw: POINTER_INPUT_TYPE) -> Self {
        match raw as i32 {
            PT_TOUCH => Self::Touch,
            PT_PEN => Self::Pen,
            PT_TOUCHPAD => Self::Touchpad,
            _ => Self::Mouse,
        }
    }

    /// Returns the contact kind the hit array is queried with, so target inflation applies to
    /// exactly the devices that have a contact patch.
    #[must_use]
    pub const fn contact(self) -> windows_scene::ContactKind {
        match self {
            Self::Mouse => windows_scene::ContactKind::Mouse,
            Self::Touch => windows_scene::ContactKind::Touch,
            Self::Pen => windows_scene::ContactKind::Pen,
            Self::Touchpad => windows_scene::ContactKind::Touchpad,
        }
    }

    /// Returns whether this device's contacts go to the precision-touchpad recogniser.
    #[must_use]
    pub const fn is_touchpad(self) -> bool {
        matches!(self, Self::Touchpad)
    }
}

/// Names the flag word a pointer message carries in the high half of its `wParam`.
///
/// `GET_POINTERID_WPARAM` and the `IS_POINTER_*_WPARAM` family are C macros with no metadata,
/// so each test is written here over the generated constants. Two are load-bearing:
/// [`confident`](Self::confident) gates palm rejection, and [`canceled`](Self::canceled)
/// separates a cancel from an up.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PointerFlags(pub u32);

impl PointerFlags {
    /// Extracts the flag word from a pointer message's `wparam`.
    #[must_use]
    pub const fn from_wparam(wparam: usize) -> Self {
        Self((wparam >> 16) as u32)
    }

    const fn has(self, bit: i32) -> bool {
        self.0 & (bit as u32) != 0
    }

    /// Returns whether this is the primary contact — the one that drives hover and the one a
    /// single-pointer gesture is measured from.
    #[must_use]
    pub const fn primary(self) -> bool {
        self.has(POINTER_FLAG_PRIMARY)
    }

    /// Returns whether the contact touches the digitizer or holds a button.
    #[must_use]
    pub const fn in_contact(self) -> bool {
        self.has(POINTER_FLAG_INCONTACT)
    }

    /// Returns whether the contact was aborted rather than released, so the pre-drag value is
    /// restored.
    #[must_use]
    pub const fn canceled(self) -> bool {
        self.has(POINTER_FLAG_CANCELED)
    }

    /// Returns whether the digitizer reports a deliberate contact rather than a palm.
    ///
    /// A contact without it is tracked but never fed to a recogniser. Palm rejection on this
    /// stack is that rule alone.
    #[must_use]
    pub const fn confident(self) -> bool {
        self.has(POINTER_FLAG_CONFIDENCE)
    }

    /// Returns the held buttons, as the five contiguous `POINTER_FLAG_*BUTTON` bits packed
    /// into bits 0 to 4.
    #[must_use]
    pub const fn buttons(self) -> u32 {
        (self.0 >> 4) & 0x1f
    }
}

/// One discrete pointer transition, recorded where and when it happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PointerEvent {
    pub id: u32,
    pub kind: EventKind,
    pub ptype: PointerType,
    /// The five `POINTER_FLAG_*BUTTON` bits, packed. Bit 0 is the primary button.
    pub buttons: u32,
    pub flags: PointerFlags,
    /// `ptPixelLocationRaw`, in physical screen pixels. Raw rather than predicted, because a
    /// press target chosen from an extrapolated point is a mis-click.
    pub x_px: i32,
    pub y_px: i32,
    /// Notches × `WHEEL_DELTA`, signed. Zero on every kind but [`EventKind::Wheel`].
    pub wheel: i32,
    pub horizontal: bool,
    pub time: u32,
    /// The recogniser's point, retained before the pump retrieves another message and the
    /// system may retire this pointer. Aborts and button changes need no recogniser point.
    point: Option<Result<PointerPoint>>,
    /// The input record a touch press hands to the compositor when the press is offered to a
    /// scroll container, captured for the same reason and at the same moment.
    manipulation: Option<Result<windows_scene::ManipulationPointer>>,
}

impl PointerEvent {
    /// Returns the retained point of a down, up or wheel.
    ///
    /// `None` on a transition that carries none, and the capture failure where taking the
    /// point failed — a failure travels with the record rather than being replaced with an
    /// invented point.
    pub(crate) fn point(&self) -> Option<Result<&PointerPoint>> {
        self.point
            .as_ref()
            .map(|held| held.as_ref().map_err(Clone::clone))
    }

    /// Returns the input record a scroll handoff redirects with, on a touch down alone.
    pub(crate) fn manipulation(&self) -> Option<Result<windows_scene::ManipulationPointer>> {
        self.manipulation.clone()
    }
}

/// Names which keyboard transition a record carries.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KeyKind {
    Down,
    Up,
    /// A translated character. `key` is one UTF-16 code unit.
    Char,
}

/// The modifier keys held when a key transition happened.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl Mods {
    /// Reads the three modifier keys. Called only on a key transition, which is discrete.
    fn now() -> Self {
        // SAFETY: `GetKeyState` takes a virtual-key code by value and writes through no
        // pointer.
        unsafe {
            Self {
                shift: GetKeyState(VK_SHIFT) < 0,
                ctrl: GetKeyState(VK_CONTROL) < 0,
                alt: GetKeyState(VK_MENU) < 0,
            }
        }
    }
}

/// One keyboard transition.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub kind: KeyKind,
    /// A virtual-key code, or for [`KeyKind::Char`] a UTF-16 code unit.
    pub key: u16,
    pub repeat: bool,
    pub mods: Mods,
}

/// Carries one record through the doorbell's ring.
///
/// One ring holds both kinds, because the order between a keystroke and a contact is
/// observable: a `Tab` that moves focus and a press that changes it resolve in the order the
/// user made them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputEvent {
    Pointer(PointerEvent),
    Key(KeyEvent),
}

/// One tracked contact. Motion writes nothing but [`moved`](Self::moved).
#[derive(Copy, Clone, Default, Debug)]
struct Contact {
    /// Zero means the slot is free.
    id: u32,
    /// The per-pointer dirty bit, which is all a `WM_POINTERUPDATE` writes.
    moved: bool,
    /// Whether the contact was in contact at its last transition, so the tick can tell a drag
    /// from a hover without asking the system.
    down: bool,
    /// The buttons held at the last message, so a button change is detectable from the flag
    /// word alone.
    buttons: u32,
}

/// Counts what the doorbell could not record.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct DoorbellHealth {
    /// Discrete transitions dropped because the ring was full. A violated invariant: any
    /// non-zero value means the ring capacity is too small for a frame the host can produce.
    pub dropped: u32,
    /// The deepest the ring has been, which the ring capacity is sized from.
    pub peak: u32,
}

/// Everything one `RefCell` borrow reaches, so a window-procedure arm takes one borrow and
/// writes.
#[derive(Default)]
struct State {
    /// The ring, allocated once at construction.
    ring: VecDeque<InputEvent>,
    slots: [Contact; SLOTS],
    /// Which pointer is over the client area, tracked from motion and the leave message, so no
    /// move needs a `TrackMouseEvent`.
    hovering: Option<u32>,
    /// A system request to stop content inertia.
    stop: bool,
    /// The frame clock, set once the window has a pacer. Messages arriving before that are
    /// recorded and consumed by the first tick.
    wake: Option<Wake>,
    /// One frame request covering everything outstanding, taken on the first arrival and
    /// dropped by the tick that finds nothing left. Not one per event, so the steady state
    /// during a drag is several messages and no kernel call at all.
    pending: Option<Tick>,
    health: DoorbellHealth,
}

impl State {
    /// Returns whether nothing is waiting for the next tick.
    fn idle(&self) -> bool {
        self.ring.is_empty() && !self.stop && !self.slots.iter().any(|c| c.moved)
    }

    /// Asks for a frame, once, for everything outstanding.
    fn frame(&mut self) {
        if self.pending.is_none() {
            self.pending = self.wake.as_ref().map(Wake::tick);
        }
    }

    /// Appends one record and asks for a frame.
    ///
    /// Overflow drops the oldest record, so the survivors stay in order, and counts the drop
    /// in [`DoorbellHealth::dropped`].
    fn push(&mut self, event: InputEvent) {
        if self.ring.len() == RING {
            debug_assert!(
                false,
                "the doorbell ring overflowed: RING is too small for a frame the host produces"
            );
            self.ring.pop_front();
            self.health.dropped += 1;
        }
        self.ring.push_back(event);
        self.health.peak = self.health.peak.max(self.ring.len() as u32);
        self.frame();
    }

    /// Finds or takes a slot for `id`. `None` once every slot is taken.
    fn slot(&mut self, id: u32) -> Option<usize> {
        if let Some(i) = self.slots.iter().position(|c| c.id == id) {
            return Some(i);
        }
        let i = self.slots.iter().position(|c| c.id == 0)?;
        self.slots[i] = Contact {
            id,
            ..Contact::default()
        };
        Some(i)
    }

    /// Returns how many contacts are tracked.
    fn live(&self) -> usize {
        self.slots.iter().filter(|c| c.id != 0).count()
    }
}

/// Records what a window message says, for the frame clock to resolve.
///
/// Installed into the window at creation and shared with the [`Router`](super::Router) that
/// drains it on the frame clock.
pub struct Doorbell {
    /// The window this doorbell serves, null until [`Doorbell::pace`] names it.
    hwnd: Cell<HWND>,
    /// The `user32` exports this build carries, resolved once and shared with every conversion
    /// this doorbell hands out.
    late: super::Late,
    /// The unit the WinRT pointer statics were measured to answer in, and the transform the
    /// recogniser's points are taken through.
    space: ComObject<PointerSpace>,
    transform: IPointerPointTransform,
    /// The request-for-service gate, shared with every other producer of latency-critical
    /// input — the dial's handler holds a clone, so one tick services both.
    service: Rc<Service>,
    state: RefCell<State>,
}

impl Default for Doorbell {
    fn default() -> Self {
        Self::new()
    }
}

impl Doorbell {
    /// Creates a doorbell with an empty ring and nothing pending.
    #[must_use]
    pub fn new() -> Self {
        let space = ComObject::new(PointerSpace::default());
        let transform = space.to_interface();
        Self {
            hwnd: Cell::new(core::ptr::null_mut()),
            late: super::Late::resolve(),
            space,
            transform,
            service: Rc::new(Service::default()),
            state: RefCell::new(State {
                ring: VecDeque::with_capacity(RING),
                ..State::default()
            }),
        }
    }

    /// Attaches the window this doorbell serves and that window's frame clock.
    ///
    /// Separate from construction: the doorbell is installed into the window builder, so it
    /// exists before the window, and a pacer cannot exist before the window it posts to.
    /// Anything that arrives in between is recorded and consumed by the first tick.
    pub fn pace(&self, window: &Window, wake: Wake) {
        let hwnd = window.hwnd();
        self.hwnd.set(hwnd);
        self.service.attach(hwnd);
        let mut state = self.state.borrow_mut();
        state.wake = Some(wake);
        if !state.idle() {
            state.frame();
        }
    }

    /// Returns the request-for-service gate, for another producer of latency-critical input.
    ///
    /// One gate per window rather than one per producer: two gates coalesce independently and
    /// post twice for the same tick's worth of work.
    #[must_use]
    pub fn service(&self) -> Rc<Service> {
        Rc::clone(&self.service)
    }

    /// Returns the counters for what the doorbell could not record.
    #[must_use]
    pub fn health(&self) -> DoorbellHealth {
        self.state.borrow().health
    }

    /// Returns the contact hovering the client area, or `None` when none is.
    #[must_use]
    pub fn hovering(&self) -> Option<u32> {
        self.state.borrow().hovering
    }

    /// Returns whether nothing is waiting for the next tick.
    ///
    /// The frame request is derived from this: a doorbell that is never idle is a window that
    /// never parks.
    #[must_use]
    pub fn idle(&self) -> bool {
        self.state.borrow().idle()
    }

    // ── the window-procedure arms ─────────────────────────────────────────────────

    /// Rings the bell for one window message. `Some(0)` means the message is consumed.
    ///
    /// Every pointer arm that carries a contact returns `Some`. `DefWindowProc` promotes
    /// pointer input into legacy mouse messages, so a fall-through is the one way a legacy
    /// message can still be produced, and nothing here can write a legacy arm because neither
    /// binding filter generates a constant to write one with.
    ///
    /// `WM_POINTERLEAVE` is the exception. The custom caption reads the same message to clear
    /// a window command's hover, and the window procedure runs the application's handler
    /// before the caption's, so consuming it here would leave a close button lit after the
    /// pointer had gone. It carries no position and starts no contact, so what
    /// `DefWindowProc` may make of it is a `WM_MOUSELEAVE`, which is in the set nothing here
    /// can handle anyway. The key messages and the two window-state messages are forwarded for
    /// the same reason: `WM_KEYDOWN` has to reach `TranslateMessage` for `WM_CHAR` to exist at
    /// all, and the application's own focus handling runs behind this one.
    pub fn wndproc(&self, message: u32, wparam: usize, lparam: isize) -> Option<isize> {
        let id = (wparam & 0xffff) as u32;
        let flags = PointerFlags::from_wparam(wparam);
        match message as i32 {
            WM_POINTERDOWN => self.discrete(id, flags, EventKind::Down, 0, false),
            WM_POINTERUP => self.discrete(id, flags, EventKind::Up, 0, false),
            WM_POINTERWHEEL => self.discrete(id, flags, EventKind::Wheel, notches(wparam), false),
            WM_POINTERHWHEEL => self.discrete(id, flags, EventKind::Wheel, notches(wparam), true),
            WM_POINTERCAPTURECHANGED => self.discrete(id, flags, EventKind::Cancel, 0, false),
            WM_POINTERUPDATE | WM_POINTERENTER => self.motion(id, flags),
            WM_POINTERLEAVE => self.leave(id),
            // A window-level capture change names no pointer: its `wParam` is a window handle.
            // The tick resolves it against whatever holds the explicit capture, and a change
            // that takes nothing away is not a cancel.
            WM_CAPTURECHANGED => self.window_event(EventKind::Cancel),
            // A window that loses focus loses every contact with it, since the input that
            // would have ended them goes elsewhere.
            WM_KILLFOCUS => self.window_event(EventKind::CaptureLost),
            WM_KEYDOWN | WM_SYSKEYDOWN => self.key(KeyKind::Down, wparam, lparam),
            WM_KEYUP | WM_SYSKEYUP => self.key(KeyKind::Up, wparam, lparam),
            WM_CHAR => self.key(KeyKind::Char, wparam, lparam),
            _ => None,
        }
    }

    /// Records a discrete transition, resolving the point it happened at.
    ///
    /// Asks for service on the next pump iteration rather than at the next composition frame:
    /// a press, a release, a cancel and a wheel notch are latency-critical, and waiting for
    /// the frame clock would add a frame to each.
    fn discrete(
        &self,
        id: u32,
        flags: PointerFlags,
        kind: EventKind,
        wheel: i32,
        horizontal: bool,
    ) -> Option<isize> {
        let mut info = POINTER_INFO::default();
        // A record is written whether or not the system still answers about this pointer: the
        // transition happened, and a dropped record is a swallowed press.
        //
        // SAFETY: `info` is a stack local of the type the call writes, and the id came from
        // the message being serviced.
        _ = unsafe { GetPointerInfo(id, &mut info) };
        // A cancel is not an up: it aborts the gesture and commits nothing.
        let kind = match (kind, flags.canceled()) {
            (EventKind::Up, true) => EventKind::Cancel,
            (kind, _) => kind,
        };
        let ptype = PointerType::from_raw(info.pointerType);
        let event = PointerEvent {
            id,
            kind,
            ptype,
            buttons: flags.buttons(),
            flags,
            x_px: info.ptPixelLocationRaw.x,
            y_px: info.ptPixelLocationRaw.y,
            wheel,
            horizontal,
            time: info.dwTime,
            point: matches!(kind, EventKind::Down | EventKind::Up | EventKind::Wheel)
                .then(|| self.capture(id, &info)),
            manipulation: (kind == EventKind::Down && ptype == PointerType::Touch)
                .then(|| windows_scene::ManipulationPointer::capture(id)),
        };
        let mut state = self.state.borrow_mut();
        // A wheel notch is not a contact: it takes no slot, so a burst of them over a window
        // cannot fill the table with pointers that will never lift.
        if kind != EventKind::Wheel
            && let Some(i) = state.slot(id)
        {
            state.slots[i].down = flags.in_contact();
            state.slots[i].buttons = event.buttons;
            // Kept dirty until the tick has consumed the transition, so the frame that ends a
            // drag still reports the contact that ended it.
            state.slots[i].moved = true;
        }
        state.push(InputEvent::Pointer(event));
        drop(state);
        self.service.now();
        Some(0)
    }

    /// Records a transition that belongs to no particular pointer.
    fn window_event(&self, kind: EventKind) -> Option<isize> {
        self.state
            .borrow_mut()
            .push(InputEvent::Pointer(PointerEvent {
                id: 0,
                kind,
                ptype: PointerType::Mouse,
                buttons: 0,
                flags: PointerFlags::default(),
                x_px: 0,
                y_px: 0,
                wheel: 0,
                horizontal: false,
                time: 0,
                point: None,
                manipulation: None,
            }));
        self.service.now();
        None
    }

    /// Records motion: sets one bit and returns.
    ///
    /// It also establishes hover presence. On 26200 a mouse moving into this window's client
    /// area produces `WM_POINTERUPDATE` and no `WM_POINTERENTER` at all, so a hover state
    /// derived from the enter message alone never begins. Only the entering half is inferred —
    /// a pointer that updates over the client area is over it — while leave stays the real
    /// message, which the custom caption depends on.
    fn motion(&self, id: u32, flags: PointerFlags) -> Option<isize> {
        let mut state = self.state.borrow_mut();
        // Only the primary contact drives hover: a second finger arriving does not move the
        // hover chrome, and a pen entering range while a finger is down does not either.
        if flags.primary() {
            state.hovering = Some(id);
        }
        let Some(i) = state.slot(id) else {
            return Some(0);
        };
        state.slots[i].moved = true;
        state.slots[i].down = flags.in_contact();
        let changed = state.slots[i].buttons != flags.buttons();
        state.slots[i].buttons = flags.buttons();
        state.frame();
        let many = state.live() > 1;
        drop(state);
        // The tick reads the whole input frame from history itself, so the rest of it need not
        // arrive one message at a time. A single contact has no rest of the frame, so the call
        // would be a syscall per move.
        if many {
            // SAFETY: `SkipPointerFrameMessages` takes the id by value and writes through no
            // pointer; the id came from the message being serviced.
            unsafe {
                _ = SkipPointerFrameMessages(id);
            }
        }
        // A second button pressed during a drag arrives as an update with a changed button
        // set, never as a second down, so the change is a bit compare against the slot.
        if changed {
            return self.discrete(id, flags, EventKind::Button, 0, false);
        }
        Some(0)
    }

    /// Clears hover presence, and the slot with it unless a captured drag still owns it.
    fn leave(&self, id: u32) -> Option<isize> {
        let mut state = self.state.borrow_mut();
        if state.hovering == Some(id) {
            state.hovering = None;
        }
        if let Some(i) = state.slot(id) {
            // The contact is gone from this window, but a captured drag still owns it until
            // its up arrives — so the slot is released only when nothing is down on it.
            if state.slots[i].down {
                state.slots[i].moved = true;
            } else {
                state.slots[i] = Contact::default();
            }
        }
        // The hover the tick has to clear is not a moved contact, so the frame is asked for
        // here rather than derived from a dirty bit.
        state.frame();
        // Not consumed, so the custom caption behind this handler still sees it.
        None
    }

    /// Records a key transition and asks for service on the next pump iteration.
    fn key(&self, kind: KeyKind, wparam: usize, lparam: isize) -> Option<isize> {
        self.state.borrow_mut().push(InputEvent::Key(KeyEvent {
            kind,
            key: wparam as u16,
            // Bit 30 of the key message's `lParam` is the previous key state.
            repeat: kind != KeyKind::Char && lparam & (1 << 30) != 0,
            mods: Mods::now(),
        }));
        self.service.now();
        None
    }

    /// Captures a point while its message owns the platform's pointer information.
    ///
    /// A pointer id does not retain its data: Windows may discard it when the pump retrieves
    /// the next message, so a touch release followed by a leave cannot be looked up again at
    /// the tick.
    ///
    /// # Errors
    ///
    /// The system no longer answers about this pointer, which is what a contact retired
    /// between the message and the capture looks like.
    fn capture(&self, id: u32, info: &POINTER_INFO) -> Result<PointerPoint> {
        // Measured before the point is taken, so even the first gesture is transformed by a
        // number that was measured rather than assumed.
        let hwnd = self.hwnd.get();
        if !self.space.measured()
            && !hwnd.is_null()
            && let Ok(untransformed) = PointerPoint::GetCurrentPoint(id)
            && let Ok(raw) = untransformed.RawPosition()
        {
            let scale = windows_window::Metrics::for_window(hwnd).scale;
            let ours = Coords::new(hwnd, self.late).client_at_scale(
                scale,
                id,
                info.ptPixelLocationRaw.x,
                info.ptPixelLocationRaw.y,
            );
            self.space
                .calibrate(windows_scene::Point { x: raw.x, y: raw.y }, ours);
        }
        PointerPoint::GetCurrentPointTransformed(id, &self.transform)
    }

    // ── what the tick reads ───────────────────────────────────────────────────────

    /// Returns the conversion every contact on this window resolves through.
    pub(crate) fn coords(&self) -> Coords {
        Coords::new(self.hwnd.get(), self.late)
    }

    /// Returns the `user32` exports this build carries.
    pub(crate) const fn late(&self) -> super::Late {
        self.late
    }

    /// Returns the measured relation between the WinRT pointer statics' space and the hit
    /// array's DIPs.
    pub(crate) fn space(&self) -> &PointerSpace {
        &self.space
    }

    /// Returns the transform the recogniser's points are taken through.
    pub(crate) fn transform(&self) -> &IPointerPointTransform {
        &self.transform
    }

    /// Removes and returns the oldest record, or `None` when the ring is empty.
    pub(crate) fn pop(&self) -> Option<InputEvent> {
        self.state.borrow_mut().ring.pop_front()
    }

    /// Writes the contacts that moved since the last tick into `out`, clearing their dirty
    /// bits.
    ///
    /// Fills a caller-owned buffer rather than returning a collection: this is on the
    /// per-frame hover path, which allocates nothing.
    pub(crate) fn moved_into(&self, out: &mut Vec<u32>) {
        out.clear();
        for contact in self.state.borrow_mut().slots.iter_mut() {
            if contact.id != 0 && core::mem::take(&mut contact.moved) {
                out.push(contact.id);
            }
        }
    }

    /// Returns whether the contact `id` is down.
    pub(crate) fn is_down(&self, id: u32) -> bool {
        self.state
            .borrow()
            .slots
            .iter()
            .any(|c| c.id == id && c.down)
    }

    /// Releases a contact's slot.
    ///
    /// Called by the tick once its up has been consumed, so a frame that both ends a drag and
    /// starts a new contact still sees both.
    pub(crate) fn release(&self, id: u32) {
        if let Some(contact) = self
            .state
            .borrow_mut()
            .slots
            .iter_mut()
            .find(|c| c.id == id)
        {
            *contact = Contact::default();
        }
    }

    /// Records a system inertia stop.
    ///
    /// No message arm reaches this: neither `WM_STOPINERTIA` nor `WM_ENDINERTIA` is in the
    /// generated bindings, so there is no constant to match on.
    pub(crate) fn stop_inertia(&self) {
        self.state.borrow_mut().stop = true;
        self.service.now();
    }

    /// Takes the system's request to stop content inertia, clearing it.
    pub(crate) fn take_stop_inertia(&self) -> bool {
        core::mem::take(&mut self.state.borrow_mut().stop)
    }

    /// Releases the frame request. The tick calls this once it finds nothing outstanding.
    pub(crate) fn settle(&self) {
        let mut state = self.state.borrow_mut();
        if state.idle() {
            state.pending = None;
        }
    }
}

/// Returns the wheel delta a pointer wheel message carries in the high half of its `wParam`,
/// which is the flag word's place on every other pointer message.
const fn notches(wparam: usize) -> i32 {
    ((wparam >> 16) as u16) as i16 as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wparam(id: u32, flags: i32) -> usize {
        (id as usize) | ((flags as u32 as usize) << 16)
    }

    fn key(key: u16) -> InputEvent {
        InputEvent::Key(KeyEvent {
            kind: KeyKind::Down,
            key,
            repeat: false,
            mods: Mods::default(),
        })
    }

    #[test]
    fn the_flag_word_is_read_out_of_the_high_half() {
        let flags = PointerFlags::from_wparam(wparam(
            7,
            POINTER_FLAG_PRIMARY | POINTER_FLAG_INCONTACT | POINTER_FLAG_FIRSTBUTTON,
        ));
        assert!(flags.primary());
        assert!(flags.in_contact());
        assert!(!flags.canceled());
        assert!(!flags.confident());
        assert_eq!(flags.buttons(), 1);
    }

    #[test]
    fn every_button_bit_packs_to_its_own_place() {
        for (bit, expected) in [
            (POINTER_FLAG_FIRSTBUTTON, 1),
            (POINTER_FLAG_SECONDBUTTON, 2),
            (POINTER_FLAG_THIRDBUTTON, 4),
            (POINTER_FLAG_FOURTHBUTTON, 8),
            (POINTER_FLAG_FIFTHBUTTON, 16),
        ] {
            assert_eq!(
                PointerFlags::from_wparam(wparam(1, bit)).buttons(),
                expected
            );
        }
    }

    #[test]
    fn the_ring_keeps_order() {
        let bell = Doorbell::new();
        for k in 0..5u16 {
            bell.state.borrow_mut().push(key(k));
        }
        for k in 0..5u16 {
            let Some(InputEvent::Key(event)) = bell.pop() else {
                panic!("the ring lost a record");
            };
            assert_eq!(event.key, k);
        }
        assert_eq!(bell.pop(), None);
    }

    #[test]
    fn overflow_drops_the_oldest_and_says_so() {
        let bell = Doorbell::new();
        for k in 0..RING as u16 {
            bell.state.borrow_mut().push(key(k));
        }
        assert_eq!(bell.health().dropped, 0);
        assert_eq!(bell.health().peak, RING as u32);
        // One past capacity. `debug_assert!` panics in a debug build, so the drop is checked
        // only where the push returned; the count reports it in either build.
        let overflowed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            bell.state.borrow_mut().push(key(999))
        }));
        if overflowed.is_ok() {
            assert_eq!(bell.health().dropped, 1);
            let Some(InputEvent::Key(first)) = bell.pop() else {
                panic!("the ring emptied");
            };
            assert_eq!(
                first.key, 1,
                "overflow dropped the newest rather than the oldest"
            );
        }
    }

    #[test]
    fn a_contact_slot_is_reused_only_after_it_is_released() {
        let bell = Doorbell::new();
        bell.wndproc(
            WM_POINTERUPDATE as u32,
            wparam(3, POINTER_FLAG_PRIMARY | POINTER_FLAG_INCONTACT),
            0,
        );
        assert!(bell.is_down(3));
        let mut moved = Vec::new();
        bell.moved_into(&mut moved);
        assert_eq!(moved, [3]);
        // Taking clears the bit, so a frame with no motion reports none.
        bell.moved_into(&mut moved);
        assert!(moved.is_empty());

        bell.release(3);
        assert!(!bell.is_down(3));
        bell.moved_into(&mut moved);
        assert!(moved.is_empty());
    }

    #[test]
    fn a_captured_drag_keeps_its_slot_across_leave() {
        // The contact is gone from this window, but its up has still to arrive: a slot
        // released here loses the release that ends the drag.
        let bell = Doorbell::new();
        let down = wparam(3, POINTER_FLAG_PRIMARY | POINTER_FLAG_INCONTACT);
        bell.wndproc(WM_POINTERUPDATE as u32, down, 0);
        assert_eq!(bell.wndproc(WM_POINTERLEAVE as u32, wparam(3, 0), 0), None);
        assert_eq!(bell.hovering(), None, "hover survived the pointer leaving");
        assert!(bell.is_down(3), "a captured drag lost its slot on leave");

        // A hovering contact owns nothing, so leave frees its slot outright.
        bell.wndproc(WM_POINTERUPDATE as u32, wparam(4, POINTER_FLAG_PRIMARY), 0);
        bell.wndproc(WM_POINTERLEAVE as u32, wparam(4, 0), 0);
        let mut moved = Vec::new();
        bell.moved_into(&mut moved);
        assert_eq!(moved, [3]);
    }

    #[test]
    fn a_pointer_type_maps_onto_the_one_hit_authority() {
        use windows_scene::ContactKind;
        assert_eq!(PointerType::from_raw(PT_TOUCH as u32), PointerType::Touch);
        assert_eq!(
            PointerType::from_raw(PT_TOUCHPAD as u32),
            PointerType::Touchpad
        );
        assert_eq!(PointerType::from_raw(PT_PEN as u32), PointerType::Pen);
        assert_eq!(PointerType::from_raw(PT_MOUSE as u32), PointerType::Mouse);
        assert_eq!(PointerType::from_raw(9999), PointerType::Mouse);
        // Only touch and pen inflate a target, and the mapping carries that.
        assert!(PointerType::Touch.contact().inflates());
        assert!(PointerType::Pen.contact().inflates());
        assert!(!PointerType::Touchpad.contact().inflates());
        assert!(!PointerType::Mouse.contact().inflates());
        assert_eq!(PointerType::Mouse.contact(), ContactKind::Mouse);
    }

    #[test]
    fn the_doorbell_is_idle_until_something_rings_it() {
        let bell = Doorbell::new();
        assert!(bell.idle());
        bell.wndproc(WM_POINTERUPDATE as u32, wparam(1, POINTER_FLAG_PRIMARY), 0);
        assert!(!bell.idle(), "a moved contact is not idle");
        let mut moved = Vec::new();
        bell.moved_into(&mut moved);
        assert!(bell.idle());
        bell.stop_inertia();
        assert!(!bell.idle());
        assert!(bell.take_stop_inertia());
        assert!(bell.idle());
    }

    #[test]
    fn a_canceled_up_is_a_cancel_and_a_window_capture_change_names_no_pointer() {
        let bell = Doorbell::new();
        bell.wndproc(WM_POINTERUP as u32, wparam(1, POINTER_FLAG_CANCELED), 0);
        let Some(InputEvent::Pointer(event)) = bell.pop() else {
            panic!("the up was not recorded");
        };
        assert_eq!(event.kind, EventKind::Cancel);
        assert_eq!(event.id, 1);

        // `WM_CAPTURECHANGED` carries a window handle, so its record names no pointer and the
        // tick resolves it against whatever holds the explicit capture.
        assert_eq!(bell.wndproc(WM_CAPTURECHANGED as u32, 0x1234, 0), None);
        let Some(InputEvent::Pointer(event)) = bell.pop() else {
            panic!("the capture change was not recorded");
        };
        assert_eq!((event.kind, event.id), (EventKind::Cancel, 0));
    }
}
