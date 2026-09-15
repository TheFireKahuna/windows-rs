//! Borrowed parent-first authoring. Constructors write retained records immediately.
use super::host::MountRow;
use super::style::{Declaration, Recipe};
use super::theme::{Appearance, PaintSource};
use super::theme::{PaintMask, Part};
use super::{Any, Host, Mount, Path};
use crate::layout::{Layout, Len, Preset};
use crate::role::{Role, Scope, Text};
use crate::signal::{Effect, Signal};
use crate::widget::{Flow, TextSource, TextStyle};
use core::marker::PhantomData;
use windows_scene::{GroupId, NodeId, Prop, SpriteId, Value};

#[derive(Copy, Clone, Debug)]
pub(super) enum Target {
    Group(GroupId),
    Sprite(SpriteId),
}
impl Target {
    pub(super) fn id(self) -> NodeId {
        match self {
            Self::Group(id) => id.node(),
            Self::Sprite(id) => id.node(),
        }
    }
}

/// Opaque retained identity. Both the node generation and runtime lifetime are checked on edit.
pub struct Node<K = Any> {
    pub(super) target: Target,
    pub(super) runtime: u64,
    pub(super) owner: Option<windows_scene::ControlId>,
    pub(super) hover_scope: Option<windows_scene::ControlId>,
    pub(super) kind: PhantomData<fn() -> K>,
}
impl<K> Copy for Node<K> {}
impl<K> Clone for Node<K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K> core::fmt::Debug for Node<K> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.target.fmt(f)
    }
}

/// A short-lived writer borrowing the runtime. Keep `id()` when later edits are needed.
pub struct Element<'a, K = Any> {
    pub(super) host: &'a mut Host,
    pub(super) members: Option<&'a mut Members>,
    pub(super) node: Node<K>,
}

#[derive(Default)]
pub(super) struct Members {
    first: NodeId,
    parent: Option<GroupId>,
    roots: Vec<NodeId>,
}
impl Members {
    fn push(&mut self, host: &mut Host, id: NodeId) {
        if self.first.is_none() {
            self.first = id;
        }
        host.mounts.place(id, MountRow::new());
    }
}

/// The authoring and update context. Construction is available only in creation transactions.
pub struct Ui<'a> {
    pub(super) host: &'a mut Host,
    pub(super) members: &'a mut Members,
    pub(super) parent: Option<GroupId>,
    pub(super) after: Option<NodeId>,
    pub(super) scope: Scope,
    pub(super) owner: Option<windows_scene::ControlId>,
    pub(super) hover_scope: Option<windows_scene::ControlId>,
}

impl Ui<'_> {
    pub(crate) fn mount_at(
        parent: GroupId,
        after: Option<NodeId>,
        scope: Scope,
        owner: Option<windows_scene::ControlId>,
        create: impl FnOnce(&mut Ui<'_>),
    ) -> Mount {
        Self::mount_scoped(parent, after, scope, owner, None, create)
    }

    fn mount_scoped(
        parent: GroupId,
        after: Option<NodeId>,
        scope: Scope,
        owner: Option<windows_scene::ControlId>,
        hover_scope: Option<windows_scene::ControlId>,
        create: impl FnOnce(&mut Ui<'_>),
    ) -> Mount {
        Host::with(|host| {
            let mut members = Members {
                roots: host.root_pool.pop().unwrap_or_default(),
                parent: Some(parent),
                ..Default::default()
            };
            let scope = scope.in_theme(host.root_scope);
            create(&mut Ui {
                host,
                members: &mut members,
                parent: Some(parent),
                after,
                scope,
                owner,
                hover_scope,
            });
            Mount::new(members.roots, host.identity)
        })
    }

    /// Retains keyed results and moves only roots outside the surviving LIS.
    pub fn each<T: 'static, K: Eq + core::hash::Hash + Clone + 'static>(
        &mut self,
        fill: impl Fn(&mut Vec<T>) + 'static,
        key: impl Fn(&T) -> &K + 'static,
        create: impl Fn(&mut Ui<'_>, &T) + 'static,
    ) {
        let parent = self
            .parent
            .expect("structure requires a creation transaction");
        let anchor = self
            .group(Preset::Bare, |_| {})
            .layout(|l| l.hidden = Some(true))
            .id()
            .target
            .id();
        let (scope, owner, hover_scope) = (self.scope, self.owner, self.hover_scope);
        let mut list = crate::structure::Keyed::<K, Mount>::new();
        let mut next = Vec::new();
        self.host.binding(anchor, move || {
            next.clear();
            fill(&mut next);
            let mut after = Some(anchor);
            list.reconcile(
                &next,
                &key,
                |_, item| {
                    Self::mount_scoped(parent, Some(anchor), scope, owner, hover_scope, |ui| {
                        create(ui, item)
                    })
                },
                |mount, _, step, _| {
                    after = mount.place(parent, after, step != crate::structure::Step::Keep);
                },
            );
        });
    }

    pub fn when<M>(
        &mut self,
        condition: impl Signal<bool, M> + 'static,
        create: impl Fn(&mut Ui<'_>) + 'static,
    ) {
        if condition.is_constant() {
            if condition.read() {
                create(self);
            }
        } else {
            self.branch(
                move || condition.read().then_some(()),
                move |ui, _| create(ui),
            );
        }
    }

    pub fn switch<K: PartialEq + 'static>(
        &mut self,
        key: impl Fn() -> K + 'static,
        create: impl Fn(&mut Ui<'_>, &K) + 'static,
    ) {
        self.branch(move || Some(key()), create);
    }

    fn branch<K: PartialEq + 'static>(
        &mut self,
        key: impl Fn() -> Option<K> + 'static,
        create: impl Fn(&mut Ui<'_>, &K) + 'static,
    ) {
        let parent = self
            .parent
            .expect("structure requires a creation transaction");
        let anchor = self
            .group(Preset::Bare, |_| {})
            .layout(|l| l.hidden = Some(true))
            .id()
            .target
            .id();
        let (scope, owner, hover_scope) = (self.scope, self.owner, self.hover_scope);
        let mut branch = crate::structure::Branch::<K, Mount>::new();
        self.host.binding(anchor, move || {
            branch.set(key(), |key| {
                Self::mount_scoped(parent, Some(anchor), scope, owner, hover_scope, |ui| {
                    create(ui, key)
                })
            });
        });
    }

    pub(crate) fn mount_root(create: impl FnOnce(&mut Ui<'_>)) -> Mount {
        Host::with(|host| {
            let root = host.model.root();
            let mut members = Members::default();
            members.roots = host.root_pool.pop().unwrap_or_default();
            members.roots.push(root.node());
            members.push(host, root.node());
            let scope = host.root_scope;
            create(&mut Ui {
                host,
                members: &mut members,
                parent: Some(root),
                after: None,
                scope,
                owner: None,
                hover_scope: None,
            });
            Mount::new(members.roots, host.identity)
        })
    }

    /// Returns the lexical design scope; width-dependent values resolve during solving.
    pub fn scope(&self) -> Scope {
        self.scope
    }

    pub fn geometry(&mut self, verbs: &[windows_scene::PathVerb]) -> windows_scene::GeomId {
        let id = self.host.model.geometry(verbs);
        crate::signal::Owner::retain(super::geometry::Lease(id, self.host.identity));
        id
    }

    pub fn path_with(
        &mut self,
        capacity: usize,
        mut draw: impl FnMut(&mut Vec<windows_scene::PathVerb>, windows_numerics::Vector2, Scope)
        + 'static,
    ) -> Element<'_, Path> {
        let geometry = self.geometry(&[]);
        let element = self.path(geometry);
        let mut verbs = Vec::with_capacity(capacity);
        super::geometry::install(
            element.host,
            element.node.target.id(),
            Box::new(move |size, scope| {
                verbs.clear();
                crate::signal::read_only(|| draw(&mut verbs, size, scope));
                super::set_geometry(geometry, &verbs);
            }),
        );
        element
    }

    /// Returns absence for a retired generation or a handle from another runtime.
    pub fn edit<K>(&mut self, node: Node<K>) -> Option<Element<'_, K>> {
        (node.runtime == self.host.identity && self.host.mounts.get(node.target.id()).is_some())
            .then_some(Element {
                host: self.host,
                members: None,
                node,
            })
    }

    pub(super) fn create<K>(&mut self, preset: Preset, sprite: bool) -> Node<K> {
        let parent = self
            .parent
            .expect("structure requires a creation transaction");
        let target = if sprite {
            Target::Sprite(self.host.model.sprite(parent, self.after))
        } else {
            Target::Group(self.host.model.group(parent, self.after))
        };
        let id = target.id();
        if self.members.parent == Some(parent) {
            self.members.roots.push(id);
        }
        self.after = Some(id);
        self.members.push(self.host, id);
        let recipe = Recipe {
            preset,
            scope: self.scope,
            layout: Declaration::default(),
        };
        self.host
            .model
            .style(id, &recipe.lower(self.scope.width, None));
        self.host.styles.place(id, recipe);
        Node {
            target,
            runtime: self.host.identity,
            owner: self.owner,
            hover_scope: self.hover_scope,
            kind: PhantomData,
        }
    }

    /// Allocates the parent immediately. Chained declarations precede `children`.
    pub fn node(&mut self, preset: Preset) -> Element<'_> {
        let node = self.create(preset, false);
        Element {
            host: self.host,
            members: Some(self.members),
            node,
        }
    }

    pub fn group(&mut self, preset: Preset, children: impl FnOnce(&mut Ui<'_>)) -> Element<'_> {
        self.node(preset).children(children)
    }

    pub fn row(&mut self, children: impl FnOnce(&mut Ui<'_>)) -> Element<'_> {
        self.group(Preset::Row, children)
    }
    pub fn stack(&mut self, children: impl FnOnce(&mut Ui<'_>)) -> Element<'_> {
        self.group(Preset::Stack, children)
    }
    pub fn grid(&mut self, children: impl FnOnce(&mut Ui<'_>)) -> Element<'_> {
        self.group(Preset::Grid, children)
    }

    pub fn scroll(
        &mut self,
        declaration: crate::layout::ScrollDecl,
        children: impl FnOnce(&mut Ui<'_>),
    ) -> Element<'_> {
        self.scroll_content(declaration, |ui| ui.stack(children).no_shrink().id())
    }
    pub(crate) fn scroll_content<K>(
        &mut self,
        declaration: crate::layout::ScrollDecl,
        create: impl FnOnce(&mut Ui<'_>) -> Node<K>,
    ) -> Element<'_> {
        let node = self.create(Preset::Scroll, false);
        let Target::Group(viewport) = node.target else {
            unreachable!()
        };
        let control = self.host.direct_control(
            viewport.node(),
            self.scope,
            crate::widget::UiaRole::None,
            windows_scene::HitFlags::SCROLL
                | windows_scene::HitFlags::INTERACTIVE
                | windows_scene::HitFlags::WHEEL,
        );
        self.host
            .controls
            .get_mut(control)
            .unwrap()
            .front
            .hover_scope = self.hover_scope;
        let mut content_ui = Ui {
            host: self.host,
            members: self.members,
            parent: Some(viewport),
            after: None,
            scope: self.scope,
            owner: self.owner,
            hover_scope: self.hover_scope,
        };
        let content = create(&mut content_ui);
        super::mount::install_scroll(
            self.host,
            viewport,
            content.target.id(),
            declaration,
            self.scope,
            viewport.node(),
        );
        Element {
            host: self.host,
            members: Some(self.members),
            node,
        }
    }

    pub fn region(
        &mut self,
        queue: windows_present::Queue,
        live: &crate::present::Live,
        build: impl FnOnce(
            &windows_present::Gpu,
            crate::present::Theme,
        ) -> windows_core::Result<Box<dyn windows_present::Frame>>
        + Send
        + 'static,
    ) -> Element<'_, super::Region> {
        let sink = self.host.model.region();
        let node = self.create(crate::present::PRESET, true);
        let Target::Sprite(sprite) = node.target else {
            unreachable!()
        };
        let control = self.host.direct_control(
            sprite.node(),
            self.scope,
            crate::widget::UiaRole::Graph,
            windows_scene::HitFlags::INTERACTIVE | windows_scene::HitFlags::GESTURE,
        );
        self.host
            .controls
            .get_mut(control)
            .unwrap()
            .front
            .hover_scope = self.hover_scope;
        let appearance = Appearance {
            id: sprite,
            mask: PaintMask::Box { radius: None },
            source: PaintSource::Region(sink),
            strength: 1.0,
            part: Part::Static,
            geom: None,
            scope: self.scope,
            surface: None,
            halo: None,
            next: NodeId::NONE,
            wash: false,
        };
        appearance.publish(self.host, true);
        self.host.appearances.place(sprite.node(), appearance);
        self.host.own_appearance(sprite, sprite.node());
        self.host.regions.place(
            sprite.node(),
            crate::present::RegionRow {
                theme: std::sync::Arc::new(crate::present::Published::new(self.scope)),
                sink,
                key: crate::present::key_of(sink),
                queue,
                live: live.clone(),
                control: Some(control),
                build: Some(Box::new(build)),
                extent: None,
            },
        );
        Element {
            host: self.host,
            members: Some(self.members),
            node,
        }
    }

    /// Creates a run from an immutable typography recipe, without an intermediate text seed.
    pub fn text(&mut self, style: TextStyle, source: impl Into<TextSource>) -> Element<'_> {
        self.text_owned(self.owner, style, source)
    }

    pub(crate) fn text_owned(
        &mut self,
        owner: Option<windows_scene::ControlId>,
        style: TextStyle,
        source: impl Into<TextSource>,
    ) -> Element<'_> {
        let source = source.into();
        let mut node = self.create(Preset::Text, style.flow == Flow::Line && !style.vertical);
        node.owner = owner;
        let (sprite, group) = match node.target {
            Target::Sprite(id) => (id, None),
            Target::Group(id) => (SpriteId::default(), Some(id)),
        };
        let ink = owner
            .and_then(|id| self.host.chrome(id))
            .map(|chrome| Role::Text(chrome.in_state(crate::widget::ModelState::Rest).text))
            .or(style.ink);
        if group.is_none() {
            let appearance = Appearance {
                id: sprite,
                mask: PaintMask::Bare,
                source: owner.filter(|id| self.host.chrome(*id).is_some()).map_or(
                    PaintSource::Role(ink.unwrap_or(Role::Text(Text::Primary))),
                    PaintSource::Owner,
                ),
                part: Part::Label,
                strength: 1.0,
                geom: None,
                scope: self.scope,
                surface: None,
                halo: None,
                next: NodeId::NONE,
                wash: false,
            };
            appearance.publish(self.host, true);
            self.host.appearances.place(sprite.node(), appearance);
            self.host.own_appearance(sprite, node.target.id());
        }
        let key = super::text::install(
            self.host,
            node.target.id(),
            super::text::Mint {
                text: super::text::Source::Static(""),
                ramp: style.typography,
                flow: style.flow,
                caps: style.caps,
                vertical: style.vertical,
                scope: self.scope,
                ink,
                sprite,
                group,
            },
            source,
        );
        if let Some(owner) = owner {
            let control = self.host.controls.get_mut(owner).unwrap();
            if control.text.is_none() {
                control.text = Some(key);
                self.host.uia_restale();
            }
        } else {
            let id = self.host.direct_control(
                node.target.id(),
                self.scope,
                crate::widget::UiaRole::Text,
                windows_scene::HitFlags::UIA,
            );
            let control = self.host.controls.get_mut(id).unwrap();
            control.text = Some(key);
            control.front.hover_scope = self.hover_scope;
        }
        Element {
            host: self.host,
            members: Some(self.members),
            node,
        }
    }

    /// Declares a single painted path. Its geometry remains a retained scene resource.
    pub fn path(&mut self, geometry: windows_scene::GeomId) -> Element<'_, Path> {
        let node = self.create(Preset::Bare, true);
        let Target::Sprite(id) = node.target else {
            unreachable!()
        };
        let appearance = Appearance {
            id,
            mask: PaintMask::Shape { stroke: None },
            source: PaintSource::Role(Role::Text(Text::Primary)),
            part: Part::Static,
            strength: 1.0,
            geom: Some(geometry),
            scope: self.scope,
            surface: None,
            halo: None,
            next: NodeId::NONE,
            wash: false,
        };
        appearance.publish(self.host, true);
        self.host.appearances.place(id.node(), appearance);
        self.host.own_appearance(id, id.node());
        Element {
            host: self.host,
            members: Some(self.members),
            node,
        }
    }

    pub fn plate(&mut self, radius: impl Into<Len>, role: Role, strength: f32) -> Element<'_> {
        let node = self.create(Preset::Bare, true);
        let Target::Sprite(id) = node.target else {
            unreachable!()
        };
        let appearance = Appearance {
            id,
            mask: PaintMask::Box {
                radius: Some(radius.into()),
            },
            source: PaintSource::Role(role),
            strength,
            part: Part::Fill,
            geom: None,
            scope: self.scope,
            surface: None,
            halo: None,
            next: NodeId::NONE,
            wash: false,
        };
        appearance.publish(self.host, true);
        self.host.appearances.place(id.node(), appearance);
        self.host.own_appearance(id, id.node());
        Element {
            host: self.host,
            members: Some(self.members),
            node,
        }
    }

    /// Updates several retained handles through the same deferred signal graph.
    pub fn effect(&mut self, mut update: impl FnMut(&mut Ui<'_>) + 'static) -> Effect {
        let owner = if self.members.first.is_none() {
            self.parent
                .map_or(self.host.model.root().node(), GroupId::node)
        } else {
            self.members.first
        };
        let scope = self.scope;
        self.host.binding(owner, move || {
            Host::with(|host| {
                let scope = scope.in_theme(host.root_scope);
                update(&mut Ui {
                    host,
                    members: &mut Members::default(),
                    parent: None,
                    after: None,
                    scope,
                    owner: None,
                    hover_scope: None,
                });
            })
        })
    }
}

impl<K> Element<'_, K> {
    pub fn id(self) -> Node<K> {
        self.node
    }

    pub fn layout(self, write: impl FnOnce(&mut Layout)) -> Self {
        let id = self.node.target.id();
        let class = self.host.model.solved(id).class;
        let recipe = self
            .host
            .styles
            .get_mut(id)
            .expect("a live element owns its declaration");
        write(&mut recipe.layout.base);
        self.host
            .model
            .style(id, &self.host.styles.lower(id, class).unwrap());
        self
    }

    pub fn layout_when(
        self,
        class: crate::role::WidthClass,
        write: impl FnOnce(&mut Layout),
    ) -> Self {
        let id = self.node.target.id();
        let active = self.host.model.solved(id).class;
        write(self.host.styles.at(id, class));
        self.host
            .model
            .style(id, &self.host.styles.lower(id, active).unwrap());
        self
    }

    pub fn responsive(self, bounds: [f32; 2]) -> Self {
        let Target::Group(group) = self.node.target else {
            panic!("a responsive scope requires a container")
        };
        self.host
            .model
            .responsive(group, windows_scene::Bounds(bounds));
        self
    }

    pub fn width(self, value: impl Into<Len>) -> Self {
        self.layout(|l| l.width = Some(value.into()))
    }
    pub fn height(self, value: impl Into<Len>) -> Self {
        self.layout(|l| l.height = Some(value.into()))
    }
    pub fn gap(self, value: impl Into<Len>) -> Self {
        self.layout(|l| l.gap = Some(value.into()))
    }
    pub fn padding(self, value: impl Into<Len>) -> Self {
        self.layout(|l| l.padding = Some([value.into(); 2]))
    }
    pub fn grow(self) -> Self {
        self.layout(|l| l.grow = Some(1.0))
    }

    pub fn opacity<M>(self, value: impl Signal<f32, M> + 'static) -> Self {
        self.channel(
            Prop::Opacity,
            crate::widget::Motion::Chrome,
            value,
            Value::Scalar,
        )
    }

    pub(crate) fn channel<T: Copy + 'static, M>(
        self,
        prop: Prop,
        motion: crate::widget::Motion,
        value: impl Signal<T, M> + 'static,
        map: impl Fn(T) -> Value + 'static,
    ) -> Self {
        let node = self.node.target.id();
        assert!(
            prop != Prop::Opacity
                || self
                    .node
                    .hover_scope
                    .and_then(|id| self.host.controls.get(id))
                    .is_none_or(|row| row.front.reveal != node),
            "an interaction reveal owns its opacity"
        );
        if let Some(owner) = self.node.owner.and_then(|id| self.host.controls.get(id)) {
            assert!(
                !owner
                    .front
                    .scalar_parts
                    .iter()
                    .flatten()
                    .any(|(id, part)| *id == node && part.property() == prop),
                "a scalar part owns its driven property"
            );
        }
        if value.is_constant() {
            self.host.set_channel(node, node, prop, map(value.read()));
        } else {
            self.host
                .bind_channel(node, node, prop, motion, move || map(value.read()));
        }
        self
    }

    pub fn layout_from(self, write: impl Fn(&mut Layout) + 'static) -> Self {
        let node = self.node.target.id();
        self.host
            .bind_to(node, super::binding::Destination::Layout, move || {
                Host::with(|host| {
                    let class = host.model.solved(node).class;
                    if let Some(recipe) = host.styles.get_mut(node) {
                        write(&mut recipe.layout.base);
                        host.model
                            .style(node, &host.styles.lower(node, class).unwrap());
                    }
                })
            });
        self
    }

    pub fn min_width(self, value: impl Into<Len>) -> Self {
        self.layout(|l| l.min_width = Some(value.into()))
    }
    pub fn min_height(self, value: impl Into<Len>) -> Self {
        self.layout(|l| l.min_height = Some(value.into()))
    }
    pub fn max_width(self, value: impl Into<Len>) -> Self {
        self.layout(|l| l.max_width = Some(value.into()))
    }
    pub fn no_shrink(self) -> Self {
        self.layout(|l| l.shrink = Some(0.0))
    }
    pub fn padding_xy(self, x: impl Into<Len>, y: impl Into<Len>) -> Self {
        self.layout(|l| l.padding = Some([x.into(), y.into()]))
    }
    pub fn align(self, align: crate::layout::Align) -> Self {
        self.layout(|l| l.align = Some(align))
    }
    pub fn justify(self, align: crate::layout::Align) -> Self {
        self.layout(|l| l.justify = Some(align))
    }
    pub fn cover(self) -> Self {
        self.layout(|l| l.position = Some(crate::layout::Position::Absolute([Len::Zero; 4])))
    }
    pub fn clip(self) -> Self {
        self.layout(|l| l.clip = Some(true))
    }
    pub fn probed(self, probe: crate::layout::Probe) -> Self {
        self.host.probes.place(self.node.target.id(), probe.cell());
        self
    }
    pub fn at(self, row: u16, column: u16) -> Self {
        self.layout(|l| {
            l.position = Some(crate::layout::Position::Grid {
                row,
                column,
                row_span: 1,
                column_span: 1,
            })
        })
    }
    pub fn rotation<M>(self, value: impl Signal<f32, M> + 'static) -> Self {
        self.channel(
            Prop::RotationAngle,
            crate::widget::Motion::Chrome,
            value,
            Value::Scalar,
        )
    }

    pub fn pivot_relative(self, fraction: windows_numerics::Vector2) -> Self {
        let node = self.node.target.id();
        self.host.relative_pivot(node, fraction);
        self
    }
}

impl Ui<'_> {
    /// Re-resolves retained appearance and typography without recreating content.
    pub fn set_theme(&mut self, root: Scope, backdrop: windows_scene::BackdropSpec) {
        self.host.set_theme(root, backdrop);
    }

    pub fn window_size(&self) -> crate::signal::Cell<windows_numerics::Vector2> {
        self.host.window_size
    }
    pub fn set_geometry(&mut self, id: windows_scene::GeomId, verbs: &[windows_scene::PathVerb]) {
        self.host.model.set_geometry(id, verbs);
    }
    pub fn ramp(
        &mut self,
        stops: &[super::Stop],
        spread: windows_scene::Spread,
    ) -> windows_scene::RampId {
        super::mount::resolve_stops(stops, self.host.root_scope, &mut self.host.ramp_stops);
        let id = self.host.model.ramp(&self.host.ramp_stops, spread);
        let mut held = self.host.ramp_pool.pop().unwrap_or_default();
        held.extend_from_slice(stops);
        self.host.ramps.place(id, (held, spread));
        crate::signal::Owner::retain(super::geometry::RampLease(super::geometry::Lease(
            id,
            self.host.identity,
        )));
        id
    }
    pub fn local_geometry(
        &mut self,
        bounds: crate::layout::Probe,
        capacity: usize,
        mut fill: impl FnMut(&mut Vec<windows_scene::PathVerb>, windows_numerics::Vector2, Scope)
        + 'static,
    ) -> windows_scene::GeomId {
        self.local_geometries(bounds, [capacity], move |[verbs], size, scope| {
            fill(verbs, size, scope)
        })[0]
    }
    pub fn local_geometries<const N: usize>(
        &mut self,
        bounds: crate::layout::Probe,
        capacities: [usize; N],
        mut fill: impl FnMut(&mut [Vec<windows_scene::PathVerb>; N], windows_numerics::Vector2, Scope)
        + 'static,
    ) -> [windows_scene::GeomId; N] {
        let ids = capacities.map(|_| self.geometry(&[]));
        let local = crate::signal::Memo::new(move || {
            let p = bounds.get();
            p.scope.map(|scope| (p.size, scope))
        });
        let mut paths = capacities.map(Vec::with_capacity);
        let runtime = self.host.identity;
        Effect::geometry(move || {
            if Host::try_with(|host| host.identity == runtime) != Some(true) {
                return;
            }
            let Some((size, scope)) = local.get() else {
                return;
            };
            paths.iter_mut().for_each(Vec::clear);
            crate::signal::read_only(|| fill(&mut paths, size, scope));
            for (id, verbs) in ids.into_iter().zip(&paths) {
                super::set_geometry(id, verbs);
            }
        });
        ids
    }
    pub fn paths_with<const N: usize, K>(
        &mut self,
        capacities: [usize; N],
        mut fill: impl FnMut(&mut [Vec<windows_scene::PathVerb>; N], windows_numerics::Vector2, Scope)
        + 'static,
        create: impl FnOnce(&mut Ui<'_>, [windows_scene::GeomId; N]) -> Node<K>,
    ) -> Element<'_, K> {
        let ids = capacities.map(|_| self.geometry(&[]));
        let mut paths = capacities.map(Vec::with_capacity);
        let node = create(self, ids);
        super::geometry::install(
            self.host,
            node.target.id(),
            Box::new(move |size, scope| {
                paths.iter_mut().for_each(Vec::clear);
                crate::signal::read_only(|| fill(&mut paths, size, scope));
                for (id, verbs) in ids.into_iter().zip(&paths) {
                    super::set_geometry(id, verbs);
                }
            }),
        );
        self.edit(node).unwrap()
    }
}
impl<K> Element<'_, K> {
    /// Runs the body synchronously with this parent's declared scope and control ownership.
    pub fn children(mut self, create: impl FnOnce(&mut Ui<'_>)) -> Self {
        let Target::Group(group) = self.node.target else {
            panic!("children require a container")
        };
        let scope = self.host.styles.get(group.node()).unwrap().scope;
        let control = self.host.mounts.get(group.node()).unwrap().control;
        let hover_scope = control
            .filter(|id| self.host.controls.get(*id).unwrap().front.hover_scope == Some(*id))
            .or(self.node.hover_scope);
        let after = self.host.model.last_child(group.node());
        create(&mut Ui {
            host: self.host,
            members: self
                .members
                .as_deref_mut()
                .expect("construction requires a creation transaction"),
            parent: Some(group),
            after,
            scope,
            owner: control.or(self.node.owner),
            hover_scope,
        });
        self
    }
    pub fn row(self, create: impl FnOnce(&mut Ui<'_>)) -> Self {
        self.layout(|l| l.flow = Some(Preset::Row)).children(create)
    }
    pub fn stack(self, create: impl FnOnce(&mut Ui<'_>)) -> Self {
        self.layout(|l| l.flow = Some(Preset::Stack))
            .children(create)
    }
    pub fn grid(self, create: impl FnOnce(&mut Ui<'_>)) -> Self {
        self.layout(|l| l.flow = Some(Preset::Grid))
            .children(create)
    }
    pub fn wrap(self, create: impl FnOnce(&mut Ui<'_>)) -> Self {
        self.layout(|l| l.flow = Some(Preset::Wrap))
            .children(create)
    }
    pub fn tiles(self, min: impl Into<Len>, create: impl FnOnce(&mut Ui<'_>)) -> Self {
        self.layout(|l| {
            l.flow = Some(Preset::Tiles);
            l.tile_min = Some(min.into());
        })
        .children(create)
    }
    pub fn cols(self, tracks: impl IntoIterator<Item = crate::layout::Track>) -> Self {
        self.layout(|l| {
            let out = l.columns();
            out.clear();
            out.extend(tracks);
        })
    }
    pub fn rows(self, tracks: impl IntoIterator<Item = crate::layout::Track>) -> Self {
        self.layout(|l| {
            let out = l.rows();
            out.clear();
            out.extend(tracks);
        })
    }
    pub fn span(self, row: u16, column: u16, row_span: u16, column_span: u16) -> Self {
        self.layout(|l| {
            l.position = Some(crate::layout::Position::Grid {
                row,
                column,
                row_span,
                column_span,
            })
        })
    }
    pub fn align_self(self, value: crate::layout::Align) -> Self {
        self.layout(|l| l.align_self = Some(value))
    }
    pub fn max_height(self, value: impl Into<Len>) -> Self {
        self.layout(|l| l.max_height = Some(value.into()))
    }
    pub fn stack_when(self, class: crate::role::WidthClass) -> Self {
        self.layout_when(class, |l| l.flow = Some(Preset::Stack))
    }
    pub fn cols_when(
        self,
        class: crate::role::WidthClass,
        tracks: impl IntoIterator<Item = crate::layout::Track>,
    ) -> Self {
        self.layout_when(class, |l| {
            let out = l.columns();
            out.clear();
            out.extend(tracks);
        })
    }
    pub fn width_when(self, class: crate::role::WidthClass, value: impl Into<Len>) -> Self {
        self.layout_when(class, |l| l.width = Some(value.into()))
    }
    pub fn min_width_when(self, class: crate::role::WidthClass, value: impl Into<Len>) -> Self {
        self.layout_when(class, |l| l.min_width = Some(value.into()))
    }
    pub fn max_width_when(self, class: crate::role::WidthClass, value: impl Into<Len>) -> Self {
        self.layout_when(class, |l| l.max_width = Some(value.into()))
    }
    pub fn hide_when(self, class: crate::role::WidthClass) -> Self {
        self.layout_when(class, |l| l.hidden = Some(true))
    }
    pub fn hide_below(mut self, class: crate::role::WidthClass) -> Self {
        for at in [
            crate::role::WidthClass::Narrow,
            crate::role::WidthClass::Medium,
            crate::role::WidthClass::Wide,
        ] {
            if at < class {
                self = self.hide_when(at);
            }
        }
        self
    }
    pub fn float_when(self, class: crate::role::WidthClass, edge: crate::layout::Edge) -> Self {
        self.layout_when(class, |l| {
            l.position = Some(crate::layout::Position::Edge(edge))
        })
    }
    pub fn float_below(
        mut self,
        class: crate::role::WidthClass,
        edge: crate::layout::Edge,
    ) -> Self {
        for at in [
            crate::role::WidthClass::Narrow,
            crate::role::WidthClass::Medium,
            crate::role::WidthClass::Wide,
        ] {
            if at < class {
                self = self.float_when(at, edge);
            }
        }
        self
    }
    pub fn pinned(self, insets: [Len; 4]) -> Self {
        self.layout(|l| l.position = Some(crate::layout::Position::Absolute(insets)))
    }
    pub fn band(self, at: impl Into<Len>, height: impl Into<Len>) -> Self {
        self.layout(|l| {
            l.position = Some(crate::layout::Position::Band {
                at: at.into(),
                height: height.into(),
            })
        })
    }
    pub fn hide_if<M>(self, value: impl Signal<bool, M> + 'static) -> Self {
        if value.is_constant() {
            self.host
                .replace_binding(self.node.target.id(), super::binding::Destination::Hidden);
            self.layout(|l| l.hidden = Some(value.read()))
        } else {
            let node = self.node.target.id();
            self.host
                .bind_to(node, super::binding::Destination::Hidden, move || {
                    let hidden = value.read();
                    Host::with(|h| {
                        let class = h.model.solved(node).class;
                        let recipe = h.styles.get_mut(node).unwrap();
                        recipe.layout.base.hidden = Some(hidden);
                        h.model.style(node, &h.styles.lower(node, class).unwrap());
                    });
                });
            self
        }
    }
    pub fn pivot<M>(self, value: impl Signal<windows_numerics::Vector2, M> + 'static) -> Self {
        self.channel(
            Prop::Center,
            crate::widget::Motion::Chrome,
            value,
            Value::Vec2,
        )
    }
}

impl<K> Element<'_, K> {
    fn decorate(self, mask: PaintMask, source: PaintSource, part: Part, strength: f32) -> Self {
        let node = self.node.target.id();
        let scope = self.host.styles.get(node).unwrap().scope;
        let id = match self.node.target {
            Target::Sprite(id) => id,
            Target::Group(group) => self.host.model.visual(group, None),
        };
        let existing = self.host.appearances.get(id.node()).copied();
        if matches!(self.node.target, Target::Group(_)) {
            self.host.model.visual_insets(id, [0.0; 4]);
        }
        let appearance = Appearance {
            id,
            mask,
            source,
            part,
            strength,
            geom: existing.and_then(|a| a.geom),
            scope,
            surface: None,
            halo: existing.and_then(|a| a.halo),
            next: existing.map_or(NodeId::NONE, |a| a.next),
            wash: false,
        };
        appearance.publish(self.host, true);
        self.host.appearances.place(id.node(), appearance);
        if existing.is_none() {
            self.host.own_appearance(id, node);
        }
        if let Some(owner) = self.node.owner {
            self.host.control_part(owner, part, id);
        }
        self
    }
    pub fn plate(self, radius: impl Into<Len>, role: Role, strength: f32) -> Self {
        self.decorate(
            PaintMask::Box {
                radius: Some(radius.into()),
            },
            PaintSource::Role(role),
            Part::Fill,
            strength,
        )
    }
    pub fn outline(self, radius: crate::role::Metric, role: Role, width: impl Into<Len>) -> Self {
        self.decorate(
            PaintMask::Border {
                radius,
                width: width.into(),
            },
            PaintSource::Role(role),
            Part::Border,
            1.0,
        )
    }
    pub fn washed(self, id: windows_scene::RampId, radius: crate::role::Metric) -> Self {
        self.decorate(
            PaintMask::Box {
                radius: Some(radius.into()),
            },
            PaintSource::Gradient(id),
            Part::Fill,
            1.0,
        )
    }
    pub fn elevate(self, elevation: crate::role::Elevation) -> Self {
        let node = self.node.target.id();
        let recipe = self.host.styles.get_mut(node).unwrap();
        recipe.scope = recipe.scope.elevate(elevation);
        let scope = recipe.scope;
        self.host
            .model
            .style(node, &self.host.styles.lower(node, scope.width).unwrap());
        let mut paint = self.host.mounts.get(node).unwrap().paints;
        while let Some(row) = self.host.appearances.get_mut(paint) {
            row.scope = scope;
            let row = *row;
            paint = row.next;
            row.publish(self.host, true);
        }
        self
    }
    pub fn appearance(self, chrome: crate::widget::Chrome) -> Self {
        let Target::Group(group) = self.node.target else {
            panic!("chrome requires a surface")
        };
        self.host.declare_surface(group, chrome);
        self
    }
    pub fn ghost(self) -> Self {
        self.appearance(crate::widget::Chrome::new(
            crate::widget::roles::BUTTON[crate::widget::roles::GHOST as usize],
            crate::role::Metric::Radius,
        ))
    }
    pub fn accent(self) -> Self {
        self.appearance(crate::widget::Chrome::new(
            crate::widget::roles::BUTTON[crate::widget::roles::ACCENT as usize],
            crate::role::Metric::Radius,
        ))
    }
    pub fn accent_subtle(self) -> Self {
        self.appearance(crate::widget::Chrome::new(
            crate::widget::roles::BUTTON[crate::widget::roles::ACCENT_SUBTLE as usize],
            crate::role::Metric::Radius,
        ))
    }
    fn halo_style(self, halo: super::theme::HaloStyle) -> Self {
        match self.node.target {
            Target::Group(group) => self.host.surface_halo(group, halo),
            Target::Sprite(sprite) => {
                if let Some(paint) = self.host.appearances.get_mut(sprite.node()) {
                    paint.halo = Some((
                        halo,
                        if paint.part == Part::Fill {
                            crate::role::Silhouette::Area
                        } else {
                            crate::role::Silhouette::Ink
                        },
                    ));
                    let paint = *paint;
                    paint.publish(self.host, false);
                }
            }
        }
        self
    }
    pub fn shadowed(self, edge: crate::layout::Edge) -> Self {
        self.host
            .replace_binding(self.node.target.id(), super::binding::Destination::Halo);
        self.halo_style(super::theme::HaloStyle::Shadow(edge))
    }
    pub fn halo<M>(self, role: impl Signal<Role, M> + 'static) -> Self
    where
        K: 'static,
    {
        if role.is_constant() {
            self.host
                .replace_binding(self.node.target.id(), super::binding::Destination::Halo);
            self.halo_style(super::theme::HaloStyle::Glow(role.read()))
        } else {
            let node = self.node;
            self.host.bind_to(
                node.target.id(),
                super::binding::Destination::Halo,
                move || {
                    let role = role.read();
                    Host::with(|host| {
                        Element {
                            host,
                            members: None,
                            node,
                        }
                        .halo_style(super::theme::HaloStyle::Glow(role));
                    });
                },
            );
            self
        }
    }
    pub fn halo_lit<M>(self, value: impl Signal<f32, M> + 'static) -> Self {
        self.channel(
            Prop::ShadowOpacity,
            crate::widget::Motion::Chrome,
            value,
            Value::Scalar,
        )
    }
}
impl Element<'_, Path> {
    pub fn fill(self, role: crate::role::DataRole) -> Self {
        self.decorate(
            PaintMask::Shape { stroke: None },
            PaintSource::Role(Role::Data(role)),
            Part::Fill,
            1.0,
        )
    }
    pub fn stroke(self, role: impl Into<Role>, width: impl Into<Len>) -> Self {
        self.decorate(
            PaintMask::Shape {
                stroke: Some(width.into()),
            },
            PaintSource::Role(role.into()),
            Part::Border,
            1.0,
        )
    }
    pub fn fill_ramp(self, id: windows_scene::RampId) -> Self {
        self.decorate(
            PaintMask::Shape { stroke: None },
            PaintSource::Gradient(id),
            Part::Fill,
            1.0,
        )
    }
    pub fn stroke_ramp(self, id: windows_scene::RampId, width: impl Into<Len>) -> Self {
        self.decorate(
            PaintMask::Shape {
                stroke: Some(width.into()),
            },
            PaintSource::Gradient(id),
            Part::Border,
            1.0,
        )
    }
    pub fn ink(self) -> Self {
        self.ink_paint(None)
    }
    pub fn ink_stroke(self, width: impl Into<Len>) -> Self {
        self.ink_paint(Some(width.into()))
    }
    fn ink_paint(self, stroke: Option<Len>) -> Self {
        let source = self
            .node
            .owner
            .filter(|id| self.host.chrome(*id).is_some())
            .map_or(
                PaintSource::Role(Role::Text(Text::Primary)),
                PaintSource::Owner,
            );
        self.decorate(PaintMask::Shape { stroke }, source, Part::Label, 1.0)
    }
    pub fn line(self, role: crate::role::Stroke) -> Self {
        self.stroke(Role::Stroke(role), crate::role::Metric::HairlineW)
    }
    pub fn line_stroke(self, role: crate::role::Stroke, width: impl Into<Len>) -> Self {
        self.stroke(Role::Stroke(role), width)
    }
    pub fn trim<M>(self, value: impl Signal<f32, M> + 'static) -> Self {
        self.channel(
            Prop::TrimEnd,
            crate::widget::Motion::Chrome,
            value,
            Value::Scalar,
        )
    }
    pub fn stroke_width<M>(self, value: impl Signal<f32, M> + 'static) -> Self {
        self.channel(
            Prop::StrokeThickness,
            crate::widget::Motion::Snap,
            value,
            Value::Scalar,
        )
    }
    pub(crate) fn slider_trail(
        self,
        origin: f32,
        ramp: Option<windows_scene::RampId>,
        width: impl Into<Len>,
    ) -> Self {
        let source = ramp.map_or(
            PaintSource::Role(Role::Fill(crate::role::Fill::Accent)),
            PaintSource::Gradient,
        );
        self.decorate(
            PaintMask::Shape {
                stroke: Some(width.into()),
            },
            source,
            Part::Trail { origin },
            1.0,
        )
    }
}

impl Element<'_, super::Region> {
    pub(crate) fn region_radius(self, radius: Len) -> Self {
        let paint = self
            .host
            .appearances
            .get_mut(self.node.target.id())
            .unwrap();
        paint.mask = PaintMask::Box {
            radius: Some(radius.into()),
        };
        let paint = *paint;
        paint.publish(self.host, true);
        self
    }
}
