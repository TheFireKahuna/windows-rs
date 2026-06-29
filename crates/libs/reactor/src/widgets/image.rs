use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub key: Option<String>,
    pub modifiers: Modifiers,
    pub source: ImageSource,
    pub stretch: Stretch,
    pub mounted: Option<Callback<Option<windows_core::IInspectable>>>,
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
    pub fn on_mounted(mut self, f: impl Fn(ElementHandle) + 'static) -> Self {
        self.mounted = Some(Callback::new(move |native: Option<_>| {
            if let Some(native) = native {
                f(ElementHandle(native));
            }
        }));
        self
    }
}

impl Widget for Image {
    widget_header!(ControlKind::Image);
    fn on_mounted_callback(&self) -> Option<&Callback<Option<windows_core::IInspectable>>> {
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
