//! `each`, `when` and `switch`: the two structure mechanisms bound to a view type.
//!
//! `windows-scene`'s neighbours supply [`Keyed`] and [`Branch`] and deliberately stop there,
//! because binding them needs a view type and the view type is this crate's. What is added
//! here is that binding and nothing else — no third mechanism, and no container special
//! case: all three are [`IntoChildren`](super::IntoChildren) implementations, so a list is
//! passed exactly where a tuple would be.
//!
//! Rows and arms build at **reconcile** time, inside a detached scope. That is where the
//! arena's re-entry rule is satisfied rather than violated: reconcile runs from an effect,
//! outside any `Build::with` borrow, so a row is free to call application code.
//!
//! Nothing here pushes data into a surviving row. A survivor is a move and a value change,
//! and the value change arrives through the ordinary channel on sinks the row already has —
//! which is what makes a filter keystroke that keeps a card reorder it rather than rebuild
//! it. A row whose contents must follow its item therefore closes over a signal, as every
//! other value in this surface does.

use super::arena::{Adapter, Build, Slot};
use super::{Any, Children, El, IntoChildren, Mount, Site, View};
use crate::layout::Preset;
use crate::signal::{Effect, Signal};
use crate::structure::{Branch, Keyed, Step};
use core::hash::Hash;
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::rc::Rc;
use windows_scene::NodeId;

/// A keyed list.
///
/// Built by [`each`]; it exists to be passed to a container.
pub struct Each<K, T> {
    fill: Box<dyn Fn(&mut Vec<(K, T)>)>,
    /// Erased to [`View`] here rather than carried as a kind: a list of one widget kind and
    /// a list of another are the same list, and a marker that survived to the container
    /// would make them different types for no property anyone can use.
    view: Box<dyn Fn(&T) -> View>,
}

/// A list whose rows are recycled by key.
///
/// `items` is read inside an effect, so it tracks whatever it reads and the list reconciles
/// when that moves. The `Vec` it returns is the one heap allocation on the path, and it is
/// visible here rather than hidden per row.
pub fn each<K, T, V>(
    items: impl Fn() -> Vec<(K, T)> + 'static,
    view: impl Fn(&T) -> El<V> + 'static,
) -> Each<K, T>
where
    K: Eq + Hash + Clone + 'static,
    T: 'static,
    V: 'static,
{
    each_into(move |out| *out = items(), view)
}

/// The same, filling a buffer the adapter keeps.
///
/// What a list reconciling **every frame of a fling** needs: the buffer reaches its
/// high-water mark once and the realization path allocates nothing after it. A caller who
/// builds a fresh `Vec` per read has no reason to reach for this.
pub(crate) fn each_into<K, T, V>(
    fill: impl Fn(&mut Vec<(K, T)>) + 'static,
    view: impl Fn(&T) -> El<V> + 'static,
) -> Each<K, T>
where
    K: Eq + Hash + Clone + 'static,
    T: 'static,
    V: 'static,
{
    Each {
        fill: Box::new(fill),
        view: Box::new(move |item| view(item).erase()),
    }
}

impl<K, T> IntoChildren for Each<K, T>
where
    K: Eq + Hash + Clone + 'static,
    T: 'static,
{
    fn append(self, out: &mut Children) {
        let Self { fill, view } = self;
        out.push(El::<Any>::at_index(adapter(move |site| {
            // Detached from whatever scope is running the reconcile, for the reason the
            // list itself is: rows belong to the list, and a list driven from an effect
            // would otherwise register every row it ever built as a child of that effect.
            let list = Rc::new(RefCell::new(Keyed::<K>::new()));
            let mounts = Rc::new(RefCell::new(FxHashMap::<K, Mount>::default()));
            let next = Rc::new(RefCell::new(Vec::<(K, T)>::new()));
            Effect::new(move || {
                let mut next = next.borrow_mut();
                next.clear();
                fill(&mut next);
                let mut list = list.borrow_mut();
                let mut previous: Option<NodeId> = None;
                list.reconcile(
                    &next,
                    |key| {
                        mounts.borrow_mut().remove(key);
                    },
                    |key, item| {
                        let mount = super::mount_at(view(item), site.parent, None, site.scope);
                        mounts.borrow_mut().insert(key.clone(), mount);
                    },
                    |key, _, step, _| {
                        let mounts = mounts.borrow();
                        let Some(mount) = mounts.get(key) else {
                            return;
                        };
                        let node = mount.node();
                        // A survivor already in place needs no op at all; the rest is the
                        // minimal move set the reconciler computed, applied front to back
                        // so the predecessor is already where it belongs.
                        if step != Step::Keep {
                            super::Host::with(|h| h.model().place(node, site.parent, previous));
                        }
                        previous = Some(node);
                    },
                );
            });
        })));
    }
}

/// A subtree that is present or absent.
///
/// Not generic over the signal's marker: the condition is boxed on the way in, so carrying
/// the marker out would make two lists of the same shape two types for no property anyone
/// can use.
pub struct When {
    cond: Box<dyn Fn() -> bool>,
    view: Box<dyn Fn() -> View>,
}

/// Present when `cond`, absent otherwise.
///
/// **Absence contributes nothing** — no node, no layout participation, no hidden
/// placeholder. A constant condition is resolved here and never reaches the graph, which is
/// what makes the common case free.
pub fn when<M>(cond: impl Signal<bool, M> + 'static, view: impl Fn() -> View + 'static) -> When {
    When {
        cond: Box::new(move || cond.read()),
        view: Box::new(view),
    }
}

impl IntoChildren for When {
    fn append(self, out: &mut Children) {
        let Self { cond, view } = self;
        switch_adapter(out, move || cond().then_some(true), move |_| view());
    }
}

/// A subtree keyed by which arm is showing.
pub struct Switch<K> {
    key: Box<dyn Fn() -> K>,
    view: Box<dyn Fn(&K) -> View>,
}

/// One arm of several, rebuilt when the key changes.
///
/// The arm's scope is dropped and rebuilt on a change, so a screen's state is genuinely
/// gone when you navigate away. Where that is *not* wanted, the state lives in a cell owned
/// above the switch — a decision the call site makes by where it puts the cell.
pub fn switch<K: PartialEq + 'static>(
    key: impl Fn() -> K + 'static,
    view: impl Fn(&K) -> View + 'static,
) -> Switch<K> {
    Switch {
        key: Box::new(key),
        view: Box::new(view),
    }
}

impl<K: PartialEq + 'static> IntoChildren for Switch<K> {
    fn append(self, out: &mut Children) {
        let Self { key, view } = self;
        switch_adapter(out, move || Some(key()), view);
    }
}

/// The shape both conditional forms share: one [`Branch`], driven by one effect.
///
/// `when` is `Branch<bool>` whose key is `None` when the condition is false, and `switch` is
/// `Branch<K>` whose key always exists. One mechanism, differing in what it keys on.
fn switch_adapter<K: PartialEq + 'static>(
    out: &mut Children,
    key: impl Fn() -> Option<K> + 'static,
    view: impl Fn(&K) -> View + 'static,
) {
    out.push(El::<Any>::at_index(adapter(move |site| {
        let branch = Rc::new(RefCell::new(Branch::<K>::new()));
        let mount = Rc::new(RefCell::new(None::<Mount>));
        Effect::new(move || {
            let next = key();
            branch.borrow_mut().set(
                next,
                |_| {
                    // Dropped here rather than in the build below, so the outgoing arm's
                    // nodes are gone before the incoming one is minted and the two never
                    // both exist.
                    mount.borrow_mut().take();
                },
                |key| {
                    *mount.borrow_mut() =
                        Some(super::mount_at(view(key), site.parent, None, site.scope));
                },
            );
        });
    })));
}

/// Appends the arena slot an adapter owns: a bare group with no children of its own.
fn adapter(install: impl FnOnce(Site) + 'static) -> u32 {
    Build::with(|b| {
        let at = b.push_slot(Slot {
            preset: Preset::Bare,
            ..Slot::default()
        });
        let adapter = b.push_adapter(Adapter {
            install: Some(Box::new(install)),
        });
        b.nodes[at as usize].adapter = Some(adapter);
        at
    })
}
