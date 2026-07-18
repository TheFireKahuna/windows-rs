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
        let (sinks, revoker) = backend::dcomp::register_element_pointer(handle.id)?;
        return Ok(PointerSurface {
            inner: PointerInner::Dcomp {
                sinks,
                _revoker: revoker,
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
    /// DirectComposition: sinks the backend input router delivers to, with
    /// implicit capture for the press-to-release span (see
    /// `backend::dcomp::pointer`). The revoker unregisters on drop.
    #[cfg(feature = "dcomp-backend")]
    Dcomp {
        sinks: Rc<backend::dcomp::PointerSinks>,
        _revoker: Subscription,
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

    /// Subscribe `PointerPressed`. Also records the active pointer so a
    /// subsequent [`capture`](Self::capture) can grab it.
    pub fn on_down(&self, f: impl Fn(PointerEventInfo) + 'static) -> Result<&Self> {
        #[cfg(feature = "dcomp-backend")]
        if let PointerInner::Dcomp { sinks, .. } = &self.inner {
            *sinks.down.borrow_mut() = Some(Box::new(f));
            return Ok(self);
        }
        self.subscribe_pointer(f, true, |iue, h| iue.PointerPressed(h))?;
        Ok(self)
    }

    /// Subscribe `PointerPressed` and **capture** the pointer to this element as
    /// part of the same handler, so a drag that leaves the element keeps
    /// delivering `PointerMoved`. Convenience for the common scrub / drag start;
    /// pair with [`release`](Self::release) on the matching up. (The
    /// DirectComposition backend captures implicitly for every surface press,
    /// so there this is identical to [`on_down`](Self::on_down).)
    pub fn on_down_capture(&self, f: impl Fn(PointerEventInfo) + 'static) -> Result<&Self> {
        #[cfg(feature = "dcomp-backend")]
        if let PointerInner::Dcomp { sinks, .. } = &self.inner {
            *sinks.down.borrow_mut() = Some(Box::new(f));
            return Ok(self);
        }
        let PointerInner::Xaml {
            element, captured, ..
        } = &self.inner
        else {
            return Ok(self);
        };
        let element = element.clone();
        let captured = captured.clone();
        self.subscribe_pointer(
            move |info| {
                if let Some(p) = captured.borrow().as_ref() {
                    let _ = element.CapturePointer(p);
                }
                f(info);
            },
            true,
            |iue, h| iue.PointerPressed(h),
        )?;
        Ok(self)
    }

    /// Subscribe `PointerMoved`. Also refreshes the active pointer.
    pub fn on_move(&self, f: impl Fn(PointerEventInfo) + 'static) -> Result<&Self> {
        #[cfg(feature = "dcomp-backend")]
        if let PointerInner::Dcomp { sinks, .. } = &self.inner {
            *sinks.moved.borrow_mut() = Some(Box::new(f));
            return Ok(self);
        }
        self.subscribe_pointer(f, true, |iue, h| iue.PointerMoved(h))?;
        Ok(self)
    }

    /// Subscribe `PointerReleased`.
    pub fn on_up(&self, f: impl Fn(PointerEventInfo) + 'static) -> Result<&Self> {
        #[cfg(feature = "dcomp-backend")]
        if let PointerInner::Dcomp { sinks, .. } = &self.inner {
            *sinks.up.borrow_mut() = Some(Box::new(f));
            return Ok(self);
        }
        self.subscribe_pointer(f, false, |iue, h| iue.PointerReleased(h))?;
        Ok(self)
    }

    /// Subscribe pointer-exit: the hover left this element's bounds (moved onto
    /// another surface, onto none, or out of the window). Hover-only — an
    /// implicitly captured drag keeps delivering moves and fires no exit until
    /// after release. The natural end-of-hover signal for surfaces that light up
    /// under the pointer.
    pub fn on_exit(&self, f: impl Fn() + 'static) -> Result<&Self> {
        #[cfg(feature = "dcomp-backend")]
        if let PointerInner::Dcomp { sinks, .. } = &self.inner {
            *sinks.exited.borrow_mut() = Some(Box::new(f));
            return Ok(self);
        }
        self.subscribe_pointer(move |_| f(), false, |iue, h| iue.PointerExited(h))?;
        Ok(self)
    }

    /// Subscribe `PointerWheelChanged`; read [`PointerEventInfo::wheel_delta`].
    pub fn on_wheel(&self, f: impl Fn(PointerEventInfo) + 'static) -> Result<&Self> {
        #[cfg(feature = "dcomp-backend")]
        if let PointerInner::Dcomp { sinks, .. } = &self.inner {
            *sinks.wheel.borrow_mut() = Some(Box::new(f));
            return Ok(self);
        }
        self.subscribe_pointer(f, false, |iue, h| iue.PointerWheelChanged(h))?;
        Ok(self)
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
        // Release any held capture; the EventRevokers (XAML subscriptions or the
        // dcomp registry entry) revoke on their own Drop.
        self.release();
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
