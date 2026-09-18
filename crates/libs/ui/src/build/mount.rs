//! Retained subtree lifetime and scene resource publication.
use super::host::{ControlRow, Host};
use super::theme::{Appearance, HaloStyle, Part, PaintMask, PaintSource};
use crate::layout::Edge;
use crate::role::{DataRole, Scope, Silhouette};
use crate::widget::Chrome;
use windows_color::Radiance;
use windows_numerics::Vector2;
use windows_scene::{
    Bind, Cap, Corners, Exit, GeomId, GroupId, Halo, Join, Mask, NodeId, PathVerb, Prop,
    RampId, Spread, SpriteId, Value,
};

#[must_use]

pub fn root_scope() -> Scope {
    Host::with_output(|h| h.root_scope)
}

pub fn set_geometry(id: GeomId, verbs: &[PathVerb]) {
    Host::with_output(|h| h.model().set_geometry(id, verbs));
}

#[derive(Copy, Clone, Debug, PartialEq)]

pub struct Stop {
    /// Where the stop sits along the ramp, `0..=1`.
    pub at: f32,
    /// The chromatic role this stop paints.
    pub role: DataRole,
    /// How much of that role, as an alpha in `0..=1`.
    pub strength: f32,
}

pub fn set_ramp(id: RampId, stops: &[Stop], spread: Spread) {
    Host::with_output(|h| {
        if let Some((held, axis)) = h.ramps.get_mut(id) {
            held.clear();
            held.extend_from_slice(stops);
            *axis = spread;
        }
        resolve_stops(stops, h.root_scope, &mut h.ramp_stops);
        h.model.set_ramp(id, &h.ramp_stops, spread);
    });
}

pub(super) fn resolve_stops(stops: &[Stop], scope: Scope, resolved: &mut Vec<(f32, Radiance)>) {
    resolved.clear();
    resolved.extend(stops.iter().map(|stop| {
        let light = crate::role::data(stop.role, scope);
        (stop.at, light.with_alpha(light.a * stop.strength))
    }));
}

pub(crate) const THUMB_ALPHA: f32 = 0.30;

#[must_use = "dropping a mount unmounts its subtree immediately"]
#[derive(Debug)]

pub(crate) struct Mount {
    roots: Vec<NodeId>,
    runtime: u64,
    exit: Exit,
}

impl Mount {
    pub(super) fn place(
        &self,
        parent: GroupId,
        mut after: Option<NodeId>,
        moving: bool,
    ) -> Option<NodeId> {
        Host::with(|host| {
            if host.identity != self.runtime {
                return after;
            }
            for &root in &self.roots {
                if moving {
                    host.model.place(root, parent, after);
                }
                after = Some(root);
            }
            after
        })
    }
    pub(super) fn new(roots: Vec<NodeId>, runtime: u64) -> Self {
        Self {
            roots,
            runtime,
            exit: Exit::None,
        }
    }

    pub(crate) fn set_exit(&mut self, exit: Exit) {
        self.exit = exit;
    }

    /// Returns the node this subtree is rooted at.
    #[must_use]
    pub fn node(&self) -> NodeId {
        self.roots.first().copied().unwrap_or(NodeId::NONE)
    }

    pub(crate) fn retire(&mut self, host: &mut Host) {
        if host.identity != self.runtime || self.roots.is_empty() {
            return;
        }
        for &root in &self.roots {
            host.retire_tree(root);
        }
        for &root in self.roots.iter().rev() {
            // The model owns the window root across creation transactions.
            if root == host.model.root().node() {
                while host.model.child_count(root) != 0 {
                    let child = host.model.child(root, 0);
                    host.model.destroy(child, self.exit);
                }
            } else {
                host.model.destroy(root, self.exit);
            }
        }
        self.roots.clear();
        host.root_pool.push(core::mem::take(&mut self.roots));
    }
}

impl Drop for Mount {
    fn drop(&mut self) {
        // Non-panicking: a drop during teardown can run after the host has gone, and a
        // panic in a drop takes the process with it.
        Host::try_with(|h| self.retire(h));
    }
}

pub(super) fn emit_halo(
    h: &mut Host,
    style: HaloStyle,
    fill: SpriteId,
    scope: Scope,
    silhouette: Silhouette,
) {
    let paint = scope.for_paint();
    let halo = match style {
        HaloStyle::Glow(role) => {
            let halo = halo_of(
                crate::role::emission(role, paint),
                silhouette,
                crate::role::resolve(role, paint),
            );
            debug_assert!(
                halo.is_some(),
                "a halo was declared in a role the palette gives no light"
            );
            halo
        }
        HaloStyle::Shadow(edge) => {
            let shadow = crate::role::shadow(paint);
            let offset = match edge {
                Edge::Left => Vector2 {
                    x: -shadow.offset,
                    y: 0.0,
                },
                Edge::Right => Vector2 {
                    x: shadow.offset,
                    y: 0.0,
                },
                Edge::Top => Vector2 {
                    x: 0.0,
                    y: -shadow.offset,
                },
                Edge::Bottom => Vector2 {
                    x: 0.0,
                    y: shadow.offset,
                },
            };
            Some(Halo {
                blur: shadow.blur,
                tint: shadow.tint,
                offset,
            })
        }
    };
    h.model().halo(fill, halo);
}

pub(super) fn surface_corners(radius: f32, chrome: Option<Chrome>) -> Corners {
    let mut corners = Corners::all(radius);
    match chrome.and_then(|c| c.attached) {
        Some(Edge::Left) => {
            corners.tl = 0.0;
            corners.bl = 0.0;
        }
        Some(Edge::Right) => {
            corners.tr = 0.0;
            corners.br = 0.0;
        }
        Some(Edge::Top) => {
            corners.tl = 0.0;
            corners.tr = 0.0;
        }
        Some(Edge::Bottom) => {
            corners.bl = 0.0;
            corners.br = 0.0;
        }
        None => {}
    }
    corners
}

pub(super) fn emit_mask(
    h: &mut Host,
    id: SpriteId,
    mask: PaintMask,
    geom: Option<GeomId>,
    scope: Scope,
) {
    let mask = match mask {
        PaintMask::Box { radius } => Mask::Box {
            radius: Corners::all(radius.and_then(|r| r.dips(scope)).unwrap_or(0.0)),
        },
        PaintMask::Radius { dips } => Mask::Box { radius: dips },
        PaintMask::Outline {
            radius,
            width,
            open,
        } => Mask::Outline {
            radius,
            width,
            open,
        },
        PaintMask::Border { radius, width } => Mask::Outline {
            radius: Corners::all(crate::role::metric(radius, scope)),
            width: width.dips(scope).unwrap_or(0.0),
            open: None,
        },
        // A run's coverage tile is minted when its text is shaped, which cannot happen
        // until layout has said how wide it is. Until then the sprite draws nothing.
        PaintMask::Bare => Mask::None,
        PaintMask::Shape { stroke } => Mask::Shape {
            geom: geom.unwrap_or_default(),
            stroke: stroke
                .and_then(|w| w.dips(scope))
                .map(|width| h.model().stroke(width, Cap::Round, Join::Round, &[])),
        },
    };
    h.model().mask(id, mask);
}

pub(super) fn halo_of(
    emission: crate::role::Emission,
    of: Silhouette,
    light: Radiance,
) -> Option<Halo> {
    let spend = emission.of(of);
    spend.is_lit().then(|| Halo {
        blur: spend.sigma,
        tint: light.with_alpha(light.a * spend.strength),
        offset: Vector2 { x: 0.0, y: 0.0 },
    })
}

pub(super) fn install_scroll(
    h: &mut Host,
    viewport: GroupId,
    content: NodeId,
    decl: crate::layout::ScrollDecl,
    scope: Scope,
    row: NodeId,
) {
    let reveal = decl.reveal;
    let tracker = h.model().tracker_id::<windows_scene::Observed>();
    h.trackers.push(super::host::TrackerSpec {
        id: tracker,
        viewport,
        content,
        axes: windows_scene::Axes::VERTICAL,
    });
    // The scrollbar lives in the viewport rather than in the content, so it does not
    // scroll with what it reports on, and above the content, because child order is paint
    // order and the order the hit array is scanned in. Below it, the bar paints under
    // whatever the list draws and a grab resolves to the row behind it.
    //
    // The rail is static geometry and carries the hit target; the thumb is moved by the
    // compositor and carries none. A hit entry on the thumb would name a rect the solve
    // fixed and the tracker then moved away from.
    let bar = (reveal != crate::layout::Reveal::Never).then(|| {
        let rail = h.model().group(viewport, Some(content));
        h.model().style(rail.node(), &crate::layout::rail_style());
        let thumb = h.model().visual(rail, None);
        // An ordinary appearance, so the one resolver that repaints every other sprite on a
        // theme change repaints this one too. It hangs on no mount: the rail is model
        // geometry rather than a declared node, and the scroll row releases it.
        h.appearances.place(
            thumb.node(),
            Appearance {
                id: thumb,
                mask: PaintMask::Radius {
                    dips: Corners::all(crate::layout::THUMB_W * 0.5),
                },
                source: PaintSource::Role(crate::role::Role::Text(crate::role::Text::Primary)),
                part: Part::Static,
                strength: THUMB_ALPHA,
                geom: None,
                scope,
                surface: None,
                halo: None,
                next: NodeId::NONE,
                wash: false,
            },
        );
        h.appearances.get(thumb.node()).copied().unwrap().publish(h, true);
        // Hidden from the mount rather than shown and faded out: a surface whose content
        // fits never overflows, and a thumb visible for one frame to say so is a flash on
        // every screen that opens.
        if reveal == crate::layout::Reveal::OnDemand {
            h.model()
                .bind(thumb.node(), Prop::Opacity, Bind::Set(Value::Scalar(0.0)));
        }
        // The rail's control carries a hit entry and a drag and no chrome row: the
        // thumb's opacity belongs to the reveal policy, and a row the front table adopted
        // would give that channel two owners. The hit entry itself is written by
        // `publish_scrolls`, because whether the rail is a target at all depends on
        // whether there is anything to scroll, which is a solve output.
        let id = h.mint_control(ControlRow::new(rail.node(), scope));
        h.gestures.push((id, crate::layout::grab_decl()));
        (rail, thumb, id)
    });
    let control = h.mounts.get(row).and_then(|row| row.control);
    h.scrolls.place(
        row,
        crate::layout::ScrollRow {
            tracker,
            viewport: viewport.node(),
            content,
            thumb: bar.map(|(_, thumb, _)| thumb),
            rail: bar.map(|(rail, ..)| rail),
            control,
            grab: bar.map(|(.., id)| id),
            reveal,
            state: decl.state,
            last: crate::layout::ThumbGeom::default(),
            front_added: false,
        },
    );
}
