use super::*;

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

impl ElementHandle {
    /// A `Send` handle for replacing this control's text from any thread,
    /// without a reconcile.
    ///
    /// For values a render pump owns — a level readout, a transport clock —
    /// which change at display rate and never travel as props. The words are
    /// queued, coalesced per control, and applied on the compositor thread,
    /// where they shape at placement time; see
    /// `live_text`.
    ///
    /// Only meaningful on a `TextBlock`: any other control ignores the words.
    /// Keeping a handle past the control's unmount is safe — updates to an id
    /// that no longer resolves are dropped.
    ///
    /// DComp backend only: live text is applied on the compositor thread at
    /// glyph-placement time, which the XAML backend has no counterpart for.
    #[cfg(feature = "dcomp-backend")]
    pub fn live_text(&self) -> LiveText {
        LiveText::new(self.id)
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
            register_element_size(self.id, move |w, h| {
                f(w as f64, h as f64);
            })
        }
        // With no DComp backend compiled there is no id-keyed registry to fall
        // back to, so an element that is not a native XAML `FrameworkElement`
        // has no size to report. Hand back an inert subscription rather than an
        // error: the contract is already that a handle whose id no longer
        // resolves silently stops firing, and a caller storing it behaves the
        // same either way.
        #[cfg(not(feature = "dcomp-backend"))]
        {
            let _ = f;
            Ok(Subscription::token(0, |_| {}))
        }
    }
}
