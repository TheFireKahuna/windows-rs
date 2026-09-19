//! Input-owned UTF-16 documents; app-owned shaping; event-rate retained publication.
//!
//! Only plain snapshots cross threads. TSF and the editable buffer stay on the window STA,
//! and a geometry revision must match the buffer before it can answer a caret operation.

mod doc;
mod geometry;
mod input;
mod session;
mod store;
pub(crate) mod system;
mod touch;

pub(crate) use geometry::{Cluster, Geometry};
pub(crate) use input::TextInput;

use std::sync::Arc;
use windows_scene::ControlId;

/// Declares a field's text-service context. Number is an input hint, not validation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputScope {
    /// Ordinary single-line text.
    #[default]
    Default,
    /// Numeric keyboard and recognition.
    Number,
    /// A web address.
    Url,
    /// A search query.
    Search,
    /// Masked text, excluded from copy and accessibility text queries.
    Password,
}

/// Which character the insertion point belongs to at a bidirectional boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Affinity {
    #[default]
    Downstream,
    Upstream,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Selection {
    pub anchor: u32,
    pub caret: u32,
    pub affinity: Affinity,
}

impl Selection {
    pub fn range(self) -> core::ops::Range<u32> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }

    /// Returns the collapsed, downstream selection at `caret`.
    pub fn at(caret: u32) -> Self {
        Self {
            anchor: caret,
            caret,
            affinity: Affinity::Downstream,
        }
    }
}

/// One immutable input transaction. Cloning this row never copies a document.
#[derive(Clone, Debug)]
pub(crate) struct Update {
    pub id: ControlId,
    pub revision: u64,
    pub text: Option<Arc<[u16]>>,
    pub selection: Selection,
    pub composition: Option<core::ops::Range<u32>>,
    pub focused: bool,
    pub commit: Option<Arc<str>>,
}

/// App source changes are based on the last user revision that app has received.
#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub id: ControlId,
    pub scope: InputScope,
    pub based_on: u64,
    pub text: Arc<[u16]>,
}

#[derive(Clone, Debug)]
pub(crate) struct Layout {
    pub id: ControlId,
    pub geometry: Arc<Geometry>,
}

#[derive(Clone, Debug)]
pub(crate) struct Commit {
    pub id: ControlId,
    pub revision: u64,
    pub text: Arc<str>,
}

/// A reveal request names a live control and the docked occlusion, in client DIPs.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Reveal {
    pub id: ControlId,
    pub occlusion: Option<crate::layout::Rect>,
}
