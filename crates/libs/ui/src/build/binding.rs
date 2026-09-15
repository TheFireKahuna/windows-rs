//! Node-owned destinations. Writer slots and deferred destruction reuse host storage.
use super::Host;
use crate::signal::{Effect, RetiredEffect};
use std::rc::Rc;
use windows_scene::{Anim, Bind, NodeId, Prop, Tuning, Value};

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

/// Existing allocations move here; no wrapper allocation is required for retirement.
pub(super) enum Retired {
    Region(crate::present::Build),
    Text(Rc<dyn Fn(&str)>),
    Effect(RetiredEffect),
    Click(Rc<dyn Fn()>),
    Change(Rc<dyn Fn(f64)>),
    Drag(Rc<dyn Fn(crate::widget::Dragging)>),
    Tip(Rc<crate::widget::TextSource>),
    Flyout(Rc<dyn Fn(&mut super::Ui<'_>)>),
    Escape(Rc<dyn Fn()>),
}

impl Retired {
    pub(super) fn release(self) {
        match self {
            Self::Region(value) => drop(value),
            Self::Text(value) => drop(value),
            Self::Effect(value) => drop(value),
            Self::Click(value) => drop(value),
            Self::Change(value) => drop(value),
            Self::Drag(value) => drop(value),
            Self::Tip(value) => drop(value),
            Self::Flyout(value) => drop(value),
            Self::Escape(value) => drop(value),
        }
    }
}

impl super::host::ControlRow {
    pub(super) fn retire(mut self, pending: &mut Vec<Retired>) {
        for callback in [self.click.take(), self.cancel.take()]
            .into_iter()
            .flatten()
        {
            pending.push(Retired::Click(callback));
        }
        for callback in [self.change.take(), self.commit.take()]
            .into_iter()
            .flatten()
        {
            pending.push(Retired::Change(callback));
        }
        if let Some(callback) = self.drag.take() {
            pending.push(Retired::Drag(callback));
        }
        if let Some((text, _)) = self.tip.take() {
            pending.push(Retired::Tip(text));
        }
        if let Some(callback) = self.flyout.take() {
            pending.push(Retired::Flyout(callback));
        }
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
                    self.retired.push(Retired::Effect(callback));
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
                self.retired.push(Retired::Effect(callback));
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
