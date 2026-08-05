//! Brushes and stroke styles.
//!
//! Every brush and every gradient stop takes a [`Scrgb`], and this crate has no colour type
//! of its own. The output transform is the only source of an `Scrgb` and nothing converts
//! one back, so a scene-referred value reaches Direct2D having passed that transform
//! exactly once.
//!
//! Brushes belong to the device rather than to a [`Draw`]: Direct2D shares any resource
//! created from a device context with every other context on the same device, so one cached
//! brush serves every pass and both drawing paths.

use super::*;

/// Reinterprets `c` as Direct2D's colour struct.
///
/// Both types are four `f32` in RGBA order with C layout, so this is a reinterpretation
/// rather than a conversion; the `const` assertions below fail the build if either layout
/// moves. The returned pointer borrows `c` and must not outlive it.
///
/// Alpha needs no handling: Direct2D reads a colour as **straight** alpha whatever the
/// target's alpha mode is and premultiplies on the way in, and `Scrgb`'s alpha is straight.
/// Nothing here multiplies a colour by its own alpha.
pub(crate) fn d2d_color(c: &Scrgb) -> *const D2D_COLOR_F {
    const {
        assert!(size_of::<Scrgb>() == size_of::<D2D_COLOR_F>());
        assert!(align_of::<Scrgb>() == align_of::<D2D_COLOR_F>());
        assert!(core::mem::offset_of!(Scrgb, r) == core::mem::offset_of!(D2D_COLOR_F, r));
        assert!(core::mem::offset_of!(Scrgb, g) == core::mem::offset_of!(D2D_COLOR_F, g));
        assert!(core::mem::offset_of!(Scrgb, b) == core::mem::offset_of!(D2D_COLOR_F, b));
        assert!(core::mem::offset_of!(Scrgb, a) == core::mem::offset_of!(D2D_COLOR_F, a));
    }
    (&raw const *c).cast()
}

/// A gradient stop: a position along the ramp and the colour at it.
///
/// `#[repr(C)]` and laid out identically to Direct2D's stop, so a slice of these *is* the
/// array the API wants and a ramp costs no scratch buffer.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Stop {
    /// Position along the ramp.
    pub at: f32,
    /// Colour at that position.
    pub color: Scrgb,
}

/// What a brush paints outside its own extent.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Extend {
    /// Holds the edge value.
    #[default]
    Clamp,
    /// Repeats.
    Wrap,
}

impl Extend {
    fn d2d(self) -> D2D1_EXTEND_MODE {
        match self {
            Self::Clamp => D2D1_EXTEND_MODE_CLAMP,
            Self::Wrap => D2D1_EXTEND_MODE_WRAP,
        }
    }
}

/// A brush this crate can paint with. Sealed to the four kinds in this module.
pub trait Brush: Sealed {
    #[doc(hidden)]
    fn brush(&self) -> &BrushRef;
}

/// Any brush, opaquely. [`Brush`] hands one to a draw call so that no generated type leaves
/// the crate, and `#[repr(transparent)]` makes that a pointer cast rather than a
/// reference-count change on a path drawing hundreds of primitives a frame.
#[repr(transparent)]
pub struct BrushRef(ID2D1Brush);

impl BrushRef {
    pub(crate) fn raw(&self) -> &ID2D1Brush {
        &self.0
    }

    pub(crate) fn owned(&self) -> ID2D1Brush {
        self.0.clone()
    }

    fn of<T: Interface>(brush: &T) -> &Self {
        // SAFETY: `BrushRef` is a transparent newtype over `ID2D1Brush`, which is itself a
        // transparent newtype over a pointer, and every `T` here is a Direct2D brush
        // interface — so the pointee is the same object viewed through a base interface
        // whose vtable prefix it shares.
        unsafe { core::mem::transmute(brush) }
    }

    /// Scales the alpha of everything this brush paints, multiplying the alpha it already
    /// carries.
    pub fn set_alpha(&self, a: f32) {
        unsafe { self.0.SetOpacity(a) };
    }
}

/// A flat colour.
pub struct Solid(ID2D1SolidColorBrush);

impl Solid {
    /// Sets the colour in place, without allocating.
    pub fn set(&self, c: Scrgb) {
        unsafe { self.0.SetColor(d2d_color(&c)) };
    }
}

impl Sealed for Solid {}
impl Brush for Solid {
    fn brush(&self) -> &BrushRef {
        BrushRef::of(&self.0)
    }
}

/// A linear ramp between stops.
pub struct Ramp(ID2D1LinearGradientBrush);

impl Ramp {
    /// Re-aims the ramp between two points, in the target's coordinate space.
    ///
    /// One brush paints N spans by re-aiming between draws, so a fade anchored to each
    /// span's own edge costs this call per span rather than a brush per span.
    pub fn aim(&self, from: Vector2, to: Vector2) {
        unsafe {
            self.0.SetStartPoint(from);
            self.0.SetEndPoint(to);
        }
    }
}

impl Sealed for Ramp {}
impl Brush for Ramp {
    fn brush(&self) -> &BrushRef {
        BrushRef::of(&self.0)
    }
}

/// A radial ramp: the same stops, run outward from a centre instead of along an axis.
///
/// The centre and radii are fixed at construction, and there is no counterpart to
/// [`Ramp::aim`].
pub struct Radial(ID2D1RadialGradientBrush);

impl Sealed for Radial {}
impl Brush for Radial {
    fn brush(&self) -> &BrushRef {
        BrushRef::of(&self.0)
    }
}

/// A target sampled as a repeating or clamped fill.
pub struct Tile(ID2D1BitmapBrush1);

impl Sealed for Tile {}
impl Brush for Tile {
    fn brush(&self) -> &BrushRef {
        BrushRef::of(&self.0)
    }
}

/// How a stroke terminates.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Cap {
    #[default]
    Flat,
    Square,
    Round,
    Triangle,
}

impl Cap {
    fn d2d(self) -> D2D1_CAP_STYLE {
        match self {
            Self::Flat => D2D1_CAP_STYLE_FLAT,
            Self::Square => D2D1_CAP_STYLE_SQUARE,
            Self::Round => D2D1_CAP_STYLE_ROUND,
            Self::Triangle => D2D1_CAP_STYLE_TRIANGLE,
        }
    }
}

/// How a stroke turns a corner.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Join {
    #[default]
    Miter,
    Bevel,
    Round,
}

impl Join {
    fn d2d(self) -> D2D1_LINE_JOIN {
        match self {
            Self::Miter => D2D1_LINE_JOIN_MITER,
            Self::Bevel => D2D1_LINE_JOIN_BEVEL,
            Self::Round => D2D1_LINE_JOIN_ROUND,
        }
    }
}

/// The parameters a [`StrokeStyle`] is built from. Every field defaults, so a dashed
/// hairline states only its dashes.
#[derive(Clone, Debug, Default)]
pub struct StrokeSpec<'a> {
    /// Both ends of the stroke.
    pub cap: Cap,
    /// Every corner of the stroke.
    pub join: Join,
    /// Beyond this multiple of the stroke width a mitre is bevelled instead.
    pub miter_limit: Option<f32>,
    /// Alternating dash and gap lengths, as **multiples of the stroke width** — so the
    /// same array on a 1-DIP rule and a 4-DIP rule draws dashes four times as long on the
    /// second.
    pub dashes: &'a [f32],
    /// Both ends of each dash.
    pub dash_cap: Cap,
    /// Where in the dash pattern the stroke starts, as a multiple of the stroke width.
    pub dash_offset: f32,
}

/// A reusable stroke style: dashes, caps, joins. A device resource, so build it once.
pub struct StrokeStyle(ID2D1StrokeStyle1);

/// A stroke: a width in DIPs, and optionally a style.
#[derive(Copy, Clone)]
pub struct Stroke<'a> {
    /// Stroke width in DIPs.
    pub width: f32,
    /// Dashes, caps and joins, or the Direct2D defaults when `None`.
    pub style: Option<&'a StrokeStyle>,
}

impl Stroke<'_> {
    /// A one-DIP stroke with no style. Direct2D rasterizes it from a coverage function over
    /// one quad, so it costs by the pixels it touches rather than by tessellation.
    pub const HAIRLINE: Self = Self {
        width: 1.0,
        style: None,
    };

    /// Returns a stroke `width` DIPs wide, with no style.
    #[must_use]
    pub const fn width(width: f32) -> Self {
        Self { width, style: None }
    }
}

impl<'a> Stroke<'a> {
    /// Attaches `style` to the stroke.
    #[must_use]
    pub const fn styled(self, style: &'a StrokeStyle) -> Self {
        Self {
            style: Some(style),
            ..self
        }
    }

    pub(crate) fn parts(&self) -> (f32, Option<&ID2D1StrokeStyle>) {
        (self.width, self.style.map(|s| (&s.0).into()))
    }
}

impl Gpu {
    /// Creates a flat-colour brush. [`Solid::set`] retints it without building another.
    pub fn solid(&self, c: Scrgb) -> Result<Solid> {
        Ok(Solid(unsafe {
            self.ctx().CreateSolidColorBrush(d2d_color(&c), None)?
        }))
    }

    /// Creates a linear ramp running from `from` to `to`, with `extend` deciding what it
    /// paints beyond them. [`Ramp::aim`] re-aims it per span.
    ///
    /// The stops are interpolated in scRGB at 16 bits of float per channel, which holds the
    /// values above white that an sRGB-gamma interpolation stage would quantize away.
    pub fn ramp(&self, stops: &[Stop], from: Vector2, to: Vector2, extend: Extend) -> Result<Ramp> {
        let collection = self.stops(stops, extend)?;
        let properties = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
            startPoint: from,
            endPoint: to,
        };
        Ok(Ramp(unsafe {
            self.ctx()
                .CreateLinearGradientBrush(&properties, None, &collection)?
        }))
    }

    /// Creates a radial ramp centred at `center` with radii `radius`, with `extend`
    /// deciding what it paints beyond them.
    ///
    /// The same stop collection as [`ramp`](Self::ramp), interpolated in scRGB at 16 bits
    /// of float per channel, which holds a narrow alpha falloff that eight bits would
    /// quantize to almost nothing.
    pub fn radial(&self, stops: &[Stop], center: Vector2, radius: Vector2, extend: Extend) -> Result<Radial> {
        let collection = self.stops(stops, extend)?;
        let properties = D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
            center,
            gradientOriginOffset: Vector2 { x: 0.0, y: 0.0 },
            radiusX: radius.x,
            radiusY: radius.y,
        };
        Ok(Radial(unsafe {
            self.ctx()
                .CreateRadialGradientBrush(&properties, None, &collection)?
        }))
    }

    /// Builds the stop collection both ramp forms take.
    fn stops(&self, stops: &[Stop], extend: Extend) -> Result<ID2D1GradientStopCollection1> {
        const {
            assert!(size_of::<Stop>() == size_of::<D2D1_GRADIENT_STOP>());
            assert!(
                core::mem::offset_of!(Stop, at)
                    == core::mem::offset_of!(D2D1_GRADIENT_STOP, position)
            );
            assert!(
                core::mem::offset_of!(Stop, color)
                    == core::mem::offset_of!(D2D1_GRADIENT_STOP, color)
            );
        }
        // SAFETY: the `const` assertions above prove `Stop` and `D2D1_GRADIENT_STOP` have
        // the same size and the same field offsets, so the slice is a valid array of the
        // Direct2D type at its natural stride.
        let stops: &[D2D1_GRADIENT_STOP] =
            unsafe { core::slice::from_raw_parts(stops.as_ptr().cast(), stops.len()) };
        unsafe {
            self.ctx().CreateGradientStopCollection(
                stops,
                D2D1_COLOR_SPACE_SCRGB,
                D2D1_COLOR_SPACE_SCRGB,
                D2D1_BUFFER_PRECISION_16BPC_FLOAT,
                extend.d2d(),
                D2D1_COLOR_INTERPOLATION_MODE_STRAIGHT,
            )
        }
    }

    /// Creates a brush that samples `src`, with `extend` deciding what it paints outside
    /// the source extent and `interp` how it samples when stretched.
    ///
    /// A sprite batch covers the rectangular case for less; this fills shapes.
    pub fn tile(&self, src: &Target, extend: Extend, interp: Interp) -> Result<Tile> {
        let properties = D2D1_BITMAP_BRUSH_PROPERTIES1 {
            extendModeX: extend.d2d(),
            extendModeY: extend.d2d(),
            interpolationMode: interp.image(),
        };
        let brush = unsafe {
            self.ctx()
                .CreateBitmapBrush(&src.bitmap, Some(&properties), None)?
        };
        Ok(Tile(brush))
    }

    /// Creates a reusable stroke style from `spec`.
    pub fn stroke_style(&self, spec: &StrokeSpec<'_>) -> Result<StrokeStyle> {
        let properties = D2D1_STROKE_STYLE_PROPERTIES1 {
            startCap: spec.cap.d2d(),
            endCap: spec.cap.d2d(),
            dashCap: spec.dash_cap.d2d(),
            lineJoin: spec.join.d2d(),
            miterLimit: spec.miter_limit.unwrap_or(10.0),
            dashStyle: if spec.dashes.is_empty() {
                D2D1_DASH_STYLE_SOLID
            } else {
                D2D1_DASH_STYLE_CUSTOM
            },
            dashOffset: spec.dash_offset,
            transformType: D2D1_STROKE_TRANSFORM_TYPE::default(),
        };
        let dashes = (!spec.dashes.is_empty()).then_some(spec.dashes);
        let style = unsafe { self.factory().CreateStrokeStyle(&properties, dashes)? };
        Ok(StrokeStyle(style))
    }
}
