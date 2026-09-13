//! Optional touch surfaces. Callbacks capture occlusions and ring once; no polling.
use crate::bindings::{
    CoreFrameworkInputView, CoreInputView, CoreInputViewOcclusion, CoreInputViewOcclusionKind,
};
use std::sync::{Arc, Mutex};
use windows_core::EventRevoker;
use windows_scene::Rect;
use windows_window::{Hwnd, WM_FRAME};

#[derive(Default)]
struct Pending {
    value: Option<Option<Rect>>,
}
pub(crate) struct Touch {
    view: Option<CoreInputView>,
    _subscriptions: Vec<EventRevoker>,
    pending: Arc<Mutex<Pending>>,
    pub docked: Option<Rect>,
}
impl Touch {
    pub fn new(hwnd: Hwnd) -> Self {
        let pending = Arc::new(Mutex::new(Pending::default()));
        let mut subscriptions = Vec::new();
        let view = match CoreInputView::GetForCurrentView() {
            Ok(view) => {
                let inbox = pending.clone();
                match view.OcclusionsChanged(move |_, args| {
                    if let Ok(args) = args.ok()
                        && let Ok(values) = args.Occlusions()
                    {
                        publish(&inbox, hwnd, &values);
                    }
                }) {
                    Ok(token) => subscriptions.push(token),
                    Err(error) => {
                        eprintln!("disabled capability CoreInputView.OcclusionsChanged: {error}");
                    }
                }
                Some(view)
            }
            Err(error) => {
                eprintln!("disabled capability CoreInputView: {error}");
                None
            }
        };
        match CoreFrameworkInputView::GetForCurrentView() {
            Ok(view) => {
                let inbox = pending.clone();
                match view.OcclusionsChanged(move |_, args| {
                    if let Ok(args) = args.ok()
                        && let Ok(values) = args.Occlusions()
                    {
                        publish(&inbox, hwnd, &values);
                    }
                }) {
                    Ok(token) => subscriptions.push(token),
                    Err(error) => eprintln!(
                        "disabled capability CoreFrameworkInputView.OcclusionsChanged: {error}"
                    ),
                }
            }
            Err(error) => eprintln!("disabled capability CoreFrameworkInputView: {error}"),
        }
        Self {
            view,
            _subscriptions: subscriptions,
            pending,
            docked: None,
        }
    }
    pub fn take(&mut self) -> bool {
        let value = self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .value
            .take();
        if let Some(value) = value {
            let changed = self.docked != value;
            self.docked = value;
            changed
        } else {
            false
        }
    }
    pub fn show(&self) {
        if let Some(view) = &self.view {
            let _ = view.TryShowPrimaryView();
        }
    }
}
fn publish(
    pending: &Mutex<Pending>,
    hwnd: Hwnd,
    values: &windows_collections::IVectorView<CoreInputViewOcclusion>,
) {
    let mut docked: Option<Rect> = None;
    if let Ok(count) = values.Size() {
        for index in 0..count {
            let Ok(value) = values.GetAt(index) else {
                continue;
            };
            if value.OcclusionKind().ok() != Some(CoreInputViewOcclusionKind::Docked) {
                continue;
            }
            let Ok(r) = value.OccludingRect() else {
                continue;
            };
            if r.y <= 0.0 || r.width <= 0.0 || r.height <= 0.0 {
                continue;
            }
            let r = Rect {
                x0: r.x,
                y0: r.y,
                x1: r.x + r.width,
                y1: r.y + r.height,
            };
            docked = Some(docked.map_or(r, |old| Rect {
                x0: old.x0.min(r.x0),
                y0: old.y0.min(r.y0),
                x1: old.x1.max(r.x1),
                y1: old.y1.max(r.y1),
            }));
        }
    }
    let first = {
        let mut slot = pending.lock().unwrap_or_else(|e| e.into_inner());
        let first = slot.value.is_none();
        slot.value = Some(docked);
        first
    };
    if first {
        hwnd.post(WM_FRAME, 0, 0);
    }
}
