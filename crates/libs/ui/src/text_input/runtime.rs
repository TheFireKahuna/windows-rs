//! Window-thread registry. Input, automation and text services share these documents.

use super::{Command, Editor, InputScope, Layout, Source, Update};
use crate::input::{KeyKind, Report};
use std::{cell::RefCell, rc::Rc};
use windows_scene::{Control, ControlId, HitTable, Slots};
use windows_window::{Hwnd, Window};

pub(crate) struct Documents {
    pub editors: Slots<Control, Editor>,
    pub active: Option<ControlId>,
    pub rect: windows_scene::Rect,
    pub scale: f32,
    pub clip: windows_scene::Rect,
    pub origin: Option<windows_numerics::Vector2>,
}

impl Default for Documents {
    fn default() -> Self {
        Self {
            editors: Slots::new(),
            active: None,
            rect: Default::default(),
            scale: 1.0,
            clip: Default::default(),
            origin: None,
        }
    }
}

impl Documents {
    pub fn active(&self) -> Option<&Editor> {
        self.active.and_then(|id| self.editors.get(id))
    }
    pub fn active_mut(&mut self) -> Option<&mut Editor> {
        self.active.and_then(|id| self.editors.get_mut(id))
    }
}

pub(crate) struct TextInput {
    pub docs: Rc<RefCell<Documents>>,
    pub tsf: Rc<super::tsf::Session>,
    pub touch: super::touch::Touch,
    high_surrogate: Option<u16>,
    changed: Vec<ControlId>,
    hwnd: Hwnd,
}

impl TextInput {
    pub fn automation(&mut self, action: crate::uia::action::TextAction) {
        use crate::uia::action::TextAction;
        let id = match &action {
            TextAction::Replace(id, ..) | TextAction::Select(id, ..) => *id,
        };
        self.changed(id);
        let mut docs = self.docs.borrow_mut();
        match action {
            TextAction::Replace(id, base, text) => {
                if let Some(e) = docs.editors.get_mut(id) {
                    if base != e.revision || e.composition.is_some() {
                        return;
                    }
                    e.command(Command::All);
                    e.command(Command::Insert(text));
                }
            }
            TextAction::Select(id, base, selection) => {
                if let Some(e) = docs.editors.get_mut(id)
                    && base == e.revision
                    && e.scope != InputScope::Password
                {
                    e.command(Command::Select(selection));
                }
            }
        }
    }

    pub fn new(window: &Window) -> windows_core::Result<Self> {
        super::settings::refresh();
        let docs = Rc::new(RefCell::new(Documents::default()));
        let tsf = super::tsf::Session::new(Rc::clone(&docs), window.handle())?;
        Ok(Self {
            docs,
            tsf: Rc::new(tsf),
            touch: super::touch::Touch::new(window.handle()),
            high_surrogate: None,
            changed: Vec::new(),
            hwnd: window.handle(),
        })
    }

    pub fn source(&mut self, source: &Source) {
        self.changed(source.id);
        let mut docs = self.docs.borrow_mut();
        if let Some(editor) = docs.editors.get_mut(source.id) {
            editor.scope = source.scope;
            editor.source(source.based_on, source.text.clone());
        } else {
            let mut editor = Editor::new(source.id, source.scope, &source.text);
            editor.publish(true, false);
            docs.editors.place(source.id, editor);
        }
    }

    pub fn layout(&mut self, layout: &Layout) {
        self.changed(layout.id);
        if let Some(editor) = self.docs.borrow_mut().editors.get_mut(layout.id) {
            editor.layout(layout.geometry.clone());
        }
        if self.docs.borrow().active == Some(layout.id) {
            self.tsf.layout_changed();
        }
    }

    pub fn forget(&mut self, id: ControlId) -> windows_core::Result<()> {
        if self.docs.borrow().active == Some(id) {
            self.focus(None)?;
        }
        self.docs.borrow_mut().editors.take(id);
        Ok(())
    }

    pub fn focus(&mut self, id: Option<ControlId>) -> windows_core::Result<()> {
        let id = id.filter(|id| self.docs.borrow().editors.get(*id).is_some());
        if self.docs.borrow().active == id {
            return Ok(());
        }
        let old = self.docs.borrow().active;
        if let Some(old) = old {
            self.changed(old);
        }
        if let Some(id) = id {
            self.changed(id);
        }
        // SetFocus may finish the OLD composition synchronously. Keep that document active
        // until the call returns, and hold no editor borrow across it.
        self.tsf.focus(false)?;
        {
            let mut docs = self.docs.borrow_mut();
            if let Some(old) = docs.active_mut() {
                old.focus(false);
            }
            docs.active = id;
            if let Some(new) = docs.active_mut() {
                new.focus(true);
            }
        }
        self.high_surrogate = None;
        self.tsf.document_changed();
        if id.is_some() {
            self.tsf.focus(true)?;
        }
        Ok(())
    }

    pub fn reports(
        &mut self,
        reports: &mut Vec<Report>,
        hits: &HitTable,
        offsets: &dyn windows_scene::ScrollOffsets,
        scale: f32,
    ) -> windows_core::Result<()> {
        // Preserve non-text reports. A text-owned key must not subsequently activate an
        // overlay or reach a second editor path on the application thread.
        let mut out = 0;
        for i in 0..reports.len() {
            let report = reports[i];
            let mut consumed = false;
            match report {
                Report::FocusChanged { to, .. } => {
                    self.focus(to)?;
                    self.geometry(hits, offsets, scale);
                }
                Report::Pressed { target, sample, .. } | Report::Moved { target, sample, .. }
                    if Some(target) == self.docs.borrow().active =>
                {
                    let select = matches!(report, Report::Moved { .. });
                    if !select && sample.kind() == windows_scene::ContactKind::Touch {
                        self.touch.show();
                    }
                    let mut docs = self.docs.borrow_mut();
                    let x = sample.raw.x - docs.rect.x0;
                    if let Some(editor) = docs.active_mut() {
                        editor.command(Command::Point { x, select });
                    }
                }
                Report::Key { target, event }
                    if target == self.docs.borrow().active && target.is_some() =>
                {
                    consumed = self.key(event);
                }
                _ => {}
            }
            if !consumed {
                reports[out] = report;
                out += 1;
            }
        }
        reports.truncate(out);
        Ok(())
    }

    pub fn geometry(
        &mut self,
        hits: &HitTable,
        shadows: &dyn windows_scene::ScrollOffsets,
        scale: f32,
    ) {
        let origin = crate::input::Coords::new(self.hwnd.raw()).origin();
        let changed = {
            let mut docs = self.docs.borrow_mut();
            let before = (docs.rect, docs.clip, docs.scale, docs.origin);
            docs.scale = scale;
            docs.origin = origin;
            if let Some(id) = docs.active
                && let Some(entry) = hits.entry(id)
            {
                let offset = shadows.offset(entry.scroll_src);
                docs.rect = windows_scene::Rect {
                    x0: entry.x0 - offset.x,
                    y0: entry.y0 - offset.y,
                    x1: entry.x1 - offset.x,
                    y1: entry.y1 - offset.y,
                };
                docs.clip = docs.rect;
                let mut parent = entry.clip_parent;
                for _ in 0..hits.entries().len() {
                    if parent == windows_scene::NO_ENTRY {
                        break;
                    }
                    let Some(entry) = hits.entries().get(parent as usize) else {
                        break;
                    };
                    let o = shadows.offset(entry.scroll_src);
                    docs.clip.x0 = docs.clip.x0.max(entry.x0 - o.x);
                    docs.clip.y0 = docs.clip.y0.max(entry.y0 - o.y);
                    docs.clip.x1 = docs.clip.x1.min(entry.x1 - o.x);
                    docs.clip.y1 = docs.clip.y1.min(entry.y1 - o.y);
                    parent = entry.clip_parent;
                }
            }
            before != (docs.rect, docs.clip, docs.scale, docs.origin)
        };
        if changed {
            self.tsf.layout_changed();
        }
    }

    fn changed(&mut self, id: ControlId) {
        if !self.changed.contains(&id) {
            self.changed.push(id);
        }
    }

    pub fn flush(&mut self, out: &mut Vec<Update>) {
        self.tsf.flush();
        let active = self.docs.borrow().active;
        if let Some(id) = active {
            self.changed(id);
        }
        let mut docs = self.docs.borrow_mut();
        for id in self.changed.drain(..) {
            if let Some(editor) = docs.editors.get_mut(id) {
                out.append(&mut editor.updates);
            }
        }
    }

    fn key(&mut self, event: crate::input::KeyEvent) -> bool {
        if event.kind == KeyKind::Up {
            return true;
        }
        if event.kind == KeyKind::Char {
            let u = event.key;
            if u < 0x20 || u == 0x7f {
                return true;
            }
            if (0xd800..0xdc00).contains(&u) {
                self.high_surrogate = Some(u);
                return true;
            }
            let (units, len) = if (0xdc00..0xe000).contains(&u) {
                let Some(high) = self.high_surrogate.take() else {
                    return true;
                };
                ([high, u], 2)
            } else {
                self.high_surrogate = None;
                ([u, 0], 1)
            };
            if let Some(e) = self.docs.borrow_mut().active_mut() {
                e.command(Command::Character(units, len));
            }
            return true;
        }
        let select = event.mods.shift;
        let word = event.mods.ctrl;
        let command = match event.key {
            0x08 => Some(Command::Backspace),
            0x2e => Some(Command::Delete),
            0x25 => Some(Command::Left { word, select }),
            0x27 => Some(Command::Right { word, select }),
            0x24 => Some(Command::Home { select }),
            0x23 => Some(Command::End { select }),
            0x41 if word => Some(Command::All),
            0x43 | 0x58 if word => {
                let copy = {
                    let docs = self.docs.borrow();
                    docs.active()
                        .filter(|e| e.scope != InputScope::Password)
                        .and_then(|e| {
                            let range = e.selection().range();
                            (!range.is_empty()).then(|| {
                                (
                                    e.id,
                                    e.revision,
                                    range.clone(),
                                    e.text[range.start as usize..range.end as usize].to_vec(),
                                )
                            })
                        })
                };
                if let Some((id, revision, range, text)) = copy {
                    // Clipboard ownership can call an external owner. No document borrow
                    // crosses that call; a reentrant edit invalidates the subsequent cut.
                    if super::clipboard::write(self.hwnd, &text).is_ok() && event.key == 0x58 {
                        let mut docs = self.docs.borrow_mut();
                        if let Some(e) = docs.editors.get_mut(id)
                            && e.revision == revision
                        {
                            e.replace(range.start, range.end, &[]);
                        }
                    }
                }
                return true;
            }
            0x56 if word => {
                if let Ok(text) = super::clipboard::read(self.hwnd)
                    && let Some(e) = self.docs.borrow_mut().active_mut()
                {
                    e.command(Command::Insert(text));
                }
                return true;
            }
            0x0d => return true,
            0x1b | 0x09 => return false,
            _ => return true,
        };
        if let Some(command) = command
            && let Some(e) = self.docs.borrow_mut().active_mut()
        {
            e.command(command);
        }
        true
    }
}
