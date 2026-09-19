//! Optional touch surfaces. One callback captures occlusions and rings once; no polling.

use crate::bindings::{CoreFrameworkInputView, CoreInputView, CoreInputViewOcclusionKind};
use std::sync::{Arc, Mutex};
use windows_core::EventRevoker;
use crate::layout::Rect;
use windows_window::{Hwnd, WM_FRAME};

/// The docked occlusion the last callback reported, and whether it is unread.
#[derive(Default)]
struct Pending {
    docked: Option<Rect>,
    unread: bool,
}

pub(crate) struct Touch {
    view: Option<CoreInputView>,
    _framework: Option<CoreFrameworkInputView>,
    _subscription: Option<EventRevoker>,
    pending: Arc<Mutex<Pending>>,
    /// The docked occlusion in client DIPs, or none while nothing docked occludes.
    pub docked: Option<Rect>,
}

impl Touch {
    /// Probes both input views once, diagnosing each missing capability by name.
    ///
    /// Both views raise `OcclusionsChanged` for the same occlusions, so the framework view
    /// carries the one subscription and the input view is kept for `TryShowPrimaryView`.
    pub fn new(hwnd: Hwnd) -> Self {
        let view = probe("CoreInputView", CoreInputView::GetForCurrentView());
        let framework = probe(
            "CoreFrameworkInputView",
            CoreFrameworkInputView::GetForCurrentView(),
        );
        let pending = Arc::new(Mutex::new(Pending::default()));
        let inbox = Arc::clone(&pending);
        let subscription = framework.as_ref().and_then(|v| {
            let handler = v.OcclusionsChanged(move |_, args| {
                let Some(values) = args.ok().ok().and_then(|a| a.Occlusions().ok()) else {
                    return;
                };
                // Floating and overlay kinds do not reflow the editor, so only docked
                // occlusions are unioned and reported.
                let mut docked: Option<Rect> = None;
                for value in values {
                    if value.OcclusionKind().ok() != Some(CoreInputViewOcclusionKind::Docked) {
                        continue;
                    }
                    let Ok(r) = value.OccludingRect() else {
                        continue;
                    };
                    // A degenerate occlusion is not a rectangle to union with: one at the
                    // origin would drag the union up to the top of the window.
                    if r.y <= 0.0 || r.width <= 0.0 || r.height <= 0.0 {
                        continue;
                    }
                    let r = Rect {
                        x0: r.x,
                        y0: r.y,
                        x1: r.x + r.width,
                        y1: r.y + r.height,
                    };
                    docked = Some(docked.map_or(r, |old| old.union(r)));
                }
                let mut held = inbox.lock().unwrap_or_else(|e| e.into_inner());
                held.docked = docked;
                if !core::mem::replace(&mut held.unread, true) {
                    hwnd.post(WM_FRAME, 0, 0);
                }
            });
            probe("CoreFrameworkInputView.OcclusionsChanged", handler)
        });
        Self {
            view,
            _framework: framework,
            _subscription: subscription,
            pending,
            docked: None,
        }
    }

    /// Adopts the last reported occlusion and reports whether it changed since the last tick.
    pub fn take(&mut self) -> bool {
        let mut held = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let changed = core::mem::take(&mut held.unread) && self.docked != held.docked;
        self.docked = held.docked;
        changed
    }

    /// Requests the primary input view for a touch press on a field.
    pub fn show(&self) {
        if let Some(view) = &self.view {
            let _ = view.TryShowPrimaryView();
        }
    }
}

fn probe<T>(name: &str, capability: windows_core::Result<T>) -> Option<T> {
    capability
        .inspect_err(|e| eprintln!("disabled capability {name}: {e}"))
        .ok()
}
