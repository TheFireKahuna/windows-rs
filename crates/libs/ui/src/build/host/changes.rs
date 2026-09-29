//! What one flush's passes read instead of their whole tables.
//!
//! The solve and the publishers name every node whose box, width class or visibility
//! changed in [`Tree::moved`](super::super::tree::Tree::moved), and each pass keeps its own
//! list of rows whose own inputs changed. A pass after the solve visits those and nothing
//! else, so a flush costs what moved rather than what is mounted.
//!
//! A change that reaches every row without moving any box — a new pixel scale, a theme —
//! owes a **sweep** instead: the next flush's passes each walk their whole table once, which
//! is what every flush did before there was a change set.
//!
//! # Which entries a flush drains
//!
//! The passes run in a fixed order and some of them move boxes of their own: the visuals
//! pass places sprites, an overlay's placement translates a subtree. An entry named after a
//! pass has read the set is one that pass has not seen, so a flush drains only what was
//! named before the first pass read it, and the rest are read again, by every pass, on the
//! next flush. Reading an entry twice costs a comparison; every pass is idempotent.

use super::Host;

pub(crate) struct Changes {
    /// How much of the change set was named before the first pass read it.
    read: usize,
    /// This flush's passes walk their whole tables.
    sweeping: bool,
    /// The next flush's do.
    owed: bool,
    /// The pixel scale the last flush resolved at. A flush that finds another sweeps,
    /// however the scale was changed.
    scale: f32,
    /// How many rows the passes have visited, for a test to bound.
    #[cfg(test)]
    pub(crate) rows: u32,
}

impl Default for Changes {
    fn default() -> Self {
        // Nothing has been published yet, so the first flush reads everything.
        Self {
            read: 0,
            sweeping: false,
            owed: true,
            scale: f32::NAN,
            #[cfg(test)]
            rows: 0,
        }
    }
}

impl Changes {
    /// Owes the next flush a sweep.
    pub(crate) fn owe_sweep(&mut self) {
        self.owed = true;
    }

    /// Whether this flush's passes walk their whole tables.
    pub(crate) fn sweeping(&self) -> bool {
        self.sweeping
    }

    /// Counts one row a pass visited. Compiled out of everything but tests.
    #[inline]
    pub(crate) fn visit(&mut self) {
        #[cfg(test)]
        {
            self.rows += 1;
        }
    }

    /// How much of the change set the first pass reads.
    pub(super) fn len(&self) -> usize {
        self.read
    }
}

impl Host {
    /// Decides whether this flush sweeps, before its first pass.
    ///
    /// A sweep owed during the flush — by a draw callback, say — is left owed for the next.
    pub(super) fn begin_changes(&mut self) {
        let scale = self.env.scale();
        let rescaled = scale.to_bits() != self.changes.scale.to_bits();
        self.changes.scale = scale;
        self.changes.sweeping = core::mem::take(&mut self.changes.owed) || rescaled;
    }

    /// Fixes how much of the change set this flush's passes read: all of it as it stands
    /// once the solve has run.
    pub(super) fn open_changes(&mut self) {
        self.changes.read = self.tree.moved.len();
    }

    /// Drains what every pass of this flush has read.
    pub(super) fn close_changes(&mut self) {
        self.tree.moved.drain(..self.changes.read);
        self.changes.read = 0;
        self.changes.sweeping = false;
    }
}
