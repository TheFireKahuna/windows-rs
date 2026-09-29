//! Retained paint recipes and one window-owned theme transaction.

use super::host::Host;
use super::tree;
use super::ui::{Element, Ui};
use crate::layout::{Edge, Len, Preset, WidthClass};
use crate::role::{
    DataRole, Elevation, Emission, Fill, Metric, Role, Scope, Silhouette, Stroke, Text,
    content_peak_nits, emission, metric, resolve, shadow,
};
use crate::signal::Signal;
use crate::widget::{Chrome, ModelState, RoleSet, Wash};
use crate::widget::roles::{FOCUS_OUTSET, FOCUS_STROKE};
use windows_color::Radiance;
use windows_numerics::Vector2;
use windows_scene::{
    BackdropSpec, Cap, ControlId, Corners, Env, Exit, GeomId, GroupId, Halo, Join, Mask, NodeId,
    Paint, PathSpace, Prop, RampId, RegionId, Side, SinkPatch, SpriteId, Value,
};

#[derive(Copy, Clone, Debug)]
pub(crate) enum HaloStyle {
    Glow(Role),
    Shadow(Edge, Option<&'static crate::role::ScopedToken<crate::role::Shadow>>),
}

/// What a sprite is to the surface that owns it.
///
/// `Border`, `Fill` and `Wash` are the closed derived set: a surface decides which of the
/// three it owns now and holds each one in a slot. `Ink` is a sprite that paints a role
/// directly — a path, a plate, a glyph tile — and is lit where that role resolves rather than
/// declaring a halo of its own.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Part {
    Border,
    Fill,
    Wash,
    Ink,
}

impl Part {
    /// The derived set, in the order a surface's slots hold them and the order they paint in.
    const DERIVED: [Self; 3] = [Self::Border, Self::Fill, Self::Wash];

    fn role(self, roles: RoleSet, wash: Wash) -> Option<Role> {
        match self {
            Self::Border => roles.stroke.map(Role::Stroke),
            Self::Fill => roles.fill.map(Role::Fill),
            Self::Wash => wash_role(wash),
            Self::Ink => Some(Role::Text(roles.text)),
        }
    }
}

/// The role a wash paints in before its opacity scales it.
///
/// The wash is the interaction light over a control's own base, so it is the foreground ink
/// or the accent and never a third colour the palette does not author.
const fn wash_role(wash: Wash) -> Option<Role> {
    match wash {
        Wash::None => None,
        Wash::Ink => Some(Role::Text(Text::Primary)),
        Wash::Accent => Some(Role::Fill(Fill::Accent)),
        Wash::AccentBorder => Some(Role::Stroke(Stroke::Focus)),
    }
}

/// The silhouette a part paints, as the recipe stated it. Resolved against a scope and a
/// solved box at publication; nothing here holds a resolved number.
#[derive(Copy, Clone, Debug)]
pub(crate) enum PaintMask {
    /// A filled box. `radius` is authored, and the resolved value is capped at half the
    /// shorter side of the solved box: `CompositionRoundedRectangleGeometry` saturates there
    /// anyway, so a pill authored at half a row height renders as a stadium of the wrong axis
    /// on any box narrower than it is tall unless the cap is applied before the write.
    Box {
        radius: Len,
    },
    InsetBox {
        radius: Len,
        inset: Len,
    },
    Outline {
        radius: Len,
        width: Len,
    },
    OuterOutline {
        radius: Len,
        width: Len,
    },
    Shape {
        geom: GeomId,
        stroke: Option<Len>,
        space: PathSpace,
    },
    /// A square box a presented region paints its own buffer over.
    Region,
}

impl PaintMask {
    /// Whether the resolved silhouette depends on the box it is drawn into, which is what
    /// decides whether a solved extent owes it a re-emission.
    const fn box_bound(self) -> bool {
        matches!(self, Self::Box { .. } | Self::InsetBox { .. } | Self::Outline { .. } | Self::OuterOutline { .. })
    }
}

/// A part has one paint source. Owner state never overwrites a gradient or region.
#[derive(Copy, Clone, Debug)]
pub(crate) enum PaintSource {
    Role(Role),
    /// Re-read from the owner's model state at every publication, so the whole of what that
    /// resolves to is what this paints.
    Owner(ControlId),
    Gradient(RampId),
    Region(RegionId),
    RegionView(RegionId, u32),
    None,
}

impl PaintSource {
    pub(crate) const fn data(role: DataRole) -> Self {
        Self::Role(Role::Data(role))
    }

    pub(crate) const fn stroke(role: Stroke) -> Self {
        Self::Role(Role::Stroke(role))
    }

    const fn owner(self) -> ControlId {
        match self {
            Self::Owner(id) => id,
            _ => ControlId::NONE,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct Appearance {
    pub id: SpriteId,
    pub source: PaintSource,
    pub mask: PaintMask,
    pub part: Part,
    /// The fraction of its role's alpha this shape paints at. Folded into the colour at
    /// publication, so a shape drawn faintly costs no compositor channel and leaves
    /// `Prop::Opacity` for a reveal to own.
    pub strength: f32,
    pub scope: Scope,
    /// The surface whose chrome states this part's silhouette, where it is a derived one.
    pub surface: u32,
    pub halo: Option<HaloStyle>,
    /// Half the shorter side of the box the mask was last resolved against. `NaN` where the
    /// silhouette does not depend on the box.
    pub cap: f32,
    scale: f32,
    /// The next paint in this mount's chain. Intrusive, because a mount owns an unbounded
    /// number of non-derived paints and a `Vec` per mount would allocate for the many nodes
    /// that own one.
    pub next: NodeId,
}

/// Canonical appearance of an owning element. Paint resources are derived at publication.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Surface {
    node: NodeId,
    chrome: Option<Chrome>,
    wash: Wash,
    halo: Option<HaloStyle>,
    selectable: bool,
    dirty: bool,
    /// The sprites of this surface's derived parts, in Border, Fill, Wash order.
    ///
    /// Slots and not a search: the derived set is closed, so the slot a part answered from
    /// last pass is where its sprite is, and a surface that keeps its parts never walks the
    /// owner's paint chain.
    parts: [Option<SpriteId>; 3],
}

impl Surface {
    fn new(node: NodeId) -> Self {
        Self {
            node,
            chrome: None,
            wash: Wash::Ink,
            halo: None,
            selectable: false,
            dirty: true,
            parts: [None; 3],
        }
    }

    /// Returns the roles this surface paints in `state`, or `None` where it has no chrome.
    fn roles(&self, state: ModelState) -> Option<RoleSet> {
        let chrome = self.chrome?;
        Some(match state {
            ModelState::Selected if !self.selectable => chrome.in_state(ModelState::Rest),
            state => chrome.in_state(state),
        })
    }

    /// Whether this surface owns `part` in any state it can show.
    ///
    /// Any state and not the current one: a button that takes a border only while disabled
    /// still owns the sprite that border paints into, so entering that state retargets a
    /// sprite rather than minting one on the interaction path.
    fn owns(&self, part: Part) -> bool {
        [ModelState::Rest, ModelState::Selected, ModelState::Disabled]
            .into_iter()
            .filter_map(|state| self.roles(state))
            .any(|roles| part.role(roles, self.wash).is_some())
    }

    /// Returns `part`'s silhouette, authored.
    ///
    /// The fill sits a hairline inside a border that exists, so the two do not alias along
    /// the corner arc.
    fn mask_of(&self, part: Part) -> PaintMask {
        let radius = self
            .chrome
            .map_or(Len::ZERO, |chrome| Len::from(chrome.radius));
        let width = self.chrome.and_then(|chrome| chrome.border_width)
            .unwrap_or_else(|| Len::from(Metric::HairlineW));
        match part {
            Part::Wash if self.wash == Wash::AccentBorder => PaintMask::Outline { radius, width },
            Part::Border => PaintMask::Outline {
                radius,
                width,
            },
            Part::Fill if self.owns(Part::Border) => PaintMask::InsetBox {
                radius,
                inset: width,
            },
            _ => PaintMask::Box { radius },
        }
    }
}

/// Every retained paint, and the surfaces whose chrome derives some of them.
///
/// One store, because a surface's parts *are* paints: retirement reads one head for the
/// chain and one for the surface, and neither can be freed without the other's table.
#[derive(Default)]
pub(crate) struct Appearances {
    scale: f32,
    paints: Vec<Option<(u32, Appearance)>>,
    /// The retained geometry of a node whose silhouette is a path, so a paint setter states
    /// the stroke without restating the shape.
    shapes: Vec<Option<(u32, GeomId)>>,
    surfaces: Vec<Option<Surface>>,
    free: Vec<u32>,
    /// Paints written since the masks pass last read them: a new paint, a new mask, a new
    /// scope. What that pass visits besides the sprites whose box moved.
    restated: Vec<NodeId>,
    /// Surface rows marked for the next resolve, so that pass visits them and not every row.
    marked: Vec<u32>,
}

impl Appearances {
    /// How many paints and surfaces are placed. What a fixture asks whether a retirement
    /// freed, since a node that paints nothing occupies no row here.
    #[cfg(test)]
    pub(crate) fn placed(&self) -> usize {
        self.paints.iter().flatten().count()
            + self.shapes.iter().flatten().count()
            + self.surfaces.iter().flatten().count()
    }

    pub(crate) fn place(&mut self, id: NodeId, paint: Appearance) {
        if self.paints.len() <= id.index() {
            self.paints.resize_with(id.index() + 1, || None);
        }
        self.paints[id.index()] = Some((id.generation(), paint));
        self.restated.push(id);
    }

    pub(crate) fn get(&self, id: NodeId) -> Option<&Appearance> {
        match self.paints.get(id.index()) {
            Some(Some((age, paint))) if *age == id.generation() => Some(paint),
            _ => None,
        }
    }

    /// A writable paint, which the masks pass then revisits: any field of one may be an
    /// input to its mask.
    pub(crate) fn get_mut(&mut self, id: NodeId) -> Option<&mut Appearance> {
        match self.paints.get_mut(id.index()) {
            Some(Some((age, paint))) if *age == id.generation() => {
                self.restated.push(id);
                Some(paint)
            }
            _ => None,
        }
    }

    /// Writes back a paint the masks pass resolved, without naming it for that pass again.
    fn settle(&mut self, id: NodeId, paint: Appearance) {
        if let Some(Some((age, held))) = self.paints.get_mut(id.index())
            && *age == id.generation()
        {
            *held = paint;
        }
    }

    fn take(&mut self, id: NodeId) -> Option<Appearance> {
        let slot = self.paints.get_mut(id.index())?;
        match slot {
            Some((age, _)) if *age == id.generation() => slot.take().map(|(_, paint)| paint),
            _ => None,
        }
    }

    /// Frees the paint at the head of a mount's chain and answers the next link.
    ///
    /// The sprite itself is a tree node the destroy cascades over, so only the row is freed
    /// here; `patch` carries the release of whatever resource the row alone was holding.
    pub(crate) fn release(&mut self, head: NodeId, patch: &mut SinkPatch) -> NodeId {
        let Some(paint) = self.take(head) else {
            return NodeId::NONE;
        };
        let _ = patch;
        paint.next
    }

    /// Frees a surface row and forgets the derived parts it held.
    ///
    /// The parts are sprites under the surface's own node, so the destroy that retires that
    /// node takes them; what is freed here is the row that named them.
    pub(crate) fn release_surface(&mut self, at: u32, patch: &mut SinkPatch) {
        let _ = patch;
        if self
            .surfaces
            .get_mut(at as usize)
            .and_then(Option::take)
            .is_some()
        {
            self.free.push(at);
        }
    }

    /// The sprite a node's declared halo hangs on, or [`NodeId::NONE`].
    ///
    /// A halo's opacity and sigma belong to the sprite casting it rather than to the node that
    /// declared it, and a node's paints are a chain, so this is where the two are joined.
    pub(crate) fn halo_bearer(&self, head: NodeId) -> NodeId {
        let mut at = head;
        while let Some(paint) = self.get(at) {
            if paint.halo.is_some() {
                return paint.id.0;
            }
            at = paint.next;
        }
        NodeId::NONE
    }

    /// How many paint slots a walk visits, vacated ones included.
    fn slots(&self) -> usize {
        self.paints.len()
    }

    /// The node occupying one paint slot, or `None` where it is vacant.
    fn id_at(&self, at: usize) -> Option<NodeId> {
        let (age, _) = self.paints.get(at)?.as_ref()?;
        Some(NodeId::raw(at as u32, *age))
    }

    /// Records the retained geometry a path node draws from.
    pub(crate) fn set_shape(&mut self, id: NodeId, geom: GeomId) {
        if self.shapes.len() <= id.index() {
            self.shapes.resize_with(id.index() + 1, || None);
        }
        self.shapes[id.index()] = Some((id.generation(), geom));
    }

    fn shape(&self, id: NodeId) -> Option<GeomId> {
        match self.shapes.get(id.index()) {
            Some(Some((age, geom))) if *age == id.generation() => Some(*geom),
            _ => None,
        }
    }

    /// How many surface rows a walk visits, vacated ones included.
    fn surface_rows(&self) -> u32 {
        self.surfaces.len() as u32
    }

    fn place_surface(&mut self, surface: Surface) -> u32 {
        match self.free.pop() {
            Some(at) => {
                self.surfaces[at as usize] = Some(surface);
                at
            }
            None => {
                self.surfaces.push(Some(surface));
                self.surfaces.len() as u32 - 1
            }
        }
    }

    fn surface(&self, at: u32) -> Option<&Surface> {
        self.surfaces.get(at as usize)?.as_ref()
    }

    fn surface_mut(&mut self, at: u32) -> Option<&mut Surface> {
        self.surfaces.get_mut(at as usize)?.as_mut()
    }
}

// The theme transaction the next fill hands to the scene thread. Held beside the host rather
// than on it because `set_theme` runs between flushes and the batch it belongs to is built
// later; the app thread owns both, as it owns the host.
thread_local! {
    static PENDING_THEME: core::cell::RefCell<Option<(Scope, BackdropSpec)>> =
        const { core::cell::RefCell::new(None) };
}

/// Takes the theme transaction the next batch carries, if one is owed.
pub(crate) fn take_theme() -> Option<(Scope, BackdropSpec)> {
    PENDING_THEME.with_borrow_mut(Option::take)
}

static FOCUS_RADIUS: crate::role::ScopedToken<f32> =
    crate::role::ScopedToken::new("focus radius", |scope| metric(Metric::Radius, scope) + FOCUS_OUTSET);

impl Host {
    /// Retains the window's focus outline outside layout and hit testing.
    pub(crate) fn focus_outline(&mut self) -> NodeId {
        let id = self.tree.mint(0);
        self.tree.c.flags[id.index()] |= tree::SPRITE | tree::DERIVED;
        self.pending.push(windows_scene::Op::New {
            id,
            kind: windows_scene::NodeKind::Sprite,
            parent: windows_scene::Attach::Overlay,
            after: None,
        });
        self.declare_part(
            id,
            Part::Ink,
            PaintSource::Role(Role::Stroke(Stroke::Focus)),
            PaintMask::Outline {
                radius: Metric::Custom(&FOCUS_RADIUS).into(),
                width: Len::dip(FOCUS_STROKE),
            },
            1.0,
        );
        self.write_channel(id, Prop::Opacity, Value::Scalar(0.0));
        id
    }

    pub(crate) fn chrome(&self, id: ControlId) -> Option<Chrome> {
        let node = self.control(id)?.node;
        self.appearances.surface(self.surface_row(node))?.chrome
    }

    /// Returns this node's surface row, minting an empty one, and marks it for the next
    /// resolve.
    fn surface_mut(&mut self, node: NodeId) -> Option<&mut Surface> {
        let mut at = self.surface_row(node);
        if at == tree::NONE {
            at = self.appearances.place_surface(Surface::new(node));
            self.set_surface_row(node, at);
        }
        self.appearances.marked.push(at);
        let surface = self.appearances.surface_mut(at)?;
        surface.dirty = true;
        Some(surface)
    }

    pub(crate) fn declare_chrome(&mut self, node: NodeId, chrome: Chrome) {
        if let Some(surface) = self.surface_mut(node) {
            surface.chrome = Some(chrome);
        }
    }

    pub(super) fn surface_selectable(&mut self, group: GroupId) {
        if let Some(surface) = self.surface_mut(group.0) {
            surface.selectable = true;
        }
    }

    pub(super) fn surface_wash(&mut self, group: GroupId, wash: Wash) {
        if let Some(surface) = self.surface_mut(group.0) {
            surface.wash = wash;
        }
    }

    pub(super) fn surface_halo(&mut self, group: GroupId, halo: HaloStyle) {
        if let Some(surface) = self.surface_mut(group.0) {
            surface.halo = Some(halo);
        }
    }

    /// Pushes `elevation` onto this node's scope, so every paint already hanging on it
    /// re-resolves against the new rung.
    pub(crate) fn elevate(&mut self, node: NodeId, elevation: Elevation) -> u32 {
        let scope = self.scope_of(node).elevate(elevation);
        let at = self.intern(scope);
        self.tree.c.scope[node.index()] = at;
        // Everything that resolves through the node's scope rather than its box.
        self.tree.moved.push(node);
        let mut link = self.tree.c.paints[node.index()];
        while let Some(paint) = self.appearances.get_mut(link) {
            paint.scope = scope;
            let paint = *paint;
            link = paint.next;
            self.publish_paint(paint, true);
        }
        at
    }

    /// Declares one part of one node with one exclusive paint source.
    ///
    /// What every appearance setter on `Element` calls. A group takes a derived sprite of its
    /// own, since a group carries no mask and no paint; a sprite paints itself.
    pub(crate) fn declare_part(
        &mut self,
        node: NodeId,
        part: Part,
        source: PaintSource,
        mask: PaintMask,
        strength: f32,
    ) -> SpriteId {
        let id = match self.tree.c.flags[node.index()] & tree::SPRITE != 0 {
            true => SpriteId(node),
            // A group paints through a derived sprite spanning its own box.
            false => {
                let id = self.chrome_visual(GroupId(node), None);
                self.visual_insets(id, [0.0; 4]);
                id
            }
        };
        if let PaintMask::OuterOutline { width, .. } = mask {
            self.visual_outset(id, width);
        }
        let held = self.appearances.get(id.0).copied();
        let paint = Appearance {
            id,
            source,
            mask,
            part,
            strength,
            scope: self.scope_of(node),
            surface: tree::NONE,
            halo: held.and_then(|held| held.halo).or_else(|| {
                self.appearances.surface(self.surface_row(node))
                    .and_then(|surface| surface.halo).filter(|_| part == Part::Fill)
            }),
            cap: f32::NAN,
            scale: self.env.scale(),
            next: held.map_or(NodeId::NONE, |held| held.next),
        };
        self.publish_paint(paint, true);
        self.appearances.place(id.0, paint);
        if held.is_none() {
            self.own_appearance(id, node);
        }
        id
    }

    /// Resolves only changed owning records, after bindings and before layout and
    /// publication.
    ///
    /// The same resolver publishes initial appearance, theme changes and owner-state changes.
    /// It updates existing parts and creates only capabilities the recipe declared: there are
    /// no construction seeds and no second interpreter.
    ///
    /// The three derived parts are slots rather than a search: the set is closed, so a pass
    /// decides which of the three this surface owns now and finds each previous sprite where
    /// the last pass left it. A surface that keeps its parts touches its paint chain not at
    /// all.
    ///
    /// Visits the rows marked since the last pass, and every row where the scale moved or a
    /// sweep is owed. A resolve that marks another row has it resolved in this pass too.
    pub(crate) fn publish_surfaces(&mut self) {
        let rescaled = self.appearances.scale != self.env.scale();
        if rescaled {
            self.appearances.scale = self.env.scale();
            for surface in self.appearances.surfaces.iter_mut().flatten() {
                surface.dirty = true;
            }
        }
        if rescaled || self.changes.sweeping() {
            self.appearances.marked.clear();
            self.appearances.marked.extend(0..self.appearances.surface_rows());
        }
        let mut marked = Vec::new();
        while !self.appearances.marked.is_empty() {
            core::mem::swap(&mut marked, &mut self.appearances.marked);
            for &at in &marked {
                self.publish_surface_at(at);
            }
            marked.clear();
        }
        self.appearances.marked = marked;
        #[cfg(debug_assertions)]
        for surface in self.appearances.surfaces.iter().flatten() {
            debug_assert!(!surface.dirty, "a marked surface was not resolved: {:?}", surface.node);
        }
    }

    /// Resolves one surface row where it is still marked.
    fn publish_surface_at(&mut self, at: u32) {
        self.changes.visit();
        let Some(surface) = self.appearances.surface_mut(at) else {
            return;
        };
        if !core::mem::take(&mut surface.dirty) {
            return;
        }
        let surface = *surface;
        self.publish_surface(at, surface);
    }

    fn publish_surface(&mut self, row: u32, surface: Surface) {
        let node = surface.node;
        let scope = self.scope_of(node);
        let owner = self.control_of(node);
        let owner = (!owner.is_none()).then_some(owner);
        let mut after = None;
        for (slot, part) in Part::DERIVED.into_iter().enumerate() {
            let held = self
                .appearances
                .surface(row)
                .and_then(|held| held.parts[slot]);
            // A wash is minted where a surface's chrome meets a control, and nowhere else:
            // most of a screen declares no chrome, owns no control and pays nothing.
            let owned = surface.chrome.is_some()
                && match part {
                    Part::Wash => owner.is_some() && surface.wash != Wash::None,
                    part => surface.owns(part),
                };
            let Some(id) = self.claim_part(row, node, slot, held, owned, after) else {
                continue;
            };
            after = Some(id.0);
            let source = match owner.filter(|_| part != Part::Wash) {
                Some(owner) => PaintSource::Owner(owner),
                None => match surface
                    .roles(ModelState::Rest)
                    .and_then(|roles| part.role(roles, surface.wash))
                {
                    Some(role) => PaintSource::Role(role),
                    None => continue,
                },
            };
            let paint = Appearance {
                id,
                source,
                mask: surface.mask_of(part),
                part,
                strength: 1.0,
                scope,
                surface: row,
                halo: surface.halo.filter(|_| part == Part::Fill),
                cap: f32::NAN,
                scale: self.env.scale(),
                next: self
                    .appearances
                    .get(id.0)
                    .map_or(NodeId::NONE, |held| held.next),
            };
            self.place_part(paint, surface);
            self.publish_paint(paint, true);
            self.appearances.place(id.0, paint);
            if held.is_none() {
                self.own_appearance(id, node);
                if let Some(held) = self.appearances.surface_mut(row) {
                    held.parts[slot] = Some(id);
                }
                // The wash is parked at opacity zero at creation; hover and press retarget it
                // scene-side, in the tick that saw the event.
                if part == Part::Wash {
                    self.write_channel(id.0, Prop::Opacity, Value::Scalar(0.0));
                }
            }
        }
        let Some(id) = owner.filter(|_| surface.chrome.is_some()) else {
            return;
        };
        let wash = self
            .appearances
            .surface(row)
            .and_then(|held| held.parts[2])
            .unwrap_or(SpriteId(NodeId::NONE));
        let paint = scope.for_paint();
        // Resolved here, on the app thread, and shipped as numbers: realizing a new FP16 cell
        // mid-hover would be a surface creation on the interaction path. The two fractions are
        // the alphas the palette authors for the two interaction fills, so the wash has one
        // colour authority and this layer states none of its own.
        let hover = resolve(Role::Fill(Fill::Hover), paint).a;
        let press = resolve(Role::Fill(Fill::Pressed), paint).a;
        if let Some(row) = self.control_mut(id) {
            row.front.wash = wash;
            row.front.hover = hover;
            row.front.press = press;
        }
        self.repaint_control(id);
    }

    /// Returns the sprite `part` paints into, minting or destroying one where ownership moved.
    fn claim_part(
        &mut self,
        row: u32,
        node: NodeId,
        slot: usize,
        held: Option<SpriteId>,
        owned: bool,
        after: Option<NodeId>,
    ) -> Option<SpriteId> {
        match (owned, held) {
            (true, Some(id)) => Some(id),
            (true, None) => Some(self.chrome_visual(GroupId(node), after)),
            (false, Some(id)) => {
                self.drop_part(row, node, slot, id);
                None
            }
            (false, None) => None,
        }
    }

    /// Insets a derived fill inside the border above it, leaving the flush edge alone.
    fn place_part(&mut self, paint: Appearance, surface: Surface) {
        let hairline = match paint.part == Part::Fill && surface.owns(Part::Border) {
            true => surface.chrome.and_then(|chrome| chrome.border_width)
                .unwrap_or_else(|| Len::from(Metric::HairlineW))
                .dips_at(paint.scope, self.env.scale()),
            false => 0.0,
        };
        let mut insets = [hairline; 4];
        if let Some(edge) = surface.chrome.and_then(|chrome| chrome.attached) {
            // Left, right, top, bottom: the order `visual_insets` reads.
            insets[match edge {
                Edge::Left => 0,
                Edge::Right => 1,
                Edge::Top => 2,
                Edge::Bottom => 3,
            }] = 0.0;
        }
        self.visual_insets(paint.id, insets);
    }

    /// The foreground a control's own chrome states in the model state it stands in.
    ///
    /// What a run belonging to that control is painted in where it states no ink of its own,
    /// so a disabled button's label goes with its border and its fill.
    pub(crate) fn owner_ink(&self, id: ControlId) -> Option<Role> {
        let row = self.control(id)?;
        let surface = self.appearances.surface(self.surface_row(row.node))?;
        Some(Role::Text(surface.roles(row.state)?.text))
    }

    /// Returns the role a paint resolves through now, or `None` where its source names none.
    fn role_of(&self, paint: Appearance) -> Option<Role> {
        match paint.source {
            PaintSource::Role(role) => Some(role),
            PaintSource::Owner(id) => {
                let row = self.control(id)?;
                let surface = self.appearances.surface(self.surface_row(row.node))?;
                surface
                    .roles(row.state)
                    .and_then(|roles| paint.part.role(roles, surface.wash))
            }
            PaintSource::Gradient(_) | PaintSource::Region(_) | PaintSource::RegionView(..) | PaintSource::None => None,
        }
    }

    /// Resolves one paint against its scope and sends it.
    ///
    /// A sprite painting a role as ink is lit where the role resolves, so a badge and a
    /// section label need no declaration at all; a surface casting light in a role it does not
    /// paint declares it, and that is the only halo here.
    fn publish_paint(&mut self, paint: Appearance, mask: bool) {
        let scope = paint.scope.for_paint();
        let role = self.role_of(paint);
        let light = role.map_or(Radiance::TRANSPARENT, |role| {
            let light = resolve(role, scope);
            light.with_alpha(light.a * paint.strength)
        });
        let fill = match paint.source {
            PaintSource::Gradient(id) => Paint::Ramp(id),
            PaintSource::Region(id) => Paint::Presented(id),
            PaintSource::RegionView(region, at) => self.regions.iter()
                .find(|(_, row)| row.sink == region)
                .and_then(|(_, row)| row.atlas.as_ref()?.views.get(at as usize))
                .map_or(Paint::None, |view| Paint::PresentedView { region, view: *view }),
            PaintSource::None => Paint::None,
            _ => Paint::Solid(light),
        };
        // One write: a declared halo is what the sprite casts, and a role-painting sprite with
        // none casts its role's own ink light.
        let halo = match paint.halo {
            // A fill casts as an area; a stroke, a border or ink casts as ink.
            Some(style) => {
                let of = if paint.part == Part::Fill { Silhouette::Area } else { Silhouette::Ink };
                self.halo_of_style(style, paint.scope, of)
            }
            None => {
                let ink = role
                    .filter(|_| paint.part == Part::Ink)
                    .map_or(Emission::NONE, |role| emission(role, scope));
                halo_of(ink, Silhouette::Ink, light)
            }
        };
        self.paint(paint.id, fill, halo);
        if mask {
            self.emit_mask(paint.id, paint.mask, paint.scope, paint.surface);
        }
    }

    /// Destroys a derived part this surface no longer owns, and unlinks it.
    ///
    /// The one walk of the paint chain left in this path, and it runs only where a chrome
    /// change took a part away.
    fn drop_part(&mut self, row: u32, owner: NodeId, slot: usize, id: SpriteId) {
        let at = id.0;
        let mut previous = NodeId::NONE;
        let mut link = self.tree.c.paints[owner.index()];
        while let Some(paint) = self.appearances.get(link).copied() {
            if link == at {
                match previous.is_none() {
                    true => self.tree.c.paints[owner.index()] = paint.next,
                    false => {
                        if let Some(held) = self.appearances.get_mut(previous) {
                            held.next = paint.next;
                        }
                    }
                }
                break;
            }
            (previous, link) = (link, paint.next);
        }
        self.appearances.take(at);
        self.destroy(at, Exit::None);
        if let Some(surface) = self.appearances.surface_mut(row) {
            surface.parts[slot] = None;
        }
    }

    /// Re-resolves everything this control's model state paints.
    ///
    /// One pass over the paint table rather than a walk of the control's subtree: a label is
    /// an owner-sourced paint like a fill, and it sits on a child node, so a search that
    /// stopped at the surface's three slots would leave a disabled button's text at its
    /// resting colour.
    pub(crate) fn repaint_control(&mut self, id: ControlId) {
        if let Some(row) = self.control(id) {
            let disabled = row.state == ModelState::Disabled;
            let focus = !disabled
                && self.appearances.surface(self.surface_row(row.node))
                    .is_some_and(|surface| surface.wash == Wash::AccentBorder);
            let row = self.control_mut(id).unwrap();
            row.front.flags = (row.front.flags
                & !(crate::widget::flag::FOCUS_WASH | crate::widget::flag::DISABLED))
                | if focus { crate::widget::flag::FOCUS_WASH } else { 0 }
                | if disabled { crate::widget::flag::DISABLED } else { 0 };
        }
        for at in 0..self.appearances.slots() {
            let Some(node) = self.appearances.id_at(at) else {
                continue;
            };
            let Some(paint) = self.appearances.get(node).copied() else {
                continue;
            };
            if paint.source.owner() == id {
                self.publish_paint(paint, false);
            }
        }
        self.relight_runs(id);
    }

    /// Hangs `id` on `owner`'s paint chain, newest first.
    pub(crate) fn own_appearance(&mut self, id: SpriteId, owner: NodeId) {
        if self.appearances.get(id.0).is_none() {
            return;
        }
        let head = core::mem::replace(&mut self.tree.c.paints[owner.index()], id.0);
        if let Some(paint) = self.appearances.get_mut(id.0) {
            paint.next = head;
        }
    }

    /// Re-emits each paint's mask at the class and the box its solved extent resolved to.
    ///
    /// Both, and not the class alone: a rounded silhouette is capped at half the shorter side
    /// of the box it fills, so a box that moved without changing class leaves a pill rounded
    /// for the extent it used to have.
    ///
    /// A mask resolves from the paint itself, its sprite's class and box, and the scale: so
    /// the pass visits the paints written since it last ran and the sprites the change set
    /// names, and every paint on a sweep.
    pub(crate) fn publish_masks(&mut self) {
        if self.changes.sweeping() {
            self.appearances.restated.clear();
            for at in 0..self.appearances.slots() {
                if let Some(node) = self.appearances.id_at(at) {
                    self.publish_mask(node);
                }
            }
        } else {
            let restated = core::mem::take(&mut self.appearances.restated);
            for &node in &restated {
                self.publish_mask(node);
            }
            self.appearances.restated = restated;
            self.appearances.restated.clear();
            for i in 0..self.tree.moved.len() {
                let node = self.tree.moved[i];
                self.publish_mask(node);
            }
        }
        #[cfg(debug_assertions)]
        for at in 0..self.appearances.slots() {
            if let Some(node) = self.appearances.id_at(at) {
                debug_assert!(self.mask_due(node).is_none(), "the change set missed a mask: {node:?}");
            }
        }
    }

    /// Re-emits one paint's mask where its class, box or scale moved under it.
    fn publish_mask(&mut self, node: NodeId) {
        self.changes.visit();
        let Some(paint) = self.mask_due(node) else {
            return;
        };
        self.emit_mask(paint.id, paint.mask, paint.scope, paint.surface);
        self.appearances.settle(node, paint);
    }

    /// The paint at `node` as its mask resolves now, or `None` where what it last sent holds.
    fn mask_due(&self, node: NodeId) -> Option<Appearance> {
        let mut paint = self.appearances.get(node).copied()?;
        let scope = paint.scope.at_width(self.tree.class(node));
        let bound = paint.mask.box_bound();
        let cap = if bound {
            self.cap_of(paint.id)
        } else {
            f32::NAN
        };
        if scope == paint.scope && paint.scale == self.env.scale() && (!bound || cap == paint.cap) {
            return None;
        }
        paint.scope = scope;
        paint.cap = cap;
        paint.scale = self.env.scale();
        Some(paint)
    }

    /// Half the shorter side of a sprite's solved box, which is where a corner radius
    /// saturates.
    fn cap_of(&self, id: SpriteId) -> f32 {
        // Detached derived sprites take their bounds from the front thread. The scene
        // clamps their profile against those bounds; they have no app-side layout box.
        if self.tree.c.flags[id.0.index()] & tree::DERIVED != 0
            && self.tree.parent(id.0).is_none()
        {
            return f32::INFINITY;
        }
        let size = self.geom(id.0).size;
        size.x.min(size.y) * 0.5
    }

    /// Resolves one authored silhouette against its scope and its solved box, and sends it.
    fn emit_mask(&mut self, id: SpriteId, mask: PaintMask, scope: Scope, surface: u32) {
        let attached = self
            .appearances
            .surface(surface)
            .and_then(|surface| surface.chrome)
            .and_then(|chrome| chrome.attached);
        let cap = self.cap_of(id);
        let scale = self.env.scale();
        let mask = match mask {
            PaintMask::Box { radius } => Mask::Box {
                radius: corners(radius.dips_at(scope, scale).min(cap), attached),
            },
            PaintMask::InsetBox { radius, inset } => Mask::Box {
                radius: corners((radius.dips_at(scope, scale) - inset.dips_at(scope, scale)).max(0.0).min(cap), attached),
            },
            PaintMask::OuterOutline { radius, width } => {
                let width = width.dips_at(scope, scale);
                Mask::Outline {
                    radius: corners((radius.dips_at(scope, scale) + width).min(cap), None),
                    width,
                    open: None,
                }
            }
            PaintMask::Outline { radius, width } => Mask::Outline {
                radius: corners(radius.dips_at(scope, scale).min(cap), attached),
                width: width.dips_at(scope, scale),
                open: attached.map(side_of),
            },
            PaintMask::Shape { geom, stroke, space } => {
                let stroke =
                    stroke.map(|width| self.stroke(width.dips_at(scope, scale), Cap::Round, Join::Round, &[]));
                Mask::Shape { geom, stroke, space }
            }
            PaintMask::Region => Mask::Box {
                radius: Corners::default(),
            },
        };
        self.mask(id, mask);
    }

    /// Resolves a declared halo: a role's own light, or the palette's occlusion cast towards
    /// `edge`.
    fn halo_of_style(&self, style: HaloStyle, scope: Scope, of: Silhouette) -> Option<Halo> {
        let paint = scope.for_paint();
        match style {
            HaloStyle::Glow(role) => {
                let halo = halo_of(emission(role, paint), of, resolve(role, paint));
                debug_assert!(
                    halo.is_some(),
                    "a halo was declared in a role the palette gives no light"
                );
                halo
            }
            HaloStyle::Shadow(edge, token) => {
                let shadow = token.map_or_else(|| shadow(paint), |token| token.resolve(paint));
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
                    sigma: shadow.sigma,
                    tint: shadow.light,
                    offset,
                })
            }
        }
    }

    /// Fills the solve's metric cache for every width class.
    ///
    /// Filled once per class, so resolving a length during the solve is an index and an FMA
    /// rather than a call into the application's palette. This is the cache's one writer and
    /// it fills it by calling `metric` itself, so the cache and the authority cannot disagree.
    pub(crate) fn fill_metrics(&mut self, root: Scope) {
        for class in WidthClass::ALL {
            self.metrics[class as usize] = Metric::BUILTIN.map(|m| metric(m, root.at_width(class)));
        }
    }

    /// Re-resolves this window's retained recipes, preserving node identity and lexical scope.
    /// The matching backdrop is handed to the scene in the same batch as these paint edits.
    pub fn set_theme(&mut self, root: Scope, backdrop: BackdropSpec) {
        if self.root_scope() == root {
            // A backdrop-only change is published too; repeating an identical request performs
            // no retained work.
            PENDING_THEME.with_borrow_mut(|held| {
                if held.as_ref().map(|(_, held)| held) != Some(&backdrop) {
                    *held = Some((root, backdrop));
                }
            });
            return;
        }
        // Every interned scope rebases the same way, so the table is rebased once and no
        // node's own column moves. Lexical elevation and solved width survive it.
        self.rebase_scopes(root);
        // Every run, mask and radius resolves through the scope, and no box need move.
        self.changes.owe_sweep();
        // The content peak is the palette's brightest authored channel, so a palette change
        // changes it. Display capability is separate and is not touched here.
        self.env = Env::new(
            self.env.dpi(),
            self.env
                .output()
                .with_content_peak_nits(content_peak_nits(&self.env.output().gamut(), root)),
        );
        self.fill_metrics(root);
        // Each presentation region has no `Scope` on the thread it draws on, so it reads one
        // through a versioned handle; the version is what wakes a region already awake.
        for (_, row) in self.regions.iter() {
            row.theme.set(row.theme.get().in_theme(root));
        }
        for at in 0..self.appearances.slots() {
            let Some(node) = self.appearances.id_at(at) else {
                continue;
            };
            let Some(mut paint) = self.appearances.get(node).copied() else {
                continue;
            };
            paint.scope = self.scope_of(node);
            self.publish_paint(paint, true);
            self.appearances.place(node, paint);
        }
        // Ramps are authored in roles, so a gradient follows the theme like any other paint.
        self.relight_ramps();
        for at in 0..self.appearances.surface_rows() {
            if let Some(surface) = self.appearances.surface_mut(at) {
                surface.dirty = true;
            }
        }
        // The type ramp moved, so every run is behind its source and the next solve reshapes
        // it. Nothing is emitted here.
        self.retheme_text(root);
        for (_, control) in self.controls.iter_mut() {
            control.scope = control.scope.in_theme(root);
        }
        PENDING_THEME.with_borrow_mut(|held| *held = Some((root, backdrop)));
    }
}

// -- the appearance setters ------------------------------------------------------------
//
// Each states one part, one source and one silhouette, and nothing else: the resolution is
// the one resolver's, and these are the vocabulary an author writes it in.

impl Ui<'_> {
    /// A filled box painted in one role.
    pub fn plate(&mut self, radius: impl Into<Len>, role: Role, strength: f32) -> Element<'_> {
        let node = self.sprite(Preset::Layer).node_id();
        self.element(node).plate(radius, role, strength)
    }
}

impl<K> Element<'_, K> {
    fn part(mut self, part: Part, source: PaintSource, mask: PaintMask, strength: f32) -> Self {
        let node = self.node_id();
        self.host().declare_part(node, part, source, mask, strength);
        self
    }

    pub fn plate(self, radius: impl Into<Len>, role: Role, strength: f32) -> Self {
        let mask = PaintMask::Box {
            radius: radius.into(),
        };
        self.part(Part::Fill, PaintSource::Role(role), mask, strength)
    }

    pub fn outline(self, radius: Metric, role: Role, width: impl Into<Len>) -> Self {
        let mask = PaintMask::Outline {
            radius: radius.into(),
            width: width.into(),
        };
        self.part(Part::Border, PaintSource::Role(role), mask, 1.0)
    }

    /// Draws an outline outside a container without changing its layout or hit rectangle.
    /// The derived border follows the container's resolved bounds. Ancestor clips apply.
    ///
    /// # Panics
    ///
    /// This element must be a container, not a painted sprite.
    pub fn outline_outside(mut self, radius: impl Into<Len>, role: Role, width: impl Into<Len>) -> Self {
        let node = self.node_id();
        assert_eq!(self.host().tree.c.flags[node.index()] & tree::SPRITE, 0,
            "an outer outline requires a container");
        self.part(Part::Border, PaintSource::Role(role),
            PaintMask::OuterOutline { radius: radius.into(), width: width.into() }, 1.0)
    }

    /// A gradient over the whole box, rounded as a plate is.
    pub fn washed(self, id: RampId, radius: Metric) -> Self {
        let mask = PaintMask::Box {
            radius: radius.into(),
        };
        self.part(Part::Fill, PaintSource::Gradient(id), mask, 1.0)
    }

    /// Declares the chrome this element's surface resolves through.
    pub fn appearance(mut self, chrome: Chrome) -> Self {
        let node = self.node_id();
        self.host().declare_chrome(node, chrome);
        self
    }

    pub fn ghost(self) -> Self {
        self.button_chrome(crate::widget::roles::GHOST)
    }

    pub fn accent(self) -> Self {
        self.button_chrome(crate::widget::roles::ACCENT)
    }

    pub fn accent_subtle(self) -> Self {
        self.button_chrome(crate::widget::roles::ACCENT_SUBTLE)
    }

    fn button_chrome(mut self, variant: u8) -> Self {
        let roles = crate::widget::roles::BUTTON[variant as usize];
        let node = self.node_id();
        let host = self.host();
        let chrome = host.appearances.surface(host.surface_row(node))
            .and_then(|surface| surface.chrome)
            .unwrap_or_else(|| Chrome::new(roles, Metric::Radius));
        self.appearance(chrome.with_roles(roles))
    }

    /// Raises this element's scope by one rung, and every paint already on it with it.
    pub fn elevate(mut self, elevation: Elevation) -> Self {
        let node = self.node_id();
        self.host().elevate(node, elevation);
        self
    }

    /// Casts this element's own occlusion towards `edge`.
    pub fn shadowed(self, edge: Edge) -> Self {
        self.halo_style(HaloStyle::Shadow(edge, None))
    }

    /// Casts a scope-resolved occlusion through the shared alpha-mask and FP16 tint path.
    pub fn shadowed_with(self, edge: Edge,
        token: &'static crate::role::ScopedToken<crate::role::Shadow>) -> Self {
        self.halo_style(HaloStyle::Shadow(edge, Some(token)))
    }

    /// Casts light in `role`, which this element need not paint in.
    pub fn halo<M>(mut self, role: impl Signal<Role, M> + 'static) -> Self {
        if role.is_constant() {
            return self.halo_style(HaloStyle::Glow(role.read()));
        }
        let node = self.node_id();
        self.host().binding(move || {
            let role = role.read();
            Host::with(|host| host.set_halo(node, HaloStyle::Glow(role)));
        });
        self
    }

    /// How much of its halo this element is spending now.
    pub fn halo_lit<M>(self, value: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::GlowOpacity, value)
    }

    fn halo_style(mut self, halo: HaloStyle) -> Self {
        let node = self.node_id();
        self.host().set_halo(node, halo);
        self
    }
}

impl Element<'_, super::Path> {
    /// Maps unit-box geometry to this path's live bounds without scaling its stroke width.
    ///
    /// Geometry radii scale with the box. The path must have declared its paint first.
    pub fn unit_box(mut self) -> Self {
        let node = self.node_id();
        let paint = self.host().appearances.get_mut(node)
            .expect("a unit path must declare its paint first");
        let PaintMask::Shape { space, .. } = &mut paint.mask else {
            unreachable!("a painted path carries a shape mask")
        };
        *space = PathSpace::Unit;
        let paint = *paint;
        self.host().publish_paint(paint, true);
        self
    }

    /// Fills this shape in a chromatic role.
    pub fn fill(self, role: DataRole) -> Self {
        self.shape(PaintSource::data(role), None, Part::Fill)
    }

    pub fn fill_ramp(self, id: RampId) -> Self {
        self.shape(PaintSource::Gradient(id), None, Part::Fill)
    }

    pub fn stroke(self, role: impl Into<Role>, width: impl Into<Len>) -> Self {
        self.shape(
            PaintSource::Role(role.into()),
            Some(width.into()),
            Part::Border,
        )
    }

    pub fn stroke_ramp(self, id: RampId, width: impl Into<Len>) -> Self {
        self.shape(PaintSource::Gradient(id), Some(width.into()), Part::Border)
    }

    pub fn line(self, role: Stroke) -> Self {
        self.line_stroke(role, Metric::HairlineW)
    }

    pub fn line_stroke(self, role: Stroke, width: impl Into<Len>) -> Self {
        self.shape(PaintSource::stroke(role), Some(width.into()), Part::Border)
    }

    /// Fills this shape in the enclosing control's own foreground, or the window's where it
    /// has no chrome to take one from.
    pub fn ink(self) -> Self {
        self.ink_paint(None)
    }

    pub fn ink_stroke(self, width: impl Into<Len>) -> Self {
        self.ink_paint(Some(width.into()))
    }

    fn ink_paint(mut self, stroke: Option<Len>) -> Self {
        let owner = self.ui.control;
        let source = match self.host().chrome(owner) {
            Some(_) => PaintSource::Owner(owner),
            None => PaintSource::Role(Role::Text(Text::Primary)),
        };
        self.shape(source, stroke, Part::Ink)
    }

    fn shape(mut self, source: PaintSource, stroke: Option<Len>, part: Part) -> Self {
        let node = self.node_id();
        // The shape is the node's own, recorded when it was minted: a path states what it
        // draws before it states what paints it.
        let Some(geom) = self.host().appearances.shape(node) else {
            return self;
        };
        let space = match self.host().appearances.get(node).map(|paint| paint.mask) {
            Some(PaintMask::Shape { space, .. }) => space,
            _ => PathSpace::Local,
        };
        self.part(part, source, PaintMask::Shape { geom, stroke, space }, 1.0)
    }

    /// Paints this shape at `strength` of the alpha its role resolves to.
    ///
    /// # Panics
    ///
    /// Panics where this shape has stated no paint: a strength scales a role, and there is
    /// none to scale before one is named.
    pub fn strength(mut self, strength: f32) -> Self {
        let node = self.node_id();
        let paint = self
            .host()
            .appearances
            .get_mut(node)
            .expect("a strength scales a paint this shape has already stated");
        paint.strength = strength;
        let paint = *paint;
        self.host().publish_paint(paint, false);
        self
    }
}

impl Host {
    /// Declares the light a node casts past its own silhouette, on its surface where it owns
    /// one and on its first paint where it does not.
    fn set_halo(&mut self, node: NodeId, halo: HaloStyle) {
        if self.surface_row(node) != tree::NONE
            || self.tree.c.flags[node.index()] & tree::SPRITE == 0
        {
            self.surface_halo(GroupId(node), halo);
            let mut link = self.tree.c.paints[node.index()];
            while let Some(mut paint) = self.appearances.get(link).copied() {
                link = paint.next;
                if paint.part == Part::Fill && paint.surface == tree::NONE {
                    paint.halo = Some(halo);
                    self.appearances.place(paint.id.0, paint);
                    self.publish_paint(paint, false);
                }
            }
            return;
        }
        let Some(paint) = self.appearances.get_mut(node) else {
            return;
        };
        paint.halo = Some(halo);
        let paint = *paint;
        self.publish_paint(paint, false);
    }
}

/// Returns one silhouette's worth of a role's light, or `None` where it spends none.
fn halo_of(emission: Emission, of: Silhouette, light: Radiance) -> Option<Halo> {
    let spend = emission.of(of);
    spend.is_lit().then(|| Halo {
        sigma: spend.sigma,
        tint: light.with_alpha(light.a * spend.strength),
        offset: Vector2::default(),
    })
}

/// Returns the four corner radii of a box rounded at `radius`, with the flush edge squared.
///
/// The attached edge of a joined control has square corners, so two buttons meeting at that
/// edge leave no rounded gap between them.
fn corners(radius: f32, attached: Option<Edge>) -> Corners {
    let mut corners = Corners::all(radius);
    match attached {
        Some(Edge::Left) => (corners.tl, corners.bl) = (0.0, 0.0),
        Some(Edge::Right) => (corners.tr, corners.br) = (0.0, 0.0),
        Some(Edge::Top) => (corners.tl, corners.tr) = (0.0, 0.0),
        Some(Edge::Bottom) => (corners.bl, corners.br) = (0.0, 0.0),
        None => {}
    }
    corners
}

const fn side_of(edge: Edge) -> Side {
    match edge {
        Edge::Left => Side::Left,
        Edge::Top => Side::Top,
        Edge::Right => Side::Right,
        Edge::Bottom => Side::Bottom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detached_focus_profile_survives_dpi_changes_without_layout_or_idle_work() {
        let mut patch = crate::build::rig::fixture();
        let ring = Host::with(Host::focus_outline);
        let count = Host::with(|host| host.live_nodes());
        for scale in [1.0, 1.5, 2.0, 1.25, 1.0] {
            Host::with(|host| host.set_env(Env::new(96.0 * scale, host.env.output().clone())));
            Host::flush(&mut patch);
            let expected = Host::with(|host| metric(Metric::Radius, host.root_scope())) + FOCUS_OUTSET;
            let mask = patch.ops().iter().rev().find_map(|op| match op {
                windows_scene::Op::Mask { id, mask } if id.0 == ring => Some(mask),
                _ => None,
            }).expect("DPI publication retains the detached outline");
            assert_eq!(*mask, Mask::Outline {
                radius: Corners::all(expected), width: FOCUS_STROKE, open: None,
            });
            assert_eq!(Host::with(|host| host.live_nodes()), count);
            patch.clear();
            Host::flush(&mut patch);
            assert!(patch.ops().is_empty());
        }
    }

    #[test]
    fn outer_rims_follow_live_bounds_and_explicit_plates_receive_scoped_shadows() {
        use windows_scene::Op;
        assert_eq!(size_of::<super::super::host::Visual>(), 20);
        static SHADOW: crate::role::ScopedToken<crate::role::Shadow> =
            crate::role::ScopedToken::new("test shadow", |_| crate::role::Shadow {
                sigma: 10.0, offset: 6.0, light: Radiance::new(0.0, 0.0, 0.0, 0.3),
            });
        let mut patch = crate::build::rig::fixture();
        let empty = Host::with(|host| host.live_nodes());
        let width = crate::signal::Cell::new(100.0);
        let mut nodes = [NodeId::NONE; 2];
        let (owner, mount) = crate::signal::Owner::scope(|| Ui::mount_root(|ui| {
            nodes[0] = ui.node(Preset::Layer).width(Len::dip(100.0)).height(Len::dip(40.0))
                .shadowed_with(Edge::Bottom, &SHADOW)
                .plate(Len::dip(7.0), Role::Fill(Fill::Surface), 1.0)
                .id().into();
            nodes[1] = ui.node(Preset::Layer).width(Len::dip(100.0)).height(Len::dip(40.0))
                .layout_from(move |l| l.width = Len::dip(width.get()))
                .plate(Len::dip(7.0), Role::Fill(Fill::Surface), 1.0)
                .outline_outside(Len::dip(7.0), Role::Stroke(Stroke::Focus), Len::px(1.0))
                .shadowed_with(Edge::Bottom, &SHADOW)
                .id().into();
        }));
        Host::flush(&mut patch);
        for node in nodes {
            Host::with(|host| {
                let mut link = host.tree.c.paints[node.index()];
                let mut fills = 0;
                while let Some(paint) = host.appearances.get(link) {
                    if paint.part == Part::Fill {
                        fills += 1;
                        assert!(patch.ops().iter().any(|op| matches!(op,
                            Op::Paint { id, halo: Some(Halo { sigma, tint, offset }), .. }
                            if *id == paint.id && *sigma == 10.0 && tint.a == 0.3
                                && *offset == Vector2::new(0.0, 6.0)
                        )));
                    } else { assert!(paint.halo.is_none()); }
                    link = paint.next;
                }
                assert_eq!(fills, 1);
            });
        }
        let count = Host::with(|host| host.live_nodes());
        for (scale, w) in [(1.5, 160.0), (2.0, 80.0), (1.25, 200.0), (1.0, 100.0)] {
            width.set(w);
            crate::signal::flush();
            Host::with(|host| host.set_env(Env::new(96.0 * scale, host.env.output())));
            patch.clear();
            Host::flush(&mut patch);
            Host::with(|host| {
                let border = host.appearances.get(host.tree.c.paints[nodes[1].index()]).unwrap();
                assert!(matches!(border.mask, PaintMask::OuterOutline { .. }));
                let geom = host.geom(border.id.0);
                assert_eq!(geom.local, Vector2::new(-1.0 / scale, -1.0 / scale));
                assert!((geom.size.x - w - 2.0 / scale).abs() < 0.001, "rim {geom:?}, owner {:?}, requested {w}", host.geom(nodes[1]));
                assert!((geom.size.y - 40.0 - 2.0 / scale).abs() < 0.001);
                assert!(patch.ops().iter().any(|op| matches!(op,
                    Op::Mask { id, mask: Mask::Outline { width, radius, .. } }
                    if *id == border.id && *width == 1.0 / scale
                        && *radius == Corners::all(7.0 + 1.0 / scale)
                )));
                assert_eq!(host.live_nodes(), count);
            });
            patch.clear(); Host::flush(&mut patch);
            assert!(patch.ops().is_empty());
        }
        let allocations = crate::counting::allocations();
        for _ in 0..20 {
            patch.clear();
            Host::flush(&mut patch);
            assert!(patch.ops().is_empty());
        }
        assert_eq!(crate::counting::allocations(), allocations);
        drop(mount);
        drop(owner);
        Host::flush(&mut patch);
        assert_eq!(Host::with(|host| host.live_nodes()), empty);
    }

    #[test]
    fn pixel_borders_and_fill_insets_follow_dpi_without_idle_publication() {
        use windows_scene::{Env, Op};
        let mut patch = crate::build::rig::fixture();
        let mut node = NodeId::NONE;
        let (_owner, _mount) = crate::signal::Owner::scope(|| super::super::Ui::mount_root(|ui| {
            node = ui.control(Some(Chrome::new(
                roles(Some(Fill::Surface), Some(Stroke::Default)), Metric::Radius,
            ).border(Len::px(1.0))), crate::widget::UiaRole::Button, |_| {})
                .width(Len::dip(100.0)).height(Len::dip(24.0)).id().into();
        }));
        for scale in [1.0, 1.5, 2.0, 1.25, 1.0] {
            Host::with(|host| host.set_env(Env::new(96.0 * scale, host.env.output().clone())));
            patch.clear();
            Host::flush(&mut patch);
            Host::with(|host| {
                let surface = host.appearances.surface(host.surface_row(node)).unwrap();
                let border = surface.parts[0].unwrap();
                let fill = surface.parts[1].unwrap();
                assert!(patch.ops().iter().any(|op| matches!(op,
                    Op::Mask { id, mask: Mask::Outline { width, .. } }
                    if *id == border && (width * scale - 1.0).abs() < 0.001
                )));
                let outer = host.geom(node).size;
                let inner = host.geom(fill.0).size;
                assert!(((outer.x - inner.x) * scale - 2.0).abs() < 0.001);
                assert!(((outer.y - inner.y) * scale - 2.0).abs() < 0.001);
            });
            patch.clear();
            Host::flush(&mut patch);
            assert!(patch.ops().is_empty());
        }
    }

    #[test]
    fn button_variants_preserve_the_authored_shape() {
        let mut rig = crate::build::rig::Rig::at(300.0, 100.0, 1.5);
        let mut node = NodeId::NONE;
        rig.mount(|ui| {
            let mut chrome = Chrome::new(roles(None, None), Metric::RadiusSurface).border(Len::px(1.0));
            chrome.attached = Some(Edge::Left);
            node = ui.button(chrome, crate::widget::TextStyle::new(crate::role::TypeRole::Body), "Action")
                .ghost().accent().accent_subtle().id().into();
        });
        Host::with(|host| {
            let chrome = host.appearances.surface(host.surface_row(node)).unwrap().chrome.unwrap();
            assert_eq!(chrome.radius, Metric::RadiusSurface);
            assert_eq!(chrome.attached, Some(Edge::Left));
            assert_eq!(chrome.border_width, Some(Len::px(1.0)));
            assert_eq!(chrome.in_state(ModelState::Rest), crate::widget::roles::BUTTON[2]);
        });
    }

    #[test]
    fn focus_border_reuses_the_wash_and_matches_chrome_across_dpi_and_availability() {
        let mut patch = crate::build::rig::fixture();
        let mut node = NodeId::NONE;
        let (_owner, _mount) = crate::signal::Owner::scope(|| Ui::mount_root(|ui| {
            let mut chrome = Chrome::new(roles(Some(Fill::Surface), Some(Stroke::Default)), Metric::Radius)
                .border(Len::px(1.0));
            chrome.attached = Some(Edge::Left);
            node = ui.control(Some(chrome), crate::widget::UiaRole::Edit, |_| {})
                .width(Len::dip(100.0)).height(Len::dip(24.0)).id().into();
        }));
        Host::flush(&mut patch);
        let count = Host::with(|host| host.live_nodes());
        Host::with(|host| host.surface_wash(GroupId(node), Wash::AccentBorder));
        for scale in [1.0, 1.5, 2.0, 1.25, 1.0] {
            Host::with(|host| host.set_env(Env::new(96.0 * scale, host.env.output())));
            patch.clear(); Host::flush(&mut patch);
            Host::with(|host| {
                let surface = host.appearances.surface(host.surface_row(node)).unwrap();
                let border = surface.parts[0].unwrap();
                let wash = surface.parts[2].unwrap();
                let mask = |id| patch.ops().iter().rev().find_map(|op| match op {
                    windows_scene::Op::Mask { id: at, mask } if *at == id => Some(*mask),
                    _ => None,
                }).unwrap();
                assert_eq!(mask(border), mask(wash));
                assert!(matches!(mask(wash), Mask::Outline { width, open: Some(Side::Left), .. }
                    if width == 1.0 / scale));
                let control = host.control_of(node);
                assert_ne!(host.control(control).unwrap().front.flags & crate::widget::flag::FOCUS_WASH, 0);
                host.set_state(control, ModelState::Disabled, true);
                assert_eq!(host.control(control).unwrap().front.flags & crate::widget::flag::FOCUS_WASH, 0);
                host.set_state(control, ModelState::Disabled, false);
                assert_ne!(host.control(control).unwrap().front.flags & crate::widget::flag::FOCUS_WASH, 0);
                assert_eq!(host.live_nodes(), count);
            });
            patch.clear(); Host::flush(&mut patch);
            patch.clear(); Host::flush(&mut patch);
            assert!(patch.ops().is_empty());
        }
    }

    fn roles(fill: Option<Fill>, stroke: Option<Stroke>) -> RoleSet {
        RoleSet::new(fill, stroke, Text::Primary)
    }

    fn surface(chrome: Chrome) -> Surface {
        Surface {
            chrome: Some(chrome),
            ..Surface::new(NodeId::NONE)
        }
    }

    /// A part the recipe never gives a role to is not owned, so nothing mints a sprite for it.
    #[test]
    fn a_surface_owns_only_the_parts_its_recipe_paints() {
        let plain = surface(Chrome::new(
            roles(Some(Fill::Surface), None),
            Metric::Radius,
        ));
        assert!(plain.owns(Part::Fill));
        assert!(!plain.owns(Part::Border));
    }

    /// The wrong replacement is what this catches: a fill drawn at the border's own radius
    /// aliases the border along the corner arc.
    #[test]
    fn a_bordered_fill_is_authored_a_hairline_inside_its_border() {
        let bordered = surface(Chrome::new(
            roles(Some(Fill::Surface), Some(Stroke::Default)),
            Metric::Radius,
        ));
        let plain = surface(Chrome::new(
            roles(Some(Fill::Surface), None),
            Metric::Radius,
        ));
        let outer = Len::from(Metric::Radius);
        let width = Len::from(Metric::HairlineW);
        assert!(
            matches!(bordered.mask_of(Part::Fill), PaintMask::InsetBox { radius, inset } if radius == outer && inset == width)
        );
        let whole = Len::from(Metric::Radius);
        assert!(matches!(plain.mask_of(Part::Fill), PaintMask::Box { radius } if radius == whole));
        assert!(matches!(
            bordered.mask_of(Part::Border),
            PaintMask::Outline { .. }
        ));
    }

    /// A disabled-only state still owns its part, so entering that state retargets a sprite
    /// rather than minting one on the interaction path.
    #[test]
    fn a_part_only_a_state_paints_is_still_owned_at_rest() {
        let chrome = Chrome::new(roles(None, None), Metric::Radius)
            .when(ModelState::Disabled, roles(Some(Fill::Surface), None));
        assert!(surface(chrome).owns(Part::Fill));
    }

    /// The flush edge squares its own two corners and opens the outline there.
    #[test]
    fn an_attached_edge_squares_the_corners_it_joins_on() {
        let joined = corners(8.0, Some(Edge::Right));
        assert_eq!((joined.tr, joined.br), (0.0, 0.0));
        assert_eq!((joined.tl, joined.bl), (8.0, 8.0));
        assert_eq!(corners(8.0, None).max(), 8.0);
        assert_eq!(side_of(Edge::Right), Side::Right);
    }

    /// A light the palette spends nothing on declares no composition object.
    #[test]
    fn an_unlit_role_casts_no_halo() {
        assert!(halo_of(Emission::NONE, Silhouette::Ink, Radiance::TRANSPARENT).is_none());
    }

    /// A region paints its own buffer, so its silhouette owes a solved box no re-emission.
    #[test]
    fn a_box_silhouette_is_box_bound_and_a_region_is_not() {
        assert!(PaintMask::Box { radius: Len::ZERO }.box_bound());
        assert!(!PaintMask::Region.box_bound());
    }
}
