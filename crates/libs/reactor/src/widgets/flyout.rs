use super::*;

/// Describes the content for a Flyout attached to a button.
///
/// Upstream's plain-text `text` content is kept verbatim. The fork adds
/// optional `rich` element-tree content (for band panels, color pickers, and
/// other rich popovers) plus an `open` binding and an `on_closed` lifecycle
/// callback. When `rich` is set it takes precedence over `text`. The attached
/// flyout opens on the button's native click; `open` can be set explicitly and
/// `on_closed` fires when it dismisses (light-dismiss, Escape, or programmatic
/// hide).
#[derive(Clone, Debug, PartialEq)]
pub struct FlyoutDef {
    pub text: String,
    pub placement: FlyoutPlacementMode,
    /// Rich element-tree content. Takes precedence over `text` when set.
    pub rich: Option<Box<Element>>,
    /// Explicit open/closed state. `None` leaves it to native button-click
    /// open + light-dismiss close.
    pub open: Option<bool>,
    /// Fired when the flyout is dismissed.
    pub on_closed: Option<Callback<()>>,
}

impl FlyoutDef {
    /// Plain-text flyout content.
    pub fn text(s: impl Into<String>) -> Self {
        Self {
            text: s.into(),
            placement: FlyoutPlacementMode::default(),
            rich: None,
            open: None,
            on_closed: None,
        }
    }

    /// Rich element-tree flyout content (band panel, color picker, …).
    pub fn rich(element: impl Into<Element>) -> Self {
        Self {
            text: String::new(),
            placement: FlyoutPlacementMode::default(),
            rich: Some(Box::new(element.into())),
            open: None,
            on_closed: None,
        }
    }

    pub fn placement(mut self, p: FlyoutPlacementMode) -> Self {
        self.placement = p;
        self
    }

    /// Bind the open/closed state explicitly.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Callback fired when the flyout is dismissed.
    pub fn on_closed(mut self, f: impl IntoCallback<()>) -> Self {
        self.on_closed = Some(f.into_callback());
        self
    }
}
