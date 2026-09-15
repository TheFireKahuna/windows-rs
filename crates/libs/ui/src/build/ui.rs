//! Borrowed parent-first authoring. Constructors write retained records immediately.
use super::arena::{MaskSeed, Part};
use super::host::MountRow;
use super::style::{Declaration, Recipe};
use super::theme::{Appearance, PaintSource};
use super::{Any, Host, Mount, Path};
use crate::layout::{Layout, Len, Preset};
use crate::role::{Role, Scope, Text};
use crate::signal::{Effect, Signal};
use crate::widget::{Flow, TextSource, TextStyle};
use core::marker::PhantomData;
use windows_scene::{Anim, Bind, GroupId, NodeId, Prop, SpriteId, Tuning, Value};

#[derive(Copy, Clone, Debug)]
enum Target {
    Group(GroupId),
    Sprite(SpriteId),
}
impl Target {
    fn id(self) -> NodeId {
        match self {
            Self::Group(id) => id.node(),
            Self::Sprite(id) => id.node(),
        }
    }
}

/// Opaque retained identity. Both the node generation and runtime lifetime are checked on edit.
pub struct Node<K = Any> {
    target: Target,
    runtime: u64,
    kind: PhantomData<fn() -> K>,
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
#[must_use]
pub struct Element<'a, K = Any> {
    host: &'a mut Host,
    node: Node<K>,
}

#[derive(Default)]
struct Members {
    first: NodeId,
    last: NodeId,
}
impl Members {
    fn push(&mut self, host: &mut Host, id: NodeId) {
        if let Some(previous) = host.mounts.get_mut(self.last) {
            previous.next = id;
        } else {
            self.first = id;
        }
        self.last = id;
        host.mounts.place(id, MountRow::new(id));
    }
}

/// The authoring and update context. Construction is available only in creation transactions.
pub struct Ui<'a> {
    host: &'a mut Host,
    members: &'a mut Members,
    parent: Option<GroupId>,
    after: Option<NodeId>,
    scope: Scope,
}

impl Ui<'_> {
    pub(crate) fn mount_root(create: impl FnOnce(&mut Ui<'_>)) -> Mount {
        Host::with(|host| {
            let root = host.model.root();
            let mut members = Members::default();
            members.push(host, root.node());
            let scope = host.root_scope;
            create(&mut Ui {
                host,
                members: &mut members,
                parent: Some(root),
                after: None,
                scope,
            });
            Mount::new(root.node(), members.first)
        })
    }

    /// Returns the lexical design scope; width-dependent values resolve during solving.
    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// Returns absence for a retired generation or a handle from another runtime.
    pub fn edit<K>(&mut self, node: Node<K>) -> Option<Element<'_, K>> {
        (node.runtime == self.host.identity && self.host.mounts.get(node.target.id()).is_some())
            .then_some(Element {
                host: self.host,
                node,
            })
    }

    fn create<K>(&mut self, preset: Preset, sprite: bool) -> Node<K> {
        let parent = self
            .parent
            .expect("structure requires a creation transaction");
        let target = if sprite {
            Target::Sprite(self.host.model.sprite(parent, self.after))
        } else {
            Target::Group(self.host.model.group(parent, self.after))
        };
        let id = target.id();
        self.after = Some(id);
        self.members.push(self.host, id);
        let recipe = Recipe {
            preset,
            scope: self.scope,
            layout: Declaration::default(),
        };
        self.host.model.style(id, &recipe.lower(self.scope.width));
        self.host.styles.place(id, recipe);
        Node {
            target,
            runtime: self.host.identity,
            kind: PhantomData,
        }
    }

    /// Creates the parent before synchronously declaring its children.
    pub fn group(&mut self, preset: Preset, children: impl FnOnce(&mut Ui<'_>)) -> Element<'_> {
        let node = self.create(preset, false);
        let Target::Group(parent) = node.target else {
            unreachable!()
        };
        children(&mut Ui {
            host: self.host,
            members: self.members,
            parent: Some(parent),
            after: None,
            scope: self.scope,
        });
        Element {
            host: self.host,
            node,
        }
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

    /// Creates a run from an immutable typography recipe, without an intermediate text seed.
    pub fn text(&mut self, style: TextStyle, source: impl Into<TextSource>) -> Element<'_> {
        let source = source.into();
        let node = self.create(Preset::Text, style.flow == Flow::Line);
        let (sprite, group) = match node.target {
            Target::Sprite(id) => (id, None),
            Target::Group(id) => (SpriteId::default(), Some(id)),
        };
        if group.is_none() {
            let appearance = Appearance {
                id: sprite,
                mask: MaskSeed::Bare,
                source: PaintSource::Role(style.ink.unwrap_or(Role::Text(Text::Primary))),
                part: Part::Label,
                strength: 1.0,
                geom: None,
                scope: self.scope,
                chrome: None,
                halo: None,
                next: NodeId::NONE,
                wash: false,
            };
            appearance.publish(self.host, true);
            self.host.appearances.place(sprite.node(), appearance);
            self.host.own_appearance(sprite, node.target.id(), None);
        }
        let (initial, read) = match source {
            TextSource::Static(text) => (super::text::Source::Static(text), None),
            TextSource::Owned(text) => (super::text::Source::Owned(text), None),
            TextSource::Dynamic(read) => (super::text::Source::Static(""), Some(read)),
        };
        let key = self.host.text.mint(super::text::Mint {
            text: initial,
            ramp: style.typography,
            flow: style.flow,
            caps: style.caps,
            vertical: false,
            scope: self.scope,
            ink: style.ink,
            sprite,
            group,
        });
        self.host.mounts.get_mut(node.target.id()).unwrap().text = Some(key);
        self.host
            .model
            .measure(node.target.id(), windows_scene::MeasureCtx::Measured(key));
        if let Some(read) = read {
            let mut text = String::new();
            self.host.binding(node.target.id(), move || {
                text.clear();
                read(&mut text);
                Host::with(|host| {
                    if let Some(id) = host.text.set_text(key, &text) {
                        host.model.remeasure(id);
                        host.uia_restale();
                    }
                });
            });
        }
        Element {
            host: self.host,
            node,
        }
    }

    /// Declares a single painted path. Its geometry remains a retained scene resource.
    pub fn path(
        &mut self,
        geometry: windows_scene::GeomId,
        role: Role,
        stroke: Option<Len>,
    ) -> Element<'_, Path> {
        let node = self.create(Preset::Bare, true);
        let Target::Sprite(id) = node.target else {
            unreachable!()
        };
        let appearance = Appearance {
            id,
            mask: MaskSeed::Shape { stroke },
            source: PaintSource::Role(role),
            part: if stroke.is_some() {
                Part::Border
            } else {
                Part::Fill
            },
            strength: 1.0,
            geom: Some(geometry),
            scope: self.scope,
            chrome: None,
            halo: None,
            next: NodeId::NONE,
            wash: false,
        };
        appearance.publish(self.host, true);
        self.host.appearances.place(id.node(), appearance);
        self.host.own_appearance(id, id.node(), None);
        Element {
            host: self.host,
            node,
        }
    }

    /// Updates several retained handles through the same deferred signal graph.
    pub fn effect(&mut self, mut update: impl FnMut(&mut Ui<'_>) + 'static) -> Effect {
        let owner = self
            .parent
            .map_or(self.host.model.root().node(), GroupId::node);
        let scope = self.scope;
        self.host.binding(owner, move || {
            Host::with(|host| {
                update(&mut Ui {
                    host,
                    members: &mut Members::default(),
                    parent: None,
                    after: None,
                    scope,
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
        self.host.model.style(id, &recipe.lower(class));
        self
    }

    pub fn layout_when(
        self,
        class: crate::role::WidthClass,
        write: impl FnOnce(&mut Layout),
    ) -> Self {
        let id = self.node.target.id();
        let active = self.host.model.solved(id).class;
        let recipe = self
            .host
            .styles
            .get_mut(id)
            .expect("a live element owns its declaration");
        write(recipe.layout.at(Some(class)));
        self.host.model.style(id, &recipe.lower(active));
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
        let node = self.node.target.id();
        if value.is_constant() {
            self.host
                .model
                .bind(node, Prop::Opacity, Bind::Set(Value::Scalar(value.read())));
        } else {
            let mut previous = None;
            self.host.binding(node, move || {
                let value = Value::Scalar(value.read());
                if previous == Some(value) {
                    return;
                }
                let bind = if previous.is_none() {
                    Bind::Set(value)
                } else {
                    Bind::Animate(Anim::Spring {
                        to: value,
                        tuning: Tuning::Chrome,
                        delay_ms: 0,
                    })
                };
                previous = Some(value);
                Host::with(|host| host.model.bind(node, Prop::Opacity, bind));
            });
        }
        self
    }
}
