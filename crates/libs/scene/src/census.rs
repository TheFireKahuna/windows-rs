//! Counting our own writes, at the site that issues them. **Front half.**
//!
//! Visual count is the compositor's frontier at idle — the tree *walk*, not the drawing —
//! so how many visuals exist and how often a property is written are first-class costs
//! rather than diagnostics.
//!
//! They are counted **here** and not in the composition wrapper. A counter welded into
//! every setter of an object model is a consumer's measurement need edited into an upstream
//! method body, which is the worst possible rebase surface. The patch applier sees the
//! identical events and is ours.

/// What this crate did, since it started.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Census {
    /// Visuals this scene is holding, ghosts included.
    ///
    /// One life event each: a mint raises it and a destroy lowers it. Re-parenting is
    /// neither, which is why an unsigned count is sound — a move that both unlinks and
    /// links must net to nothing rather than to a number that can go below zero.
    pub visuals_live: u32,
    /// Visuals ever minted. The difference from `visuals_live` is what was destroyed, and
    /// a mint rate that climbs while the live count is flat is recycling that is not
    /// recycling.
    pub visuals_minted: u64,
    /// Property groups actually pushed to a composition object — the writes that survived
    /// the idempotent early return, and therefore the ones that cost something.
    pub props_written: u64,
    /// Property writes the early return absorbed. A high ratio against `props_written` is
    /// the emitter sending a whole subtree where three nodes moved.
    pub props_skipped: u64,
    pub ops_applied: u64,
    /// Animations started, which is the event-rate cost of motion.
    pub animations: u64,
    /// Patches applied under a different environment than they were solved under.
    ///
    /// **Counted rather than refused, because a mismatch is not necessarily wrong.** A
    /// display change that lands between a flush and its apply leaves the patch's geometry
    /// snapped to the previous pixel grid, and the right response is to apply it and let
    /// the next solve correct it — the scene syncs to the environment it was *given*, so
    /// the rasters are right and only one frame of placement is stale.
    ///
    /// What that makes this is a discriminator. An occasional bump is that race. A count
    /// that keeps pace with the patch count is the failure the [`Env`](crate::Env) seam
    /// exists to prevent, surviving one level up: two derivations of one fact, on two
    /// threads, silently disagreeing. Neither is visible in any single call.
    pub env_mismatches: u64,
}

/// A walk's ground truth against the running tallies.
///
/// A running count drifts; only a walk knows. The two disagreeing is a link or a life event
/// being wrong, and neither shows up any other way — a leaked node still renders.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Audit {
    /// Nodes the walk reached from the root.
    pub reached: u32,
    /// Nodes the arena is holding. Above `reached` means something was orphaned rather
    /// than destroyed; below means the chain has a cycle or crosses parents.
    pub held: u32,
}

impl Audit {
    /// Whether the tree the walk found is the tree the arena holds.
    #[must_use]
    pub const fn agrees(&self) -> bool {
        self.reached == self.held
    }
}

impl Census {
    /// Records one attempted property write.
    ///
    /// A skip is not a non-event: the ratio against `props_written` is what says whether
    /// the emitter is sending a subtree where three nodes moved.
    pub(crate) fn count(&mut self, written: bool) {
        if written {
            self.props_written += 1;
        } else {
            self.props_skipped += 1;
        }
    }

    /// Whether the tick did anything: something was applied, or an animation started.
    ///
    /// A woken tick that answers `false` is something having asked for a frame it did not
    /// need, which is the one shape of idle waste this crate cannot see from the inside.
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
        assert!(!after.changed_since(&before), "an absorbed write is not a change");
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
