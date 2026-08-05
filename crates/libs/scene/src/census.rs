//! Counting this crate's own writes, at the site that issues them. **Front half.**
//!
//! At idle the compositor's cost is the tree walk rather than the drawing, so the number of
//! visuals alive and the number of property writes issued are first-class costs rather than
//! diagnostics.
//!
//! Counting happens in the patch applier, which sees every mint, destroy and property write,
//! and not in the composition wrapper's setters.

/// Running tallies of what the scene has done since it was created.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Census {
    /// Visuals this scene is holding, ghosts included.
    ///
    /// One life event each: a mint raises it and a destroy lowers it. Re-parenting is
    /// neither, so a move that both unlinks and links nets to nothing and the count never
    /// goes below zero.
    pub visuals_live: u32,
    /// Visuals ever minted. The difference from `visuals_live` is what was destroyed, and a
    /// mint rate that climbs while the live count is flat is nodes being rebuilt instead of
    /// reused.
    pub visuals_minted: u64,
    /// Property groups actually pushed to a composition object — the writes that survived
    /// the idempotent early return, and therefore the ones that cost something.
    pub props_written: u64,
    /// Property writes the early return absorbed. A high ratio against `props_written` is
    /// the emitter sending a whole subtree where three nodes moved.
    pub props_skipped: u64,
    /// Interaction trackers this scene is holding.
    ///
    /// Watched because a tracker that was never built is invisible from every other angle:
    /// the bindings onto it apply, the ops addressed to it are dropped on a missing row, and
    /// the surface never scrolls. One life event each, like the visuals.
    pub trackers_live: u32,
    /// Ops applied, across every patch.
    pub ops_applied: u64,
    /// Animations started, which is the event-rate cost of motion.
    pub animations: u64,
    /// Patches applied under a different environment than they were solved under.
    ///
    /// A mismatch is counted and not refused. A display change landing between a flush and
    /// its apply leaves that patch's geometry snapped to the previous pixel grid; the scene
    /// applies it under the environment it was given, so the rasters are right and one frame
    /// of placement is stale until the next solve.
    ///
    /// An occasional bump is that race. A count that keeps pace with the patch count is the
    /// two halves deriving the [`Env`](crate::Env) independently and disagreeing, which no
    /// single call shows.
    pub env_mismatches: u64,
}

/// What a walk of the tree found, against what the arena holds.
///
/// The two disagreeing means a link or a life event is wrong, which nothing else reports: an
/// orphaned node still renders.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Audit {
    /// Nodes the walk reached from the root.
    pub reached: u32,
    /// Nodes the arena is holding. Above `reached` means something was orphaned rather
    /// than destroyed; below means the chain has a cycle or crosses parents.
    pub held: u32,
}

impl Audit {
    /// Returns whether the tree the walk found is the tree the arena holds.
    #[must_use]
    pub const fn agrees(&self) -> bool {
        self.reached == self.held
    }
}

impl Census {
    /// Records one attempted property write, as written or as absorbed by the idempotent
    /// early return.
    pub(crate) fn count(&mut self, written: bool) {
        if written {
            self.props_written += 1;
        } else {
            self.props_skipped += 1;
        }
    }

    /// Returns whether the tick did anything: an op applied, a property written, an
    /// animation started, or a visual minted.
    ///
    /// A woken tick that answers `false` is a frame something asked for and did not need.
    #[must_use]
    pub fn changed_since(&self, previous: &Self) -> bool {
        self.ops_applied != previous.ops_applied
            || self.props_written != previous.props_written
            || self.animations != previous.animations
            || self.visuals_minted != previous.visuals_minted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pass_that_only_skipped_writes_is_not_a_change() {
        let before = Census::default();
        let after = Census {
            props_skipped: 40,
            ..before
        };
        assert!(
            !after.changed_since(&before),
            "an absorbed write is not a change"
        );
    }

    #[test]
    fn a_pass_that_wrote_anything_is_a_change() {
        let before = Census::default();
        for after in [
            Census {
                props_written: 1,
                ..before
            },
            Census {
                ops_applied: 1,
                ..before
            },
            Census {
                animations: 1,
                ..before
            },
            Census {
                visuals_minted: 1,
                ..before
            },
        ] {
            assert!(after.changed_since(&before));
        }
    }
}
