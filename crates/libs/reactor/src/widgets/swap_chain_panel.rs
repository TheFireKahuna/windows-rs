use super::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Opaque handle to the native `SwapChainPanel` control, passed to the
/// Opaque handle to the native `SwapChainPanel` control, passed to the
/// [`on_mounted`](SwapChainPanel::on_mounted) callback.
#[derive(Clone)]
pub struct SwapChainPanelHandle(windows_core::IInspectable);

impl SwapChainPanelHandle {
    /// Attach a DXGI swap chain (created with `CreateSwapChainForComposition`).
    ///
    /// # Safety contract
    /// The caller must pass a valid `IDXGISwapChain` (or `IDXGISwapChain1`).
    /// Passing an unrelated COM interface will fail at the WinUI layer.
    pub fn set_swap_chain(&self, swap_chain: &impl Interface) -> Result<()> {
        let native: bindings::ISwapChainPanelNative = self.0.cast()?;
        unsafe { native.SetSwapChain(swap_chain.as_raw()).ok() }
    }

    /// Returns the current composition scale (DPI scale factor) as `(scale_x, scale_y)`.
    ///
    /// Multiply DIP dimensions by these values to get pixel dimensions for the swap chain.
    /// Typically both values are equal (e.g., 1.5 at 150% display scaling).
    pub fn composition_scale(&self) -> Result<(f32, f32)> {
        let panel: bindings::ISwapChainPanel = self.0.cast()?;
        let x = panel.CompositionScaleX()?;
        let y = panel.CompositionScaleY()?;
        Ok((x, y))
    }

    /// Subscribe to composition scale changes (e.g., window moved to a different monitor).
    ///
    /// The callback receives `(scale_x, scale_y)`.
    pub fn on_composition_scale_changed(
        &self,
        f: impl Fn(f32, f32) + 'static,
    ) -> Result<windows_core::EventRevoker> {
        let panel: bindings::ISwapChainPanel = self.0.cast()?;
        panel.CompositionScaleChanged(move |sender, _| {
            if let Some(sender) = sender.as_ref() {
                let scp: &bindings::ISwapChainPanel = sender;
                let x = scp.CompositionScaleX().unwrap_or(1.0);
                let y = scp.CompositionScaleY().unwrap_or(1.0);
                f(x, y);
            }
        })
    }

    /// Open a live [`PointerSurface`] over this panel's native `UIElement`.
    ///
    /// Unlike the declarative [`on_pointer_moved`](crate::ElementExt::on_pointer_moved)
    /// / [`on_pointer_wheel`](crate::ElementExt::on_pointer_wheel) modifiers, this
    /// imperative surface also supports **pointer capture** — the piece a knob /
    /// slider / EQ-node drag needs so moves keep arriving when the pointer leaves
    /// the element bounds. Subscribe down / move / up / wheel via the returned
    /// surface's builder methods; the registrations (and any held capture) are
    /// revoked on `Drop`. Coordinates in every [`PointerEventInfo`] are
    /// element-relative DIPs.
    pub fn pointer_surface(&self) -> Result<PointerSurface> {
        xaml_pointer_surface(&self.0).ok_or_else(Error::empty)
    }
}

/// Open a capture-capable [`PointerSurface`] over any mounted native element.
/// Shared by [`SwapChainPanelHandle::pointer_surface`] and
/// [`ElementHandle::pointer_surface`]. On the WinUI backend the native object
/// is a XAML `UIElement` (pointer events + `CapturePointer`); on the
/// DirectComposition backend it is the node's system `ContainerVisual`, and
/// the subscription registers with the backend's pointer registry instead
/// (element-relative delivery + implicit capture — see `backend::dcomp::pointer`).
fn xaml_pointer_surface(native: &windows_core::IInspectable) -> Option<PointerSurface> {
    let element = native.cast::<bindings::UIElement>().ok()?;
    Some(PointerSurface {
        inner: PointerInner::Xaml {
            element,
            captured: Rc::new(RefCell::new(None)),
            revokers: RefCell::new(Vec::new()),
        },
    })
}

fn open_pointer_surface(handle: &ElementHandle) -> Result<PointerSurface> {
    if let Some(native) = &handle.native
        && let Some(surface) = xaml_pointer_surface(native)
    {
        return Ok(surface);
    }
    #[cfg(feature = "dcomp-backend")]
    {
        return Ok(PointerSurface {
            inner: PointerInner::Dcomp {
                id: handle.id,
                action: RefCell::new(None),
            },
        });
    }
    #[cfg(not(feature = "dcomp-backend"))]
    Err(Error::empty())
}

/// A live subscription — an element's size notifications, a pointer sink —
/// that ends when this drops.
///
/// Two shapes sit behind it. The WinUI backend subscribes a WinRT event on the
/// native element and hands back its `EventRevoker`. The DirectComposition
/// backend registers against an id-keyed backend registry and hands back only a
/// token: **no COM, so the holder is not pinned to the backend's thread**, which
/// is what lets app code retain one across renders (and, once the reconciler
/// moves off the UI thread, across threads).
pub struct Subscription(SubscriptionInner);

enum SubscriptionInner {
    /// WinRT event registration; revokes itself on drop.
    Winrt(#[allow(dead_code, reason = "revocation is the Drop impl")] windows_core::EventRevoker),
    /// Token in an id-keyed backend registry, removed by `remove` on drop.
    Token { token: i64, remove: fn(i64) },
}

impl Subscription {
    pub(crate) fn winrt(revoker: windows_core::EventRevoker) -> Self {
        Self(SubscriptionInner::Winrt(revoker))
    }

    pub(crate) fn token(token: i64, remove: fn(i64)) -> Self {
        Self(SubscriptionInner::Token { token, remove })
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        // The WinRT arm revokes through `EventRevoker`'s own `Drop`.
        if let SubscriptionInner::Token { token, remove } = &self.0 {
            remove(*token);
        }
    }
}

/// Handle to a mounted control, given to a widget's `on_mounted` callback
/// (e.g. [`Image::on_mounted`](crate::Image::on_mounted)).
///
/// It carries the control's [`id`](Self::id) — the address the backend's
/// id-keyed services take — and, on backends that expose one, the native
/// element. Its imperative, capture-capable
/// [`PointerSurface`](Self::pointer_surface) is what a custom-drawn control
/// hosted in an `Image` / `SurfaceImageSource` needs so a knob / slider / node
/// drag keeps tracking after the pointer leaves the element bounds — the
/// declarative [`on_pointer_moved`](crate::ElementExt::on_pointer_moved)
/// modifier cannot capture.
#[derive(Clone)]
pub struct ElementHandle {
    pub(crate) id: ControlId,
    pub(crate) native: Option<windows_core::IInspectable>,
}

#[cfg(feature = "dcomp-backend")]
impl ElementHandle {
    /// A `Send` handle for replacing this control's text from any thread,
    /// without a reconcile.
    ///
    /// For values a render pump owns — a level readout, a transport clock —
    /// which change at display rate and never travel as props. The words are
    /// queued, coalesced per control, and applied on the compositor thread,
    /// where they shape at placement time; see
    /// [`live_text`](crate::backend::dcomp::live_text).
    ///
    /// Only meaningful on a `TextBlock`: any other control ignores the words.
    /// Keeping a handle past the control's unmount is safe — updates to an id
    /// that no longer resolves are dropped.
    pub fn live_text(&self) -> crate::backend::dcomp::live_text::LiveText {
        crate::backend::dcomp::live_text::LiveText::new(self.id)
    }
}

impl From<MountInfo> for ElementHandle {
    fn from(info: MountInfo) -> Self {
        Self {
            id: info.id,
            native: info.native,
        }
    }
}

impl ElementHandle {
    /// The control's backend id — the address every id-keyed backend service
    /// takes (size subscription, pointer sinks, composition-surface hosting).
    ///
    /// Unlike [`native`](Self::native) this is always present, `Copy`, and
    /// `Send`, so retaining it across renders pins nothing to a thread.
    pub fn id(&self) -> ControlId {
        self.id
    }

    /// Open a live [`PointerSurface`] over this element.
    /// See [`SwapChainPanelHandle::pointer_surface`] for the capture semantics.
    pub fn pointer_surface(&self) -> Result<PointerSurface> {
        open_pointer_surface(self)
    }

    /// The underlying native object, when the backend exposes one: the XAML
    /// `UIElement` on the WinUI backend, where interfaces such as
    /// `ISwapChainPanelNative` are only reachable through it.
    ///
    /// The DirectComposition backend returns `None` — its controls are
    /// addressed by [`id`](Self::id), so nothing hands out a thread-affine
    /// visual that app code could retain. Reach for this only on a WinUI-only
    /// path; anything with an id-keyed equivalent should use that instead.
    pub fn native(&self) -> Option<&windows_core::IInspectable> {
        self.native.as_ref()
    }

    /// Subscribe `SizeChanged` on this element; the callback receives the new
    /// `(width, height)` in DIPs and also fires once after the first layout
    /// pass. Returns the [`EventRevoker`](windows_core::EventRevoker) — **store
    /// it** (e.g. in a `use_ref`, alongside the [`PointerSurface`]); the
    /// subscription is revoked when the revoker drops (on unmount), so nothing
    /// leaks. Use it to recreate a fixed-size [`SurfaceImageSource`] at the new
    /// size so it stays crisp.
    pub fn on_size_changed(&self, f: impl Fn(f64, f64) + 'static) -> Result<Subscription> {
        // WinUI backend: the native element is a XAML FrameworkElement.
        if let Some(native) = &self.native
            && let Ok(fe) = native.cast::<bindings::IFrameworkElement>()
        {
            return fe
                .SizeChanged(move |_sender, args| {
                    if let Some(args) = args.as_ref()
                        && let Ok(s) = args.NewSize()
                    {
                        f(s.width as f64, s.height as f64);
                    }
                })
                .map(Subscription::winrt);
        }
        // DirectComposition backend: the control's size is whatever the Taffy
        // layout pass last assigned its node. Register with the backend's
        // id-keyed size registry, which fires on a change (see
        // `backend::dcomp::fire_element_size`). Returns an `EventRevoker` that
        // unregisters on drop, so the call site matches the XAML path.
        #[cfg(feature = "dcomp-backend")]
        {
            use backend::dcomp::register_element_size;
            return register_element_size(self.id, move |w, h| {
                f(w as f64, h as f64);
            });
        }
        #[cfg(not(feature = "dcomp-backend"))]
        Err(Error::empty())
    }
}

/// Live pointer interop bound to one native `UIElement` (obtained from a
/// [`SwapChainPanelHandle::pointer_surface`]).
///
/// Subscribe the pointer transitions you care about with [`on_down`](Self::on_down)
/// / [`on_move`](Self::on_move) / [`on_up`](Self::on_up) / [`on_wheel`](Self::on_wheel),
/// then [`capture`](Self::capture) on a down that begins a drag and
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
    #[cfg(feature = "dcomp-backend")]
    Dcomp {
        id: ControlId,
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
    /// every drag. See [`crate::gesture`].
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
            let g = gesture.clone();
            let a = on_action.clone();
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

/// Built-in widget for `Microsoft.UI.Xaml.Controls.SwapChainPanel` — hosts
/// custom Direct3D / Direct2D rendering inside a WinUI 3 XAML tree.
///
/// Use [`on_mounted`](SwapChainPanel::on_mounted) to receive a
/// [`SwapChainPanelHandle`] for attaching your DXGI swap chain.
#[derive(Clone, Debug, PartialEq)]
pub struct SwapChainPanel {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub mounted: Option<Callback<MountInfo>>,
    pub unmounted: Option<Callback<MountInfo>>,
}

impl Default for SwapChainPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SwapChainPanel {
    pub fn new() -> Self {
        Self {
            key: None,
            modifiers: Modifiers::default(),
            mounted: None,
            unmounted: None,
        }
    }

    /// Callback invoked once after the native control is created.
    pub fn on_mounted(mut self, f: impl Fn(SwapChainPanelHandle) + 'static) -> Self {
        // A `SwapChainPanel` always has a native control in practice; the
        // handle is only built when one is present.
        self.mounted = Some(Callback::new(move |info: MountInfo| {
            if let Some(native) = info.native {
                f(SwapChainPanelHandle(native));
            }
        }));
        self
    }

    /// Callback invoked just before the native control is destroyed, while it
    /// still exists. Use this to tear down resources bound to the panel (for
    /// example, stop and join a render thread that presents into its swap
    /// chain) before the panel — and its swap chain — go away.
    pub fn on_unmounted(mut self, f: impl Fn(SwapChainPanelHandle) + 'static) -> Self {
        self.unmounted = Some(Callback::new(move |info: MountInfo| {
            if let Some(native) = info.native {
                f(SwapChainPanelHandle(native));
            }
        }));
        self
    }

    /// Callback invoked when the panel's layout size changes (width, height in
    /// DIPs). Also fires once after the first layout pass. Use this to resize
    /// your swap chain buffers.
    pub fn on_resize(mut self, f: impl Fn(f64, f64) + 'static) -> Self {
        let f = Rc::new(f);
        let prev = self.mounted.take();
        self.mounted = Some(Callback::new(move |info: MountInfo| {
            if let Some(ref cb) = prev {
                cb.invoke(info.clone());
            }
            let Some(native) = info.native else {
                return;
            };
            // Subscribe to SizeChanged on the FrameworkElement.
            if let Ok(fe) = native.cast::<bindings::IFrameworkElement>() {
                let f = f.clone();
                // Store the revoker so the subscription lives as long as the control.
                let revoker: Rc<RefCell<Option<windows_core::EventRevoker>>> =
                    Rc::new(RefCell::new(None));
                let r = fe.SizeChanged(move |_sender, args| {
                    if let Some(args) = args.as_ref()
                        && let Ok(s) = args.NewSize()
                    {
                        f(s.width as f64, s.height as f64);
                    }
                });
                if let Ok(revoker_val) = r {
                    *revoker.borrow_mut() = Some(revoker_val);
                    // Leak the Rc so the subscription outlives this scope.
                    // The revoker prevent leaks — it revokes on Drop when
                    // the control is destroyed.
                    std::mem::forget(revoker);
                }
            }
        }));
        self
    }
}

impl Widget for SwapChainPanel {
    widget_header!(ControlKind::SwapChainPanel);
    fn bindings(&self) -> PropBindings {
        Vec::new()
    }
    fn on_mounted_callback(&self) -> Option<&Callback<MountInfo>> {
        self.mounted.as_ref()
    }
    fn on_unmounted_callback(&self) -> Option<&Callback<MountInfo>> {
        self.unmounted.as_ref()
    }
}

/// Factory function for a [`SwapChainPanel`].
pub fn swap_chain_panel() -> SwapChainPanel {
    SwapChainPanel::new()
}
