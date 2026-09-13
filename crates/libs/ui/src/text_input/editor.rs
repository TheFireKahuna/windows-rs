//! The single edit authority. Geometry-dependent commands pause without blocking a thread.

use super::{Affinity, Geometry, InputScope, Selection, Update};
use std::{collections::VecDeque, sync::Arc};
use windows_scene::ControlId;

#[derive(Clone, Debug)]
pub(crate) enum Command {
    Insert(Vec<u16>),
    Character([u16; 2], u8),
    Backspace,
    Delete,
    Left { word: bool, select: bool },
    Right { word: bool, select: bool },
    Home { select: bool },
    End { select: bool },
    All,
    Point { x: f32, select: bool },
    Select(Selection),
}

pub(crate) struct Editor {
    pub id: ControlId,
    pub scope: InputScope,
    pub text: Vec<u16>,
    pub revision: u64,
    selection: Selection,
    pub composition: Option<core::ops::Range<u32>>,
    pub focused: bool,
    pub geometry: Option<Arc<Geometry>>,
    pending_source: Option<(u64, Arc<[u16]>)>,
    pending: VecDeque<Command>,
    committed: Arc<[u16]>,
    published: Arc<[u16]>,
    pub updates: Vec<Update>,
}

impl Editor {
    pub fn new(id: ControlId, scope: InputScope, text: &[u16]) -> Self {
        let published: Arc<[u16]> = Arc::from(text);
        Self {
            id,
            scope,
            text: text.to_vec(),
            revision: 0,
            selection: Selection::default(),
            composition: None,
            focused: false,
            geometry: None,
            pending_source: None,
            pending: VecDeque::new(),
            committed: published.clone(),
            published,
            updates: Vec::new(),
        }
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }
    pub fn accepts_selection(&self, selection: Selection) -> bool {
        [selection.anchor, selection.caret]
            .into_iter()
            .all(|at| at as usize <= self.text.len() && boundary(&self.text, at as usize))
    }
    pub fn waiting(&self) -> bool {
        !self.pending.is_empty()
    }

    // Affinity is required at the only caret mutation site: an index alone is ambiguous
    // at a bidi boundary. All input paths, including ACP SetSelection, go through here.
    fn set_caret(&mut self, at: u32, affinity: Affinity, select: bool) {
        self.selection.caret = at.min(self.text.len() as u32);
        self.selection.affinity = affinity;
        if !select {
            self.selection.anchor = self.selection.caret;
        }
    }

    pub fn source(&mut self, based_on: u64, text: Arc<[u16]>) {
        if based_on < self.revision {
            return;
        }
        if text.as_ref() == self.text {
            self.pending_source = None;
            return;
        }
        if self.focused || self.composition.is_some() || self.waiting() {
            self.pending_source = Some((based_on, text));
        } else {
            self.replace_source(&text);
        }
    }

    fn replace_source(&mut self, text: &[u16]) {
        let old_len = self.text.len();
        let prefix = self
            .text
            .iter()
            .zip(text)
            .take_while(|(a, b)| a == b)
            .count();
        let mut suffix = 0;
        while suffix < (old_len - prefix).min(text.len() - prefix)
            && self.text[old_len - 1 - suffix] == text[text.len() - 1 - suffix]
        {
            suffix += 1;
        }
        let map = |at: u32| -> u32 {
            let at = at as usize;
            if at <= prefix {
                at as u32
            } else if at >= old_len - suffix {
                (text.len() - suffix + at - (old_len - suffix)) as u32
            } else {
                (text.len() - suffix) as u32
            }
        };
        let anchor = map(self.selection.anchor);
        let caret = map(self.selection.caret);
        let affinity = if caret == self.selection.caret {
            self.selection.affinity
        } else {
            Affinity::Downstream
        };
        self.text.clear();
        self.text.extend_from_slice(text);
        let snap = |at: u32| {
            if boundary(text, at as usize) {
                at
            } else {
                at.saturating_sub(1)
            }
        };
        self.set_caret(snap(caret), affinity, true);
        self.selection.anchor = snap(anchor);
        self.revision += 1;
        self.publish(true, false);
        self.committed = self.published.clone();
    }

    pub fn focus(&mut self, focused: bool) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        self.apply_pending_source();
        self.publish(false, false);
    }

    // A key preceding blur may still await shaped geometry. Its mutation must happen
    // before deciding whether the model replacement has been superseded.
    fn apply_pending_source(&mut self) {
        if !self.focused
            && self.composition.is_none()
            && !self.waiting()
            && let Some((base, text)) = self.pending_source.take()
            && base >= self.revision
        {
            self.replace_source(&text);
        }
    }

    pub fn compose(&mut self, range: Option<core::ops::Range<u32>>) {
        let ended = self.composition.is_some() && range.is_none();
        self.composition = range;
        self.publish(false, ended);
        self.apply_pending_source();
    }

    /// ACP edits must preserve their declared offsets. Reject malformed text/ranges before
    /// mutation instead of clamping them into a different successful operation.
    pub fn replace(&mut self, start: u32, end: u32, text: &[u16]) -> bool {
        if start > end
            || end as usize > self.text.len()
            || !boundary(&self.text, start as usize)
            || !boundary(&self.text, end as usize)
            || char::decode_utf16(text.iter().copied()).any(|c| c.is_err())
            || text
                .iter()
                .any(|c| matches!(c, 0x0a | 0x0d | 0x0085 | 0x2028 | 0x2029))
        {
            return false;
        }
        if self.text[start as usize..end as usize] == *text {
            return true;
        }
        self.text
            .splice(start as usize..end as usize, text.iter().copied());
        self.set_caret(start + text.len() as u32, Affinity::Upstream, false);
        self.revision += 1;
        self.pending_source = None;
        self.publish(true, true);
        true
    }

    pub fn publish(&mut self, text_changed: bool, may_commit: bool) {
        if text_changed {
            self.published = Arc::from(self.text.as_slice());
        }
        // Non-composition mutations already proved a change in replace(). Only the end
        // of a composition compares its final value with the shared starting snapshot.
        let commit = if may_commit
            && self.composition.is_none()
            && (text_changed || self.committed != self.published)
        {
            self.committed = self.published.clone();
            Some(Arc::<str>::from(String::from_utf16_lossy(&self.text)))
        } else {
            None
        };
        self.updates.push(Update {
            id: self.id,
            revision: self.revision,
            text: text_changed.then(|| self.published.clone()),
            selection: self.selection,
            composition: self.composition.clone(),
            focused: self.focused,
            commit,
        });
    }

    pub fn layout(&mut self, geometry: Arc<Geometry>) {
        if geometry.revision != self.revision
            || self.geometry.as_ref().is_some_and(|old| {
                old.revision == geometry.revision && old.layout_revision > geometry.layout_revision
            })
        {
            return;
        }
        self.geometry = Some(geometry);
        self.drain();
    }

    pub fn command(&mut self, command: Command) {
        self.pending.push_back(command);
        self.drain();
    }

    fn drain(&mut self) {
        while let Some(command) = self.pending.front() {
            let needs_geometry = matches!(
                command,
                Command::Left { .. }
                    | Command::Right { .. }
                    | Command::Point { .. }
                    | Command::Backspace
                    | Command::Delete
            );
            if needs_geometry
                && !self
                    .geometry
                    .as_ref()
                    .is_some_and(|g| g.revision == self.revision)
            {
                break;
            }
            let command = self.pending.pop_front().expect("front exists");
            self.apply(command);
        }
        self.apply_pending_source();
    }

    fn apply(&mut self, command: Command) {
        let before = self.selection;
        let range = before.range();
        match command {
            Command::Character(units, len) => {
                self.replace(range.start, range.end, &units[..usize::from(len)]);
            }
            Command::Insert(mut text) => {
                text.retain(|c| !matches!(c, 0x0a | 0x0d | 0x0085 | 0x2028 | 0x2029));
                self.replace(range.start, range.end, &text);
            }
            Command::Backspace | Command::Delete => {
                let g = self.geometry.as_ref().expect("geometry checked");
                let (start, end) = if !range.is_empty() {
                    (range.start, range.end)
                } else if matches!(command, Command::Backspace) {
                    (g.previous(before.caret), before.caret)
                } else {
                    (before.caret, g.next(before.caret))
                };
                self.replace(start, end, &[]);
            }
            Command::Left { word, select } | Command::Right { word, select } => {
                let right = matches!(command, Command::Right { .. });
                let g = self.geometry.as_ref().expect("geometry checked");
                let mut at = if !select && !range.is_empty() {
                    if right { range.end } else { range.start }
                } else if right {
                    g.next(before.caret)
                } else {
                    g.previous(before.caret)
                };
                if word {
                    let n = self.text.len() as u32;
                    at = before.caret;
                    if right && at < n {
                        let class = class(self.text[at as usize]);
                        while at < n && class == self::class(self.text[at as usize]) {
                            at = g.next(at);
                        }
                        while at < n && self::class(self.text[at as usize]) == 0 {
                            at = g.next(at);
                        }
                    } else if !right {
                        while at > 0 && class(self.text[(at - 1) as usize]) == 0 {
                            at = g.previous(at);
                        }
                        if at > 0 {
                            let class = class(self.text[(at - 1) as usize]);
                            while at > 0 && class == self::class(self.text[(at - 1) as usize]) {
                                at = g.previous(at);
                            }
                        }
                    }
                }
                self.set_caret(
                    at,
                    if right {
                        Affinity::Upstream
                    } else {
                        Affinity::Downstream
                    },
                    select,
                );
            }
            Command::Home { select } => self.set_caret(0, Affinity::Downstream, select),
            Command::End { select } => {
                self.set_caret(self.text.len() as u32, Affinity::Upstream, select);
            }
            Command::All => {
                self.selection.anchor = 0;
                self.set_caret(self.text.len() as u32, Affinity::Upstream, true);
            }
            Command::Point { x, select } => {
                let g = self.geometry.as_ref().expect("geometry checked");
                let (at, affinity) = g.hit(x - g.origin.x);
                self.set_caret(at, affinity, select);
            }
            Command::Select(s) => {
                if s.anchor as usize > self.text.len()
                    || s.caret as usize > self.text.len()
                    || !boundary(&self.text, s.anchor as usize)
                    || !boundary(&self.text, s.caret as usize)
                {
                    return;
                }
                self.set_caret(s.caret, s.affinity, true);
                self.selection.anchor = s.anchor;
            }
        }
        if self.selection != before {
            self.publish(false, false);
        }
    }
}

fn boundary(text: &[u16], at: usize) -> bool {
    at == 0
        || at == text.len()
        || !(0xd800..0xdc00).contains(&text[at - 1])
        || !(0xdc00..0xe000).contains(&text[at])
}

fn class(unit: u16) -> u8 {
    match char::from_u32(unit as u32) {
        Some(c) if c.is_whitespace() => 0,
        Some(c) if c.is_alphanumeric() || c == '_' => 1,
        None => 1,
        _ => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn editor(s: &str) -> Editor {
        Editor::new(
            ControlId::default(),
            InputScope::Default,
            &s.encode_utf16().collect::<Vec<_>>(),
        )
    }
    #[test]
    fn composition_is_a_transaction_even_with_empty_range() {
        let mut e = editor("");
        e.compose(Some(0..0));
        assert!(e.replace(0, 0, &[0x3042]));
        assert!(e.updates.iter().all(|u| u.commit.is_none()));
        e.compose(None);
        e.focus(false);
        assert_eq!(e.updates.iter().filter(|u| u.commit.is_some()).count(), 1);
    }
    #[test]
    fn typing_supersedes_deferred_source_without_resetting_caret() {
        let mut e = editor("a");
        e.focus(true);
        e.source(0, Arc::from([98u16]));
        assert_eq!(e.text, [97]);
        e.replace(1, 1, &[99]);
        e.focus(false);
        assert_eq!(e.text, [97, 99]);
        e.source(0, Arc::from([100u16]));
        assert_eq!(e.text, [97, 99]);
    }
    #[test]
    fn invalid_acp_does_not_change_the_document() {
        let mut e = editor("a👍b");
        for (a, b) in [(3, 1), (2, 3), (0, 9)] {
            assert!(!e.replace(a, b, &[]));
        }
        assert!(!e.replace(0, 0, &[0xd800]));
        assert_eq!(String::from_utf16(&e.text).unwrap(), "a👍b");
    }
    #[test]
    fn equal_echo_is_a_noop_and_blur_does_not_recommit() {
        let mut e = editor("a");
        e.focus(true);
        e.replace(1, 1, &[98]);
        let n = e.updates.len();
        let selection = e.selection();
        e.source(1, Arc::from([97u16, 98]));
        assert_eq!(e.updates.len(), n);
        assert_eq!(e.selection(), selection);
        e.focus(false);
        assert_eq!(e.updates.iter().filter(|u| u.commit.is_some()).count(), 1);
    }
}

#[cfg(test)]
mod delayed_tests {
    use super::*;
    fn setup() -> Editor {
        Editor::new(ControlId::default(), InputScope::Default, &[])
    }
    fn geometry(revision: u64, text: &str) -> Arc<Geometry> {
        let mut at = 0u32;
        let clusters = text
            .chars()
            .map(|ch| {
                let start = at;
                at += ch.len_utf16() as u32;
                super::super::Cluster {
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
            .collect::<Vec<_>>();
        Arc::new(Geometry {
            revision,
            clusters: clusters.into(),
            ..Default::default()
        })
    }
    #[test]
    fn delayed_geometry_preserves_rapid_edit_navigation_order() {
        let mut e = setup();
        e.command(Command::Insert(vec![97]));
        e.command(Command::Left {
            word: false,
            select: false,
        });
        e.command(Command::Insert(vec![98]));
        assert_eq!(e.text, [97]);
        e.layout(geometry(0, ""));
        assert_eq!(e.text, [97]);
        e.layout(geometry(1, "a"));
        assert_eq!(e.text, [98, 97]);
        let commits: Vec<_> = e
            .updates
            .iter()
            .filter_map(|u| u.commit.as_deref())
            .collect();
        assert_eq!(commits, ["a", "ba"]);
    }
    #[test]
    fn equal_latest_echo_cancels_an_older_replacement() {
        let mut e = setup();
        e.focus(true);
        e.source(0, Arc::from([97]));
        e.source(0, Arc::from([]));
        e.focus(false);
        assert!(e.text.is_empty());
    }
    #[test]
    fn composition_cancellation_and_selection_never_commit() {
        let mut e = setup();
        e.compose(Some(0..0));
        e.replace(0, 0, &[97]);
        e.replace(0, 1, &[]);
        e.compose(None);
        e.command(Command::All);
        e.focus(true);
        e.focus(false);
        assert!(e.updates.iter().all(|u| u.commit.is_none()));
    }
    #[test]
    fn paste_is_single_line_and_one_commit() {
        let mut e = setup();
        e.command(Command::Insert("a\r\nb\nc".encode_utf16().collect()));
        assert_eq!(e.text, [97, 98, 99]);
        assert_eq!(e.updates.iter().filter(|u| u.commit.is_some()).count(), 1);
    }
    #[test]
    fn blur_waits_for_a_preceding_key_before_applying_a_model_replacement() {
        let mut e = setup();
        e.focus(true);
        e.command(Command::Insert(vec![97]));
        e.command(Command::Left {
            word: false,
            select: false,
        });
        e.command(Command::Insert(vec![98]));
        e.source(1, Arc::from([120]));
        e.focus(false);
        assert_eq!(e.text, [97]);
        e.layout(geometry(1, "a"));
        assert_eq!(e.text, [98, 97]);
        assert!(!e.waiting());
    }
    #[test]
    fn empty_composition_defers_source_until_end_after_blur() {
        let mut e = setup();
        e.focus(true);
        e.compose(Some(0..0));
        e.source(0, Arc::from([120]));
        e.focus(false);
        assert!(e.text.is_empty());
        e.compose(None);
        assert_eq!(e.text, [120]);
        assert!(e.updates.iter().all(|u| u.commit.is_none()));
    }
    #[test]
    fn unicode_line_separators_are_removed_on_paste() {
        let mut e = setup();
        e.command(Command::Insert(
            "a\u{0085}b\u{2028}c\u{2029}d".encode_utf16().collect(),
        ));
        assert_eq!(e.text, [97, 98, 99, 100]);
    }
}
