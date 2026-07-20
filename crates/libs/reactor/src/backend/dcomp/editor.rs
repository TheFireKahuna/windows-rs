//! The shared text-editor core for the drawn text controls — `NumberBox`,
//! `TextBox`, `PasswordBox`, and `AutoSuggestBox` / search. One [`Editor`] per
//! editable node holds a UTF-16 document buffer, a caret + selection, a cached
//! `IDWriteTextLayout` (rebuilt only when the text or box changes), single-line
//! horizontal scroll, and the editing / clipboard / numeric-commit operations
//! every text control reuses. Interaction routing lives in `input.rs`. Nothing
//! about an editor is painted: the box fill, border and spin divider are
//! retained parts (`parts::editor_plan`), and the run, placeholder, selection,
//! composition rule, chevrons and caret are sprites (`glyph_text::editor_sync`,
//! `parts::sync_caret`). This is the single DRY editor used by all four text
//! `ControlKind`s — there is no native HWND/edit-control island.

use super::theme;
use crate::backend::ControlKind;
use windows_canvas_core::{FontWeight, Rect, TextAlignment, TextFormat, TextLayout};

/// Glyph the `PasswordBox` masks every character with (`●`).
const PASSWORD_MASK: char = '\u{25CF}';

/// Inner horizontal padding from the box edge to the text (DIP).
pub(crate) const PAD_X: f32 = theme::SPACE_8;
/// Trailing reserve for the spin buttons on a `NumberBox` (DIP).
pub(crate) const SPIN_W: f32 = 18.0;
/// Below this box width a `NumberBox` hides its spin buttons (e.g. the narrow
/// EQ-band value tiles) — arrow keys / wheel still adjust the value.
pub(crate) const SPIN_MIN_BOX_W: f32 = 72.0;

/// A wide `NumberBox`'s two spin buttons, node-local: `(up, down)`. `None` when
/// the box is too narrow to carry them.
///
/// One definition, read by the divider that paints, the two chevron sprites that
/// place, and the press that steps the value. A glyph sitting anywhere but where
/// the press lands is a button that visibly ignores its own clicks — and the
/// three used to derive the same split independently.
pub(crate) fn spin_boxes(w: f32, h: f32) -> Option<(Rect, Rect)> {
    (w >= SPIN_MIN_BOX_W).then(|| {
        let x = w - SPIN_W;
        let mid = h / 2.0;
        (Rect::new(x, 0.0, w, mid), Rect::new(x, mid, w, h))
    })
}

/// Where the caret sits inside its layout, as DirectWrite reports it.
/// Layout-relative DIP; the caller adds the band's origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CaretGeom {
    /// The insertion point. The bar straddles this, it does not start at it.
    pub x: f32,
    /// Top of the region enclosing the text position.
    pub top: f32,
    /// Height of that region — the line height at the caret.
    pub height: f32,
}

/// Which side of the caret index the insertion point visually belongs to.
///
/// A caret index names a *gap* between characters, and in bidirectional text one
/// gap has two places on screen. Take `abc` followed by the Hebrew `אבג`: index
/// 3 is the gap between `c` and `א`, and it can be drawn immediately after `c`
/// or immediately after `א` — on the far side of the Hebrew word, because the
/// Hebrew renders right-to-left. Both are index 3. Affinity is the bit that says
/// which, and it is the difference between a caret that follows the user and one
/// that teleports across a word.
///
/// In left-to-right text the two resolve to the same point, which is why an
/// editor can ship without the bit and look correct — right up until someone
/// pastes Arabic or Hebrew into a field.
///
/// Maintained at every caret write and read at three places (the sprite, the IME
/// candidate rect, UIA). That asymmetry is the hazard: a site that forgets to
/// set it is invisible in Latin test data, so [`Editor::set_caret`] takes it as
/// a required argument rather than defaulting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Affinity {
    /// The caret belongs to the character *after* it — its leading edge.
    /// Where a caret lands when it moves backwards onto a character, and the
    /// safe default for a position with no history behind it.
    Downstream,
    /// The caret belongs to the character *before* it — its trailing edge.
    /// Where a caret lands after typing, or after stepping forward over a
    /// character: adjacent to what it just passed.
    Upstream,
}

// ── Caret width (Settings → Accessibility → Text cursor → thickness) ─────────

/// The widest the Settings slider goes. Clamped rather than trusted: this is a
/// registry-backed value, and a caret wider than the field it sits in would
/// cover the text it exists to point at.
const CARET_W_MAX: u32 = 20;

/// Cached `SPI_GETCARETWIDTH`. Read once at host creation and on the
/// `WM_SETTINGCHANGE` that the thickness slider broadcasts, because
/// [`caret_width`] is called from the paint path.
static CARET_WIDTH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

/// The user's caret thickness, in DIP.
///
/// Treated as a LOGICAL width, not physical pixels. The setting exists so a
/// caret can be *found*, and a fixed pixel count shrinks with display density —
/// exactly backwards for its purpose. A DIP width is the same apparent size on
/// every monitor, and at the default of 1 the two readings coincide anyway.
pub(crate) fn caret_width() -> f32 {
    CARET_WIDTH.load(std::sync::atomic::Ordering::Relaxed) as f32
}

/// Re-read the thickness into the cache.
///
/// No change signal, unlike [`crate::motion::refresh_reduced_motion`]: the only
/// caller pairs this with `refresh_caret_blink`, which repaints the focused
/// field on every `WM_SETTINGCHANGE` regardless, so a "did it change" answer
/// would gate nothing. A store of the same value is the whole cost.
///
/// **Fails open** to 1, the Windows default: a preference that cannot be read
/// should leave a working caret, not a missing one.
pub(crate) fn refresh_caret_width() {
    let mut px: u32 = 1;
    let ok = unsafe {
        crate::system_bindings::SystemParametersInfoW(
            crate::system_bindings::SPI_GETCARETWIDTH,
            0,
            (&raw mut px).cast(),
            0,
        )
    };
    let now = if ok.as_bool() { px.clamp(1, CARET_W_MAX) } else { 1 };
    CARET_WIDTH.store(now, std::sync::atomic::Ordering::Relaxed);
}

/// Per-node editable text state.
pub(crate) struct Editor {
    /// Document, in UTF-16 code units.
    pub buf: Vec<u16>,
    /// Caret position (code-unit index, `0..=buf.len()`).
    ///
    /// Private, unlike its `anchor` twin, because it carries an invariant the
    /// anchor does not: it is only half of the insertion point, and moving it
    /// without deciding [`caret_affinity`](Self::caret_affinity) leaves the
    /// other half describing where the caret used to be. Read it with
    /// [`caret`](Self::caret), move it with [`set_caret`](Self::set_caret).
    caret: usize,
    /// Which side of `caret` the insertion point belongs to. Only observable
    /// where visual and logical order disagree, which is why it is written
    /// through one required-argument setter rather than left to each caller.
    caret_affinity: Affinity,
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
    /// Active composition span `[comp_start, comp_start + comp_len)`, marked by
    /// the TSF composition sink. Non-empty means a TIP is composing here: the
    /// run is underlined, and the §7.2 guard refuses every programmatic write.
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
            caret_affinity: Affinity::Downstream,
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

    /// Caret position (code-unit index, `0..=buf.len()`).
    pub fn caret(&self) -> usize {
        self.caret
    }

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
        // Collapsed to the end of a document it did not type: the caret trails
        // the last character, as it would after typing it.
        self.set_caret(self.buf.len(), Affinity::Upstream, false);
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
        let moved = map(self.caret).min(new.len());
        // A caret the mapping did not move kept whatever it meant; one that did
        // move landed by arithmetic on a region it has no relationship to, and
        // claiming it still trails a specific character would be inventing
        // history. Downstream is the honest answer there.
        let affinity = if moved == self.caret {
            self.caret_affinity
        } else {
            Affinity::Downstream
        };
        let anchor = map(self.anchor).min(new.len());
        self.buf = new;
        self.set_caret(moved, affinity, true);
        self.anchor = anchor;
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
        // The removed run is what the caret was last adjacent to, so there is
        // nothing left downstream of it to belong to; it trails the text before.
        self.set_caret(a, Affinity::Upstream, false);
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
        // "The caret follows what you typed" — it trails the inserted run, on
        // the run's own side even when that run reads right-to-left.
        self.set_caret(at + units.len(), Affinity::Upstream, false);
        self.mark_dirty();
    }

    /// Backspace: delete the selection, else the unit before the caret.
    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.caret > 0 {
            // The whole cluster, not one code unit: removing half a surrogate
            // pair leaves a buffer that is not valid UTF-16.
            let from = self.prev_stop(self.caret);
            self.buf.drain(from..self.caret);
            self.set_caret(from, Affinity::Upstream, false);
            self.mark_dirty();
        }
    }

    /// Delete: remove the selection, else the unit after the caret.
    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.caret < self.buf.len() {
            let to = self.next_stop(self.caret);
            self.buf.drain(self.caret..to);
            // The index does not move, but a Downstream caret was anchored to
            // the character just removed. Re-anchor upstream, to text that
            // still exists, rather than silently re-pointing at whatever slid
            // into the gap.
            self.set_caret(self.caret, Affinity::Upstream, false);
            self.mark_dirty();
        }
    }

    fn clamp(&self, i: usize) -> usize {
        i.min(self.buf.len())
    }

    // ── Cluster stops ────────────────────────────────────────────────────────
    //
    // A caret stop is a whole *cluster*, not a code unit. A surrogate pair, a
    // base character plus its diacritics, a character plus a variation selector
    // and a ZWJ emoji sequence are each one indivisible thing the caret may sit
    // either side of but never inside. Stepping or deleting by one code unit
    // splits them — for a surrogate pair that produces text which is not valid
    // UTF-16 at all.
    //
    // This mirrors what Microsoft's own DirectWrite editor sample does
    // (PadWrite `AlignCaretToNearestCluster`): only DirectWrite knows where
    // clusters begin and end, because only DirectWrite has shaped the text, so
    // it is asked rather than second-guessed.

    /// The whole cluster containing code-unit `i`, as `[start, end)`.
    ///
    /// `end` is guaranteed strictly greater than `i` for any `i` inside the
    /// buffer, so a caller stepping through the text always makes progress even
    /// if DirectWrite reports a degenerate zero-length cluster.
    fn cluster_at(&self, i: usize) -> (usize, usize) {
        let n = self.buf.len();
        if i >= n {
            return (n, n);
        }
        if let Some(layout) = &self.layout
            && let Ok((_, hit)) = layout.caret_at(i as u32, false)
        {
            let start = (hit.text_position as usize).min(i);
            let end = start.saturating_add(hit.length as usize).clamp(i + 1, n);
            return (start, end);
        }
        // No layout yet (the field has not been painted). DirectWrite cannot be
        // asked, so fall back to the one cluster rule that is a property of the
        // encoding rather than of shaping: a surrogate pair is inseparable.
        // Combining marks are not caught here, but a caret cannot reach this
        // path in a field the user is looking at.
        let start = if i > 0
            && is_low_surrogate(self.buf[i])
            && is_high_surrogate(self.buf[i - 1])
        {
            i - 1
        } else {
            i
        };
        let end = if is_high_surrogate(self.buf[start])
            && start + 1 < n
            && is_low_surrogate(self.buf[start + 1])
        {
            start + 2
        } else {
            start + 1
        };
        (start, end.min(n))
    }

    /// The caret stop after `from`, skipping clusters that occupy no width.
    ///
    /// Zero-width clusters are real — a zero-width joiner, a bidi mark, a
    /// zero-width space all survive a paste — and stopping on one parks the
    /// caret at a position visually identical to the last, which is
    /// indistinguishable from a dropped keypress. PadWrite forces past them for
    /// the same reason. The skip is bounded because the loop's termination
    /// otherwise depends on DirectWrite's answers rather than on this code.
    fn next_stop(&self, from: usize) -> usize {
        let mut i = self.cluster_at(from).1;
        for _ in 0..MAX_ZERO_WIDTH_SKIP {
            if i >= self.buf.len() || !self.is_zero_width_at(i) {
                break;
            }
            i = self.cluster_at(i).1;
        }
        i
    }

    /// The caret stop before `from`, skipping zero-width clusters.
    fn prev_stop(&self, from: usize) -> usize {
        if from == 0 {
            return 0;
        }
        let mut i = self.cluster_at(from - 1).0;
        for _ in 0..MAX_ZERO_WIDTH_SKIP {
            if i == 0 || !self.is_zero_width_at(i) {
                break;
            }
            i = self.cluster_at(i - 1).0;
        }
        i
    }

    /// Whether the cluster starting at `i` draws nothing. Only answerable with a
    /// layout; without one every cluster is treated as visible, which costs a
    /// caret stop on an invisible character rather than a correctness failure.
    fn is_zero_width_at(&self, i: usize) -> bool {
        self.layout
            .as_ref()
            .and_then(|l| l.caret_at(i as u32, false).ok())
            .is_some_and(|(_, hit)| hit.glyph_rect.2 == 0.0)
    }

    /// Move the caret one unit (or one word with `word`) left, optionally
    /// extending the selection.
    ///
    /// Movement is **logical**, not visual: the index decreases, and across a
    /// direction boundary the caret may therefore travel rightwards on screen.
    /// This is a deliberate choice over visual movement — word moves, Home/End
    /// and selection extension are all logical here, and a model that is
    /// logical for four operations and visual for two is worse than either
    /// consistently. What affinity buys is that the caret is drawn *adjacent to
    /// the character it just stepped over* instead of at whichever edge
    /// DirectWrite was asked for.
    pub fn move_left(&mut self, word: bool, select: bool) {
        let to = if self.has_selection() && !select {
            self.sel().0
        } else if word {
            // A word boundary is normally also a cluster boundary, but a
            // combining mark can follow a space; align rather than assume.
            self.cluster_at(self.word_left(self.caret)).0
        } else {
            self.prev_stop(self.caret)
        };
        // Stepping backwards onto a character puts the caret on its leading side.
        self.set_caret(to, Affinity::Downstream, select);
    }

    /// Right-moving twin of [`move_left`](Self::move_left); see it for why the
    /// movement is logical.
    pub fn move_right(&mut self, word: bool, select: bool) {
        let to = if self.has_selection() && !select {
            self.sel().1
        } else if word {
            self.cluster_at(self.word_right(self.caret)).0
        } else {
            self.next_stop(self.caret)
        };
        // Stepping forward over a character puts the caret on its trailing side.
        self.set_caret(to, Affinity::Upstream, select);
    }

    pub fn home(&mut self, select: bool) {
        self.set_caret(0, Affinity::Downstream, select);
    }

    pub fn end(&mut self, select: bool) {
        self.set_caret(self.buf.len(), Affinity::Upstream, select);
    }

    pub fn select_all(&mut self) {
        self.set_caret(self.buf.len(), Affinity::Upstream, true);
        self.anchor = 0;
    }

    /// Move the caret, stating where it belongs. The one write path — every
    /// other mutation in this file and every caller outside it goes through
    /// here, so a new caret move cannot forget the affinity: there is no
    /// signature that lets it.
    ///
    /// `select` extends the selection rather than collapsing it.
    pub fn set_caret(&mut self, to: usize, affinity: Affinity, select: bool) {
        self.caret = self.clamp(to);
        self.caret_affinity = affinity;
        if !select {
            self.anchor = self.caret;
        }
    }

    /// Start of the word before `from` — Ctrl+Left.
    ///
    /// Steps back over any whitespace, then over one run of a single
    /// [`CharClass`]. Because punctuation is its own class, `foo.bar` stops at
    /// `bar`, then at `.`, then at `foo`, which is what every other Windows
    /// text field does.
    fn word_left(&self, from: usize) -> usize {
        let mut i = from.min(self.buf.len());
        while i > 0 && class_of(self.buf[i - 1]) == CharClass::Space {
            i -= 1;
        }
        if i == 0 {
            return 0;
        }
        let class = class_of(self.buf[i - 1]);
        while i > 0 && class_of(self.buf[i - 1]) == class {
            i -= 1;
        }
        i
    }

    /// Start of the word after `from` — Ctrl+Right.
    ///
    /// Steps over the run `from` sits in, then over the whitespace that trails
    /// it, so the caret lands at the *beginning* of the next word rather than
    /// in the gap before it.
    fn word_right(&self, from: usize) -> usize {
        let n = self.buf.len();
        let mut i = from.min(n);
        if i < n {
            let class = class_of(self.buf[i]);
            while i < n && class_of(self.buf[i]) == class {
                i += 1;
            }
        }
        while i < n && class_of(self.buf[i]) == CharClass::Space {
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
            // Underline the active composition span. DWrite exposes only a
            // boolean underline here, so a TIP's display attribute (squiggly /
            // coloured clause styles) is deliberately not resolved — see
            // `tsf::mod`.
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
        self.caret_geom().map_or(0.0, |g| g.x)
    }

    /// The caret's geometry at the insertion point, straight out of DirectWrite
    /// — `HitTestTextPosition`'s point plus the enclosing region's height.
    ///
    /// `None` before the first layout exists, which the caller must answer from
    /// [`TextBand`]'s fallback rather than by guessing a height here: this
    /// function's whole value is that it does not guess.
    ///
    /// The height is the one DirectWrite reports for the region enclosing the
    /// text position, which is the line height at that position — the same field
    /// Microsoft's own DirectWrite editor sample (PadWrite `GetCaretRect`) sizes
    /// its caret from. Deriving it from the font size instead is what makes a
    /// caret disagree with the line it sits on the moment anything about the
    /// run's metrics is not what the derivation assumed.
    ///
    /// A non-finite or non-positive height is rejected rather than propagated: a
    /// NaN reaching a visual's Size is not a wrong caret, it is a caret that
    /// silently stops being composited.
    pub fn caret_geom(&self) -> Option<CaretGeom> {
        let (pos, after) = self.caret_query();
        let ((x, y), hit) = self.layout.as_ref()?.caret_at(pos, after).ok()?;
        let h = hit.glyph_rect.3;
        (x.is_finite() && y.is_finite() && h.is_finite() && h > 0.0)
            .then_some(CaretGeom { x, top: y, height: h })
    }

    /// The `(text_position, after)` pair that asks DirectWrite for the caret's
    /// point, resolving [`Affinity`] into `HitTestTextPosition`'s edge model.
    ///
    /// Upstream is the trailing edge of the *preceding* character, which is why
    /// it queries `caret - 1`. At index 0 there is no preceding character, so
    /// the upstream position does not exist and the downstream one is the only
    /// answer — a degeneracy, not a fallback: the two coincide there for the
    /// same reason they coincide throughout left-to-right text.
    pub fn caret_query(&self) -> (u32, bool) {
        match self.caret_affinity {
            Affinity::Upstream if self.caret > 0 => (self.caret as u32 - 1, true),
            _ => (self.caret as u32, false),
        }
    }

    /// Keep the caret inside the visible content width by adjusting `scroll_x`
    /// (left-aligned single-line only; aligned fields do not scroll).
    pub fn scroll_to_caret(&mut self, content_w: f32, aligned: bool) {
        if aligned {
            self.scroll_x = 0.0;
            return;
        }
        self.scroll_x = scroll_for(self.scroll_x, self.caret_x(), content_w);
    }

    /// Rebuild the layout (if needed) and re-scroll the caret into view, in one
    /// call — run once per dirty repaint before the chrome is drawn.
    pub fn prepare(&mut self, font_size: f32, weight: u16, content_w: f32, align: i32) {
        let aligned = align == 1 || align == 2;
        self.ensure_layout(font_size, weight, content_w, align);
        self.scroll_to_caret(content_w, aligned);
    }

    // ── TSF document edits ────────────────────────────────────────────────
    //
    // The text store owns composition end-to-end: composing text arrives as
    // ordinary range replacements, and the composing *span* is marked
    // separately by the composition sink. There is no second (IMM32) path —
    // see `tsf::mod`.

    /// Replace the ACP range `[start, end)` with `units`, collapsing the caret
    /// to the end of the inserted run. Offsets are UTF-16 code-unit indices
    /// (the ACP space is exactly this buffer's indexing) and are clamped, so a
    /// misbehaving TIP can never index out of bounds.
    pub fn replace_range(&mut self, start: usize, end: usize, units: &[u16]) {
        let start = start.min(self.buf.len());
        let end = end.clamp(start, self.buf.len());
        self.buf.splice(start..end, units.iter().copied());
        // Same rule as `insert`: a committed composition trails the run it
        // produced.
        self.set_caret(start + units.len(), Affinity::Upstream, false);
        self.mark_dirty();
    }

    /// Mark the active composing run, or clear it with `len == 0`. Only the
    /// span is stored — the composing text itself arrives through
    /// [`replace_range`](Self::replace_range) like any other TSF edit. This is
    /// what the underline paints over and what the §7.2 composition guard reads.
    pub fn set_composition_span(&mut self, start: usize, len: usize) {
        let start = start.min(self.buf.len());
        let len = len.min(self.buf.len() - start);
        if (self.comp_start, self.comp_len) == (start, len) {
            return;
        }
        self.comp_start = start;
        self.comp_len = len;
        self.mark_dirty();
    }

    /// Map a surface-local x (DIP, relative to the box) to a caret index and the
    /// affinity the click implies, given the text origin x the layout is drawn at.
    ///
    /// Both halves come from the same hit test. A trailing hit means the point
    /// fell on the far half of a cluster, so the caret goes *after* that cluster
    /// and belongs to it — upstream. A leading hit puts the caret before the
    /// cluster, belonging to it — downstream. Taking only the index and
    /// discarding which half was hit is what makes a click near a direction
    /// boundary land on the wrong side of a word.
    ///
    /// The step past a trailing hit is the cluster's own `length`, not one code
    /// unit: a surrogate pair, a combining sequence and a ligature are each one
    /// indivisible caret stop spanning several units, and stepping by one lands
    /// the caret inside a character that has no inside.
    pub fn index_at_x(&self, x: f32, origin_x: f32) -> (usize, Affinity) {
        let Some(layout) = &self.layout else {
            return (self.caret, self.caret_affinity);
        };
        match layout.hit_test_point(x - origin_x, 1.0) {
            Ok(h) if h.is_trailing_hit => (
                (h.text_position as usize + h.length as usize).min(self.buf.len()),
                Affinity::Upstream,
            ),
            Ok(h) => (
                (h.text_position as usize).min(self.buf.len()),
                Affinity::Downstream,
            ),
            Err(_) => (self.caret, self.caret_affinity),
        }
    }
}

/// The scroll offset that brings caret x `cx` inside the window
/// `[scroll_x, scroll_x + content_w)`, moving as little as possible.
///
/// Split out from [`Editor::scroll_to_caret`] so it can be tested without a
/// device: the caret x it is handed is the only thing DirectWrite contributes,
/// and this function's job is purely to keep that number in the window.
///
/// Affinity does not change the shape of the problem. A caret in a
/// right-to-left run still has a single x within a left-to-right field, and it
/// is still that x that has to be visible — what affinity changed is *which* x,
/// which is decided before this is called.
fn scroll_for(scroll_x: f32, cx: f32, content_w: f32) -> f32 {
    let mut s = scroll_x;
    if cx - s > content_w {
        s = cx - content_w;
    }
    if cx - s < 0.0 {
        s = cx;
    }
    s.max(0.0)
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

/// Where an editor's text run sits inside its box, in NODE-LOCAL DIPs.
///
/// The one definition of that geometry. It used to exist in four:
/// `controls::paint_editor` (which drew the run, now retired), `controls::editor_caret_box`
/// (which placed the caret sprite), `tsf::doc::text_band` (which parked the IME
/// candidate window) and `uia::uia_text_origin` (which answered screen readers).
/// Each recomputed the same fallback line height and the same vertical centring,
/// and two of them carried comments promising they matched the painter — which
/// is the tell: a promise in a comment is what a shared function makes
/// unnecessary.
///
/// The cost of them drifting is not a wrong pixel. It is the caret landing off
/// the text, the candidate window opening away from the composition, and
/// Narrator reading a rectangle that is not where the words are — three
/// failures with no common symptom, each looking like a bug in its own
/// subsystem.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextBand {
    /// Measured line height, or a font-size-derived fallback before the layout
    /// exists.
    pub text_h: f32,
    /// Top of the text band — the run's baseline box, vertically centred.
    pub origin_y: f32,
    /// Left edge of the run, with horizontal scroll already applied. Left of
    /// [`content_x`](Self::content_x) exactly when the field is scrolled.
    pub origin_x: f32,
    /// Left edge of the content column (the clip), scroll NOT applied.
    pub content_x: f32,
    /// Width of the content column.
    pub content_w: f32,
}

impl TextBand {
    /// The band from its scalars alone — no node, no layout, no device, so the
    /// arithmetic every consumer depends on is exhaustively testable.
    ///
    /// `measured_h` is the laid-out line height where one exists. `None` covers
    /// the window between a field gaining content and its first paint, and the
    /// fallback it selects is not incidental: it is what the caret sprite and
    /// the IME candidate window are placed by for that whole window.
    pub(crate) fn compute(
        kind: ControlKind,
        box_w: f32,
        box_h: f32,
        font_size: f32,
        measured_h: Option<f32>,
        scroll_x: f32,
    ) -> Self {
        let (content_x, content_w) = editor_content(kind, box_w);
        let text_h = measured_h
            .filter(|h| *h > 0.0)
            .unwrap_or(font_size * 1.4);
        Self {
            text_h,
            origin_y: (box_h - text_h) / 2.0,
            origin_x: content_x - scroll_x,
            content_x,
            content_w,
        }
    }

    /// The band for `node`, or `None` if it is not an editor.
    pub(crate) fn of(node: &super::node::Node) -> Option<Self> {
        let ed = node.editor.as_ref()?;
        Some(Self::compute(
            node.kind,
            node.rect.w,
            node.rect.h,
            node.paint.font_size,
            ed.layout.as_ref().and_then(|l| l.measure().ok()).map(|(_, h)| h),
            ed.scroll_x,
        ))
    }
}

/// UTF-16 whitespace test (covers the common ASCII / NBSP cases).
fn is_space(u: u16) -> bool {
    matches!(u, 0x20 | 0x09 | 0x0A | 0x0D | 0xA0)
}

/// How a code unit participates in word navigation.
///
/// Ctrl+Arrow in a Windows edit control does not split on whitespace alone —
/// punctuation is its own word. `foo.bar` is three words there (`foo`, `.`,
/// `bar`) and Ctrl+Left from the end stops at `bar`, not at the start of the
/// whole token. Splitting on whitespace only is the single largest behavioural
/// divergence a text field can have from the rest of the system, because the
/// user's hands already know the answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CharClass {
    /// Whitespace: never a word on its own; trails the word before it.
    Space,
    /// Letters, digits, and `_` — the run that Ctrl+Arrow treats as one word.
    Word,
    /// Everything else. A run of punctuation is a word in its own right.
    Punct,
}

/// Classify one UTF-16 code unit for word navigation.
///
/// Both halves of a surrogate pair classify as [`Word`](CharClass::Word), so a
/// supplementary character — an emoji, a rarer CJK ideograph — is traversed as
/// content rather than splitting a word down its middle.
pub(crate) fn class_of(u: u16) -> CharClass {
    if is_space(u) {
        return CharClass::Space;
    }
    if is_high_surrogate(u) || is_low_surrogate(u) {
        return CharClass::Word;
    }
    match char::from_u32(u as u32) {
        // `is_alphanumeric` is the Unicode property, not the ASCII range: an
        // accented letter and a Cyrillic one are word characters like any other.
        Some(c) if c.is_alphanumeric() || c == '_' => CharClass::Word,
        // The whitespace `is_space` does not list — ideographic space, the
        // various fixed-width spaces.
        Some(c) if c.is_whitespace() => CharClass::Space,
        _ => CharClass::Punct,
    }
}

/// How many consecutive zero-width clusters a caret move will step over before
/// giving up and stopping on one.
///
/// A bound rather than a `while`: the loop's termination would otherwise rest on
/// DirectWrite always reporting progress, and a caret that cannot be moved is a
/// field that cannot be edited. Real runs of invisible characters are one or two
/// long — a joiner, a bidi mark — so this never binds in practice.
const MAX_ZERO_WIDTH_SKIP: usize = 16;

/// First half of a UTF-16 surrogate pair.
fn is_high_surrogate(u: u16) -> bool {
    (0xD800..0xDC00).contains(&u)
}

/// Second half of a UTF-16 surrogate pair.
fn is_low_surrogate(u: u16) -> bool {
    (0xDC00..0xE000).contains(&u)
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
        e.set_caret(caret, Affinity::Downstream, false);
        e
    }

    /// Insertion after the caret must not move it; insertion before shifts it.
    #[test]
    fn program_text_maps_caret_across_insertion() {
        let mut e = editor_with("hello world", 5);
        e.apply_program_text("hello brave world");
        assert_eq!(e.text(), "hello brave world");
        assert_eq!(e.caret(), 5, "caret before the change must stay put");

        let mut e = editor_with("hello world", 11);
        e.apply_program_text("well hello world");
        assert_eq!(e.caret(), 16, "caret after the change shifts by the delta");
    }

    /// A caret inside the replaced region lands at the end of the replacement,
    /// and deletion ahead of the caret pulls it back by the removed length.
    #[test]
    fn program_text_maps_caret_across_replacement_and_deletion() {
        let mut e = editor_with("abcdef", 3);
        e.apply_program_text("abXYef");
        assert_eq!(e.caret(), 4, "caret inside the changed region → end of the replacement");

        let mut e = editor_with("abcdef", 6);
        e.apply_program_text("adef");
        // Common prefix "a", common suffix "def": "bc" was deleted ahead of
        // the caret, which pulls it back by the removed length.
        assert_eq!(e.caret(), 4);
    }

    // ── TextBand ─────────────────────────────────────────────────────────────

    /// The run must sit vertically centred in the box, and the band must report
    /// the same height it centred by. Every consumer derives its own geometry
    /// from these two, so an inconsistency here is one the caret, the IME and
    /// the screen reader each inherit separately.
    #[test]
    fn the_band_centres_the_run_it_measures() {
        for &box_h in &[24.0f32, 32.0, 40.0, 57.0] {
            for &measured in &[14.0f32, 18.0, 21.5] {
                let b = TextBand::compute(
                    ControlKind::TextBox,
                    200.0,
                    box_h,
                    14.0,
                    Some(measured),
                    0.0,
                );
                assert_eq!(b.text_h, measured);
                // Equal space above and below is what "centred" means, and it
                // is the property the four old copies each re-derived.
                let above = b.origin_y;
                let below = box_h - (b.origin_y + b.text_h);
                assert!(
                    (above - below).abs() < 1.0e-4,
                    "box {box_h} run {measured}: {above} above vs {below} below"
                );
            }
        }
    }

    /// With no layout yet the band must fall back rather than collapse — a
    /// zero-height band would park the caret and the candidate window on the
    /// box's centre line instead of on the text.
    #[test]
    fn an_unmeasured_band_falls_back_to_the_font_size() {
        for &(measured, why) in &[(None, "no layout"), (Some(0.0), "degenerate measure")] {
            let b = TextBand::compute(ControlKind::TextBox, 200.0, 32.0, 15.0, measured, 0.0);
            assert!(b.text_h > 0.0, "{why}: band collapsed");
            assert_eq!(b.text_h, 15.0 * 1.4, "{why}");
        }
    }

    /// Horizontal scroll moves the RUN and not the column. The clip is what
    /// stays put; the text slides under it.
    #[test]
    fn scroll_moves_the_run_and_not_the_column() {
        let at_rest = TextBand::compute(ControlKind::TextBox, 200.0, 32.0, 14.0, Some(18.0), 0.0);
        let scrolled = TextBand::compute(ControlKind::TextBox, 200.0, 32.0, 14.0, Some(18.0), 37.0);

        assert_eq!(at_rest.origin_x, at_rest.content_x, "unscrolled: run at the column");
        assert_eq!(scrolled.content_x, at_rest.content_x, "the column must not move");
        assert_eq!(scrolled.content_w, at_rest.content_w);
        assert_eq!(scrolled.origin_x, at_rest.origin_x - 37.0);
        // Vertical geometry is untouched by horizontal scroll.
        assert_eq!(scrolled.origin_y, at_rest.origin_y);
        assert_eq!(scrolled.text_h, at_rest.text_h);
    }

    /// A wide NumberBox reserves its spin column; the band's content width must
    /// be the one the text is actually clipped to, not the whole box.
    #[test]
    fn the_band_honours_the_spin_column() {
        let wide = TextBand::compute(
            ControlKind::NumberBox,
            SPIN_MIN_BOX_W + 40.0,
            32.0,
            14.0,
            Some(18.0),
            0.0,
        );
        let plain = TextBand::compute(
            ControlKind::TextBox,
            SPIN_MIN_BOX_W + 40.0,
            32.0,
            14.0,
            Some(18.0),
            0.0,
        );
        assert!(
            wide.content_w < plain.content_w,
            "the spin column must narrow the content: {} vs {}",
            wide.content_w,
            plain.content_w
        );
        assert_eq!(wide.content_x, plain.content_x, "only the width changes");
    }

    /// Identical text is a strict no-op — the caret never moves on an echo.
    #[test]
    fn program_text_identical_is_a_noop() {
        let mut e = editor_with("query", 2);
        e.layout_dirty = false;
        e.apply_program_text("query");
        assert_eq!(e.caret(), 2);
        assert!(!e.layout_dirty, "an identical write must not dirty the layout");
    }

    /// The selection anchor is mapped independently of the caret, so a
    /// selection spanning an untouched region survives the write.
    #[test]
    fn program_text_maps_anchor_independently() {
        let mut e = editor_with("hello world", 0);
        e.set_caret(5, Affinity::Upstream, true); // "hello" selected
        e.anchor = 0;
        e.apply_program_text("hello there world");
        assert_eq!((e.anchor, e.caret()), (0, 5), "selection over the prefix survives");
    }

    // ── Caret affinity ───────────────────────────────────────────────────────

    /// Every operation that moves the caret must leave the affinity describing
    /// where it *now* is. These are unobservable in Latin text, which is exactly
    /// why they are asserted here rather than left to a rendering check: a site
    /// that forgets the rule looks perfectly correct in every English fixture.
    #[test]
    fn typing_leaves_the_caret_trailing_what_was_typed() {
        let mut e = editor_with("", 0);
        e.insert("abc");
        assert_eq!((e.caret(), e.caret_affinity), (3, Affinity::Upstream));
    }

    /// Stepping forward puts the caret on the trailing side of the character it
    /// crossed; stepping backward puts it on the leading side. This is the whole
    /// user-visible payoff of the bit — the caret stays adjacent to the letter
    /// the arrow key just moved over, instead of jumping to the far end of a
    /// word whose direction differs.
    #[test]
    fn arrow_keys_anchor_to_the_character_they_cross() {
        let mut e = editor_with("abcdef", 3);
        e.move_right(false, false);
        assert_eq!((e.caret(), e.caret_affinity), (4, Affinity::Upstream));
        e.move_left(false, false);
        assert_eq!((e.caret(), e.caret_affinity), (3, Affinity::Downstream));
    }

    /// Home and End sit at the two ends of the line, and the character each can
    /// belong to is forced: there is nothing before Home and nothing after End.
    #[test]
    fn home_and_end_take_the_only_affinity_available() {
        let mut e = editor_with("abcdef", 3);
        e.home(false);
        assert_eq!((e.caret(), e.caret_affinity), (0, Affinity::Downstream));
        e.end(false);
        assert_eq!((e.caret(), e.caret_affinity), (6, Affinity::Upstream));
    }

    /// Deleting re-anchors the caret to text that still exists. A caret left
    /// pointing at a character that was just removed silently re-points at
    /// whatever slid into the gap.
    #[test]
    fn deleting_re_anchors_to_surviving_text() {
        let mut e = editor_with("abcdef", 3);
        e.backspace();
        assert_eq!((e.caret(), e.caret_affinity), (2, Affinity::Upstream));

        let mut e = editor_with("abcdef", 3);
        e.set_caret(3, Affinity::Downstream, false);
        e.delete_forward();
        assert_eq!(
            (e.caret(), e.caret_affinity),
            (3, Affinity::Upstream),
            "the index holds but the anchor must leave the deleted character"
        );
    }

    /// At index 0 the upstream position does not exist — there is no preceding
    /// character to trail — so the query must degenerate to the downstream one
    /// rather than underflow or ask DirectWrite for position -1.
    #[test]
    fn upstream_at_the_start_degenerates_to_downstream() {
        let mut e = editor_with("abc", 0);
        e.set_caret(0, Affinity::Upstream, false);
        assert_eq!(e.caret_query(), (0, false));
    }

    /// Upstream anywhere else is the trailing edge of the preceding character —
    /// the formulation that makes the two affinities different points in bidi
    /// text and identical points everywhere else.
    #[test]
    fn upstream_queries_the_preceding_characters_trailing_edge() {
        let mut e = editor_with("abc", 2);
        e.set_caret(2, Affinity::Upstream, false);
        assert_eq!(e.caret_query(), (1, true));
        e.set_caret(2, Affinity::Downstream, false);
        assert_eq!(e.caret_query(), (2, false));
    }

    /// A programmatic write that did not move the caret must not silently
    /// rewrite what it meant; one that did move it has no grounds to claim the
    /// caret still trails any particular character.
    #[test]
    fn program_text_only_resets_affinity_when_it_moves_the_caret() {
        let mut e = editor_with("hello world", 5);
        e.set_caret(5, Affinity::Upstream, false);
        e.apply_program_text("hello brave world");
        assert_eq!((e.caret(), e.caret_affinity), (5, Affinity::Upstream));

        let mut e = editor_with("hello world", 11);
        e.set_caret(11, Affinity::Upstream, false);
        e.apply_program_text("well hello world");
        assert_eq!((e.caret(), e.caret_affinity), (16, Affinity::Downstream));
    }

    // ── Scroll ───────────────────────────────────────────────────────────────

    /// The caret must end up inside the window whatever it started outside of,
    /// and a caret already visible must not move the view at all. The affinity
    /// work changed *which* x arrives here; this asserts the window logic is
    /// indifferent to that, so a caret in a right-to-left run is kept in view by
    /// the same rule as any other.
    #[test]
    fn scroll_brings_the_caret_into_view_and_otherwise_holds_still() {
        const W: f32 = 100.0;
        assert_eq!(scroll_for(0.0, 50.0, W), 0.0, "already visible: no movement");
        assert_eq!(scroll_for(0.0, 150.0, W), 50.0, "past the right edge: scroll to it");
        assert_eq!(scroll_for(80.0, 20.0, W), 20.0, "left of the window: scroll back");
        assert_eq!(scroll_for(0.0, 0.0, W), 0.0, "never scrolls negative");
        assert_eq!(scroll_for(-5.0, 10.0, W), 0.0, "a negative offset is clamped away");

        for &cx in &[0.0f32, 1.0, 99.0, 100.0, 250.0] {
            let s = scroll_for(0.0, cx, W);
            assert!(
                cx - s >= 0.0 && cx - s <= W,
                "caret {cx} must land inside the window after scrolling to {s}"
            );
        }
    }

    /// A thumbs-up is one character of two UTF-16 code units. Stepping or
    /// deleting by one unit splits it into a lone surrogate — text that is not
    /// valid UTF-16 and renders as a replacement glyph.
    #[test]
    fn clusters_are_not_split_by_movement_or_deletion() {
        let mut e = editor_with("a\u{1F44D}b", 0);
        e.end(false);
        e.move_left(false, false);
        assert_eq!(e.caret(), 3, "left from the end must clear the whole emoji");
        e.move_left(false, false);
        assert_eq!(e.caret(), 1, "and again must land before it, not inside it");

        let mut e = editor_with("a\u{1F44D}b", 0);
        e.end(false);
        e.backspace();
        e.backspace();
        assert_eq!(e.text(), "a", "backspace must remove the emoji whole");
    }

    // ── Word navigation ──────────────────────────────────────────────────────

    /// Punctuation is its own word, as it is in every other Windows text field.
    /// Splitting on whitespace alone made `foo.bar` one nine-unit token, so
    /// Ctrl+Left from the end jumped the whole thing.
    #[test]
    fn punctuation_is_its_own_word() {
        let mut e = editor_with("foo.bar", 7);
        e.move_left(true, false);
        assert_eq!(e.caret(), 4, "back to the start of `bar`");
        e.move_left(true, false);
        assert_eq!(e.caret(), 3, "then to the `.`, not past it");
        e.move_left(true, false);
        assert_eq!(e.caret(), 0, "then to the start of `foo`");
    }

    /// Ctrl+Right lands on the *beginning* of the next word, stepping over the
    /// whitespace that trails the current one rather than stopping in the gap.
    #[test]
    fn word_right_lands_on_the_next_word_not_the_gap() {
        let mut e = editor_with("alpha   beta", 0);
        e.move_right(true, false);
        assert_eq!(e.caret(), 8, "past `alpha` and its three spaces");
        e.move_right(true, false);
        assert_eq!(e.caret(), 12, "and to the end");
    }

    /// A run of punctuation is one word, not one word per mark.
    #[test]
    fn a_run_of_punctuation_is_one_word() {
        let mut e = editor_with("a...b", 0);
        e.move_right(true, false);
        assert_eq!(e.caret(), 1);
        e.move_right(true, false);
        assert_eq!(e.caret(), 4, "the three dots are a single word");
    }

    /// Word characters are the Unicode property, not the ASCII range — an
    /// accented or Cyrillic letter must not read as punctuation and split a
    /// word down its middle.
    #[test]
    fn accented_and_non_latin_letters_are_word_characters() {
        assert_eq!(class_of('é' as u16), CharClass::Word);
        assert_eq!(class_of('ж' as u16), CharClass::Word);
        assert_eq!(class_of('_' as u16), CharClass::Word);
        assert_eq!(class_of('7' as u16), CharClass::Word);
        assert_eq!(class_of('.' as u16), CharClass::Punct);
        assert_eq!(class_of('-' as u16), CharClass::Punct);
        assert_eq!(class_of(' ' as u16), CharClass::Space);
        // U+3000 IDEOGRAPHIC SPACE — whitespace `is_space` does not list.
        assert_eq!(class_of(0x3000), CharClass::Space);
    }

    /// Both halves of a surrogate pair are content, so a word move never stops
    /// inside a supplementary character.
    #[test]
    fn a_surrogate_pair_does_not_break_a_word() {
        let mut e = editor_with("a\u{1F44D}b c", 0);
        e.move_right(true, false);
        assert_eq!(e.caret(), 5, "the emoji is part of the word, not a break in it");
    }

    /// Word navigation must terminate at both ends whatever it starts on —
    /// including on whitespace, on punctuation, and on an empty buffer.
    #[test]
    fn word_navigation_terminates_from_every_starting_class() {
        for text in ["", " ", "..", "a b", "  a  ", ".a.", "\u{1F44D}"] {
            let n = text.encode_utf16().count();
            for start in 0..=n {
                let mut e = editor_with(text, start);
                for _ in 0..n + 2 {
                    e.move_right(true, false);
                }
                assert_eq!(e.caret(), n, "{text:?} from {start} must reach the end");

                let mut e = editor_with(text, start);
                for _ in 0..n + 2 {
                    e.move_left(true, false);
                }
                assert_eq!(e.caret(), 0, "{text:?} from {start} must reach the start");
            }
        }
    }
}
