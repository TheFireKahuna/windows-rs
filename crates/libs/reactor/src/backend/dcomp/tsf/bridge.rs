//! The front-thread glue between the backend and TSF: activation lifetime,
//! keystroke pre-emption, and the change flush.
//!
//! ## The one rule
//!
//! Entering TSF re-enters us: `SetFocus`, a keystroke, or a change notification
//! all give a TIP the chance to call straight back into the text store, which
//! borrows the backend. So **every call into TSF happens with no backend borrow
//! held**. There are exactly two entry points and both are in the pump, between
//! messages:
//!
//! * [`filter_key`] — before `TranslateMessage`, so a TIP claims composition
//!   keys before they become `WM_CHAR`s.
//! * [`flush`] — after `DispatchMessageW`, so whatever that message changed is
//!   reported once the WndProc's borrow is long gone.
//!
//! ## Pull, don't push
//!
//! Nothing in the backend calls TSF. [`flush`] instead *compares* the focused
//! field, its text and its selection against what TSF was last told, and reports
//! the difference. That is why no `note_text_change` calls are sprinkled through
//! the editing paths: any edit — keystroke, paste, UIA `SetValue`, an app write
//! replayed from a commit — is picked up by the same comparison, and a path that
//! forgets to announce itself cannot exist.
//!
//! The one thing the comparison must not do is report a TIP's own edit back to
//! it, so the store marks those with [`note_store_edit`] and the next flush
//! resyncs its snapshot silently.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::acp::{TextInput, TextStore};
use super::store::TextChange;
use super::thread_mgr::TsfActivation;
use super::{DocSelection, TsfDocument};
use crate::backend::dcomp::DCompBackend;
use crate::system_bindings::{
    ITfKeystrokeMgr, LPARAM, MSG, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WPARAM,
};
use crate::ControlId;

thread_local! {
    /// The live activation, or `None` when TSF failed to start (the app still
    /// runs; it just has no IME / dictation).
    static TSF: RefCell<Option<Live>> = const { RefCell::new(None) };
    /// What TSF was last told the document is. `None` = no focused field.
    static SEEN: RefCell<Option<Seen>> = const { RefCell::new(None) };
    /// Set by the store when a TIP edited the document itself. A `Cell`, not a
    /// field of [`Live`], because the store sets it from inside a TSF call-out
    /// that [`flush`] may be in the middle of.
    static STORE_EDITED: Cell<bool> = const { Cell::new(false) };
}

/// The pieces of a live activation, cloned out of the thread-local under a short
/// borrow so no borrow is held across a COM call.
#[derive(Clone)]
struct Live {
    act: Rc<TsfActivation>,
    input: TextInput,
    keystroke: Option<ITfKeystrokeMgr>,
    backend: Rc<RefCell<DCompBackend>>,
}

/// The document as TSF last saw it.
struct Seen {
    id: ControlId,
    text: Vec<u16>,
    sel: DocSelection,
}

/// Clone the live activation out of the thread-local. The borrow ends before
/// this returns, which is the whole point.
fn live() -> Option<Live> {
    TSF.with(|t| t.borrow().clone())
}

/// Stand TSF up over `backend`, owned by window `hwnd`. Call once on the front
/// thread after the window and backend exist and the apartment is initialised.
/// A failure is logged and left as "no TSF": text still types, but there is no
/// IME, dictation or handwriting.
pub(crate) fn activate(backend: Rc<RefCell<DCompBackend>>, hwnd: isize) {
    let doc: Rc<RefCell<dyn TsfDocument>> = backend.clone();
    let store = TextStore::new(doc, hwnd);
    match TsfActivation::new(store) {
        Ok(act) => {
            let live = Live {
                input: act.input(),
                keystroke: act.keystroke(),
                act: Rc::new(act),
                backend,
            };
            TSF.with(|t| *t.borrow_mut() = Some(live));
        }
        Err(e) => {
            // Not fatal, but not silent either: no IME is a large, invisible
            // degradation and the reason belongs where a bug report can find it.
            eprintln!("windows-reactor: TSF activation failed ({e}); no IME/dictation input");
        }
    }
}

/// Tear TSF down (window destroy). Dropping the activation pops the context and
/// deactivates the thread manager.
pub(crate) fn shutdown() {
    TSF.with(|t| *t.borrow_mut() = None);
    SEEN.with(|s| *s.borrow_mut() = None);
}

/// Marks the change the store is currently making as already known to TSF, so
/// the next [`flush`] resyncs its snapshot without reporting the TIP's own edit
/// back to it.
pub(crate) fn note_store_edit() {
    STORE_EDITED.with(|c| c.set(true));
}

// ─────────────────────────────────────────────────────────────────────────────
// Keystroke pre-emption
// ─────────────────────────────────────────────────────────────────────────────

/// Offer a key message to the active TIP before the pump translates it.
/// Returns `true` when the TIP consumed it, in which case the message must be
/// neither translated (no `WM_CHAR`) nor dispatched.
///
/// Only offered while a text field has focus: a TIP that ate our arrow keys or
/// Tab on a slider would be a far worse bug than a missed composition key.
pub(crate) fn filter_key(msg: &MSG) -> bool {
    let down = matches!(msg.message, WM_KEYDOWN | WM_SYSKEYDOWN);
    let up = matches!(msg.message, WM_KEYUP | WM_SYSKEYUP);
    if !down && !up {
        return false;
    }
    let Some(live) = live() else { return false };
    let Some(km) = live.keystroke.as_ref() else {
        return false;
    };
    // A short, non-panicking borrow: during teardown or a re-entrant moment the
    // honest answer is "not ours", which routes the key the ordinary way.
    if !live
        .backend
        .try_borrow()
        .is_ok_and(|b| b.has_text_focus())
    {
        return false;
    }
    let (w, l) = (msg.wParam as WPARAM, msg.lParam as LPARAM);
    // SAFETY: live keystroke manager on its own (front) thread.
    let eaten = unsafe {
        let test = if down {
            km.TestKeyDown(w, l)
        } else {
            km.TestKeyUp(w, l)
        };
        if !test.is_ok_and(|b| b.as_bool()) {
            return false;
        }
        if down { km.KeyDown(w, l) } else { km.KeyUp(w, l) }
    };
    eaten.is_ok_and(|b| b.as_bool())
}

// ─────────────────────────────────────────────────────────────────────────────
// Change flush
// ─────────────────────────────────────────────────────────────────────────────

/// Report to TSF whatever the last message changed: focus in or out of a text
/// field, a text edit, or a caret/selection move. Cheap and idempotent when
/// nothing changed, which is the overwhelmingly common case.
pub(crate) fn flush() {
    let Some(live) = live() else { return };
    // Note there is deliberately no "is a sink advised yet?" gate: TSF advises
    // the store's sink in response to the very `SetFocus` this function makes,
    // so gating on it would mean the sink is never advised at all. The notify
    // calls below are already no-ops until one exists.

    // Snapshot under a short borrow, then release it: everything below can call
    // into TSF and be re-entered.
    let Ok(b) = live.backend.try_borrow() else {
        return;
    };
    let now = b.focused_editable().map(|id| Seen {
        id,
        text: b.node(id).and_then(|n| n.editor.as_ref()).map(|e| e.buf.clone()).unwrap_or_default(),
        sel: b.selection(),
    });
    drop(b);

    // The TIP made this change; it does not need to hear its own edit back.
    if STORE_EDITED.with(|c| c.replace(false)) {
        SEEN.with(|s| *s.borrow_mut() = now);
        return;
    }

    let prev = SEEN.with(|s| s.borrow_mut().take());
    match (prev, now) {
        (None, None) => {}
        // A text field took focus: hand TSF the document.
        (None, Some(now)) => {
            let _ = live.act.set_focus(true);
            SEEN.with(|s| *s.borrow_mut() = Some(now));
        }
        // The last text field blurred: there is no document to compose into.
        (Some(_), None) => {
            let _ = live.act.set_focus(false);
        }
        (Some(prev), Some(now)) => {
            if prev.id != now.id {
                // Focus moved field-to-field. TSF's document is "whatever is
                // focused", so from its side the whole document was replaced.
                live.input.notify_text_change(TextChange {
                    start: 0,
                    old_end: prev.text.len() as i32,
                    new_end: now.text.len() as i32,
                });
                live.input.notify_selection_change();
            } else {
                if prev.text != now.text {
                    live.input.notify_text_change(diff(&prev.text, &now.text));
                }
                if prev.sel != now.sel {
                    live.input.notify_selection_change();
                }
            }
            SEEN.with(|s| *s.borrow_mut() = Some(now));
        }
    }
}

/// The ACP delta between two documents, aligned by their common prefix and
/// suffix — the same alignment the editor's caret mapping uses, so a one-word
/// app correction is reported as one word changed, not as the whole field.
fn diff(old: &[u16], new: &[u16]) -> TextChange {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let max_suffix = (old.len() - prefix).min(new.len() - prefix);
    let mut suffix = 0;
    while suffix < max_suffix && old[old.len() - 1 - suffix] == new[new.len() - 1 - suffix] {
        suffix += 1;
    }
    TextChange {
        start: prefix as i32,
        old_end: (old.len() - suffix) as i32,
        new_end: (new.len() - suffix) as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    /// An insertion is reported as the inserted span alone, not the whole field.
    #[test]
    fn diff_reports_the_changed_span() {
        let c = diff(&units("hello world"), &units("hello brave world"));
        assert_eq!((c.start, c.old_end, c.new_end), (6, 6, 12));
    }

    /// A deletion reports an old range collapsing to an empty new one.
    #[test]
    fn diff_reports_a_deletion() {
        let c = diff(&units("abcdef"), &units("adef"));
        assert_eq!((c.start, c.old_end, c.new_end), (1, 3, 1));
    }

    /// A replacement of the whole buffer has no common affix to trim.
    #[test]
    fn diff_reports_a_wholesale_replacement() {
        let c = diff(&units("abc"), &units("xyz"));
        assert_eq!((c.start, c.old_end, c.new_end), (0, 3, 3));
    }
}
