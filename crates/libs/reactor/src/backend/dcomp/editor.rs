//! The shared text-editor core for the drawn text controls — `NumberBox`,
//! `TextBox`, `PasswordBox`, and `AutoSuggestBox` / search. One [`Editor`] per
//! editable node holds a UTF-16 document buffer, a caret + selection, a cached
//! `IDWriteTextLayout` (rebuilt only when the text or box changes), single-line
//! horizontal scroll, and the editing / clipboard / numeric-commit operations
//! every text control reuses. Interaction routing lives in `input.rs`; the
//! chrome + caret + selection are painted in `controls.rs`. This is the single
//! DRY editor used by all four text `ControlKind`s — there is no native
//! HWND/edit-control island.

use super::theme;
use crate::backend::ControlKind;
use windows_canvas_core::{FontWeight, TextAlignment, TextFormat, TextLayout};

/// Glyph the `PasswordBox` masks every character with (`●`).
const PASSWORD_MASK: char = '\u{25CF}';

/// Inner horizontal padding from the box edge to the text (DIP).
pub(crate) const PAD_X: f32 = theme::SPACE_8;
/// Trailing reserve for the spin buttons on a `NumberBox` (DIP).
pub(crate) const SPIN_W: f32 = 18.0;
/// Below this box width a `NumberBox` hides its spin buttons (e.g. the narrow
/// EQ-band value tiles) — arrow keys / wheel still adjust the value.
pub(crate) const SPIN_MIN_BOX_W: f32 = 72.0;

/// Per-node editable text state.
pub(crate) struct Editor {
    /// Document, in UTF-16 code units.
    pub buf: Vec<u16>,
    /// Caret position (code-unit index, `0..=buf.len()`).
    pub caret: usize,
    /// Selection anchor; the selection is `[min(anchor, caret), max(..))`.
    pub anchor: usize,
    /// Horizontal scroll offset (DIP) keeping the caret in view (single-line).
    pub scroll_x: f32,
    /// Cached measured layout over the *display* string (masked for passwords).
    pub layout: Option<TextLayout>,
    /// The layout must be rebuilt (text / font / width / mask changed).
    pub layout_dirty: bool,
    /// Box width the cached layout was last built for (detects a resize).
    pub built_w: f32,
    /// Caret visibility gate: false while the host window is deactivated
    /// (keyboard focus is retained but the caret hides, mirroring system
    /// behavior). The caret itself is a compositor sprite whose blink plays
    /// DWM-side (see `parts::sync_caret`) — no timer, no repaints.
    pub caret_shown: bool,
    /// Set when the caret moved / text changed; the next paint restarts the
    /// compositor blink animation solid-first (the standard "caret goes solid
    /// while typing" behavior) and clears this.
    pub caret_moved: bool,
    /// True once seeded from a prop. While focused the user owns the buffer, so
    /// programmatic value props are ignored until blur (no clobbering mid-edit).
    pub seeded: bool,
    /// Monotonic revision of the buffer, bumped on every **user-originated**
    /// edit (keystroke, paste, IME commit, suggestion choice, UIA SetValue).
    /// Rides out on the editor-text intents; the app's programmatic write
    /// comes back stamped with the revision it was based on, and a stale one
    /// is dropped instead of retracting text the user typed since — the §7.2
    /// revision protocol, text half (the control-value twin is
    /// `Node::value_rev`).
    pub text_rev: u64,
    /// Active IME composition span `[comp_start, comp_start + comp_len)`.
    pub comp_start: usize,
    pub comp_len: usize,
    /// Render glyphs masked (`PasswordBox`).
    pub mask: bool,
    /// Numeric editing mode (`NumberBox`): input filter + arithmetic commit.
    pub numeric: bool,
}

impl Editor {
    pub fn new(kind: ControlKind) -> Self {
        Self {
            buf: Vec::new(),
            caret: 0,
            anchor: 0,
            scroll_x: 0.0,
            layout: None,
            layout_dirty: true,
            built_w: -1.0,
            caret_shown: true,
            caret_moved: true,
            seeded: false,
            text_rev: 0,
            comp_start: 0,
            comp_len: 0,
            mask: kind == ControlKind::PasswordBox,
            numeric: kind == ControlKind::NumberBox,
        }
    }

    // ── Buffer / selection helpers ────────────────────────────────────────

    /// The document as a `String`.
    pub fn text(&self) -> String {
        String::from_utf16_lossy(&self.buf)
    }

    /// Replace the whole buffer (programmatic seed); collapse the caret to end.
    pub fn set_text(&mut self, s: &str) {
        let new: Vec<u16> = s.encode_utf16().collect();
        if new == self.buf {
            return;
        }
        self.buf = new;
        self.caret = self.buf.len();
        self.anchor = self.caret;
        self.scroll_x = 0.0;
        self.mark_dirty();
    }

    /// Whether the buffer already holds exactly `s` (no allocation).
    pub fn text_eq(&self, s: &str) -> bool {
        s.encode_utf16().eq(self.buf.iter().copied())
    }

    /// Apply a programmatic (reconciliation) write with caret
    /// **position-mapping** — never collapse-to-end, which is reserved for
    /// user-action replacements ([`set_text`](Self::set_text)). The old and
    /// new documents are aligned by their common prefix/suffix; a caret or
    /// anchor before the changed region stays put, one after it shifts by the
    /// length delta, and one inside it lands at the end of the replacement.
    /// This is what keeps an app echo (or a light transform) from teleporting
    /// the caret out from under the user.
    pub fn apply_program_text(&mut self, s: &str) {
        let new: Vec<u16> = s.encode_utf16().collect();
        if new == self.buf {
            return;
        }
        let prefix = self
            .buf
            .iter()
            .zip(&new)
            .take_while(|(a, b)| a == b)
            .count();
        let max_suffix = (self.buf.len() - prefix).min(new.len() - prefix);
        let mut suffix = 0;
        while suffix < max_suffix
            && self.buf[self.buf.len() - 1 - suffix] == new[new.len() - 1 - suffix]
        {
            suffix += 1;
        }
        // The changed region is `[prefix, old_end)` → `[prefix, new_end)`.
        let old_end = self.buf.len() - suffix;
        let new_end = new.len() - suffix;
        let map = |i: usize| {
            if i <= prefix {
                i
            } else if i >= old_end {
                new_end + (i - old_end)
            } else {
                new_end
            }
        };
        self.caret = map(self.caret).min(new.len());
        self.anchor = map(self.anchor).min(new.len());
        self.buf = new;
        self.mark_dirty();
    }

    /// The selection as an ordered `[start, end)` code-unit range.
    pub fn sel(&self) -> (usize, usize) {
        (self.anchor.min(self.caret), self.anchor.max(self.caret))
    }

    pub fn has_selection(&self) -> bool {
        self.anchor != self.caret
    }

    /// The selected text (empty when there is no selection).
    pub fn selected_text(&self) -> String {
        let (a, b) = self.sel();
        String::from_utf16_lossy(&self.buf[a..b])
    }

    fn mark_dirty(&mut self) {
        self.layout_dirty = true;
    }

    // ── Editing ───────────────────────────────────────────────────────────

    /// Delete the current selection if any; returns true if it removed text.
    fn delete_selection(&mut self) -> bool {
        if !self.has_selection() {
            return false;
        }
        let (a, b) = self.sel();
        self.buf.drain(a..b);
        self.caret = a;
        self.anchor = a;
        self.mark_dirty();
        true
    }

    /// Insert a string at the caret, replacing any selection. For single-line
    /// fields the caller should pre-strip newlines.
    pub fn insert(&mut self, s: &str) {
        self.delete_selection();
        let units: Vec<u16> = s.encode_utf16().collect();
        let at = self.caret.min(self.buf.len());
        self.buf.splice(at..at, units.iter().copied());
        self.caret = at + units.len();
        self.anchor = self.caret;
        self.mark_dirty();
    }

    /// Backspace: delete the selection, else the unit before the caret.
    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.caret > 0 {
            self.caret -= 1;
            self.buf.remove(self.caret);
            self.anchor = self.caret;
            self.mark_dirty();
        }
    }

    /// Delete: remove the selection, else the unit after the caret.
    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.caret < self.buf.len() {
            self.buf.remove(self.caret);
            self.mark_dirty();
        }
    }

    fn clamp(&self, i: usize) -> usize {
        i.min(self.buf.len())
    }

    /// Move the caret one unit (or one word with `word`) left, optionally
    /// extending the selection.
    pub fn move_left(&mut self, word: bool, select: bool) {
        let to = if self.has_selection() && !select {
            self.sel().0
        } else if word {
            self.word_left(self.caret)
        } else {
            self.caret.saturating_sub(1)
        };
        self.set_caret(to, select);
    }

    pub fn move_right(&mut self, word: bool, select: bool) {
        let to = if self.has_selection() && !select {
            self.sel().1
        } else if word {
            self.word_right(self.caret)
        } else {
            self.clamp(self.caret + 1)
        };
        self.set_caret(to, select);
    }

    pub fn home(&mut self, select: bool) {
        self.set_caret(0, select);
    }

    pub fn end(&mut self, select: bool) {
        self.set_caret(self.buf.len(), select);
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.buf.len();
    }

    fn set_caret(&mut self, to: usize, select: bool) {
        self.caret = self.clamp(to);
        if !select {
            self.anchor = self.caret;
        }
    }

    /// Previous word boundary before `from` (skip spaces, then word chars).
    fn word_left(&self, from: usize) -> usize {
        let mut i = from;
        while i > 0 && is_space(self.buf[i - 1]) {
            i -= 1;
        }
        while i > 0 && !is_space(self.buf[i - 1]) {
            i -= 1;
        }
        i
    }

    fn word_right(&self, from: usize) -> usize {
        let n = self.buf.len();
        let mut i = from;
        while i < n && !is_space(self.buf[i]) {
            i += 1;
        }
        while i < n && is_space(self.buf[i]) {
            i += 1;
        }
        i
    }

    // ── Layout ────────────────────────────────────────────────────────────

    /// The display string DWrite lays out: the buffer, or the password mask.
    fn display_string(&self) -> String {
        if self.mask {
            PASSWORD_MASK.to_string().repeat(self.buf.len())
        } else {
            self.text()
        }
    }

    /// Rebuild the cached layout if the text/box changed. `align` mirrors WinRT
    /// `HorizontalAlignment` (0 Left, 1 Center, 2 Right, 3 Stretch; -1 = left).
    /// Returns the content width the text is laid out within.
    pub fn ensure_layout(
        &mut self,
        font_size: f32,
        weight: u16,
        content_w: f32,
        align: i32,
    ) -> f32 {
        let aligned = align == 1 || align == 2;
        if !self.layout_dirty
            && self.layout.is_some()
            && (!aligned || (self.built_w - content_w).abs() < 0.5)
        {
            return content_w;
        }
        let s = self.display_string();
        // Centered / right-aligned fields lay out within the content box so
        // DWrite positions the run; left-aligned fields use a generous box and
        // are scrolled (so a long value stays editable). DWrite rejects an
        // infinite constraint, so use a large finite box (mirrors the
        // TextBlock layout builder).
        let max_w = if aligned { content_w.max(1.0) } else { 100_000.0 };
        let layout = TextFormat::with_weight("Segoe UI", font_size, FontWeight(weight as i32))
            .and_then(|fmt| {
                // Vertical centering is done by the caller's `origin_y`; the
                // layout box uses the default top paragraph alignment.
                let fmt = fmt
                    .with_word_wrap(false)
                    .with_alignment(match align {
                        1 => TextAlignment::Center,
                        2 => TextAlignment::Trailing,
                        _ => TextAlignment::Leading,
                    });
                TextLayout::new(&s, &fmt, max_w, 100_000.0)
            })
            .ok();
        if let Some(l) = &layout {
            let _ = l.set_word_wrap(false);
            // Underline the active IME composition span.
            if self.comp_len > 0 {
                let _ = l.set_underline(true, self.comp_start as u32, self.comp_len as u32);
            }
        }
        self.layout = layout;
        self.layout_dirty = false;
        self.built_w = content_w;
        content_w
    }

    /// Caret x within the layout (DIP), via `caret_at`.
    pub fn caret_x(&self) -> f32 {
        match &self.layout {
            Some(l) => l
                .caret_at(self.caret as u32, false)
                .map(|((x, _), _)| x)
                .unwrap_or(0.0),
            None => 0.0,
        }
    }

    /// Keep the caret inside the visible content width by adjusting `scroll_x`
    /// (left-aligned single-line only; aligned fields do not scroll).
    pub fn scroll_to_caret(&mut self, content_w: f32, aligned: bool) {
        if aligned {
            self.scroll_x = 0.0;
            return;
        }
        let cx = self.caret_x();
        if cx - self.scroll_x > content_w {
            self.scroll_x = cx - content_w;
        }
        if cx - self.scroll_x < 0.0 {
            self.scroll_x = cx;
        }
        if self.scroll_x < 0.0 {
            self.scroll_x = 0.0;
        }
    }

    /// Rebuild the layout (if needed) and re-scroll the caret into view, in one
    /// call — run once per dirty repaint before the chrome is drawn.
    pub fn prepare(&mut self, font_size: f32, weight: u16, content_w: f32, align: i32) {
        let aligned = align == 1 || align == 2;
        self.ensure_layout(font_size, weight, content_w, align);
        self.scroll_to_caret(content_w, aligned);
    }

    // ── IME composition (IMM32 fallback) ──────────────────────────────────

    /// Begin an IME composition: drop any selection and anchor the (empty)
    /// composition span at the caret.
    pub fn ime_begin(&mut self) {
        self.delete_selection();
        self.comp_start = self.caret.min(self.buf.len());
        self.comp_len = 0;
    }

    /// Replace the current composition span with `s`. When `composing`, the run
    /// stays the (underlined) active composition; otherwise it is committed
    /// (the span is cleared).
    pub fn ime_replace(&mut self, s: &str, composing: bool) {
        let start = self.comp_start.min(self.buf.len());
        let end = (self.comp_start + self.comp_len).min(self.buf.len());
        let units: Vec<u16> = s.encode_utf16().collect();
        self.buf.splice(start..end, units.iter().copied());
        self.caret = start + units.len();
        self.anchor = self.caret;
        self.comp_start = start;
        self.comp_len = if composing { units.len() } else { 0 };
        self.mark_dirty();
    }

    /// Cancel/end composition without committing more text.
    pub fn ime_end(&mut self) {
        self.comp_len = 0;
        self.mark_dirty();
    }

    /// Map a surface-local x (DIP, relative to the box) to a caret index, given
    /// the text origin x the layout is drawn at.
    pub fn index_at_x(&self, x: f32, origin_x: f32) -> usize {
        match &self.layout {
            Some(l) => l
                .hit_test_point(x - origin_x, 1.0)
                .map(|h| {
                    let i = h.text_position as usize + usize::from(h.is_trailing_hit);
                    i.min(self.buf.len())
                })
                .unwrap_or(self.caret),
            None => self.caret,
        }
    }
}

/// Content geometry inside an editor box of width `box_w`: `(left_pad,
/// content_width)`. A `NumberBox` wide enough reserves a trailing spin-button
/// column; narrow numeric tiles (e.g. EQ-band values) get the full width.
pub(crate) fn editor_content(kind: ControlKind, box_w: f32) -> (f32, f32) {
    let spin = if kind == ControlKind::NumberBox && box_w >= SPIN_MIN_BOX_W {
        SPIN_W
    } else {
        0.0
    };
    let width = (box_w - PAD_X * 2.0 - spin).max(1.0);
    (PAD_X, width)
}

/// UTF-16 whitespace test (covers the common ASCII / NBSP cases).
fn is_space(u: u16) -> bool {
    matches!(u, 0x20 | 0x09 | 0x0A | 0x0D | 0xA0)
}

/// Whether a typed character is admissible in a numeric field (digits, sign,
/// decimal/group separators, and the inline-arithmetic operators / parens).
pub(crate) fn numeric_char_ok(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '.' | ',' | '+' | '-' | '*' | '/' | '(' | ')' | ' ' | 'e' | 'E')
}

// ── Numeric commit: parse → arithmetic eval → clamp → round → format ───────

/// Parse the editor text as a number or a small arithmetic expression
/// (`12*3`, `-6+2`, `(1+2)/4`), returning the value. Falls back to a plain
/// float parse. Returns `None` if neither parses.
pub(crate) fn eval_numeric(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(v) = trimmed.parse::<f64>() {
        return Some(v);
    }
    let mut p = Parser {
        chars: trimmed.chars().filter(|c| !c.is_whitespace()).collect(),
        pos: 0,
    };
    let v = p.expr()?;
    if p.pos == p.chars.len() && v.is_finite() {
        Some(v)
    } else {
        None
    }
}

/// Clamp to `[min, max]`, round to `precision` fraction digits, and format.
pub(crate) fn commit_format(value: f64, min: f64, max: f64, precision: Option<i32>) -> (f64, String) {
    let mut v = value.clamp(min, max);
    let digits = precision.unwrap_or(2).clamp(0, 12) as usize;
    let scale = 10f64.powi(digits as i32);
    v = (v * scale).round() / scale;
    // Re-clamp after rounding (rounding can nudge past a bound).
    v = v.clamp(min, max);
    let s = format!("{v:.digits$}");
    (v, s)
}

/// A minimal recursive-descent arithmetic evaluator over `+ - * /`, unary
/// minus, and parentheses. Operates on a pre-stripped char vector.
struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        while let Some(op) = self.peek() {
            if op == '+' || op == '-' {
                self.pos += 1;
                let rhs = self.term()?;
                v = if op == '+' { v + rhs } else { v - rhs };
            } else {
                break;
            }
        }
        Some(v)
    }

    fn term(&mut self) -> Option<f64> {
        let mut v = self.factor()?;
        while let Some(op) = self.peek() {
            if op == '*' || op == '/' {
                self.pos += 1;
                let rhs = self.factor()?;
                v = if op == '*' { v * rhs } else { v / rhs };
            } else {
                break;
            }
        }
        Some(v)
    }

    fn factor(&mut self) -> Option<f64> {
        match self.peek()? {
            '+' => {
                self.pos += 1;
                self.factor()
            }
            '-' => {
                self.pos += 1;
                self.factor().map(|v| -v)
            }
            '(' => {
                self.pos += 1;
                let v = self.expr()?;
                if self.peek() == Some(')') {
                    self.pos += 1;
                    Some(v)
                } else {
                    None
                }
            }
            _ => self.number(),
        }
    }

    fn number(&mut self) -> Option<f64> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' {
                self.pos += 1;
            } else if (c == '+' || c == '-')
                && self.pos > start
                && matches!(self.chars[self.pos - 1], 'e' | 'E')
            {
                // Exponent sign.
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return None;
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        s.parse::<f64>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor_with(text: &str, caret: usize) -> Editor {
        let mut e = Editor::new(ControlKind::TextBox);
        e.set_text(text);
        e.caret = caret;
        e.anchor = caret;
        e
    }

    /// Insertion after the caret must not move it; insertion before shifts it.
    #[test]
    fn program_text_maps_caret_across_insertion() {
        let mut e = editor_with("hello world", 5);
        e.apply_program_text("hello brave world");
        assert_eq!(e.text(), "hello brave world");
        assert_eq!(e.caret, 5, "caret before the change must stay put");

        let mut e = editor_with("hello world", 11);
        e.apply_program_text("well hello world");
        assert_eq!(e.caret, 16, "caret after the change shifts by the delta");
    }

    /// A caret inside the replaced region lands at the end of the replacement,
    /// and deletion ahead of the caret pulls it back by the removed length.
    #[test]
    fn program_text_maps_caret_across_replacement_and_deletion() {
        let mut e = editor_with("abcdef", 3);
        e.apply_program_text("abXYef");
        assert_eq!(e.caret, 4, "caret inside the changed region → end of the replacement");

        let mut e = editor_with("abcdef", 6);
        e.apply_program_text("adef");
        // Common prefix "a", common suffix "def": "bc" was deleted ahead of
        // the caret, which pulls it back by the removed length.
        assert_eq!(e.caret, 4);
    }

    /// Identical text is a strict no-op — the caret never moves on an echo.
    #[test]
    fn program_text_identical_is_a_noop() {
        let mut e = editor_with("query", 2);
        e.layout_dirty = false;
        e.apply_program_text("query");
        assert_eq!(e.caret, 2);
        assert!(!e.layout_dirty, "an identical write must not dirty the layout");
    }

    /// The selection anchor is mapped independently of the caret, so a
    /// selection spanning an untouched region survives the write.
    #[test]
    fn program_text_maps_anchor_independently() {
        let mut e = editor_with("hello world", 0);
        e.anchor = 0;
        e.caret = 5; // "hello" selected
        e.apply_program_text("hello there world");
        assert_eq!((e.anchor, e.caret), (0, 5), "selection over the prefix survives");
    }
}
