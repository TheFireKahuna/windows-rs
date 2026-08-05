//! The window-procedure half of the pointer stack: it records what a message says and leaves
//! every decision to the frame-clock half.
//!
//! A pointer message signals that samples are available; it does not carry them. The samples
//! live in a system-side history ring addressed by pointer id, so a consumer that reads the
//! ring once per frame keeps the intermediate samples legacy coalescing discards.
//!
//! Every arm here writes a ring slot or a bit and returns. It performs no hit test, touches
//! no tree state, mutates no interaction state and allocates nothing: the cost of hover is
//! (moves × tree size), and the frame clock bounds the first factor. A source lint enforces
//! the shape — a window procedure's pointer arms may not hit-test, reach into a tree, or
//! allocate.
//!
//! One syscall is made here. A discrete transition — down, up, button change, cancel —
//! records where it happened, because `GetPointerInfo` answers for the pointer's current
//! position and by tick time the contact has moved on; a press target chosen from that is a
//! mis-click. Motion makes no call at all: it sets a bit.

use super::service::Service;
use crate::bindings::*;
use core::cell::Cell;
use std::rc::Rc;
use windows_window::{Tick, Wake};

/// How many discrete transitions one frame may carry.
///
/// Sized for the worst frame rather than the common one: ten contacts lifting together with
/// their button changes, plus a burst of wheel notches. Overflow is a violated invariant
/// rather than a lossy path — asserted in debug, and counted in every build.
const RING_CAPACITY: usize = 64;

/// How many contacts are tracked at once. Ten is what a digitizer reports; the two spare
/// slots absorb a pen and a mouse arriving alongside a full hand.
const MAX_CONTACTS: usize = 12;

/// Names which transition a ring record carries. Motion has no variant: it sets a
/// per-contact bit instead.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    Down,
    Up,
    /// A button changed while the contact stayed down — the second mouse button pressed
    /// during a drag, or a pen's barrel button.
    Button,
    /// Ends the contact without an up: the gesture aborts and no value is committed.
    Cancel,
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
    /// Maps a `POINTER_INPUT_TYPE` onto this enum. An unknown type reads as a mouse, which
    /// is the one classification that routes through every path unchanged.
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

    /// Returns `true` when this device's contacts go to the precision-touchpad recogniser.
    #[must_use]
    pub const fn is_touchpad(self) -> bool {
        matches!(self, Self::Touchpad)
    }
}

/// Names the flag word a pointer message carries in the high half of its `wParam`.
///
/// `GET_POINTERID_WPARAM` and the `IS_POINTER_*_WPARAM` family are C macros with no
/// metadata, so each test is written here over the generated constants. Two are load-bearing
/// rather than informational: [`confident`](Self::confident) gates palm rejection, and
/// [`canceled`](Self::canceled) separates a cancel from an up.
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

    /// Returns `true` for the primary contact — the one that drives hover and the one a
    /// single-pointer gesture is measured from.
    #[must_use]
    pub const fn primary(self) -> bool {
        self.has(POINTER_FLAG_PRIMARY)
    }

    /// Returns `true` while the contact touches the digitizer or holds a button.
    #[must_use]
    pub const fn in_contact(self) -> bool {
        self.has(POINTER_FLAG_INCONTACT)
    }

    /// Returns `true` while the device is detectable without touching, which is what makes
    /// pen hover possible.
    #[must_use]
    pub const fn in_range(self) -> bool {
        self.has(POINTER_FLAG_INRANGE)
    }

    /// Returns `true` on the contact's first message.
    #[must_use]
    pub const fn new(self) -> bool {
        self.has(POINTER_FLAG_NEW)
    }

    /// Returns `true` when the contact was aborted rather than released, so the pre-drag
    /// value is restored.
    #[must_use]
    pub const fn canceled(self) -> bool {
        self.has(POINTER_FLAG_CANCELED)
    }

    /// Returns `true` when the digitizer reports a deliberate contact rather than a palm.
    ///
    /// A contact without it is tracked but never fed to a recogniser, so it cannot start a
    /// gesture. Palm rejection on this stack is that rule alone.
    #[must_use]
    pub const fn confident(self) -> bool {
        self.has(POINTER_FLAG_CONFIDENCE)
    }

    /// Returns the held buttons, as the five `POINTER_FLAG_*BUTTON` bits packed into bits
    /// 0 to 4.
    #[must_use]
    pub const fn buttons(self) -> u32 {
        let mut held = 0;
        if self.has(POINTER_FLAG_FIRSTBUTTON) {
            held |= 1;
        }
        if self.has(POINTER_FLAG_SECONDBUTTON) {
            held |= 2;
        }
        if self.has(POINTER_FLAG_THIRDBUTTON) {
            held |= 4;
        }
        if self.has(POINTER_FLAG_FOURTHBUTTON) {
            held |= 8;
        }
        if self.has(POINTER_FLAG_FIFTHBUTTON) {
            held |= 16;
        }
        held
    }
}

/// One discrete pointer transition, recorded where and when it happened.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
    /// Whether the wheel was horizontal.
    pub horizontal: bool,
    pub time: u32,
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
                shift: GetKeyState(VK_SHIFT as i32) < 0,
                ctrl: GetKeyState(VK_CONTROL as i32) < 0,
                alt: GetKeyState(VK_MENU as i32) < 0,
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
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InputEvent {
    Pointer(PointerEvent),
    Key(KeyEvent),
}

/// One tracked contact. Motion writes nothing but [`moved`](Self::moved).
#[derive(Default)]
struct Contact {
    id: Cell<u32>,
    live: Cell<bool>,
    /// The per-pointer dirty bit, which is all a `WM_POINTERUPDATE` writes.
    moved: Cell<bool>,
    /// Whether the contact was in contact at its last transition, so the tick can tell a
    /// drag from a hover without asking the system.
    down: Cell<bool>,
    /// Which buttons were held at the last message, so a button change is detectable from
    /// the flag word alone.
    buttons: Cell<u32>,
}

/// Counts what the doorbell could not record.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct DoorbellHealth {
    /// Discrete transitions dropped because the ring was full. A violated invariant: any
    /// non-zero value means the ring capacity is too small for a frame the host can produce.
    pub dropped: u32,
    /// Contacts refused because every slot was taken.
    pub unslotted: u32,
    /// The deepest the ring has been, which the ring capacity is sized from.
    pub peak: u32,
}

/// Records what a window message says, for the frame clock to resolve.
///
/// Installed into the window at creation and shared with the [`Router`](super::Router) that
/// drains it on the frame clock.
pub struct Doorbell {
    /// The ring, allocated once at construction. An empty slot is `None`: there is no
    /// [`InputEvent`] meaning "no transition", and inventing one would be a variant every
    /// match has to handle.
    slots: Box<[Cell<Option<InputEvent>>]>,
    head: Cell<usize>,
    tail: Cell<usize>,
    contacts: [Contact; MAX_CONTACTS],
    /// Which pointer is over the client area, tracked from the enter and leave messages, so
    /// no move needs a `TrackMouseEvent`.
    hover: Cell<Option<u32>>,
    /// A system request to stop content inertia. See [`super::Inertia`].
    stop_inertia: Cell<bool>,
    /// The frame clock, set once the window has a pacer. Messages arriving before that are
    /// recorded and consumed by the first tick.
    wake: Cell<Option<Wake>>,
    /// The request-for-service gate, shared with every other producer of latency-critical
    /// input — the dial's handler holds a clone of it, so one tick services both.
    service: Rc<Service>,
    /// One frame request covering everything outstanding, taken on the first arrival and
    /// dropped by the tick that finds nothing left. Not one per event, so the steady state
    /// during a drag is several messages and no kernel call at all.
    pending: Cell<Option<Tick>>,
    /// Whether [`pending`](Self::pending) holds a guard. A `Cell` cannot be peeked, and
    /// taking the guard out to inspect it would drop the request it stands for.
    held: Cell<bool>,
    health: Cell<DoorbellHealth>,
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
        Self {
            slots: (0..RING_CAPACITY).map(|_| Cell::new(None)).collect(),
            head: Cell::new(0),
            tail: Cell::new(0),
            contacts: Default::default(),
            hover: Cell::new(None),
            stop_inertia: Cell::new(false),
            wake: Cell::new(None),
            service: Rc::new(Service::new()),
            pending: Cell::new(None),
            held: Cell::new(false),
            health: Cell::new(DoorbellHealth::default()),
        }
    }

    /// Attaches the window this doorbell serves and that window's frame clock.
    ///
    /// Separate from construction: the doorbell is installed into the window builder, so it
    /// exists before the window, and a pacer cannot exist before the window it posts to.
    /// Anything that arrives in between is recorded and consumed by the first tick.
    pub fn pace(&self, window: &windows_window::Window, wake: Wake) {
        self.service.attach(window.hwnd());
        self.wake.set(Some(wake));
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
        self.health.get()
    }

    /// Returns the contact hovering the client area, or `None` when none is.
    #[must_use]
    pub fn hovering(&self) -> Option<u32> {
        self.hover.get()
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
    /// pointer had gone. It carries no position and starts no contact, so a promoted move has
    /// nothing to be about; what `DefWindowProc` may make of it is a `WM_MOUSELEAVE`, which
    /// is in the set nothing here can handle anyway.
    pub fn wndproc(&self, message: u32, wparam: usize, lparam: isize) -> Option<isize> {
        match message as i32 {
            WM_POINTERDOWN => self.transition(wparam, EventKind::Down),
            WM_POINTERUP => self.transition(wparam, EventKind::Up),
            WM_POINTERUPDATE => self.update(wparam),
            WM_POINTERENTER => self.enter(wparam),
            WM_POINTERLEAVE => self.leave(wparam),
            WM_POINTERCAPTURECHANGED | WM_CAPTURECHANGED => {
                self.transition(wparam, EventKind::CaptureLost)
            }
            WM_POINTERWHEEL => self.wheel(wparam, false),
            WM_POINTERHWHEEL => self.wheel(wparam, true),
            WM_KEYDOWN | WM_SYSKEYDOWN => self.key(KeyKind::Down, wparam, lparam),
            WM_KEYUP | WM_SYSKEYUP => self.key(KeyKind::Up, wparam, lparam),
            WM_CHAR => self.key(KeyKind::Char, wparam, lparam),
            // A window that loses focus loses every contact with it, since the input that
            // would have ended them goes elsewhere. Recorded as a lost capture and not
            // consumed, so the application's own focus handling still runs.
            WM_KILLFOCUS => {
                self.transition_now(EventKind::CaptureLost);
                None
            }
            _ => None,
        }
    }

    /// Records a discrete transition, resolving the point it happened at.
    fn transition(&self, wparam: usize, kind: EventKind) -> Option<isize> {
        let id = pointer_id(wparam);
        let flags = PointerFlags::from_wparam(wparam);
        // A cancel arrives as an up carrying the canceled bit. The two stay distinct because
        // an up commits a value and a cancel must not.
        let kind = if flags.canceled() && kind == EventKind::Up {
            EventKind::Cancel
        } else {
            kind
        };

        let mut info = POINTER_INFO::default();
        // SAFETY: the destination is a stack local of the type the call writes, and the id
        // came from the message being serviced.
        _ = unsafe { GetPointerInfo(id, &mut info) };
        let ptype = PointerType::from_raw(info.pointerType);

        self.mark(id, kind, ptype, flags);
        self.push(InputEvent::Pointer(PointerEvent {
            id,
            kind,
            ptype,
            buttons: flags.buttons(),
            flags,
            x_px: info.ptPixelLocationRaw.x,
            y_px: info.ptPixelLocationRaw.y,
            wheel: 0,
            horizontal: false,
            time: info.dwTime,
        }));
        self.skip_frame(id);
        Some(0)
    }

    /// Records a transition that belongs to no particular pointer — the window losing focus,
    /// or capture being taken away.
    fn transition_now(&self, kind: EventKind) {
        self.push(InputEvent::Pointer(PointerEvent {
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
        }));
    }

    /// Records motion: sets one bit and returns.
    ///
    /// It also establishes hover presence. On 26200 a mouse moving into this window's client
    /// area produces `WM_POINTERUPDATE` and no `WM_POINTERENTER` at all, so a hover state
    /// derived from the enter message alone never begins. Only the entering half is inferred
    /// — a pointer that updates over the client area is over it — while leave stays the real
    /// message, which the custom caption depends on.
    fn update(&self, wparam: usize) -> Option<isize> {
        let id = pointer_id(wparam);
        let flags = PointerFlags::from_wparam(wparam);
        let buttons = flags.buttons();
        match self.slot(id) {
            Some(slot) => {
                slot.moved.set(true);
                slot.down.set(flags.in_contact());
                // A second button pressed while the first is held arrives here, not as a
                // second `WM_POINTERDOWN`. The flag word already says which buttons are held,
                // so noticing the change is a bit compare rather than a syscall per move, and
                // only the change itself pays for a point.
                if slot.buttons.replace(buttons) != buttons {
                    self.transition(wparam, EventKind::Button);
                }
            }
            // A pointer whose enter was missed — a contact that began outside the window and
            // was captured into it. Claim a slot rather than dropping the motion.
            None => self.claim(id, flags.in_contact()),
        }
        if flags.primary() {
            self.hover.set(Some(id));
        }
        self.request();
        self.skip_frame(id);
        Some(0)
    }

    fn enter(&self, wparam: usize) -> Option<isize> {
        let id = pointer_id(wparam);
        let flags = PointerFlags::from_wparam(wparam);
        self.claim(id, flags.in_contact());
        // Only the primary contact drives hover: a second finger arriving does not move the
        // hover chrome, and a pen entering range while a finger is down does not either.
        if flags.primary() {
            self.hover.set(Some(id));
        }
        self.request();
        Some(0)
    }

    fn leave(&self, wparam: usize) -> Option<isize> {
        let id = pointer_id(wparam);
        if self.hover.get() == Some(id) {
            self.hover.set(None);
        }
        if let Some(slot) = self.slot(id) {
            // The contact is gone from this window, but a captured drag still owns it until
            // its up arrives — so the slot is released only when nothing is down on it.
            if !slot.down.get() {
                slot.live.set(false);
            }
            slot.moved.set(true);
        }
        self.request();
        // Not consumed, so the custom caption behind this handler still sees it. `wndproc`
        // states why.
        None
    }

    fn wheel(&self, wparam: usize, horizontal: bool) -> Option<isize> {
        let id = pointer_id(wparam);
        let flags = PointerFlags::from_wparam(wparam);
        let mut info = POINTER_INFO::default();
        // SAFETY: the destination is a stack local of the type the call writes, and the id
        // came from the message being serviced.
        let read = unsafe { GetPointerInfo(id, &mut info) }.as_bool();
        // The notch count rides in the high half of `wParam`, signed, as it does on the
        // legacy wheel message.
        let notches = ((wparam >> 16) as u16 as i16) as i32;
        self.push(InputEvent::Pointer(PointerEvent {
            id,
            kind: EventKind::Wheel,
            ptype: PointerType::from_raw(info.pointerType),
            buttons: flags.buttons(),
            flags,
            x_px: info.ptPixelLocationRaw.x,
            y_px: info.ptPixelLocationRaw.y,
            wheel: notches,
            horizontal,
            time: if read { info.dwTime } else { 0 },
        }));
        Some(0)
    }

    fn key(&self, kind: KeyKind, wparam: usize, lparam: isize) -> Option<isize> {
        self.push(InputEvent::Key(KeyEvent {
            kind,
            key: wparam as u16,
            // Bit 30 of the key message's `lParam` is the previous key state.
            repeat: kind != KeyKind::Char && lparam & (1 << 30) != 0,
            mods: Mods::now(),
        }));
        // Not consumed: `WM_KEYDOWN` has to reach `TranslateMessage` for `WM_CHAR` to exist
        // at all, and `WM_SYSKEY*` carries the system's own commands.
        None
    }

    /// Tells the system not to deliver the rest of this pointer's input frame one message
    /// at a time, because the tick reads the whole frame from history itself.
    ///
    /// Called only while more than one contact is live: a single contact has no rest of the
    /// frame, so the call would add a syscall per move.
    fn skip_frame(&self, id: u32) {
        if self.contacts.iter().filter(|c| c.live.get()).count() < 2 {
            return;
        }
        // SAFETY: `SkipPointerFrameMessages` takes the id by value and writes through no
        // pointer; the id came from the message being serviced.
        unsafe {
            _ = SkipPointerFrameMessages(id);
        }
    }

    // ── the ring and the contact table ────────────────────────────────────────────

    /// Appends one record and asks for service. Overflow drops the oldest record, so the
    /// survivors stay in order, and counts the drop in [`DoorbellHealth::dropped`].
    fn push(&self, event: InputEvent) {
        let head = self.head.get();
        let tail = self.tail.get();
        let mut health = self.health.get();
        if head - tail == RING_CAPACITY {
            debug_assert!(
                false,
                "the doorbell ring overflowed: RING_CAPACITY is too small"
            );
            health.dropped += 1;
            self.tail.set(tail + 1);
        }
        self.slots[head % RING_CAPACITY].set(Some(event));
        self.head.set(head + 1);
        health.peak = health.peak.max((self.head.get() - self.tail.get()) as u32);
        self.health.set(health);
        self.request();
        self.now();
    }

    /// Removes and returns the oldest record, or `None` when the ring is empty.
    pub(crate) fn pop(&self) -> Option<InputEvent> {
        let tail = self.tail.get();
        if tail == self.head.get() {
            return None;
        }
        self.tail.set(tail + 1);
        self.slots[tail % RING_CAPACITY].take()
    }

    /// Writes the contacts that moved since the last tick into `out`, clearing their dirty
    /// bits.
    ///
    /// Fills a caller-owned buffer rather than returning a collection: this is on the
    /// per-frame hover path, which allocates nothing.
    pub(crate) fn take_moved(&self, out: &mut Vec<u32>) {
        out.clear();
        for contact in &self.contacts {
            if contact.live.get() && contact.moved.replace(false) {
                out.push(contact.id.get());
            }
        }
    }

    /// Returns `true` while the contact `id` is down.
    pub(crate) fn is_down(&self, id: u32) -> bool {
        self.slot(id).is_some_and(|slot| slot.down.get())
    }

    /// Takes the system's request to stop content inertia, clearing it.
    pub(crate) fn take_stop_inertia(&self) -> bool {
        self.stop_inertia.replace(false)
    }

    /// Records a system inertia stop.
    ///
    /// No message arm reaches this: `WM_STOPINERTIA` and `WM_ENDINERTIA` are redacted from
    /// the 26100 SDK's own `winuser.h` and absent from the vendored metadata, so there is no
    /// constant to match on. [`super::Inertia`] drives it instead.
    pub(crate) fn stop_inertia(&self) {
        self.stop_inertia.set(true);
        self.request();
    }

    /// Records what a transition did to a contact's slot.
    fn mark(&self, id: u32, kind: EventKind, _ptype: PointerType, flags: PointerFlags) {
        match kind {
            EventKind::Down | EventKind::Button => {
                self.claim(id, flags.in_contact());
                if let Some(slot) = self.slot(id) {
                    slot.buttons.set(flags.buttons());
                }
            }
            EventKind::Up | EventKind::Cancel | EventKind::CaptureLost => {
                if let Some(slot) = self.slot(id) {
                    slot.down.set(false);
                    // Kept live until the tick has consumed the up, so the frame that ends a
                    // drag still reports the contact that ended it.
                    slot.moved.set(true);
                }
            }
            EventKind::Wheel => {}
        }
    }

    /// Finds or takes a slot for `id`.
    fn claim(&self, id: u32, down: bool) {
        if let Some(slot) = self.slot(id) {
            slot.down.set(down);
            return;
        }
        for contact in &self.contacts {
            if !contact.live.get() {
                contact.id.set(id);
                contact.live.set(true);
                contact.moved.set(true);
                contact.down.set(down);
                contact.buttons.set(0);
                return;
            }
        }
        let mut health = self.health.get();
        health.unslotted += 1;
        self.health.set(health);
    }

    /// Releases a contact's slot. Called by the tick once its up has been consumed, so a
    /// frame that both ends a drag and starts a new contact still sees both.
    pub(crate) fn release(&self, id: u32) {
        if let Some(slot) = self.slot(id) {
            slot.live.set(false);
            slot.moved.set(false);
            slot.down.set(false);
            slot.buttons.set(0);
        }
    }

    fn slot(&self, id: u32) -> Option<&Contact> {
        self.contacts
            .iter()
            .find(|contact| contact.live.get() && contact.id.get() == id)
    }

    /// Asks to be serviced on the next pump iteration rather than at the next composition
    /// frame.
    ///
    /// Only discrete transitions reach this — a press, a release, a cancel, a wheel notch, a
    /// keystroke — because waiting for the frame clock would add a frame of latency to each.
    /// Motion does not: it coalesces into a bit and is consumed once per frame, because an
    /// intermediate hover state between two presents is not observable and a manipulation
    /// reads its samples from history at whatever instant the tick runs. [`Service`] holds
    /// the gate.
    fn now(&self) {
        self.service.now();
    }

    /// Asks for a frame, once, for everything outstanding.
    fn request(&self) {
        if self.held.get() {
            return;
        }
        let wake = self.wake.take();
        if let Some(wake) = &wake {
            self.pending.set(Some(wake.tick()));
            self.held.set(true);
        }
        self.wake.set(wake);
    }

    /// Re-opens the immediate-service gate.
    pub(crate) fn begin(&self) {
        self.service.begin();
    }

    /// Releases the frame request. The tick calls this once it finds nothing outstanding.
    pub(crate) fn settle(&self) {
        self.held.set(false);
        drop(self.pending.take());
    }

    /// Returns `true` when nothing is waiting for the next tick.
    ///
    /// The frame request is derived from this: a doorbell that is never idle is a window
    /// that never parks.
    #[must_use]
    pub fn idle(&self) -> bool {
        self.head.get() == self.tail.get()
            && !self.stop_inertia.get()
            && !self.contacts.iter().any(|c| c.live.get() && c.moved.get())
    }
}

/// Extracts the pointer id from a message's `wparam`. `GET_POINTERID_WPARAM` is a C macro
/// with no metadata.
const fn pointer_id(wparam: usize) -> u32 {
    (wparam & 0xffff) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wparam(id: u32, flags: i32) -> usize {
        (id as usize) | ((flags as u32 as usize) << 16)
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
        assert_eq!(pointer_id(wparam(7, 0)), 7);
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
        for key in 0..5u16 {
            bell.push(InputEvent::Key(KeyEvent {
                kind: KeyKind::Down,
                key,
                repeat: false,
                mods: Mods::default(),
            }));
        }
        for key in 0..5u16 {
            let Some(InputEvent::Key(event)) = bell.pop() else {
                panic!("the ring lost a record");
            };
            assert_eq!(event.key, key);
        }
        assert_eq!(bell.pop(), None);
    }

    #[test]
    fn overflow_drops_the_oldest_and_says_so() {
        let bell = Doorbell::new();
        let push = |key| {
            bell.push(InputEvent::Key(KeyEvent {
                kind: KeyKind::Down,
                key,
                repeat: false,
                mods: Mods::default(),
            }));
        };
        for key in 0..RING_CAPACITY as u16 {
            push(key);
        }
        assert_eq!(bell.health().dropped, 0);
        assert_eq!(bell.health().peak, RING_CAPACITY as u32);
        // One past capacity. `debug_assert!` panics in a debug build, so the drop is checked
        // only where the push returned; the count reports it in either build.
        let overflowed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| push(999)));
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
        bell.claim(3, true);
        assert!(bell.is_down(3));
        let mut moved = Vec::new();
        bell.take_moved(&mut moved);
        assert_eq!(moved, [3]);
        // Taking clears the bit, so a frame with no motion reports none.
        bell.take_moved(&mut moved);
        assert!(moved.is_empty());

        bell.release(3);
        assert!(!bell.is_down(3));
        bell.take_moved(&mut moved);
        assert!(moved.is_empty());
    }

    #[test]
    fn a_canceled_up_is_a_cancel() {
        // Exercised over the flag word rather than through `transition`, which needs a live
        // pointer id to resolve a point.
        let flags = PointerFlags::from_wparam(wparam(1, POINTER_FLAG_CANCELED));
        assert!(flags.canceled());
        let kind = if flags.canceled() {
            EventKind::Cancel
        } else {
            EventKind::Up
        };
        assert_eq!(kind, EventKind::Cancel);
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
        bell.claim(1, false);
        assert!(!bell.idle(), "a moved contact is not idle");
        let mut moved = Vec::new();
        bell.take_moved(&mut moved);
        assert!(bell.idle());
        bell.stop_inertia();
        assert!(!bell.idle());
        assert!(bell.take_stop_inertia());
        assert!(bell.idle());
    }
}
