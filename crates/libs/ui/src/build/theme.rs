//! Retained paint recipes and one window-owned theme transaction.
use super::{
    Host,
    arena::{HaloSeed, MaskSeed, Part},
};
use crate::role::{Role, Scope, Silhouette};
use crate::widget::Chrome;
use windows_scene::{
    BackdropSpec, ControlId, Env, GeomId, NodeId, Paint, RampId, RegionId, SpriteId,
};

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
    pub mask: MaskSeed,
    pub source: PaintSource,
    pub part: Part,
    pub strength: f32,
    pub geom: Option<GeomId>,
    pub scope: Scope,
    pub chrome: Option<Chrome>,
    pub halo: Option<(HaloSeed, Silhouette)>,
    pub next: NodeId,
    pub wash: bool,
}

impl Appearance {
    fn resolve(&mut self, scope: Scope) {
        self.scope = scope;
        let Some(chrome) = self.chrome else { return };
        if self.wash {
            self.mask = MaskSeed::Radius {
                dips: super::mount::surface_corners(
                    crate::role::metric(chrome.radius, scope),
                    Some(chrome),
                ),
            };
        } else if let Some((_, seed)) =
            super::mount::chrome_seeds(Some(chrome.roles), Some(chrome), scope, true)
                .find(|(part, _)| *part == self.part)
        {
            self.mask = seed.mask;
        }
    }

    /// The same resolver publishes initial appearance, theme changes and owner-state changes.
    pub(super) fn publish(&self, host: &mut Host, mask: bool) {
        if mask {
            super::mount::emit_mask(host, self.id, self.mask, self.geom, self.scope);
        }
        let role = match self.source {
            PaintSource::Role(role) => Some((role, self.strength)),
            PaintSource::Owner(id) => host.controls.get(id).and_then(|owner| {
                let roles = owner.chrome?.in_state(owner.state);
                let role = match self.part {
                    Part::Fill => roles.fill.map(Role::Fill),
                    Part::Border => roles.stroke.map(Role::Stroke),
                    Part::Label => Some(Role::Text(roles.text)),
                    _ => None,
                };
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

impl Host {
    pub(crate) fn repaint_control(&mut self, id: ControlId) {
        let Some(control) = self.controls.get(id).filter(|c| c.chrome.is_some()) else {
            return;
        };
        let parts = [control.fill, control.label, control.border];
        for id in parts.into_iter().flatten() {
            if let Some(part) = self.appearances.get(id.node()).copied()
                && matches!(part.source, PaintSource::Owner(_))
            {
                part.publish(self, false);
            }
        }
    }

    pub(crate) fn claim_control_paint(&mut self, id: ControlId) {
        let Some(control) = self.controls.get(id).filter(|c| c.chrome.is_some()) else {
            return;
        };
        for sprite in [control.fill, control.label, control.border]
            .into_iter()
            .flatten()
        {
            if let Some(part) = self.appearances.get_mut(sprite.node())
                && matches!(part.source, PaintSource::Role(_))
            {
                part.source = PaintSource::Owner(id);
            }
        }
        if let Some(label) = self.controls.get(id).and_then(|c| c.label)
            && let Some(part) = self.appearances.get(label.node()).copied()
            && matches!(part.source, PaintSource::Owner(_))
        {
            part.publish(self, false);
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
            paint.resolve(scope);
            if !matches!(paint.mask, super::arena::MaskSeed::Run { .. }) {
                super::mount::emit_mask(self, paint.id, paint.mask, paint.geom, scope);
            }
            self.appearances.place(id, paint);
        }
    }

    pub(crate) fn own_appearance(&mut self, id: SpriteId, owner: NodeId, chrome: Option<Chrome>) {
        let Some(paint) = self.appearances.get_mut(id.node()) else {
            return;
        };
        let mount = self
            .mounts
            .get_mut(owner)
            .expect("the paint's mount exists");
        paint.next = mount.paints;
        paint.chrome = chrome;
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
        for (id, recipe) in self.styles.iter_mut() {
            recipe.scope = recipe.scope.in_theme(root);
            let style = recipe.lower(self.model.solved(id).class);
            self.model.style(id, &style);
        }
        // Copy each small recipe outside the table borrow; no callback or per-frame work.
        for index in self.appearances.positions() {
            let Some(id) = self.appearances.id_at(index) else {
                continue;
            };
            let mut paint = *self.appearances.get(id).unwrap();
            paint.resolve(paint.scope.in_theme(root));
            paint.publish(self, true);
            self.appearances.place(id, paint);
        }
        for (id, (stops, spread)) in self.ramps.iter() {
            let resolved: Vec<_> = stops
                .iter()
                .map(|stop| {
                    let light = crate::role::data(stop.role, root);
                    (stop.at, light.with_alpha(light.a * stop.strength))
                })
                .collect();
            self.model.set_ramp(id, &resolved, *spread);
        }
        self.text.retheme(root, &mut self.model);
        self.retheme_fields(root);
        for (_, probe) in self.probes.iter_mut() {
            probe.scope = probe.scope.in_theme(root);
        }
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
                    windows_scene::Paint::Solid(crate::role::ink(
                        super::mount::THUMB_ALPHA,
                        scope.for_paint(),
                    )),
                );
            }
        }
        self.theme_update = Some((root, backdrop));
    }
}
