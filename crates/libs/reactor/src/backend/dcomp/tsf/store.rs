//! The TSF document protocol core — binding-independent and headless-testable.
//!
//! Everything a `ITextStoreACP` implementation gets subtly wrong lives here: the
//! document-lock state machine (sync grant, async queue, re-entrant upgrade), the
//! "never notify the sink while it holds a lock" discipline, and the ACP text /
//! selection / insert operations. None of it touches a Windows type — the COM
//! layer in `acp.rs` is a pure translator over this. That split is the whole
//! point: TSF cannot run headless, but this state machine can, and it is the
//! part that has historically broken (see the module tests).

use std::cell::RefCell;
use std::collections::VecDeque;

use super::{lock, DocSelection};

/// What the store needs from the backend's editor. Every offset is an ACP
/// position — a UTF-16 code-unit index, identical to `editor::Editor::buf`
/// indexing, so nothing is re-mapped at the boundary.
///
/// Object-safe on purpose: the store holds it as `Rc<RefCell<dyn TsfDocument>>`
/// and the backend supplies a thin adapter that reaches the focused editor. The
/// store never keeps a borrow across a COM call-out, so a `&mut` method here is
/// only ever entered from a released-lock or granted-lock callback.
pub trait TsfDocument {
    // ── content ──────────────────────────────────────────────────────────────
    /// Length of the document in UTF-16 code units.
    fn text_len(&self) -> usize;
    /// Append the units in `[start, end)` (already clamped by the caller) to
    /// `out`. Split from a returning method so the store can size the TIP's
    /// buffer without a second copy.
    fn copy_text(&self, start: usize, end: usize, out: &mut Vec<u16>);
    /// Replace `[start, end)` with `text`; the caret/anchor collapse to the end
    /// of the inserted run. Mirrors `editor::Editor::ime_replace` / `insert`.
    fn replace(&mut self, start: usize, end: usize, text: &[u16]);

    // ── selection ────────────────────────────────────────────────────────────
    fn selection(&self) -> DocSelection;
    fn set_selection(&mut self, sel: DocSelection);

    // ── status ───────────────────────────────────────────────────────────────
    /// A text field currently has focus and can take input. When false the store
    /// reports an empty, read-only document and refuses locks — a TIP must never
    /// see a live document with no focused field.
    fn is_enabled(&self) -> bool;
    /// The focused field rejects edits (rare — e.g. a disabled field that keeps
    /// focus). Write locks are still granted; the writes fail `TS_E_READONLY`.
    fn is_read_only(&self) -> bool;

    // ── geometry (screen pixels) ─────────────────────────────────────────────
    /// Bounding rect of ACP `[start, end)` in screen pixels, or `None` if the
    /// field has not been laid out yet (→ `TS_E_NOLAYOUT`, which is a normal,
    /// transient answer a TIP retries after the next layout pass).
    fn range_rect(&self, start: usize, end: usize) -> Option<super::DocRect>;
    /// The field's client area in screen pixels (`GetScreenExt`).
    fn screen_rect(&self) -> Option<super::DocRect>;

    // ── composition (store → editor) ─────────────────────────────────────────
    //
    // Composing *text* is not passed here: it arrives through `replace` like any
    // other TSF edit. These carry only the composing **span**, which is what the
    // underline paints over and what the §7.2 guard reads.
    /// The composing run now covers `[start, start + len)`. Called at the
    /// composition's start and on every subsequent range change.
    fn composition_update(&mut self, start: usize, len: usize);
    /// The composition ended (committed or cancelled): clear the composing span.
    /// The committed text, if any, arrived through `replace` before this call.
    fn composition_end(&mut self);
}

/// The advise-sink side, abstracted so the lock machine is testable without a
/// real `ITextStoreACPSink`. `acp.rs` implements this over the COM sink; the
/// tests implement it with a recorder. Methods take `&self` because a sink
/// call-out may legally re-enter the store (request a lock, read text), and no
/// store borrow may be held across it.
pub trait StoreSink {
    /// Returns the session `HRESULT` (raw `i32`) the TIP produced — written back
    /// into `RequestLock`'s out-parameter for a synchronous grant.
    fn on_lock_granted(&self, flags: u32) -> i32;
    fn on_text_change(&self, change: TextChange);
    fn on_selection_change(&self);
    fn on_layout_change(&self);
}

/// An ACP text delta (`TS_TEXTCHANGE`): the replaced range and its new end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextChange {
    pub start: i32,
    pub old_end: i32,
    pub new_end: i32,
}

/// Plain mirror of the ACP-store `HRESULT` sentinels, mapped to `HRESULT` in
/// `acp.rs`. Keeping the core error-typed rather than `HRESULT`-typed is what
/// lets it build with no Windows dependency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcpError {
    InvalidPos,
    // The lock is enforced at the COM boundary in `acp.rs`, so the core never
    // raises this — kept whole: this enum mirrors the ACP sentinel set.
    #[allow(dead_code)]
    NoLock,
    NoSelection,
    ReadOnly,
}

/// A change notification deferred until the document lock is released.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notify {
    Text(TextChange),
    Selection,
    Layout,
}

/// The result of a `RequestLock`, in COM-free terms. `acp.rs` writes the
/// out-parameter and picks the `RequestLock` return from this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockResult {
    /// A lock was granted synchronously; `session_hr` is what the TIP returned
    /// from `OnLockGranted` and goes into `*phrSession`.
    Granted { session_hr: i32 },
    /// A synchronous lock was refused because one is already held
    /// (`*phrSession = TS_E_SYNCHRONOUS`).
    Sync,
    /// The request was queued behind the active lock (`*phrSession = TS_S_ASYNC`).
    Queued,
}

/// Outcome of the internal "may I begin a lock now?" check.
enum BeginLock {
    Granted(u32),
    Sync,
    Queued,
}

/// The document-protocol state. Owns *only* protocol state — never the document
/// itself (the COM object holds the `TsfDocument` separately), so a re-entrant
/// TIP call-out can borrow the document while this stays untouched.
pub struct TextStoreCore {
    /// Current lock: `0` = unlocked, else the granted read/write flags.
    lock: u32,
    /// Re-entrant lock requests made while a lock was held, granted in order
    /// after the current one releases. Only an upgrade (read→readwrite) or a
    /// same-class re-request ever lands here in practice, but the queue is
    /// order-preserving regardless.
    lock_queue: VecDeque<u32>,
    /// App/backend-originated notifications that arrived while locked. Flushed,
    /// in order, only after the lock fully releases — a sink may never receive a
    /// change notification while it holds a lock (TSF requirement; violating it
    /// is a classic hang/crash).
    pending: Vec<Notify>,
    /// A read/write lock performed at least one edit, so the layout may have
    /// moved — emit one `OnLayoutChange` after release so the TIP repositions its
    /// candidate UI. Set by TIP-originated edits only.
    edit_during_lock: bool,
}

impl Default for TextStoreCore {
    fn default() -> Self {
        Self::new()
    }
}

impl TextStoreCore {
    pub fn new() -> Self {
        Self {
            lock: 0,
            lock_queue: VecDeque::new(),
            pending: Vec::new(),
            edit_during_lock: false,
        }
    }

    pub fn is_locked(&self) -> bool {
        self.lock != 0
    }

    pub fn has_write_lock(&self) -> bool {
        lock::is_write(self.lock)
    }

    /// Decide whether a lock may begin now, mutating lock state for the grant /
    /// queue cases. `SYNC`-while-locked is refused; async-while-locked queues.
    fn begin_lock(&mut self, flags: u32) -> BeginLock {
        if self.lock != 0 {
            // A lock is held. Only the re-entrant path reaches here — the TIP is
            // inside its own `OnLockGranted`.
            if flags & lock::SYNC != 0 {
                return BeginLock::Sync;
            }
            self.lock_queue.push_back(lock::rw(flags));
            return BeginLock::Queued;
        }
        self.lock = lock::rw(flags);
        BeginLock::Granted(self.lock)
    }

    /// Pop the next queued re-entrant lock, installing it as current.
    fn next_queued(&mut self) -> Option<u32> {
        let f = self.lock_queue.pop_front()?;
        self.lock = f;
        Some(f)
    }

    /// Release the lock and return the notifications to flush now (in order),
    /// including a trailing `Layout` if any edit happened under the lock.
    fn release_lock(&mut self) -> Vec<Notify> {
        self.lock = 0;
        let mut out = std::mem::take(&mut self.pending);
        if self.edit_during_lock {
            self.edit_during_lock = false;
            out.push(Notify::Layout);
        }
        out
    }

    /// Route a backend/app-originated notification: emit now if unlocked, else
    /// defer. Returns the notification to emit immediately, or `None` if queued.
    fn record_notify(&mut self, n: Notify) -> Option<Notify> {
        if self.lock != 0 {
            self.pending.push(n);
            None
        } else {
            Some(n)
        }
    }
}

/// The `RequestLock` state machine. This is the shipping implementation — the
/// COM `RequestLock` and the tests both drive it, so what the tests exercise is
/// exactly what a TIP hits.
///
/// Contract enforced here:
/// * No sink → the caller must not reach this (checked in `acp.rs`, which
///   returns `E_FAIL`); a sink is always present on entry.
/// * Not locked → grant (sync or async both grant inline by calling
///   `OnLockGranted`), then drain any re-entrant upgrade requests, then release
///   and flush deferred notifications.
/// * Already locked + sync → `Sync` (`TS_E_SYNCHRONOUS`).
/// * Already locked + async → `Queued` (`TS_S_ASYNC`); serviced when the current
///   lock releases.
///
/// Crucially, **no `core` borrow is held across `sink.on_lock_granted`** — the
/// TIP may re-enter (`RequestLock`, `GetText`, …) during that call, and each of
/// those takes its own short borrow.
pub fn run_request_lock(
    core: &RefCell<TextStoreCore>,
    sink: &dyn StoreSink,
    flags: u32,
) -> LockResult {
    let granted_rw = match core.borrow_mut().begin_lock(flags) {
        BeginLock::Sync => return LockResult::Sync,
        BeginLock::Queued => return LockResult::Queued,
        BeginLock::Granted(rw) => rw,
    };

    // Borrow released — the TIP owns the document for the duration of this call
    // and may call back into the store re-entrantly.
    let session_hr = sink.on_lock_granted(granted_rw);

    // Drain re-entrant upgrade/re-request locks, each a fresh `OnLockGranted`.
    loop {
        let next = core.borrow_mut().next_queued();
        match next {
            Some(f) => {
                let _ = sink.on_lock_granted(f);
            }
            None => break,
        }
    }

    let pending = core.borrow_mut().release_lock();
    for n in pending {
        emit(sink, n);
    }

    LockResult::Granted { session_hr }
}

/// Emit one released-lock notification on the sink.
fn emit(sink: &dyn StoreSink, n: Notify) {
    match n {
        Notify::Text(c) => sink.on_text_change(c),
        Notify::Selection => sink.on_selection_change(),
        Notify::Layout => sink.on_layout_change(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Backend/app-originated change notifications (the direction opposite to a lock).
//
// Called when the *app* changes the document out from under the TIP (a forced
// `SetText` after the composition guard cleared, a UIA `SetValue`, a reducer
// correction). They must never fire while the sink holds a lock, so they route
// through `record_notify` and defer if locked.
// ─────────────────────────────────────────────────────────────────────────────

/// Report an app-originated text change to the TIP (deferred if locked).
pub fn notify_app_text_change(
    core: &RefCell<TextStoreCore>,
    sink: &dyn StoreSink,
    change: TextChange,
) {
    let emit_now = core.borrow_mut().record_notify(Notify::Text(change));
    if let Some(n) = emit_now {
        emit(sink, n);
    }
}

/// Report an app-originated selection change to the TIP (deferred if locked).
pub fn notify_app_selection_change(core: &RefCell<TextStoreCore>, sink: &dyn StoreSink) {
    let emit_now = core.borrow_mut().record_notify(Notify::Selection);
    if let Some(n) = emit_now {
        emit(sink, n);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ACP text / selection operations. Plain functions over a `TsfDocument`, called
// by `acp.rs` under a granted lock. Positions are validated here so a
// misbehaving TIP gets a clean `AcpError`, never an out-of-bounds panic.
// ─────────────────────────────────────────────────────────────────────────────

/// Clamp an ACP position argument. `end == -1` means "end of document"; a
/// position past the end is an error (TSF is strict about this).
fn resolve_pos(pos: i32, len: usize, allow_neg_one: bool) -> Result<usize, AcpError> {
    if pos == -1 && allow_neg_one {
        return Ok(len);
    }
    if pos < 0 {
        return Err(AcpError::InvalidPos);
    }
    let p = pos as usize;
    if p > len {
        return Err(AcpError::InvalidPos);
    }
    Ok(p)
}

/// `GetText`: copy the plain text of `[acp_start, acp_end)` (with `-1` = end),
/// returning the units and the next ACP position (`pacpNext`). The run array is
/// always a single PLAIN run, produced by `acp.rs` from the returned length.
pub fn get_text(
    doc: &dyn TsfDocument,
    acp_start: i32,
    acp_end: i32,
    max_units: u32,
) -> Result<(Vec<u16>, i32), AcpError> {
    let len = doc.text_len();
    let start = resolve_pos(acp_start, len, false)?;
    let end = resolve_pos(acp_end, len, true)?;
    if end < start {
        return Err(AcpError::InvalidPos);
    }
    let capped_end = (start + max_units as usize).min(end);
    let mut out = Vec::with_capacity(capped_end - start);
    doc.copy_text(start, capped_end, &mut out);
    Ok((out, capped_end as i32))
}

/// `GetSelection` (single, default selection). `None` → `TS_E_NOSELECTION`.
pub fn get_selection(doc: &dyn TsfDocument) -> Result<DocSelection, AcpError> {
    if !doc.is_enabled() {
        return Err(AcpError::NoSelection);
    }
    Ok(doc.selection())
}

/// `SetSelection` — validate the range lies within the document and apply it.
pub fn set_selection(doc: &mut dyn TsfDocument, sel: DocSelection) -> Result<(), AcpError> {
    let len = doc.text_len();
    if sel.start > len || sel.end > len || sel.end < sel.start {
        return Err(AcpError::InvalidPos);
    }
    doc.set_selection(sel);
    Ok(())
}

/// `SetText`: replace `[acp_start, acp_end)` with `text`, leaving the selection
/// on the inserted range. Requires a write lock (checked by the caller) and
/// records the edit for the post-lock layout notification. Returns the delta.
pub fn set_text(
    core: &RefCell<TextStoreCore>,
    doc: &mut dyn TsfDocument,
    acp_start: i32,
    acp_end: i32,
    text: &[u16],
) -> Result<TextChange, AcpError> {
    if doc.is_read_only() {
        return Err(AcpError::ReadOnly);
    }
    let len = doc.text_len();
    let start = resolve_pos(acp_start, len, false)?;
    let end = resolve_pos(acp_end, len, true)?;
    if end < start {
        return Err(AcpError::InvalidPos);
    }
    doc.replace(start, end, text);
    let new_end = start + text.len();
    // Leave the selection covering the freshly inserted run (TSF convention).
    doc.set_selection(DocSelection { start, end: new_end, reversed: false });
    core.borrow_mut().edit_during_lock = true;
    Ok(TextChange { start: start as i32, old_end: end as i32, new_end: new_end as i32 })
}

/// `InsertTextAtSelection`: replace the current selection with `text`. With
/// `QUERYONLY` nothing changes and only the prospective range is reported. The
/// resulting selection collapses to the end of the inserted run. Returns
/// `(start, end, change)` where `change` is `None` for a query-only call.
pub fn insert_at_selection(
    core: &RefCell<TextStoreCore>,
    doc: &mut dyn TsfDocument,
    text: &[u16],
    flags: u32,
) -> Result<(i32, i32, Option<TextChange>), AcpError> {
    let sel = doc.selection();
    let (a, b) = (sel.start.min(sel.end), sel.start.max(sel.end));

    if flags & super::insert::QUERYONLY != 0 {
        // Report where the text *would* land without editing.
        let end = a + text.len();
        return Ok((a as i32, end as i32, None));
    }
    if doc.is_read_only() {
        return Err(AcpError::ReadOnly);
    }
    doc.replace(a, b, text);
    let new_end = a + text.len();
    doc.set_selection(DocSelection::caret(new_end));
    core.borrow_mut().edit_during_lock = true;
    let change = TextChange { start: a as i32, old_end: b as i32, new_end: new_end as i32 };
    Ok((a as i32, new_end as i32, Some(change)))
}

