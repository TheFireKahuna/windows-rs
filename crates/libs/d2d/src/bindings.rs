windows_core::link!("d2d1.dll" "system" fn D2D1CreateFactory(factorytype : D2D1_FACTORY_TYPE, riid : *const windows_core::GUID, pfactoryoptions : *const D2D1_FACTORY_OPTIONS, ppifactory : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("d3d11.dll" "system" fn D3D11CreateDevice(padapter : *mut core::ffi::c_void, drivertype : D3D_DRIVER_TYPE, software : HMODULE, flags : u32, pfeaturelevels : *const D3D_FEATURE_LEVEL, featurelevels : u32, sdkversion : u32, ppdevice : *mut *mut core::ffi::c_void, pfeaturelevel : *mut D3D_FEATURE_LEVEL, ppimmediatecontext : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
pub type D2D1_ALPHA_MODE = i32;
pub const D2D1_ALPHA_MODE_IGNORE: D2D1_ALPHA_MODE = 3;
pub const D2D1_ALPHA_MODE_PREMULTIPLIED: D2D1_ALPHA_MODE = 1;
pub type D2D1_ANTIALIAS_MODE = i32;
pub const D2D1_ANTIALIAS_MODE_ALIASED: D2D1_ANTIALIAS_MODE = 1;
pub const D2D1_ANTIALIAS_MODE_PER_PRIMITIVE: D2D1_ANTIALIAS_MODE = 0;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_BEZIER_SEGMENT {
    pub point1: windows_numerics::Vector2,
    pub point2: windows_numerics::Vector2,
    pub point3: windows_numerics::Vector2,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct D2D1_BITMAP_BRUSH_PROPERTIES1 {
    pub extendModeX: D2D1_EXTEND_MODE,
    pub extendModeY: D2D1_EXTEND_MODE,
    pub interpolationMode: D2D1_INTERPOLATION_MODE,
}
pub type D2D1_BITMAP_INTERPOLATION_MODE = i32;
pub const D2D1_BITMAP_INTERPOLATION_MODE_LINEAR: D2D1_BITMAP_INTERPOLATION_MODE = 1;
pub const D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR: D2D1_BITMAP_INTERPOLATION_MODE = 0;
pub type D2D1_BITMAP_OPTIONS = u32;
pub const D2D1_BITMAP_OPTIONS_CANNOT_DRAW: D2D1_BITMAP_OPTIONS = 2;
pub const D2D1_BITMAP_OPTIONS_CPU_READ: D2D1_BITMAP_OPTIONS = 4;
pub const D2D1_BITMAP_OPTIONS_TARGET: D2D1_BITMAP_OPTIONS = 1;
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct D2D1_BITMAP_PROPERTIES1 {
    pub pixelFormat: D2D1_PIXEL_FORMAT,
    pub dpiX: f32,
    pub dpiY: f32,
    pub bitmapOptions: D2D1_BITMAP_OPTIONS,
    pub colorContext: core::mem::ManuallyDrop<Option<ID2D1ColorContext>>,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_BRUSH_PROPERTIES {
    pub opacity: f32,
    pub transform: windows_numerics::Matrix3x2,
}
pub type D2D1_BUFFER_PRECISION = i32;
pub const D2D1_BUFFER_PRECISION_16BPC_FLOAT: D2D1_BUFFER_PRECISION = 4;
pub const D2D1_BUFFER_PRECISION_16BPC_UNORM: D2D1_BUFFER_PRECISION = 3;
pub const D2D1_BUFFER_PRECISION_32BPC_FLOAT: D2D1_BUFFER_PRECISION = 5;
pub const D2D1_BUFFER_PRECISION_8BPC_UNORM: D2D1_BUFFER_PRECISION = 1;
pub const D2D1_BUFFER_PRECISION_8BPC_UNORM_SRGB: D2D1_BUFFER_PRECISION = 2;
pub const D2D1_BUFFER_PRECISION_FORCE_DWORD: D2D1_BUFFER_PRECISION = -1;
pub const D2D1_BUFFER_PRECISION_UNKNOWN: D2D1_BUFFER_PRECISION = 0;
pub type D2D1_CAP_STYLE = i32;
pub const D2D1_CAP_STYLE_FLAT: D2D1_CAP_STYLE = 0;
pub const D2D1_CAP_STYLE_ROUND: D2D1_CAP_STYLE = 2;
pub const D2D1_CAP_STYLE_SQUARE: D2D1_CAP_STYLE = 1;
pub const D2D1_CAP_STYLE_TRIANGLE: D2D1_CAP_STYLE = 3;
pub type D2D1_COLOR_INTERPOLATION_MODE = i32;
pub const D2D1_COLOR_INTERPOLATION_MODE_STRAIGHT: D2D1_COLOR_INTERPOLATION_MODE = 0;
pub type D2D1_COLOR_SPACE = i32;
pub const D2D1_COLOR_SPACE_SCRGB: D2D1_COLOR_SPACE = 2;
pub type D2D1_COMBINE_MODE = i32;
pub const D2D1_COMBINE_MODE_EXCLUDE: D2D1_COMBINE_MODE = 3;
pub const D2D1_COMBINE_MODE_INTERSECT: D2D1_COMBINE_MODE = 1;
pub const D2D1_COMBINE_MODE_UNION: D2D1_COMBINE_MODE = 0;
pub const D2D1_COMBINE_MODE_XOR: D2D1_COMBINE_MODE = 2;
pub type D2D1_DASH_STYLE = i32;
pub const D2D1_DASH_STYLE_CUSTOM: D2D1_DASH_STYLE = 5;
pub const D2D1_DASH_STYLE_SOLID: D2D1_DASH_STYLE = 0;
pub type D2D1_DEBUG_LEVEL = i32;
pub type D2D1_DEVICE_CONTEXT_OPTIONS = u32;
pub const D2D1_DEVICE_CONTEXT_OPTIONS_NONE: D2D1_DEVICE_CONTEXT_OPTIONS = 0;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_ELLIPSE {
    pub point: windows_numerics::Vector2,
    pub radiusX: f32,
    pub radiusY: f32,
}
pub type D2D1_EXTEND_MODE = i32;
pub const D2D1_EXTEND_MODE_CLAMP: D2D1_EXTEND_MODE = 0;
pub const D2D1_EXTEND_MODE_WRAP: D2D1_EXTEND_MODE = 1;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct D2D1_FACTORY_OPTIONS {
    pub debugLevel: D2D1_DEBUG_LEVEL,
}
pub type D2D1_FACTORY_TYPE = i32;
pub const D2D1_FACTORY_TYPE_SINGLE_THREADED: D2D1_FACTORY_TYPE = 0;
pub type D2D1_FIGURE_BEGIN = i32;
pub const D2D1_FIGURE_BEGIN_FILLED: D2D1_FIGURE_BEGIN = 0;
pub const D2D1_FIGURE_BEGIN_HOLLOW: D2D1_FIGURE_BEGIN = 1;
pub type D2D1_FIGURE_END = i32;
pub const D2D1_FIGURE_END_CLOSED: D2D1_FIGURE_END = 1;
pub const D2D1_FIGURE_END_OPEN: D2D1_FIGURE_END = 0;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_GRADIENT_STOP {
    pub position: f32,
    pub color: D2D_COLOR_F,
}
pub type D2D1_INTERPOLATION_MODE = i32;
pub const D2D1_INTERPOLATION_MODE_LINEAR: D2D1_INTERPOLATION_MODE = 1;
pub const D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR: D2D1_INTERPOLATION_MODE = 0;
pub type D2D1_LAYER_OPTIONS1 = u32;
pub const D2D1_LAYER_OPTIONS1_IGNORE_ALPHA: D2D1_LAYER_OPTIONS1 = 2;
pub const D2D1_LAYER_OPTIONS1_INITIALIZE_FROM_BACKGROUND: D2D1_LAYER_OPTIONS1 = 1;
#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct D2D1_LAYER_PARAMETERS1 {
    pub contentBounds: D2D_RECT_F,
    pub geometricMask: core::mem::ManuallyDrop<Option<ID2D1Geometry>>,
    pub maskAntialiasMode: D2D1_ANTIALIAS_MODE,
    pub maskTransform: windows_numerics::Matrix3x2,
    pub opacity: f32,
    pub opacityBrush: core::mem::ManuallyDrop<Option<ID2D1Brush>>,
    pub layerOptions: D2D1_LAYER_OPTIONS1,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
    pub startPoint: windows_numerics::Vector2,
    pub endPoint: windows_numerics::Vector2,
}
pub type D2D1_LINE_JOIN = i32;
pub const D2D1_LINE_JOIN_BEVEL: D2D1_LINE_JOIN = 1;
pub const D2D1_LINE_JOIN_MITER: D2D1_LINE_JOIN = 0;
pub const D2D1_LINE_JOIN_ROUND: D2D1_LINE_JOIN = 2;
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct D2D1_MAPPED_RECT {
    pub pitch: u32,
    pub bits: *mut u8,
}
impl Default for D2D1_MAPPED_RECT {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub type D2D1_MAP_OPTIONS = u32;
pub const D2D1_MAP_OPTIONS_READ: D2D1_MAP_OPTIONS = 1;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct D2D1_PIXEL_FORMAT {
    pub format: DXGI_FORMAT,
    pub alphaMode: D2D1_ALPHA_MODE,
}
pub type D2D1_PRIMITIVE_BLEND = i32;
pub const D2D1_PRIMITIVE_BLEND_ADD: D2D1_PRIMITIVE_BLEND = 3;
pub const D2D1_PRIMITIVE_BLEND_COPY: D2D1_PRIMITIVE_BLEND = 1;
pub const D2D1_PRIMITIVE_BLEND_SOURCE_OVER: D2D1_PRIMITIVE_BLEND = 0;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
    pub center: windows_numerics::Vector2,
    pub gradientOriginOffset: windows_numerics::Vector2,
    pub radiusX: f32,
    pub radiusY: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct D2D1_RENDERING_CONTROLS {
    pub bufferPrecision: D2D1_BUFFER_PRECISION,
    pub tileSize: D2D_SIZE_U,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_ROUNDED_RECT {
    pub rect: D2D_RECT_F,
    pub radiusX: f32,
    pub radiusY: f32,
}
pub type D2D1_SPRITE_OPTIONS = u32;
pub const D2D1_SPRITE_OPTIONS_NONE: D2D1_SPRITE_OPTIONS = 0;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_STROKE_STYLE_PROPERTIES1 {
    pub startCap: D2D1_CAP_STYLE,
    pub endCap: D2D1_CAP_STYLE,
    pub dashCap: D2D1_CAP_STYLE,
    pub lineJoin: D2D1_LINE_JOIN,
    pub miterLimit: f32,
    pub dashStyle: D2D1_DASH_STYLE,
    pub dashOffset: f32,
    pub transformType: D2D1_STROKE_TRANSFORM_TYPE,
}
pub type D2D1_STROKE_TRANSFORM_TYPE = i32;
pub type D2D1_TAG = u64;
pub type D2D1_TEXT_ANTIALIAS_MODE = i32;
pub const D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE: D2D1_TEXT_ANTIALIAS_MODE = 2;
pub type D2D1_UNIT_MODE = i32;
pub const D2D1_UNIT_MODE_DIPS: D2D1_UNIT_MODE = 0;
pub const D2DERR_RECREATE_TARGET: windows_core::HRESULT =
    windows_core::HRESULT(0x8899000C_u32 as _);
pub type D2D_COLOR_F = D3DCOLORVALUE;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct D2D_POINT_2U {
    pub x: u32,
    pub y: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D_RECT_F {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct D2D_RECT_U {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D_SIZE_F {
    pub width: f32,
    pub height: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct D2D_SIZE_U {
    pub width: u32,
    pub height: u32,
}
pub const D3D11_CREATE_DEVICE_BGRA_SUPPORT: D3D11_CREATE_DEVICE_FLAG = 32;
pub type D3D11_CREATE_DEVICE_FLAG = i32;
pub const D3D11_CREATE_DEVICE_PREVENT_INTERNAL_THREADING_OPTIMIZATIONS: D3D11_CREATE_DEVICE_FLAG =
    8;
pub const D3D11_CREATE_DEVICE_SINGLETHREADED: D3D11_CREATE_DEVICE_FLAG = 1;
pub const D3D11_SDK_VERSION: i32 = 7;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D3DCOLORVALUE {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
pub type D3D_DRIVER_TYPE = i32;
pub const D3D_DRIVER_TYPE_HARDWARE: D3D_DRIVER_TYPE = 1;
pub const D3D_DRIVER_TYPE_WARP: D3D_DRIVER_TYPE = 5;
pub type D3D_FEATURE_LEVEL = i32;
pub const D3D_FEATURE_LEVEL_10_0: D3D_FEATURE_LEVEL = 40960;
pub const D3D_FEATURE_LEVEL_10_1: D3D_FEATURE_LEVEL = 41216;
pub const D3D_FEATURE_LEVEL_11_0: D3D_FEATURE_LEVEL = 45056;
pub const D3D_FEATURE_LEVEL_11_1: D3D_FEATURE_LEVEL = 45312;
pub const D3D_FEATURE_LEVEL_9_1: D3D_FEATURE_LEVEL = 37120;
pub const D3D_FEATURE_LEVEL_9_2: D3D_FEATURE_LEVEL = 37376;
pub const D3D_FEATURE_LEVEL_9_3: D3D_FEATURE_LEVEL = 37632;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DWRITE_GLYPH_OFFSET {
    pub advanceOffset: f32,
    pub ascenderOffset: f32,
}
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct DWRITE_GLYPH_RUN {
    pub fontFace: core::mem::ManuallyDrop<Option<IDWriteFontFace>>,
    pub fontEmSize: f32,
    pub glyphCount: u32,
    pub glyphIndices: *const u16,
    pub glyphAdvances: *const f32,
    pub glyphOffsets: *const DWRITE_GLYPH_OFFSET,
    pub isSideways: windows_core::BOOL,
    pub bidiLevel: u32,
}
impl Default for DWRITE_GLYPH_RUN {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DWRITE_GLYPH_RUN_DESCRIPTION {
    pub localeName: *const u16,
    pub string: *const u16,
    pub stringLength: u32,
    pub clusterMap: *const u16,
    pub textPosition: u32,
}
impl Default for DWRITE_GLYPH_RUN_DESCRIPTION {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub type DWRITE_MEASURING_MODE = i32;
pub const DWRITE_MEASURING_MODE_NATURAL: DWRITE_MEASURING_MODE = 0;
pub const DXGI_ERROR_DEVICE_HUNG: windows_core::HRESULT =
    windows_core::HRESULT(0x887A0006_u32 as _);
pub const DXGI_ERROR_DEVICE_REMOVED: windows_core::HRESULT =
    windows_core::HRESULT(0x887A0005_u32 as _);
pub const DXGI_ERROR_DEVICE_RESET: windows_core::HRESULT =
    windows_core::HRESULT(0x887A0007_u32 as _);
pub const DXGI_ERROR_DRIVER_INTERNAL_ERROR: windows_core::HRESULT =
    windows_core::HRESULT(0x887A0020_u32 as _);
pub type DXGI_FORMAT = i32;
pub const DXGI_FORMAT_R16G16B16A16_FLOAT: DXGI_FORMAT = 10;
pub type DXGI_RESIDENCY = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DXGI_SAMPLE_DESC {
    pub Count: u32,
    pub Quality: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DXGI_SHARED_RESOURCE {
    pub Handle: HANDLE,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DXGI_SURFACE_DESC {
    pub Width: u32,
    pub Height: u32,
    pub Format: DXGI_FORMAT,
    pub SampleDesc: DXGI_SAMPLE_DESC,
}
pub type DXGI_USAGE = u32;
pub const E_FAIL: windows_core::HRESULT = windows_core::HRESULT(0x80004005_u32 as _);
pub const E_INVALIDARG: windows_core::HRESULT = windows_core::HRESULT(0x80070057_u32 as _);
pub type HANDLE = *mut core::ffi::c_void;
pub type HINSTANCE = *mut core::ffi::c_void;
pub type HMODULE = HINSTANCE;
windows_core::imp::define_interface!(
    ID2D1Bitmap,
    ID2D1Bitmap_Vtbl,
    0xa2296057_ea42_4099_983b_539fb6505426
);
impl core::ops::Deref for ID2D1Bitmap {
    type Target = ID2D1Image;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1Bitmap,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Image
);
impl ID2D1Bitmap {
    pub unsafe fn GetSize(&self) -> D2D_SIZE_F {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetSize)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            );
            result__
        }
    }
    pub unsafe fn GetPixelSize(&self) -> D2D_SIZE_U {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetPixelSize)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            );
            result__
        }
    }
    pub unsafe fn CopyFromBitmap<P1>(
        &self,
        destpoint: Option<*const D2D_POINT_2U>,
        bitmap: P1,
        srcrect: Option<*const D2D_RECT_U>,
    ) -> windows_core::HRESULT
    where
        P1: windows_core::Param<Self>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).CopyFromBitmap)(
                windows_core::Interface::as_raw(self),
                destpoint.unwrap_or(core::mem::zeroed()) as _,
                bitmap.param().abi(),
                srcrect.unwrap_or(core::mem::zeroed()) as _,
            )
        }
    }
}
#[repr(C)]
pub struct ID2D1Bitmap_Vtbl {
    pub base__: ID2D1Image_Vtbl,
    pub GetSize: unsafe extern "system" fn(*mut core::ffi::c_void, *mut D2D_SIZE_F),
    pub GetPixelSize: unsafe extern "system" fn(*mut core::ffi::c_void, *mut D2D_SIZE_U),
    GetPixelFormat: usize,
    GetDpi: usize,
    pub CopyFromBitmap: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D_POINT_2U,
        *mut core::ffi::c_void,
        *const D2D_RECT_U,
    ) -> windows_core::HRESULT,
    CopyFromRenderTarget: usize,
    CopyFromMemory: usize,
}
impl windows_core::RuntimeName for ID2D1Bitmap {}
windows_core::imp::define_interface!(
    ID2D1Bitmap1,
    ID2D1Bitmap1_Vtbl,
    0xa898a84c_3873_4588_b08b_ebbf978df041
);
impl core::ops::Deref for ID2D1Bitmap1 {
    type Target = ID2D1Bitmap;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1Bitmap1,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Image,
    ID2D1Bitmap
);
impl ID2D1Bitmap1 {
    pub unsafe fn Map(&self, options: D2D1_MAP_OPTIONS) -> windows_core::Result<D2D1_MAPPED_RECT> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Map)(
                windows_core::Interface::as_raw(self),
                options,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn Unmap(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Unmap)(windows_core::Interface::as_raw(self))
        }
    }
}
#[repr(C)]
pub struct ID2D1Bitmap1_Vtbl {
    pub base__: ID2D1Bitmap_Vtbl,
    GetColorContext: usize,
    GetOptions: usize,
    GetSurface: usize,
    pub Map: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        D2D1_MAP_OPTIONS,
        *mut D2D1_MAPPED_RECT,
    ) -> windows_core::HRESULT,
    pub Unmap: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for ID2D1Bitmap1 {}
windows_core::imp::define_interface!(
    ID2D1BitmapBrush,
    ID2D1BitmapBrush_Vtbl,
    0x2cd906aa_12e2_11dc_9fed_001143a055f9
);
impl core::ops::Deref for ID2D1BitmapBrush {
    type Target = ID2D1Brush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1BitmapBrush,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Brush
);
#[repr(C)]
pub struct ID2D1BitmapBrush_Vtbl {
    pub base__: ID2D1Brush_Vtbl,
    SetExtendModeX: usize,
    SetExtendModeY: usize,
    SetInterpolationMode: usize,
    SetBitmap: usize,
    GetExtendModeX: usize,
    GetExtendModeY: usize,
    GetInterpolationMode: usize,
    GetBitmap: usize,
}
impl windows_core::RuntimeName for ID2D1BitmapBrush {}
windows_core::imp::define_interface!(
    ID2D1BitmapBrush1,
    ID2D1BitmapBrush1_Vtbl,
    0x41343a53_e41a_49a2_91cd_21793bbb62e5
);
impl core::ops::Deref for ID2D1BitmapBrush1 {
    type Target = ID2D1BitmapBrush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1BitmapBrush1,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Brush,
    ID2D1BitmapBrush
);
impl ID2D1BitmapBrush1 {
    pub unsafe fn SetInterpolationMode1(&self, interpolationmode: D2D1_INTERPOLATION_MODE) {
        unsafe {
            (windows_core::Interface::vtable(self).SetInterpolationMode1)(
                windows_core::Interface::as_raw(self),
                interpolationmode,
            );
        }
    }
    pub unsafe fn GetInterpolationMode1(&self) -> D2D1_INTERPOLATION_MODE {
        unsafe {
            (windows_core::Interface::vtable(self).GetInterpolationMode1)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
}
#[repr(C)]
pub struct ID2D1BitmapBrush1_Vtbl {
    pub base__: ID2D1BitmapBrush_Vtbl,
    pub SetInterpolationMode1:
        unsafe extern "system" fn(*mut core::ffi::c_void, D2D1_INTERPOLATION_MODE),
    pub GetInterpolationMode1:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> D2D1_INTERPOLATION_MODE,
}
impl windows_core::RuntimeName for ID2D1BitmapBrush1 {}
windows_core::imp::define_interface!(
    ID2D1Brush,
    ID2D1Brush_Vtbl,
    0x2cd906a8_12e2_11dc_9fed_001143a055f9
);
impl core::ops::Deref for ID2D1Brush {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1Brush, windows_core::IUnknown, ID2D1Resource);
impl ID2D1Brush {
    pub unsafe fn SetOpacity(&self, opacity: f32) {
        unsafe {
            (windows_core::Interface::vtable(self).SetOpacity)(
                windows_core::Interface::as_raw(self),
                opacity,
            );
        }
    }
}
#[repr(C)]
pub struct ID2D1Brush_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    pub SetOpacity: unsafe extern "system" fn(*mut core::ffi::c_void, f32),
    SetTransform: usize,
    GetOpacity: usize,
    GetTransform: usize,
}
impl windows_core::RuntimeName for ID2D1Brush {}
windows_core::imp::define_interface!(
    ID2D1ColorContext,
    ID2D1ColorContext_Vtbl,
    0x1c4820bb_5771_4518_a581_2fe4dd0ec657
);
impl core::ops::Deref for ID2D1ColorContext {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1ColorContext, windows_core::IUnknown, ID2D1Resource);
#[repr(C)]
pub struct ID2D1ColorContext_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    GetColorSpace: usize,
    GetProfileSize: usize,
    GetProfile: usize,
}
impl windows_core::RuntimeName for ID2D1ColorContext {}
windows_core::imp::define_interface!(
    ID2D1Device,
    ID2D1Device_Vtbl,
    0x47dd575d_ac05_4cdd_8049_9b02cd16f44c
);
impl core::ops::Deref for ID2D1Device {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1Device, windows_core::IUnknown, ID2D1Resource);
#[repr(C)]
pub struct ID2D1Device_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    CreateDeviceContext: usize,
    CreatePrintControl: usize,
    SetMaximumTextureMemory: usize,
    GetMaximumTextureMemory: usize,
    ClearResources: usize,
}
impl windows_core::RuntimeName for ID2D1Device {}
windows_core::imp::define_interface!(
    ID2D1Device1,
    ID2D1Device1_Vtbl,
    0xd21768e1_23a4_4823_a14b_7c3eba85d658
);
impl core::ops::Deref for ID2D1Device1 {
    type Target = ID2D1Device;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1Device1,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Device
);
#[repr(C)]
pub struct ID2D1Device1_Vtbl {
    pub base__: ID2D1Device_Vtbl,
    GetRenderingPriority: usize,
    SetRenderingPriority: usize,
    CreateDeviceContext: usize,
}
impl windows_core::RuntimeName for ID2D1Device1 {}
windows_core::imp::define_interface!(
    ID2D1Device2,
    ID2D1Device2_Vtbl,
    0xa44472e1_8dfb_4e60_8492_6e2861c9ca8b
);
impl core::ops::Deref for ID2D1Device2 {
    type Target = ID2D1Device1;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1Device2,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Device,
    ID2D1Device1
);
#[repr(C)]
pub struct ID2D1Device2_Vtbl {
    pub base__: ID2D1Device1_Vtbl,
    CreateDeviceContext: usize,
    FlushDeviceContexts: usize,
    GetDxgiDevice: usize,
}
impl windows_core::RuntimeName for ID2D1Device2 {}
windows_core::imp::define_interface!(
    ID2D1Device3,
    ID2D1Device3_Vtbl,
    0x852f2087_802c_4037_ab60_ff2e7ee6fc01
);
impl core::ops::Deref for ID2D1Device3 {
    type Target = ID2D1Device2;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1Device3,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Device,
    ID2D1Device1,
    ID2D1Device2
);
#[repr(C)]
pub struct ID2D1Device3_Vtbl {
    pub base__: ID2D1Device2_Vtbl,
    CreateDeviceContext: usize,
}
impl windows_core::RuntimeName for ID2D1Device3 {}
windows_core::imp::define_interface!(
    ID2D1Device4,
    ID2D1Device4_Vtbl,
    0xd7bdb159_5683_4a46_bc9c_72dc720b858b
);
impl core::ops::Deref for ID2D1Device4 {
    type Target = ID2D1Device3;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1Device4,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Device,
    ID2D1Device1,
    ID2D1Device2,
    ID2D1Device3
);
#[repr(C)]
pub struct ID2D1Device4_Vtbl {
    pub base__: ID2D1Device3_Vtbl,
    CreateDeviceContext: usize,
    SetMaximumColorGlyphCacheMemory: usize,
    GetMaximumColorGlyphCacheMemory: usize,
}
impl windows_core::RuntimeName for ID2D1Device4 {}
windows_core::imp::define_interface!(
    ID2D1Device5,
    ID2D1Device5_Vtbl,
    0xd55ba0a4_6405_4694_aef5_08ee1a4358b4
);
impl core::ops::Deref for ID2D1Device5 {
    type Target = ID2D1Device4;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1Device5,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Device,
    ID2D1Device1,
    ID2D1Device2,
    ID2D1Device3,
    ID2D1Device4
);
#[repr(C)]
pub struct ID2D1Device5_Vtbl {
    pub base__: ID2D1Device4_Vtbl,
    CreateDeviceContext: usize,
}
impl windows_core::RuntimeName for ID2D1Device5 {}
windows_core::imp::define_interface!(
    ID2D1Device6,
    ID2D1Device6_Vtbl,
    0x7bfef914_2d75_4bad_be87_e18ddb077b6d
);
impl core::ops::Deref for ID2D1Device6 {
    type Target = ID2D1Device5;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1Device6,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Device,
    ID2D1Device1,
    ID2D1Device2,
    ID2D1Device3,
    ID2D1Device4,
    ID2D1Device5
);
impl ID2D1Device6 {
    pub unsafe fn CreateDeviceContext(
        &self,
        options: D2D1_DEVICE_CONTEXT_OPTIONS,
    ) -> windows_core::Result<ID2D1DeviceContext6> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateDeviceContext)(
                windows_core::Interface::as_raw(self),
                options,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ID2D1Device6_Vtbl {
    pub base__: ID2D1Device5_Vtbl,
    pub CreateDeviceContext: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        D2D1_DEVICE_CONTEXT_OPTIONS,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for ID2D1Device6 {}
windows_core::imp::define_interface!(
    ID2D1DeviceContext,
    ID2D1DeviceContext_Vtbl,
    0xe8f7fe7a_191c_466d_ad95_975678bda998
);
impl core::ops::Deref for ID2D1DeviceContext {
    type Target = ID2D1RenderTarget;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1DeviceContext,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1RenderTarget
);
impl ID2D1DeviceContext {
    pub unsafe fn CreateBitmap(
        &self,
        size: D2D_SIZE_U,
        sourcedata: Option<*const core::ffi::c_void>,
        pitch: u32,
        bitmapproperties: *const D2D1_BITMAP_PROPERTIES1,
    ) -> windows_core::Result<ID2D1Bitmap1> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateBitmap)(
                windows_core::Interface::as_raw(self),
                size,
                sourcedata.unwrap_or(core::mem::zeroed()) as _,
                pitch,
                bitmapproperties,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateBitmapFromDxgiSurface<P0>(
        &self,
        surface: P0,
        bitmapproperties: Option<*const D2D1_BITMAP_PROPERTIES1>,
    ) -> windows_core::Result<ID2D1Bitmap1>
    where
        P0: windows_core::Param<IDXGISurface>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateBitmapFromDxgiSurface)(
                windows_core::Interface::as_raw(self),
                surface.param().abi(),
                bitmapproperties.unwrap_or(core::mem::zeroed()) as _,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateGradientStopCollection(
        &self,
        straightalphagradientstops: &[D2D1_GRADIENT_STOP],
        preinterpolationspace: D2D1_COLOR_SPACE,
        postinterpolationspace: D2D1_COLOR_SPACE,
        bufferprecision: D2D1_BUFFER_PRECISION,
        extendmode: D2D1_EXTEND_MODE,
        colorinterpolationmode: D2D1_COLOR_INTERPOLATION_MODE,
    ) -> windows_core::Result<ID2D1GradientStopCollection1> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateGradientStopCollection)(
                windows_core::Interface::as_raw(self),
                straightalphagradientstops.as_ptr(),
                straightalphagradientstops.len().try_into().unwrap(),
                preinterpolationspace,
                postinterpolationspace,
                bufferprecision,
                extendmode,
                colorinterpolationmode,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateBitmapBrush<P0>(
        &self,
        bitmap: P0,
        bitmapbrushproperties: Option<*const D2D1_BITMAP_BRUSH_PROPERTIES1>,
        brushproperties: Option<*const D2D1_BRUSH_PROPERTIES>,
    ) -> windows_core::Result<ID2D1BitmapBrush1>
    where
        P0: windows_core::Param<ID2D1Bitmap>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateBitmapBrush)(
                windows_core::Interface::as_raw(self),
                bitmap.param().abi(),
                bitmapbrushproperties.unwrap_or(core::mem::zeroed()) as _,
                brushproperties.unwrap_or(core::mem::zeroed()) as _,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn IsDxgiFormatSupported(&self, format: DXGI_FORMAT) -> windows_core::BOOL {
        unsafe {
            (windows_core::Interface::vtable(self).IsDxgiFormatSupported)(
                windows_core::Interface::as_raw(self),
                format,
            )
        }
    }
    pub unsafe fn IsBufferPrecisionSupported(
        &self,
        bufferprecision: D2D1_BUFFER_PRECISION,
    ) -> windows_core::BOOL {
        unsafe {
            (windows_core::Interface::vtable(self).IsBufferPrecisionSupported)(
                windows_core::Interface::as_raw(self),
                bufferprecision,
            )
        }
    }
    pub unsafe fn SetTarget<P0>(&self, image: P0)
    where
        P0: windows_core::Param<ID2D1Image>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetTarget)(
                windows_core::Interface::as_raw(self),
                image.param().abi(),
            );
        }
    }
    pub unsafe fn SetRenderingControls(&self, renderingcontrols: *const D2D1_RENDERING_CONTROLS) {
        unsafe {
            (windows_core::Interface::vtable(self).SetRenderingControls)(
                windows_core::Interface::as_raw(self),
                renderingcontrols,
            );
        }
    }
    pub unsafe fn GetRenderingControls(&self) -> D2D1_RENDERING_CONTROLS {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetRenderingControls)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            );
            result__
        }
    }
    pub unsafe fn SetPrimitiveBlend(&self, primitiveblend: D2D1_PRIMITIVE_BLEND) {
        unsafe {
            (windows_core::Interface::vtable(self).SetPrimitiveBlend)(
                windows_core::Interface::as_raw(self),
                primitiveblend,
            );
        }
    }
    pub unsafe fn SetUnitMode(&self, unitmode: D2D1_UNIT_MODE) {
        unsafe {
            (windows_core::Interface::vtable(self).SetUnitMode)(
                windows_core::Interface::as_raw(self),
                unitmode,
            );
        }
    }
    pub unsafe fn DrawGlyphRun<P3>(
        &self,
        baselineorigin: windows_numerics::Vector2,
        glyphrun: *const DWRITE_GLYPH_RUN,
        glyphrundescription: Option<*const DWRITE_GLYPH_RUN_DESCRIPTION>,
        foregroundbrush: P3,
        measuringmode: DWRITE_MEASURING_MODE,
    ) where
        P3: windows_core::Param<ID2D1Brush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawGlyphRun)(
                windows_core::Interface::as_raw(self),
                baselineorigin,
                glyphrun,
                glyphrundescription.unwrap_or(core::mem::zeroed()) as _,
                foregroundbrush.param().abi(),
                measuringmode,
            );
        }
    }
    pub unsafe fn DrawBitmap<P0>(
        &self,
        bitmap: P0,
        destinationrectangle: Option<*const D2D_RECT_F>,
        opacity: f32,
        interpolationmode: D2D1_INTERPOLATION_MODE,
        sourcerectangle: Option<*const D2D_RECT_F>,
        perspectivetransform: Option<*const windows_numerics::Matrix4x4>,
    ) where
        P0: windows_core::Param<ID2D1Bitmap>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawBitmap)(
                windows_core::Interface::as_raw(self),
                bitmap.param().abi(),
                destinationrectangle.unwrap_or(core::mem::zeroed()) as _,
                opacity,
                interpolationmode,
                sourcerectangle.unwrap_or(core::mem::zeroed()) as _,
                perspectivetransform.unwrap_or(core::mem::zeroed()) as _,
            );
        }
    }
    pub unsafe fn PushLayer<P1>(&self, layerparameters: *const D2D1_LAYER_PARAMETERS1, layer: P1)
    where
        P1: windows_core::Param<ID2D1Layer>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).PushLayer)(
                windows_core::Interface::as_raw(self),
                layerparameters,
                layer.param().abi(),
            );
        }
    }
}
#[repr(C)]
pub struct ID2D1DeviceContext_Vtbl {
    pub base__: ID2D1RenderTarget_Vtbl,
    pub CreateBitmap: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        D2D_SIZE_U,
        *const core::ffi::c_void,
        u32,
        *const D2D1_BITMAP_PROPERTIES1,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateBitmapFromWicBitmap: usize,
    CreateColorContext: usize,
    CreateColorContextFromFilename: usize,
    CreateColorContextFromWicColorContext: usize,
    pub CreateBitmapFromDxgiSurface: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *const D2D1_BITMAP_PROPERTIES1,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateEffect: usize,
    pub CreateGradientStopCollection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D1_GRADIENT_STOP,
        u32,
        D2D1_COLOR_SPACE,
        D2D1_COLOR_SPACE,
        D2D1_BUFFER_PRECISION,
        D2D1_EXTEND_MODE,
        D2D1_COLOR_INTERPOLATION_MODE,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateImageBrush: usize,
    pub CreateBitmapBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *const D2D1_BITMAP_BRUSH_PROPERTIES1,
        *const D2D1_BRUSH_PROPERTIES,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateCommandList: usize,
    pub IsDxgiFormatSupported:
        unsafe extern "system" fn(*mut core::ffi::c_void, DXGI_FORMAT) -> windows_core::BOOL,
    pub IsBufferPrecisionSupported: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        D2D1_BUFFER_PRECISION,
    ) -> windows_core::BOOL,
    GetImageLocalBounds: usize,
    GetImageWorldBounds: usize,
    GetGlyphRunWorldBounds: usize,
    GetDevice: usize,
    pub SetTarget: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void),
    GetTarget: usize,
    pub SetRenderingControls:
        unsafe extern "system" fn(*mut core::ffi::c_void, *const D2D1_RENDERING_CONTROLS),
    pub GetRenderingControls:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut D2D1_RENDERING_CONTROLS),
    pub SetPrimitiveBlend: unsafe extern "system" fn(*mut core::ffi::c_void, D2D1_PRIMITIVE_BLEND),
    GetPrimitiveBlend: usize,
    pub SetUnitMode: unsafe extern "system" fn(*mut core::ffi::c_void, D2D1_UNIT_MODE),
    GetUnitMode: usize,
    pub DrawGlyphRun: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
        *const DWRITE_GLYPH_RUN,
        *const DWRITE_GLYPH_RUN_DESCRIPTION,
        *mut core::ffi::c_void,
        DWRITE_MEASURING_MODE,
    ),
    DrawImage: usize,
    DrawGdiMetafile: usize,
    pub DrawBitmap: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *const D2D_RECT_F,
        f32,
        D2D1_INTERPOLATION_MODE,
        *const D2D_RECT_F,
        *const windows_numerics::Matrix4x4,
    ),
    pub PushLayer: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D1_LAYER_PARAMETERS1,
        *mut core::ffi::c_void,
    ),
    InvalidateEffectInputRectangle: usize,
    GetEffectInvalidRectangleCount: usize,
    GetEffectInvalidRectangles: usize,
    GetEffectRequiredInputRectangles: usize,
    FillOpacityMask: usize,
}
impl windows_core::RuntimeName for ID2D1DeviceContext {}
windows_core::imp::define_interface!(
    ID2D1DeviceContext1,
    ID2D1DeviceContext1_Vtbl,
    0xd37f57e4_6908_459f_a199_e72f24f79987
);
impl core::ops::Deref for ID2D1DeviceContext1 {
    type Target = ID2D1DeviceContext;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1DeviceContext1,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1RenderTarget,
    ID2D1DeviceContext
);
impl ID2D1DeviceContext1 {
    pub unsafe fn CreateFilledGeometryRealization<P0>(
        &self,
        geometry: P0,
        flatteningtolerance: f32,
    ) -> windows_core::Result<ID2D1GeometryRealization>
    where
        P0: windows_core::Param<ID2D1Geometry>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateFilledGeometryRealization)(
                windows_core::Interface::as_raw(self),
                geometry.param().abi(),
                flatteningtolerance,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateStrokedGeometryRealization<P0, P3>(
        &self,
        geometry: P0,
        flatteningtolerance: f32,
        strokewidth: f32,
        strokestyle: P3,
    ) -> windows_core::Result<ID2D1GeometryRealization>
    where
        P0: windows_core::Param<ID2D1Geometry>,
        P3: windows_core::Param<ID2D1StrokeStyle>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateStrokedGeometryRealization)(
                windows_core::Interface::as_raw(self),
                geometry.param().abi(),
                flatteningtolerance,
                strokewidth,
                strokestyle.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn DrawGeometryRealization<P0, P1>(&self, geometryrealization: P0, brush: P1)
    where
        P0: windows_core::Param<ID2D1GeometryRealization>,
        P1: windows_core::Param<ID2D1Brush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawGeometryRealization)(
                windows_core::Interface::as_raw(self),
                geometryrealization.param().abi(),
                brush.param().abi(),
            );
        }
    }
}
#[repr(C)]
pub struct ID2D1DeviceContext1_Vtbl {
    pub base__: ID2D1DeviceContext_Vtbl,
    pub CreateFilledGeometryRealization: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        f32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateStrokedGeometryRealization: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        f32,
        f32,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub DrawGeometryRealization: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ),
}
impl windows_core::RuntimeName for ID2D1DeviceContext1 {}
windows_core::imp::define_interface!(
    ID2D1DeviceContext2,
    ID2D1DeviceContext2_Vtbl,
    0x394ea6a3_0c34_4321_950b_6ca20f0be6c7
);
impl core::ops::Deref for ID2D1DeviceContext2 {
    type Target = ID2D1DeviceContext1;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1DeviceContext2,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1RenderTarget,
    ID2D1DeviceContext,
    ID2D1DeviceContext1
);
#[repr(C)]
pub struct ID2D1DeviceContext2_Vtbl {
    pub base__: ID2D1DeviceContext1_Vtbl,
    CreateInk: usize,
    CreateInkStyle: usize,
    CreateGradientMesh: usize,
    CreateImageSourceFromWic: usize,
    CreateLookupTable3D: usize,
    CreateImageSourceFromDxgi: usize,
    GetGradientMeshWorldBounds: usize,
    DrawInk: usize,
    DrawGradientMesh: usize,
    DrawGdiMetafile: usize,
    CreateTransformedImageSource: usize,
}
impl windows_core::RuntimeName for ID2D1DeviceContext2 {}
windows_core::imp::define_interface!(
    ID2D1DeviceContext3,
    ID2D1DeviceContext3_Vtbl,
    0x235a7496_8351_414c_bcd4_6672ab2d8e00
);
impl core::ops::Deref for ID2D1DeviceContext3 {
    type Target = ID2D1DeviceContext2;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1DeviceContext3,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1RenderTarget,
    ID2D1DeviceContext,
    ID2D1DeviceContext1,
    ID2D1DeviceContext2
);
impl ID2D1DeviceContext3 {
    pub unsafe fn CreateSpriteBatch(&self) -> windows_core::Result<ID2D1SpriteBatch> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateSpriteBatch)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn DrawSpriteBatch<P0, P3>(
        &self,
        spritebatch: P0,
        startindex: u32,
        spritecount: u32,
        bitmap: P3,
        interpolationmode: D2D1_BITMAP_INTERPOLATION_MODE,
        spriteoptions: D2D1_SPRITE_OPTIONS,
    ) where
        P0: windows_core::Param<ID2D1SpriteBatch>,
        P3: windows_core::Param<ID2D1Bitmap>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawSpriteBatch)(
                windows_core::Interface::as_raw(self),
                spritebatch.param().abi(),
                startindex,
                spritecount,
                bitmap.param().abi(),
                interpolationmode,
                spriteoptions,
            );
        }
    }
}
#[repr(C)]
pub struct ID2D1DeviceContext3_Vtbl {
    pub base__: ID2D1DeviceContext2_Vtbl,
    pub CreateSpriteBatch: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub DrawSpriteBatch: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        u32,
        u32,
        *mut core::ffi::c_void,
        D2D1_BITMAP_INTERPOLATION_MODE,
        D2D1_SPRITE_OPTIONS,
    ),
}
impl windows_core::RuntimeName for ID2D1DeviceContext3 {}
windows_core::imp::define_interface!(
    ID2D1DeviceContext4,
    ID2D1DeviceContext4_Vtbl,
    0x8c427831_3d90_4476_b647_c4fae349e4db
);
impl core::ops::Deref for ID2D1DeviceContext4 {
    type Target = ID2D1DeviceContext3;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1DeviceContext4,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1RenderTarget,
    ID2D1DeviceContext,
    ID2D1DeviceContext1,
    ID2D1DeviceContext2,
    ID2D1DeviceContext3
);
#[repr(C)]
pub struct ID2D1DeviceContext4_Vtbl {
    pub base__: ID2D1DeviceContext3_Vtbl,
    CreateSvgGlyphStyle: usize,
    DrawText: usize,
    DrawTextLayout: usize,
    DrawColorBitmapGlyphRun: usize,
    DrawSvgGlyphRun: usize,
    GetColorBitmapGlyphImage: usize,
    GetSvgGlyphImage: usize,
}
impl windows_core::RuntimeName for ID2D1DeviceContext4 {}
windows_core::imp::define_interface!(
    ID2D1DeviceContext5,
    ID2D1DeviceContext5_Vtbl,
    0x7836d248_68cc_4df6_b9e8_de991bf62eb7
);
impl core::ops::Deref for ID2D1DeviceContext5 {
    type Target = ID2D1DeviceContext4;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1DeviceContext5,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1RenderTarget,
    ID2D1DeviceContext,
    ID2D1DeviceContext1,
    ID2D1DeviceContext2,
    ID2D1DeviceContext3,
    ID2D1DeviceContext4
);
#[repr(C)]
pub struct ID2D1DeviceContext5_Vtbl {
    pub base__: ID2D1DeviceContext4_Vtbl,
    CreateSvgDocument: usize,
    DrawSvgDocument: usize,
    CreateColorContextFromDxgiColorSpace: usize,
    CreateColorContextFromSimpleColorProfile: usize,
}
impl windows_core::RuntimeName for ID2D1DeviceContext5 {}
windows_core::imp::define_interface!(
    ID2D1DeviceContext6,
    ID2D1DeviceContext6_Vtbl,
    0x985f7e37_4ed0_4a19_98a3_15b0edfde306
);
impl core::ops::Deref for ID2D1DeviceContext6 {
    type Target = ID2D1DeviceContext5;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1DeviceContext6,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1RenderTarget,
    ID2D1DeviceContext,
    ID2D1DeviceContext1,
    ID2D1DeviceContext2,
    ID2D1DeviceContext3,
    ID2D1DeviceContext4,
    ID2D1DeviceContext5
);
#[repr(C)]
pub struct ID2D1DeviceContext6_Vtbl {
    pub base__: ID2D1DeviceContext5_Vtbl,
    BlendImage: usize,
}
impl windows_core::RuntimeName for ID2D1DeviceContext6 {}
windows_core::imp::define_interface!(
    ID2D1Factory,
    ID2D1Factory_Vtbl,
    0x06152247_6f50_465a_9245_118bfd3b6007
);
windows_core::imp::interface_hierarchy!(ID2D1Factory, windows_core::IUnknown);
#[repr(C)]
pub struct ID2D1Factory_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    ReloadSystemMetrics: usize,
    GetDesktopDpi: usize,
    CreateRectangleGeometry: usize,
    CreateRoundedRectangleGeometry: usize,
    CreateEllipseGeometry: usize,
    CreateGeometryGroup: usize,
    CreateTransformedGeometry: usize,
    CreatePathGeometry: usize,
    CreateStrokeStyle: usize,
    CreateDrawingStateBlock: usize,
    CreateWicBitmapRenderTarget: usize,
    CreateHwndRenderTarget: usize,
    CreateDxgiSurfaceRenderTarget: usize,
    CreateDCRenderTarget: usize,
}
impl windows_core::RuntimeName for ID2D1Factory {}
windows_core::imp::define_interface!(
    ID2D1Factory1,
    ID2D1Factory1_Vtbl,
    0xbb12d362_daee_4b9a_aa1d_14ba401cfa1f
);
impl core::ops::Deref for ID2D1Factory1 {
    type Target = ID2D1Factory;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1Factory1, windows_core::IUnknown, ID2D1Factory);
impl ID2D1Factory1 {
    pub unsafe fn CreateStrokeStyle(
        &self,
        strokestyleproperties: *const D2D1_STROKE_STYLE_PROPERTIES1,
        dashes: Option<&[f32]>,
    ) -> windows_core::Result<ID2D1StrokeStyle1> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateStrokeStyle)(
                windows_core::Interface::as_raw(self),
                strokestyleproperties,
                dashes.map_or(core::ptr::null(), |slice| slice.as_ptr()),
                dashes.map_or(0, |slice| slice.len().try_into().unwrap()),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreatePathGeometry(&self) -> windows_core::Result<ID2D1PathGeometry1> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreatePathGeometry)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ID2D1Factory1_Vtbl {
    pub base__: ID2D1Factory_Vtbl,
    CreateDevice: usize,
    pub CreateStrokeStyle: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D1_STROKE_STYLE_PROPERTIES1,
        *const f32,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreatePathGeometry: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateDrawingStateBlock: usize,
    CreateGdiMetafile: usize,
    RegisterEffectFromStream: usize,
    RegisterEffectFromString: usize,
    UnregisterEffect: usize,
    GetRegisteredEffects: usize,
    GetEffectProperties: usize,
}
impl windows_core::RuntimeName for ID2D1Factory1 {}
windows_core::imp::define_interface!(
    ID2D1Factory2,
    ID2D1Factory2_Vtbl,
    0x94f81a73_9212_4376_9c58_b16a3a0d3992
);
impl core::ops::Deref for ID2D1Factory2 {
    type Target = ID2D1Factory1;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1Factory2,
    windows_core::IUnknown,
    ID2D1Factory,
    ID2D1Factory1
);
#[repr(C)]
pub struct ID2D1Factory2_Vtbl {
    pub base__: ID2D1Factory1_Vtbl,
    CreateDevice: usize,
}
impl windows_core::RuntimeName for ID2D1Factory2 {}
windows_core::imp::define_interface!(
    ID2D1Factory3,
    ID2D1Factory3_Vtbl,
    0x0869759f_4f00_413f_b03e_2bda45404d0f
);
impl core::ops::Deref for ID2D1Factory3 {
    type Target = ID2D1Factory2;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1Factory3,
    windows_core::IUnknown,
    ID2D1Factory,
    ID2D1Factory1,
    ID2D1Factory2
);
#[repr(C)]
pub struct ID2D1Factory3_Vtbl {
    pub base__: ID2D1Factory2_Vtbl,
    CreateDevice: usize,
}
impl windows_core::RuntimeName for ID2D1Factory3 {}
windows_core::imp::define_interface!(
    ID2D1Factory4,
    ID2D1Factory4_Vtbl,
    0xbd4ec2d2_0662_4bee_ba8e_6f29f032e096
);
impl core::ops::Deref for ID2D1Factory4 {
    type Target = ID2D1Factory3;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1Factory4,
    windows_core::IUnknown,
    ID2D1Factory,
    ID2D1Factory1,
    ID2D1Factory2,
    ID2D1Factory3
);
#[repr(C)]
pub struct ID2D1Factory4_Vtbl {
    pub base__: ID2D1Factory3_Vtbl,
    CreateDevice: usize,
}
impl windows_core::RuntimeName for ID2D1Factory4 {}
windows_core::imp::define_interface!(
    ID2D1Factory5,
    ID2D1Factory5_Vtbl,
    0xc4349994_838e_4b0f_8cab_44997d9eeacc
);
impl core::ops::Deref for ID2D1Factory5 {
    type Target = ID2D1Factory4;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1Factory5,
    windows_core::IUnknown,
    ID2D1Factory,
    ID2D1Factory1,
    ID2D1Factory2,
    ID2D1Factory3,
    ID2D1Factory4
);
#[repr(C)]
pub struct ID2D1Factory5_Vtbl {
    pub base__: ID2D1Factory4_Vtbl,
    CreateDevice: usize,
}
impl windows_core::RuntimeName for ID2D1Factory5 {}
windows_core::imp::define_interface!(
    ID2D1Factory6,
    ID2D1Factory6_Vtbl,
    0xf9976f46_f642_44c1_97ca_da32ea2a2635
);
impl core::ops::Deref for ID2D1Factory6 {
    type Target = ID2D1Factory5;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1Factory6,
    windows_core::IUnknown,
    ID2D1Factory,
    ID2D1Factory1,
    ID2D1Factory2,
    ID2D1Factory3,
    ID2D1Factory4,
    ID2D1Factory5
);
#[repr(C)]
pub struct ID2D1Factory6_Vtbl {
    pub base__: ID2D1Factory5_Vtbl,
    CreateDevice: usize,
}
impl windows_core::RuntimeName for ID2D1Factory6 {}
windows_core::imp::define_interface!(
    ID2D1Factory7,
    ID2D1Factory7_Vtbl,
    0xbdc2bdd3_b96c_4de6_bdf7_99d4745454de
);
impl core::ops::Deref for ID2D1Factory7 {
    type Target = ID2D1Factory6;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1Factory7,
    windows_core::IUnknown,
    ID2D1Factory,
    ID2D1Factory1,
    ID2D1Factory2,
    ID2D1Factory3,
    ID2D1Factory4,
    ID2D1Factory5,
    ID2D1Factory6
);
impl ID2D1Factory7 {
    pub unsafe fn CreateDevice<P0>(&self, dxgidevice: P0) -> windows_core::Result<ID2D1Device6>
    where
        P0: windows_core::Param<IDXGIDevice>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateDevice)(
                windows_core::Interface::as_raw(self),
                dxgidevice.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ID2D1Factory7_Vtbl {
    pub base__: ID2D1Factory6_Vtbl,
    pub CreateDevice: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for ID2D1Factory7 {}
windows_core::imp::define_interface!(
    ID2D1Geometry,
    ID2D1Geometry_Vtbl,
    0x2cd906a1_12e2_11dc_9fed_001143a055f9
);
impl core::ops::Deref for ID2D1Geometry {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1Geometry, windows_core::IUnknown, ID2D1Resource);
impl ID2D1Geometry {
    pub unsafe fn GetBounds(
        &self,
        worldtransform: Option<*const windows_numerics::Matrix3x2>,
    ) -> windows_core::Result<D2D_RECT_F> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetBounds)(
                windows_core::Interface::as_raw(self),
                worldtransform.unwrap_or(core::mem::zeroed()) as _,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn StrokeContainsPoint<P2>(
        &self,
        point: windows_numerics::Vector2,
        strokewidth: f32,
        strokestyle: P2,
        worldtransform: Option<*const windows_numerics::Matrix3x2>,
        flatteningtolerance: f32,
    ) -> windows_core::Result<windows_core::BOOL>
    where
        P2: windows_core::Param<ID2D1StrokeStyle>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).StrokeContainsPoint)(
                windows_core::Interface::as_raw(self),
                point,
                strokewidth,
                strokestyle.param().abi(),
                worldtransform.unwrap_or(core::mem::zeroed()) as _,
                flatteningtolerance,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn FillContainsPoint(
        &self,
        point: windows_numerics::Vector2,
        worldtransform: Option<*const windows_numerics::Matrix3x2>,
        flatteningtolerance: f32,
    ) -> windows_core::Result<windows_core::BOOL> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).FillContainsPoint)(
                windows_core::Interface::as_raw(self),
                point,
                worldtransform.unwrap_or(core::mem::zeroed()) as _,
                flatteningtolerance,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn CombineWithGeometry<P0, P4>(
        &self,
        inputgeometry: P0,
        combinemode: D2D1_COMBINE_MODE,
        inputgeometrytransform: Option<*const windows_numerics::Matrix3x2>,
        flatteningtolerance: f32,
        geometrysink: P4,
    ) -> windows_core::HRESULT
    where
        P0: windows_core::Param<Self>,
        P4: windows_core::Param<ID2D1SimplifiedGeometrySink>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).CombineWithGeometry)(
                windows_core::Interface::as_raw(self),
                inputgeometry.param().abi(),
                combinemode,
                inputgeometrytransform.unwrap_or(core::mem::zeroed()) as _,
                flatteningtolerance,
                geometrysink.param().abi(),
            )
        }
    }
    pub unsafe fn Outline<P2>(
        &self,
        worldtransform: Option<*const windows_numerics::Matrix3x2>,
        flatteningtolerance: f32,
        geometrysink: P2,
    ) -> windows_core::HRESULT
    where
        P2: windows_core::Param<ID2D1SimplifiedGeometrySink>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).Outline)(
                windows_core::Interface::as_raw(self),
                worldtransform.unwrap_or(core::mem::zeroed()) as _,
                flatteningtolerance,
                geometrysink.param().abi(),
            )
        }
    }
    pub unsafe fn Widen<P1, P4>(
        &self,
        strokewidth: f32,
        strokestyle: P1,
        worldtransform: Option<*const windows_numerics::Matrix3x2>,
        flatteningtolerance: f32,
        geometrysink: P4,
    ) -> windows_core::HRESULT
    where
        P1: windows_core::Param<ID2D1StrokeStyle>,
        P4: windows_core::Param<ID2D1SimplifiedGeometrySink>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).Widen)(
                windows_core::Interface::as_raw(self),
                strokewidth,
                strokestyle.param().abi(),
                worldtransform.unwrap_or(core::mem::zeroed()) as _,
                flatteningtolerance,
                geometrysink.param().abi(),
            )
        }
    }
}
#[repr(C)]
pub struct ID2D1Geometry_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    pub GetBounds: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_numerics::Matrix3x2,
        *mut D2D_RECT_F,
    ) -> windows_core::HRESULT,
    GetWidenedBounds: usize,
    pub StrokeContainsPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
        f32,
        *mut core::ffi::c_void,
        *const windows_numerics::Matrix3x2,
        f32,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub FillContainsPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
        *const windows_numerics::Matrix3x2,
        f32,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    CompareWithGeometry: usize,
    Simplify: usize,
    Tessellate: usize,
    pub CombineWithGeometry: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        D2D1_COMBINE_MODE,
        *const windows_numerics::Matrix3x2,
        f32,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub Outline: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_numerics::Matrix3x2,
        f32,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    ComputeArea: usize,
    ComputeLength: usize,
    ComputePointAtLength: usize,
    pub Widen: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        *mut core::ffi::c_void,
        *const windows_numerics::Matrix3x2,
        f32,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for ID2D1Geometry {}
windows_core::imp::define_interface!(
    ID2D1GeometryRealization,
    ID2D1GeometryRealization_Vtbl,
    0xa16907d7_bc02_4801_99e8_8cf7f485f774
);
impl core::ops::Deref for ID2D1GeometryRealization {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1GeometryRealization,
    windows_core::IUnknown,
    ID2D1Resource
);
#[repr(C)]
pub struct ID2D1GeometryRealization_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
}
impl windows_core::RuntimeName for ID2D1GeometryRealization {}
windows_core::imp::define_interface!(
    ID2D1GeometrySink,
    ID2D1GeometrySink_Vtbl,
    0x2cd9069f_12e2_11dc_9fed_001143a055f9
);
impl core::ops::Deref for ID2D1GeometrySink {
    type Target = ID2D1SimplifiedGeometrySink;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1GeometrySink,
    windows_core::IUnknown,
    ID2D1SimplifiedGeometrySink
);
#[repr(C)]
pub struct ID2D1GeometrySink_Vtbl {
    pub base__: ID2D1SimplifiedGeometrySink_Vtbl,
    AddLine: usize,
    AddBezier: usize,
    AddQuadraticBezier: usize,
    AddQuadraticBeziers: usize,
    AddArc: usize,
}
impl windows_core::RuntimeName for ID2D1GeometrySink {}
windows_core::imp::define_interface!(
    ID2D1GradientStopCollection,
    ID2D1GradientStopCollection_Vtbl,
    0x2cd906a7_12e2_11dc_9fed_001143a055f9
);
impl core::ops::Deref for ID2D1GradientStopCollection {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1GradientStopCollection,
    windows_core::IUnknown,
    ID2D1Resource
);
#[repr(C)]
pub struct ID2D1GradientStopCollection_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    GetGradientStopCount: usize,
    GetGradientStops: usize,
    GetColorInterpolationGamma: usize,
    GetExtendMode: usize,
}
impl windows_core::RuntimeName for ID2D1GradientStopCollection {}
windows_core::imp::define_interface!(
    ID2D1GradientStopCollection1,
    ID2D1GradientStopCollection1_Vtbl,
    0xae1572f4_5dd0_4777_998b_9279472ae63b
);
impl core::ops::Deref for ID2D1GradientStopCollection1 {
    type Target = ID2D1GradientStopCollection;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1GradientStopCollection1,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1GradientStopCollection
);
impl ID2D1GradientStopCollection1 {
    pub unsafe fn GetGradientStops1(&self, gradientstops: &mut [D2D1_GRADIENT_STOP]) {
        unsafe {
            (windows_core::Interface::vtable(self).GetGradientStops1)(
                windows_core::Interface::as_raw(self),
                gradientstops.as_mut_ptr(),
                gradientstops.len().try_into().unwrap(),
            );
        }
    }
    pub unsafe fn GetPreInterpolationSpace(&self) -> D2D1_COLOR_SPACE {
        unsafe {
            (windows_core::Interface::vtable(self).GetPreInterpolationSpace)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn GetPostInterpolationSpace(&self) -> D2D1_COLOR_SPACE {
        unsafe {
            (windows_core::Interface::vtable(self).GetPostInterpolationSpace)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn GetBufferPrecision(&self) -> D2D1_BUFFER_PRECISION {
        unsafe {
            (windows_core::Interface::vtable(self).GetBufferPrecision)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn GetColorInterpolationMode(&self) -> D2D1_COLOR_INTERPOLATION_MODE {
        unsafe {
            (windows_core::Interface::vtable(self).GetColorInterpolationMode)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
}
#[repr(C)]
pub struct ID2D1GradientStopCollection1_Vtbl {
    pub base__: ID2D1GradientStopCollection_Vtbl,
    pub GetGradientStops1:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut D2D1_GRADIENT_STOP, u32),
    pub GetPreInterpolationSpace:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> D2D1_COLOR_SPACE,
    pub GetPostInterpolationSpace:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> D2D1_COLOR_SPACE,
    pub GetBufferPrecision:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> D2D1_BUFFER_PRECISION,
    pub GetColorInterpolationMode:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> D2D1_COLOR_INTERPOLATION_MODE,
}
impl windows_core::RuntimeName for ID2D1GradientStopCollection1 {}
windows_core::imp::define_interface!(
    ID2D1Image,
    ID2D1Image_Vtbl,
    0x65019f75_8da2_497c_b32c_dfa34e48ede6
);
impl core::ops::Deref for ID2D1Image {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1Image, windows_core::IUnknown, ID2D1Resource);
#[repr(C)]
pub struct ID2D1Image_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
}
impl windows_core::RuntimeName for ID2D1Image {}
windows_core::imp::define_interface!(
    ID2D1Layer,
    ID2D1Layer_Vtbl,
    0x2cd9069b_12e2_11dc_9fed_001143a055f9
);
impl core::ops::Deref for ID2D1Layer {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1Layer, windows_core::IUnknown, ID2D1Resource);
impl ID2D1Layer {
    pub unsafe fn GetSize(&self) -> D2D_SIZE_F {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetSize)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            );
            result__
        }
    }
}
#[repr(C)]
pub struct ID2D1Layer_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    pub GetSize: unsafe extern "system" fn(*mut core::ffi::c_void, *mut D2D_SIZE_F),
}
impl windows_core::RuntimeName for ID2D1Layer {}
windows_core::imp::define_interface!(
    ID2D1LinearGradientBrush,
    ID2D1LinearGradientBrush_Vtbl,
    0x2cd906ab_12e2_11dc_9fed_001143a055f9
);
impl core::ops::Deref for ID2D1LinearGradientBrush {
    type Target = ID2D1Brush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1LinearGradientBrush,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Brush
);
impl ID2D1LinearGradientBrush {
    pub unsafe fn SetStartPoint(&self, startpoint: windows_numerics::Vector2) {
        unsafe {
            (windows_core::Interface::vtable(self).SetStartPoint)(
                windows_core::Interface::as_raw(self),
                startpoint,
            );
        }
    }
    pub unsafe fn SetEndPoint(&self, endpoint: windows_numerics::Vector2) {
        unsafe {
            (windows_core::Interface::vtable(self).SetEndPoint)(
                windows_core::Interface::as_raw(self),
                endpoint,
            );
        }
    }
}
#[repr(C)]
pub struct ID2D1LinearGradientBrush_Vtbl {
    pub base__: ID2D1Brush_Vtbl,
    pub SetStartPoint: unsafe extern "system" fn(*mut core::ffi::c_void, windows_numerics::Vector2),
    pub SetEndPoint: unsafe extern "system" fn(*mut core::ffi::c_void, windows_numerics::Vector2),
    GetStartPoint: usize,
    GetEndPoint: usize,
    GetGradientStopCollection: usize,
}
impl windows_core::RuntimeName for ID2D1LinearGradientBrush {}
windows_core::imp::define_interface!(
    ID2D1PathGeometry,
    ID2D1PathGeometry_Vtbl,
    0x2cd906a5_12e2_11dc_9fed_001143a055f9
);
impl core::ops::Deref for ID2D1PathGeometry {
    type Target = ID2D1Geometry;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1PathGeometry,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Geometry
);
impl ID2D1PathGeometry {
    pub unsafe fn Open(&self) -> windows_core::Result<ID2D1GeometrySink> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Open)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetSegmentCount(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetSegmentCount)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn GetFigureCount(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFigureCount)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct ID2D1PathGeometry_Vtbl {
    pub base__: ID2D1Geometry_Vtbl,
    pub Open: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    Stream: usize,
    pub GetSegmentCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub GetFigureCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for ID2D1PathGeometry {}
windows_core::imp::define_interface!(
    ID2D1PathGeometry1,
    ID2D1PathGeometry1_Vtbl,
    0x62baa2d2_ab54_41b7_b872_787e0106a421
);
impl core::ops::Deref for ID2D1PathGeometry1 {
    type Target = ID2D1PathGeometry;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1PathGeometry1,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Geometry,
    ID2D1PathGeometry
);
#[repr(C)]
pub struct ID2D1PathGeometry1_Vtbl {
    pub base__: ID2D1PathGeometry_Vtbl,
    ComputePointAndSegmentAtLength: usize,
}
impl windows_core::RuntimeName for ID2D1PathGeometry1 {}
windows_core::imp::define_interface!(
    ID2D1RadialGradientBrush,
    ID2D1RadialGradientBrush_Vtbl,
    0x2cd906ac_12e2_11dc_9fed_001143a055f9
);
impl core::ops::Deref for ID2D1RadialGradientBrush {
    type Target = ID2D1Brush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1RadialGradientBrush,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Brush
);
impl ID2D1RadialGradientBrush {
    pub unsafe fn SetCenter(&self, center: windows_numerics::Vector2) {
        unsafe {
            (windows_core::Interface::vtable(self).SetCenter)(
                windows_core::Interface::as_raw(self),
                center,
            );
        }
    }
    pub unsafe fn SetGradientOriginOffset(&self, gradientoriginoffset: windows_numerics::Vector2) {
        unsafe {
            (windows_core::Interface::vtable(self).SetGradientOriginOffset)(
                windows_core::Interface::as_raw(self),
                gradientoriginoffset,
            );
        }
    }
    pub unsafe fn SetRadiusX(&self, radiusx: f32) {
        unsafe {
            (windows_core::Interface::vtable(self).SetRadiusX)(
                windows_core::Interface::as_raw(self),
                radiusx,
            );
        }
    }
    pub unsafe fn SetRadiusY(&self, radiusy: f32) {
        unsafe {
            (windows_core::Interface::vtable(self).SetRadiusY)(
                windows_core::Interface::as_raw(self),
                radiusy,
            );
        }
    }
    pub unsafe fn GetCenter(&self) -> windows_numerics::Vector2 {
        unsafe {
            (windows_core::Interface::vtable(self).GetCenter)(windows_core::Interface::as_raw(self))
        }
    }
    pub unsafe fn GetGradientOriginOffset(&self) -> windows_numerics::Vector2 {
        unsafe {
            (windows_core::Interface::vtable(self).GetGradientOriginOffset)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn GetRadiusX(&self) -> f32 {
        unsafe {
            (windows_core::Interface::vtable(self).GetRadiusX)(windows_core::Interface::as_raw(
                self,
            ))
        }
    }
    pub unsafe fn GetRadiusY(&self) -> f32 {
        unsafe {
            (windows_core::Interface::vtable(self).GetRadiusY)(windows_core::Interface::as_raw(
                self,
            ))
        }
    }
    pub unsafe fn GetGradientStopCollection(
        &self,
    ) -> windows_core::Result<ID2D1GradientStopCollection> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetGradientStopCollection)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            );
            windows_core::Type::from_abi(result__)
        }
    }
}
#[repr(C)]
pub struct ID2D1RadialGradientBrush_Vtbl {
    pub base__: ID2D1Brush_Vtbl,
    pub SetCenter: unsafe extern "system" fn(*mut core::ffi::c_void, windows_numerics::Vector2),
    pub SetGradientOriginOffset:
        unsafe extern "system" fn(*mut core::ffi::c_void, windows_numerics::Vector2),
    pub SetRadiusX: unsafe extern "system" fn(*mut core::ffi::c_void, f32),
    pub SetRadiusY: unsafe extern "system" fn(*mut core::ffi::c_void, f32),
    pub GetCenter: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_numerics::Vector2,
    pub GetGradientOriginOffset:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_numerics::Vector2,
    pub GetRadiusX: unsafe extern "system" fn(*mut core::ffi::c_void) -> f32,
    pub GetRadiusY: unsafe extern "system" fn(*mut core::ffi::c_void) -> f32,
    pub GetGradientStopCollection:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void),
}
impl windows_core::RuntimeName for ID2D1RadialGradientBrush {}
windows_core::imp::define_interface!(
    ID2D1RenderTarget,
    ID2D1RenderTarget_Vtbl,
    0x2cd90694_12e2_11dc_9fed_001143a055f9
);
impl core::ops::Deref for ID2D1RenderTarget {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1RenderTarget, windows_core::IUnknown, ID2D1Resource);
impl ID2D1RenderTarget {
    pub unsafe fn CreateSolidColorBrush(
        &self,
        color: *const D2D_COLOR_F,
        brushproperties: Option<*const D2D1_BRUSH_PROPERTIES>,
    ) -> windows_core::Result<ID2D1SolidColorBrush> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateSolidColorBrush)(
                windows_core::Interface::as_raw(self),
                color,
                brushproperties.unwrap_or(core::mem::zeroed()) as _,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateLinearGradientBrush<P2>(
        &self,
        lineargradientbrushproperties: *const D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
        brushproperties: Option<*const D2D1_BRUSH_PROPERTIES>,
        gradientstopcollection: P2,
    ) -> windows_core::Result<ID2D1LinearGradientBrush>
    where
        P2: windows_core::Param<ID2D1GradientStopCollection>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateLinearGradientBrush)(
                windows_core::Interface::as_raw(self),
                lineargradientbrushproperties,
                brushproperties.unwrap_or(core::mem::zeroed()) as _,
                gradientstopcollection.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateRadialGradientBrush<P2>(
        &self,
        radialgradientbrushproperties: *const D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES,
        brushproperties: Option<*const D2D1_BRUSH_PROPERTIES>,
        gradientstopcollection: P2,
    ) -> windows_core::Result<ID2D1RadialGradientBrush>
    where
        P2: windows_core::Param<ID2D1GradientStopCollection>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateRadialGradientBrush)(
                windows_core::Interface::as_raw(self),
                radialgradientbrushproperties,
                brushproperties.unwrap_or(core::mem::zeroed()) as _,
                gradientstopcollection.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn DrawLine<P2, P4>(
        &self,
        point0: windows_numerics::Vector2,
        point1: windows_numerics::Vector2,
        brush: P2,
        strokewidth: f32,
        strokestyle: P4,
    ) where
        P2: windows_core::Param<ID2D1Brush>,
        P4: windows_core::Param<ID2D1StrokeStyle>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawLine)(
                windows_core::Interface::as_raw(self),
                point0,
                point1,
                brush.param().abi(),
                strokewidth,
                strokestyle.param().abi(),
            );
        }
    }
    pub unsafe fn DrawRectangle<P1, P3>(
        &self,
        rect: *const D2D_RECT_F,
        brush: P1,
        strokewidth: f32,
        strokestyle: P3,
    ) where
        P1: windows_core::Param<ID2D1Brush>,
        P3: windows_core::Param<ID2D1StrokeStyle>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawRectangle)(
                windows_core::Interface::as_raw(self),
                rect,
                brush.param().abi(),
                strokewidth,
                strokestyle.param().abi(),
            );
        }
    }
    pub unsafe fn FillRectangle<P1>(&self, rect: *const D2D_RECT_F, brush: P1)
    where
        P1: windows_core::Param<ID2D1Brush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).FillRectangle)(
                windows_core::Interface::as_raw(self),
                rect,
                brush.param().abi(),
            );
        }
    }
    pub unsafe fn DrawRoundedRectangle<P1, P3>(
        &self,
        roundedrect: *const D2D1_ROUNDED_RECT,
        brush: P1,
        strokewidth: f32,
        strokestyle: P3,
    ) where
        P1: windows_core::Param<ID2D1Brush>,
        P3: windows_core::Param<ID2D1StrokeStyle>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawRoundedRectangle)(
                windows_core::Interface::as_raw(self),
                roundedrect,
                brush.param().abi(),
                strokewidth,
                strokestyle.param().abi(),
            );
        }
    }
    pub unsafe fn FillRoundedRectangle<P1>(&self, roundedrect: *const D2D1_ROUNDED_RECT, brush: P1)
    where
        P1: windows_core::Param<ID2D1Brush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).FillRoundedRectangle)(
                windows_core::Interface::as_raw(self),
                roundedrect,
                brush.param().abi(),
            );
        }
    }
    pub unsafe fn DrawEllipse<P1, P3>(
        &self,
        ellipse: *const D2D1_ELLIPSE,
        brush: P1,
        strokewidth: f32,
        strokestyle: P3,
    ) where
        P1: windows_core::Param<ID2D1Brush>,
        P3: windows_core::Param<ID2D1StrokeStyle>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawEllipse)(
                windows_core::Interface::as_raw(self),
                ellipse,
                brush.param().abi(),
                strokewidth,
                strokestyle.param().abi(),
            );
        }
    }
    pub unsafe fn FillEllipse<P1>(&self, ellipse: *const D2D1_ELLIPSE, brush: P1)
    where
        P1: windows_core::Param<ID2D1Brush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).FillEllipse)(
                windows_core::Interface::as_raw(self),
                ellipse,
                brush.param().abi(),
            );
        }
    }
    pub unsafe fn DrawGeometry<P0, P1, P3>(
        &self,
        geometry: P0,
        brush: P1,
        strokewidth: f32,
        strokestyle: P3,
    ) where
        P0: windows_core::Param<ID2D1Geometry>,
        P1: windows_core::Param<ID2D1Brush>,
        P3: windows_core::Param<ID2D1StrokeStyle>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawGeometry)(
                windows_core::Interface::as_raw(self),
                geometry.param().abi(),
                brush.param().abi(),
                strokewidth,
                strokestyle.param().abi(),
            );
        }
    }
    pub unsafe fn FillGeometry<P0, P1, P2>(&self, geometry: P0, brush: P1, opacitybrush: P2)
    where
        P0: windows_core::Param<ID2D1Geometry>,
        P1: windows_core::Param<ID2D1Brush>,
        P2: windows_core::Param<ID2D1Brush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).FillGeometry)(
                windows_core::Interface::as_raw(self),
                geometry.param().abi(),
                brush.param().abi(),
                opacitybrush.param().abi(),
            );
        }
    }
    pub unsafe fn SetTransform(&self, transform: *const windows_numerics::Matrix3x2) {
        unsafe {
            (windows_core::Interface::vtable(self).SetTransform)(
                windows_core::Interface::as_raw(self),
                transform,
            );
        }
    }
    pub unsafe fn GetTransform(&self, transform: *mut windows_numerics::Matrix3x2) {
        unsafe {
            (windows_core::Interface::vtable(self).GetTransform)(
                windows_core::Interface::as_raw(self),
                transform as _,
            );
        }
    }
    pub unsafe fn SetAntialiasMode(&self, antialiasmode: D2D1_ANTIALIAS_MODE) {
        unsafe {
            (windows_core::Interface::vtable(self).SetAntialiasMode)(
                windows_core::Interface::as_raw(self),
                antialiasmode,
            );
        }
    }
    pub unsafe fn GetAntialiasMode(&self) -> D2D1_ANTIALIAS_MODE {
        unsafe {
            (windows_core::Interface::vtable(self).GetAntialiasMode)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn SetTextAntialiasMode(&self, textantialiasmode: D2D1_TEXT_ANTIALIAS_MODE) {
        unsafe {
            (windows_core::Interface::vtable(self).SetTextAntialiasMode)(
                windows_core::Interface::as_raw(self),
                textantialiasmode,
            );
        }
    }
    pub unsafe fn SetTextRenderingParams<P0>(&self, textrenderingparams: P0)
    where
        P0: windows_core::Param<IDWriteRenderingParams>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetTextRenderingParams)(
                windows_core::Interface::as_raw(self),
                textrenderingparams.param().abi(),
            );
        }
    }
    pub unsafe fn GetTextRenderingParams(&self) -> windows_core::Result<IDWriteRenderingParams> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetTextRenderingParams)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            );
            windows_core::Type::from_abi(result__)
        }
    }
    pub unsafe fn SetTags(&self, tag1: D2D1_TAG, tag2: D2D1_TAG) {
        unsafe {
            (windows_core::Interface::vtable(self).SetTags)(
                windows_core::Interface::as_raw(self),
                tag1,
                tag2,
            );
        }
    }
    pub unsafe fn PopLayer(&self) {
        unsafe {
            (windows_core::Interface::vtable(self).PopLayer)(windows_core::Interface::as_raw(self));
        }
    }
    pub unsafe fn PushAxisAlignedClip(
        &self,
        cliprect: *const D2D_RECT_F,
        antialiasmode: D2D1_ANTIALIAS_MODE,
    ) {
        unsafe {
            (windows_core::Interface::vtable(self).PushAxisAlignedClip)(
                windows_core::Interface::as_raw(self),
                cliprect,
                antialiasmode,
            );
        }
    }
    pub unsafe fn PopAxisAlignedClip(&self) {
        unsafe {
            (windows_core::Interface::vtable(self).PopAxisAlignedClip)(
                windows_core::Interface::as_raw(self),
            );
        }
    }
    pub unsafe fn Clear(&self, clearcolor: Option<*const D2D_COLOR_F>) {
        unsafe {
            (windows_core::Interface::vtable(self).Clear)(
                windows_core::Interface::as_raw(self),
                clearcolor.unwrap_or(core::mem::zeroed()) as _,
            );
        }
    }
    pub unsafe fn BeginDraw(&self) {
        unsafe {
            (windows_core::Interface::vtable(self).BeginDraw)(windows_core::Interface::as_raw(
                self,
            ));
        }
    }
    pub unsafe fn EndDraw(
        &self,
        tag1: Option<*mut D2D1_TAG>,
        tag2: Option<*mut D2D1_TAG>,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).EndDraw)(
                windows_core::Interface::as_raw(self),
                tag1.unwrap_or(core::mem::zeroed()) as _,
                tag2.unwrap_or(core::mem::zeroed()) as _,
            )
        }
    }
    pub unsafe fn SetDpi(&self, dpix: f32, dpiy: f32) {
        unsafe {
            (windows_core::Interface::vtable(self).SetDpi)(
                windows_core::Interface::as_raw(self),
                dpix,
                dpiy,
            );
        }
    }
    pub unsafe fn GetDpi(&self, dpix: *mut f32, dpiy: *mut f32) {
        unsafe {
            (windows_core::Interface::vtable(self).GetDpi)(
                windows_core::Interface::as_raw(self),
                dpix as _,
                dpiy as _,
            );
        }
    }
    pub unsafe fn GetPixelSize(&self) -> D2D_SIZE_U {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetPixelSize)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            );
            result__
        }
    }
}
#[repr(C)]
pub struct ID2D1RenderTarget_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    CreateBitmap: usize,
    CreateBitmapFromWicBitmap: usize,
    CreateSharedBitmap: usize,
    CreateBitmapBrush: usize,
    pub CreateSolidColorBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D_COLOR_F,
        *const D2D1_BRUSH_PROPERTIES,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateGradientStopCollection: usize,
    pub CreateLinearGradientBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES,
        *const D2D1_BRUSH_PROPERTIES,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateRadialGradientBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES,
        *const D2D1_BRUSH_PROPERTIES,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateCompatibleRenderTarget: usize,
    CreateLayer: usize,
    CreateMesh: usize,
    pub DrawLine: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
        windows_numerics::Vector2,
        *mut core::ffi::c_void,
        f32,
        *mut core::ffi::c_void,
    ),
    pub DrawRectangle: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D_RECT_F,
        *mut core::ffi::c_void,
        f32,
        *mut core::ffi::c_void,
    ),
    pub FillRectangle: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D_RECT_F,
        *mut core::ffi::c_void,
    ),
    pub DrawRoundedRectangle: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D1_ROUNDED_RECT,
        *mut core::ffi::c_void,
        f32,
        *mut core::ffi::c_void,
    ),
    pub FillRoundedRectangle: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D1_ROUNDED_RECT,
        *mut core::ffi::c_void,
    ),
    pub DrawEllipse: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D1_ELLIPSE,
        *mut core::ffi::c_void,
        f32,
        *mut core::ffi::c_void,
    ),
    pub FillEllipse: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D1_ELLIPSE,
        *mut core::ffi::c_void,
    ),
    pub DrawGeometry: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        f32,
        *mut core::ffi::c_void,
    ),
    pub FillGeometry: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ),
    FillMesh: usize,
    FillOpacityMask: usize,
    DrawBitmap: usize,
    DrawText: usize,
    DrawTextLayout: usize,
    DrawGlyphRun: usize,
    pub SetTransform:
        unsafe extern "system" fn(*mut core::ffi::c_void, *const windows_numerics::Matrix3x2),
    pub GetTransform:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut windows_numerics::Matrix3x2),
    pub SetAntialiasMode: unsafe extern "system" fn(*mut core::ffi::c_void, D2D1_ANTIALIAS_MODE),
    pub GetAntialiasMode: unsafe extern "system" fn(*mut core::ffi::c_void) -> D2D1_ANTIALIAS_MODE,
    pub SetTextAntialiasMode:
        unsafe extern "system" fn(*mut core::ffi::c_void, D2D1_TEXT_ANTIALIAS_MODE),
    GetTextAntialiasMode: usize,
    pub SetTextRenderingParams:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void),
    pub GetTextRenderingParams:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void),
    pub SetTags: unsafe extern "system" fn(*mut core::ffi::c_void, D2D1_TAG, D2D1_TAG),
    GetTags: usize,
    PushLayer: usize,
    pub PopLayer: unsafe extern "system" fn(*mut core::ffi::c_void),
    Flush: usize,
    SaveDrawingState: usize,
    RestoreDrawingState: usize,
    pub PushAxisAlignedClip:
        unsafe extern "system" fn(*mut core::ffi::c_void, *const D2D_RECT_F, D2D1_ANTIALIAS_MODE),
    pub PopAxisAlignedClip: unsafe extern "system" fn(*mut core::ffi::c_void),
    pub Clear: unsafe extern "system" fn(*mut core::ffi::c_void, *const D2D_COLOR_F),
    pub BeginDraw: unsafe extern "system" fn(*mut core::ffi::c_void),
    pub EndDraw: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut D2D1_TAG,
        *mut D2D1_TAG,
    ) -> windows_core::HRESULT,
    GetPixelFormat: usize,
    pub SetDpi: unsafe extern "system" fn(*mut core::ffi::c_void, f32, f32),
    pub GetDpi: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f32, *mut f32),
    GetSize: usize,
    pub GetPixelSize: unsafe extern "system" fn(*mut core::ffi::c_void, *mut D2D_SIZE_U),
    GetMaximumBitmapSize: usize,
    IsSupported: usize,
}
impl windows_core::RuntimeName for ID2D1RenderTarget {}
windows_core::imp::define_interface!(
    ID2D1Resource,
    ID2D1Resource_Vtbl,
    0x2cd90691_12e2_11dc_9fed_001143a055f9
);
windows_core::imp::interface_hierarchy!(ID2D1Resource, windows_core::IUnknown);
#[repr(C)]
pub struct ID2D1Resource_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetFactory: usize,
}
impl windows_core::RuntimeName for ID2D1Resource {}
windows_core::imp::define_interface!(
    ID2D1SimplifiedGeometrySink,
    ID2D1SimplifiedGeometrySink_Vtbl,
    0x2cd9069e_12e2_11dc_9fed_001143a055f9
);
windows_core::imp::interface_hierarchy!(ID2D1SimplifiedGeometrySink, windows_core::IUnknown);
impl ID2D1SimplifiedGeometrySink {
    pub unsafe fn BeginFigure(
        &self,
        startpoint: windows_numerics::Vector2,
        figurebegin: D2D1_FIGURE_BEGIN,
    ) {
        unsafe {
            (windows_core::Interface::vtable(self).BeginFigure)(
                windows_core::Interface::as_raw(self),
                startpoint,
                figurebegin,
            );
        }
    }
    pub unsafe fn AddLines(&self, points: &[windows_numerics::Vector2]) {
        unsafe {
            (windows_core::Interface::vtable(self).AddLines)(
                windows_core::Interface::as_raw(self),
                points.as_ptr(),
                points.len().try_into().unwrap(),
            );
        }
    }
    pub unsafe fn AddBeziers(&self, beziers: &[D2D1_BEZIER_SEGMENT]) {
        unsafe {
            (windows_core::Interface::vtable(self).AddBeziers)(
                windows_core::Interface::as_raw(self),
                beziers.as_ptr(),
                beziers.len().try_into().unwrap(),
            );
        }
    }
    pub unsafe fn EndFigure(&self, figureend: D2D1_FIGURE_END) {
        unsafe {
            (windows_core::Interface::vtable(self).EndFigure)(
                windows_core::Interface::as_raw(self),
                figureend,
            );
        }
    }
    pub unsafe fn Close(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Close)(windows_core::Interface::as_raw(self))
        }
    }
}
#[repr(C)]
pub struct ID2D1SimplifiedGeometrySink_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    SetFillMode: usize,
    SetSegmentFlags: usize,
    pub BeginFigure: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
        D2D1_FIGURE_BEGIN,
    ),
    pub AddLines:
        unsafe extern "system" fn(*mut core::ffi::c_void, *const windows_numerics::Vector2, u32),
    pub AddBeziers:
        unsafe extern "system" fn(*mut core::ffi::c_void, *const D2D1_BEZIER_SEGMENT, u32),
    pub EndFigure: unsafe extern "system" fn(*mut core::ffi::c_void, D2D1_FIGURE_END),
    pub Close: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for ID2D1SimplifiedGeometrySink {}
windows_core::imp::define_interface!(
    ID2D1SolidColorBrush,
    ID2D1SolidColorBrush_Vtbl,
    0x2cd906a9_12e2_11dc_9fed_001143a055f9
);
impl core::ops::Deref for ID2D1SolidColorBrush {
    type Target = ID2D1Brush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1SolidColorBrush,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Brush
);
impl ID2D1SolidColorBrush {
    pub unsafe fn SetColor(&self, color: *const D2D_COLOR_F) {
        unsafe {
            (windows_core::Interface::vtable(self).SetColor)(
                windows_core::Interface::as_raw(self),
                color,
            );
        }
    }
}
#[repr(C)]
pub struct ID2D1SolidColorBrush_Vtbl {
    pub base__: ID2D1Brush_Vtbl,
    pub SetColor: unsafe extern "system" fn(*mut core::ffi::c_void, *const D2D_COLOR_F),
    GetColor: usize,
}
impl windows_core::RuntimeName for ID2D1SolidColorBrush {}
windows_core::imp::define_interface!(
    ID2D1SpriteBatch,
    ID2D1SpriteBatch_Vtbl,
    0x4dc583bf_3a10_438a_8722_e9765224f1f1
);
impl core::ops::Deref for ID2D1SpriteBatch {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1SpriteBatch, windows_core::IUnknown, ID2D1Resource);
impl ID2D1SpriteBatch {
    pub unsafe fn AddSprites(
        &self,
        spritecount: u32,
        destinationrectangles: *const D2D_RECT_F,
        sourcerectangles: Option<*const D2D_RECT_U>,
        colors: Option<*const D2D_COLOR_F>,
        transforms: Option<*const windows_numerics::Matrix3x2>,
        destinationrectanglesstride: u32,
        sourcerectanglesstride: u32,
        colorsstride: u32,
        transformsstride: u32,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).AddSprites)(
                windows_core::Interface::as_raw(self),
                spritecount,
                destinationrectangles,
                sourcerectangles.unwrap_or(core::mem::zeroed()) as _,
                colors.unwrap_or(core::mem::zeroed()) as _,
                transforms.unwrap_or(core::mem::zeroed()) as _,
                destinationrectanglesstride,
                sourcerectanglesstride,
                colorsstride,
                transformsstride,
            )
        }
    }
    pub unsafe fn SetSprites(
        &self,
        startindex: u32,
        spritecount: u32,
        destinationrectangles: Option<*const D2D_RECT_F>,
        sourcerectangles: Option<*const D2D_RECT_U>,
        colors: Option<*const D2D_COLOR_F>,
        transforms: Option<*const windows_numerics::Matrix3x2>,
        destinationrectanglesstride: u32,
        sourcerectanglesstride: u32,
        colorsstride: u32,
        transformsstride: u32,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetSprites)(
                windows_core::Interface::as_raw(self),
                startindex,
                spritecount,
                destinationrectangles.unwrap_or(core::mem::zeroed()) as _,
                sourcerectangles.unwrap_or(core::mem::zeroed()) as _,
                colors.unwrap_or(core::mem::zeroed()) as _,
                transforms.unwrap_or(core::mem::zeroed()) as _,
                destinationrectanglesstride,
                sourcerectanglesstride,
                colorsstride,
                transformsstride,
            )
        }
    }
    pub unsafe fn GetSpriteCount(&self) -> u32 {
        unsafe {
            (windows_core::Interface::vtable(self).GetSpriteCount)(windows_core::Interface::as_raw(
                self,
            ))
        }
    }
    pub unsafe fn Clear(&self) {
        unsafe {
            (windows_core::Interface::vtable(self).Clear)(windows_core::Interface::as_raw(self));
        }
    }
}
#[repr(C)]
pub struct ID2D1SpriteBatch_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    pub AddSprites: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *const D2D_RECT_F,
        *const D2D_RECT_U,
        *const D2D_COLOR_F,
        *const windows_numerics::Matrix3x2,
        u32,
        u32,
        u32,
        u32,
    ) -> windows_core::HRESULT,
    pub SetSprites: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        u32,
        *const D2D_RECT_F,
        *const D2D_RECT_U,
        *const D2D_COLOR_F,
        *const windows_numerics::Matrix3x2,
        u32,
        u32,
        u32,
        u32,
    ) -> windows_core::HRESULT,
    GetSprites: usize,
    pub GetSpriteCount: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
    pub Clear: unsafe extern "system" fn(*mut core::ffi::c_void),
}
impl windows_core::RuntimeName for ID2D1SpriteBatch {}
windows_core::imp::define_interface!(
    ID2D1StrokeStyle,
    ID2D1StrokeStyle_Vtbl,
    0x2cd9069d_12e2_11dc_9fed_001143a055f9
);
impl core::ops::Deref for ID2D1StrokeStyle {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1StrokeStyle, windows_core::IUnknown, ID2D1Resource);
#[repr(C)]
pub struct ID2D1StrokeStyle_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    GetStartCap: usize,
    GetEndCap: usize,
    GetDashCap: usize,
    GetMiterLimit: usize,
    GetLineJoin: usize,
    GetDashOffset: usize,
    GetDashStyle: usize,
    GetDashesCount: usize,
    GetDashes: usize,
}
impl windows_core::RuntimeName for ID2D1StrokeStyle {}
windows_core::imp::define_interface!(
    ID2D1StrokeStyle1,
    ID2D1StrokeStyle1_Vtbl,
    0x10a72a66_e91c_43f4_993f_ddf4b82b0b4a
);
impl core::ops::Deref for ID2D1StrokeStyle1 {
    type Target = ID2D1StrokeStyle;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1StrokeStyle1,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1StrokeStyle
);
#[repr(C)]
pub struct ID2D1StrokeStyle1_Vtbl {
    pub base__: ID2D1StrokeStyle_Vtbl,
    GetStrokeTransformType: usize,
}
impl windows_core::RuntimeName for ID2D1StrokeStyle1 {}
windows_core::imp::define_interface!(
    ID3D11Device,
    ID3D11Device_Vtbl,
    0xdb6f6ddb_ac77_4e88_8253_819df9bbf140
);
windows_core::imp::interface_hierarchy!(ID3D11Device, windows_core::IUnknown);
#[repr(C)]
pub struct ID3D11Device_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    CreateBuffer: usize,
    CreateTexture1D: usize,
    CreateTexture2D: usize,
    CreateTexture3D: usize,
    CreateShaderResourceView: usize,
    CreateUnorderedAccessView: usize,
    CreateRenderTargetView: usize,
    CreateDepthStencilView: usize,
    CreateInputLayout: usize,
    CreateVertexShader: usize,
    CreateGeometryShader: usize,
    CreateGeometryShaderWithStreamOutput: usize,
    CreatePixelShader: usize,
    CreateHullShader: usize,
    CreateDomainShader: usize,
    CreateComputeShader: usize,
    CreateClassLinkage: usize,
    CreateBlendState: usize,
    CreateDepthStencilState: usize,
    CreateRasterizerState: usize,
    CreateSamplerState: usize,
    CreateQuery: usize,
    CreatePredicate: usize,
    CreateCounter: usize,
    CreateDeferredContext: usize,
    OpenSharedResource: usize,
    CheckFormatSupport: usize,
    CheckMultisampleQualityLevels: usize,
    CheckCounterInfo: usize,
    CheckCounter: usize,
    CheckFeatureSupport: usize,
    GetPrivateData: usize,
    SetPrivateData: usize,
    SetPrivateDataInterface: usize,
    GetFeatureLevel: usize,
    GetCreationFlags: usize,
    GetDeviceRemovedReason: usize,
    GetImmediateContext: usize,
    SetExceptionMode: usize,
    GetExceptionMode: usize,
}
impl windows_core::RuntimeName for ID3D11Device {}
windows_core::imp::define_interface!(
    ID3D11DeviceChild,
    ID3D11DeviceChild_Vtbl,
    0x1841e5c8_16b0_489b_bcc8_44cfb0d5deae
);
windows_core::imp::interface_hierarchy!(ID3D11DeviceChild, windows_core::IUnknown);
#[repr(C)]
pub struct ID3D11DeviceChild_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetDevice: usize,
    GetPrivateData: usize,
    SetPrivateData: usize,
    SetPrivateDataInterface: usize,
}
impl windows_core::RuntimeName for ID3D11DeviceChild {}
windows_core::imp::define_interface!(
    ID3D11DeviceContext,
    ID3D11DeviceContext_Vtbl,
    0xc0bfa96c_e089_44fb_8eaf_26f8796190da
);
impl core::ops::Deref for ID3D11DeviceContext {
    type Target = ID3D11DeviceChild;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID3D11DeviceContext,
    windows_core::IUnknown,
    ID3D11DeviceChild
);
#[repr(C)]
pub struct ID3D11DeviceContext_Vtbl {
    pub base__: ID3D11DeviceChild_Vtbl,
    VSSetConstantBuffers: usize,
    PSSetShaderResources: usize,
    PSSetShader: usize,
    PSSetSamplers: usize,
    VSSetShader: usize,
    DrawIndexed: usize,
    Draw: usize,
    Map: usize,
    Unmap: usize,
    PSSetConstantBuffers: usize,
    IASetInputLayout: usize,
    IASetVertexBuffers: usize,
    IASetIndexBuffer: usize,
    DrawIndexedInstanced: usize,
    DrawInstanced: usize,
    GSSetConstantBuffers: usize,
    GSSetShader: usize,
    IASetPrimitiveTopology: usize,
    VSSetShaderResources: usize,
    VSSetSamplers: usize,
    Begin: usize,
    End: usize,
    GetData: usize,
    SetPredication: usize,
    GSSetShaderResources: usize,
    GSSetSamplers: usize,
    OMSetRenderTargets: usize,
    OMSetRenderTargetsAndUnorderedAccessViews: usize,
    OMSetBlendState: usize,
    OMSetDepthStencilState: usize,
    SOSetTargets: usize,
    DrawAuto: usize,
    DrawIndexedInstancedIndirect: usize,
    DrawInstancedIndirect: usize,
    Dispatch: usize,
    DispatchIndirect: usize,
    RSSetState: usize,
    RSSetViewports: usize,
    RSSetScissorRects: usize,
    CopySubresourceRegion: usize,
    CopyResource: usize,
    UpdateSubresource: usize,
    CopyStructureCount: usize,
    ClearRenderTargetView: usize,
    ClearUnorderedAccessViewUint: usize,
    ClearUnorderedAccessViewFloat: usize,
    ClearDepthStencilView: usize,
    GenerateMips: usize,
    SetResourceMinLOD: usize,
    GetResourceMinLOD: usize,
    ResolveSubresource: usize,
    ExecuteCommandList: usize,
    HSSetShaderResources: usize,
    HSSetShader: usize,
    HSSetSamplers: usize,
    HSSetConstantBuffers: usize,
    DSSetShaderResources: usize,
    DSSetShader: usize,
    DSSetSamplers: usize,
    DSSetConstantBuffers: usize,
    CSSetShaderResources: usize,
    CSSetUnorderedAccessViews: usize,
    CSSetShader: usize,
    CSSetSamplers: usize,
    CSSetConstantBuffers: usize,
    VSGetConstantBuffers: usize,
    PSGetShaderResources: usize,
    PSGetShader: usize,
    PSGetSamplers: usize,
    VSGetShader: usize,
    PSGetConstantBuffers: usize,
    IAGetInputLayout: usize,
    IAGetVertexBuffers: usize,
    IAGetIndexBuffer: usize,
    GSGetConstantBuffers: usize,
    GSGetShader: usize,
    IAGetPrimitiveTopology: usize,
    VSGetShaderResources: usize,
    VSGetSamplers: usize,
    GetPredication: usize,
    GSGetShaderResources: usize,
    GSGetSamplers: usize,
    OMGetRenderTargets: usize,
    OMGetRenderTargetsAndUnorderedAccessViews: usize,
    OMGetBlendState: usize,
    OMGetDepthStencilState: usize,
    SOGetTargets: usize,
    RSGetState: usize,
    RSGetViewports: usize,
    RSGetScissorRects: usize,
    HSGetShaderResources: usize,
    HSGetShader: usize,
    HSGetSamplers: usize,
    HSGetConstantBuffers: usize,
    DSGetShaderResources: usize,
    DSGetShader: usize,
    DSGetSamplers: usize,
    DSGetConstantBuffers: usize,
    CSGetShaderResources: usize,
    CSGetUnorderedAccessViews: usize,
    CSGetShader: usize,
    CSGetSamplers: usize,
    CSGetConstantBuffers: usize,
    ClearState: usize,
    Flush: usize,
    GetType: usize,
    GetContextFlags: usize,
    FinishCommandList: usize,
}
impl windows_core::RuntimeName for ID3D11DeviceContext {}
windows_core::imp::define_interface!(
    IDWriteFontFace,
    IDWriteFontFace_Vtbl,
    0x5f49804d_7024_4d43_bfa9_d25984f53849
);
windows_core::imp::interface_hierarchy!(IDWriteFontFace, windows_core::IUnknown);
#[repr(C)]
pub struct IDWriteFontFace_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetType: usize,
    GetFiles: usize,
    GetIndex: usize,
    GetSimulations: usize,
    IsSymbolFont: usize,
    GetMetrics: usize,
    GetGlyphCount: usize,
    GetDesignGlyphMetrics: usize,
    GetGlyphIndices: usize,
    TryGetFontTable: usize,
    ReleaseFontTable: usize,
    GetGlyphRunOutline: usize,
    GetRecommendedRenderingMode: usize,
    GetGdiCompatibleMetrics: usize,
    GetGdiCompatibleGlyphMetrics: usize,
}
impl windows_core::RuntimeName for IDWriteFontFace {}
windows_core::imp::define_interface!(
    IDWriteRenderingParams,
    IDWriteRenderingParams_Vtbl,
    0x2f0da53a_2add_47cd_82ee_d9ec34688e75
);
windows_core::imp::interface_hierarchy!(IDWriteRenderingParams, windows_core::IUnknown);
#[repr(C)]
pub struct IDWriteRenderingParams_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetGamma: usize,
    GetEnhancedContrast: usize,
    GetClearTypeLevel: usize,
    GetPixelGeometry: usize,
    GetRenderingMode: usize,
}
impl windows_core::RuntimeName for IDWriteRenderingParams {}
windows_core::imp::define_interface!(
    IDXGIAdapter,
    IDXGIAdapter_Vtbl,
    0x2411e7e1_12ac_4ccf_bd14_9798e8534dc0
);
impl core::ops::Deref for IDXGIAdapter {
    type Target = IDXGIObject;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IDXGIAdapter, windows_core::IUnknown, IDXGIObject);
#[repr(C)]
pub struct IDXGIAdapter_Vtbl {
    pub base__: IDXGIObject_Vtbl,
    EnumOutputs: usize,
    GetDesc: usize,
    CheckInterfaceSupport: usize,
}
impl windows_core::RuntimeName for IDXGIAdapter {}
windows_core::imp::define_interface!(
    IDXGIDevice,
    IDXGIDevice_Vtbl,
    0x54ec77fa_1377_44e6_8c32_88fd5f44c84c
);
impl core::ops::Deref for IDXGIDevice {
    type Target = IDXGIObject;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IDXGIDevice, windows_core::IUnknown, IDXGIObject);
impl IDXGIDevice {
    pub unsafe fn GetAdapter(&self) -> windows_core::Result<IDXGIAdapter> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetAdapter)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateSurface(
        &self,
        pdesc: *const DXGI_SURFACE_DESC,
        usage: DXGI_USAGE,
        psharedresource: Option<*const DXGI_SHARED_RESOURCE>,
        ppsurface: &mut [Option<IDXGISurface>],
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).CreateSurface)(
                windows_core::Interface::as_raw(self),
                pdesc,
                ppsurface.len().try_into().unwrap(),
                usage,
                psharedresource.unwrap_or(core::mem::zeroed()) as _,
                core::mem::transmute(ppsurface.as_mut_ptr()),
            )
        }
    }
    pub unsafe fn QueryResourceResidency(
        &self,
        ppresources: *const Option<windows_core::IUnknown>,
        presidencystatus: *mut DXGI_RESIDENCY,
        numresources: u32,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).QueryResourceResidency)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute(ppresources),
                presidencystatus as _,
                numresources,
            )
        }
    }
    pub unsafe fn SetGPUThreadPriority(&self, priority: i32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetGPUThreadPriority)(
                windows_core::Interface::as_raw(self),
                priority,
            )
        }
    }
    pub unsafe fn GetGPUThreadPriority(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetGPUThreadPriority)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IDXGIDevice_Vtbl {
    pub base__: IDXGIObject_Vtbl,
    pub GetAdapter: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateSurface: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const DXGI_SURFACE_DESC,
        u32,
        DXGI_USAGE,
        *const DXGI_SHARED_RESOURCE,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub QueryResourceResidency: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const *mut core::ffi::c_void,
        *mut DXGI_RESIDENCY,
        u32,
    ) -> windows_core::HRESULT,
    pub SetGPUThreadPriority:
        unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    pub GetGPUThreadPriority:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for IDXGIDevice {}
windows_core::imp::define_interface!(
    IDXGIDeviceSubObject,
    IDXGIDeviceSubObject_Vtbl,
    0x3d3e0379_f9de_4d58_bb6c_18d62992f1a6
);
impl core::ops::Deref for IDXGIDeviceSubObject {
    type Target = IDXGIObject;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IDXGIDeviceSubObject, windows_core::IUnknown, IDXGIObject);
#[repr(C)]
pub struct IDXGIDeviceSubObject_Vtbl {
    pub base__: IDXGIObject_Vtbl,
    GetDevice: usize,
}
impl windows_core::RuntimeName for IDXGIDeviceSubObject {}
windows_core::imp::define_interface!(
    IDXGIObject,
    IDXGIObject_Vtbl,
    0xaec22fb8_76f3_4639_9be0_28eb43a67a2e
);
windows_core::imp::interface_hierarchy!(IDXGIObject, windows_core::IUnknown);
#[repr(C)]
pub struct IDXGIObject_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    SetPrivateData: usize,
    SetPrivateDataInterface: usize,
    GetPrivateData: usize,
    GetParent: usize,
}
impl windows_core::RuntimeName for IDXGIObject {}
windows_core::imp::define_interface!(
    IDXGISurface,
    IDXGISurface_Vtbl,
    0xcafcb56c_6ac3_4889_bf47_9e23bbd260ec
);
impl core::ops::Deref for IDXGISurface {
    type Target = IDXGIDeviceSubObject;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGISurface,
    windows_core::IUnknown,
    IDXGIObject,
    IDXGIDeviceSubObject
);
#[repr(C)]
pub struct IDXGISurface_Vtbl {
    pub base__: IDXGIDeviceSubObject_Vtbl,
    GetDesc: usize,
    Map: usize,
    Unmap: usize,
}
impl windows_core::RuntimeName for IDXGISurface {}
