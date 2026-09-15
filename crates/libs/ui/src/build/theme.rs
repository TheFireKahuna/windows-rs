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
                let role = part_role(self.part, roles);
                role.map(|role| (role, 1.0))
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
            let mut previous = NodeId::NONE;
            let mut at = self.mounts.get(node).unwrap().paints;
            let mut held = [None; 3];
            while let Some(paint) = self.appearances.get(at).copied() {
                let next = paint.next;
                let slot = match paint.part {
                    Part::Border => Some(0),
                    Part::Fill => Some(1),
                    Part::Wash => Some(2),
                    _ => None,
                };
                if paint.surface == Some(node)
                    && let Some(slot) = slot
                {
                    let needed = if slot == 2 {
                        control.is_some() && surface.chrome.is_some()
                    } else {
                        chrome_parts(surface.chrome, surface.selectable)
                            .any(|part| part == paint.part)
                    };
                    if needed {
                        held[slot] = Some(paint.id);
                    } else {
                        if previous.is_none() {
                            self.mounts.get_mut(node).unwrap().paints = next;
                        } else {
                            self.appearances.get_mut(previous).unwrap().next = next;
                        }
                        self.appearances.take(at);
                        self.styles.take(at);
                        self.model.destroy(at, windows_scene::Exit::None);
                        at = next;
                        continue;
                    }
                }
                previous = at;
                at = next;
            }
            let mut after = None;
            for (slot, part) in [Part::Border, Part::Fill, Part::Wash]
                .into_iter()
                .enumerate()
            {
                let Some(chrome) = surface.chrome else { break };
                let wash = part == Part::Wash;
                if if wash {
                    control.is_none()
                } else {
                    !chrome_parts(Some(chrome), surface.selectable).any(|p| p == part)
                } {
                    continue;
                }
                let existing = held[slot];
                let id = existing.unwrap_or_else(|| self.model.visual(surface.group, after));
                after = Some(id.node());
                held[slot] = Some(id);
                let role = if wash {
                    Some(match surface.wash {
                        Wash::Ink => Role::Text(crate::role::Text::Primary),
                        Wash::Accent => Role::Fill(crate::role::Fill::Accent),
                    })
                } else {
                    part_role(part, chrome.roles)
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
                    source: if wash {
                        PaintSource::Role(role.unwrap())
                    } else {
                        control.map_or(
                            PaintSource::Role(
                                role.unwrap_or(Role::Text(crate::role::Text::Primary)),
                            ),
                            PaintSource::Owner,
                        )
                    },
                    part,
                    strength: f32::from(role.is_some()),
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
                if existing.is_none() {
                    self.own_appearance(id, node);
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
                let row = self.controls.get_mut(id).unwrap();
                row.border = held[0];
                row.fill = held[1];
                row.front.wash = held[2];
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
            let style = self.styles.lower(id, self.model.solved(id).class).unwrap();
            self.model.style(id, &style);
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
        for (_, scroll) in self.scrolls.iter() {
            if let Some(thumb) = scroll.thumb {
                let scope = scroll
                    .grab
                    .and_then(|id| self.controls.get(id))
                    .map_or(root, |c| c.scope);
                self.model.paint(
                    thumb,
                    Paint::Solid(crate::role::ink(
                        super::mount::THUMB_ALPHA,
                        scope.for_paint(),
                    )),
                );
            }
        }
        self.theme_update = Some((root, backdrop));
    }
}
