use super::*;
use std::cell::RefCell;
use std::rc::Rc;

/// Opaque handle to the native `Image` control, promoted from the
/// [`ElementHandle`] that [`on_mounted`](Image::on_mounted) delivers.
#[derive(Clone)]
pub struct ImageHandle(windows_core::IInspectable);

impl ImageHandle {
    /// Promote the [`ElementHandle`] handed to [`Image::on_mounted`] into an
    /// `ImageHandle`.
    ///
    /// Returns `None` when the element has no native XAML object behind it —
    /// i.e. under the self-hosted DirectComposition backend, which has no
    /// `XamlRoot` to report a rasterization scale. There, take the scale from
    /// [`RenderCx::use_dpi`](crate::RenderCx::use_dpi) instead.
    pub fn from_element(element: &ElementHandle) -> Option<Self> {
        element.native().cloned().map(Self)
    }

    /// Deliver the host's rasterization (DPI) scale to `f`: once the control is
    /// loaded into the tree, and again whenever the scale changes (for example the
    /// window moves to a monitor with different scaling).
    ///
    /// The scale is `1.0` at 96 DPI, `1.5` at 150%, `2.0` at 192 DPI. Multiply a
    /// device-independent size by it to allocate a crisp on-demand surface — pass
    /// the result as the `scale` argument to `CanvasImageSource::new`. The scale
    /// is only available once the control is loaded, so it is delivered through
    /// this callback rather than returned directly.
    ///
    /// Keep the returned [`EventRevoker`](windows_core::EventRevoker) alive for as
    /// long as you want updates; dropping it revokes both this and the underlying
    /// scale-changed subscription.
    pub fn on_rasterization_scale_changed(
        &self,
        f: impl Fn(f64) + 'static,
    ) -> Result<windows_core::EventRevoker> {
        let element: bindings::IFrameworkElement = self.0.cast()?;
        let f = Rc::new(f);
        // Owned by the `Loaded` closure so it is revoked when the returned
        // `Loaded` revoker is dropped.
        let changed: Rc<RefCell<Option<windows_core::EventRevoker>>> = Rc::new(RefCell::new(None));
        element.Loaded(move |sender, _| {
            let Some(element) = sender.as_ref().and_then(|s| s.cast::<bindings::IUIElement>().ok())
            else {
                return;
            };
            let Ok(root) = element.XamlRoot() else {
                return;
            };
            if let Ok(scale) = root.RasterizationScale() {
                f(scale);
            }
            let f = f.clone();
            let revoker = root.Changed(move |sender, _| {
                if let Some(sender) = sender.as_ref()
                    && let Ok(scale) = sender.RasterizationScale()
                {
                    f(scale);
                }
            });
            *changed.borrow_mut() = revoker.ok();
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub source: ImageSource,
    pub stretch: Stretch,
    pub mounted: Option<Callback<MountInfo>>,
}
impl Default for Image {
    fn default() -> Self {
        Self {
            key: None,
            modifiers: Modifiers::default(),
            source: ImageSource::default(),
            stretch: Stretch::Uniform,
            mounted: None,
        }
    }
}
#[derive(Clone, Default, Debug, PartialEq)]
pub enum ImageSource {
    #[default]
    None,
    Uri(String),
    Surface(SurfaceImageSource),
    Virtual(VirtualSurfaceImageSource),
}
impl From<SurfaceImageSource> for ImageSource {
    fn from(source: SurfaceImageSource) -> Self {
        Self::Surface(source)
    }
}
impl From<Option<SurfaceImageSource>> for ImageSource {
    fn from(source: Option<SurfaceImageSource>) -> Self {
        source.map_or(Self::None, ImageSource::Surface)
    }
}
impl From<VirtualSurfaceImageSource> for ImageSource {
    fn from(source: VirtualSurfaceImageSource) -> Self {
        Self::Virtual(source)
    }
}
impl From<Option<VirtualSurfaceImageSource>> for ImageSource {
    fn from(source: Option<VirtualSurfaceImageSource>) -> Self {
        source.map_or(Self::None, ImageSource::Virtual)
    }
}
impl Image {
    pub fn new(source: ImageSource) -> Self {
        Self {
            source,
            ..Default::default()
        }
    }

    pub fn new_with_uri(source: impl Into<String>) -> Self {
        Self::new(ImageSource::Uri(source.into()))
    }

    pub fn stretch(mut self, v: Stretch) -> Self {
        self.stretch = v;
        self
    }

    /// Callback invoked once after the native control is created, handed an
    /// [`ElementHandle`] for the native `Image` element. Use it to open a
    /// capture-capable [`PointerSurface`](crate::PointerSurface) over an
    /// `Image` that hosts a custom-drawn [`SurfaceImageSource`] — so a knob /
    /// slider / node drag keeps tracking past the element bounds — and to
    /// subscribe [`ElementHandle::on_size_changed`](crate::ElementHandle::on_size_changed)
    /// to recreate a fixed-size `SurfaceImageSource` when the layout resizes.
    ///
    /// On the WinUI backend, [`ImageHandle::from_element`] promotes the handle to
    /// an [`ImageHandle`] to observe the host rasterization (DPI) scale.
    pub fn on_mounted(mut self, f: impl Fn(ElementHandle) + 'static) -> Self {
        self.mounted = Some(Callback::new(move |info: MountInfo| {
            f(ElementHandle::from(info));
        }));
        self
    }
}

impl Widget for Image {
    widget_header!(ControlKind::Image);
    fn on_mounted_callback(&self) -> Option<&Callback<MountInfo>> {
        self.mounted.as_ref()
    }
    fn bindings(&self) -> PropBindings {
        let mut out = generated::image_bindings(self);
        // ImageSource is a compound type not expressible in TOML.
        match &self.source {
            ImageSource::Uri(uri) => {
                out.push(Binding::Prop(Prop::ImageSource, PropValue::Str(uri.clone())));
            }
            ImageSource::Surface(s) => {
                out.push(Binding::Prop(
                    Prop::ImageSource,
                    PropValue::SurfaceImageSource(s.clone()),
                ));
            }
            ImageSource::Virtual(s) => {
                out.push(Binding::Prop(
                    Prop::ImageSource,
                    PropValue::VirtualSurfaceImageSource(s.clone()),
                ));
            }
            ImageSource::None => {}
        }
        out
    }
}
