//! Borrowed parent-first authoring. Constructors write retained records immediately.
//!
//! A constructor mints the node, links it under the current parent and returns an `Element`
//! borrowing the context; chained setters edit columns in place. There is no temporary tree,
//! no child collector and no per-element runtime object.

use super::host::Host;
use super::mount::Mount;
use super::tree;
use crate::layout::{Align, Anchors, Layout, Len, Preset, Probe, Track, WidthClass};
use crate::role::{Scope, ScopedToken};
use crate::signal::{Effect, Signal};
use crate::structure::{Branch, Keyed, Step};
use core::marker::PhantomData;
use windows_numerics::Vector2;
use windows_scene::{ControlId, GeomId, GroupId, NodeId, PathVerb, Prop, Value};

/// Opaque retained identity.
///
/// The node generation is checked on every edit, so a handle held across an unmount reads
/// back absence rather than whatever now occupies the slot.
pub struct Node<K = super::Any> {
    pub(crate) id: NodeId,
    kind: PhantomData<fn() -> K>,
}

impl<K> Copy for Node<K> {}

impl<K> Clone for Node<K> {
    fn clone(&self) -> Self {
        *self
    }
}

// A fixture names what it built by handle and asks the arena about it by id.
#[cfg(test)]
impl<K> From<Node<K>> for NodeId {
    fn from(node: Node<K>) -> Self {
        node.id
    }
}

impl<K> core::fmt::Debug for Node<K> {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.id.fmt(out)
    }
}

/// A short-lived writer borrowing the runtime. Keep `id()` when later edits are needed.
pub struct Element<'a, K = super::Any> {
    pub(crate) ui: Ui<'a>,
    pub(crate) node: NodeId,
    kind: PhantomData<fn() -> K>,
}

/// The authoring and update context. Construction is available only in creation transactions.
pub struct Ui<'a> {
    pub(crate) host: &'a mut Host,
    /// The roots this transaction owns, which the retirement walk reads. Distinct from layout
    /// ancestry, since a keyed result may own several siblings and an overlay a detached root.
    pub(crate) members: &'a mut Vec<NodeId>,
    pub(crate) parent: NodeId,
    /// The sibling the next mint is ordered above. Carried rather than re-derived, because
    /// the links carry no last-child pointer and finding one is a walk.
    pub(crate) after: Option<NodeId>,
    pub(crate) scope: u32,
    /// The nearest enclosing control, which a part attaches itself to.
    pub(crate) control: ControlId,
    /// Whether a node minted here is one of this transaction's roots.
    pub(crate) root: bool,
}

impl<'a> Ui<'a> {
    /// Runs `create` against a fresh transaction under `parent`, above `after`.
    ///
    /// Takes a whole `Scope` and interns it, which is what a fixture holds; production mounts
    /// through [`mount_interned`](Self::mount_interned), whose caller already has the index.
    #[cfg(test)]
    pub(crate) fn mount_at(
        parent: NodeId,
        after: Option<NodeId>,
        scope: Scope,
        control: ControlId,
        create: impl FnOnce(&mut Ui<'_>),
    ) -> Mount {
        Host::with(|host| {
            let root = host.root_scope();
            let scope = host.intern(scope.in_theme(root));
            Self::transact(host, parent, after, scope, control, create)
        })
    }

    /// The same, against a scope this host has already interned.
    pub(crate) fn mount_interned(
        parent: NodeId,
        after: Option<NodeId>,
        scope: u32,
        control: ControlId,
        create: impl FnOnce(&mut Ui<'_>),
    ) -> Mount {
        Host::with(|host| Self::transact(host, parent, after, scope, control, create))
    }

    fn transact(
        host: &mut Host,
        parent: NodeId,
        after: Option<NodeId>,
        scope: u32,
        control: ControlId,
        create: impl FnOnce(&mut Ui<'_>),
    ) -> Mount {
        // Taken from the pool a retired mount returns its list to, so a warm mount of a
        // screen already built once allocates nothing.
        let mut members = host.take_roots();
        create(&mut Ui { host, members: &mut members, parent, after, scope, control, root: true });
        Mount::new(members)
    }

    /// Runs `create` against the window root, which outlives the transaction.
    pub(crate) fn mount_root(create: impl FnOnce(&mut Ui<'_>)) -> Mount {
        Host::with(|host| {
            let (root, scope) = (host.root(), 0);
            let mut members = host.take_roots();
            create(&mut Ui {
                host,
                members: &mut members,
                parent: root,
                after: None,
                scope,
                control: ControlId::NONE,
                root: false,
            });
            members.clear();
            members.push(root);
            Mount::rooted(members)
        })
    }

    /// Returns the lexical design scope; width-dependent values resolve during solving.
    pub fn scope(&self) -> Scope {
        self.host.scope_at(self.scope)
    }

    pub fn window_size(&self) -> crate::signal::Cell<Vector2> {
        self.host.window
    }

    /// Mints a node under the current parent and returns a writer for it.
    ///
    /// Allocates the parent immediately; chained declarations precede `children`.
    pub fn node(&mut self, preset: Preset) -> Element<'_> {
        let id = self.mint(preset, false);
        self.element(id)
    }

    /// Mints a painted node under the current parent.
    pub fn sprite(&mut self, preset: Preset) -> Element<'_> {
        let id = self.mint(preset, true);
        self.element(id)
    }

    fn mint(&mut self, preset: Preset, sprite: bool) -> NodeId {
        let parent = GroupId(self.parent);
        let id = if sprite {
            self.host.sprite(parent, self.after).0
        } else {
            self.host.group(parent, self.after).0
        };
        self.host.tree.c.scope[id.index()] = self.scope;
        self.host.tree.c.layout[id.index()] = Layout::of(preset);
        self.host.tree.adopt_layout_bits(id);
        self.after = Some(id);
        if self.root {
            self.members.push(id);
        }
        id
    }

    pub(crate) fn element<K>(&mut self, node: NodeId) -> Element<'_, K> {
        Element {
            ui: Ui {
                host: &mut *self.host,
                members: &mut *self.members,
                parent: self.parent,
                after: self.after,
                scope: self.scope,
                control: self.control,
                root: self.root,
            },
            node,
            kind: PhantomData,
        }
    }

    pub fn group(&mut self, preset: Preset, create: impl FnOnce(&mut Ui<'_>)) -> Element<'_> {
        self.node(preset).children(create)
    }

    pub fn row(&mut self, create: impl FnOnce(&mut Ui<'_>)) -> Element<'_> {
        self.group(Preset::Row, create)
    }

    pub fn stack(&mut self, create: impl FnOnce(&mut Ui<'_>)) -> Element<'_> {
        self.group(Preset::Stack, create)
    }

    pub fn grid(&mut self, create: impl FnOnce(&mut Ui<'_>)) -> Element<'_> {
        self.group(Preset::Grid, create)
    }

    pub fn layer(&mut self, create: impl FnOnce(&mut Ui<'_>)) -> Element<'_> {
        self.group(Preset::Layer, create)
    }

    /// Declares a single painted path. Its geometry remains a retained scene resource.
    pub fn path(&mut self, geometry: GeomId) -> Element<'_, super::Path> {
        let id = self.mint(Preset::Figure, true);
        // Recorded rather than sent: the paint declaration that follows emits the mask, with
        // the stroke only it knows.
        self.host.appearances.set_shape(id, geometry);
        self.element(id)
    }

    /// Declares a presentation region: one sprite painting a buffer the present thread draws.
    ///
    /// The node is an ordinary element in every other respect: it takes a size from its
    /// container, it can be placed in a grid, and chrome may sit over it. Nothing may sit under
    /// it: a region that lets the ground show through its own box is composed every frame
    /// instead of flipping.
    ///
    /// `queue` is the presentation queue this region asks for. [`windows_present::Queue::Solo`]
    /// is for the one surface whose plane the layout protects; everything else shares one named
    /// queue, so a second per-frame surface degrades its own company and never that one.
    /// `build` runs on the present thread with that thread's `Gpu`.
    ///
    /// The node carries a hit entry, so a contact resolves to the region through the one hit
    /// array like any control. Which part of it the contact landed on is resolved by
    /// [`crate::present::pick`] after the region has won.
    pub fn region(
        &mut self,
        queue: windows_present::Queue,
        live: &crate::present::Live,
        build: impl FnOnce(&windows_present::Gpu, crate::present::Theme) -> windows_core::Result<Box<dyn windows_present::Frame>>
            + Send
            + 'static,
    ) -> Element<'_, super::Region> {
        let node = self.mint(crate::present::PRESET, true);
        let scope = self.host.scope_at(self.scope);
        let sink = self.host.region();
        let control = self
            .host
            .mint_control(super::control::ControlRow::blank(node, scope));
        // Automation has no control type for a drawn surface, so a region is a graph unless
        // its author says otherwise: it reports a value and it holds parts.
        if let Some(row) = self.host.control_mut(control) {
            row.uia = crate::widget::UiaRole::Graph;
        }
        self.host.hit(
            node,
            Some(windows_scene::HitDecl {
                flags: windows_scene::HitFlags::INTERACTIVE,
                id: control,
                touch_inflate: None,
            }),
        );
        let at = self.host.regions.place(crate::present::RegionRow {
            node,
            sink,
            control,
            queue,
            live: live.clone(),
            theme: std::sync::Arc::new(crate::present::Published::new(scope)),
            build: Some(Box::new(build)),
            extent: None,
            active: false,
            layout: None,
        });
        self.host.set_region_row(node, at);
        self.host.declare_part(
            node,
            super::theme::Part::Ink,
            super::theme::PaintSource::Region(sink),
            super::theme::PaintMask::Region,
            1.0,
        );
        self.element(node)
    }

    /// Returns absence for a retired generation.
    pub fn edit<K>(&mut self, node: Node<K>) -> Option<Element<'_, K>> {
        if self.host.tree.is_live(node.id) {
            Some(self.element(node.id))
        } else {
            None
        }
    }

    /// Mints the hidden node the results of a structural combinator are ordered above.
    ///
    /// A `Layer` because it is the container with no opinion, and hidden because it takes no
    /// space: what it carries is a position in its parent's child list.
    fn anchor(&mut self) -> NodeId {
        let id = self.mint(Preset::Layer, false);
        self.host.tree.set_flag(id, tree::HIDDEN, true);
        id
    }

    /// Retains keyed results and moves only roots outside the surviving order.
    pub fn each<T: 'static, K: Eq + core::hash::Hash + Clone + 'static>(
        &mut self,
        fill: impl Fn(&mut Vec<T>) + 'static,
        key: impl Fn(&T) -> &K + 'static,
        create: impl Fn(&mut Ui<'_>, &T) + 'static,
    ) {
        let anchor = self.anchor();
        let (parent, scope, control) = (self.parent, self.scope, self.control);
        let mut list = Keyed::<K, Mount>::new();
        let mut items = Vec::new();
        self.host.binding(move || {
            items.clear();
            fill(&mut items);
            let mut after = Some(anchor);
            list.reconcile(
                &items,
                &key,
                |item| {
                    Ui::mount_interned(parent, Some(anchor), scope, control, |ui| {
                        create(ui, item);
                    })
                },
                |mount, step| {
                    // A survivor already in the right place is a rebind and nothing
                    // structural, so only the moves reach the splice.
                    if step != Step::Keep {
                        Host::with(|host| mount.place(host, parent, after));
                    }
                    after = mount.last().or(after);
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
            return;
        }
        self.switch_on(move || condition.read().then_some(()), move |ui, ()| create(ui));
    }

    pub fn switch<K: PartialEq + 'static>(
        &mut self,
        key: impl Fn() -> K + 'static,
        create: impl Fn(&mut Ui<'_>, &K) + 'static,
    ) {
        self.switch_on(move || Some(key()), create);
    }

    /// Mounts conditional content with a measured-size compositor slide on entry and exit.
    ///
    /// Input is suspended until entry completes. Removal releases layout space immediately;
    /// the compositor retains an unpickable exit ghost until its animation completes.
    pub fn when_slide<M>(
        &mut self,
        condition: impl Signal<bool, M> + 'static,
        slide: crate::overlay::Slide,
        create: impl Fn(&mut Ui<'_>) + 'static,
    ) {
        self.switch_with_slide(
            move || condition.read().then_some(()),
            Some(slide),
            move |ui, ()| create(ui),
        );
    }

    fn switch_on<K: PartialEq + 'static>(
        &mut self,
        key: impl Fn() -> Option<K> + 'static,
        create: impl Fn(&mut Ui<'_>, &K) + 'static,
    ) {
        self.switch_with_slide(key, None, create);
    }

    fn switch_with_slide<K: PartialEq + 'static>(
        &mut self,
        key: impl Fn() -> Option<K> + 'static,
        slide: Option<crate::overlay::Slide>,
        create: impl Fn(&mut Ui<'_>, &K) + 'static,
    ) {
        let anchor = self.anchor();
        let (parent, scope, control) = (self.parent, self.scope, self.control);
        let mut branch = Branch::<K, Mount>::new();
        self.host.binding(move || {
            branch.set(key(), |key| {
                let mut mount = Ui::mount_interned(parent, Some(anchor), scope, control, |ui| {
                    create(ui, key);
                });
                Host::with(|host| {
                    mount.place(host, parent, Some(anchor));
                    if let Some(slide) = slide {
                        mount.slide(host, slide);
                    }
                });
                mount
            });
        });
    }

    /// Updates several retained handles through the same deferred signal graph.
    ///
    /// Deferred until the creation borrow ends, like every UI binding, and it creates no
    /// structure: creation belongs to explicit transactions.
    pub fn effect(&mut self, mut update: impl FnMut(&mut Ui<'_>) + 'static) -> Effect {
        let (parent, scope, control) = (self.parent, self.scope, self.control);
        self.host.binding(move || {
            Host::with(|host| {
                let mut members = Vec::new();
                update(&mut Ui {
                    host,
                    members: &mut members,
                    parent,
                    after: None,
                    scope,
                    control,
                    root: false,
                });
                debug_assert!(members.is_empty(), "an update transaction created structure");
            });
        })
    }

    /// Installs one writer for `value`, or writes it once where it is constant.
    pub(crate) fn bind<T, M>(
        &mut self,
        value: impl Signal<T, M> + 'static,
        mut write: impl FnMut(&mut Host, T) + 'static,
    ) where
        T: Copy + PartialEq + 'static,
    {
        if value.is_constant() {
            let held = value.read();
            write(self.host, held);
            return;
        }
        let mut last: Option<T> = None;
        self.host.binding(move || {
            let next = value.read();
            if last.replace(next) != Some(next) {
                Host::with(|host| write(host, next));
            }
        });
    }

    pub fn geometry(&mut self, verbs: &[PathVerb]) -> GeomId {
        let id = self.host.geometry(verbs);
        crate::signal::Owner::retain(super::geometry::Lease(id));
        id
    }

    pub fn set_geometry(&mut self, id: GeomId, verbs: &[PathVerb]) {
        self.host.set_geometry(id, verbs);
    }

    pub fn ramp(&mut self, stops: &[super::Stop], spread: windows_scene::Spread) -> windows_scene::RampId {
        let id = self.host.ramp(stops, spread);
        crate::signal::Owner::retain(super::geometry::Lease(id));
        id
    }

    /// Re-resolves retained appearance and typography without recreating content.
    pub fn set_theme(&mut self, root: Scope, backdrop: windows_scene::BackdropSpec) {
        self.host.set_theme(root, backdrop);
    }

    // ── drawing in the geometry phase ───────────────────────────────────────────────

    /// Mints `N` retained geometries and fills them in the geometry phase from `source`.
    ///
    /// The geometry phase runs after the flush has published every box, so `fill` reads the
    /// box this batch solved and its verbs reach the same scene patch. It may not write
    /// layout, structure or application state. Each path has its own buffer, reserved once at
    /// `caps` and reused, and the set is re-emitted when the source box or a tracked read
    /// inside `fill` moves.
    pub(crate) fn geometries<const N: usize>(
        &mut self,
        source: super::geometry::Source,
        caps: [usize; N],
        fill: impl FnMut(&super::geometry::Inputs<'_>, &mut [Vec<PathVerb>; N]) + 'static,
    ) -> [GeomId; N] {
        self.geometries_on(self.parent, source, caps, fill)
    }

    /// Mints `N` geometries filled from the box of the container being built into, for figures
    /// stacked inside it that have to agree on one extent.
    pub fn own_geometries<const N: usize>(
        &mut self,
        caps: [usize; N],
        fill: impl FnMut(&super::geometry::Inputs<'_>, &mut [Vec<PathVerb>; N]) + 'static,
    ) -> [GeomId; N] {
        self.geometries(super::geometry::Source::Own(self.parent), caps, fill)
    }

    fn geometries_on<const N: usize>(
        &mut self,
        node: NodeId,
        source: super::geometry::Source,
        caps: [usize; N],
        fill: impl FnMut(&super::geometry::Inputs<'_>, &mut [Vec<PathVerb>; N]) + 'static,
    ) -> [GeomId; N] {
        let ids = core::array::from_fn(|_| self.geometry(&[]));
        self.host.add_geometry_job(node, source, ids, caps, fill);
        ids
    }

    /// Declares one painted path filled from its own solved box.
    pub fn path_with(
        &mut self,
        cap: usize,
        mut fill: impl FnMut(&super::geometry::Inputs<'_>, &mut Vec<PathVerb>) + 'static,
    ) -> Element<'_, super::Path> {
        let id = self.mint(Preset::Figure, true);
        let [geom] = self.geometries_on(
            id,
            super::geometry::Source::Own(id),
            [cap],
            move |inputs, [out]| fill(inputs, out),
        );
        self.host.appearances.set_shape(id, geom);
        self.element(id)
    }

    /// Mints one retained geometry filled from any probe's box.
    ///
    /// The probe need not be this node's own: the box a set of figures has to agree with is
    /// often a container none of them belongs to.
    pub fn local_geometry(
        &mut self,
        bounds: Probe,
        cap: usize,
        mut fill: impl FnMut(&super::geometry::Inputs<'_>, &mut Vec<PathVerb>) + 'static,
    ) -> GeomId {
        let [id] = self.geometries(
            super::geometry::Source::Probe(bounds),
            [cap],
            move |inputs, [out]| fill(inputs, out),
        );
        id
    }

    /// The keyed-set half: the same phase, the same buffers and the same equality cutoff,
    /// over a whole anchor table rather than one box.
    pub fn anchored_geometries<const N: usize>(
        &mut self,
        anchors: Anchors,
        caps: [usize; N],
        fill: impl FnMut(&super::geometry::Inputs<'_>, &mut [Vec<PathVerb>; N]) + 'static,
    ) -> [GeomId; N] {
        self.geometries(super::geometry::Source::Anchors(anchors), caps, fill)
    }
}

impl<'a, K> Element<'a, K> {
    /// Returns this element under another kind marker; the node is the same.
    pub(crate) fn retype<T>(self) -> Element<'a, T> {
        Element { ui: self.ui, node: self.node, kind: PhantomData }
    }

    pub fn id(self) -> Node<K> {
        Node { id: self.node, kind: PhantomData }
    }

    pub(crate) fn node_id(&self) -> NodeId {
        self.node
    }

    pub(crate) fn host(&mut self) -> &mut Host {
        self.ui.host
    }

    /// Runs the body synchronously with this parent's declared scope and control ownership.
    ///
    /// The body's first node goes above whatever the parent already holds: a plate declared
    /// before the body is a derived sprite at the bottom of the list, and a child minted at
    /// the bottom would paint beneath it.
    pub fn children(self, create: impl FnOnce(&mut Ui<'_>)) -> Self {
        let node = self.node;
        let scope = self.ui.host.tree.c.scope[node.index()];
        let control = self.ui.control;
        let after = self.ui.host.tree.children(node).last();
        // The parent's own list, not one of this container's: `root` is false here, so
        // nothing is pushed into it and a container costs no storage of its own.
        create(&mut Ui {
            host: &mut *self.ui.host,
            members: &mut *self.ui.members,
            parent: node,
            after,
            scope,
            control,
            root: false,
        });
        self
    }

    pub fn row(self, create: impl FnOnce(&mut Ui<'_>)) -> Self {
        self.preset(Preset::Row).children(create)
    }

    pub fn stack(self, create: impl FnOnce(&mut Ui<'_>)) -> Self {
        self.preset(Preset::Stack).children(create)
    }

    pub fn grid(self, create: impl FnOnce(&mut Ui<'_>)) -> Self {
        self.preset(Preset::Grid).children(create)
    }

    pub fn layer(self, create: impl FnOnce(&mut Ui<'_>)) -> Self {
        self.preset(Preset::Layer).children(create)
    }

    fn preset(self, preset: Preset) -> Self {
        let node = self.node;
        // The arrangement alone: everything else stated on this element so far stands, and an
        // alignment still at the old preset's default follows the new one.
        self.ui.host.tree.author(node, |l| {
            let (was, now) = (Layout::of(l.preset), Layout::of(preset));
            if l.align == was.align {
                l.align = now.align;
            }
            if l.justify == was.justify {
                l.justify = now.justify;
            }
            l.preset = preset;
        });
        self.ui.host.tree.adopt_layout_bits(node);
        self
    }

    /// Edits the authored layout and marks the node. The path every layout setter takes.
    pub fn layout(self, write: impl FnOnce(&mut Layout)) -> Self {
        self.author(write)
    }

    pub(crate) fn author(self, write: impl FnOnce(&mut Layout)) -> Self {
        self.ui.host.tree.author(self.node, write);
        self
    }

    pub(crate) fn flag(self, bit: tree::Bits, on: bool) -> Self {
        self.ui.host.tree.set_flag(self.node, bit, on);
        self
    }

    // ── extent ──────────────────────────────────────────────────────────────────────

    pub fn width(self, v: impl Into<Len>) -> Self {
        self.author(|l| l.width = v.into())
    }

    pub fn height(self, v: impl Into<Len>) -> Self {
        self.author(|l| l.height = v.into())
    }

    pub fn size(self, v: impl Into<Len> + Copy) -> Self {
        self.author(|l| (l.width, l.height) = (v.into(), v.into()))
    }

    pub fn min_width(self, v: impl Into<Len>) -> Self {
        self.author(|l| l.min_width = v.into())
    }

    pub fn min_height(self, v: impl Into<Len>) -> Self {
        self.author(|l| l.min_height = v.into())
    }

    pub fn max_width(self, v: impl Into<Len>) -> Self {
        self.author(|l| l.max_width = v.into())
    }

    pub fn max_height(self, v: impl Into<Len>) -> Self {
        self.author(|l| l.max_height = v.into())
    }

    pub fn aspect(self, ratio: f32) -> Self {
        self.author(|l| l.aspect = ratio)
    }

    /// Takes a share of the surplus, weighted. Zero by default, so growing is stated.
    pub fn grow(self) -> Self {
        self.grow_by(1.0)
    }

    pub fn grow_by(self, weight: f32) -> Self {
        self.author(|l| l.grow = weight)
    }

    // ── spacing and alignment ───────────────────────────────────────────────────────

    pub fn gap(self, v: impl Into<Len>) -> Self {
        self.author(|l| l.gap = v.into())
    }

    pub fn padding(self, v: impl Into<Len> + Copy) -> Self {
        self.author(|l| l.padding = [v.into(), v.into()])
    }

    pub fn padding_xy(self, x: impl Into<Len>, y: impl Into<Len>) -> Self {
        self.author(|l| l.padding = [x.into(), y.into()])
    }

    pub fn align(self, align: Align) -> Self {
        self.author(|l| l.align = align)
    }

    pub fn justify(self, align: Align) -> Self {
        self.author(|l| l.justify = align)
    }

    pub fn align_self(self, align: Align) -> Self {
        self.author(|l| l.align_self = align)
    }

    // ── placement ───────────────────────────────────────────────────────────────────

    pub fn at(self, row: u16, column: u16) -> Self {
        self.span(row, column, 1, 1)
    }

    pub fn span(self, row: u16, col: u16, row_span: u16, col_span: u16) -> Self {
        self.author(|l| {
            l.position = crate::layout::Position::Cell { row, col, row_span, col_span };
        })
    }

    /// Places this node at a normalized point of its parent, aligned by its own size.
    ///
    /// `x` and `y` are fractions of the parent's box; `align` says where the node's own
    /// measured extent sits against that point on each axis. Out of flow: it takes no track
    /// and no space from its siblings.
    pub fn anchor(self, x: f32, y: f32, align: [Align; 2]) -> Self {
        self.author(|l| l.position = crate::layout::Position::Anchor { at: [x, y, x, y], align })
    }

    pub fn cols(self, tracks: impl IntoIterator<Item = Track>) -> Self {
        self.author(|l| l.set_cols(tracks))
    }

    pub fn rows(self, tracks: impl IntoIterator<Item = Track>) -> Self {
        self.author(|l| l.set_rows(tracks))
    }

    /// Takes the column ladder the scope resolves `token` to, so a class-dependent column
    /// count is one registered table rather than a variant row per class.
    pub fn cols_by(self, token: &'static ScopedToken<&'static [Track]>) -> Self {
        self.author(|l| l.set_cols_by(token))
    }

    pub fn rows_by(self, token: &'static ScopedToken<&'static [Track]>) -> Self {
        self.author(|l| l.set_rows_by(token))
    }

    // ── response and visibility ─────────────────────────────────────────────────────

    /// Classifies this container from its own solved inline width, with the hysteresis band.
    pub fn responsive(self, bounds: [f32; 2]) -> Self {
        self.author(|l| l.bounds = bounds).flag(tree::RESPONSIVE, true)
    }

    /// Lays this row out as a stack at or below `class`, which is the one structural response
    /// a container states about itself.
    pub fn stack_below(self, class: WidthClass) -> Self {
        self.author(|l| l.stack_below = class)
    }

    pub fn clip(self) -> Self {
        self.flag(tree::CLIP, true)
    }

    /// Clips the subtree to its live compositor bounds with a resolved corner radius.
    /// Layout and hit containment use the same rectangular bounds; corners trim paint only.
    pub fn clip_rounded(self, radius: impl Into<Len>) -> Self {
        self.ui.host.clip_rounded(self.node, radius.into());
        self.flag(tree::CLIP | tree::ROUNDED_CLIP, true)
    }

    /// Springs changed layout bounds in this subtree on the compositor.
    ///
    /// Initial geometry snaps. Subsequent solves retarget native springs;
    /// unchanged bounds and tracker-owned offsets are left alone.
    pub fn animate_layout(self) -> Self {
        self.flag(tree::ANIMATE_LAYOUT, true)
    }

    /// Announces a change to this element's content to a listening client.
    ///
    /// `assertive` interrupts whatever the client is reading; otherwise the announcement waits
    /// until it is idle. Declared on the element and not on a control, because the thing that
    /// changes is the text inside a region rather than a control's own value.
    pub fn live_region(self, assertive: bool) -> Self {
        let live = if assertive { tree::LIVE_ASSERTIVE } else { tree::LIVE_POLITE };
        self.ui.host.tree.set_live(self.node, live);
        self
    }

    /// Keeps the node and its drafts and takes no space. A binding, so revealing is a setter
    /// and not a remount.
    pub fn hide_if<M>(mut self, value: impl Signal<bool, M> + 'static) -> Self {
        let node = self.node;
        self.ui.bind(value, move |host, on| host.tree.set_flag(node, tree::HIDDEN, on));
        self
    }

    // ── probes and anchor sets ──────────────────────────────────────────────────────

    pub fn probed(self, probe: Probe) -> Self {
        self.ui.host.set_probe(self.node, probe);
        self
    }

    /// Makes this node the space `anchors` reports its keyed boxes in.
    ///
    /// One origin per set. Its own solved size and scope ride the same table, so a consumer
    /// drawing over the set needs no second probe to rebase what it reads.
    pub fn anchors_origin(self, anchors: Anchors) -> Self {
        self.ui.host.set_anchor_origin(self.node, anchors);
        self
    }

    /// Reports this node's solved box into `anchors` under `key`.
    ///
    /// `key` is the application's own identity for what the node stands for, so a node
    /// recycled onto another subject reports under the subject's key rather than the node's.
    pub fn anchored(self, anchors: Anchors, key: u64) -> Self {
        self.ui.host.attach_anchor(self.node, anchors, key);
        self
    }

    /// Writes this node's computed layout on every call, reusing its track buffers.
    pub fn layout_from(self, write: impl Fn(&mut Layout) + 'static) -> Self {
        let node = self.node;
        self.ui
            .host
            .binding(move || Host::with(|host| host.tree.author(node, &write)));
        self
    }

    // ── channels ────────────────────────────────────────────────────────────────────

    /// Installs one retained writer per destination; a constant snaps and equal values stop
    /// here rather than reaching the wire.
    pub(crate) fn channel<T, M>(self, prop: Prop, value: impl Signal<T, M> + 'static) -> Self
    where
        T: Copy + Into<Value> + PartialEq + 'static,
    {
        self.ui.host.install_channel(self.node, prop, value);
        self
    }

    pub fn opacity<M>(self, value: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::Opacity, value)
    }

    /// Fades this subtree in while translating from a displacement in DIPs.
    ///
    /// Runs once after the first nonempty solve. Input is suspended until the compositor
    /// completes the translation; a zero displacement fades without suspending input.
    /// Text and paths retain their native scale. System animation preferences apply.
    /// The node's opacity and anchor channels must remain exclusive to this entrance.
    /// `by` must be finite, `ms` must be nonzero, and `ms + delay_ms` must fit in `u32`.
    pub fn enter_from(self, by: Vector2, ms: u32, delay_ms: u32, easing: windows_scene::Easing) -> Self {
        assert!(by.x.is_finite() && by.y.is_finite() && ms > 0);
        assert!(ms.checked_add(delay_ms).is_some());
        self.ui.host.enter_from(self.node, crate::overlay::Slide { by, ms, easing }, delay_ms);
        self
    }

    pub fn rotation<M>(self, value: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::RotationAngle, value)
    }

    pub fn trim<M>(self, value: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::TrimEnd, value)
    }

    pub fn stroke_width<M>(self, value: impl Signal<f32, M> + 'static) -> Self {
        self.channel(Prop::StrokeThickness, value)
    }

    pub fn pivot<M>(self, value: impl Signal<Vector2, M> + 'static) -> Self {
        self.channel(Prop::Center, value)
    }

    /// Sets the pivot as a fraction of this node's own solved box, resolved at publication.
    pub fn pivot_relative(self, fraction: Vector2) -> Self {
        self.ui.host.set_relative_pivot(self.node, fraction);
        self
    }

}

impl Element<'_, super::Region> {
    /// Rounds the region's own corners.
    ///
    /// The mask is this side's, not the renderer's: the buffer is a rectangle and the
    /// compositor is what clips it, so a region on a card takes the card's radius without the
    /// renderer knowing what shape it is drawing into.
    pub(crate) fn region_radius(self, radius: Len) -> Self {
        let node = self.node;
        let sink = self.ui.host.region_sink(node);
        self.ui.host.declare_part(
            node,
            super::theme::Part::Ink,
            super::theme::PaintSource::Region(sink),
            super::theme::PaintMask::Box { radius },
            1.0,
        );
        self
    }
}
