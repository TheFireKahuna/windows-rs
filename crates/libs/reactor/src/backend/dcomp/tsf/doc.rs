//! `impl TsfDocument for DCompBackend` — the focused editor, seen as a TSF
//! document.
//!
//! There is no mirror buffer and no adapter object: the store holds the host's
//! own `Rc<RefCell<DCompBackend>>`, so what a TIP reads and writes is the very
//! text the user sees. "The document" is whichever editable node currently has
//! keyboard focus; with none focused the store reports an empty, disabled
//! document and TSF stays quiet.
//!
//! Every ACP offset is a UTF-16 code-unit index into [`Editor::buf`], which is
//! exactly how the editor already indexes, so nothing is re-mapped here.
//!
//! Two rules this file exists to keep:
//!
//! * **A store edit is still a user edit.** A TIP commit runs the same
//!   `editor_after_edit` path a keystroke does — revision bump, change intent to
//!   the app, repaint — so composition text reaches the app like any other typing.
//! * **A store edit must not echo back to the TIP.** `bridge::note_store_edit`
//!   marks the change as already-known, so [`bridge::flush`] resyncs its snapshot
//!   silently instead of reporting the TIP's own edit back to it.

use super::store::TsfDocument;
use super::{bridge, DocRect, DocSelection};
use crate::backend::dcomp::node::Node;
use crate::backend::dcomp::{editor, DCompBackend};
use crate::system_bindings::{ClientToScreen, POINT};

impl DCompBackend {
    /// The focused editable node's editor, if any.
    fn tsf_editor(&self) -> Option<&editor::Editor> {
        let id = self.focused_editable()?;
        self.node(id)?.editor.as_ref()
    }
}

impl TsfDocument for DCompBackend {
    // ── content ──────────────────────────────────────────────────────────────

    fn text_len(&self) -> usize {
        self.tsf_editor().map_or(0, |e| e.buf.len())
    }

    fn copy_text(&self, start: usize, end: usize, out: &mut Vec<u16>) {
        if let Some(e) = self.tsf_editor() {
            let start = start.min(e.buf.len());
            let end = end.clamp(start, e.buf.len());
            out.extend_from_slice(&e.buf[start..end]);
        }
    }

    fn replace(&mut self, start: usize, end: usize, text: &[u16]) {
        let Some(id) = self.focused_editable() else {
            return;
        };
        self.with_editor(id, |e| e.replace_range(start, end, text));
        // The TIP made this change, so TSF must not be told about it again.
        bridge::note_store_edit();
        // …but the app must: a composition commit is a user edit exactly like a
        // keystroke, and this is the one place every user edit funnels through
        // (revision bump + the change intent + repaint).
        self.editor_after_edit(id);
    }

    // ── selection ────────────────────────────────────────────────────────────

    fn selection(&self) -> DocSelection {
        let Some(e) = self.tsf_editor() else {
            return DocSelection::caret(0);
        };
        let (start, end) = e.sel();
        DocSelection { start, end, reversed: e.caret < e.anchor }
    }

    fn set_selection(&mut self, sel: DocSelection) {
        let Some(id) = self.focused_editable() else {
            return;
        };
        self.with_editor(id, |e| {
            let (anchor, caret) = if sel.reversed {
                (sel.end, sel.start)
            } else {
                (sel.start, sel.end)
            };
            e.anchor = anchor.min(e.buf.len());
            e.caret = caret.min(e.buf.len());
        });
        bridge::note_store_edit();
        self.editor_caret_moved(id);
    }

    // ── status ───────────────────────────────────────────────────────────────

    fn is_enabled(&self) -> bool {
        self.focused_editable().is_some()
    }

    fn is_read_only(&self) -> bool {
        // A field that kept focus while being disabled: locks are still granted
        // (a TIP may read), but writes fail rather than editing a dead control.
        self.focused_editable()
            .and_then(|id| self.node(id))
            .is_some_and(|n| !n.paint.is_enabled)
    }

    // ── geometry (screen pixels) ─────────────────────────────────────────────

    fn range_rect(&self, start: usize, end: usize) -> Option<DocRect> {
        let id = self.focused_editable()?;
        let n = self.node(id)?;
        let ed = n.editor.as_ref()?;
        // No layout yet → `TS_E_NOLAYOUT`, which a TIP retries after the next
        // paint. Answering with a guess would park the candidate window in the
        // wrong place for the whole composition.
        let layout = ed.layout.as_ref()?;
        let (pad_left, _) = editor::editor_content(n.kind, n.rect.w);
        let x_at = |i: usize| {
            layout
                .caret_at(i as u32, false)
                .map(|((x, _), _)| x)
                .unwrap_or(0.0)
        };
        let (text_h, origin_y) = text_band(n, ed);
        let x0 = n.rect.x + pad_left - ed.scroll_x;
        self.to_screen(
            x0 + x_at(start),
            n.rect.y + origin_y,
            x0 + x_at(end.max(start)),
            n.rect.y + origin_y + text_h,
        )
    }

    fn screen_rect(&self) -> Option<DocRect> {
        let id = self.focused_editable()?;
        let n = self.node(id)?;
        self.to_screen(n.rect.x, n.rect.y, n.rect.x + n.rect.w, n.rect.y + n.rect.h)
    }

    // ── composition ──────────────────────────────────────────────────────────

    fn composition_update(&mut self, start: usize, len: usize) {
        let Some(id) = self.focused_editable() else {
            return;
        };
        self.with_editor(id, |e| e.set_composition_span(start, len));
        self.editor_caret_moved(id);
    }

    fn composition_end(&mut self) {
        let Some(id) = self.focused_editable() else {
            return;
        };
        self.with_editor(id, |e| e.set_composition_span(0, 0));
        self.editor_caret_moved(id);
    }
}

impl DCompBackend {
    /// Window-relative DIPs → screen pixels, the space every ACP extent method
    /// reports in. `None` if the window is gone.
    fn to_screen(&self, left: f32, top: f32, right: f32, bottom: f32) -> Option<DocRect> {
        let s = self.scale();
        let mut origin = POINT { x: 0, y: 0 };
        // SAFETY: `origin` is a valid out-parameter; the HWND is the live host
        // window (the backend outlives it only during teardown, where the call
        // fails and we report no extent).
        if !unsafe { ClientToScreen(self.hwnd as _, &mut origin) }.as_bool() {
            return None;
        }
        Some(DocRect {
            left: origin.x + (left * s) as i32,
            top: origin.y + (top * s) as i32,
            right: origin.x + (right * s).ceil() as i32,
            bottom: origin.y + (bottom * s).ceil() as i32,
        })
    }
}

/// The text band inside an editor box: `(height, y offset from the box top)`,
/// mirroring how `controls::paint_editor` centres the run vertically — a TIP's
/// candidate window must sit under the text it is composing, not under the box.
fn text_band(n: &Node, ed: &editor::Editor) -> (f32, f32) {
    let text_h = ed
        .layout
        .as_ref()
        .and_then(|l| l.measure().ok())
        .map(|(_, h)| h)
        .filter(|h| *h > 0.0)
        .unwrap_or(n.paint.font_size * 1.4);
    (text_h, (n.rect.h - text_h) / 2.0)
}
