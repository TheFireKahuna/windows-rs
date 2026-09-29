//! Owner-scoped local geometry over the existing layout publication and signal graph.
//!
//! A job is one geometry-phase effect and the retained paths it fills. It reads the box this
//! batch solved, tracks whatever else the fill closure reads, and cannot write layout,
//! structure or application state.

use super::binding::Retired;
use super::host::Host;
use super::tree::{Pool, Tree};
use crate::layout::{Anchors, Probe};
use crate::signal::Effect;
use windows_numerics::Vector2;
use windows_scene::{GeomId, Id, NodeId, PathVerb};

/// Where a job's box comes from.
#[derive(Copy, Clone, PartialEq)]
pub enum Source {
    /// This node's own solved local box.
    Own(NodeId),
    /// Any probe, not necessarily this node's: the box a set of figures has to agree with is
    /// often a container none of them belongs to.
    Probe(Probe),
    /// A whole keyed set, in its origin container's space.
    Anchors(Anchors),
}

/// What a fill callback is handed: the box, the scope, and the DPI the figure is
/// rasterized at.
pub struct Inputs<'a> {
    pub size: Vector2,
    pub scope: crate::role::Scope,
    pub anchors: Option<&'a crate::layout::Table>,
    /// The DIP-to-pixel factor, so a figure can sample at its rasterized density.
    pub scale: f32,
}

struct Job {
    source: Source,
    effect: Effect,
    /// The box the fill last ran against, so a solve that moved nothing wakes nothing.
    last: Vector2,
}

/// The jobs, pooled and headed by the node's side row.
#[derive(Default)]
pub(crate) struct Jobs(Pool<Job>);

impl Jobs {
    /// Schedules every job whose source box moved since the last publication.
    ///
    /// Gated here rather than inside the effect, so a solve that moved nothing wakes nothing;
    /// a tracked read inside the fill schedules it through the graph as usual.
    pub(crate) fn schedule(&mut self, tree: &Tree) {
        for (_, job) in self.0.iter_mut() {
            let Source::Own(node) = job.source else { continue };
            if !tree.is_live(node) {
                continue;
            }
            let size = tree.c.geom[node.index()].size;
            if size != job.last {
                job.last = size;
                job.effect.schedule();
            }
        }
    }

    pub(crate) fn release(&mut self, at: u32, retired: &mut Vec<Retired>) {
        if let Some(job) = self.0.free(at) {
            retired.push(Retired::new(job.effect.retire()));
        }
    }
}

/// Holds a retained scene resource for as long as the signal scope that minted it lives.
///
/// The resource outlives no owner: `Ui::geometry` and `Ui::ramp` hand back an id the
/// application keeps, and the release is owed when the scope that minted it is disposed
/// rather than when a node unmounts, since the two are not the same lifetime.
pub(crate) struct Lease<const F: u8>(pub Id<F>);

impl<const F: u8> Drop for Lease<F> {
    fn drop(&mut self) {
        // Non-panicking: a lease can outlive the host at thread teardown.
        Host::try_with(|h| h.release(self.0));
    }
}

impl Host {
    /// The box and the scope a job whose source is one node's own reads.
    ///
    /// A keyed set is not answered here: its table is borrowed from the set for exactly the
    /// call that reads it, so it cannot be carried out past this borrow.
    pub(crate) fn own_inputs(&self, node: NodeId, source: Source) -> Inputs<'static> {
        match source {
            Source::Probe(probe) => {
                let placed = probe.get();
                // A probe read before its node's first solve has no scope yet; the root's is what that
                // node would inherit.
                let scope = placed.scope.unwrap_or_else(|| self.root_scope());
                Inputs { size: placed.size, scope, anchors: None, scale: self.env.scale() }
            }
            Source::Own(own) => Inputs {
                size: self.tree.c.geom[own.index()].size,
                scope: self.scope_of(own),
                anchors: None,
                scale: self.env.scale(),
            },
            Source::Anchors(_) => Inputs {
                size: self.tree.c.geom[node.index()].size,
                scope: self.scope_of(node),
                anchors: None,
                scale: self.env.scale(),
            },
        }
    }

    /// Attaches one job to `node`, minting its retained geometries and reserving its buffers.
    ///
    /// The buffers are reserved once and reused, so re-emitting a figure allocates nothing
    /// after the first pass.
    pub(crate) fn add_geometry_job<const N: usize>(
        &mut self,
        node: NodeId,
        source: Source,
        ids: [GeomId; N],
        caps: [usize; N],
        mut fill: impl FnMut(&Inputs<'_>, &mut [Vec<PathVerb>; N]) + 'static,
    ) {
        let mut buffers: [Vec<PathVerb>; N] =
            core::array::from_fn(|at| Vec::with_capacity(caps[at]));
        // The fill is application code, so it runs outside the host borrow: it may set a ramp
        // or read a probe, and both enter the host themselves. Signal writes are refused for
        // its duration: a write here would feed the solve this effect runs downstream of.
        let effect = Effect::geometry(move || {
            for buffer in &mut buffers {
                buffer.clear();
            }
            match source {
                Source::Anchors(set) => set.with(|table| {
                    let Some(scope) = table.published() else { return };
                    let scale = Host::with(|h| h.env.scale());
                    let inputs = Inputs { size: table.size(), scope, anchors: Some(table), scale };
                    crate::signal::read_only(|| fill(&inputs, &mut buffers));
                }),
                _ => {
                    let inputs = Host::with(|h| h.own_inputs(node, source));
                    crate::signal::read_only(|| fill(&inputs, &mut buffers));
                }
            }
            Host::with(|h| {
                for (id, buffer) in ids.iter().zip(&buffers) {
                    h.set_geometry(*id, buffer);
                }
            });
        });
        let last = Vector2 { x: f32::NAN, y: f32::NAN };
        let at = self.geometry.0.place(Job { source, effect, last });
        self.set_geometry_job(node, at);
    }
}
