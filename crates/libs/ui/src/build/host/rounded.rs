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
    }

    pub(super) fn publish_rounded_clips(&mut self) {
        for at in 0..self.rounded.slots() {
            let Some(row) = self.rounded.get(at) else { continue };
            let node = row.node;
            let radius = row.radius.dips_at(self.scope_of(node), self.env.scale()).max(0.0);
            if radius != row.sent {
                self.pending.push(Op::Clip { id: node, clip: Clip::RoundedBounds(Corners::all(radius)) });
                self.rounded[at].sent = radius;
            }
        }
    }
}
