//! `backend/dcomp/tsf` — the TSF text store's binding-independent protocol core.
//!
//! TSF cannot run headless (it needs a message pump and a real TIP), so these
//! tests drive the state machine directly through the test seam: a mock document,
//! a recording sink, and — for the re-entrant cases — a sink that calls back into
//! the store exactly as a TIP does from inside `OnLockGranted`. What is exercised
//! here is the shipping `run_request_lock` / text-op code, not a copy of it.

use std::cell::RefCell;
use std::rc::Rc;

use windows_reactor::dcomp_test_api::tsf::{
    get_selection, get_text, insert, insert_at_selection, lock, notify_app_selection_change,
    notify_app_text_change, run_request_lock, set_selection, set_text, underline_from_fields,
    AcpError, Composition, DocRect, DocSelection, InputScope, LockResult, StoreSink, TextChange,
    TextStoreCore, TsfDocument, UnderlineStyle, TF_LS_DASH, TF_LS_DOT, TF_LS_NONE, TF_LS_SOLID,
    TF_LS_SQUIGGLE,
};

// ── A minimal in-memory document ─────────────────────────────────────────────

struct MockDoc {
    buf: Vec<u16>,
    sel: DocSelection,
    enabled: bool,
    read_only: bool,
    laid_out: bool,
}

impl MockDoc {
    fn new(s: &str) -> Self {
        let buf: Vec<u16> = s.encode_utf16().collect();
        let end = buf.len();
        Self { buf, sel: DocSelection::caret(end), enabled: true, read_only: false, laid_out: true }
    }
    fn text(&self) -> String {
        String::from_utf16_lossy(&self.buf)
    }
}

impl TsfDocument for MockDoc {
    fn text_len(&self) -> usize {
        self.buf.len()
    }
    fn copy_text(&self, start: usize, end: usize, out: &mut Vec<u16>) {
        out.extend_from_slice(&self.buf[start..end]);
    }
    fn replace(&mut self, start: usize, end: usize, text: &[u16]) {
        self.buf.splice(start..end, text.iter().copied());
    }
    fn selection(&self) -> DocSelection {
        self.sel
    }
    fn set_selection(&mut self, sel: DocSelection) {
        self.sel = sel;
    }
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    fn is_read_only(&self) -> bool {
        self.read_only
    }
    fn input_scope(&self) -> InputScope {
        InputScope::Default
    }
    fn range_rect(&self, _start: usize, _end: usize) -> Option<DocRect> {
        self.laid_out.then_some(DocRect { left: 0, top: 0, right: 8, bottom: 16 })
    }
    fn screen_rect(&self) -> Option<DocRect> {
        self.laid_out.then_some(DocRect { left: 0, top: 0, right: 100, bottom: 20 })
    }
    fn composition_begin(&mut self) {}
    fn composition_update(&mut self, _comp: Composition) {}
    fn composition_end(&mut self) {}
}

/// Records the sink call sequence; the lock-granted behaviour is scriptable so a
/// test can make the sink re-enter the store, exactly as a TIP would.
struct RecSink {
    log: RefCell<Vec<String>>,
    on_grant: RefCell<Option<Box<dyn Fn(u32)>>>,
}

impl RecSink {
    fn new() -> Rc<Self> {
        Rc::new(Self { log: RefCell::new(Vec::new()), on_grant: RefCell::new(None) })
    }
    fn log(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}

impl StoreSink for RecSink {
    fn on_lock_granted(&self, flags: u32) -> i32 {
        self.log.borrow_mut().push(format!("grant:{flags:#x}"));
        // Take the one-shot hook out for the duration so a re-entrant grant does
        // not recursively fire the same script.
        let hook = self.on_grant.borrow_mut().take();
        if let Some(h) = hook {
            h(flags);
        }
        0 // S_OK
    }
    fn on_text_change(&self, c: TextChange) {
        self.log.borrow_mut().push(format!("text:{}-{}->{}", c.start, c.old_end, c.new_end));
    }
    fn on_selection_change(&self) {
        self.log.borrow_mut().push("sel".into());
    }
    fn on_layout_change(&self) {
        self.log.borrow_mut().push("layout".into());
    }
}

fn core() -> Rc<RefCell<TextStoreCore>> {
    Rc::new(RefCell::new(TextStoreCore::new()))
}

// ── Lock state machine ───────────────────────────────────────────────────────

#[test]
fn sync_lock_grants_and_releases() {
    let core = core();
    let sink = RecSink::new();
    let r = run_request_lock(&core, &*sink, lock::SYNC | lock::READ);
    assert_eq!(r, LockResult::Granted { session_hr: 0 });
    assert!(!core.borrow().is_locked(), "lock must clear after the grant returns");
    // OnLockGranted receives the read/write portion only — SYNC is stripped.
    assert_eq!(sink.log(), vec!["grant:0x2"]);
}

#[test]
fn async_lock_grants_inline_when_free() {
    // An async request on a free document is granted immediately (OnLockGranted
    // called inline), not deferred.
    let core = core();
    let sink = RecSink::new();
    let r = run_request_lock(&core, &*sink, lock::READWRITE);
    assert_eq!(r, LockResult::Granted { session_hr: 0 });
    assert_eq!(sink.log(), vec!["grant:0x6"]);
}

#[test]
fn sync_lock_refused_while_locked() {
    let core = core();
    let sink = RecSink::new();
    {
        let core2 = Rc::clone(&core);
        let sink2 = Rc::clone(&sink);
        *sink.on_grant.borrow_mut() = Some(Box::new(move |_| {
            let r = run_request_lock(&core2, &*sink2, lock::SYNC | lock::READ);
            assert_eq!(r, LockResult::Sync, "a sync lock must be refused while locked");
        }));
    }
    run_request_lock(&core, &*sink, lock::SYNC | lock::READ);
}

#[test]
fn reentrant_readwrite_upgrade_is_queued_then_granted() {
    // Hold a READ lock; from inside, request READWRITE async (an upgrade). It must
    // queue (TS_S_ASYNC) and then be granted after the read grant returns — the
    // canonical re-entrant upgrade path.
    let core = core();
    let sink = RecSink::new();
    {
        let core2 = Rc::clone(&core);
        let sink2 = Rc::clone(&sink);
        *sink.on_grant.borrow_mut() = Some(Box::new(move |flags| {
            if flags == lock::READ {
                let r = run_request_lock(&core2, &*sink2, lock::READWRITE);
                assert_eq!(r, LockResult::Queued);
            }
        }));
    }
    run_request_lock(&core, &*sink, lock::READ);
    assert_eq!(sink.log(), vec!["grant:0x2", "grant:0x6"], "read grant, then the queued rw grant");
    assert!(!core.borrow().is_locked());
}

#[test]
fn app_notifications_defer_until_unlock_then_flush_in_order() {
    // While the sink holds a lock, an app-originated text + selection change must
    // NOT reach it; both flush, in order, after release.
    let core = core();
    let sink = RecSink::new();
    {
        let core2 = Rc::clone(&core);
        let sink2 = Rc::clone(&sink);
        *sink.on_grant.borrow_mut() = Some(Box::new(move |_| {
            notify_app_text_change(&core2, &*sink2, TextChange { start: 0, old_end: 0, new_end: 1 });
            notify_app_selection_change(&core2, &*sink2);
            assert_eq!(sink2.log(), vec!["grant:0x2"], "nothing delivered under the lock");
        }));
    }
    run_request_lock(&core, &*sink, lock::READ);
    assert_eq!(sink.log(), vec!["grant:0x2", "text:0-0->1", "sel"]);
}

#[test]
fn app_notification_emits_immediately_when_unlocked() {
    let core = core();
    let sink = RecSink::new();
    notify_app_selection_change(&core, &*sink);
    assert_eq!(sink.log(), vec!["sel"]);
}

#[test]
fn edit_under_lock_emits_one_layout_change_after_release() {
    let core = core();
    let doc = Rc::new(RefCell::new(MockDoc::new("ab")));
    let sink = RecSink::new();
    {
        let core2 = Rc::clone(&core);
        let doc2 = Rc::clone(&doc);
        *sink.on_grant.borrow_mut() = Some(Box::new(move |flags| {
            if lock::is_write(flags) {
                let text: Vec<u16> = "X".encode_utf16().collect();
                set_text(&core2, &mut *doc2.borrow_mut(), 0, 1, &text).unwrap();
            }
        }));
    }
    run_request_lock(&core, &*sink, lock::READWRITE);
    // The TIP's own edit produces no text/selection notify (it knows), but one
    // layout change follows so it can reposition its candidate window.
    assert_eq!(sink.log(), vec!["grant:0x6", "layout"]);
    assert_eq!(doc.borrow().text(), "Xb");
}

// ── Text / selection operations ──────────────────────────────────────────────

#[test]
fn get_text_clamps_and_reports_next() {
    let doc = MockDoc::new("hello");
    let (units, next) = get_text(&doc, 1, -1, 100).unwrap();
    assert_eq!(String::from_utf16_lossy(&units), "ello");
    assert_eq!(next, 5);
    let (units, next) = get_text(&doc, 0, -1, 2).unwrap();
    assert_eq!(String::from_utf16_lossy(&units), "he");
    assert_eq!(next, 2, "max_units caps both the copy and pacpNext");
}

#[test]
fn get_text_rejects_out_of_range() {
    let doc = MockDoc::new("hi");
    assert_eq!(get_text(&doc, 3, -1, 100), Err(AcpError::InvalidPos));
    assert_eq!(get_text(&doc, 2, 1, 100), Err(AcpError::InvalidPos));
}

#[test]
fn get_selection_reports_the_caret() {
    let mut doc = MockDoc::new("hello");
    doc.set_selection(DocSelection { start: 1, end: 3, reversed: true });
    assert_eq!(get_selection(&doc), Ok(DocSelection { start: 1, end: 3, reversed: true }));
}

#[test]
fn set_selection_validates_range() {
    let mut doc = MockDoc::new("hi");
    assert!(set_selection(&mut doc, DocSelection { start: 0, end: 2, reversed: false }).is_ok());
    assert_eq!(
        set_selection(&mut doc, DocSelection { start: 0, end: 3, reversed: false }),
        Err(AcpError::InvalidPos),
    );
}

#[test]
fn set_text_replaces_and_reports_delta() {
    let core = core();
    let mut doc = MockDoc::new("hello");
    let text: Vec<u16> = "XY".encode_utf16().collect();
    let change = set_text(&core, &mut doc, 1, 3, &text).unwrap();
    assert_eq!(doc.text(), "hXYlo");
    assert_eq!(change, TextChange { start: 1, old_end: 3, new_end: 3 });
    assert_eq!(doc.selection(), DocSelection { start: 1, end: 3, reversed: false });
}

#[test]
fn insert_at_selection_query_only_changes_nothing() {
    let core = core();
    let mut doc = MockDoc::new("hello");
    doc.set_selection(DocSelection { start: 1, end: 3, reversed: false });
    let text: Vec<u16> = "ZZZ".encode_utf16().collect();
    let (a, b, change) = insert_at_selection(&core, &mut doc, &text, insert::QUERYONLY).unwrap();
    assert_eq!((a, b), (1, 4));
    assert!(change.is_none());
    assert_eq!(doc.text(), "hello", "query-only must not edit");
}

#[test]
fn insert_at_selection_replaces_selection_and_collapses_caret() {
    let core = core();
    let mut doc = MockDoc::new("hello");
    doc.set_selection(DocSelection { start: 0, end: 2, reversed: false });
    let text: Vec<u16> = "AB".encode_utf16().collect();
    let (a, b, change) = insert_at_selection(&core, &mut doc, &text, 0).unwrap();
    assert_eq!((a, b), (0, 2));
    assert_eq!(change, Some(TextChange { start: 0, old_end: 2, new_end: 2 }));
    assert_eq!(doc.text(), "ABllo");
    assert_eq!(doc.selection(), DocSelection::caret(2));
}

#[test]
fn set_text_on_read_only_field_errors() {
    let core = core();
    let mut doc = MockDoc::new("x");
    doc.read_only = true;
    let text: Vec<u16> = "y".encode_utf16().collect();
    assert_eq!(set_text(&core, &mut doc, 0, 1, &text), Err(AcpError::ReadOnly));
}

// ── Composition guard + underline resolution ─────────────────────────────────

#[test]
fn composition_active_flag_tracks_the_guard() {
    let mut c = TextStoreCore::new();
    assert!(!c.composition_active());
    c.set_composition_active(true);
    assert!(c.composition_active(), "the §7.2 guard reads this while composing");
    c.set_composition_active(false);
    assert!(!c.composition_active());
}

#[test]
fn line_styles_map_to_underline_enum() {
    assert_eq!(underline_from_fields(TF_LS_SOLID, false, None).style, UnderlineStyle::Solid);
    assert_eq!(underline_from_fields(TF_LS_DOT, false, None).style, UnderlineStyle::Dotted);
    assert_eq!(underline_from_fields(TF_LS_DASH, false, None).style, UnderlineStyle::Dashed);
    assert_eq!(underline_from_fields(TF_LS_SQUIGGLE, false, None).style, UnderlineStyle::Squiggly);
    assert_eq!(underline_from_fields(TF_LS_NONE, false, None).style, UnderlineStyle::None);
}

#[test]
fn unknown_line_style_falls_back_to_solid() {
    assert_eq!(underline_from_fields(999, false, None).style, UnderlineStyle::Solid);
}

#[test]
fn underline_carries_bold_and_masks_colour() {
    let u = underline_from_fields(TF_LS_SOLID, true, Some(0xFFAB_CDEF));
    assert!(u.bold);
    assert_eq!(u.color, Some(0x00AB_CDEF), "only the low 24 bits are the RGB");
    assert_eq!(underline_from_fields(TF_LS_SOLID, false, None).color, None);
}
