//! Raw Text Services Framework (TSF) text store for the front (HWND) thread —
//! **the only text-input path there is**.
//!
//! A real `ITfThreadMgr2` + hand-implemented `ITextStoreACP` document. There is
//! deliberately no IMM32 fallback: `imm32.dll` is unavailable to a packaged /
//! Store app, the IMM32 path silently drops Win+H voice typing, dictation and
//! several non-CJK TIP features, and a second composition path that only some
//! fields take is exactly how IME bugs get shipped. See
//! `docs/INPUT-OFFTHREAD-DESIGN.md` §7.1 (text detail) and §7.2 (the text
//! authority model + composition guard).
//!
//! ## Shape of the wiring
//!
//! The document the store reflects is **the backend itself**: `DCompBackend`
//! implements [`TsfDocument`] over whichever editor currently has focus (see
//! [`doc`]), and the host hands the store its own `Rc<RefCell<DCompBackend>>`.
//! No mirror, no second copy of the text.
//!
//! That makes re-entrancy the load-bearing rule: a TIP call-out borrows the
//! backend, so **TSF may only ever be entered with no backend borrow held**.
//! Every entry point in [`bridge`] is called from an unborrowed context (the
//! pump, or straight after a dispatch's borrow scope ends), and everything the
//! backend wants to tell TSF is *pulled* by [`bridge::flush`] rather than pushed
//! from inside a borrow.
//!
//! ## Display attributes
//!
//! A TIP can request a styled clause underline (squiggly, coloured) through
//! `ITfDisplayAttributeMgr`. The text layout underneath exposes only a boolean
//! underline, so resolving those attributes would produce data nothing could
//! draw: the composing run gets one plain underline and the display-attribute
//! walk is left unwritten until the paint side can honour it.
//!
//! ## `ITextStoreACP` vs `ITextStoreACP2`
//!
//! §7.1 names `ITextStoreACP2`. This module implements **`ITextStoreACP` (v1)**,
//! a deliberate, documented choice:
//!
//! * Both reference implementations this design is modelled on — Chromium
//!   `ui/base/ime/win/tsf_text_store.cc` and Mozilla `TSFTextStore.cpp` —
//!   implement v1.
//! * v1 exposes `GetWnd` and window-relative extents, which is exactly right for
//!   a store that owns a real top-level HWND on the front thread (R1's model).
//!   `ITextStoreACP2` is the *windowless* variant (it drops `GetWnd`); its value
//!   is for controls with no HWND, which is not our case.
//! * The fork's generated bindings already wire v1 as implementable.
//!
//! The document protocol (locks, ACP offsets, change notifications) is identical
//! between the two; if a windowless host ever needs ACP2 the state machine in
//! [`store`] ports unchanged.
//!
//! ## Module shape
//!
//! * [`store`] — the binding-independent protocol core: the document-lock state
//!   machine, text / selection / insert operations over a [`TsfDocument`], and
//!   change-notification queueing. This is where TSF stores historically break,
//!   so it is plain Rust with exhaustive headless tests and no COM types.
//! * [`acp`] — the `#[implement]`'d `ITextStoreACP` COM object, a thin translator
//!   between the ACP vtable and [`store`], plus the `ITextStoreACPSink` adapter.
//! * [`thread_mgr`] — `ITfThreadMgr2` activation, document-manager / context
//!   creation + push, the composition-sink advise, and focus association.
//! * [`comp_sink`] — the `#[implement]`'d `ITfContextOwnerCompositionSink`:
//!   composition boundaries and the composing range, in ACP offsets.
//! * [`doc`] — `impl TsfDocument for DCompBackend`: the focused editor as a TSF
//!   document (text, selection, geometry, composition span).
//! * [`bridge`] — the front-thread glue: activation lifetime, keystroke
//!   pre-emption from the pump, and the pull-based change flush.

pub(crate) mod acp;
pub(crate) mod bridge;
pub(crate) mod comp_sink;
pub(crate) mod doc;
pub(crate) mod store;
pub(crate) mod thread_mgr;

pub(crate) use acp::TextInput;
pub(crate) use store::TsfDocument;

// ─────────────────────────────────────────────────────────────────────────────
// Plain data the trait exchanges. All text offsets are ACP positions — UTF-16
// code-unit indices into the document, matching `editor::Editor::buf` exactly,
// so there is no re-indexing at the seam.
// ─────────────────────────────────────────────────────────────────────────────

/// A selection / caret as ACP offsets. An empty selection (`start == end`) is a
/// caret. `reversed` records the active end (caret at `start`) so the TIP can
/// keep the anchor stable across shift-extension — mirrors `TS_SELECTIONSTYLE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocSelection {
    pub start: usize,
    pub end: usize,
    pub reversed: bool,
}

impl DocSelection {
    pub fn caret(at: usize) -> Self {
        Self { start: at, end: at, reversed: false }
    }
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// A rectangle in **screen pixels** (physical), the coordinate space every ACP
/// extent method (`GetTextExt`, `GetScreenExt`) reports in. The document trait
/// owns the DIP→pixel + client→screen mapping so the store stays unit-agnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

// ─────────────────────────────────────────────────────────────────────────────
// TSF protocol constants.
//
// These are ABI values from `textstor.h` / `msctf.h`. The lock flags and run
// type are used by the binding-independent core in `store`, so they live here
// (not behind the generated bindings) to keep that core buildable and testable
// with no Windows types. The `HRESULT` sentinels are used by the COM layer.
// ─────────────────────────────────────────────────────────────────────────────

/// `dwLockFlags` bits (`RequestLock` / `OnLockGranted`). `TS_LF_READWRITE` is the
/// pair, matching how TSF combines them; a write lock always implies read.
pub mod lock {
    pub const SYNC: u32 = 0x1;
    pub const READ: u32 = 0x2;
    pub const WRITE: u32 = 0x4;
    pub const READWRITE: u32 = READ | WRITE; // 0x6

    /// The read/write portion of a lock-flags word (strips `SYNC`).
    #[inline]
    pub const fn rw(flags: u32) -> u32 {
        flags & READWRITE
    }
    /// Whether a lock word grants write access.
    #[inline]
    pub const fn is_write(flags: u32) -> bool {
        flags & WRITE != 0
    }
}

/// `SetText` / `InsertTextAtSelection` flag bits.
pub(crate) mod set_text {
    /// `TS_ST_CORRECTION` — the edit is a reconversion, not a new keystroke.
    pub const CORRECTION: u32 = 0x1;
}

/// `InsertTextAtSelection` query flags.
pub mod insert {
    /// `TS_IAS_NOQUERY` — perform the insert, do not report the resulting range.
    pub const NOQUERY: u32 = 0x1;
    /// `TS_IAS_QUERYONLY` — report where an insert *would* land; change nothing.
    pub const QUERYONLY: u32 = 0x2;
}

/// `TsRunType` — we only ever expose one PLAIN run over the whole document (no
/// hidden / opaque runs), so `GetText`'s run array is always a single entry.
pub(crate) const TS_RT_PLAIN: i32 = 0;

/// `TsLayoutCode` for `OnLayoutChange`.
pub(crate) mod layout_code {
    pub const CHANGE: i32 = 1; // TS_LC_CHANGE
}

/// `GetStatus` static flags. `TS_SS_NOHIDDENTEXT` promises the store never
/// returns hidden runs — true here, and it lets a TIP skip hidden-text probes.
pub(crate) const TS_SS_NOHIDDENTEXT: u32 = 0x8;
/// `GetStatus` dynamic flag: the document is read-only right now.
pub(crate) const TS_SD_READONLY: u32 = 0x1;

/// `ulIndex` sentinel meaning "the default selection / caret" in
/// `GetSelection`/`SetSelection`.
pub(crate) const TS_DEFAULT_SELECTION: u32 = 0xFFFF_FFFF;

/// `TsActiveSelEnd` values recorded in `TS_SELECTIONSTYLE::ase`.
pub(crate) mod active_end {
    pub const START: i32 = 1; // TS_AE_START
    pub const END: i32 = 2; // TS_AE_END
}

/// ACP-store `HRESULT` sentinels (`textstor.h`). Returned raw from the COM layer
/// — a misbehaving TIP must get the exact documented code, never a panic.
pub(crate) mod hr {
    use windows_core::HRESULT;
    /// An ACP position argument was out of range.
    pub const TS_E_INVALIDPOS: HRESULT = HRESULT(0x8004_0200u32 as i32);
    /// The document is not locked (a lock-only method was called without one).
    pub const TS_E_NOLOCK: HRESULT = HRESULT(0x8004_0201u32 as i32);
    /// No selection exists to report.
    pub const TS_E_NOSELECTION: HRESULT = HRESULT(0x8004_0205u32 as i32);
    /// Layout is not yet available for the requested range (field not measured).
    pub const TS_E_NOLAYOUT: HRESULT = HRESULT(0x8004_0206u32 as i32);
    /// A synchronous lock was requested while the document was already locked.
    pub const TS_E_SYNCHRONOUS: HRESULT = HRESULT(0x8004_0208u32 as i32);
    /// A write lock was requested against a read-only document.
    pub const TS_E_READONLY: HRESULT = HRESULT(0x8004_0209u32 as i32);
    /// A lock was granted asynchronously (queued behind the current one).
    pub const TS_S_ASYNC: HRESULT = HRESULT(0x0004_0300);
}
