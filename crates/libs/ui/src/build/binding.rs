//! Node-owned destinations. Writer slots and deferred destruction reuse host storage.
//!
//! One retained writer per destination, chained from the node's `bindings` head, so replacing
//! a source retires its predecessor and retiring a node drops the lot. Every drop that runs
//! application code is held until the host's borrow has ended.

use super::control::Handlers;
use super::host::Host;
use super::tree::{self, Pool};
use crate::signal::{Effect, Signal};
use windows_scene::{Anim, Bind, ControlId, NodeId, Prop, Tuning, Value};

/// One value whose drop belongs outside the host's borrow, with its type erased.
///
/// Erasure rather than a variant per payload: dropping is the only thing ever done with one,
/// and a named arm per kind makes every new handler cost a match arm as well as a setter.
pub(crate) struct Retired(#[expect(dead_code, reason = "held only to be dropped")] Box<dyn core::any::Any>);

impl Retired {
    pub(crate) fn new<T: 'static>(value: T) -> Self {
        Self(Box::new(value))
    }
}

/// Handler rows and the indices free to take one.
///
/// A free list rather than a table keyed by [`ControlId`]: a keyed one grows to the highest
/// live control and holds a full-size vacant row for every control below it that declared
/// nothing, which is the storage this table exists to not spend.
#[derive(Default)]
pub(crate) struct HandlerTable {
    rows: Vec<Handlers>,
    free: Vec<u32>,
}

impl HandlerTable {
    /// Returns how many rows are placed, vacant ones excluded.
    #[cfg(test)]
    pub(crate) fn placed(&self) -> usize {
        self.rows.len() - self.free.len()
    }

    /// Returns `at`'s row, placing one the first time the control declares a handler.
    pub(crate) fn claim(&mut self, at: &mut u32) -> &mut Handlers {
        if *at == tree::NONE {
            *at = match self.free.pop() {
                Some(free) => free,
                None => {
                    self.rows.push(Handlers::default());
                    self.rows.len() as u32 - 1
                }
            };
        }
        &mut self.rows[*at as usize]
    }

    pub(crate) fn get(&self, at: u32) -> Option<&Handlers> {
        (at != tree::NONE).then(|| &self.rows[at as usize])
    }

    /// Vacates a released control's row, retiring what it held outside the host's borrow.
    pub(crate) fn vacate(&mut self, at: u32, retired: &mut Vec<Retired>) {
        if at == tree::NONE {
            return;
        }
        let row = core::mem::take(&mut self.rows[at as usize]);
        retired.push(Retired::new(row));
        self.free.push(at);
    }
}

/// One retained writer for one destination.
///
/// A row per installed binding, chained from the node's `bindings` head, so retirement walks
/// the chain and drops every writer the node owned.
pub(crate) struct Binder {
    next: u32,
    prop: Prop,
    effect: Effect,
}

impl Host {
    /// Schedules a node-owned binding without entering the host during construction.
    ///
    /// Deferred until the creation borrow ends and run before first publication, so a
    /// resource-dependent property publishes after its masks and halos exist, in the same
    /// scene batch, and its first value snaps instead of animating from a default. The
    /// binding belongs to the signal scope current at this call, which creation installs.
    pub(crate) fn binding(&mut self, run: impl FnMut() + 'static) -> Effect {
        Effect::deferred(run)
    }

    /// Installs a writer for one channel, retiring whatever it displaces.
    ///
    /// A constant writes the record and installs no effect; a reactive source installs a
    /// writer for that destination alone.
    pub(crate) fn install_channel<T, M>(
        &mut self,
        node: NodeId,
        prop: Prop,
        value: impl Signal<T, M> + 'static,
    ) where
        T: Copy + Into<Value> + PartialEq + 'static,
    {
        self.retire_channel(node, prop);
        if value.is_constant() {
            self.write_channel(node, prop, value.read().into());
            return;
        }
        let mut last: Option<T> = None;
        let effect = self.binding(move || {
            let next = value.read();
            if last.replace(next) != Some(next) {
                Host::with(|h| h.write_channel(node, prop, next.into()));
            }
        });
        let next = self.tree.c.bindings[node.index()];
        let at = self.binders.place(Binder { next, prop, effect });
        self.tree.c.bindings[node.index()] = at;
    }

    /// Queues one channel write, snapping the first value and springing afterwards.
    ///
    /// Queued rather than emitted: a resource-dependent property must publish after the
    /// masks and halos it names exist, in the same scene batch. The queue is keyed by
    /// `(node, prop)`, so the last writer of a channel in one transaction is the one that
    /// crosses.
    pub(crate) fn write_channel(&mut self, node: NodeId, prop: Prop, value: Value) {
        let channels = &mut self.tree.c.channels[node.index()];
        let first = *channels & (1 << prop as u32) == 0;
        *channels |= 1 << prop as u32;
        let bind = if first || snaps(prop) {
            Bind::Set(value)
        } else {
            Bind::Animate(Anim::Spring { to: value, tuning: Tuning::Chrome, delay_ms: 0 })
        };
        match self.queued.iter_mut().find(|row| row.0 == node && row.1 == prop) {
            Some(row) => row.2 = bind,
            None => self.queued.push((node, prop, bind)),
        }
    }

    /// Emits the queued channel writes, each addressed to the part that owns the property.
    ///
    /// A halo's opacity belongs to the sprite the halo hangs on rather than to the node that
    /// declared it, so the chain is walked here, once the paints exist.
    pub(crate) fn publish_channels(&mut self) {
        let mut queued = core::mem::take(&mut self.queued);
        for (node, prop, bind) in queued.drain(..) {
            let target = if matches!(prop, Prop::ShadowOpacity | Prop::BlurRadius) {
                let head = self.tree.c.paints[node.index()];
                let bearer = self.appearances.halo_bearer(head);
                if bearer.is_none() { node } else { bearer }
            } else {
                node
            };
            self.bind(target, prop, bind);
        }
        self.queued = queued;
    }

    /// Drops the writer a destination already had, so reuse of this node's slot cannot leave
    /// a writer pointing at a recycled generation.
    fn retire_channel(&mut self, node: NodeId, prop: Prop) {
        let mut at = self.tree.c.bindings[node.index()];
        let mut prev = tree::NONE;
        while at != tree::NONE {
            let next = self.binders[at].next;
            if self.binders[at].prop == prop {
                if let Some(row) = self.binders.free(at) {
                    self.retired.push(Retired::new(row.effect.retire()));
                }
                if prev == tree::NONE {
                    self.tree.c.bindings[node.index()] = next;
                } else {
                    self.binders[prev].next = next;
                }
                return;
            }
            (prev, at) = (at, next);
        }
    }

    /// Releases every writer a retiring node owned.
    pub(crate) fn binding_release(&mut self, mut at: u32) {
        while at != tree::NONE {
            let Some(row) = self.binders.free(at) else { return };
            at = row.next;
            self.retired.push(Retired::new(row.effect.retire()));
        }
    }

    /// Installs one handler on a control, retiring whatever it displaces.
    ///
    /// Repeating a setter therefore replaces, and the displaced value is released outside
    /// this borrow since dropping it runs whatever the application captured.
    pub(crate) fn set_handler(
        &mut self,
        id: ControlId,
        write: impl FnOnce(&mut Handlers) -> Option<Retired>,
    ) {
        let Some(row) = self.control(id) else { return };
        let mut at = row.handlers;
        let displaced = write(self.handlers.claim(&mut at));
        if let Some(row) = self.control_mut(id) {
            row.handlers = at;
        }
        self.retired.extend(displaced);
    }
}

/// Whether a channel is written outright rather than sprung toward.
///
/// A geometry channel the solve owns snaps: springing it would animate every box on a window
/// resize. A chrome channel a hover or a press owns springs.
const fn snaps(prop: Prop) -> bool {
    matches!(
        prop,
        Prop::Offset
            | Prop::OffsetX
            | Prop::OffsetY
            | Prop::Size
            | Prop::SizeX
            | Prop::SizeY
            | Prop::Center
            | Prop::CenterX
            | Prop::CenterY
            | Prop::ClipL
            | Prop::ClipT
            | Prop::ClipR
            | Prop::ClipB
    )
}

/// The binder pool, held on the host behind the node's `bindings` head.
pub(crate) type Binders = Pool<Binder>;
