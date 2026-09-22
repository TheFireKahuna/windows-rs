//! One editable document per window, and one row per mounted field.
//!
//! The active buffer holds keyboard and composition edits. Other fields retain their value
//! and selection in rows, which automation edits without changing the active buffer.
//! An active field's row also holds the application replacement it adopts on blur.

use super::{Affinity, Geometry, InputScope, Selection, Source, Update};
use crate::uia::action::TextAction;
use std::collections::VecDeque;
use std::sync::Arc;
use crate::layout::Rect;
use windows_scene::ControlId;

/// Holds an inactive field's state or the active field's pending application value.
struct Row {
    id: ControlId,
    scope: InputScope,
    revision: u64,
    text: Arc<[u16]>,
    selection: Selection,
}

/// The focused field's box and clip in screen pixels, and the scale a screen point divides
/// by. Pre-multiplied at publication, so two of the store's queries are a return.
///
/// A scale of zero means no field owns the view, and every position query fails.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct View {
    pub rect: Rect,
    pub clip: Rect,
    pub scale: f32,
}

/// What one publication tells the application changed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Change {
    /// The selection or the focus moved. The text is unchanged and nothing commits.
    Caret,
    /// The text changed inside a composition, or by adopting an application value.
    Text,
    /// A completed changed edit: typing, deletion, paste, or the end of a composition.
    Edit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    Home,
    End,
}

#[derive(Clone, Debug)]
pub(crate) enum Command {
    Move {
        to: Motion,
        select: bool,
    },
    Erase {
        forward: bool,
        word: bool,
    },
    /// Text at the selection: one character or a paste.
    Write(Arc<[u16]>),
    /// A caret placed at a point, in control-relative DIPs.
    Point {
        x: f32,
        extend: bool,
    },
    All,
    Select(Selection),
    Automation(TextAction),
}

impl Command {
    /// Reports whether the command reads cluster geometry shaped from the current text.
    fn needs_shaping(&self) -> bool {
        matches!(
            self,
            Self::Move {
                to: Motion::Left | Motion::Right,
                ..
            } | Self::Erase { word: false, .. }
                | Self::Point { .. }
        )
    }
}

const FOCUSED: u8 = 1;
const COMPOSING: u8 = 2;
const UPSTREAM: u8 = 4;

/// The window's text state: one editable buffer, one row per mounted field, one outbox.
///
/// `id` names the buffer's field. Buffer operations require that identity; row-targeted
/// automation does not. The buffer can outlive focus while a command awaits geometry or
/// a composition remains open.
#[derive(Default)]
pub(crate) struct Doc {
    pub id: Option<ControlId>,
    /// One number per field: advanced here on every accepted mutation, carried by every
    /// `Update`, quoted back by every `Source`, and stamped on every `Geometry`.
    pub revision: u64,
    pub view: View,
    text: Vec<u16>,
    anchor: u32,
    caret: u32,
    flags: u8,
    composition: (u32, u32),
    /// The value the open composition started from, for the one comparison at its end.
    start: Option<Arc<[u16]>>,
    shaped: Option<Arc<Geometry>>,
    pending: VecDeque<Command>,
    rows: Vec<Row>,
    out: Vec<Update>,
}

impl Doc {
    pub fn text(&self) -> &[u16] {
        &self.text
    }

    pub fn len(&self) -> u32 {
        self.text.len() as u32
    }

    pub fn selection(&self) -> Selection {
        Selection {
            anchor: self.anchor,
            caret: self.caret,
            affinity: match self.flags & UPSTREAM != 0 {
                true => Affinity::Upstream,
                false => Affinity::Downstream,
            },
        }
    }

    pub fn range(&self) -> core::ops::Range<u32> {
        self.selection().range()
    }

    /// Returns the geometry only where it was shaped from the current buffer.
    pub fn geometry(&self) -> Option<&Geometry> {
        self.shaped
            .as_deref()
            .filter(|g| g.revision == self.revision)
    }

    /// Reports whether a command is still waiting for geometry, which blur must respect.
    pub fn waiting(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn composing(&self) -> bool {
        self.flags & COMPOSING != 0
    }

    /// Returns the field that holds text focus, which the buffer's field outlives while a
    /// command waits for geometry.
    pub fn focused(&self) -> Option<ControlId> {
        self.id.filter(|_| self.flags & FOCUSED != 0)
    }

    pub fn scope(&self) -> InputScope {
        self.row().map_or(InputScope::Default, |row| row.scope)
    }

    fn row(&self) -> Option<&Row> {
        self.rows.iter().find(|row| Some(row.id) == self.id)
    }

    pub fn holds(&self, id: ControlId) -> bool {
        self.rows.iter().any(|row| row.id == id)
    }

    /// Reports whether `selection` is in range and splits no surrogate pair.
    pub fn accepts(&self, selection: Selection) -> bool {
        accepts(&self.text, selection)
    }

    /// ACP edits must preserve their declared offsets. Reject malformed text and ranges
    /// before mutation instead of clamping them into a different successful operation.
    pub fn replace(&mut self, start: u32, end: u32, text: &[u16]) -> Option<Change> {
        if self.id.is_none()
            || start > end
            || end > self.len()
            || !boundary(&self.text, start)
            || !boundary(&self.text, end)
            || !valid_text(text)
            || text.iter().copied().any(is_break)
        {
            return None;
        }
        if self.text[start as usize..end as usize] == *text {
            return Some(Change::Caret);
        }
        self.text
            .splice(start as usize..end as usize, text.iter().copied());
        self.revision += 1;
        self.place(start + text.len() as u32, None, Affinity::Upstream);
        Some(match self.composing() {
            true => Change::Text,
            false => Change::Edit,
        })
    }

    /// Opens, moves or ends the composition.
    ///
    /// Only its end compares the final value against the value it started from, so an input
    /// method that revises its own text repeatedly still produces one completed edit.
    pub fn compose(&mut self, range: Option<(u32, u32)>) -> Option<Change> {
        match range {
            Some(range) if self.id.is_some() && range.0 <= range.1 && range.1 <= self.len() => {
                if !self.composing() {
                    self.start = Some(Arc::from(&self.text[..]));
                    self.flags |= COMPOSING;
                }
                self.composition = range;
                Some(Change::Caret)
            }
            None if self.composing() => {
                self.flags &= !COMPOSING;
                let changed = self
                    .start
                    .take()
                    .is_some_and(|before| *before != self.text[..]);
                Some(match changed {
                    true => Change::Edit,
                    false => Change::Caret,
                })
            }
            _ => None,
        }
    }

    /// Queues or runs one command, publishing whatever it changed.
    ///
    /// Later text input stays behind a waiting command, so the order the user typed in
    /// survives the wait.
    pub fn command(&mut self, command: Command) {
        if self.id.is_none() {
            return;
        }
        if self.waiting() || (command.needs_shaping() && self.geometry().is_none()) {
            self.pending.push_back(command);
            return;
        }
        let change = self.run(command);
        self.emit(change);
    }

    /// Applies one revision-checked automation transaction without moving focus.
    pub fn automation(&mut self, action: TextAction) {
        let (id, revision) = match &action {
            TextAction::Replace(id, revision, _) | TextAction::Select(id, revision, _) => (*id, *revision),
        };
        if self.id == Some(id) {
            self.command(Command::Automation(action));
            return;
        }
        let Some(row) = self.rows.iter_mut().find(|row| row.id == id && row.revision == revision) else {
            return;
        };
        let mut commit = None;
        let text = match action {
            TextAction::Replace(_, _, text) => {
                let text = stripped(&text);
                if !valid_text(&text) { return; }
                if *row.text == text { return; }
                row.revision += 1;
                row.selection = Selection { affinity: Affinity::Upstream, ..Selection::at(text.len() as u32) };
                commit = Some(Arc::from(String::from_utf16_lossy(&text)));
                row.text = text.into();
                Some(Arc::clone(&row.text))
            }
            TextAction::Select(_, _, selection) => {
                if row.scope == InputScope::Password || !accepts(&row.text, selection) { return; }
                if row.selection == selection { return; }
                row.selection = selection;
                None
            }
        };
        self.publish(id, false, Some((text, commit)));
    }

    /// Installs shaped geometry and releases whatever was waiting for it.
    pub fn layout(&mut self, id: ControlId, geometry: &Arc<Geometry>) {
        let stale = self.id != Some(id)
            || geometry.revision != self.revision
            || self.shaped.as_ref().is_some_and(|old| {
                old.revision == geometry.revision && old.layout_revision > geometry.layout_revision
            });
        if stale {
            return;
        }
        self.shaped = Some(Arc::clone(geometry));
        while self
            .pending
            .front()
            .is_some_and(|command| !command.needs_shaping() || self.geometry().is_some())
        {
            let Some(command) = self.pending.pop_front() else {
                break;
            };
            let change = self.run(command);
            self.emit(change);
        }
        self.settle();
    }

    fn run(&mut self, command: Command) -> Option<Change> {
        let selection = self.selection();
        let range = selection.range();
        match command {
            Command::Write(text) => {
                let text = stripped(&text);
                self.replace(range.start, range.end, &text)
            }
            Command::Erase { forward, word } => {
                let (start, end) = match () {
                    () if !range.is_empty() => (range.start, range.end),
                    () if forward => (selection.caret, self.step(selection.caret, true, word)),
                    () => (self.step(selection.caret, false, word), selection.caret),
                };
                self.replace(start, end, &[])
            }
            Command::Move { to, select } => {
                let (at, affinity) = self.destination(to, selection, select);
                self.place(at, select.then_some(selection.anchor), affinity);
                Some(Change::Caret)
            }
            Command::Point { x, extend } => {
                let (at, affinity) = {
                    let geometry = self.geometry()?;
                    geometry.hit(x - geometry.origin.x)
                };
                self.place(at, extend.then_some(selection.anchor), affinity);
                Some(Change::Caret)
            }
            Command::All => {
                self.place(self.len(), Some(0), Affinity::Upstream);
                Some(Change::Caret)
            }
            Command::Select(wanted) => self.accepts(wanted).then(|| {
                self.place(wanted.caret, Some(wanted.anchor), wanted.affinity);
                Change::Caret
            }),
            Command::Automation(action) => {
                let (id, revision) = match &action {
                    TextAction::Replace(id, revision, _) | TextAction::Select(id, revision, _) => (*id, *revision),
                };
                if self.id != Some(id) || self.revision != revision || self.composing() {
                    return None;
                }
                match action {
                    TextAction::Replace(_, _, text) => self.replace(0, self.len(), &stripped(&text)),
                    TextAction::Select(_, _, selection) if self.scope() != InputScope::Password => {
                        self.run(Command::Select(selection))
                    }
                    TextAction::Select(..) => None,
                }
            }
        }
    }

    /// Returns where a motion lands and the affinity it lands with.
    fn destination(&self, to: Motion, selection: Selection, select: bool) -> (u32, Affinity) {
        let range = selection.range();
        let collapsing = !select && !range.is_empty();
        let at = selection.caret;
        match to {
            Motion::Home => (0, Affinity::Downstream),
            Motion::End => (self.len(), Affinity::Upstream),
            Motion::Left if collapsing => (range.start, Affinity::Downstream),
            Motion::Right if collapsing => (range.end, Affinity::Upstream),
            Motion::Left => (self.step(at, false, false), Affinity::Downstream),
            Motion::Right => (self.step(at, true, false), Affinity::Upstream),
            Motion::WordLeft => (self.step(at, false, true), Affinity::Downstream),
            Motion::WordRight => (self.step(at, true, true), Affinity::Upstream),
        }
    }

    /// Returns the next position in one direction: a cluster boundary, or a word edge.
    ///
    /// A word edge is logical and needs no geometry, which is why word motion never queues.
    fn step(&self, at: u32, forward: bool, word: bool) -> u32 {
        if !word {
            return match self.geometry() {
                Some(g) if forward => g.next(at),
                Some(g) => g.previous(at),
                None => at,
            };
        }
        let mut i = at as usize;
        if forward {
            let kind = self.text.get(i).copied().map_or(0, class);
            while i < self.text.len() && class(self.text[i]) == kind {
                i += 1;
            }
            while i < self.text.len() && class(self.text[i]) == 0 {
                i += 1;
            }
        } else {
            while i > 0 && class(self.text[i - 1]) == 0 {
                i -= 1;
            }
            let kind = if i > 0 { class(self.text[i - 1]) } else { 0 };
            while i > 0 && class(self.text[i - 1]) == kind {
                i -= 1;
            }
        }
        i as u32
    }

    // Affinity is required at the only caret mutation site: an index alone is ambiguous at a
    // bidi boundary. All input paths, including ACP SetSelection, go through here.
    fn place(&mut self, caret: u32, anchor: Option<u32>, affinity: Affinity) {
        self.caret = caret.min(self.len());
        self.anchor = anchor.unwrap_or(self.caret).min(self.len());
        self.flags &= !UPSTREAM;
        if affinity == Affinity::Upstream {
            self.flags |= UPSTREAM;
        }
    }

    /// Records the application's value for a field, creating the row on its first source.
    ///
    /// Line breaks are stripped once, here, so the row, the echo and the buffer are the same
    /// text and an equal echo compares equal.
    pub fn source(&mut self, source: &Source) {
        let text: Arc<[u16]> = stripped(&source.text).into();
        match self.rows.iter().position(|row| row.id == source.id) {
            Some(at) if source.based_on < self.rows[at].revision => return,
            Some(at) => {
                let row = &mut self.rows[at];
                if row.text != text {
                    row.selection = Selection::at(text.len() as u32);
                }
                (row.scope, row.revision, row.text) = (source.scope, source.based_on, text);
            }
            None => self.rows.push(Row {
                id: source.id,
                scope: source.scope,
                revision: source.based_on,
                selection: Selection::at(text.len() as u32),
                text,
            }),
        }
        match self.id == Some(source.id) {
            // A focused field keeps typing over it: the row is the replacement, and it lands
            // at blur or at the end of a composition.
            true => self.settle(),
            // An unfocused field is shaped from what input holds, not from what the
            // application sent, because line breaks are stripped here.
            false => self.publish(source.id, false, None),
        }
    }

    /// Applies the application's value over the buffer where it is allowed to land.
    ///
    /// It lands only when the field is neither focused nor composing and no key is still
    /// waiting for shaped geometry, and only when the application based it on this revision
    /// or a later one: later typing supersedes it.
    pub fn settle(&mut self) {
        if self.id.is_none() || self.flags & (FOCUSED | COMPOSING) != 0 || self.waiting() {
            return;
        }
        let revision = self.revision;
        let Some(text) = self
            .row()
            .filter(|row| row.revision >= revision && *row.text != self.text[..])
            .map(|row| Arc::clone(&row.text))
        else {
            return;
        };
        self.text.clear();
        self.text.extend_from_slice(&text);
        self.revision += 1;
        self.place(self.caret, Some(self.anchor), self.selection().affinity);
        self.emit(Some(Change::Text));
    }

    /// Moves text focus, retiring the buffer into its row and hydrating the new one.
    ///
    /// Blur keeps the buffer bound to its field while a command waits for shaped geometry or
    /// an input method holds a composition open, because both still have to finish. Taking
    /// focus to another field retires it either way.
    pub fn focus(&mut self, id: Option<ControlId>) {
        if self.focused() == id {
            return;
        }
        if self.id == id {
            self.flags |= FOCUSED;
            self.emit(Some(Change::Caret));
            return;
        }
        if self.id.is_some() {
            self.flags &= !FOCUSED;
            self.emit(Some(Change::Caret));
            self.settle();
            if id.is_none() && (self.waiting() || self.composing()) {
                return;
            }
            self.retire();
        }
        let Some((revision, text, selection)) = id
            .and_then(|id| self.rows.iter().find(|row| row.id == id))
            .map(|row| (row.revision, Arc::clone(&row.text), row.selection))
        else {
            return;
        };
        (self.id, self.flags, self.revision) = (id, FOCUSED, revision);
        self.text.extend_from_slice(&text);
        self.place(selection.caret, Some(selection.anchor), selection.affinity);
        self.emit(Some(Change::Caret));
    }

    /// Releases a field's storage on unmount, dropping the buffer with it.
    pub fn forget(&mut self, id: ControlId) {
        if self.id == Some(id) {
            self.focus(None);
            self.clear();
        }
        self.rows.retain(|row| row.id != id);
    }

    /// Writes the buffer back into its row, then clears it.
    ///
    /// The row is the application's word until the user leaves the field; from here it is
    /// the user's, at the revision the user left it at.
    fn retire(&mut self) {
        if let Some(old) = self.id {
            let (revision, text, selection) = (self.revision, Arc::<[u16]>::from(&self.text[..]), self.selection());
            if let Some(row) = self.rows.iter_mut().find(|row| row.id == old) {
                (row.revision, row.text) = (revision, text);
                row.selection = selection;
            }
        }
        self.clear();
    }

    /// Drops the buffer, the queue and the composition, keeping the rows, the outbox and the
    /// published view, which belong to the window rather than to any field.
    fn clear(&mut self) {
        let (rows, out, view) = (
            core::mem::take(&mut self.rows),
            core::mem::take(&mut self.out),
            self.view,
        );
        *self = Self {
            rows,
            out,
            view,
            ..Self::default()
        };
    }

    /// Publishes one transaction for the application thread to shape.
    pub fn emit(&mut self, change: Option<Change>) {
        let Some((id, change)) = self.id.zip(change) else {
            return;
        };
        let text = (change != Change::Caret).then(|| Arc::from(&self.text[..]));
        let commit =
            (change == Change::Edit).then(|| Arc::from(String::from_utf16_lossy(&self.text)));
        self.publish(id, true, Some((text, commit)));
    }

    /// Publishes the active buffer or a stored field through the shared outbox.
    fn publish(
        &mut self,
        id: ControlId,
        buffered: bool,
        edit: Option<(Option<Arc<[u16]>>, Option<Arc<str>>)>,
    ) {
        let row = self.rows.iter().find(|row| row.id == id);
        let revision = match buffered {
            true => self.revision,
            false => row.map_or(0, |row| row.revision),
        };
        let (text, commit) = match edit {
            Some(edit) => edit,
            None => (row.map(|row| Arc::clone(&row.text)), None),
        };
        let selection = match buffered {
            true => self.selection(),
            false => row.map_or_else(|| Selection::at(0), |row| row.selection),
        };
        self.out.push(Update {
            id,
            revision,
            text,
            selection,
            composition: (buffered && self.composing())
                .then(|| self.composition.0..self.composition.1),
            focused: buffered && self.flags & FOCUSED != 0,
            commit,
        });
    }

    pub fn drain(&mut self, out: &mut Vec<Update>) {
        out.append(&mut self.out);
    }
}

/// CR, LF, NEL, line separator and paragraph separator, none of which a single-line field
/// can hold.
fn is_break(unit: u16) -> bool {
    matches!(unit, 0x0a | 0x0d | 0x0085 | 0x2028 | 0x2029)
}

fn stripped(text: &[u16]) -> Vec<u16> {
    text.iter().copied().filter(|&u| !is_break(u)).collect()
}

fn valid_text(text: &[u16]) -> bool {
    char::decode_utf16(text.iter().copied()).all(|c| c.is_ok())
}

fn accepts(text: &[u16], selection: Selection) -> bool {
    [selection.anchor, selection.caret].into_iter()
        .all(|at| at as usize <= text.len() && boundary(text, at))
}

/// Reports whether `at` falls between code units rather than inside a surrogate pair.
fn boundary(text: &[u16], at: u32) -> bool {
    let at = at as usize;
    at == 0
        || at >= text.len()
        || !(0xd800..0xdc00).contains(&text[at - 1])
        || !(0xdc00..0xe000).contains(&text[at])
}

/// Zero for whitespace, one for word characters, two for everything else. An unpaired
/// surrogate counts as a word character, so a supplementary character is one word.
fn class(unit: u16) -> u8 {
    match char::from_u32(u32::from(unit)) {
        Some(c) if c.is_whitespace() => 0,
        Some(c) if c.is_alphanumeric() || c == '_' => 1,
        None => 1,
        _ => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::super::Cluster;
    use super::*;

    fn source(text: &str, based_on: u64) -> Source {
        Source {
            id: ControlId::default(),
            scope: InputScope::Default,
            based_on,
            text: text.encode_utf16().collect::<Vec<_>>().into(),
        }
    }

    /// A document focused on one field holding `text`, with its mount updates dropped.
    fn focused(text: &str) -> Doc {
        let mut doc = Doc::default();
        doc.source(&source(text, 0));
        doc.focus(Some(ControlId::default()));
        doc.out.clear();
        doc
    }

    /// Runs one mutation the way the text store runs it, publishing what it changed.
    fn edit(doc: &mut Doc, mutate: impl FnOnce(&mut Doc) -> Option<Change>) -> bool {
        let change = mutate(doc);
        doc.emit(change);
        change.is_some()
    }

    /// What input holds for `id`: the buffer while the document is bound to it, the row
    /// otherwise.
    fn held(doc: &Doc, id: ControlId) -> Vec<u16> {
        match doc.id == Some(id) {
            true => doc.text.clone(),
            false => doc
                .rows
                .iter()
                .find(|row| row.id == id)
                .map_or_else(Vec::new, |row| row.text.to_vec()),
        }
    }

    fn commits(doc: &Doc) -> Vec<String> {
        doc.out
            .iter()
            .filter_map(|u| u.commit.as_deref().map(str::to_owned))
            .collect()
    }

    fn shaped(revision: u64, text: &str) -> Arc<Geometry> {
        let mut at = 0u32;
        let clusters: Vec<Cluster> = text
            .chars()
            .map(|ch| {
                let start = at;
                at += ch.len_utf16() as u32;
                Cluster {
                    start,
                    end: at,
                    rect: windows_text::Rect {
                        x: start as f32,
                        y: 0.0,
                        w: 1.0,
                        h: 10.0,
                    },
                    leading: start as f32,
                    trailing: at as f32,
                }
            })
            .collect();
        Arc::new(Geometry {
            revision,
            clusters: clusters.into(),
            ..Default::default()
        })
    }

    /// One input method edit revises its own text several times. Only the end of the
    /// composition compares the value against the one it started from, so the application is
    /// told once.
    #[test]
    fn several_writes_inside_one_composition_are_one_completed_edit() {
        let mut doc = focused("");
        edit(&mut doc, |d| d.compose(Some((0, 0))));
        for (start, end, text) in [(0u32, 0u32, "n"), (0, 1, "ni"), (0, 2, "\u{306b}")] {
            let units: Vec<u16> = text.encode_utf16().collect();
            assert!(edit(&mut doc, |d| d.replace(start, end, &units)));
        }
        assert!(commits(&doc).is_empty(), "nothing commits mid-composition");
        edit(&mut doc, |d| d.compose(None));
        assert_eq!(commits(&doc), ["\u{306b}"]);
    }

    /// The same three writes outside a composition are three completed edits, which is what
    /// makes the composition the transaction rather than the write.
    #[test]
    fn the_same_writes_outside_a_composition_are_three_completed_edits() {
        let mut doc = focused("");
        for (start, end, text) in [(0u32, 0u32, "a"), (1, 1, "b"), (2, 2, "c")] {
            let units: Vec<u16> = text.encode_utf16().collect();
            assert!(edit(&mut doc, |d| d.replace(start, end, &units)));
        }
        assert_eq!(commits(&doc), ["a", "ab", "abc"]);
    }

    /// A field unmounted while an input method still holds a composition open commits nothing
    /// and keeps no storage. Its buffer goes with its row.
    #[test]
    fn unmounting_a_field_mid_composition_commits_nothing_and_keeps_no_storage() {
        let id = ControlId::default();
        let mut doc = focused("");
        edit(&mut doc, |d| d.compose(Some((0, 0))));
        assert!(edit(&mut doc, |d| d.replace(0, 0, &[0x306b])));
        doc.out.clear();

        doc.forget(id);
        assert!(commits(&doc).is_empty(), "an unmount is not an edit");
        assert!(!doc.composing());
        assert_eq!(doc.focused(), None);
        assert!(!doc.holds(id));
        assert!(held(&doc, id).is_empty());
    }

    /// Focus moving between two fields while both are being typed into keeps each buffer with
    /// its own field, and each field's completed edit names that field.
    #[test]
    fn rapid_input_across_two_fields_keeps_each_buffer_with_its_own_field() {
        let (first, second) = (ControlId::FIRST, ControlId::default());
        assert_ne!(first, second, "two fields, two control ids");
        let mut doc = Doc::default();
        for id in [first, second] {
            doc.source(&Source {
                id,
                scope: InputScope::Default,
                based_on: 0,
                text: Vec::new().into(),
            });
        }
        doc.out.clear();

        doc.focus(Some(first));
        assert!(edit(&mut doc, |d| d.replace(0, 0, &[97])));
        doc.focus(Some(second));
        assert!(edit(&mut doc, |d| d.replace(0, 0, &[98])));
        doc.focus(Some(first));
        assert!(edit(&mut doc, |d| d.replace(1, 1, &[99])));

        assert_eq!(held(&doc, first), [97, 99]);
        assert_eq!(held(&doc, second), [98]);
        let owned: Vec<(ControlId, String)> = doc
            .out
            .iter()
            .filter_map(|u| u.commit.as_deref().map(|text| (u.id, text.to_owned())))
            .collect();
        assert_eq!(
            owned,
            [
                (first, "a".to_owned()),
                (second, "b".to_owned()),
                (first, "ac".to_owned())
            ]
        );
    }

    /// A field remounted into the slot a released one held starts from the value its own
    /// source states, not from whatever the previous occupant was holding.
    #[test]
    fn a_remounted_field_starts_from_its_own_source() {
        let id = ControlId::default();
        let mut doc = focused("");
        assert!(edit(&mut doc, |d| d.replace(0, 0, &[97, 98])));
        doc.forget(id);
        doc.out.clear();

        doc.source(&source("z", 0));
        doc.focus(Some(id));
        assert_eq!(held(&doc, id), [122]);
        assert_eq!(doc.revision, 0, "and at the revision its source states");
        assert!(commits(&doc).is_empty());
    }

    #[test]
    fn composition_is_a_transaction_even_with_empty_range() {
        let mut doc = focused("");
        edit(&mut doc, |d| d.compose(Some((0, 0))));
        assert!(edit(&mut doc, |d| d.replace(0, 0, &[0x3042])));
        assert!(commits(&doc).is_empty());
        edit(&mut doc, |d| d.compose(None));
        doc.focus(None);
        assert_eq!(commits(&doc).len(), 1);
    }

    #[test]
    fn invalid_acp_does_not_change_the_document() {
        let mut doc = focused("a👍b");
        for (a, b) in [(3, 1), (2, 3), (0, 9)] {
            assert!(doc.replace(a, b, &[]).is_none());
        }
        assert!(doc.replace(0, 0, &[0xd800]).is_none());
        assert_eq!(String::from_utf16(doc.text()).unwrap(), "a👍b");
    }

    #[test]
    fn equal_echo_is_a_noop_and_blur_does_not_recommit() {
        let mut doc = focused("a");
        edit(&mut doc, |d| d.replace(1, 1, &[98]));
        let published = doc.out.len();
        let selection = doc.selection();
        doc.source(&source("ab", 1));
        assert_eq!(doc.out.len(), published);
        assert_eq!(doc.selection(), selection);
        doc.focus(None);
        assert_eq!(commits(&doc).len(), 1);
    }

    #[test]
    fn equal_latest_echo_cancels_an_older_replacement() {
        let mut doc = focused("");
        doc.source(&source("a", 0));
        doc.source(&source("", 0));
        doc.focus(None);
        assert!(held(&doc, ControlId::default()).is_empty());
    }

    #[test]
    fn composition_cancellation_and_selection_never_commit() {
        let mut doc = focused("");
        edit(&mut doc, |d| d.compose(Some((0, 0))));
        edit(&mut doc, |d| d.replace(0, 0, &[97]));
        edit(&mut doc, |d| d.replace(0, 1, &[]));
        edit(&mut doc, |d| d.compose(None));
        doc.command(Command::All);
        doc.focus(None);
        assert!(commits(&doc).is_empty());
    }

    #[test]
    fn paste_is_single_line_and_one_commit() {
        let mut doc = focused("");
        doc.command(Command::Write("a\r\nb\nc".encode_utf16().collect()));
        assert_eq!(doc.text(), [97, 98, 99]);
        assert_eq!(commits(&doc).len(), 1);
    }

    #[test]
    fn unicode_line_separators_are_removed_on_paste() {
        let mut doc = focused("");
        doc.command(Command::Write(
            "a\u{0085}b\u{2028}c\u{2029}d".encode_utf16().collect(),
        ));
        assert_eq!(doc.text(), [97, 98, 99, 100]);
    }

    #[test]
    fn delayed_geometry_preserves_rapid_edit_navigation_order() {
        let id = ControlId::default();
        let mut doc = focused("");
        doc.command(Command::Write(Arc::from([97u16])));
        doc.command(Command::Move {
            to: Motion::Left,
            select: false,
        });
        doc.command(Command::Write(Arc::from([98u16])));
        assert_eq!(doc.text(), [97]);
        doc.layout(id, &shaped(0, ""));
        assert_eq!(doc.text(), [97], "geometry for another revision is ignored");
        doc.layout(id, &shaped(1, "a"));
        assert_eq!(doc.text(), [98, 97]);
        assert_eq!(commits(&doc), ["a", "ba"]);
    }

    #[test]
    fn blur_waits_for_a_preceding_key_before_applying_a_model_replacement() {
        let id = ControlId::default();
        let mut doc = focused("");
        doc.command(Command::Write(Arc::from([97u16])));
        doc.command(Command::Move {
            to: Motion::Left,
            select: false,
        });
        doc.command(Command::Write(Arc::from([98u16])));
        doc.source(&source("x", 1));
        doc.focus(None);
        assert_eq!(held(&doc, id), [97]);
        doc.layout(id, &shaped(1, "a"));
        assert_eq!(held(&doc, id), [98, 97]);
        assert!(!doc.waiting());
    }

    #[test]
    fn empty_composition_defers_source_until_end_after_blur() {
        let id = ControlId::default();
        let mut doc = focused("");
        edit(&mut doc, |d| d.compose(Some((0, 0))));
        doc.source(&source("x", 0));
        doc.focus(None);
        assert!(held(&doc, id).is_empty());
        edit(&mut doc, |d| d.compose(None));
        doc.settle();
        assert_eq!(held(&doc, id), [120]);
        assert!(commits(&doc).is_empty());
    }

    #[test]
    fn focus_hydrates_a_row_at_its_own_revision() {
        let id = ControlId::default();
        let mut doc = Doc::default();
        doc.source(&source("ab", 7));
        doc.focus(Some(id));
        assert_eq!(doc.revision, 7);
        doc.layout(id, &shaped(7, "ab"));
        assert!(
            doc.geometry().is_some(),
            "geometry shaped while unfocused still matches the hydrated buffer"
        );
    }

    #[test]
    fn blur_writes_the_buffer_back_into_the_row() {
        let id = ControlId::default();
        let mut doc = focused("a");
        edit(&mut doc, |d| d.replace(1, 1, &[98]));
        doc.focus(None);
        assert_eq!(doc.id, None, "the buffer is released with the focus");
        let row = doc.rows.iter().find(|row| row.id == id).unwrap();
        assert_eq!(*row.text, [97, 98]);
        assert_eq!(row.revision, 1);
    }

    #[test]
    fn an_unfocused_source_publishes_what_input_holds() {
        let mut doc = Doc::default();
        doc.source(&source("a\r\nb", 0));
        let echo = doc.out.last().expect("a new row publishes its value");
        assert!(!echo.focused);
        assert!(echo.commit.is_none());
        assert_eq!(*echo.text.clone().unwrap(), [97, 98]);
    }

    #[test]
    fn background_automation_preserves_another_fields_composition() {
        let mut doc = focused("typing");
        let active = doc.id.unwrap();
        let other = ControlId::FIRST;
        assert_ne!(active, other);
        doc.source(&Source { id: other, ..source("old", 7) });
        edit(&mut doc, |d| d.compose(Some((0, 6))));
        doc.out.clear();
        doc.automation(TextAction::Replace(other, 7, "new\r\ntext".encode_utf16().collect()));
        assert_eq!(doc.focused(), Some(active));
        assert!(doc.composing());
        assert_eq!(doc.text(), &"typing".encode_utf16().collect::<Vec<_>>());
        assert_eq!(doc.revision, 0);
        assert_eq!(commits(&doc), ["newtext"]);
        assert_eq!(doc.out.len(), 1);
        assert_eq!(doc.out[0].id, other);
        assert_eq!(doc.out[0].revision, 8);
        assert!(!doc.out[0].focused);
        assert_eq!(doc.out[0].composition, None);
        doc.source(&Source { id: other, ..source("late", 7) });
        assert_eq!(held(&doc, other), "newtext".encode_utf16().collect::<Vec<_>>());
    }

    #[test]
    fn background_selection_survives_echo_and_focus_without_committing() {
        let mut doc = Doc::default();
        let id = ControlId::default();
        doc.source(&source("a😀b", 4));
        doc.out.clear();
        let wanted = Selection { anchor: 1, caret: 3, affinity: Affinity::Upstream };
        doc.automation(TextAction::Select(id, 4, wanted));
        assert_eq!(doc.focused(), None);
        assert_eq!(doc.id, None);
        assert_eq!(doc.out.len(), 1);
        assert_eq!(doc.out[0].selection, wanted);
        assert!(doc.out[0].text.is_none());
        assert!(doc.out[0].commit.is_none());
        doc.source(&source("a😀b", 4));
        assert_eq!(doc.out.last().unwrap().selection, wanted);
        doc.focus(Some(id));
        assert_eq!(doc.selection(), wanted);
        assert!(commits(&doc).is_empty());
    }

    #[test]
    fn automation_replacement_is_one_transaction_and_preserves_selection_on_rejection() {
        let mut doc = focused("abc");
        let id = doc.id.unwrap();
        doc.command(Command::Select(Selection::at(1)));
        doc.out.clear();
        doc.automation(TextAction::Replace(id, 0, vec![0xd800]));
        assert_eq!(doc.selection(), Selection::at(1));
        assert!(doc.out.is_empty());
        doc.automation(TextAction::Replace(id, 0, "xyz".encode_utf16().collect()));
        assert_eq!(doc.out.len(), 1);
        assert_eq!(commits(&doc), ["xyz"]);
        assert_eq!(doc.out[0].revision, 1);
        assert!(doc.out[0].focused);
        assert_eq!(doc.out[0].selection.caret, 3);
    }

    #[test]
    fn automation_does_not_replace_the_composing_field() {
        let mut doc = focused("composing");
        let id = doc.id.unwrap();
        edit(&mut doc, |d| d.compose(Some((0, 9))));
        doc.out.clear();
        let selection = doc.selection();
        doc.automation(TextAction::Replace(id, 0, vec![]));
        doc.automation(TextAction::Select(id, 0, Selection::at(0)));
        assert_eq!(doc.selection(), selection);
        assert_eq!(doc.text(), &"composing".encode_utf16().collect::<Vec<_>>());
        assert!(doc.composing());
        assert!(doc.out.is_empty());
    }

    #[test]
    fn queued_automation_rechecks_revision_after_preceding_keyboard_edit() {
        let mut doc = focused("ab");
        let id = doc.id.unwrap();
        doc.command(Command::Erase { forward: false, word: false });
        assert!(doc.waiting());
        doc.automation(TextAction::Replace(id, 0, "overwrite".encode_utf16().collect()));
        assert!(doc.out.is_empty());
        doc.layout(id, &shaped(0, "ab"));
        assert_eq!(doc.text(), &[b'a' as u16]);
        assert_eq!(commits(&doc), ["a"]);
        assert!(!doc.waiting());
    }

    #[test]
    fn automation_rejects_stale_missing_and_invalid_background_edits() {
        let mut doc = Doc::default();
        let id = ControlId::default();
        doc.source(&source("a😀b", 4));
        doc.out.clear();
        doc.automation(TextAction::Replace(id, 3, vec![b'x' as u16]));
        doc.automation(TextAction::Replace(id, 4, vec![0xd800]));
        doc.automation(TextAction::Replace(ControlId::FIRST, 4, vec![]));
        doc.automation(TextAction::Select(id, 4, Selection::at(2)));
        doc.automation(TextAction::Select(id, 4, Selection::at(5)));
        assert!(doc.out.is_empty());
        assert_eq!(held(&doc, id), "a😀b".encode_utf16().collect::<Vec<_>>());
        doc.source(&Source { scope: InputScope::Password, ..source("secret", 4) });
        doc.out.clear();
        doc.automation(TextAction::Select(id, 4, Selection::at(1)));
        assert!(doc.out.is_empty());
        doc.automation(TextAction::Replace(id, 4, "changed".encode_utf16().collect()));
        assert_eq!(commits(&doc), ["changed"]);
        doc.out.clear();
        doc.forget(id);
        doc.automation(TextAction::Replace(id, 5, vec![]));
        assert!(doc.out.is_empty());
    }
}
