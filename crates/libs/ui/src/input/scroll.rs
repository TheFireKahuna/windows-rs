//! Pointer arbitration for scroll containers, against the same hit array as controls.

use crate::gesture::GestureDecl;
use windows_scene::{ControlId, Hit, HitFlags, HitTable, NO_ENTRY};

/// The nearest viewport containing this hit. An overlay blocker has no parent, so a
/// gesture outside a flyout cannot fall through into the document behind it.
pub(super) fn ancestor(hits: &HitTable, hit: Hit) -> Option<ControlId> {
    let mut at = hit.index;
    while at != NO_ENTRY {
        let entry = hits.entries().get(at as usize)?;
        if entry.flags.contains(HitFlags::SCROLL) {
            return Some(entry.id);
        }
        at = entry.parent;
    }
    None
}

pub(super) fn touch(hits: &HitTable, hit: Hit, decl: GestureDecl) -> Option<ControlId> {
    // Editing gestures retain their target. Ordinary buttons can become a pan after
    // the recogniser crosses its manipulation threshold.
    if hit.flags.contains(HitFlags::TEXT)
        || decl.drag.is_some()
        || decl.pivot.is_some()
        || decl.manipulates()
    {
        None
    } else {
        ancestor(hits, hit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{Host, mount, tests::fixture};
    use crate::layout::{Len, scroll};

    #[test]
    fn content_touch_uses_the_nearest_mounted_scroll_but_editing_keeps_ownership() {
        let mut patch = fixture();
        let _held = mount(
            scroll(
                scroll(crate::widget::button("band"))
                    .height(Len::Times(crate::role::Metric::RowH, 6.0)),
            )
            .height(Len::Times(crate::role::Metric::RowH, 12.0)),
            Host::with(|h| h.model().root()),
        );
        let mut down = crate::seam::Down::default();
        Host::with(|h| {
            h.flush(&mut patch);
            h.fill(&mut down);
        });
        let mut hits = HitTable::default();
        hits.replace(patch.hit_entries());
        let row = down.chrome.iter().find(|row| row.wash.is_some()).unwrap();
        let index = hits
            .entries()
            .iter()
            .position(|entry| entry.id == row.id)
            .unwrap();
        let entry = &hits.entries()[index];
        let hit = Hit {
            index: index as u32,
            id: entry.id,
            flags: entry.flags,
            local: Default::default(),
        };
        let scrolls: Vec<_> = hits
            .entries()
            .iter()
            .filter(|entry| entry.flags.contains(HitFlags::SCROLL))
            .collect();
        assert_eq!(scrolls.len(), 2);
        assert_eq!(touch(&hits, hit, GestureDecl::tap()), Some(scrolls[1].id));
        assert_eq!(touch(&hits, hit, GestureDecl::slider(true)), None);
        assert_eq!(
            touch(&hits, hit, GestureDecl::knob(Default::default(), 20.0)),
            None
        );
        let text = Hit {
            flags: hit.flags | HitFlags::TEXT,
            ..hit
        };
        assert_eq!(touch(&hits, text, GestureDecl::tap()), None);
    }
}
