//! What the window thread calls. Input, automation and text services share one document.

use super::doc::{Change, Command, Doc, Motion, View};
use super::session::Session;
use super::touch::Touch;
use super::{InputScope, Layout, Source, Update, system};
use crate::input::{KeyEvent, KeyKind, Report, client_origin};
use crate::uia::action::TextAction;
use core::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use windows_core::Result;
use crate::layout::Rect;
use windows_scene::{ContactKind, ControlId, HitEntry, HitFlags, HitTable, NO_ENTRY};
use windows_window::{Hwnd, Window};

pub(crate) struct TextInput {
    pub doc: Rc<RefCell<Doc>>,
    pub tsf: Rc<Session>,
    pub touch: Touch,
    /// A lone high surrogate from a translated character message, held for its pair.
    high: Option<u16>,
    hwnd: Hwnd,
}

impl TextInput {
    pub fn new(window: &Window) -> Result<Self> {
        system::refresh();
        let doc = Rc::new(RefCell::new(Doc::default()));
        let hwnd = window.handle();
        Ok(Self {
            tsf: Rc::new(Session::new(Rc::clone(&doc), hwnd)?),
            touch: Touch::new(hwnd),
            doc,
            high: None,
            hwnd,
        })
    }

    pub fn focused(&self) -> Option<ControlId> {
        self.doc.borrow().focused()
    }

    /// Republishes the focused field after a system settings change, so the caret redraws
    /// at the new width and blink.
    pub fn settings_changed(&mut self) {
        system::refresh();
        self.doc.borrow_mut().emit(Some(Change::Caret));
    }

    pub fn source(&mut self, source: &Source) {
        self.doc.borrow_mut().source(source);
        if self.focused() == Some(source.id) {
            self.tsf.inner.changed();
        }
    }

    pub fn layout(&mut self, layout: &Layout) {
        self.doc.borrow_mut().layout(layout.id, &layout.geometry);
    }

    /// Releases a field's storage on unmount, dropping focus with it.
    pub fn forget(&mut self, id: ControlId) -> Result<()> {
        if self.focused() == Some(id) {
            self.focus(None)?;
        }
        self.doc.borrow_mut().forget(id);
        Ok(())
    }

    pub fn focus(&mut self, id: Option<ControlId>) -> Result<()> {
        let id = id.filter(|id| self.doc.borrow().holds(*id));
        if self.focused() == id {
            return Ok(());
        }
        // SetFocus may finish the OLD composition synchronously, so it runs before the
        // document moves and with no borrow held across it.
        self.tsf.focus(false)?;
        self.doc.borrow_mut().focus(id);
        self.high = None;
        self.tsf.inner.changed();
        if id.is_some() {
            self.tsf.focus(true)?;
        }
        Ok(())
    }

    /// Runs one automation write, checked against the revision it was issued at.
    pub fn automation(&mut self, action: TextAction) {
        let mut doc = self.doc.borrow_mut();
        let (id, revision) = match &action {
            TextAction::Replace(id, revision, _) | TextAction::Select(id, revision, _) => {
                (*id, *revision)
            }
        };
        if doc.focused() != Some(id) || doc.revision != revision || doc.composing() {
            return;
        }
        match action {
            TextAction::Replace(_, _, text) => {
                doc.command(Command::All);
                doc.command(Command::Write(text.into()));
            }
            // A password field refuses selection, because the client that placed the range
            // reads it back and names masked text with it. A write returns nothing.
            TextAction::Select(..) if doc.scope() == InputScope::Password => (),
            TextAction::Select(_, _, selection) => doc.command(Command::Select(selection)),
        }
    }

    /// Consumes the reports the focused field owns and leaves every other one in place.
    pub fn reports(
        &mut self,
        reports: &mut Vec<Report>,
        hits: &HitTable,
        scale: f32,
    ) -> Result<()> {
        // Preserve non-text reports. A text-owned key must not subsequently activate an
        // overlay or reach a second editor path on the application thread.
        let mut kept = 0;
        for i in 0..reports.len() {
            let report = reports[i];
            let consumed = match report {
                Report::FocusChanged { to, .. } => {
                    self.focus(to)?;
                    self.geometry(hits, scale);
                    false
                }
                Report::Pressed { target, sample, .. } | Report::Moved { target, sample, .. }
                    if Some(target) == self.focused() =>
                {
                    let extend = matches!(report, Report::Moved { .. });
                    if !extend && sample.kind() == ContactKind::Touch {
                        self.touch.show();
                    }
                    // The published view is in screen pixels, so the field's own left edge
                    // comes from the array the contact resolved against.
                    let left = hits
                        .entry(target)
                        .map_or(0.0, |entry| box_of(hits, entry, false).x0);
                    self.doc.borrow_mut().command(Command::Point {
                        x: sample.raw.x - left,
                        extend,
                    });
                    false
                }
                Report::Key { target, event } if target.is_some() && target == self.focused() => {
                    self.key(event)?
                }
                _ => false,
            };
            if !consumed {
                reports[kept] = report;
                kept += 1;
            }
        }
        reports.truncate(kept);
        Ok(())
    }

    /// Publishes the focused field's box and clip in screen pixels, which is what the store
    /// reports positions from.
    pub fn geometry(&mut self, hits: &HitTable, scale: f32) {
        let origin = client_origin(self.hwnd.raw()).unwrap_or_default();
        let screen = |r: Rect| Rect {
            x0: origin.x + r.x0 * scale,
            y0: origin.y + r.y0 * scale,
            x1: origin.x + r.x1 * scale,
            y1: origin.y + r.y1 * scale,
        };
        let view = self
            .focused()
            .and_then(|id| hits.entry(id))
            .map(|entry| View {
                rect: screen(box_of(hits, entry, false)),
                clip: screen(box_of(hits, entry, true)),
                scale,
            })
            .unwrap_or_default();
        if core::mem::replace(&mut self.doc.borrow_mut().view, view) != view {
            self.tsf.inner.dirty_layout();
        }
    }

    pub fn flush(&mut self, out: &mut Vec<Update>) {
        self.tsf.inner.notify();
        self.doc.borrow_mut().drain(out);
    }

    fn key(&mut self, event: KeyEvent) -> Result<bool> {
        if event.kind == KeyKind::Up {
            return Ok(false);
        }
        if event.kind == KeyKind::Char {
            return Ok(self.character(event.key));
        }
        let (select, ctrl) = (event.mods.shift, event.mods.ctrl);
        let command = match event.key {
            0x08 => Command::Erase {
                forward: false,
                word: ctrl,
            },
            0x2e => Command::Erase {
                forward: true,
                word: ctrl,
            },
            0x41 if ctrl => Command::All,
            0x43 | 0x58 if ctrl => return self.copy(event.key == 0x58),
            0x56 if ctrl => return self.paste(),
            key => match motion(key, ctrl, select) {
                Some(command) => command,
                // Enter carries no text, and an unclaimed Escape or Tab belongs to the
                // focus scope. Every other key reaches the application unchanged.
                None => return Ok(key == 0x0d),
            },
        };
        self.doc.borrow_mut().command(command);
        Ok(true)
    }

    /// Takes one translated character, joining a surrogate pair across its two messages.
    fn character(&mut self, unit: u16) -> bool {
        if unit < 0x20 || unit == 0x7f {
            return unit != 0x1b && unit != 0x09;
        }
        if let Some(units) = joined(&mut self.high, unit) {
            self.doc.borrow_mut().command(Command::Write(units));
        }
        true
    }

    fn copy(&mut self, cut: bool) -> Result<bool> {
        let taken = {
            let doc = self.doc.borrow();
            let range = doc.range();
            (doc.scope() != InputScope::Password && !range.is_empty()).then(|| {
                (
                    doc.revision,
                    doc.text()[range.start as usize..range.end as usize].to_vec(),
                )
            })
        };
        let Some((revision, text)) = taken else {
            return Ok(true);
        };
        // Clipboard ownership can call an external owner. No document borrow crosses that
        // call; a reentrant edit invalidates the subsequent cut.
        system::write(self.hwnd, &text)?;
        let mut doc = self.doc.borrow_mut();
        if cut && doc.revision == revision {
            doc.command(Command::Erase {
                forward: true,
                word: false,
            });
        }
        Ok(true)
    }

    fn paste(&mut self) -> Result<bool> {
        if let Ok(text) = system::read(self.hwnd) {
            self.doc.borrow_mut().command(Command::Write(text.into()));
        }
        Ok(true)
    }
}

/// Returns the code units one translated character contributes, holding a lone high
/// surrogate until the message carrying its low one arrives.
///
/// A low surrogate without a held high one is dropped: half a supplementary character is
/// not text the buffer can hold.
fn joined(high: &mut Option<u16>, unit: u16) -> Option<Arc<[u16]>> {
    match unit {
        lead @ 0xd800..0xdc00 => {
            *high = Some(lead);
            None
        }
        trail @ 0xdc00..0xe000 => high.take().map(|lead| Arc::from([lead, trail])),
        unit => {
            *high = None;
            Some(Arc::from([unit]))
        }
    }
}

/// Virtual key to the motion it names, with the word-wise row Ctrl selects.
const MOTIONS: [(u16, Motion, Motion); 4] = [
    (0x25, Motion::Left, Motion::WordLeft),
    (0x27, Motion::Right, Motion::WordRight),
    (0x24, Motion::Home, Motion::Home),
    (0x23, Motion::End, Motion::End),
];

fn motion(key: u16, ctrl: bool, select: bool) -> Option<Command> {
    let &(_, plain, word) = MOTIONS.iter().find(|(vk, ..)| *vk == key)?;
    Some(Command::Move {
        to: if ctrl { word } else { plain },
        select,
    })
}

/// Returns `entry`'s box in client DIPs, moved by the live scroll shadows of its scrolling
/// ancestors and, when `clipped`, cut by every clipping ancestor above it.
fn box_of(hits: &HitTable, entry: &HitEntry, clipped: bool) -> Rect {
    let scrolled = |e: &HitEntry| {
        let o = if e.flags.contains(HitFlags::UNSCROLLED) {
            windows_scene::Point::zero()
        } else {
            hits.offset(e.scroll_src)
        };
        Rect {
            x0: e.x0 - o.x,
            y0: e.y0 - o.y,
            x1: e.x1 - o.x,
            y1: e.y1 - o.y,
        }
    };
    let mut box_ = scrolled(entry);
    let mut parent = if clipped { entry.clip_parent } else { NO_ENTRY };
    while let Some(ancestor) = hits.entries().get(parent as usize) {
        let clip = scrolled(ancestor);
        box_ = Rect {
            x0: box_.x0.max(clip.x0),
            y0: box_.y0.max(clip.y0),
            x1: box_.x1.min(clip.x1),
            y1: box_.y1.min(clip.y1),
        };
        parent = ancestor.clip_parent;
    }
    box_
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_surrogate_pair_joins_across_its_two_character_messages() {
        let mut high = None;
        assert!(joined(&mut high, 0xd83d).is_none(), "the lead unit waits");
        assert_eq!(*joined(&mut high, 0xde00).unwrap(), [0xd83d, 0xde00]);
        assert_eq!(high, None);
    }

    #[test]
    fn an_unpaired_low_surrogate_writes_nothing() {
        let mut high = None;
        assert!(joined(&mut high, 0xde00).is_none());
    }

    #[test]
    fn an_ordinary_character_drops_a_stale_lead_unit() {
        let mut high = Some(0xd83d);
        assert_eq!(*joined(&mut high, 97).unwrap(), [97]);
        assert_eq!(high, None);
    }

    #[test]
    fn motion_keys_map_to_word_motion_only_under_control() {
        assert!(matches!(
            motion(0x25, false, true),
            Some(Command::Move {
                to: Motion::Left,
                select: true
            })
        ));
        assert!(matches!(
            motion(0x27, true, false),
            Some(Command::Move {
                to: Motion::WordRight,
                select: false
            })
        ));
        assert!(motion(0x41, true, false).is_none());
    }
}
