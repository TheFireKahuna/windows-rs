//! Retained paint recipes and one window-owned theme transaction.
use super::{
    Host,
    arena::{HaloSeed, SpriteSeed},
    host::MountId,
};
use crate::role::Scope;
use crate::widget::{Chrome, RoleSet};
use windows_scene::{BackdropSpec, Env, GeomId, NodeId, SpriteId};

#[derive(Clone)]
pub(crate) struct Appearance {
    pub id: SpriteId,
    pub seed: SpriteSeed,
    pub geom: Option<GeomId>,
    pub scope: Scope,
    pub roles: Option<RoleSet>,
    pub chrome: Option<Chrome>,
    pub halo: Option<(HaloSeed, crate::role::Silhouette)>,
    pub next: NodeId,
    pub wash: bool,
}

impl Appearance {
    fn resolve(&mut self, scope: Scope) {
        self.scope = scope;
        let Some(chrome) = self.chrome else { return };
        if self.wash {
            self.seed.mask = super::arena::MaskSeed::Radius {
                dips: super::mount::surface_corners(
                    crate::role::metric(chrome.radius, scope),
                    Some(chrome),
                ),
            };
        } else if let Some((_, seed)) =
            super::mount::chrome_seeds(self.roles, Some(chrome), scope, true)
                .find(|(part, _)| *part == self.seed.part)
        {
            self.seed = seed;
        }
    }
}

impl Host {
    pub(crate) fn publish_masks(&mut self) {
        for index in self.appearances.positions() {
            let Some(id) = self.appearances.id_at(index) else {
                continue;
            };
            let mut paint = self.appearances.get(id).unwrap().clone();
            let scope = paint.scope.at_width(self.model.solved(id).class);
            if scope == paint.scope {
                continue;
            }
            paint.resolve(scope);
            if !matches!(paint.seed.mask, super::arena::MaskSeed::Run { .. }) {
                super::mount::emit_mask(self, paint.id, &paint.seed, paint.geom, scope);
            }
            self.appearances.place(id, paint);
        }
    }

    pub(crate) fn own_appearance(&mut self, id: SpriteId, owner: MountId, chrome: Option<Chrome>) {
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
        super::style::with(|table| {
            for (id, recipe) in table.iter_mut() {
                recipe.scope = recipe.scope.in_theme(root);
                let style = recipe.lower(self.model.solved(id).class);
                self.model.style(id, &style);
            }
        });
        // Copy each small recipe outside the table borrow; no callback or per-frame work.
        for index in self.appearances.positions() {
            let Some(id) = self.appearances.id_at(index) else {
                continue;
            };
            let mut paint = self.appearances.get(id).unwrap().clone();
            paint.resolve(paint.scope.in_theme(root));
            super::mount::emit_sprite_on(
                self,
                paint.id,
                &paint.seed,
                paint.geom,
                paint.scope,
                paint.roles,
            );
            if let Some((halo, silhouette)) = paint.halo {
                super::mount::emit_halo(self, halo, paint.id, paint.scope, silhouette);
            }
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
        super::text::with(|table| table.retheme(root, &mut self.model));
        self.retheme_fields(root);
        for (_, probe) in self.probes.iter_mut() {
            probe.scope = probe.scope.in_theme(root);
        }
        for (_, control) in self.controls.iter_mut() {
            control.scope = control.scope.in_theme(root);
            control.repaint(&mut self.model);
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
