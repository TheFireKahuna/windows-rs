//! Retained declarations with responsive fields only where authored.
use crate::layout::{Layout, Preset};
use crate::role::Scope;
use windows_scene::{Node, Slots, WidthClass, taffy};

#[derive(Clone, Default)]
pub(crate) struct Declaration {
    pub base: Layout,
    pub variants: Vec<(WidthClass, Layout)>,
}

impl Declaration {
    pub fn at(&mut self, class: Option<WidthClass>) -> &mut Layout {
        let Some(class) = class else {
            return &mut self.base;
        };
        let index = self
            .variants
            .iter()
            .position(|(at, _)| *at == class)
            .unwrap_or_else(|| {
                self.variants.push((class, Layout::default()));
                self.variants.len() - 1
            });
        &mut self.variants[index].1
    }
}

pub(crate) struct Recipe {
    pub preset: Preset,
    pub scope: Scope,
    pub layout: Declaration,
}

impl Recipe {
    pub(crate) fn lower(&self, class: WidthClass) -> taffy::Style {
        let variant = self
            .layout
            .variants
            .iter()
            .find(|(at, _)| *at == class)
            .map(|(_, layout)| layout);
        self.layout
            .base
            .lower(self.preset, variant, self.scope.at_width(class))
    }
}

pub(crate) type Styles = Slots<Node, Recipe>;
