//! Node-owned destinations. Writer slots and deferred destruction reuse host storage.
use super::Host;
use crate::signal::Effect;
use std::rc::Rc;
use windows_scene::{Anim, Bind, ControlId, NodeId, Prop, Tuning, Value};

#[derive(Copy, Clone, PartialEq)]
pub(super) enum Destination {
    Channel(Prop),
    Layout,
    Text,
    Selected,
    Disabled,
    Scalar,
    Hidden,
    Halo,
    Popup,
    Validation,
}

pub(super) struct Binding {
    destination: Option<Destination>,
    effect: Effect,
    next: Option<usize>,
}

#[derive(Default)]
pub(super) struct Bindings {
    rows: Vec<Option<Binding>>,
    free: Vec<usize>,
}

/// One value whose drop belongs outside the host's borrow, with its type erased.
///
/// Erasure rather than a variant per payload: dropping is the only thing ever done with
/// one, and a named arm per kind makes every new handler cost a match arm as well as a
/// setter. The box is the one allocation retirement makes, and a control's whole
/// [`Handlers`] row travels in a single one.
pub(super) struct Retired(#[expect(dead_code, reason = "held to drop")] Box<dyn std::any::Any>);

impl Retired {
    pub(super) fn new(value: impl std::any::Any) -> Self {
        Self(Box::new(value))
    }
}

/// One control's application callbacks, held apart from its hot row.
///
/// Every field owns what it holds, which is what splits this from
/// [`ControlRow`](super::host::ControlRow): that row is `Copy` and releasing a control is
/// dropping this one. A control that declares no callback — a blocker, a scroll rail —
/// places no row here at all.
#[derive(Default)]
pub(crate) struct Handlers {
    pub click: Option<Rc<dyn Fn()>>,
    /// The scalar gesture's handler, where the application declared one.
    pub scalar: Option<Rc<dyn Fn(crate::widget::Gesturing<f64>)>>,
    /// The two-axis drag's handler, where the application declared one.
    pub drag: Option<Rc<dyn Fn(crate::widget::Gesturing<crate::gesture::DragUpdate>)>>,
    /// The hover description and the side it opens on.
    ///
    /// `Rc` rather than `Box` for both this and [`flyout`](Self::flyout): building either
    /// body is application code, so the overlay layer clones it out of the host's borrow
    /// before running it. Both stay in the row, since a picker's flyout opens once per press
    /// and not once per lifetime.
    ///
    /// The side is authored rather than derived: which side clears a control's neighbours
    /// depends on the axis its author stacked them on, so a description below a toolbar
    /// button clears its neighbours and the same one below a rail item lands on the next.
    pub tip: Option<(Rc<crate::widget::TextSource>, crate::overlay::Side)>,
    pub flyout: Option<Rc<dyn Fn(&mut super::Ui<'_>)>>,
    pub name: Option<std::borrow::Cow<'static, str>>,
}

/// Handler rows and the indices free to take one.
///
/// A free list rather than a slotted table keyed by [`ControlId`]: a slotted one grows to
/// the highest live control and holds a full-size vacant row for every control below it
/// that declared nothing, which is the storage this table exists to not spend.
#[derive(Default)]
pub(crate) struct HandlerTable {
    rows: Vec<Handlers>,
    free: Vec<u32>,
}

impl HandlerTable {
    /// Returns how many rows are placed, vacant ones included. What a test asks whether a
    /// control paid handler storage with.
    #[cfg(test)]
    pub(crate) fn placed(&self) -> usize {
        self.rows.len() - self.free.len()
    }
}

impl Host {
    /// Returns `id`'s handler row, placing one the first time it declares a handler.
    ///
    /// # Panics
    ///
    /// Panics where `id` names no live control. Every caller has just resolved the control
    /// it is writing into.
    fn handlers_mut(&mut self, id: ControlId) -> &mut Handlers {
        let placed = self
            .controls
            .get(id)
            .expect("a live control owns its handlers")
            .handlers;
        let at = match placed {
            Some(at) => at,
            None => {
                let at = match self.handlers.free.pop() {
                    Some(at) => at,
                    None => {
                        self.handlers.rows.push(Handlers::default());
                        u32::try_from(self.handlers.rows.len() - 1)
                            .expect("handler storage exhausted")
                    }
                };
                self.controls.get_mut(id).unwrap().handlers = Some(at);
                at
            }
        };
        &mut self.handlers.rows[at as usize]
    }

    /// Returns `id`'s handler row, or `None` where it has none or the id is stale.
    pub(crate) fn handlers(&self, id: ControlId) -> Option<&Handlers> {
        let at = self.controls.get(id)?.handlers?;
        Some(&self.handlers.rows[at as usize])
    }

    /// Installs one handler, retiring whatever it displaces.
    ///
    /// The displaced value is released outside this host's borrow, since dropping it runs
    /// whatever the application captured. Repeating a setter therefore replaces.
    pub(super) fn set_handler<T: 'static>(
        &mut self,
        id: ControlId,
        pick: fn(&mut Handlers) -> &mut Option<T>,
        value: T,
    ) {
        let displaced = pick(self.handlers_mut(id)).replace(value);
        if let Some(displaced) = displaced {
            self.retired.push(Retired::new(displaced));
        }
    }

    /// Vacates a released control's handler row, retiring what it held.
    pub(super) fn release_handlers(&mut self, at: Option<u32>) {
        let Some(at) = at else { return };
        let row = core::mem::take(&mut self.handlers.rows[at as usize]);
        self.handlers.free.push(at);
        self.retired.push(Retired::new(row));
    }
}

impl Host {
    pub(super) fn set_channel(&mut self, node: NodeId, target: NodeId, prop: Prop, value: Value) {
        self.replace_binding(node, Destination::Channel(prop));
        self.mounts.get_mut(node).unwrap().channels |= 1 << prop as u8;
        if prop == Prop::Center
            && let Some(job) = self.geometry_jobs.get_mut(node)
        {
            job.pivot = None;
        }
        self.queue_channel(node, target, prop, Bind::Set(value));
    }

    /// One retained writer per destination; its first value snaps and equal values stop here.
    pub(super) fn bind_channel(
        &mut self,
        node: NodeId,
        target: NodeId,
        prop: Prop,
        motion: crate::widget::Motion,
        read: impl Fn() -> Value + 'static,
    ) {
        self.mounts.get_mut(node).unwrap().channels |= 1 << prop as u8;
        if prop == Prop::Center
            && let Some(job) = self.geometry_jobs.get_mut(node)
        {
            job.pivot = None;
        }
        let mut previous = None;
        self.bind_to(node, Destination::Channel(prop), move || {
            let value = read();
            if previous == Some(value) {
                return;
            }
            let bind = if previous.is_none() || matches!(motion, crate::widget::Motion::Snap) {
                Bind::Set(value)
            } else {
                Bind::Animate(Anim::Spring {
                    to: value,
                    tuning: Tuning::Chrome,
                    delay_ms: 0,
                })
            };
            previous = Some(value);
            Host::with(|host| host.queue_channel(node, target, prop, bind));
        });
    }

    pub(super) fn relative_pivot(&mut self, node: NodeId, pivot: windows_numerics::Vector2) {
        self.replace_binding(node, Destination::Channel(Prop::Center));
        self.mounts.get_mut(node).unwrap().channels |= 1 << Prop::Center as u8;
        if let Some(job) = self.geometry_jobs.get_mut(node) {
            job.pivot = Some(pivot);
        } else {
            self.geometry_jobs.place(
                node,
                super::geometry::Row {
                    local: None,
                    effect: None,
                    pivot: Some(pivot),
                },
            );
        }
    }

    pub(super) fn replace_binding(&mut self, node: NodeId, destination: Destination) {
        let mut link = self.mounts.get(node).and_then(|row| row.bindings);
        while let Some(index) = link {
            let row = self.bindings.rows[index].as_ref().unwrap();
            link = row.next;
            if row.destination == Some(destination) {
                if let Some(callback) = row.effect.retire() {
                    self.retired.push(Retired::new(callback));
                }
                // Reuse this destination's slot when the next writer is installed.
                return;
            }
        }
    }

    pub(super) fn bind_to(
        &mut self,
        node: NodeId,
        destination: Destination,
        update: impl FnMut() + 'static,
    ) {
        self.replace_binding(node, destination);
        let effect = self.binding_effect(node, update);
        self.own_binding(node, Some(destination), effect);
    }

    pub(super) fn own_binding(
        &mut self,
        node: NodeId,
        destination: Option<Destination>,
        effect: Effect,
    ) {
        let head = self
            .mounts
            .get(node)
            .expect("bindings require a live node")
            .bindings;
        let mut link = head;
        while let Some(index) = link {
            let row = self.bindings.rows[index].as_mut().unwrap();
            if destination.is_some() && row.destination == destination {
                row.effect = effect;
                return;
            }
            link = row.next;
        }
        let row = Some(Binding {
            destination,
            effect,
            next: head,
        });
        let index = if let Some(index) = self.bindings.free.pop() {
            self.bindings.rows[index] = row;
            index
        } else {
            self.bindings.rows.push(row);
            self.bindings.rows.len() - 1
        };
        self.mounts.get_mut(node).unwrap().bindings = Some(index);
    }

    pub(super) fn retire_bindings(&mut self, mut link: Option<usize>) {
        while let Some(index) = link {
            let row = self.bindings.rows[index].take().unwrap();
            link = row.next;
            if let Some(callback) = row.effect.retire() {
                self.retired.push(Retired::new(callback));
            }
            self.bindings.free.push(index);
        }
    }
}

impl Host {
    fn queue_channel(&mut self, owner: NodeId, target: NodeId, prop: Prop, bind: Bind) {
        if let Some(row) = self
            .channels
            .iter_mut()
            .find(|row| row.0 == owner && row.2 == prop)
        {
            *row = (owner, target, prop, bind);
        } else {
            self.channels.push((owner, target, prop, bind));
        }
    }

    /// Resource-dependent properties publish after creation, effects and geometry settle.
    pub(super) fn publish_channels(&mut self) {
        for (owner, mut target, prop, bind) in self.channels.drain(..) {
            let Some(row) = self.mounts.get(owner) else {
                continue;
            };
            if prop == Prop::ShadowOpacity && self.appearances.get(target).is_none() {
                let mut at = row.paints;
                while let Some(paint) = self.appearances.get(at) {
                    if paint.halo.is_some() {
                        target = paint.id.node();
                        break;
                    }
                    at = paint.next;
                }
            }
            self.model.bind(target, prop, bind);
        }
    }
}
