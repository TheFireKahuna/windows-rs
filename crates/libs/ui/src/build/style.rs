//! Retained declarations with pooled responsive fields only where authored.
use crate::layout::{Layout, Len, Preset};
use crate::role::Scope;
use windows_scene::{Node, NodeId, Slots, WidthClass, taffy};

#[derive(Default)]
pub(crate) struct Declaration {
    pub base: Layout,
    variants: u32,
}

pub(crate) struct Recipe {
    pub preset: Preset,
    pub scope: Scope,
    pub layout: Declaration,
    /// The class this recipe is waiting to be lowered at, once for the whole transaction.
    ///
    /// Present exactly while the node sits in the host's pending list, so a chained setter
    /// restates the class rather than queuing the node a second time.
    pub pending: Option<WidthClass>,
}

impl Recipe {
    pub(crate) fn lower(
        &self,
        class: WidthClass,
        variant: Option<&Layout>,
    ) -> (taffy::Style, windows_numerics::Vector2) {
        self.layout
            .base
            .lower(self.preset, variant, self.scope.at_width(class))
    }
}

struct Variant {
    class: WidthClass,
    layout: Layout,
    next: u32,
}

#[derive(Default)]
pub(crate) struct Styles {
    recipes: Slots<Node, Recipe>,
    variants: Vec<Variant>,
    free: u32,
}

impl Styles {
    pub fn get(&self, node: NodeId) -> Option<&Recipe> {
        self.recipes.get(node)
    }
    pub fn get_mut(&mut self, node: NodeId) -> Option<&mut Recipe> {
        self.recipes.get_mut(node)
    }
    pub fn positions(&self) -> core::ops::Range<u32> {
        self.recipes.positions()
    }
    pub fn id_at(&self, index: u32) -> Option<NodeId> {
        self.recipes.id_at(index)
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.recipes.len()
    }
    #[cfg(test)]
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &Recipe)> {
        self.recipes.iter()
    }
    pub fn place(&mut self, node: NodeId, recipe: Recipe) {
        self.take(node);
        self.recipes.place(node, recipe);
    }
    pub fn take(&mut self, node: NodeId) {
        let Some(recipe) = self.recipes.take(node) else {
            return;
        };
        let mut index = recipe.layout.variants;
        while index != 0 {
            let row = &mut self.variants[index as usize - 1];
            let next = row.next;
            row.layout = Layout::default();
            row.next = self.free;
            self.free = index;
            index = next;
        }
    }
    pub fn at(&mut self, node: NodeId, class: WidthClass) -> &mut Layout {
        let recipe = self
            .recipes
            .get_mut(node)
            .expect("a live element owns its declaration");
        let mut index = recipe.layout.variants;
        while index != 0 {
            let row = &self.variants[index as usize - 1];
            if row.class == class {
                return &mut self.variants[index as usize - 1].layout;
            }
            index = row.next;
        }
        index = self.free;
        if index == 0 {
            self.variants.push(Variant {
                class,
                layout: Layout::default(),
                next: recipe.layout.variants,
            });
            index = u32::try_from(self.variants.len()).expect("responsive storage exhausted");
        } else {
            let row = &mut self.variants[index as usize - 1];
            self.free = row.next;
            row.class = class;
            row.next = recipe.layout.variants;
        }
        recipe.layout.variants = index;
        &mut self.variants[index as usize - 1].layout
    }
    /// Returns `node`'s solver style at `class`, with the anchor fraction it lowers to.
    pub fn lower(
        &self,
        node: NodeId,
        class: WidthClass,
    ) -> Option<(taffy::Style, windows_numerics::Vector2)> {
        let recipe = self.recipes.get(node)?;
        let mut index = recipe.layout.variants;
        while index != 0 {
            let row = &self.variants[index as usize - 1];
            if row.class == class {
                return Some(recipe.lower(class, Some(&row.layout)));
            }
            index = row.next;
        }
        Some(recipe.lower(class, None))
    }
}

impl<K> super::Element<'_, K> {
    /// States that this node adds nothing of its own: no gap, no padding, and no minimum.
    ///
    /// One token for the combination a compact container states together, and the way a
    /// control says its size is exactly what its author gave it. The project rule that a
    /// container meaning zero must state it holds — this states it.
    pub fn tight(self) -> Self {
        self.layout(|l| {
            l.gap = Some(Len::Zero);
            l.padding = Some([Len::Zero; 2]);
            l.min_width = Some(Len::Zero);
            l.min_height = Some(Len::Zero);
        })
    }
}
