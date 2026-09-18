//! Retained paint recipes and one window-owned theme transaction.
use super::Host;
use crate::layout::{Edge, Len};
use crate::role::{Metric, Role, Scope, Silhouette};
use crate::widget::{Chrome, ModelState, RoleSet, Wash};
use windows_scene::{
    BackdropSpec, ControlId, Env, GeomId, NodeId, Paint, RampId, RegionId, SpriteId,
};

#[derive(Copy, Clone, Debug)]
pub(crate) enum HaloStyle {
    Glow(Role),
    Shadow(Edge),
}
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Part {
    Static,
    Fill,
    Label,
    Border,
    Trail { origin: f32 },
    Wash,
}
#[derive(Copy, Clone, Debug)]
pub(crate) enum PaintMask {
    Box {
        radius: Option<Len>,
    },
    Radius {
        dips: windows_scene::Corners,
    },
    Outline {
        radius: windows_scene::Corners,
        width: f32,
        open: Option<windows_scene::Side>,
    },
    Border {
        radius: Metric,
        width: Len,
    },
    Shape {
        stroke: Option<Len>,
    },
    Bare,
}

/// A part has one paint source. Owner state never overwrites a gradient or region.
#[derive(Copy, Clone, Debug)]
pub(crate) enum PaintSource {
    Role(Role),
    Owner(ControlId),
    Gradient(RampId),
    Region(RegionId),
}

#[derive(Copy, Clone)]
pub(crate) struct Appearance {
    pub id: SpriteId,
    pub mask: PaintMask,
    pub source: PaintSource,
    pub part: Part,
    pub strength: f32,
    pub geom: Option<GeomId>,
    pub scope: Scope,
    pub surface: Option<NodeId>,
    pub halo: Option<(HaloStyle, Silhouette)>,
    pub next: NodeId,
    pub wash: bool,
}

impl Appearance {
    pub(super) fn resolve(&mut self, host: &Host, scope: Scope) {
        self.scope = scope;
        let Some(surface) = self.surface.and_then(|node| host.surfaces.get(node)) else {
            return;
        };
        let Some(chrome) = surface.chrome else { return };
        if self.wash {
            self.mask = PaintMask::Radius {
                dips: super::mount::surface_corners(
                    crate::role::metric(chrome.radius, scope),
                    Some(chrome),
                ),
            };
        } else if matches!(self.part, Part::Fill | Part::Border) {
            self.mask = chrome_mask(chrome, self.part, scope, surface.selectable);
        }
    }

    fn place(&self, host: &mut Host) {
        let Some(chrome) = self
            .surface
            .and_then(|node| host.surfaces.get(node))
            .and_then(|surface| surface.chrome)
        else {
            return;
        };
        let mut insets = [if self.part == Part::Fill {
            crate::role::metric(Metric::HairlineW, self.scope)
        } else {
            0.0
        }; 4];
        if let Some(edge) = chrome.attached {
            insets[match edge {
                Edge::Left => 0,
                Edge::Right => 1,
                Edge::Top => 2,
                Edge::Bottom => 3,
            }] = 0.0;
        }
        host.model.visual_insets(self.id, insets);
    }

    /// The same resolver publishes initial appearance, theme changes and owner-state changes.
    pub(super) fn publish(&self, host: &mut Host, mask: bool) {
        if mask {
            self.place(host);
            super::mount::emit_mask(host, self.id, self.mask, self.geom, self.scope);
        }
        let role = match self.source {
            PaintSource::Role(role) => Some((role, self.strength)),
            PaintSource::Owner(id) => host.controls.get(id).and_then(|owner| {
                let roles = host.chrome(id)?.in_state(owner.state);
                part_role(self.part, roles).map(|role| (role, self.strength))
            }),
            PaintSource::Gradient(_) | PaintSource::Region(_) => None,
        };
        let scope = self.scope.for_paint();
        let light = role.map_or(windows_color::Radiance::TRANSPARENT, |(role, strength)| {
            let light = crate::role::resolve(role, scope);
            light.with_alpha(light.a * strength)
        });
        let paint = match self.source {
            PaintSource::Gradient(id) => Paint::Ramp(id),
            PaintSource::Region(id) => Paint::Presented(id),
            _ => Paint::Solid(light),
        };
        host.model.paint(self.id, paint);
        let emission = role
            .filter(|_| self.part == Part::Label)
            .map_or(crate::role::Emission::NONE, |(role, _)| {
                crate::role::emission(role, scope)
            });
        host.model.halo(
            self.id,
            super::mount::halo_of(emission, Silhouette::Ink, light),
        );
        if let Some((halo, silhouette)) = self.halo {
            super::mount::emit_halo(host, halo, self.id, self.scope, silhouette);
        }
    }
}

fn part_role(part: Part, roles: RoleSet) -> Option<Role> {
    match part {
        Part::Fill => roles.fill.map(Role::Fill),
        Part::Border => roles.stroke.map(Role::Stroke),
        Part::Label => Some(Role::Text(roles.text)),
        _ => None,
    }
}

pub(super) fn chrome_parts(chrome: Option<Chrome>, selectable: bool) -> impl Iterator<Item = Part> {
    [Part::Border, Part::Fill].into_iter().filter(move |&part| {
        chrome.is_some_and(|chrome| {
            [
                Some(chrome.roles),
                selectable.then(|| chrome.in_state(ModelState::Selected)),
                chrome.disabled,
            ]
            .into_iter()
            .flatten()
            .any(|roles| part_role(part, roles).is_some())
        })
    })
}

fn chrome_mask(chrome: Chrome, part: Part, scope: Scope, selectable: bool) -> PaintMask {
    let radius = crate::role::metric(chrome.radius, scope);
    let width = crate::role::metric(Metric::HairlineW, scope);
    if part == Part::Border {
        PaintMask::Outline {
            radius: super::mount::surface_corners(radius, Some(chrome)),
            width,
            open: chrome.attached.map(|edge| match edge {
                Edge::Left => windows_scene::Side::Left,
                Edge::Top => windows_scene::Side::Top,
                Edge::Right => windows_scene::Side::Right,
                Edge::Bottom => windows_scene::Side::Bottom,
            }),
        }
    } else {
        let border = chrome_parts(Some(chrome), selectable).any(|part| part == Part::Border);
        PaintMask::Radius {
            dips: super::mount::surface_corners(
                (radius - if border { width } else { 0.0 }).max(0.0),
                Some(chrome),
            ),
        }
    }
}

/// Canonical appearance of an owning element. Paint resources are derived at publication.
#[derive(Copy, Clone)]
pub(crate) struct Surface {
    group: windows_scene::GroupId,
    chrome: Option<Chrome>,
    selectable: bool,
    wash: Wash,
    halo: Option<HaloStyle>,
    dirty: bool,
    /// The sprites of this surface's derived parts, in Border, Fill, Wash order.
    ///
    /// Slots and not a search: the derived set is closed, so the slot a part answered from
    /// last pass is where its sprite is, and a surface that keeps its parts never walks the
    /// owner's paint chain. Here rather than on the mount, because a node with no chrome has
    /// no derived part and no row in this table.
    parts: [Option<SpriteId>; 3],
}

impl Host {
    pub(crate) fn chrome(&self, id: ControlId) -> Option<Chrome> {
        self.surfaces.get(self.controls.get(id)?.node)?.chrome
    }

    fn surface(&mut self, group: windows_scene::GroupId) -> &mut Surface {
        let node = group.node();
        if self.surfaces.get(node).is_none() {
            self.surfaces.place(
                node,
                Surface {
                    group,
                    chrome: None,
                    selectable: false,
                    wash: Wash::Ink,
                    halo: None,
                    dirty: true,
                    parts: [None; 3],
                },
            );
        }
        self.surfaces_dirty = true;
        let surface = self.surfaces.get_mut(node).unwrap();
        surface.dirty = true;
        surface
    }

    pub(super) fn declare_surface(&mut self, group: windows_scene::GroupId, chrome: Chrome) {
        self.surface(group).chrome = Some(chrome);
    }

    pub(super) fn surface_selectable(&mut self, group: windows_scene::GroupId) {
        self.surface(group).selectable = true;
    }

    pub(super) fn surface_wash(&mut self, group: windows_scene::GroupId, wash: Wash) {
        self.surface(group).wash = wash;
    }

    pub(super) fn surface_halo(&mut self, group: windows_scene::GroupId, halo: HaloStyle) {
        self.surface(group).halo = Some(halo);
    }

    /// Resolve only changed owning records, after bindings and before layout/publication.
    ///
    /// The three derived parts are slots rather than a search: the set is closed, so a pass
    /// decides which of the three this surface owns now and finds each previous sprite where
    /// the last pass left it. A surface that keeps its parts touches its paint chain not at
    /// all.
    pub(crate) fn publish_surfaces(&mut self) {
        if !core::mem::take(&mut self.surfaces_dirty) {
            return;
        }
        for index in self.surfaces.positions() {
            let Some(node) = self.surfaces.id_at(index) else {
                continue;
            };
            let surface = self.surfaces.get_mut(node).unwrap();
            if !core::mem::take(&mut surface.dirty) {
                continue;
            }
            let surface = *surface;
            let scope = self.styles.get(node).unwrap().scope;
            let control = self.mounts.get(node).and_then(|m| m.control);
            let mut after = None;
            for (slot, part) in [Part::Border, Part::Fill, Part::Wash]
                .into_iter()
                .enumerate()
            {
                let wash = part == Part::Wash;
                let held = self.surfaces.get(node).unwrap().parts[slot];
                let Some(chrome) = surface.chrome.filter(|chrome| {
                    if wash {
                        control.is_some()
                    } else {
                        chrome_parts(Some(*chrome), surface.selectable).any(|p| p == part)
                    }
                }) else {
                    if let Some(id) = held {
                        self.drop_part(node, slot, id);
                    }
                    continue;
                };
                let id = match held {
                    Some(id) => id,
                    None => self.model.visual(surface.group, after),
                };
                after = Some(id.node());
                let role = if wash {
                    Some(match surface.wash {
                        Wash::Ink => Role::Text(crate::role::Text::Primary),
                        Wash::Accent => Role::Fill(crate::role::Fill::Accent),
                    })
                } else {
                    part_role(part, chrome.roles)
                };
                // A part with no role of its own resolves transparent; an owned one re-reads
                // its role from the owner's state at every publication, so the whole of what
                // that resolves to is what it paints.
                let (source, strength) = match control.filter(|_| !wash) {
                    Some(owner) => (PaintSource::Owner(owner), 1.0),
                    None => (
                        PaintSource::Role(
                            role.unwrap_or(Role::Text(crate::role::Text::Primary)),
                        ),
                        f32::from(role.is_some()),
                    ),
                };
                let paint = Appearance {
                    id,
                    mask: if wash {
                        PaintMask::Radius {
                            dips: super::mount::surface_corners(
                                crate::role::metric(chrome.radius, scope),
                                Some(chrome),
                            ),
                        }
                    } else {
                        chrome_mask(chrome, part, scope, surface.selectable)
                    },
                    source,
                    part,
                    strength,
                    geom: None,
                    scope,
                    surface: Some(node),
                    halo: if part == Part::Fill {
                        surface.halo.map(|halo| (halo, Silhouette::Area))
                    } else {
                        None
                    },
                    next: self
                        .appearances
                        .get(id.node())
                        .map_or(NodeId::NONE, |paint| paint.next),
                    wash,
                };
                paint.publish(self, true);
                self.appearances.place(id.node(), paint);
                if held.is_none() {
                    self.own_appearance(id, node);
                    self.surfaces.get_mut(node).unwrap().parts[slot] = Some(id);
                    if wash {
                        self.model.bind(
                            id.node(),
                            windows_scene::Prop::Opacity,
                            windows_scene::Bind::Set(windows_scene::Value::Scalar(0.0)),
                        );
                    }
                }
            }
            if let Some(id) = control.filter(|_| surface.chrome.is_some()) {
                let parts = self.surfaces.get(node).unwrap().parts;
                let row = self.controls.get_mut(id).unwrap();
                row.border = parts[0];
                row.fill = parts[1];
                row.front.wash = parts[2];
                row.dirty = true;
                self.repaint_control(id);
            }
            if let Some(halo) = surface.halo.filter(|_| surface.chrome.is_none()) {
                let mut at = self.mounts.get(node).unwrap().paints;
                while let Some(paint) = self.appearances.get_mut(at) {
                    at = paint.next;
                    if paint.part == Part::Fill {
                        paint.halo = Some((halo, Silhouette::Area));
                        let paint = *paint;
                        paint.publish(self, false);
                        break;
                    }
                }
            }
        }
    }

    /// Destroys a derived part this surface no longer owns, and unlinks it.
    ///
    /// The one walk of the paint chain left in this path, and it runs only where a chrome
    /// change took a part away.
    fn drop_part(&mut self, owner: NodeId, slot: usize, id: SpriteId) {
        let at = id.node();
        let mut previous = NodeId::NONE;
        let mut link = self.mounts.get(owner).unwrap().paints;
        while let Some(paint) = self.appearances.get(link) {
            let next = paint.next;
            if link == at {
                if previous.is_none() {
                    self.mounts.get_mut(owner).unwrap().paints = next;
                } else {
                    self.appearances.get_mut(previous).unwrap().next = next;
                }
                break;
            }
            previous = link;
            link = next;
        }
        self.appearances.take(at);
        self.styles.take(at);
        self.model.destroy(at, windows_scene::Exit::None);
        self.surfaces.get_mut(owner).unwrap().parts[slot] = None;
    }

    pub(crate) fn repaint_control(&mut self, id: ControlId) {
        if self.chrome(id).is_none() {
            return;
        }
        self.repaint_owned(self.controls.get(id).unwrap().node, id);
    }

    fn repaint_owned(&mut self, node: NodeId, owner: ControlId) {
        if let Some(paint) = self.appearances.get(node).copied()
            && matches!(paint.source, PaintSource::Owner(id) if id == owner)
        {
            paint.publish(self, false);
        }
        for index in 0..self.model.child_count(node) {
            self.repaint_owned(self.model.child(node, index), owner);
        }
    }

    pub(crate) fn publish_masks(&mut self) {
        for index in self.appearances.positions() {
            let Some(id) = self.appearances.id_at(index) else {
                continue;
            };
            let mut paint = *self.appearances.get(id).unwrap();
            let scope = paint.scope.at_width(self.model.solved(id).class);
            if scope == paint.scope {
                continue;
            }
            paint.resolve(self, scope);
            paint.place(self);
            if !matches!(paint.mask, PaintMask::Bare) {
                super::mount::emit_mask(self, paint.id, paint.mask, paint.geom, scope);
            }
            self.appearances.place(id, paint);
        }
    }

    pub(crate) fn own_appearance(&mut self, id: SpriteId, owner: NodeId) {
        let Some(paint) = self.appearances.get_mut(id.node()) else {
            return;
        };
        let mount = self
            .mounts
            .get_mut(owner)
            .expect("the paint's mount exists");
        paint.next = mount.paints;
        mount.paints = id.node();
    }

    /// Re-resolves this window's retained recipes, preserving node identity and lexical scope.
    /// The matching backdrop is handed to the scene in the same batch as these paint edits.
    pub fn set_theme(&mut self, root: Scope, backdrop: BackdropSpec) {
        let backdrop_changed = self.theme_backdrop.as_ref() != Some(&backdrop);
        if self.root_scope == root {
            if backdrop_changed {
                self.theme_backdrop = Some(backdrop.clone());
                self.theme_update = Some((root, backdrop));
            }
            return;
        }
        self.theme_backdrop = Some(backdrop.clone());
        self.root_scope = root;
        self.env = Env::new(
            self.env.dpi(),
            self.env
                .output()
                .with_content_peak_nits(crate::role::content_peak_nits(
                    &self.env.output().gamut(),
                    root,
                )),
        );
        for (_, row) in self.regions.iter_mut() {
            if row.build.is_some() {
                row.theme.set(row.theme.get().in_theme(root));
            }
        }
        for position in self.styles.positions() {
            let Some(id) = self.styles.id_at(position) else {
                continue;
            };
            let recipe = self.styles.get_mut(id).unwrap();
            recipe.scope = recipe.scope.in_theme(root);
            let class = self.model.solved(id).class;
            self.mark_style(id, class);
        }
        // Copy each small recipe outside the table borrow; no callback or per-frame work.
        for index in self.appearances.positions() {
            let Some(id) = self.appearances.id_at(index) else {
                continue;
            };
            let mut paint = *self.appearances.get(id).unwrap();
            paint.resolve(self, paint.scope.in_theme(root));
            paint.publish(self, true);
            self.appearances.place(id, paint);
        }
        for (id, (stops, spread)) in self.ramps.iter() {
            super::mount::resolve_stops(stops, root, &mut self.ramp_stops);
            self.model.set_ramp(id, &self.ramp_stops, *spread);
        }
        self.text.retheme(root, &mut self.model);
        self.retheme_fields(root);
        for (_, control) in self.controls.iter_mut() {
            control.scope = control.scope.in_theme(root);
        }
        self.theme_update = Some((root, backdrop));
    }
}

impl super::Element<'_, super::Path> {
    /// Paints this shape at `strength` of the alpha its role resolves to.
    ///
    /// Folded into the colour at publication, so a shape drawn faintly costs no compositor
    /// channel and leaves `Prop::Opacity` for a reveal to own.
    ///
    /// # Panics
    ///
    /// Panics where this shape has stated no paint: a strength scales a role, and there is
    /// none to scale before one is named.
    pub fn strength(self, strength: f32) -> Self {
        let paint = self
            .host
            .appearances
            .get_mut(self.node.target.id())
            .expect("a strength scales a paint this shape has already stated");
        paint.strength = strength;
        let paint = *paint;
        paint.publish(self.host, false);
        self
    }
}
