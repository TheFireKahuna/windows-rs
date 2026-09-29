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
//! # Which entries each pass reads
//!
//! The passes run in a fixed order and some of them name entries of their own: the visuals
//! pass places sprites, the text pass places line tiles, an overlay's placement translates a
//! subtree, and a draw callback or the second encode can move a box after every pass has
//! run. So each pass that reads the set keeps its own cursor, reads from it to the end, and
//! moves it to the end once it is done, past whatever it named itself. Every entry reaches
//! every pass exactly once, however late in a flush it was named, and a flush drains only the
//! prefix every cursor has passed.

use super::Host;

/// The passes that read the change set, each with a cursor of its own.
#[derive(Copy, Clone)]
pub(crate) enum Pass {
    Visuals,
    Rounded,
    Text,
    Masks,
    Anchors,
}

impl Pass {
    /// Every pass, in the order a flush runs them.
    pub(crate) const ALL: [Self; 5] = [Self::Visuals, Self::Rounded, Self::Text, Self::Anchors, Self::Masks];
    const COUNT: usize = Self::ALL.len();
}

pub(crate) struct Changes {
    /// How far into the change set each pass has read.
    cursors: [usize; Pass::COUNT],
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
            cursors: [0; Pass::COUNT],
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

    /// The entries `pass` has not read: from its cursor to the end as the set stands now.
    ///
    /// A range and not a slice, because the pass writes the tree while it reads; entries it
    /// names on the way are past the range and are its own.
    pub(crate) fn unread(&self, pass: Pass) -> core::ops::Range<usize> {
        self.changes.cursors[pass as usize]..self.tree.moved.len()
    }

    /// Moves `pass`'s cursor to the end of the set, past everything it read and everything
    /// it named while reading: the rows it just resolved are already current for it.
    pub(crate) fn mark_read(&mut self, pass: Pass) {
        self.changes.cursors[pass as usize] = self.tree.moved.len();
    }

    /// Drains the prefix every pass has read, and ends the flush's sweep.
    pub(crate) fn close_changes(&mut self) {
        let read = self.changes.cursors.iter().copied().min().unwrap_or(0);
        self.tree.moved.drain(..read);
        for cursor in &mut self.changes.cursors {
            *cursor -= read;
        }
        self.changes.sweeping = false;
    }
}
