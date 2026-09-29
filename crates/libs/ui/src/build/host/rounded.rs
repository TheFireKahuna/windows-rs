use super::*;
use windows_scene::{Clip, Corners};

pub(super) struct Rounded {
    node: NodeId,
    radius: Len,
    sent: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{Ui, rig::fixture_at};
    use crate::layout::Preset;
    use crate::signal::Owner;

    #[test]
    fn rounded_clip_resolves_dpi_without_republishing_size_or_leaking_rows() {
        for dpi in [96.0, 144.0, 192.0] {
            let mut patch = fixture_at(dpi);
            let (_owner, mount) = Owner::scope(|| Ui::mount_root(|ui| {
                ui.node(Preset::Stack).grow().animate_layout().clip_rounded(Len::px(9.0));
            }));
            Host::flush(&mut patch);
            let clips: Vec<_> = patch.ops().iter().filter_map(|op| match *op {
                Op::Clip { id, clip } => Some((id, clip)),
                _ => None,
            }).collect();
            assert_eq!(clips.len(), 1);
            let (id, clip) = clips[0];
            assert_eq!(clip, Clip::RoundedBounds(Corners::all(9.0 * 96.0 / dpi)));
            for width in [400.0, 600.0, 800.0] {
                patch.clear();
                Host::with(|h| h.set_window(Vector2::new(width, 600.0)));
                Host::flush(&mut patch);
                assert!(!patch.ops().iter().any(|op| matches!(op, Op::Clip { id: n, .. } if *n == id)));
            }
            Host::with(|h| {
                let env = Env::new(dpi * 2.0, h.env.output());
                h.set_env(env);
            });
            patch.clear();
            Host::flush(&mut patch);
            assert!(patch.ops().iter().any(|op| matches!(op,
                Op::Clip { id: n, clip: Clip::RoundedBounds(radius) }
                if *n == id && radius.tl == 9.0 * 48.0 / dpi)));
            patch.clear();
            Host::flush(&mut patch);
            let before = crate::counting::allocations();
            for _ in 0..20 { Host::flush(&mut patch); }
            assert!(patch.ops().is_empty());
            assert_eq!(crate::counting::allocations(), before);
            drop(mount);
            Host::flush(&mut patch);
            Host::with(|h| assert_eq!(h.rounded.len(), 0));
        }
    }
}

impl Host {
    pub(crate) fn clip_rounded(&mut self, node: NodeId, radius: Len) {
        let at = self.side_mut(node).rounded;
        if at != tree::NONE {
            self.rounded[at].radius = radius;
        } else {
            let at = self.rounded.place(Rounded { node, radius, sent: f32::NAN });
            self.side_mut(node).rounded = at;
        }
        // Names the node rather than the row: the pass finds the row through the side row,
        // as it does for a node whose scope moved.
        self.tree.moved.push(node);
    }

    /// Resolves each rounded clip whose radius, scope or scale may have moved.
    ///
    /// The radius resolves from the node's scope and the scale and never from its box, so
    /// the pass visits the nodes the change set names — a new radius names its node, and so
    /// does an elevation — and every row on a sweep.
    pub(super) fn publish_rounded_clips(&mut self) {
        if self.changes.sweeping() {
            for at in 0..self.rounded.slots() {
                self.publish_rounded(at);
            }
        } else {
            for i in self.unread(changes::Pass::Rounded) {
                let node = self.tree.moved[i];
                if !self.tree.is_live(node) {
                    continue;
                }
                let at = self.tree.c.side[node.index()];
                if at != tree::NONE && self.sides[at].rounded != tree::NONE {
                    self.publish_rounded(self.sides[at].rounded);
                }
            }
        }
        self.mark_read(changes::Pass::Rounded);
        #[cfg(debug_assertions)]
        for at in 0..self.rounded.slots() {
            if let Some(row) = self.rounded.get(at) {
                debug_assert!(
                    self.rounded_radius(row.node, row.radius) == row.sent,
                    "the change set missed a rounded clip: {:?}", row.node
                );
            }
        }
    }

    fn rounded_radius(&self, node: NodeId, radius: Len) -> f32 {
        radius.dips_at(self.scope_of(node), self.env.scale()).max(0.0)
    }

    fn publish_rounded(&mut self, at: u32) {
        self.changes.visit();
        let Some(row) = self.rounded.get(at) else { return };
        let node = row.node;
        let radius = self.rounded_radius(node, row.radius);
        if radius != row.sent {
            self.pending.push(Op::Clip { id: node, clip: Clip::RoundedBounds(Corners::all(radius)) });
            self.rounded[at].sent = radius;
        }
    }
}
