//! `each`, `when` and `switch`: the two structure mechanisms of [`Keyed`] and [`Branch`],
//! bound to this crate's view type.
//!
//! Binding is the whole of what this module adds. There is no third mechanism and no
//! container special case: all three types are [`IntoChildren`](super::IntoChildren)
//! implementations, so a list is passed exactly where a tuple would be.
//!
//! Rows and arms build at **reconcile** time, inside a detached scope. Reconcile runs from an
//! effect, outside any `Build::with` borrow, so a row is free to call application code.
//!
//! Nothing here pushes data into a surviving row. A survivor gets a move and, through the
//! ordinary channels on the sinks it already has, a value change — so a filter keystroke that
//! keeps a card reorders it rather than rebuilding it. A row whose contents follow its item
//! closes over a signal, as every other value in this surface does.

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
    fill: Box<dyn Fn(&mut Vec<T>)>,
    key: Box<dyn Fn(&T) -> &K>,
    /// Erased to [`View`] here rather than carried as a kind, so a list of one widget kind
    /// and a list of another are the same type at the container.
    view: Box<dyn Fn(&T) -> View>,
}

/// Builds a list whose rows are recycled by key.
///
/// `items` is read inside an effect, so it tracks whatever it reads and the list reconciles
/// when that moves. The `Vec` it returns is the one heap allocation on the path;
/// [`each_into`] is the form without it.
///
/// `key` **projects** the identity out of the item rather than being carried beside it, so a
/// list whose items are their own keys writes `|item| item` and stores each one once.
pub fn each<K, T, V>(
    items: impl Fn() -> Vec<T> + 'static,
    key: impl Fn(&T) -> &K + 'static,
    view: impl Fn(&T) -> El<V> + 'static,
) -> Each<K, T>
where
    K: Eq + Hash + Clone + 'static,
    T: 'static,
    V: 'static,
{
    each_into(move |out| *out = items(), key, view)
}

/// Builds a keyed list that fills a buffer the adapter keeps.
///
/// The buffer reaches its high-water mark once and nothing after that allocates, which is
/// what a list reconciling **every frame of a fling** needs, and what any list on a path that
/// must not allocate needs. A caller copying items out of state it already holds wants this
/// form; a caller building a fresh `Vec` per read gains nothing over [`each`].
pub fn each_into<K, T, V>(
    fill: impl Fn(&mut Vec<T>) + 'static,
    key: impl Fn(&T) -> &K + 'static,
    view: impl Fn(&T) -> El<V> + 'static,
) -> Each<K, T>
where
    K: Eq + Hash + Clone + 'static,
    T: 'static,
    V: 'static,
{
    Each {
        fill: Box::new(fill),
        key: Box::new(key),
        view: Box::new(move |item| view(item).erase()),
    }
}

impl<K, T> IntoChildren for Each<K, T>
where
    K: Eq + Hash + Clone + 'static,
    T: 'static,
{
    fn append(self, out: &mut Children) {
        let Self { fill, key, view } = self;
        out.push(El::<Any>::at_index(adapter(move |site| {
            // Detached from whatever scope runs the reconcile: rows belong to the list, and
            // a list driven from an effect would otherwise register every row it ever built
            // as a child of that effect.
            let list = Rc::new(RefCell::new(Keyed::<K>::new()));
            let mounts = Rc::new(RefCell::new(FxHashMap::<K, Mount>::default()));
            let next = Rc::new(RefCell::new(Vec::<T>::new()));
            Effect::new(move || {
                let mut next = next.borrow_mut();
                next.clear();
                fill(&mut next);
                let mut list = list.borrow_mut();
                // The anchor, not the head of the container: rows share their parent with
                // the container's static children, so starting from `None` would put the
                // first row above everything written before the list.
                let mut previous: Option<NodeId> = site.after;
                list.reconcile(
                    &next,
                    &key,
                    |key| {
                        mounts.borrow_mut().remove(key);
                    },
                    |key, item| {
                        // Born at the anchor rather than at the head, so a new row exists
                        // inside this list from the moment it is mounted. The pass below
                        // places every insert afterwards.
                        let mount =
                            super::mount_at(view(item), site.parent, site.after, site.scope);
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
/// Not generic over the signal's marker: the condition is boxed on the way in, so two
/// conditionals of the same shape are one type.
pub struct When {
    cond: Box<dyn Fn() -> bool>,
    view: Box<dyn Fn() -> View>,
}

/// Builds a subtree that is present while `cond` holds and absent otherwise.
///
/// **Absence contributes nothing**: no node, no layout participation, no hidden placeholder.
/// A constant condition tracks nothing, so the effect driving it runs once and the subtree is
/// decided once.
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

/// Builds one arm of several, rebuilt when `key` changes.
///
/// The outgoing arm's scope is dropped, so state owned inside it is gone. State that has to
/// outlive the arm lives in a cell owned above the switch.
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

/// Appends the adapter both conditional forms share: one [`Branch`], driven by one effect.
///
/// `when` passes a key that is `None` while its condition is false; `switch` passes a key
/// that always exists.
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
                    // The anchor is the whole of this arm's position: a branch has one arm
                    // and no reorder pass to correct it afterwards.
                    *mount.borrow_mut() = Some(super::mount_at(
                        view(key),
                        site.parent,
                        site.after,
                        site.scope,
                    ));
                },
            );
        });
    })));
}

/// Appends the arena slot an adapter owns — its **anchor** — and returns its index.
///
/// The node this becomes is hidden at mount and never holds a child. What it holds is a
/// position, as [`Site`] describes: rows and arms are laid out by the container the list was
/// passed to rather than by a box the author never wrote, and two adjacent lists keep their
/// order across being empty.
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
