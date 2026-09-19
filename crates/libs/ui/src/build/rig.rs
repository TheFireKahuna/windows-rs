//! The fixture every claim below it shares: a host at a stated size and scale, the view a
//! test mounts into it, and the questions a published patch takes several steps to answer.
//!
//! A question one call already answers is not here. A test that wants a solved box calls
//! `Host::geom`, and one that wants a live count calls `Host::live_nodes`.

use super::host::Host;
use super::mount::Mount;
use super::ui::Ui;
use crate::role::{AccentId, Density, Scope};
use crate::signal::{Cell, Owner};
use crate::uia::{ColFlags, Snapshot, State, Tree};
use crate::widget::{Range, UiaRole};
use windows_color::{DisplayCapability, OutputTransform};
use windows_numerics::Vector2;
use windows_scene::{
    Anim, Bind, ContactKind, Env, HitEntry, NodeId, Op, Point, Prop, SinkPatch, Value,
};
use windows_text::FontLadder;

/// Installs this thread's host and text engine and answers a drained patch.
pub(crate) fn fixture() -> SinkPatch {
    fixture_at(96.0)
}

/// The same, at a stated pixel density.
pub(crate) fn fixture_at(dpi: f32) -> SinkPatch {
    Host::install(
        Env::new(dpi, OutputTransform::for_display(DisplayCapability::Sdr, 1000.0)),
        Scope::root(crate::role::tests::palette(), AccentId(0), Density::Comfortable),
    );
    // A fresh host has not resolved its metric table, and until it has, every length stated
    // in a metric solves to zero.
    Host::with(|h| {
        let root = h.root_scope();
        h.fill_metrics(root);
        h.set_window(Vector2 { x: 800.0, y: 600.0 });
    });
    Host::with(|h| h.text.install(FontLadder::new(["Segoe UI Variable Text", "Cascadia Mono"])))
        .expect("a text engine");
    let mut patch = SinkPatch::default();
    Host::flush(&mut patch);
    patch.clear();
    patch
}

/// A host, the view mounted into it, and the scope that view's bindings belong to.
pub(crate) struct Rig {
    patch: SinkPatch,
    /// The array as the last publication that rebuilt it left it: a flush that moved nothing
    /// republishes none, and what a test asks about is the array that stands.
    hits: Vec<HitEntry>,
    held: Vec<Mount>,
    owner: Owner,
}

impl Rig {
    /// A host at 800 by 600 DIPs, one physical pixel to the DIP.
    pub(crate) fn new() -> Self {
        Self::at(800.0, 600.0, 1.0)
    }

    pub(crate) fn at(w: f32, h: f32, scale: f32) -> Self {
        let patch = fixture_at(scale * 96.0);
        let (owner, ()) = Owner::scope(|| ());
        let mut rig = Self { patch, hits: Vec::new(), held: Vec::new(), owner };
        rig.window(w, h);
        rig
    }

    fn window(&mut self, w: f32, h: f32) {
        Host::with(|host| host.set_window(Vector2 { x: w, y: h }));
    }

    /// Mounts `body` under the window root and publishes it.
    pub(crate) fn mount(&mut self, body: impl FnOnce(&mut Ui<'_>)) -> Frame<'_> {
        let mount = self.owner.run(|| Ui::mount_root(body));
        self.held.push(mount);
        self.flush()
    }

    /// Retires everything this rig mounted and publishes what that left.
    pub(crate) fn unmount(&mut self) -> Frame<'_> {
        self.held.clear();
        self.flush()
    }

    pub(crate) fn resize(&mut self, w: f32, h: f32) -> Frame<'_> {
        self.window(w, h);
        self.flush()
    }

    /// Writes `value` and publishes what the graph made of it.
    pub(crate) fn set<T: Copy + PartialEq + 'static>(
        &mut self,
        cell: Cell<T>,
        value: T,
    ) -> Frame<'_> {
        cell.set(value);
        self.flush()
    }

    pub(crate) fn flush(&mut self) -> Frame<'_> {
        self.patch.clear();
        Host::flush(&mut self.patch);
        let span = self.patch.ops().iter().find_map(|op| match op {
            Op::Hits { entries, .. } => Some(*entries),
            _ => None,
        });
        if let Some(span) = span {
            self.hits = self.patch.hits(span).to_vec();
        }
        Frame(self)
    }
}

/// Which op a count is over.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    New,
    Move,
    Drop,
}

/// One automation element, as a client reads it.
pub(crate) struct UiaRow {
    pub role: UiaRole,
    pub flags: ColFlags,
    pub state: State,
    pub range: Option<Range>,
    /// This element's children, by name, in forward order.
    pub children: Vec<String>,
}

/// What one publication answers.
pub(crate) struct Frame<'a>(&'a mut Rig);

impl Frame<'_> {
    /// Everything this publication put on the wire.
    pub(crate) fn patch(&self) -> &SinkPatch {
        &self.0.patch
    }

    /// The hit array that stands, in paint order.
    pub(crate) fn hits(&self) -> &[HitEntry] {
        &self.0.hits
    }

    /// The value this publication bound to one of `node`'s channels, last write winning.
    pub(crate) fn bound(&self, node: impl Into<NodeId>, prop: Prop) -> Option<Value> {
        let node = node.into();
        self.0.patch.ops().iter().rev().find_map(|op| match op {
            Op::Bind { id, prop: p, bind } if *id == node && *p == prop => match bind {
                Bind::Set(v) | Bind::Animate(Anim::Spring { to: v, .. }) => Some(*v),
                _ => None,
            },
            _ => None,
        })
    }

    /// How many ops of one kind this publication carried.
    pub(crate) fn ops(&self, kind: Kind) -> usize {
        self.0
            .patch
            .ops()
            .iter()
            .filter(|op| {
                matches!(
                    (kind, op),
                    (Kind::New, Op::New { .. })
                        | (Kind::Move, Op::Move { .. })
                        | (Kind::Drop, Op::Drop { .. })
                )
            })
            .count()
    }

    /// The node a contact at `x`, `y` resolves to, through the one hit array.
    pub(crate) fn pick(&self, x: f32, y: f32, contact: ContactKind) -> Option<NodeId> {
        let hit = windows_scene::scan(&self.0.hits, Point { x, y }, contact, 0, &|_| Vector2 {
            x: 0.0,
            y: 0.0,
        })?;
        Host::with(|h| h.control(hit.id).map(|row| row.node))
    }

    /// How many rows the host holds beside the arena: controls, appearances, fields, side
    /// rows and handler rows.
    pub(crate) fn rows(&self) -> usize {
        Host::with(|h| {
            h.controls.iter().count()
                + h.appearances.placed()
                + h.fields.iter().count()
                + h.side_rows()
                + h.handlers.placed()
        })
    }

    /// Every run's text, in paint order, which is the order the window draws them in.
    pub(crate) fn runs(&self) -> Vec<String> {
        Host::with(|h| {
            let mut out = Vec::new();
            let mut stack = vec![h.root()];
            while let Some(node) = stack.pop() {
                if let Some(text) = h.text.str_of(h.tree.c.text[node.index()]) {
                    out.push(text.to_owned());
                }
                let children: Vec<NodeId> = h.tree.children(node).collect();
                stack.extend(children.into_iter().rev());
            }
            out
        })
    }

    /// The automation element published under `name`.
    ///
    /// The walk builds into the host's own patch, and a frame answers after its flush, when
    /// nothing else is queued there, so the buffers it filled are dropped.
    pub(crate) fn uia(&mut self, name: &str) -> Option<UiaRow> {
        let mut snapshot = Snapshot::default();
        Host::with(|h| {
            h.uia_entries(&mut snapshot);
            h.pending.clear();
        });
        let tree = Tree::adopt(&snapshot);
        let at = tree
            .entries()
            .iter()
            .position(|entry| String::from_utf16_lossy(tree.text(entry.name)) == name)?
            as u16;
        let entry = *tree.at(at)?;
        let mut children = Vec::new();
        let mut child = entry.child;
        while let Some(row) = tree.at(child) {
            children.push(String::from_utf16_lossy(tree.text(row.name)));
            child = row.next;
        }
        Some(UiaRow {
            role: entry.role,
            flags: entry.flags,
            state: tree.state(at),
            range: tree.range(at),
            children,
        })
    }
}
