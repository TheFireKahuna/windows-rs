use super::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Live pointer interop bound to one native `UIElement` (obtained from a
/// [`SwapChainPanelHandle::pointer_surface`]).
///
/// Declare the pointer transitions you care about with [`on_gesture`](Self::on_gesture) —
/// a [`GestureInterest`](crate::GestureInterest) routes them all at once, so a surface is
/// never half-registered — then [`capture`](Self::capture) on a down that begins a drag and
/// [`release`](Self::release) on the matching up. The subscriptions and any
/// outstanding capture are torn down when the surface is dropped, so store it
/// alongside the panel (e.g. in a `use_ref`).
///
/// The most recently seen pointer is tracked internally on every down / move, so
/// [`capture`](Self::capture) works without the caller threading a pointer id
/// through.
pub struct PointerSurface {
    inner: PointerInner,
}

enum PointerInner {
    /// WinUI: XAML pointer events + explicit `CapturePointer`.
    Xaml {
        element: bindings::UIElement,
        captured: Rc<RefCell<Option<bindings::Pointer>>>,
        revokers: RefCell<Vec<windows_core::EventRevoker>>,
    },
    /// DirectComposition: the node the input router delivers gesture
    /// transitions to, with implicit capture for the press-to-release span (see
    /// `backend::dcomp::pointer`).
    ///
    /// Nothing is registered until [`on_gesture`](PointerSurface::on_gesture) —
    /// opening a surface declares no interest, so an element with no gesture is
    /// wholly inert rather than routed-but-empty. `action` holds the app-side
    /// drain's subscription, which unregisters it on drop; the front-side
    /// gesture is forgotten in this type's `Drop`.
    Dcomp {
        #[cfg_attr(
            not(feature = "dcomp-backend"),
            expect(dead_code, reason = "read only by the DComp gesture router")
        )]
        id: ControlId,
        #[cfg_attr(
            not(feature = "dcomp-backend"),
            expect(dead_code, reason = "holds the DComp action subscription")
        )]
        action: RefCell<Option<Subscription>>,
    },
}

impl PointerSurface {
    fn subscribe_pointer<S>(
        &self,
        f: impl Fn(PointerEventInfo) + 'static,
        track: bool,
        subscribe: S,
    ) -> Result<()>
    where
        S: FnOnce(
            &bindings::IUIElement,
            Box<
                dyn Fn(
                    windows_core::Ref<windows_core::IInspectable>,
                    windows_core::Ref<bindings::PointerRoutedEventArgs>,
                ),
            >,
        ) -> Result<windows_core::EventRevoker>,
    {
        let PointerInner::Xaml {
            element,
            captured,
            revokers,
        } = &self.inner
        else {
            unreachable!("subscribe_pointer is only reached from the Xaml arms");
        };
        let iue: bindings::IUIElement = element.cast()?;
        let captured = captured.clone();
        let handler = Box::new(
            move |sender: windows_core::Ref<windows_core::IInspectable>,
                  args: windows_core::Ref<bindings::PointerRoutedEventArgs>| {
                if track
                    && let Some(a) = args.as_ref()
                    && let Ok(iargs) = a.cast::<bindings::IPointerRoutedEventArgs>()
                {
                    *captured.borrow_mut() = iargs.Pointer().ok();
                }
                f(pointer_event_info(sender, args));
            },
        );
        let revoker = subscribe(&iue, handler)?;
        revokers.borrow_mut().push(revoker);
        Ok(())
    }

    /// Install this surface's **gesture** — the handler that turns pointer
    /// transitions into visible change, and the only way to get input-driven
    /// feedback out of a custom-drawn element.
    ///
    /// `gesture` runs wherever input is routed. On the DirectComposition backend
    /// that is the front thread, *inside the input router*, with the transition
    /// handled and whatever it publishes committed before the app is told
    /// anything at all. `on_action` runs on the app thread afterwards, and only
    /// when the gesture asked for it.
    ///
    /// ## Why `gesture` is `Send`
    ///
    /// Because it must be able to run on the input thread, and the bound is what
    /// makes that checkable. A closure that captured a `HookRef`, a `Dispatch`,
    /// or anything else living in app-thread render state will not compile here
    /// — which is the point: the previous design accepted such closures happily,
    /// deferred them across the seam, and so put reconcile load in the path of
    /// every drag. See [`GestureInterest`](crate::GestureInterest).
    ///
    /// A gesture owns its live state (drag anchor, hovered index) by capturing
    /// it `mut`, and publishes through whatever shared channel its renderer
    /// reads — typically an `Arc` draw slot. The reactor never sees that slot.
    ///
    /// ## The action half
    ///
    /// Anything the app must *persist* travels as an action: the gesture posts
    /// the newest one to an [`ActionSlot`](crate::ActionSlot) it captured and
    /// returns [`GestureOutcome::Notify`]; `on_action` then drains it. Posting
    /// coalesces, so a burst of moves wakes the app once and it applies only the
    /// newest — the notification path gets cheaper under load, not dearer.
    ///
    /// `interest` declares which transitions to route, all at once, so a surface
    /// is never half-registered. A gesture without
    /// [`down`](crate::GestureInterest::down) leaves the element
    /// click-transparent.
    pub fn on_gesture<G>(
        &self,
        interest: GestureInterest,
        gesture: G,
        on_action: impl Fn() + 'static,
    ) -> Result<&Self>
    where
        G: FnMut(GestureEvent) -> GestureOutcome + Send + 'static,
    {
        #[cfg(feature = "dcomp-backend")]
        if let PointerInner::Dcomp { id, action, .. } = &self.inner {
            dcomp::declare_gesture(*id, interest, Box::new(gesture));
            *action.borrow_mut() =
                Some(dcomp::register_gesture_action(*id, Callback::new(move |()| on_action())));
            return Ok(self);
        }
        self.subscribe_xaml_gesture(interest, gesture, on_action)?;
        Ok(self)
    }

    /// The XAML arm of [`on_gesture`](Self::on_gesture): subscribe each
    /// interested transition and run the gesture in the handler.
    ///
    /// There is no seam on this backend — input, render and app state are all
    /// the one thread — so `on_action` is invoked inline the moment the gesture
    /// asks for it. The coalescing an [`ActionSlot`](crate::ActionSlot) performs
    /// is then merely harmless rather than load-bearing.
    #[allow(unused_variables)]
    fn subscribe_xaml_gesture<G>(
        &self,
        interest: GestureInterest,
        gesture: G,
        on_action: impl Fn() + 'static,
    ) -> Result<()>
    where
        G: FnMut(GestureEvent) -> GestureOutcome + Send + 'static,
    {
        let PointerInner::Xaml {
            element, captured, ..
        } = &self.inner
        else {
            return Ok(());
        };

        // One gesture shared by up to five event handlers. `RefCell` rather than
        // a plain `Rc` because the handler is `FnMut`; XAML pointer events do not
        // nest, so the borrow is never contended.
        let gesture = Rc::new(RefCell::new(gesture));
        let on_action = Rc::new(on_action);

        // Build the per-transition sink once: run the gesture, and honour a
        // `Notify` immediately.
        macro_rules! sink {
            ($make:expr) => {{
                let g = gesture.clone();
                let a = on_action.clone();
                let make = $make;
                move |info: PointerEventInfo| {
                    if (g.borrow_mut())(make(info)) == GestureOutcome::Notify {
                        a();
                    }
                }
            }};
        }

        if interest.down {
            // Capture on press so a drag that leaves the element keeps
            // delivering moves — the DComp router does this implicitly.
            let element = element.clone();
            let captured = captured.clone();
            let inner = sink!(GestureEvent::Down);
            self.subscribe_pointer(
                move |info| {
                    if let Some(p) = captured.borrow().as_ref() {
                        let _ = element.CapturePointer(p);
                    }
                    inner(info);
                },
                true,
                |iue, h| iue.PointerPressed(h),
            )?;
        }
        if interest.moved {
            self.subscribe_pointer(sink!(GestureEvent::Move), true, |iue, h| {
                iue.PointerMoved(h)
            })?;
        }
        if interest.up {
            self.subscribe_pointer(sink!(GestureEvent::Up), false, |iue, h| {
                iue.PointerReleased(h)
            })?;
        }
        if interest.wheel {
            self.subscribe_pointer(sink!(GestureEvent::Wheel), false, |iue, h| {
                iue.PointerWheelChanged(h)
            })?;
        }
        if interest.exited {
            let g = gesture;
            let a = on_action;
            self.subscribe_pointer(
                move |_| {
                    if (g.borrow_mut())(GestureEvent::Exit) == GestureOutcome::Notify {
                        a();
                    }
                },
                false,
                |iue, h| iue.PointerExited(h),
            )?;
        }
        Ok(())
    }

    /// Capture the most recently seen pointer to this element for the duration
    /// of a drag, so moves keep arriving even when the pointer leaves the
    /// element bounds. Call on a `Down` that begins a scrub / drag; pair with
    /// [`release`](Self::release). No-op if no pointer has been seen yet, and on
    /// the DirectComposition backend (capture there is implicit per press).
    pub fn capture(&self) {
        if let PointerInner::Xaml {
            element, captured, ..
        } = &self.inner
            && let Some(p) = captured.borrow().as_ref()
        {
            let _ = element.CapturePointer(p);
        }
    }

    /// Release a pointer captured by [`capture`](Self::capture) (call on `Up`).
    pub fn release(&self) {
        if let PointerInner::Xaml {
            element, captured, ..
        } = &self.inner
            && let Some(p) = captured.borrow().as_ref()
        {
            let _ = element.ReleasePointerCapture(p);
        }
    }
}

impl Drop for PointerSurface {
    fn drop(&mut self) {
        // Release any held capture; the XAML EventRevokers and the app-side
        // action subscription revoke on their own Drop.
        self.release();
        // The front-side gesture is not owned by a Subscription — it lives in
        // the router's own map, reached only through the ops queue — so forget
        // it explicitly. Ordering behind any pending declaration is what the
        // queue is for.
        #[cfg(feature = "dcomp-backend")]
        if let PointerInner::Dcomp { id, .. } = &self.inner {
            dcomp::forget_gesture(*id);
        }
    }
}

/// Build a [`PointerEventInfo`] from a routed-pointer event. Mirrors the backend
/// `pointer_event_info`: element-relative DIP position (relative to the sender),
/// button state, and signed wheel delta.
fn pointer_event_info(
    sender: windows_core::Ref<windows_core::IInspectable>,
    args: windows_core::Ref<bindings::PointerRoutedEventArgs>,
) -> PointerEventInfo {
    let mut info = PointerEventInfo::default();
    let Some(args) = args.as_ref() else {
        return info;
    };
    let Ok(iargs) = args.cast::<bindings::IPointerRoutedEventArgs>() else {
        return info;
    };
    let relative: Option<bindings::UIElement> = sender
        .as_ref()
        .and_then(|s| s.cast::<bindings::UIElement>().ok());
    let point = match relative.as_ref() {
        Some(ue) => iargs.GetCurrentPoint(ue),
        None => iargs.GetCurrentPoint(None),
    };
    let Ok(point) = point else {
        return info;
    };
    let Ok(ipoint) = point.cast::<bindings::IPointerPoint>() else {
        return info;
    };
    if let Ok(pos) = ipoint.Position() {
        info.x = pos.x as f64;
        info.y = pos.y as f64;
    }
    let Ok(props) = ipoint.Properties() else {
        return info;
    };
    let Ok(iprops) = props.cast::<bindings::IPointerPointProperties>() else {
        return info;
    };
    info.is_left_button_pressed = iprops.IsLeftButtonPressed().unwrap_or(false);
    info.is_right_button_pressed = iprops.IsRightButtonPressed().unwrap_or(false);
    info.is_middle_button_pressed = iprops.IsMiddleButtonPressed().unwrap_or(false);
    info.wheel_delta = iprops.MouseWheelDelta().unwrap_or(0);
    info
}

/// Open a capture-capable [`PointerSurface`] over any mounted native element.
/// Shared by [`SwapChainPanelHandle::pointer_surface`] and
/// [`ElementHandle::pointer_surface`]. On the WinUI backend the native object
/// is a XAML `UIElement` (pointer events + `CapturePointer`); on the
/// DirectComposition backend it is the node's system `ContainerVisual`, and
/// the subscription registers with the backend's pointer registry instead
/// (element-relative delivery + implicit capture — see `backend::dcomp::pointer`).
pub(crate) fn xaml_pointer_surface(native: &windows_core::IInspectable) -> Option<PointerSurface> {
    let element = native.cast::<bindings::UIElement>().ok()?;
    Some(PointerSurface {
        inner: PointerInner::Xaml {
            element,
            captured: Rc::new(RefCell::new(None)),
            revokers: RefCell::new(Vec::new()),
        },
    })
}

pub(crate) fn open_pointer_surface(handle: &ElementHandle) -> Result<PointerSurface> {
    if let Some(native) = &handle.native
        && let Some(surface) = xaml_pointer_surface(native)
    {
        return Ok(surface);
    }
    Ok(PointerSurface {
        inner: PointerInner::Dcomp {
            id: handle.id,
            action: RefCell::new(None),
        },
    })
}
