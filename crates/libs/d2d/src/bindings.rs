windows_core::link!("ole32.dll" "system" fn CoCreateInstance(rclsid : *const windows_core::GUID, punkouter : *mut core::ffi::c_void, dwclscontext : u32, riid : *const windows_core::GUID, ppv : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("d2d1.dll" "system" fn D2D1CreateFactory(factorytype : D2D1_FACTORY_TYPE, riid : *const windows_core::GUID, pfactoryoptions : *const D2D1_FACTORY_OPTIONS, ppifactory : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("d3d11.dll" "system" fn D3D11CreateDevice(padapter : *mut core::ffi::c_void, drivertype : D3D_DRIVER_TYPE, software : HMODULE, flags : u32, pfeaturelevels : *const D3D_FEATURE_LEVEL, featurelevels : u32, sdkversion : u32, ppdevice : *mut *mut core::ffi::c_void, pfeaturelevel : *mut D3D_FEATURE_LEVEL, ppimmediatecontext : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("kernel32.dll" "system" fn WaitForSingleObjectEx(hhandle : HANDLE, dwmilliseconds : u32, balertable : windows_core::BOOL) -> u32);
pub type CLSCTX = u32;
pub const CLSCTX_INPROC_SERVER: CLSCTX = 1;
pub const CLSID_D2D1ColorManagement: windows_core::GUID =
    windows_core::GUID::from_u128(0x1a28524c_fdd6_4aa4_ae8f_837eb8267b37);
pub const CLSID_D2D1GaussianBlur: windows_core::GUID =
    windows_core::GUID::from_u128(0x1feb6d69_2fe6_4ac9_8c58_1d7f93e7a6a5);
pub const CLSID_D2D1Shadow: windows_core::GUID =
    windows_core::GUID::from_u128(0xc67ea361_1863_4e69_89db_695d3e9a5b6b);
pub const CLSID_WICImagingFactory: windows_core::GUID =
    windows_core::GUID::from_u128(0xcacaf262_9370_4615_a13b_9f5539da4c0a);
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
pub const D2D1_BITMAP_OPTIONS_NONE: D2D1_BITMAP_OPTIONS = 0;
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
pub type D2D1_BLEND_MODE = i32;
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
pub type D2D1_COLOR_BITMAP_GLYPH_SNAP_OPTION = i32;
pub type D2D1_COLOR_CONTEXT_TYPE = i32;
pub type D2D1_COLOR_INTERPOLATION_MODE = i32;
pub const D2D1_COLOR_INTERPOLATION_MODE_STRAIGHT: D2D1_COLOR_INTERPOLATION_MODE = 0;
pub type D2D1_COLOR_SPACE = i32;
pub const D2D1_COLOR_SPACE_CUSTOM: D2D1_COLOR_SPACE = 0;
pub const D2D1_COLOR_SPACE_FORCE_DWORD: D2D1_COLOR_SPACE = -1;
pub const D2D1_COLOR_SPACE_SCRGB: D2D1_COLOR_SPACE = 2;
pub const D2D1_COLOR_SPACE_SRGB: D2D1_COLOR_SPACE = 1;
pub type D2D1_COMBINE_MODE = i32;
pub const D2D1_COMBINE_MODE_EXCLUDE: D2D1_COMBINE_MODE = 3;
pub const D2D1_COMBINE_MODE_INTERSECT: D2D1_COMBINE_MODE = 1;
pub const D2D1_COMBINE_MODE_UNION: D2D1_COMBINE_MODE = 0;
pub const D2D1_COMBINE_MODE_XOR: D2D1_COMBINE_MODE = 2;
pub type D2D1_COMPOSITE_MODE = i32;
pub type D2D1_DASH_STYLE = i32;
pub const D2D1_DASH_STYLE_CUSTOM: D2D1_DASH_STYLE = 5;
pub const D2D1_DASH_STYLE_DASH: D2D1_DASH_STYLE = 1;
pub const D2D1_DASH_STYLE_DASH_DOT: D2D1_DASH_STYLE = 3;
pub const D2D1_DASH_STYLE_DOT: D2D1_DASH_STYLE = 2;
pub const D2D1_DASH_STYLE_SOLID: D2D1_DASH_STYLE = 0;
pub type D2D1_DEBUG_LEVEL = i32;
pub type D2D1_DEVICE_CONTEXT_OPTIONS = u32;
pub const D2D1_DEVICE_CONTEXT_OPTIONS_ENABLE_MULTITHREADED_OPTIMIZATIONS:
    D2D1_DEVICE_CONTEXT_OPTIONS = 1;
pub const D2D1_DEVICE_CONTEXT_OPTIONS_NONE: D2D1_DEVICE_CONTEXT_OPTIONS = 0;
pub type D2D1_DRAW_TEXT_OPTIONS = u32;
pub const D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT: D2D1_DRAW_TEXT_OPTIONS = 4;
pub const D2D1_DRAW_TEXT_OPTIONS_NONE: D2D1_DRAW_TEXT_OPTIONS = 0;
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
pub const D2D1_FACTORY_TYPE_MULTI_THREADED: D2D1_FACTORY_TYPE = 1;
pub const D2D1_FACTORY_TYPE_SINGLE_THREADED: D2D1_FACTORY_TYPE = 0;
pub type D2D1_FIGURE_BEGIN = i32;
pub const D2D1_FIGURE_BEGIN_FILLED: D2D1_FIGURE_BEGIN = 0;
pub const D2D1_FIGURE_BEGIN_HOLLOW: D2D1_FIGURE_BEGIN = 1;
pub type D2D1_FIGURE_END = i32;
pub const D2D1_FIGURE_END_CLOSED: D2D1_FIGURE_END = 1;
pub const D2D1_FIGURE_END_OPEN: D2D1_FIGURE_END = 0;
pub type D2D1_GAMMA = i32;
pub type D2D1_GAMMA1 = i32;
pub const D2D1_GAMMA_1_0: D2D1_GAMMA = 1;
pub const D2D1_GAMMA_2_2: D2D1_GAMMA = 0;
pub type D2D1_GAUSSIANBLUR_PROP = i32;
pub const D2D1_GAUSSIANBLUR_PROP_STANDARD_DEVIATION: D2D1_GAUSSIANBLUR_PROP = 0;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_GRADIENT_MESH_PATCH {
    pub point00: windows_numerics::Vector2,
    pub point01: windows_numerics::Vector2,
    pub point02: windows_numerics::Vector2,
    pub point03: windows_numerics::Vector2,
    pub point10: windows_numerics::Vector2,
    pub point11: windows_numerics::Vector2,
    pub point12: windows_numerics::Vector2,
    pub point13: windows_numerics::Vector2,
    pub point20: windows_numerics::Vector2,
    pub point21: windows_numerics::Vector2,
    pub point22: windows_numerics::Vector2,
    pub point23: windows_numerics::Vector2,
    pub point30: windows_numerics::Vector2,
    pub point31: windows_numerics::Vector2,
    pub point32: windows_numerics::Vector2,
    pub point33: windows_numerics::Vector2,
    pub color00: D2D_COLOR_F,
    pub color03: D2D_COLOR_F,
    pub color30: D2D_COLOR_F,
    pub color33: D2D_COLOR_F,
    pub topEdgeMode: D2D1_PATCH_EDGE_MODE,
    pub leftEdgeMode: D2D1_PATCH_EDGE_MODE,
    pub bottomEdgeMode: D2D1_PATCH_EDGE_MODE,
    pub rightEdgeMode: D2D1_PATCH_EDGE_MODE,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_GRADIENT_STOP {
    pub position: f32,
    pub color: D2D_COLOR_F,
}
pub type D2D1_IMAGE_SOURCE_FROM_DXGI_OPTIONS = u32;
pub type D2D1_IMAGE_SOURCE_LOADING_OPTIONS = u32;
pub type D2D1_INK_NIB_SHAPE = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_INK_POINT {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_INK_STYLE_PROPERTIES {
    pub nibShape: D2D1_INK_NIB_SHAPE,
    pub nibTransform: windows_numerics::Matrix3x2,
}
pub type D2D1_INTERPOLATION_MODE = i32;
pub const D2D1_INTERPOLATION_MODE_LINEAR: D2D1_INTERPOLATION_MODE = 1;
pub const D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR: D2D1_INTERPOLATION_MODE = 0;
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
pub type D2D1_ORIENTATION = i32;
pub type D2D1_PATCH_EDGE_MODE = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct D2D1_PIXEL_FORMAT {
    pub format: DXGI_FORMAT,
    pub alphaMode: D2D1_ALPHA_MODE,
}
pub type D2D1_PRIMITIVE_BLEND = i32;
pub const D2D1_PRIMITIVE_BLEND_ADD: D2D1_PRIMITIVE_BLEND = 3;
pub const D2D1_PRIMITIVE_BLEND_SOURCE_OVER: D2D1_PRIMITIVE_BLEND = 0;
pub type D2D1_PROPERTY_TYPE = i32;
pub const D2D1_PROPERTY_TYPE_ENUM: D2D1_PROPERTY_TYPE = 11;
pub const D2D1_PROPERTY_TYPE_FLOAT: D2D1_PROPERTY_TYPE = 5;
pub const D2D1_PROPERTY_TYPE_VECTOR4: D2D1_PROPERTY_TYPE = 8;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_RADIAL_GRADIENT_BRUSH_PROPERTIES {
    pub center: windows_numerics::Vector2,
    pub gradientOriginOffset: windows_numerics::Vector2,
    pub radiusX: f32,
    pub radiusY: f32,
}
pub type D2D1_RENDERING_PRIORITY = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_ROUNDED_RECT {
    pub rect: D2D_RECT_F,
    pub radiusX: f32,
    pub radiusY: f32,
}
pub type D2D1_SHADOW_PROP = i32;
pub const D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION: D2D1_SHADOW_PROP = 0;
pub const D2D1_SHADOW_PROP_COLOR: D2D1_SHADOW_PROP = 1;
pub const D2D1_SHADOW_PROP_FORCE_DWORD: D2D1_SHADOW_PROP = -1;
pub const D2D1_SHADOW_PROP_OPTIMIZATION: D2D1_SHADOW_PROP = 2;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_SIMPLE_COLOR_PROFILE {
    pub redPrimary: windows_numerics::Vector2,
    pub greenPrimary: windows_numerics::Vector2,
    pub bluePrimary: windows_numerics::Vector2,
    pub whitePointXZ: windows_numerics::Vector2,
    pub gamma: D2D1_GAMMA1,
}
pub type D2D1_SPRITE_OPTIONS = u32;
pub const D2D1_SPRITE_OPTIONS_CLAMP_TO_SOURCE_RECTANGLE: D2D1_SPRITE_OPTIONS = 1;
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
pub type D2D1_TRANSFORMED_IMAGE_SOURCE_OPTIONS = u32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D2D1_TRANSFORMED_IMAGE_SOURCE_PROPERTIES {
    pub orientation: D2D1_ORIENTATION,
    pub scaleX: f32,
    pub scaleY: f32,
    pub interpolationMode: D2D1_INTERPOLATION_MODE,
    pub options: D2D1_TRANSFORMED_IMAGE_SOURCE_OPTIONS,
}
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
pub type DWRITE_GLYPH_IMAGE_FORMATS = u32;
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
pub type DXGI_ALPHA_MODE = i32;
pub const DXGI_ALPHA_MODE_IGNORE: DXGI_ALPHA_MODE = 3;
pub const DXGI_ALPHA_MODE_PREMULTIPLIED: DXGI_ALPHA_MODE = 1;
pub const DXGI_COLOR_SPACE_CUSTOM: DXGI_COLOR_SPACE_TYPE = -1;
pub const DXGI_COLOR_SPACE_RESERVED: DXGI_COLOR_SPACE_TYPE = 4;
pub const DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P2020: DXGI_COLOR_SPACE_TYPE = 25;
pub const DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709: DXGI_COLOR_SPACE_TYPE = 1;
pub const DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020: DXGI_COLOR_SPACE_TYPE = 12;
pub const DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P2020: DXGI_COLOR_SPACE_TYPE = 17;
pub const DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709: DXGI_COLOR_SPACE_TYPE = 0;
pub const DXGI_COLOR_SPACE_RGB_STUDIO_G2084_NONE_P2020: DXGI_COLOR_SPACE_TYPE = 14;
pub const DXGI_COLOR_SPACE_RGB_STUDIO_G22_NONE_P2020: DXGI_COLOR_SPACE_TYPE = 3;
pub const DXGI_COLOR_SPACE_RGB_STUDIO_G22_NONE_P709: DXGI_COLOR_SPACE_TYPE = 2;
pub const DXGI_COLOR_SPACE_RGB_STUDIO_G24_NONE_P2020: DXGI_COLOR_SPACE_TYPE = 21;
pub const DXGI_COLOR_SPACE_RGB_STUDIO_G24_NONE_P709: DXGI_COLOR_SPACE_TYPE = 20;
pub type DXGI_COLOR_SPACE_TYPE = i32;
pub const DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P2020: DXGI_COLOR_SPACE_TYPE = 11;
pub const DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P601: DXGI_COLOR_SPACE_TYPE = 7;
pub const DXGI_COLOR_SPACE_YCBCR_FULL_G22_LEFT_P709: DXGI_COLOR_SPACE_TYPE = 9;
pub const DXGI_COLOR_SPACE_YCBCR_FULL_G22_NONE_P709_X601: DXGI_COLOR_SPACE_TYPE = 5;
pub const DXGI_COLOR_SPACE_YCBCR_FULL_GHLG_TOPLEFT_P2020: DXGI_COLOR_SPACE_TYPE = 19;
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G2084_LEFT_P2020: DXGI_COLOR_SPACE_TYPE = 13;
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G2084_TOPLEFT_P2020: DXGI_COLOR_SPACE_TYPE = 16;
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P2020: DXGI_COLOR_SPACE_TYPE = 10;
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P601: DXGI_COLOR_SPACE_TYPE = 6;
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_LEFT_P709: DXGI_COLOR_SPACE_TYPE = 8;
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G22_TOPLEFT_P2020: DXGI_COLOR_SPACE_TYPE = 15;
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G24_LEFT_P2020: DXGI_COLOR_SPACE_TYPE = 23;
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G24_LEFT_P709: DXGI_COLOR_SPACE_TYPE = 22;
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_G24_TOPLEFT_P2020: DXGI_COLOR_SPACE_TYPE = 24;
pub const DXGI_COLOR_SPACE_YCBCR_STUDIO_GHLG_TOPLEFT_P2020: DXGI_COLOR_SPACE_TYPE = 18;
pub const DXGI_ERROR_DEVICE_HUNG: windows_core::HRESULT =
    windows_core::HRESULT(0x887A0006_u32 as _);
pub const DXGI_ERROR_DEVICE_REMOVED: windows_core::HRESULT =
    windows_core::HRESULT(0x887A0005_u32 as _);
pub const DXGI_ERROR_DEVICE_RESET: windows_core::HRESULT =
    windows_core::HRESULT(0x887A0007_u32 as _);
pub const DXGI_ERROR_DRIVER_INTERNAL_ERROR: windows_core::HRESULT =
    windows_core::HRESULT(0x887A0020_u32 as _);
pub type DXGI_FORMAT = i32;
pub const DXGI_FORMAT_B8G8R8A8_UNORM: DXGI_FORMAT = 87;
pub const DXGI_FORMAT_R16G16B16A16_FLOAT: DXGI_FORMAT = 10;
pub const DXGI_FORMAT_UNKNOWN: DXGI_FORMAT = 0;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DXGI_MATRIX_3X2_F {
    pub _11: f32,
    pub _12: f32,
    pub _21: f32,
    pub _22: f32,
    pub _31: f32,
    pub _32: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DXGI_MODE_DESC1 {
    pub Width: u32,
    pub Height: u32,
    pub RefreshRate: DXGI_RATIONAL,
    pub Format: DXGI_FORMAT,
    pub ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER,
    pub Scaling: DXGI_MODE_SCALING,
    pub Stereo: windows_core::BOOL,
}
pub type DXGI_MODE_ROTATION = i32;
pub type DXGI_MODE_SCALING = i32;
pub type DXGI_MODE_SCANLINE_ORDER = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DXGI_OUTPUT_DESC1 {
    pub DeviceName: [u16; 32],
    pub DesktopCoordinates: RECT,
    pub AttachedToDesktop: windows_core::BOOL,
    pub Rotation: DXGI_MODE_ROTATION,
    pub Monitor: HMONITOR,
    pub BitsPerColor: u32,
    pub ColorSpace: DXGI_COLOR_SPACE_TYPE,
    pub RedPrimary: [f32; 2],
    pub GreenPrimary: [f32; 2],
    pub BluePrimary: [f32; 2],
    pub WhitePoint: [f32; 2],
    pub MinLuminance: f32,
    pub MaxLuminance: f32,
    pub MaxFullFrameLuminance: f32,
}
impl Default for DXGI_OUTPUT_DESC1 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DXGI_RATIONAL {
    pub Numerator: u32,
    pub Denominator: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DXGI_SAMPLE_DESC {
    pub Count: u32,
    pub Quality: u32,
}
pub type DXGI_SCALING = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DXGI_SWAP_CHAIN_DESC1 {
    pub Width: u32,
    pub Height: u32,
    pub Format: DXGI_FORMAT,
    pub Stereo: windows_core::BOOL,
    pub SampleDesc: DXGI_SAMPLE_DESC,
    pub BufferUsage: DXGI_USAGE,
    pub BufferCount: u32,
    pub Scaling: DXGI_SCALING,
    pub SwapEffect: DXGI_SWAP_EFFECT,
    pub AlphaMode: DXGI_ALPHA_MODE,
    pub Flags: u32,
}
pub type DXGI_SWAP_CHAIN_FLAG = i32;
pub const DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT: DXGI_SWAP_CHAIN_FLAG = 64;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DXGI_SWAP_CHAIN_FULLSCREEN_DESC {
    pub RefreshRate: DXGI_RATIONAL,
    pub ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER,
    pub Scaling: DXGI_MODE_SCALING,
    pub Windowed: windows_core::BOOL,
}
pub type DXGI_SWAP_EFFECT = i32;
pub const DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL: DXGI_SWAP_EFFECT = 3;
pub type DXGI_USAGE = u32;
pub const DXGI_USAGE_RENDER_TARGET_OUTPUT: u32 = 32;
pub const E_FAIL: windows_core::HRESULT = windows_core::HRESULT(0x80004005_u32 as _);
pub const E_INVALIDARG: windows_core::HRESULT = windows_core::HRESULT(0x80070057_u32 as _);
pub const GENERIC_READ: u32 = 2147483648;
pub const GUID_WICPixelFormat32bppPBGRA: windows_core::GUID =
    windows_core::GUID::from_u128(0x6fddc324_4e03_4bfe_b185_3d77768dc910);
pub type HANDLE = *mut core::ffi::c_void;
pub type HINSTANCE = *mut core::ffi::c_void;
pub type HMODULE = HINSTANCE;
pub type HMONITOR = *mut core::ffi::c_void;
pub type HWND = *mut core::ffi::c_void;
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
    GetPixelSize: usize,
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
    ID2D1ColorContext1,
    ID2D1ColorContext1_Vtbl,
    0x1ab42875_c57f_4be9_bd85_9cd78d6f55ee
);
impl core::ops::Deref for ID2D1ColorContext1 {
    type Target = ID2D1ColorContext;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1ColorContext1,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1ColorContext
);
impl ID2D1ColorContext1 {
    pub unsafe fn GetColorContextType(&self) -> D2D1_COLOR_CONTEXT_TYPE {
        unsafe {
            (windows_core::Interface::vtable(self).GetColorContextType)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn GetDXGIColorSpace(&self) -> DXGI_COLOR_SPACE_TYPE {
        unsafe {
            (windows_core::Interface::vtable(self).GetDXGIColorSpace)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn GetSimpleColorProfile(
        &self,
        simpleprofile: *mut D2D1_SIMPLE_COLOR_PROFILE,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetSimpleColorProfile)(
                windows_core::Interface::as_raw(self),
                simpleprofile as _,
            )
        }
    }
}
#[repr(C)]
pub struct ID2D1ColorContext1_Vtbl {
    pub base__: ID2D1ColorContext_Vtbl,
    pub GetColorContextType:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> D2D1_COLOR_CONTEXT_TYPE,
    pub GetDXGIColorSpace:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> DXGI_COLOR_SPACE_TYPE,
    pub GetSimpleColorProfile: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut D2D1_SIMPLE_COLOR_PROFILE,
    ) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for ID2D1ColorContext1 {}
windows_core::imp::define_interface!(
    ID2D1CommandList,
    ID2D1CommandList_Vtbl,
    0xb4f34a19_2383_4d76_94f6_ec343657c3dc
);
impl core::ops::Deref for ID2D1CommandList {
    type Target = ID2D1Image;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1CommandList,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Image
);
impl ID2D1CommandList {
    pub unsafe fn Stream<P0>(&self, sink: P0) -> windows_core::HRESULT
    where
        P0: windows_core::Param<ID2D1CommandSink>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).Stream)(
                windows_core::Interface::as_raw(self),
                sink.param().abi(),
            )
        }
    }
    pub unsafe fn Close(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Close)(windows_core::Interface::as_raw(self))
        }
    }
}
#[repr(C)]
pub struct ID2D1CommandList_Vtbl {
    pub base__: ID2D1Image_Vtbl,
    pub Stream: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub Close: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for ID2D1CommandList {}
windows_core::imp::define_interface!(
    ID2D1CommandSink,
    ID2D1CommandSink_Vtbl,
    0x54d7898a_a061_40a7_bec7_e465bcba2c4f
);
windows_core::imp::interface_hierarchy!(ID2D1CommandSink, windows_core::IUnknown);
#[repr(C)]
pub struct ID2D1CommandSink_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    BeginDraw: usize,
    EndDraw: usize,
    SetAntialiasMode: usize,
    SetTags: usize,
    SetTextAntialiasMode: usize,
    SetTextRenderingParams: usize,
    SetTransform: usize,
    SetPrimitiveBlend: usize,
    SetUnitMode: usize,
    Clear: usize,
    DrawGlyphRun: usize,
    DrawLine: usize,
    DrawGeometry: usize,
    DrawRectangle: usize,
    DrawBitmap: usize,
    DrawImage: usize,
    DrawGdiMetafile: usize,
    FillMesh: usize,
    FillOpacityMask: usize,
    FillGeometry: usize,
    FillRectangle: usize,
    PushAxisAlignedClip: usize,
    PushLayer: usize,
    PopAxisAlignedClip: usize,
    PopLayer: usize,
}
impl windows_core::RuntimeName for ID2D1CommandSink {}
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
impl ID2D1Device {
    pub unsafe fn CreateDeviceContext(
        &self,
        options: D2D1_DEVICE_CONTEXT_OPTIONS,
    ) -> windows_core::Result<ID2D1DeviceContext> {
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
pub struct ID2D1Device_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    pub CreateDeviceContext: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        D2D1_DEVICE_CONTEXT_OPTIONS,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
impl ID2D1Device1 {
    pub unsafe fn GetRenderingPriority(&self) -> D2D1_RENDERING_PRIORITY {
        unsafe {
            (windows_core::Interface::vtable(self).GetRenderingPriority)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn SetRenderingPriority(&self, renderingpriority: D2D1_RENDERING_PRIORITY) {
        unsafe {
            (windows_core::Interface::vtable(self).SetRenderingPriority)(
                windows_core::Interface::as_raw(self),
                renderingpriority,
            );
        }
    }
    pub unsafe fn CreateDeviceContext(
        &self,
        options: D2D1_DEVICE_CONTEXT_OPTIONS,
    ) -> windows_core::Result<ID2D1DeviceContext1> {
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
pub struct ID2D1Device1_Vtbl {
    pub base__: ID2D1Device_Vtbl,
    pub GetRenderingPriority:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> D2D1_RENDERING_PRIORITY,
    pub SetRenderingPriority:
        unsafe extern "system" fn(*mut core::ffi::c_void, D2D1_RENDERING_PRIORITY),
    pub CreateDeviceContext: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        D2D1_DEVICE_CONTEXT_OPTIONS,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
impl ID2D1Device2 {
    pub unsafe fn CreateDeviceContext(
        &self,
        options: D2D1_DEVICE_CONTEXT_OPTIONS,
    ) -> windows_core::Result<ID2D1DeviceContext2> {
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
    pub unsafe fn FlushDeviceContexts<P0>(&self, bitmap: P0)
    where
        P0: windows_core::Param<ID2D1Bitmap>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).FlushDeviceContexts)(
                windows_core::Interface::as_raw(self),
                bitmap.param().abi(),
            );
        }
    }
    pub unsafe fn GetDxgiDevice(&self) -> windows_core::Result<IDXGIDevice> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetDxgiDevice)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ID2D1Device2_Vtbl {
    pub base__: ID2D1Device1_Vtbl,
    pub CreateDeviceContext: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        D2D1_DEVICE_CONTEXT_OPTIONS,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub FlushDeviceContexts:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void),
    pub GetDxgiDevice: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
impl ID2D1Device3 {
    pub unsafe fn CreateDeviceContext(
        &self,
        options: D2D1_DEVICE_CONTEXT_OPTIONS,
    ) -> windows_core::Result<ID2D1DeviceContext3> {
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
pub struct ID2D1Device3_Vtbl {
    pub base__: ID2D1Device2_Vtbl,
    pub CreateDeviceContext: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        D2D1_DEVICE_CONTEXT_OPTIONS,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
impl ID2D1Device4 {
    pub unsafe fn CreateDeviceContext(
        &self,
        options: D2D1_DEVICE_CONTEXT_OPTIONS,
    ) -> windows_core::Result<ID2D1DeviceContext4> {
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
    pub unsafe fn SetMaximumColorGlyphCacheMemory(&self, maximuminbytes: u64) {
        unsafe {
            (windows_core::Interface::vtable(self).SetMaximumColorGlyphCacheMemory)(
                windows_core::Interface::as_raw(self),
                maximuminbytes,
            );
        }
    }
    pub unsafe fn GetMaximumColorGlyphCacheMemory(&self) -> u64 {
        unsafe {
            (windows_core::Interface::vtable(self).GetMaximumColorGlyphCacheMemory)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
}
#[repr(C)]
pub struct ID2D1Device4_Vtbl {
    pub base__: ID2D1Device3_Vtbl,
    pub CreateDeviceContext: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        D2D1_DEVICE_CONTEXT_OPTIONS,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetMaximumColorGlyphCacheMemory: unsafe extern "system" fn(*mut core::ffi::c_void, u64),
    pub GetMaximumColorGlyphCacheMemory: unsafe extern "system" fn(*mut core::ffi::c_void) -> u64,
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
impl ID2D1Device5 {
    pub unsafe fn CreateDeviceContext(
        &self,
        options: D2D1_DEVICE_CONTEXT_OPTIONS,
    ) -> windows_core::Result<ID2D1DeviceContext5> {
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
pub struct ID2D1Device5_Vtbl {
    pub base__: ID2D1Device4_Vtbl,
    pub CreateDeviceContext: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        D2D1_DEVICE_CONTEXT_OPTIONS,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
    pub unsafe fn CreateBitmapFromWicBitmap<P0>(
        &self,
        wicbitmapsource: P0,
        bitmapproperties: Option<*const D2D1_BITMAP_PROPERTIES1>,
    ) -> windows_core::Result<ID2D1Bitmap1>
    where
        P0: windows_core::Param<IWICBitmapSource>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateBitmapFromWicBitmap)(
                windows_core::Interface::as_raw(self),
                wicbitmapsource.param().abi(),
                bitmapproperties.unwrap_or(core::mem::zeroed()) as _,
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
    pub unsafe fn CreateEffect(
        &self,
        effectid: *const windows_core::GUID,
    ) -> windows_core::Result<ID2D1Effect> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateEffect)(
                windows_core::Interface::as_raw(self),
                effectid,
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
    pub unsafe fn CreateCommandList(&self) -> windows_core::Result<ID2D1CommandList> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateCommandList)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
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
    pub unsafe fn GetTarget(&self) -> windows_core::Result<ID2D1Image> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetTarget)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            );
            windows_core::Type::from_abi(result__)
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
    pub unsafe fn DrawImage<P0>(
        &self,
        image: P0,
        targetoffset: Option<*const windows_numerics::Vector2>,
        imagerectangle: Option<*const D2D_RECT_F>,
        interpolationmode: D2D1_INTERPOLATION_MODE,
        compositemode: D2D1_COMPOSITE_MODE,
    ) where
        P0: windows_core::Param<ID2D1Image>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawImage)(
                windows_core::Interface::as_raw(self),
                image.param().abi(),
                targetoffset.unwrap_or(core::mem::zeroed()) as _,
                imagerectangle.unwrap_or(core::mem::zeroed()) as _,
                interpolationmode,
                compositemode,
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
    pub CreateBitmapFromWicBitmap: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *const D2D1_BITMAP_PROPERTIES1,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateColorContext: usize,
    CreateColorContextFromFilename: usize,
    CreateColorContextFromWicColorContext: usize,
    pub CreateBitmapFromDxgiSurface: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *const D2D1_BITMAP_PROPERTIES1,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateEffect: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
    pub CreateCommandList: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    IsDxgiFormatSupported: usize,
    IsBufferPrecisionSupported: usize,
    GetImageLocalBounds: usize,
    GetImageWorldBounds: usize,
    GetGlyphRunWorldBounds: usize,
    GetDevice: usize,
    pub SetTarget: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void),
    pub GetTarget: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void),
    SetRenderingControls: usize,
    GetRenderingControls: usize,
    pub SetPrimitiveBlend: unsafe extern "system" fn(*mut core::ffi::c_void, D2D1_PRIMITIVE_BLEND),
    GetPrimitiveBlend: usize,
    SetUnitMode: usize,
    GetUnitMode: usize,
    pub DrawGlyphRun: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
        *const DWRITE_GLYPH_RUN,
        *const DWRITE_GLYPH_RUN_DESCRIPTION,
        *mut core::ffi::c_void,
        DWRITE_MEASURING_MODE,
    ),
    pub DrawImage: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *const windows_numerics::Vector2,
        *const D2D_RECT_F,
        D2D1_INTERPOLATION_MODE,
        D2D1_COMPOSITE_MODE,
    ),
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
    PushLayer: usize,
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
impl ID2D1DeviceContext2 {
    pub unsafe fn CreateInk(
        &self,
        startpoint: *const D2D1_INK_POINT,
    ) -> windows_core::Result<ID2D1Ink> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateInk)(
                windows_core::Interface::as_raw(self),
                startpoint,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateInkStyle(
        &self,
        inkstyleproperties: Option<*const D2D1_INK_STYLE_PROPERTIES>,
    ) -> windows_core::Result<ID2D1InkStyle> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateInkStyle)(
                windows_core::Interface::as_raw(self),
                inkstyleproperties.unwrap_or(core::mem::zeroed()) as _,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateGradientMesh(
        &self,
        patches: &[D2D1_GRADIENT_MESH_PATCH],
    ) -> windows_core::Result<ID2D1GradientMesh> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateGradientMesh)(
                windows_core::Interface::as_raw(self),
                patches.as_ptr(),
                patches.len().try_into().unwrap(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateImageSourceFromWic<P0>(
        &self,
        wicbitmapsource: P0,
        loadingoptions: D2D1_IMAGE_SOURCE_LOADING_OPTIONS,
        alphamode: D2D1_ALPHA_MODE,
    ) -> windows_core::Result<ID2D1ImageSourceFromWic>
    where
        P0: windows_core::Param<IWICBitmapSource>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateImageSourceFromWic)(
                windows_core::Interface::as_raw(self),
                wicbitmapsource.param().abi(),
                loadingoptions,
                alphamode,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateLookupTable3D(
        &self,
        precision: D2D1_BUFFER_PRECISION,
        extents: &[u32; 3],
        data: &[u8],
        strides: &[u32; 2],
    ) -> windows_core::Result<ID2D1LookupTable3D> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateLookupTable3D)(
                windows_core::Interface::as_raw(self),
                precision,
                extents.as_ptr(),
                data.as_ptr(),
                data.len().try_into().unwrap(),
                strides.as_ptr(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateImageSourceFromDxgi(
        &self,
        surfaces: &[Option<IDXGISurface>],
        colorspace: DXGI_COLOR_SPACE_TYPE,
        options: D2D1_IMAGE_SOURCE_FROM_DXGI_OPTIONS,
    ) -> windows_core::Result<ID2D1ImageSource> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateImageSourceFromDxgi)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute(surfaces.as_ptr()),
                surfaces.len().try_into().unwrap(),
                colorspace,
                options,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetGradientMeshWorldBounds<P0>(
        &self,
        gradientmesh: P0,
    ) -> windows_core::Result<D2D_RECT_F>
    where
        P0: windows_core::Param<ID2D1GradientMesh>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetGradientMeshWorldBounds)(
                windows_core::Interface::as_raw(self),
                gradientmesh.param().abi(),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn DrawInk<P0, P1, P2>(&self, ink: P0, brush: P1, inkstyle: P2)
    where
        P0: windows_core::Param<ID2D1Ink>,
        P1: windows_core::Param<ID2D1Brush>,
        P2: windows_core::Param<ID2D1InkStyle>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawInk)(
                windows_core::Interface::as_raw(self),
                ink.param().abi(),
                brush.param().abi(),
                inkstyle.param().abi(),
            );
        }
    }
    pub unsafe fn DrawGradientMesh<P0>(&self, gradientmesh: P0)
    where
        P0: windows_core::Param<ID2D1GradientMesh>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawGradientMesh)(
                windows_core::Interface::as_raw(self),
                gradientmesh.param().abi(),
            );
        }
    }
    pub unsafe fn DrawGdiMetafile<P0>(
        &self,
        gdimetafile: P0,
        destinationrectangle: Option<*const D2D_RECT_F>,
        sourcerectangle: Option<*const D2D_RECT_F>,
    ) where
        P0: windows_core::Param<ID2D1GdiMetafile>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawGdiMetafile)(
                windows_core::Interface::as_raw(self),
                gdimetafile.param().abi(),
                destinationrectangle.unwrap_or(core::mem::zeroed()) as _,
                sourcerectangle.unwrap_or(core::mem::zeroed()) as _,
            );
        }
    }
    pub unsafe fn CreateTransformedImageSource<P0>(
        &self,
        imagesource: P0,
        properties: *const D2D1_TRANSFORMED_IMAGE_SOURCE_PROPERTIES,
    ) -> windows_core::Result<ID2D1TransformedImageSource>
    where
        P0: windows_core::Param<ID2D1ImageSource>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateTransformedImageSource)(
                windows_core::Interface::as_raw(self),
                imagesource.param().abi(),
                properties,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ID2D1DeviceContext2_Vtbl {
    pub base__: ID2D1DeviceContext1_Vtbl,
    pub CreateInk: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D1_INK_POINT,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateInkStyle: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D1_INK_STYLE_PROPERTIES,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateGradientMesh: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D1_GRADIENT_MESH_PATCH,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateImageSourceFromWic: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        D2D1_IMAGE_SOURCE_LOADING_OPTIONS,
        D2D1_ALPHA_MODE,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateLookupTable3D: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        D2D1_BUFFER_PRECISION,
        *const u32,
        *const u8,
        u32,
        *const u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateImageSourceFromDxgi: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const *mut core::ffi::c_void,
        u32,
        DXGI_COLOR_SPACE_TYPE,
        D2D1_IMAGE_SOURCE_FROM_DXGI_OPTIONS,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetGradientMeshWorldBounds: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut D2D_RECT_F,
    ) -> windows_core::HRESULT,
    pub DrawInk: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ),
    pub DrawGradientMesh: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void),
    pub DrawGdiMetafile: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *const D2D_RECT_F,
        *const D2D_RECT_F,
    ),
    pub CreateTransformedImageSource: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *const D2D1_TRANSFORMED_IMAGE_SOURCE_PROPERTIES,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
impl ID2D1DeviceContext4 {
    pub unsafe fn CreateSvgGlyphStyle(&self) -> windows_core::Result<ID2D1SvgGlyphStyle> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateSvgGlyphStyle)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn DrawText<P2, P4, P5>(
        &self,
        string: &[u16],
        textformat: P2,
        layoutrect: *const D2D_RECT_F,
        defaultfillbrush: P4,
        svgglyphstyle: P5,
        colorpaletteindex: u32,
        options: D2D1_DRAW_TEXT_OPTIONS,
        measuringmode: DWRITE_MEASURING_MODE,
    ) where
        P2: windows_core::Param<IDWriteTextFormat>,
        P4: windows_core::Param<ID2D1Brush>,
        P5: windows_core::Param<ID2D1SvgGlyphStyle>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawText)(
                windows_core::Interface::as_raw(self),
                string.as_ptr(),
                string.len().try_into().unwrap(),
                textformat.param().abi(),
                layoutrect,
                defaultfillbrush.param().abi(),
                svgglyphstyle.param().abi(),
                colorpaletteindex,
                options,
                measuringmode,
            );
        }
    }
    pub unsafe fn DrawTextLayout<P1, P2, P3>(
        &self,
        origin: windows_numerics::Vector2,
        textlayout: P1,
        defaultfillbrush: P2,
        svgglyphstyle: P3,
        colorpaletteindex: u32,
        options: D2D1_DRAW_TEXT_OPTIONS,
    ) where
        P1: windows_core::Param<IDWriteTextLayout>,
        P2: windows_core::Param<ID2D1Brush>,
        P3: windows_core::Param<ID2D1SvgGlyphStyle>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawTextLayout)(
                windows_core::Interface::as_raw(self),
                origin,
                textlayout.param().abi(),
                defaultfillbrush.param().abi(),
                svgglyphstyle.param().abi(),
                colorpaletteindex,
                options,
            );
        }
    }
    pub unsafe fn DrawColorBitmapGlyphRun(
        &self,
        glyphimageformat: DWRITE_GLYPH_IMAGE_FORMATS,
        baselineorigin: windows_numerics::Vector2,
        glyphrun: *const DWRITE_GLYPH_RUN,
        measuringmode: DWRITE_MEASURING_MODE,
        bitmapsnapoption: D2D1_COLOR_BITMAP_GLYPH_SNAP_OPTION,
    ) {
        unsafe {
            (windows_core::Interface::vtable(self).DrawColorBitmapGlyphRun)(
                windows_core::Interface::as_raw(self),
                glyphimageformat,
                baselineorigin,
                glyphrun,
                measuringmode,
                bitmapsnapoption,
            );
        }
    }
    pub unsafe fn DrawSvgGlyphRun<P2, P3>(
        &self,
        baselineorigin: windows_numerics::Vector2,
        glyphrun: *const DWRITE_GLYPH_RUN,
        defaultfillbrush: P2,
        svgglyphstyle: P3,
        colorpaletteindex: u32,
        measuringmode: DWRITE_MEASURING_MODE,
    ) where
        P2: windows_core::Param<ID2D1Brush>,
        P3: windows_core::Param<ID2D1SvgGlyphStyle>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawSvgGlyphRun)(
                windows_core::Interface::as_raw(self),
                baselineorigin,
                glyphrun,
                defaultfillbrush.param().abi(),
                svgglyphstyle.param().abi(),
                colorpaletteindex,
                measuringmode,
            );
        }
    }
    pub unsafe fn GetColorBitmapGlyphImage<P2>(
        &self,
        glyphimageformat: DWRITE_GLYPH_IMAGE_FORMATS,
        glyphorigin: windows_numerics::Vector2,
        fontface: P2,
        fontemsize: f32,
        glyphindex: u16,
        issideways: bool,
        worldtransform: Option<*const windows_numerics::Matrix3x2>,
        dpix: f32,
        dpiy: f32,
        glyphtransform: *mut windows_numerics::Matrix3x2,
        glyphimage: *mut Option<ID2D1Image>,
    ) -> windows_core::HRESULT
    where
        P2: windows_core::Param<IDWriteFontFace>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).GetColorBitmapGlyphImage)(
                windows_core::Interface::as_raw(self),
                glyphimageformat,
                glyphorigin,
                fontface.param().abi(),
                fontemsize,
                glyphindex,
                issideways.into(),
                worldtransform.unwrap_or(core::mem::zeroed()) as _,
                dpix,
                dpiy,
                glyphtransform as _,
                core::mem::transmute(glyphimage),
            )
        }
    }
    pub unsafe fn GetSvgGlyphImage<P1, P6, P7>(
        &self,
        glyphorigin: windows_numerics::Vector2,
        fontface: P1,
        fontemsize: f32,
        glyphindex: u16,
        issideways: bool,
        worldtransform: Option<*const windows_numerics::Matrix3x2>,
        defaultfillbrush: P6,
        svgglyphstyle: P7,
        colorpaletteindex: u32,
        glyphtransform: *mut windows_numerics::Matrix3x2,
        glyphimage: *mut Option<ID2D1CommandList>,
    ) -> windows_core::HRESULT
    where
        P1: windows_core::Param<IDWriteFontFace>,
        P6: windows_core::Param<ID2D1Brush>,
        P7: windows_core::Param<ID2D1SvgGlyphStyle>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).GetSvgGlyphImage)(
                windows_core::Interface::as_raw(self),
                glyphorigin,
                fontface.param().abi(),
                fontemsize,
                glyphindex,
                issideways.into(),
                worldtransform.unwrap_or(core::mem::zeroed()) as _,
                defaultfillbrush.param().abi(),
                svgglyphstyle.param().abi(),
                colorpaletteindex,
                glyphtransform as _,
                core::mem::transmute(glyphimage),
            )
        }
    }
}
#[repr(C)]
pub struct ID2D1DeviceContext4_Vtbl {
    pub base__: ID2D1DeviceContext3_Vtbl,
    pub CreateSvgGlyphStyle: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub DrawText: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const u16,
        u32,
        *mut core::ffi::c_void,
        *const D2D_RECT_F,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        u32,
        D2D1_DRAW_TEXT_OPTIONS,
        DWRITE_MEASURING_MODE,
    ),
    pub DrawTextLayout: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        u32,
        D2D1_DRAW_TEXT_OPTIONS,
    ),
    pub DrawColorBitmapGlyphRun: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_GLYPH_IMAGE_FORMATS,
        windows_numerics::Vector2,
        *const DWRITE_GLYPH_RUN,
        DWRITE_MEASURING_MODE,
        D2D1_COLOR_BITMAP_GLYPH_SNAP_OPTION,
    ),
    pub DrawSvgGlyphRun: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
        *const DWRITE_GLYPH_RUN,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        u32,
        DWRITE_MEASURING_MODE,
    ),
    pub GetColorBitmapGlyphImage: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_GLYPH_IMAGE_FORMATS,
        windows_numerics::Vector2,
        *mut core::ffi::c_void,
        f32,
        u16,
        windows_core::BOOL,
        *const windows_numerics::Matrix3x2,
        f32,
        f32,
        *mut windows_numerics::Matrix3x2,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetSvgGlyphImage: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
        *mut core::ffi::c_void,
        f32,
        u16,
        windows_core::BOOL,
        *const windows_numerics::Matrix3x2,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        u32,
        *mut windows_numerics::Matrix3x2,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
impl ID2D1DeviceContext5 {
    pub unsafe fn CreateColorContextFromDxgiColorSpace(
        &self,
        colorspace: DXGI_COLOR_SPACE_TYPE,
    ) -> windows_core::Result<ID2D1ColorContext1> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateColorContextFromDxgiColorSpace)(
                windows_core::Interface::as_raw(self),
                colorspace,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ID2D1DeviceContext5_Vtbl {
    pub base__: ID2D1DeviceContext4_Vtbl,
    CreateSvgDocument: usize,
    DrawSvgDocument: usize,
    pub CreateColorContextFromDxgiColorSpace: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DXGI_COLOR_SPACE_TYPE,
        *mut *mut core::ffi::c_void,
    )
        -> windows_core::HRESULT,
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
impl ID2D1DeviceContext6 {
    pub unsafe fn BlendImage<P0>(
        &self,
        image: P0,
        blendmode: D2D1_BLEND_MODE,
        targetoffset: Option<*const windows_numerics::Vector2>,
        imagerectangle: Option<*const D2D_RECT_F>,
        interpolationmode: D2D1_INTERPOLATION_MODE,
    ) where
        P0: windows_core::Param<ID2D1Image>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).BlendImage)(
                windows_core::Interface::as_raw(self),
                image.param().abi(),
                blendmode,
                targetoffset.unwrap_or(core::mem::zeroed()) as _,
                imagerectangle.unwrap_or(core::mem::zeroed()) as _,
                interpolationmode,
            );
        }
    }
}
#[repr(C)]
pub struct ID2D1DeviceContext6_Vtbl {
    pub base__: ID2D1DeviceContext5_Vtbl,
    pub BlendImage: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        D2D1_BLEND_MODE,
        *const windows_numerics::Vector2,
        *const D2D_RECT_F,
        D2D1_INTERPOLATION_MODE,
    ),
}
impl windows_core::RuntimeName for ID2D1DeviceContext6 {}
windows_core::imp::define_interface!(
    ID2D1Effect,
    ID2D1Effect_Vtbl,
    0x28211a43_7d89_476f_8181_2d6159b220ad
);
impl core::ops::Deref for ID2D1Effect {
    type Target = ID2D1Properties;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1Effect, windows_core::IUnknown, ID2D1Properties);
impl ID2D1Effect {
    pub unsafe fn SetInput<P1>(&self, index: u32, input: P1, invalidate: bool)
    where
        P1: windows_core::Param<ID2D1Image>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetInput)(
                windows_core::Interface::as_raw(self),
                index,
                input.param().abi(),
                invalidate.into(),
            );
        }
    }
    pub unsafe fn GetOutput(&self) -> windows_core::Result<ID2D1Image> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetOutput)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            );
            windows_core::Type::from_abi(result__)
        }
    }
}
#[repr(C)]
pub struct ID2D1Effect_Vtbl {
    pub base__: ID2D1Properties_Vtbl,
    pub SetInput: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut core::ffi::c_void,
        windows_core::BOOL,
    ),
    SetInputCount: usize,
    GetInput: usize,
    GetInputCount: usize,
    pub GetOutput: unsafe extern "system" fn(*mut core::ffi::c_void, *mut *mut core::ffi::c_void),
}
impl windows_core::RuntimeName for ID2D1Effect {}
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
    pub unsafe fn CreateDevice<P0>(&self, dxgidevice: P0) -> windows_core::Result<ID2D1Device>
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
    pub CreateDevice: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
impl ID2D1Factory2 {
    pub unsafe fn CreateDevice<P0>(&self, dxgidevice: P0) -> windows_core::Result<ID2D1Device1>
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
pub struct ID2D1Factory2_Vtbl {
    pub base__: ID2D1Factory1_Vtbl,
    pub CreateDevice: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
impl ID2D1Factory3 {
    pub unsafe fn CreateDevice<P0>(&self, dxgidevice: P0) -> windows_core::Result<ID2D1Device2>
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
pub struct ID2D1Factory3_Vtbl {
    pub base__: ID2D1Factory2_Vtbl,
    pub CreateDevice: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
impl ID2D1Factory4 {
    pub unsafe fn CreateDevice<P0>(&self, dxgidevice: P0) -> windows_core::Result<ID2D1Device3>
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
pub struct ID2D1Factory4_Vtbl {
    pub base__: ID2D1Factory3_Vtbl,
    pub CreateDevice: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
impl ID2D1Factory5 {
    pub unsafe fn CreateDevice<P0>(&self, dxgidevice: P0) -> windows_core::Result<ID2D1Device4>
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
pub struct ID2D1Factory5_Vtbl {
    pub base__: ID2D1Factory4_Vtbl,
    pub CreateDevice: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
impl ID2D1Factory6 {
    pub unsafe fn CreateDevice<P0>(&self, dxgidevice: P0) -> windows_core::Result<ID2D1Device5>
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
pub struct ID2D1Factory6_Vtbl {
    pub base__: ID2D1Factory5_Vtbl,
    pub CreateDevice: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
    ID2D1GdiMetafile,
    ID2D1GdiMetafile_Vtbl,
    0x2f543dc3_cfc1_4211_864f_cfd91c6f3395
);
impl core::ops::Deref for ID2D1GdiMetafile {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1GdiMetafile, windows_core::IUnknown, ID2D1Resource);
#[repr(C)]
pub struct ID2D1GdiMetafile_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    Stream: usize,
    GetBounds: usize,
}
impl windows_core::RuntimeName for ID2D1GdiMetafile {}
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
impl ID2D1GeometrySink {
    pub unsafe fn AddLine(&self, point: windows_numerics::Vector2) {
        unsafe {
            (windows_core::Interface::vtable(self).AddLine)(
                windows_core::Interface::as_raw(self),
                point,
            );
        }
    }
    pub unsafe fn AddBezier(&self, bezier: *const D2D1_BEZIER_SEGMENT) {
        unsafe {
            (windows_core::Interface::vtable(self).AddBezier)(
                windows_core::Interface::as_raw(self),
                bezier,
            );
        }
    }
}
#[repr(C)]
pub struct ID2D1GeometrySink_Vtbl {
    pub base__: ID2D1SimplifiedGeometrySink_Vtbl,
    pub AddLine: unsafe extern "system" fn(*mut core::ffi::c_void, windows_numerics::Vector2),
    pub AddBezier: unsafe extern "system" fn(*mut core::ffi::c_void, *const D2D1_BEZIER_SEGMENT),
    AddQuadraticBezier: usize,
    AddQuadraticBeziers: usize,
    AddArc: usize,
}
impl windows_core::RuntimeName for ID2D1GeometrySink {}
windows_core::imp::define_interface!(
    ID2D1GradientMesh,
    ID2D1GradientMesh_Vtbl,
    0xf292e401_c050_4cde_83d7_04962d3b23c2
);
impl core::ops::Deref for ID2D1GradientMesh {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1GradientMesh, windows_core::IUnknown, ID2D1Resource);
#[repr(C)]
pub struct ID2D1GradientMesh_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    GetPatchCount: usize,
    GetPatches: usize,
}
impl windows_core::RuntimeName for ID2D1GradientMesh {}
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
    ID2D1ImageSource,
    ID2D1ImageSource_Vtbl,
    0xc9b664e5_74a1_4378_9ac2_eefc37a3f4d8
);
impl core::ops::Deref for ID2D1ImageSource {
    type Target = ID2D1Image;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1ImageSource,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Image
);
#[repr(C)]
pub struct ID2D1ImageSource_Vtbl {
    pub base__: ID2D1Image_Vtbl,
    OfferResources: usize,
    TryReclaimResources: usize,
}
impl windows_core::RuntimeName for ID2D1ImageSource {}
windows_core::imp::define_interface!(
    ID2D1ImageSourceFromWic,
    ID2D1ImageSourceFromWic_Vtbl,
    0x77395441_1c8f_4555_8683_f50dab0fe792
);
impl core::ops::Deref for ID2D1ImageSourceFromWic {
    type Target = ID2D1ImageSource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1ImageSourceFromWic,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Image,
    ID2D1ImageSource
);
#[repr(C)]
pub struct ID2D1ImageSourceFromWic_Vtbl {
    pub base__: ID2D1ImageSource_Vtbl,
    EnsureCached: usize,
    TrimCache: usize,
    GetSource: usize,
}
impl windows_core::RuntimeName for ID2D1ImageSourceFromWic {}
windows_core::imp::define_interface!(
    ID2D1Ink,
    ID2D1Ink_Vtbl,
    0xb499923b_7029_478f_a8b3_432c7c5f5312
);
impl core::ops::Deref for ID2D1Ink {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1Ink, windows_core::IUnknown, ID2D1Resource);
#[repr(C)]
pub struct ID2D1Ink_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    SetStartPoint: usize,
    GetStartPoint: usize,
    AddSegments: usize,
    RemoveSegmentsAtEnd: usize,
    SetSegments: usize,
    SetSegmentAtEnd: usize,
    GetSegmentCount: usize,
    GetSegments: usize,
    StreamAsGeometry: usize,
    GetBounds: usize,
}
impl windows_core::RuntimeName for ID2D1Ink {}
windows_core::imp::define_interface!(
    ID2D1InkStyle,
    ID2D1InkStyle_Vtbl,
    0xbae8b344_23fc_4071_8cb5_d05d6f073848
);
impl core::ops::Deref for ID2D1InkStyle {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1InkStyle, windows_core::IUnknown, ID2D1Resource);
#[repr(C)]
pub struct ID2D1InkStyle_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    SetNibTransform: usize,
    GetNibTransform: usize,
    SetNibShape: usize,
    GetNibShape: usize,
}
impl windows_core::RuntimeName for ID2D1InkStyle {}
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
    ID2D1LookupTable3D,
    ID2D1LookupTable3D_Vtbl,
    0x53dd9855_a3b0_4d5b_82e1_26e25c5e5797
);
impl core::ops::Deref for ID2D1LookupTable3D {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1LookupTable3D, windows_core::IUnknown, ID2D1Resource);
#[repr(C)]
pub struct ID2D1LookupTable3D_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
}
impl windows_core::RuntimeName for ID2D1LookupTable3D {}
windows_core::imp::define_interface!(
    ID2D1Multithread,
    ID2D1Multithread_Vtbl,
    0x31e6e7bc_e0ff_4d46_8c64_a0a8c41c15d3
);
windows_core::imp::interface_hierarchy!(ID2D1Multithread, windows_core::IUnknown);
impl ID2D1Multithread {
    pub unsafe fn Enter(&self) {
        unsafe {
            (windows_core::Interface::vtable(self).Enter)(windows_core::Interface::as_raw(self));
        }
    }
    pub unsafe fn Leave(&self) {
        unsafe {
            (windows_core::Interface::vtable(self).Leave)(windows_core::Interface::as_raw(self));
        }
    }
}
#[repr(C)]
pub struct ID2D1Multithread_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetMultithreadProtected: usize,
    pub Enter: unsafe extern "system" fn(*mut core::ffi::c_void),
    pub Leave: unsafe extern "system" fn(*mut core::ffi::c_void),
}
impl windows_core::RuntimeName for ID2D1Multithread {}
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
    ID2D1Properties,
    ID2D1Properties_Vtbl,
    0x483473d7_cd46_4f9d_9d3a_3112aa80159d
);
windows_core::imp::interface_hierarchy!(ID2D1Properties, windows_core::IUnknown);
impl ID2D1Properties {
    pub unsafe fn SetValue(
        &self,
        index: u32,
        r#type: D2D1_PROPERTY_TYPE,
        data: &[u8],
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetValue)(
                windows_core::Interface::as_raw(self),
                index,
                r#type,
                data.as_ptr(),
                data.len().try_into().unwrap(),
            )
        }
    }
}
#[repr(C)]
pub struct ID2D1Properties_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetPropertyCount: usize,
    GetPropertyName: usize,
    GetPropertyNameLength: usize,
    GetType: usize,
    GetPropertyIndex: usize,
    SetValueByName: usize,
    pub SetValue: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        D2D1_PROPERTY_TYPE,
        *const u8,
        u32,
    ) -> windows_core::HRESULT,
    GetValueByName: usize,
    GetValue: usize,
    GetValueSize: usize,
    GetSubProperties: usize,
}
impl windows_core::RuntimeName for ID2D1Properties {}
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
#[repr(C)]
pub struct ID2D1RadialGradientBrush_Vtbl {
    pub base__: ID2D1Brush_Vtbl,
    SetCenter: usize,
    SetGradientOriginOffset: usize,
    SetRadiusX: usize,
    SetRadiusY: usize,
    GetCenter: usize,
    GetGradientOriginOffset: usize,
    GetRadiusX: usize,
    GetRadiusY: usize,
    GetGradientStopCollection: usize,
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
    pub unsafe fn CreateGradientStopCollection(
        &self,
        gradientstops: &[D2D1_GRADIENT_STOP],
        colorinterpolationgamma: D2D1_GAMMA,
        extendmode: D2D1_EXTEND_MODE,
    ) -> windows_core::Result<ID2D1GradientStopCollection> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateGradientStopCollection)(
                windows_core::Interface::as_raw(self),
                gradientstops.as_ptr(),
                gradientstops.len().try_into().unwrap(),
                colorinterpolationgamma,
                extendmode,
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
    pub unsafe fn DrawText<P2, P4>(
        &self,
        string: &[u16],
        textformat: P2,
        layoutrect: *const D2D_RECT_F,
        defaultfillbrush: P4,
        options: D2D1_DRAW_TEXT_OPTIONS,
        measuringmode: DWRITE_MEASURING_MODE,
    ) where
        P2: windows_core::Param<IDWriteTextFormat>,
        P4: windows_core::Param<ID2D1Brush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawText)(
                windows_core::Interface::as_raw(self),
                string.as_ptr(),
                string.len().try_into().unwrap(),
                textformat.param().abi(),
                layoutrect,
                defaultfillbrush.param().abi(),
                options,
                measuringmode,
            );
        }
    }
    pub unsafe fn DrawTextLayout<P1, P2>(
        &self,
        origin: windows_numerics::Vector2,
        textlayout: P1,
        defaultfillbrush: P2,
        options: D2D1_DRAW_TEXT_OPTIONS,
    ) where
        P1: windows_core::Param<IDWriteTextLayout>,
        P2: windows_core::Param<ID2D1Brush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).DrawTextLayout)(
                windows_core::Interface::as_raw(self),
                origin,
                textlayout.param().abi(),
                defaultfillbrush.param().abi(),
                options,
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
    pub CreateGradientStopCollection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const D2D1_GRADIENT_STOP,
        u32,
        D2D1_GAMMA,
        D2D1_EXTEND_MODE,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
    pub DrawText: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const u16,
        u32,
        *mut core::ffi::c_void,
        *const D2D_RECT_F,
        *mut core::ffi::c_void,
        D2D1_DRAW_TEXT_OPTIONS,
        DWRITE_MEASURING_MODE,
    ),
    pub DrawTextLayout: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        D2D1_DRAW_TEXT_OPTIONS,
    ),
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
    GetTextRenderingParams: usize,
    SetTags: usize,
    GetTags: usize,
    PushLayer: usize,
    PopLayer: usize,
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
    ID2D1SvgGlyphStyle,
    ID2D1SvgGlyphStyle_Vtbl,
    0xaf671749_d241_4db8_8e41_dcc2e5c1a438
);
impl core::ops::Deref for ID2D1SvgGlyphStyle {
    type Target = ID2D1Resource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ID2D1SvgGlyphStyle, windows_core::IUnknown, ID2D1Resource);
#[repr(C)]
pub struct ID2D1SvgGlyphStyle_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    SetFill: usize,
    GetFill: usize,
    SetStroke: usize,
    GetStrokeDashesCount: usize,
    GetStroke: usize,
}
impl windows_core::RuntimeName for ID2D1SvgGlyphStyle {}
windows_core::imp::define_interface!(
    ID2D1TransformedImageSource,
    ID2D1TransformedImageSource_Vtbl,
    0x7f1f79e5_2796_416c_8f55_700f911445e5
);
impl core::ops::Deref for ID2D1TransformedImageSource {
    type Target = ID2D1Image;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    ID2D1TransformedImageSource,
    windows_core::IUnknown,
    ID2D1Resource,
    ID2D1Image
);
#[repr(C)]
pub struct ID2D1TransformedImageSource_Vtbl {
    pub base__: ID2D1Image_Vtbl,
    GetSource: usize,
    GetProperties: usize,
}
impl windows_core::RuntimeName for ID2D1TransformedImageSource {}
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
    IDWriteTextFormat,
    IDWriteTextFormat_Vtbl,
    0x9c906818_31d7_4fd3_a151_7c5e225db55a
);
windows_core::imp::interface_hierarchy!(IDWriteTextFormat, windows_core::IUnknown);
#[repr(C)]
pub struct IDWriteTextFormat_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    SetTextAlignment: usize,
    SetParagraphAlignment: usize,
    SetWordWrapping: usize,
    SetReadingDirection: usize,
    SetFlowDirection: usize,
    SetIncrementalTabStop: usize,
    SetTrimming: usize,
    SetLineSpacing: usize,
    GetTextAlignment: usize,
    GetParagraphAlignment: usize,
    GetWordWrapping: usize,
    GetReadingDirection: usize,
    GetFlowDirection: usize,
    GetIncrementalTabStop: usize,
    GetTrimming: usize,
    GetLineSpacing: usize,
    GetFontCollection: usize,
    GetFontFamilyNameLength: usize,
    GetFontFamilyName: usize,
    GetFontWeight: usize,
    GetFontStyle: usize,
    GetFontStretch: usize,
    GetFontSize: usize,
    GetLocaleNameLength: usize,
    GetLocaleName: usize,
}
impl windows_core::RuntimeName for IDWriteTextFormat {}
windows_core::imp::define_interface!(
    IDWriteTextLayout,
    IDWriteTextLayout_Vtbl,
    0x53737037_6d14_410b_9bfe_0b182bb70961
);
impl core::ops::Deref for IDWriteTextLayout {
    type Target = IDWriteTextFormat;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteTextLayout,
    windows_core::IUnknown,
    IDWriteTextFormat
);
#[repr(C)]
pub struct IDWriteTextLayout_Vtbl {
    pub base__: IDWriteTextFormat_Vtbl,
    SetMaxWidth: usize,
    SetMaxHeight: usize,
    SetFontCollection: usize,
    SetFontFamilyName: usize,
    SetFontWeight: usize,
    SetFontStyle: usize,
    SetFontStretch: usize,
    SetFontSize: usize,
    SetUnderline: usize,
    SetStrikethrough: usize,
    SetDrawingEffect: usize,
    SetInlineObject: usize,
    SetTypography: usize,
    SetLocaleName: usize,
    GetMaxWidth: usize,
    GetMaxHeight: usize,
    GetFontCollection: usize,
    GetFontFamilyNameLength: usize,
    GetFontFamilyName: usize,
    GetFontWeight: usize,
    GetFontStyle: usize,
    GetFontStretch: usize,
    GetFontSize: usize,
    GetUnderline: usize,
    GetStrikethrough: usize,
    GetDrawingEffect: usize,
    GetInlineObject: usize,
    GetTypography: usize,
    GetLocaleNameLength: usize,
    GetLocaleName: usize,
    Draw: usize,
    GetLineMetrics: usize,
    GetMetrics: usize,
    GetOverhangMetrics: usize,
    GetClusterMetrics: usize,
    DetermineMinWidth: usize,
    HitTestPoint: usize,
    HitTestTextPosition: usize,
    HitTestTextRange: usize,
}
impl windows_core::RuntimeName for IDWriteTextLayout {}
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
}
#[repr(C)]
pub struct IDXGIDevice_Vtbl {
    pub base__: IDXGIObject_Vtbl,
    pub GetAdapter: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateSurface: usize,
    QueryResourceResidency: usize,
    SetGPUThreadPriority: usize,
    GetGPUThreadPriority: usize,
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
    IDXGIFactory,
    IDXGIFactory_Vtbl,
    0x7b7166ec_21c7_44ae_b21a_c9ae321ae369
);
impl core::ops::Deref for IDXGIFactory {
    type Target = IDXGIObject;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IDXGIFactory, windows_core::IUnknown, IDXGIObject);
#[repr(C)]
pub struct IDXGIFactory_Vtbl {
    pub base__: IDXGIObject_Vtbl,
    EnumAdapters: usize,
    MakeWindowAssociation: usize,
    GetWindowAssociation: usize,
    CreateSwapChain: usize,
    CreateSoftwareAdapter: usize,
}
impl windows_core::RuntimeName for IDXGIFactory {}
windows_core::imp::define_interface!(
    IDXGIFactory1,
    IDXGIFactory1_Vtbl,
    0x770aae78_f26f_4dba_a829_253c83d1b387
);
impl core::ops::Deref for IDXGIFactory1 {
    type Target = IDXGIFactory;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGIFactory1,
    windows_core::IUnknown,
    IDXGIObject,
    IDXGIFactory
);
#[repr(C)]
pub struct IDXGIFactory1_Vtbl {
    pub base__: IDXGIFactory_Vtbl,
    EnumAdapters1: usize,
    IsCurrent: usize,
}
impl windows_core::RuntimeName for IDXGIFactory1 {}
windows_core::imp::define_interface!(
    IDXGIFactory2,
    IDXGIFactory2_Vtbl,
    0x50c83a1c_e072_4c48_87b0_3630fa36a6d0
);
impl core::ops::Deref for IDXGIFactory2 {
    type Target = IDXGIFactory1;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGIFactory2,
    windows_core::IUnknown,
    IDXGIObject,
    IDXGIFactory,
    IDXGIFactory1
);
impl IDXGIFactory2 {
    pub unsafe fn CreateSwapChainForHwnd<P0, P4>(
        &self,
        pdevice: P0,
        hwnd: HWND,
        pdesc: *const DXGI_SWAP_CHAIN_DESC1,
        pfullscreendesc: Option<*const DXGI_SWAP_CHAIN_FULLSCREEN_DESC>,
        prestricttooutput: P4,
    ) -> windows_core::Result<IDXGISwapChain1>
    where
        P0: windows_core::Param<windows_core::IUnknown>,
        P4: windows_core::Param<IDXGIOutput>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateSwapChainForHwnd)(
                windows_core::Interface::as_raw(self),
                pdevice.param().abi(),
                hwnd,
                pdesc,
                pfullscreendesc.unwrap_or(core::mem::zeroed()) as _,
                prestricttooutput.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateSwapChainForComposition<P0, P2>(
        &self,
        pdevice: P0,
        pdesc: *const DXGI_SWAP_CHAIN_DESC1,
        prestricttooutput: P2,
    ) -> windows_core::Result<IDXGISwapChain1>
    where
        P0: windows_core::Param<windows_core::IUnknown>,
        P2: windows_core::Param<IDXGIOutput>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateSwapChainForComposition)(
                windows_core::Interface::as_raw(self),
                pdevice.param().abi(),
                pdesc,
                prestricttooutput.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDXGIFactory2_Vtbl {
    pub base__: IDXGIFactory1_Vtbl,
    IsWindowedStereoEnabled: usize,
    pub CreateSwapChainForHwnd: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        HWND,
        *const DXGI_SWAP_CHAIN_DESC1,
        *const DXGI_SWAP_CHAIN_FULLSCREEN_DESC,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateSwapChainForCoreWindow: usize,
    GetSharedResourceAdapterLuid: usize,
    RegisterStereoStatusWindow: usize,
    RegisterStereoStatusEvent: usize,
    UnregisterStereoStatus: usize,
    RegisterOcclusionStatusWindow: usize,
    RegisterOcclusionStatusEvent: usize,
    UnregisterOcclusionStatus: usize,
    pub CreateSwapChainForComposition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *const DXGI_SWAP_CHAIN_DESC1,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for IDXGIFactory2 {}
windows_core::imp::define_interface!(
    IDXGIObject,
    IDXGIObject_Vtbl,
    0xaec22fb8_76f3_4639_9be0_28eb43a67a2e
);
windows_core::imp::interface_hierarchy!(IDXGIObject, windows_core::IUnknown);
impl IDXGIObject {
    pub unsafe fn GetParent<T>(&self) -> windows_core::Result<T>
    where
        T: windows_core::Interface,
    {
        let mut result__ = core::ptr::null_mut();
        unsafe {
            (windows_core::Interface::vtable(self).GetParent)(
                windows_core::Interface::as_raw(self),
                &T::IID,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDXGIObject_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    SetPrivateData: usize,
    SetPrivateDataInterface: usize,
    GetPrivateData: usize,
    pub GetParent: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for IDXGIObject {}
windows_core::imp::define_interface!(
    IDXGIOutput,
    IDXGIOutput_Vtbl,
    0xae02eedb_c735_4690_8d52_5a8dc20213aa
);
impl core::ops::Deref for IDXGIOutput {
    type Target = IDXGIObject;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IDXGIOutput, windows_core::IUnknown, IDXGIObject);
#[repr(C)]
pub struct IDXGIOutput_Vtbl {
    pub base__: IDXGIObject_Vtbl,
    GetDesc: usize,
    GetDisplayModeList: usize,
    FindClosestMatchingMode: usize,
    WaitForVBlank: usize,
    TakeOwnership: usize,
    ReleaseOwnership: usize,
    GetGammaControlCapabilities: usize,
    SetGammaControl: usize,
    GetGammaControl: usize,
    SetDisplaySurface: usize,
    GetDisplaySurfaceData: usize,
    GetFrameStatistics: usize,
}
impl windows_core::RuntimeName for IDXGIOutput {}
windows_core::imp::define_interface!(
    IDXGIOutput1,
    IDXGIOutput1_Vtbl,
    0x00cddea8_939b_4b83_a340_a685226666cc
);
impl core::ops::Deref for IDXGIOutput1 {
    type Target = IDXGIOutput;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGIOutput1,
    windows_core::IUnknown,
    IDXGIObject,
    IDXGIOutput
);
impl IDXGIOutput1 {
    pub unsafe fn GetDisplayModeList1(
        &self,
        enumformat: DXGI_FORMAT,
        flags: u32,
        pnummodes: *mut u32,
        pdesc: Option<*mut DXGI_MODE_DESC1>,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetDisplayModeList1)(
                windows_core::Interface::as_raw(self),
                enumformat,
                flags,
                pnummodes as _,
                pdesc.unwrap_or(core::mem::zeroed()) as _,
            )
        }
    }
    pub unsafe fn FindClosestMatchingMode1<P2>(
        &self,
        pmodetomatch: *const DXGI_MODE_DESC1,
        pclosestmatch: *mut DXGI_MODE_DESC1,
        pconcerneddevice: P2,
    ) -> windows_core::HRESULT
    where
        P2: windows_core::Param<windows_core::IUnknown>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).FindClosestMatchingMode1)(
                windows_core::Interface::as_raw(self),
                pmodetomatch,
                pclosestmatch as _,
                pconcerneddevice.param().abi(),
            )
        }
    }
    pub unsafe fn GetDisplaySurfaceData1<P0>(&self, pdestination: P0) -> windows_core::HRESULT
    where
        P0: windows_core::Param<IDXGIResource>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).GetDisplaySurfaceData1)(
                windows_core::Interface::as_raw(self),
                pdestination.param().abi(),
            )
        }
    }
    pub unsafe fn DuplicateOutput<P0>(
        &self,
        pdevice: P0,
    ) -> windows_core::Result<IDXGIOutputDuplication>
    where
        P0: windows_core::Param<windows_core::IUnknown>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).DuplicateOutput)(
                windows_core::Interface::as_raw(self),
                pdevice.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDXGIOutput1_Vtbl {
    pub base__: IDXGIOutput_Vtbl,
    pub GetDisplayModeList1: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DXGI_FORMAT,
        u32,
        *mut u32,
        *mut DXGI_MODE_DESC1,
    ) -> windows_core::HRESULT,
    pub FindClosestMatchingMode1: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const DXGI_MODE_DESC1,
        *mut DXGI_MODE_DESC1,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetDisplaySurfaceData1: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub DuplicateOutput: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for IDXGIOutput1 {}
windows_core::imp::define_interface!(
    IDXGIOutput2,
    IDXGIOutput2_Vtbl,
    0x595e39d1_2724_4663_99b1_da969de28364
);
impl core::ops::Deref for IDXGIOutput2 {
    type Target = IDXGIOutput1;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGIOutput2,
    windows_core::IUnknown,
    IDXGIObject,
    IDXGIOutput,
    IDXGIOutput1
);
impl IDXGIOutput2 {
    pub unsafe fn SupportsOverlays(&self) -> windows_core::BOOL {
        unsafe {
            (windows_core::Interface::vtable(self).SupportsOverlays)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
}
#[repr(C)]
pub struct IDXGIOutput2_Vtbl {
    pub base__: IDXGIOutput1_Vtbl,
    pub SupportsOverlays: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::BOOL,
}
impl windows_core::RuntimeName for IDXGIOutput2 {}
windows_core::imp::define_interface!(
    IDXGIOutput3,
    IDXGIOutput3_Vtbl,
    0x8a6bb301_7e7e_41f4_a8e0_5b32f7f99b18
);
impl core::ops::Deref for IDXGIOutput3 {
    type Target = IDXGIOutput2;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGIOutput3,
    windows_core::IUnknown,
    IDXGIObject,
    IDXGIOutput,
    IDXGIOutput1,
    IDXGIOutput2
);
impl IDXGIOutput3 {
    pub unsafe fn CheckOverlaySupport<P1>(
        &self,
        enumformat: DXGI_FORMAT,
        pconcerneddevice: P1,
    ) -> windows_core::Result<u32>
    where
        P1: windows_core::Param<windows_core::IUnknown>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CheckOverlaySupport)(
                windows_core::Interface::as_raw(self),
                enumformat,
                pconcerneddevice.param().abi(),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IDXGIOutput3_Vtbl {
    pub base__: IDXGIOutput2_Vtbl,
    pub CheckOverlaySupport: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DXGI_FORMAT,
        *mut core::ffi::c_void,
        *mut u32,
    ) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for IDXGIOutput3 {}
windows_core::imp::define_interface!(
    IDXGIOutput4,
    IDXGIOutput4_Vtbl,
    0xdc7dca35_2196_414d_9f53_617884032a60
);
impl core::ops::Deref for IDXGIOutput4 {
    type Target = IDXGIOutput3;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGIOutput4,
    windows_core::IUnknown,
    IDXGIObject,
    IDXGIOutput,
    IDXGIOutput1,
    IDXGIOutput2,
    IDXGIOutput3
);
impl IDXGIOutput4 {
    pub unsafe fn CheckOverlayColorSpaceSupport<P2>(
        &self,
        format: DXGI_FORMAT,
        colorspace: DXGI_COLOR_SPACE_TYPE,
        pconcerneddevice: P2,
    ) -> windows_core::Result<u32>
    where
        P2: windows_core::Param<windows_core::IUnknown>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CheckOverlayColorSpaceSupport)(
                windows_core::Interface::as_raw(self),
                format,
                colorspace,
                pconcerneddevice.param().abi(),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IDXGIOutput4_Vtbl {
    pub base__: IDXGIOutput3_Vtbl,
    pub CheckOverlayColorSpaceSupport: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DXGI_FORMAT,
        DXGI_COLOR_SPACE_TYPE,
        *mut core::ffi::c_void,
        *mut u32,
    ) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for IDXGIOutput4 {}
windows_core::imp::define_interface!(
    IDXGIOutput5,
    IDXGIOutput5_Vtbl,
    0x80a07424_ab52_42eb_833c_0c42fd282d98
);
impl core::ops::Deref for IDXGIOutput5 {
    type Target = IDXGIOutput4;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGIOutput5,
    windows_core::IUnknown,
    IDXGIObject,
    IDXGIOutput,
    IDXGIOutput1,
    IDXGIOutput2,
    IDXGIOutput3,
    IDXGIOutput4
);
impl IDXGIOutput5 {
    pub unsafe fn DuplicateOutput1<P0>(
        &self,
        pdevice: P0,
        flags: u32,
        psupportedformats: &[DXGI_FORMAT],
    ) -> windows_core::Result<IDXGIOutputDuplication>
    where
        P0: windows_core::Param<windows_core::IUnknown>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).DuplicateOutput1)(
                windows_core::Interface::as_raw(self),
                pdevice.param().abi(),
                flags,
                psupportedformats.len().try_into().unwrap(),
                psupportedformats.as_ptr(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDXGIOutput5_Vtbl {
    pub base__: IDXGIOutput4_Vtbl,
    pub DuplicateOutput1: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        u32,
        u32,
        *const DXGI_FORMAT,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for IDXGIOutput5 {}
windows_core::imp::define_interface!(
    IDXGIOutput6,
    IDXGIOutput6_Vtbl,
    0x068346e8_aaec_4b84_add7_137f513f77a1
);
impl core::ops::Deref for IDXGIOutput6 {
    type Target = IDXGIOutput5;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGIOutput6,
    windows_core::IUnknown,
    IDXGIObject,
    IDXGIOutput,
    IDXGIOutput1,
    IDXGIOutput2,
    IDXGIOutput3,
    IDXGIOutput4,
    IDXGIOutput5
);
impl IDXGIOutput6 {
    pub unsafe fn GetDesc1(&self, pdesc: *mut DXGI_OUTPUT_DESC1) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetDesc1)(
                windows_core::Interface::as_raw(self),
                pdesc as _,
            )
        }
    }
}
#[repr(C)]
pub struct IDXGIOutput6_Vtbl {
    pub base__: IDXGIOutput5_Vtbl,
    pub GetDesc1: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut DXGI_OUTPUT_DESC1,
    ) -> windows_core::HRESULT,
    CheckHardwareCompositionSupport: usize,
}
impl windows_core::RuntimeName for IDXGIOutput6 {}
windows_core::imp::define_interface!(
    IDXGIOutputDuplication,
    IDXGIOutputDuplication_Vtbl,
    0x191cfac3_a341_470d_b26e_a864f428319c
);
impl core::ops::Deref for IDXGIOutputDuplication {
    type Target = IDXGIObject;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGIOutputDuplication,
    windows_core::IUnknown,
    IDXGIObject
);
#[repr(C)]
pub struct IDXGIOutputDuplication_Vtbl {
    pub base__: IDXGIObject_Vtbl,
    GetDesc: usize,
    AcquireNextFrame: usize,
    GetFrameDirtyRects: usize,
    GetFrameMoveRects: usize,
    GetFramePointerShape: usize,
    MapDesktopSurface: usize,
    UnMapDesktopSurface: usize,
    ReleaseFrame: usize,
}
impl windows_core::RuntimeName for IDXGIOutputDuplication {}
windows_core::imp::define_interface!(
    IDXGIResource,
    IDXGIResource_Vtbl,
    0x035f3ab4_482e_4e50_b41f_8a7f8bd8960b
);
impl core::ops::Deref for IDXGIResource {
    type Target = IDXGIDeviceSubObject;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGIResource,
    windows_core::IUnknown,
    IDXGIObject,
    IDXGIDeviceSubObject
);
#[repr(C)]
pub struct IDXGIResource_Vtbl {
    pub base__: IDXGIDeviceSubObject_Vtbl,
    GetSharedHandle: usize,
    GetUsage: usize,
    SetEvictionPriority: usize,
    GetEvictionPriority: usize,
}
impl windows_core::RuntimeName for IDXGIResource {}
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
windows_core::imp::define_interface!(
    IDXGISwapChain,
    IDXGISwapChain_Vtbl,
    0x310d36a0_d2e7_4c0a_aa04_6a9d23b8886a
);
impl core::ops::Deref for IDXGISwapChain {
    type Target = IDXGIDeviceSubObject;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGISwapChain,
    windows_core::IUnknown,
    IDXGIObject,
    IDXGIDeviceSubObject
);
impl IDXGISwapChain {
    pub unsafe fn Present(&self, syncinterval: u32, flags: u32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Present)(
                windows_core::Interface::as_raw(self),
                syncinterval,
                flags,
            )
        }
    }
    pub unsafe fn GetBuffer<T>(&self, buffer: u32) -> windows_core::Result<T>
    where
        T: windows_core::Interface,
    {
        let mut result__ = core::ptr::null_mut();
        unsafe {
            (windows_core::Interface::vtable(self).GetBuffer)(
                windows_core::Interface::as_raw(self),
                buffer,
                &T::IID,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn ResizeBuffers(
        &self,
        buffercount: u32,
        width: u32,
        height: u32,
        newformat: DXGI_FORMAT,
        swapchainflags: u32,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).ResizeBuffers)(
                windows_core::Interface::as_raw(self),
                buffercount,
                width,
                height,
                newformat,
                swapchainflags,
            )
        }
    }
}
#[repr(C)]
pub struct IDXGISwapChain_Vtbl {
    pub base__: IDXGIDeviceSubObject_Vtbl,
    pub Present:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32, u32) -> windows_core::HRESULT,
    pub GetBuffer: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    SetFullscreenState: usize,
    GetFullscreenState: usize,
    GetDesc: usize,
    pub ResizeBuffers: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        u32,
        u32,
        DXGI_FORMAT,
        u32,
    ) -> windows_core::HRESULT,
    ResizeTarget: usize,
    GetContainingOutput: usize,
    GetFrameStatistics: usize,
    GetLastPresentCount: usize,
}
impl windows_core::RuntimeName for IDXGISwapChain {}
windows_core::imp::define_interface!(
    IDXGISwapChain1,
    IDXGISwapChain1_Vtbl,
    0x790a45f7_0d42_4876_983a_0a55cfe6f4aa
);
impl core::ops::Deref for IDXGISwapChain1 {
    type Target = IDXGISwapChain;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGISwapChain1,
    windows_core::IUnknown,
    IDXGIObject,
    IDXGIDeviceSubObject,
    IDXGISwapChain
);
#[repr(C)]
pub struct IDXGISwapChain1_Vtbl {
    pub base__: IDXGISwapChain_Vtbl,
    GetDesc1: usize,
    GetFullscreenDesc: usize,
    GetHwnd: usize,
    GetCoreWindow: usize,
    Present1: usize,
    IsTemporaryMonoSupported: usize,
    GetRestrictToOutput: usize,
    SetBackgroundColor: usize,
    GetBackgroundColor: usize,
    SetRotation: usize,
    GetRotation: usize,
}
impl windows_core::RuntimeName for IDXGISwapChain1 {}
windows_core::imp::define_interface!(
    IDXGISwapChain2,
    IDXGISwapChain2_Vtbl,
    0xa8be2ac4_199f_4946_b331_79599fb98de7
);
impl core::ops::Deref for IDXGISwapChain2 {
    type Target = IDXGISwapChain1;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGISwapChain2,
    windows_core::IUnknown,
    IDXGIObject,
    IDXGIDeviceSubObject,
    IDXGISwapChain,
    IDXGISwapChain1
);
impl IDXGISwapChain2 {
    pub unsafe fn SetMaximumFrameLatency(&self, maxlatency: u32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetMaximumFrameLatency)(
                windows_core::Interface::as_raw(self),
                maxlatency,
            )
        }
    }
    pub unsafe fn GetFrameLatencyWaitableObject(&self) -> HANDLE {
        unsafe {
            (windows_core::Interface::vtable(self).GetFrameLatencyWaitableObject)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn SetMatrixTransform(
        &self,
        pmatrix: *const DXGI_MATRIX_3X2_F,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetMatrixTransform)(
                windows_core::Interface::as_raw(self),
                pmatrix,
            )
        }
    }
}
#[repr(C)]
pub struct IDXGISwapChain2_Vtbl {
    pub base__: IDXGISwapChain1_Vtbl,
    SetSourceSize: usize,
    GetSourceSize: usize,
    pub SetMaximumFrameLatency:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    GetMaximumFrameLatency: usize,
    pub GetFrameLatencyWaitableObject: unsafe extern "system" fn(*mut core::ffi::c_void) -> HANDLE,
    pub SetMatrixTransform: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const DXGI_MATRIX_3X2_F,
    ) -> windows_core::HRESULT,
    GetMatrixTransform: usize,
}
impl windows_core::RuntimeName for IDXGISwapChain2 {}
windows_core::imp::define_interface!(
    IDXGISwapChain3,
    IDXGISwapChain3_Vtbl,
    0x94d99bdb_f1f8_4ab0_b236_7da0170edab1
);
impl core::ops::Deref for IDXGISwapChain3 {
    type Target = IDXGISwapChain2;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDXGISwapChain3,
    windows_core::IUnknown,
    IDXGIObject,
    IDXGIDeviceSubObject,
    IDXGISwapChain,
    IDXGISwapChain1,
    IDXGISwapChain2
);
impl IDXGISwapChain3 {
    pub unsafe fn CheckColorSpaceSupport(
        &self,
        colorspace: DXGI_COLOR_SPACE_TYPE,
    ) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CheckColorSpaceSupport)(
                windows_core::Interface::as_raw(self),
                colorspace,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn SetColorSpace1(
        &self,
        colorspace: DXGI_COLOR_SPACE_TYPE,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetColorSpace1)(
                windows_core::Interface::as_raw(self),
                colorspace,
            )
        }
    }
}
#[repr(C)]
pub struct IDXGISwapChain3_Vtbl {
    pub base__: IDXGISwapChain2_Vtbl,
    GetCurrentBackBufferIndex: usize,
    pub CheckColorSpaceSupport: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DXGI_COLOR_SPACE_TYPE,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub SetColorSpace1: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DXGI_COLOR_SPACE_TYPE,
    ) -> windows_core::HRESULT,
    ResizeBuffers1: usize,
}
impl windows_core::RuntimeName for IDXGISwapChain3 {}
windows_core::imp::define_interface!(
    IWICBitmapDecoder,
    IWICBitmapDecoder_Vtbl,
    0x9edde9e7_8dee_47ea_99df_e6faf2ed44bf
);
windows_core::imp::interface_hierarchy!(IWICBitmapDecoder, windows_core::IUnknown);
impl IWICBitmapDecoder {
    pub unsafe fn GetFrame(&self, index: u32) -> windows_core::Result<IWICBitmapFrameDecode> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFrame)(
                windows_core::Interface::as_raw(self),
                index,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IWICBitmapDecoder_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    QueryCapability: usize,
    Initialize: usize,
    GetContainerFormat: usize,
    GetDecoderInfo: usize,
    CopyPalette: usize,
    GetMetadataQueryReader: usize,
    GetPreview: usize,
    GetColorContexts: usize,
    GetThumbnail: usize,
    GetFrameCount: usize,
    pub GetFrame: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
impl windows_core::RuntimeName for IWICBitmapDecoder {}
windows_core::imp::define_interface!(
    IWICBitmapFrameDecode,
    IWICBitmapFrameDecode_Vtbl,
    0x3b16811b_6a43_4ec9_a813_3d930c13b940
);
impl core::ops::Deref for IWICBitmapFrameDecode {
    type Target = IWICBitmapSource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IWICBitmapFrameDecode,
    windows_core::IUnknown,
    IWICBitmapSource
);
#[repr(C)]
pub struct IWICBitmapFrameDecode_Vtbl {
    pub base__: IWICBitmapSource_Vtbl,
    GetMetadataQueryReader: usize,
    GetColorContexts: usize,
    GetThumbnail: usize,
}
impl windows_core::RuntimeName for IWICBitmapFrameDecode {}
windows_core::imp::define_interface!(
    IWICBitmapSource,
    IWICBitmapSource_Vtbl,
    0x00000120_a8f2_4877_ba0a_fd2b6645fb94
);
windows_core::imp::interface_hierarchy!(IWICBitmapSource, windows_core::IUnknown);
#[repr(C)]
pub struct IWICBitmapSource_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetSize: usize,
    GetPixelFormat: usize,
    GetResolution: usize,
    CopyPalette: usize,
    CopyPixels: usize,
}
impl windows_core::RuntimeName for IWICBitmapSource {}
windows_core::imp::define_interface!(
    IWICFormatConverter,
    IWICFormatConverter_Vtbl,
    0x00000301_a8f2_4877_ba0a_fd2b6645fb94
);
impl core::ops::Deref for IWICFormatConverter {
    type Target = IWICBitmapSource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IWICFormatConverter,
    windows_core::IUnknown,
    IWICBitmapSource
);
impl IWICFormatConverter {
    pub unsafe fn Initialize<P0, P3>(
        &self,
        pisource: P0,
        dstformat: REFWICPixelFormatGUID,
        dither: WICBitmapDitherType,
        pipalette: P3,
        alphathresholdpercent: f64,
        palettetranslate: WICBitmapPaletteType,
    ) -> windows_core::HRESULT
    where
        P0: windows_core::Param<IWICBitmapSource>,
        P3: windows_core::Param<IWICPalette>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).Initialize)(
                windows_core::Interface::as_raw(self),
                pisource.param().abi(),
                dstformat,
                dither,
                pipalette.param().abi(),
                alphathresholdpercent,
                palettetranslate,
            )
        }
    }
}
#[repr(C)]
pub struct IWICFormatConverter_Vtbl {
    pub base__: IWICBitmapSource_Vtbl,
    pub Initialize: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        REFWICPixelFormatGUID,
        WICBitmapDitherType,
        *mut core::ffi::c_void,
        f64,
        WICBitmapPaletteType,
    ) -> windows_core::HRESULT,
    CanConvert: usize,
}
impl windows_core::RuntimeName for IWICFormatConverter {}
windows_core::imp::define_interface!(
    IWICImagingFactory,
    IWICImagingFactory_Vtbl,
    0xec5ec8a9_c395_4314_9c77_54d7a935ff70
);
windows_core::imp::interface_hierarchy!(IWICImagingFactory, windows_core::IUnknown);
impl IWICImagingFactory {
    pub unsafe fn CreateDecoderFromFilename<P0>(
        &self,
        wzfilename: P0,
        pguidvendor: *const windows_core::GUID,
        dwdesiredaccess: u32,
        metadataoptions: WICDecodeOptions,
    ) -> windows_core::Result<IWICBitmapDecoder>
    where
        P0: windows_core::Param<windows_core::PCWSTR>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateDecoderFromFilename)(
                windows_core::Interface::as_raw(self),
                wzfilename.param().abi(),
                pguidvendor,
                dwdesiredaccess,
                metadataoptions,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateFormatConverter(&self) -> windows_core::Result<IWICFormatConverter> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateFormatConverter)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IWICImagingFactory_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub CreateDecoderFromFilename: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::PCWSTR,
        *const windows_core::GUID,
        u32,
        WICDecodeOptions,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateDecoderFromStream: usize,
    CreateDecoderFromFileHandle: usize,
    CreateComponentInfo: usize,
    CreateDecoder: usize,
    CreateEncoder: usize,
    CreatePalette: usize,
    pub CreateFormatConverter: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateBitmapScaler: usize,
    CreateBitmapClipper: usize,
    CreateBitmapFlipRotator: usize,
    CreateStream: usize,
    CreateColorContext: usize,
    CreateColorTransformer: usize,
    CreateBitmap: usize,
    CreateBitmapFromSource: usize,
    CreateBitmapFromSourceRect: usize,
    CreateBitmapFromMemory: usize,
    CreateBitmapFromHBITMAP: usize,
    CreateBitmapFromHICON: usize,
    CreateComponentEnumerator: usize,
    CreateFastMetadataEncoderFromDecoder: usize,
    CreateFastMetadataEncoderFromFrameDecode: usize,
    CreateQueryWriter: usize,
    CreateQueryWriterFromReader: usize,
}
impl windows_core::RuntimeName for IWICImagingFactory {}
windows_core::imp::define_interface!(
    IWICPalette,
    IWICPalette_Vtbl,
    0x00000040_a8f2_4877_ba0a_fd2b6645fb94
);
windows_core::imp::interface_hierarchy!(IWICPalette, windows_core::IUnknown);
#[repr(C)]
pub struct IWICPalette_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    InitializePredefined: usize,
    InitializeCustom: usize,
    InitializeFromBitmap: usize,
    InitializeFromPalette: usize,
    GetType: usize,
    GetColorCount: usize,
    GetColors: usize,
    IsBlackWhite: usize,
    IsGrayscale: usize,
    HasAlpha: usize,
}
impl windows_core::RuntimeName for IWICPalette {}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
pub type REFWICPixelFormatGUID = *const windows_core::GUID;
pub type WICBitmapDitherType = i32;
pub const WICBitmapDitherTypeNone: WICBitmapDitherType = 0;
pub type WICBitmapPaletteType = i32;
pub const WICBitmapPaletteTypeMedianCut: WICBitmapPaletteType = 1;
pub const WICDecodeMetadataCacheOnDemand: WICDecodeOptions = 0;
pub type WICDecodeOptions = i32;
