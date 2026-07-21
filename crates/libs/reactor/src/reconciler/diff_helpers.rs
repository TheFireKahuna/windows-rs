use super::*;

impl<B: Backend> Reconciler<B> {
    pub fn diff_opt_f64(&mut self, id: ControlId, prop: Prop, old: Option<f64>, new: Option<f64>) {
        if old == new {
            return;
        }
        match new {
            Some(v) => self.backend.set_prop(id, prop, &PropValue::F64(v)),
            None => self.backend.set_prop(id, prop, &PropValue::Unset),
        }
    }

    pub fn diff_opt_copy<T: Copy + PartialEq>(
        &mut self,
        id: ControlId,
        prop: Prop,
        old: Option<T>,
        new: Option<T>,
        wrap: fn(T) -> PropValue,
    ) {
        if old == new {
            return;
        }
        match new {
            Some(v) => self.backend.set_prop(id, prop, &wrap(v)),
            None => self.backend.set_prop(id, prop, &PropValue::Unset),
        }
    }

    pub fn diff_opt_clone<T: Clone + PartialEq>(
        &mut self,
        id: ControlId,
        prop: Prop,
        old: &Option<T>,
        new: &Option<T>,
        wrap: fn(T) -> PropValue,
    ) {
        if old == new {
            return;
        }
        match new {
            Some(v) => self.backend.set_prop(id, prop, &wrap(v.clone())),
            None => self.backend.set_prop(id, prop, &PropValue::Unset),
        }
    }

    pub fn apply_props(&mut self, id: ControlId, bindings: &[Binding]) {
        for b in bindings {
            match b {
                Binding::Prop(p, v) => self.backend.set_prop(id, *p, v),
                Binding::Event(e, Some(h)) => self.backend.attach_event(id, *e, h.clone()),
                Binding::Event(_, None) => {}
            }
        }
    }

    pub fn diff_props(&mut self, id: ControlId, kind: ControlKind, old: &[Binding], new: &[Binding]) {
        for b in new {
            match b {
                // An editor's declared text is exempt from equality-dropping:
                // the backend buffer is front-authoritative (the user types
                // into it), so "old prop == new prop" does not mean the
                // backend already holds this text — and dropping the write
                // would make "revert to the same prop value" (an app
                // declining a delivered edit) inexpressible. The write is
                // re-emitted every render; the dcomp recorder dedupes it
                // against its delivered-text view, so the steady state stays
                // silent (§7.2, text half).
                Binding::Prop(p, v) if editor_text_prop(kind, *p) && matches!(v, PropValue::Str(_)) => {
                    self.backend.set_prop(id, *p, v);
                }
                Binding::Prop(p, v) => match find_prop(old, *p) {
                    Some(ov) if ov == v => {}
                    _ => self.backend.set_prop(id, *p, v),
                },
                Binding::Event(e, new_h) => {
                    let old_inner: Option<&_> = find_event(old, *e).and_then(|o| o.as_ref());
                    if old_inner == new_h.as_ref() {
                        continue;
                    }
                    match new_h {
                        Some(h) => self.backend.attach_event(id, *e, h.clone()),
                        None => self.backend.detach_event(id, *e),
                    }
                }
            }
        }
        for b in old {
            match b {
                Binding::Prop(p, _) => {
                    if find_prop(new, *p).is_none() {
                        self.backend.set_prop(id, *p, &PropValue::Unset);
                    }
                }
                Binding::Event(e, old_h) => {
                    if find_event(new, *e).is_none() && old_h.is_some() {
                        self.backend.detach_event(id, *e);
                    }
                }
            }
        }
    }
}

/// Which prop carries a text editor's programmatic buffer text, per widget
/// contract: `TextBox` / `PasswordBox` declare it as `Prop::Value`, an
/// `AutoSuggestBox` as `Prop::Text`. (A `NumberBox` seeds its buffer from the
/// numeric `Prop::Value`, which has its own revision gate.)
fn editor_text_prop(kind: ControlKind, prop: Prop) -> bool {
    match kind {
        ControlKind::TextBox | ControlKind::PasswordBox => prop == Prop::Value,
        ControlKind::AutoSuggestBox => prop == Prop::Text,
        _ => false,
    }
}
