windows_core::link!("dwrite.dll" "system" fn DWriteCreateFactory(factorytype : DWRITE_FACTORY_TYPE, iid : *const windows_core::GUID, factory : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct D3DCOLORVALUE {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
pub type DWRITE_AUTOMATIC_FONT_AXES = u32;
pub const DWRITE_AUTOMATIC_FONT_AXES_OPTICAL_SIZE: DWRITE_AUTOMATIC_FONT_AXES = 1;
pub type DWRITE_COLOR_F = D3DCOLORVALUE;
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct DWRITE_COLOR_GLYPH_RUN {
    pub glyphRun: DWRITE_GLYPH_RUN,
    pub glyphRunDescription: *mut DWRITE_GLYPH_RUN_DESCRIPTION,
    pub baselineOriginX: f32,
    pub baselineOriginY: f32,
    pub runColor: DWRITE_COLOR_F,
    pub paletteIndex: u16,
}
impl Default for DWRITE_COLOR_GLYPH_RUN {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub type DWRITE_CONTAINER_TYPE = i32;
pub type DWRITE_FACTORY_TYPE = i32;
pub const DWRITE_FACTORY_TYPE_SHARED: DWRITE_FACTORY_TYPE = 0;
pub type DWRITE_FLOW_DIRECTION = i32;
pub type DWRITE_FONT_AXIS_TAG = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DWRITE_FONT_AXIS_VALUE {
    pub axisTag: DWRITE_FONT_AXIS_TAG,
    pub value: f32,
}
pub type DWRITE_FONT_FAMILY_MODEL = i32;
pub type DWRITE_FONT_LINE_GAP_USAGE = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DWRITE_FONT_METRICS {
    pub designUnitsPerEm: u16,
    pub ascent: u16,
    pub descent: u16,
    pub lineGap: i16,
    pub capHeight: u16,
    pub xHeight: u16,
    pub underlinePosition: i16,
    pub underlineThickness: u16,
    pub strikethroughPosition: i16,
    pub strikethroughThickness: u16,
}
pub type DWRITE_FONT_SIMULATIONS = u32;
pub type DWRITE_FONT_STRETCH = i32;
pub const DWRITE_FONT_STRETCH_CONDENSED: DWRITE_FONT_STRETCH = 3;
pub const DWRITE_FONT_STRETCH_EXPANDED: DWRITE_FONT_STRETCH = 7;
pub const DWRITE_FONT_STRETCH_EXTRA_CONDENSED: DWRITE_FONT_STRETCH = 2;
pub const DWRITE_FONT_STRETCH_EXTRA_EXPANDED: DWRITE_FONT_STRETCH = 8;
pub const DWRITE_FONT_STRETCH_MEDIUM: DWRITE_FONT_STRETCH = 5;
pub const DWRITE_FONT_STRETCH_NORMAL: DWRITE_FONT_STRETCH = 5;
pub const DWRITE_FONT_STRETCH_SEMI_CONDENSED: DWRITE_FONT_STRETCH = 4;
pub const DWRITE_FONT_STRETCH_SEMI_EXPANDED: DWRITE_FONT_STRETCH = 6;
pub const DWRITE_FONT_STRETCH_ULTRA_CONDENSED: DWRITE_FONT_STRETCH = 1;
pub const DWRITE_FONT_STRETCH_ULTRA_EXPANDED: DWRITE_FONT_STRETCH = 9;
pub const DWRITE_FONT_STRETCH_UNDEFINED: DWRITE_FONT_STRETCH = 0;
pub type DWRITE_FONT_STYLE = i32;
pub const DWRITE_FONT_STYLE_ITALIC: DWRITE_FONT_STYLE = 2;
pub const DWRITE_FONT_STYLE_NORMAL: DWRITE_FONT_STYLE = 0;
pub const DWRITE_FONT_STYLE_OBLIQUE: DWRITE_FONT_STYLE = 1;
pub type DWRITE_FONT_WEIGHT = i32;
pub const DWRITE_FONT_WEIGHT_BLACK: DWRITE_FONT_WEIGHT = 900;
pub const DWRITE_FONT_WEIGHT_BOLD: DWRITE_FONT_WEIGHT = 700;
pub const DWRITE_FONT_WEIGHT_DEMI_BOLD: DWRITE_FONT_WEIGHT = 600;
pub const DWRITE_FONT_WEIGHT_EXTRA_BLACK: DWRITE_FONT_WEIGHT = 950;
pub const DWRITE_FONT_WEIGHT_EXTRA_BOLD: DWRITE_FONT_WEIGHT = 800;
pub const DWRITE_FONT_WEIGHT_EXTRA_LIGHT: DWRITE_FONT_WEIGHT = 200;
pub const DWRITE_FONT_WEIGHT_HEAVY: DWRITE_FONT_WEIGHT = 900;
pub const DWRITE_FONT_WEIGHT_LIGHT: DWRITE_FONT_WEIGHT = 300;
pub const DWRITE_FONT_WEIGHT_MEDIUM: DWRITE_FONT_WEIGHT = 500;
pub const DWRITE_FONT_WEIGHT_NORMAL: DWRITE_FONT_WEIGHT = 400;
pub const DWRITE_FONT_WEIGHT_REGULAR: DWRITE_FONT_WEIGHT = 400;
pub const DWRITE_FONT_WEIGHT_SEMI_BOLD: DWRITE_FONT_WEIGHT = 600;
pub const DWRITE_FONT_WEIGHT_SEMI_LIGHT: DWRITE_FONT_WEIGHT = 350;
pub const DWRITE_FONT_WEIGHT_THIN: DWRITE_FONT_WEIGHT = 100;
pub const DWRITE_FONT_WEIGHT_ULTRA_BLACK: DWRITE_FONT_WEIGHT = 950;
pub const DWRITE_FONT_WEIGHT_ULTRA_BOLD: DWRITE_FONT_WEIGHT = 800;
pub const DWRITE_FONT_WEIGHT_ULTRA_LIGHT: DWRITE_FONT_WEIGHT = 200;
pub type DWRITE_GLYPH_IMAGE_FORMATS = u32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DWRITE_GLYPH_METRICS {
    pub leftSideBearing: i32,
    pub advanceWidth: u32,
    pub rightSideBearing: i32,
    pub topSideBearing: i32,
    pub advanceHeight: u32,
    pub bottomSideBearing: i32,
    pub verticalOriginY: i32,
}
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
pub type DWRITE_GRID_FIT_MODE = i32;
pub const DWRITE_GRID_FIT_MODE_DEFAULT: DWRITE_GRID_FIT_MODE = 0;
pub const DWRITE_GRID_FIT_MODE_DISABLED: DWRITE_GRID_FIT_MODE = 1;
pub const DWRITE_GRID_FIT_MODE_ENABLED: DWRITE_GRID_FIT_MODE = 2;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DWRITE_HIT_TEST_METRICS {
    pub textPosition: u32,
    pub length: u32,
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub bidiLevel: u32,
    pub isText: windows_core::BOOL,
    pub isTrimmed: windows_core::BOOL,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DWRITE_LINE_METRICS {
    pub length: u32,
    pub trailingWhitespaceLength: u32,
    pub newlineLength: u32,
    pub height: f32,
    pub baseline: f32,
    pub isTrimmed: windows_core::BOOL,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DWRITE_LINE_METRICS1 {
    pub Base: DWRITE_LINE_METRICS,
    pub leadingBefore: f32,
    pub leadingAfter: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DWRITE_LINE_SPACING {
    pub method: DWRITE_LINE_SPACING_METHOD,
    pub height: f32,
    pub baseline: f32,
    pub leadingBefore: f32,
    pub fontLineGapUsage: DWRITE_FONT_LINE_GAP_USAGE,
}
pub type DWRITE_LINE_SPACING_METHOD = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DWRITE_MATRIX {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub dx: f32,
    pub dy: f32,
}
pub type DWRITE_MEASURING_MODE = i32;
pub const DWRITE_MEASURING_MODE_NATURAL: DWRITE_MEASURING_MODE = 0;
pub type DWRITE_OPTICAL_ALIGNMENT = i32;
pub type DWRITE_PARAGRAPH_ALIGNMENT = i32;
pub const DWRITE_PARAGRAPH_ALIGNMENT_CENTER: DWRITE_PARAGRAPH_ALIGNMENT = 2;
pub const DWRITE_PARAGRAPH_ALIGNMENT_FAR: DWRITE_PARAGRAPH_ALIGNMENT = 1;
pub const DWRITE_PARAGRAPH_ALIGNMENT_NEAR: DWRITE_PARAGRAPH_ALIGNMENT = 0;
pub type DWRITE_PIXEL_GEOMETRY = i32;
pub const DWRITE_PIXEL_GEOMETRY_FLAT: DWRITE_PIXEL_GEOMETRY = 0;
pub type DWRITE_READING_DIRECTION = i32;
pub type DWRITE_RENDERING_MODE = i32;
pub type DWRITE_RENDERING_MODE1 = i32;
pub const DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC: DWRITE_RENDERING_MODE1 = 5;
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DWRITE_STRIKETHROUGH {
    pub width: f32,
    pub thickness: f32,
    pub offset: f32,
    pub readingDirection: DWRITE_READING_DIRECTION,
    pub flowDirection: DWRITE_FLOW_DIRECTION,
    pub localeName: *const u16,
    pub measuringMode: DWRITE_MEASURING_MODE,
}
impl Default for DWRITE_STRIKETHROUGH {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub const DWRITE_TEXTURE_ALIASED_1x1: DWRITE_TEXTURE_TYPE = 0;
pub const DWRITE_TEXTURE_CLEARTYPE_3x1: DWRITE_TEXTURE_TYPE = 1;
pub type DWRITE_TEXTURE_TYPE = i32;
pub type DWRITE_TEXT_ALIGNMENT = i32;
pub const DWRITE_TEXT_ALIGNMENT_CENTER: DWRITE_TEXT_ALIGNMENT = 2;
pub const DWRITE_TEXT_ALIGNMENT_LEADING: DWRITE_TEXT_ALIGNMENT = 0;
pub const DWRITE_TEXT_ALIGNMENT_TRAILING: DWRITE_TEXT_ALIGNMENT = 1;
pub type DWRITE_TEXT_ANTIALIAS_MODE = i32;
pub const DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE: DWRITE_TEXT_ANTIALIAS_MODE = 1;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DWRITE_TEXT_METRICS {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub widthIncludingTrailingWhitespace: f32,
    pub height: f32,
    pub layoutWidth: f32,
    pub layoutHeight: f32,
    pub maxBidiReorderingDepth: u32,
    pub lineCount: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DWRITE_TEXT_METRICS1 {
    pub Base: DWRITE_TEXT_METRICS,
    pub heightIncludingTrailingWhitespace: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DWRITE_TEXT_RANGE {
    pub startPosition: u32,
    pub length: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DWRITE_TRIMMING {
    pub granularity: DWRITE_TRIMMING_GRANULARITY,
    pub delimiter: u32,
    pub delimiterCount: u32,
}
pub type DWRITE_TRIMMING_GRANULARITY = i32;
pub const DWRITE_TRIMMING_GRANULARITY_CHARACTER: DWRITE_TRIMMING_GRANULARITY = 1;
pub const DWRITE_TRIMMING_GRANULARITY_NONE: DWRITE_TRIMMING_GRANULARITY = 0;
pub const DWRITE_TRIMMING_GRANULARITY_WORD: DWRITE_TRIMMING_GRANULARITY = 2;
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DWRITE_UNDERLINE {
    pub width: f32,
    pub thickness: f32,
    pub offset: f32,
    pub runHeight: f32,
    pub readingDirection: DWRITE_READING_DIRECTION,
    pub flowDirection: DWRITE_FLOW_DIRECTION,
    pub localeName: *const u16,
    pub measuringMode: DWRITE_MEASURING_MODE,
}
impl Default for DWRITE_UNDERLINE {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub type DWRITE_VERTICAL_GLYPH_ORIENTATION = i32;
pub type DWRITE_WORD_WRAPPING = i32;
pub const DWRITE_WORD_WRAPPING_NO_WRAP: DWRITE_WORD_WRAPPING = 1;
pub const DWRITE_WORD_WRAPPING_WRAP: DWRITE_WORD_WRAPPING = 0;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FILETIME {
    pub dwLowDateTime: u32,
    pub dwHighDateTime: u32,
}
windows_core::imp::define_interface!(
    IDWriteColorGlyphRunEnumerator,
    IDWriteColorGlyphRunEnumerator_Vtbl,
    0xd31fbe17_f157_41a2_8d24_cb779e0560e8
);
windows_core::imp::interface_hierarchy!(IDWriteColorGlyphRunEnumerator, windows_core::IUnknown);
impl IDWriteColorGlyphRunEnumerator {
    pub unsafe fn MoveNext(&self) -> windows_core::Result<windows_core::BOOL> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).MoveNext)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn GetCurrentRun(&self) -> windows_core::Result<*mut DWRITE_COLOR_GLYPH_RUN> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetCurrentRun)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IDWriteColorGlyphRunEnumerator_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub MoveNext: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub GetCurrentRun: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut DWRITE_COLOR_GLYPH_RUN,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteColorGlyphRunEnumerator1,
    IDWriteColorGlyphRunEnumerator1_Vtbl,
    0x7c5f86da_c7a1_4f05_b8e1_55a179fe5a35
);
impl core::ops::Deref for IDWriteColorGlyphRunEnumerator1 {
    type Target = IDWriteColorGlyphRunEnumerator;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteColorGlyphRunEnumerator1,
    windows_core::IUnknown,
    IDWriteColorGlyphRunEnumerator
);
#[repr(C)]
pub struct IDWriteColorGlyphRunEnumerator1_Vtbl {
    pub base__: IDWriteColorGlyphRunEnumerator_Vtbl,
    GetCurrentRun: usize,
}
windows_core::imp::define_interface!(
    IDWriteFactory,
    IDWriteFactory_Vtbl,
    0xb859ee5a_d838_4b5b_a2e8_1adc7d93db48
);
windows_core::imp::interface_hierarchy!(IDWriteFactory, windows_core::IUnknown);
impl IDWriteFactory {
    pub unsafe fn GetSystemFontCollection(
        &self,
        fontcollection: *mut Option<IDWriteFontCollection>,
        checkforupdates: bool,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetSystemFontCollection)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute(fontcollection),
                checkforupdates.into(),
            )
        }
    }
    pub unsafe fn CreateCustomRenderingParams(
        &self,
        gamma: f32,
        enhancedcontrast: f32,
        cleartypelevel: f32,
        pixelgeometry: DWRITE_PIXEL_GEOMETRY,
        renderingmode: DWRITE_RENDERING_MODE,
    ) -> windows_core::Result<IDWriteRenderingParams> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateCustomRenderingParams)(
                windows_core::Interface::as_raw(self),
                gamma,
                enhancedcontrast,
                cleartypelevel,
                pixelgeometry,
                renderingmode,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateTextFormat<P0, P1, P6>(
        &self,
        fontfamilyname: P0,
        fontcollection: P1,
        fontweight: DWRITE_FONT_WEIGHT,
        fontstyle: DWRITE_FONT_STYLE,
        fontstretch: DWRITE_FONT_STRETCH,
        fontsize: f32,
        localename: P6,
    ) -> windows_core::Result<IDWriteTextFormat>
    where
        P0: windows_core::Param<windows_core::PCWSTR>,
        P1: windows_core::Param<IDWriteFontCollection>,
        P6: windows_core::Param<windows_core::PCWSTR>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateTextFormat)(
                windows_core::Interface::as_raw(self),
                fontfamilyname.param().abi(),
                fontcollection.param().abi(),
                fontweight,
                fontstyle,
                fontstretch,
                fontsize,
                localename.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateTextLayout<P2>(
        &self,
        string: &[u16],
        textformat: P2,
        maxwidth: f32,
        maxheight: f32,
    ) -> windows_core::Result<IDWriteTextLayout>
    where
        P2: windows_core::Param<IDWriteTextFormat>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateTextLayout)(
                windows_core::Interface::as_raw(self),
                string.as_ptr(),
                string.len().try_into().unwrap(),
                textformat.param().abi(),
                maxwidth,
                maxheight,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateEllipsisTrimmingSign<P0>(
        &self,
        textformat: P0,
    ) -> windows_core::Result<IDWriteInlineObject>
    where
        P0: windows_core::Param<IDWriteTextFormat>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateEllipsisTrimmingSign)(
                windows_core::Interface::as_raw(self),
                textformat.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDWriteFactory_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetSystemFontCollection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        windows_core::BOOL,
    ) -> windows_core::HRESULT,
    CreateCustomFontCollection: usize,
    RegisterFontCollectionLoader: usize,
    UnregisterFontCollectionLoader: usize,
    CreateFontFileReference: usize,
    CreateCustomFontFileReference: usize,
    CreateFontFace: usize,
    CreateRenderingParams: usize,
    CreateMonitorRenderingParams: usize,
    pub CreateCustomRenderingParams: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        f32,
        f32,
        DWRITE_PIXEL_GEOMETRY,
        DWRITE_RENDERING_MODE,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    RegisterFontFileLoader: usize,
    UnregisterFontFileLoader: usize,
    pub CreateTextFormat: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::PCWSTR,
        *mut core::ffi::c_void,
        DWRITE_FONT_WEIGHT,
        DWRITE_FONT_STYLE,
        DWRITE_FONT_STRETCH,
        f32,
        windows_core::PCWSTR,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateTypography: usize,
    GetGdiInterop: usize,
    pub CreateTextLayout: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const u16,
        u32,
        *mut core::ffi::c_void,
        f32,
        f32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateGdiCompatibleTextLayout: usize,
    pub CreateEllipsisTrimmingSign: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateTextAnalyzer: usize,
    CreateNumberSubstitution: usize,
    CreateGlyphRunAnalysis: usize,
}
windows_core::imp::define_interface!(
    IDWriteFactory1,
    IDWriteFactory1_Vtbl,
    0x30572f99_dac6_41db_a16e_0486307e606a
);
impl core::ops::Deref for IDWriteFactory1 {
    type Target = IDWriteFactory;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IDWriteFactory1, windows_core::IUnknown, IDWriteFactory);
impl IDWriteFactory1 {
    pub unsafe fn GetEudcFontCollection(
        &self,
        fontcollection: *mut Option<IDWriteFontCollection>,
        checkforupdates: bool,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetEudcFontCollection)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute(fontcollection),
                checkforupdates.into(),
            )
        }
    }
    pub unsafe fn CreateCustomRenderingParams(
        &self,
        gamma: f32,
        enhancedcontrast: f32,
        enhancedcontrastgrayscale: f32,
        cleartypelevel: f32,
        pixelgeometry: DWRITE_PIXEL_GEOMETRY,
        renderingmode: DWRITE_RENDERING_MODE,
    ) -> windows_core::Result<IDWriteRenderingParams1> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateCustomRenderingParams)(
                windows_core::Interface::as_raw(self),
                gamma,
                enhancedcontrast,
                enhancedcontrastgrayscale,
                cleartypelevel,
                pixelgeometry,
                renderingmode,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDWriteFactory1_Vtbl {
    pub base__: IDWriteFactory_Vtbl,
    pub GetEudcFontCollection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub CreateCustomRenderingParams: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        f32,
        f32,
        f32,
        DWRITE_PIXEL_GEOMETRY,
        DWRITE_RENDERING_MODE,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteFactory2,
    IDWriteFactory2_Vtbl,
    0x0439fc60_ca44_4994_8dee_3a9af7b732ec
);
impl core::ops::Deref for IDWriteFactory2 {
    type Target = IDWriteFactory1;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteFactory2,
    windows_core::IUnknown,
    IDWriteFactory,
    IDWriteFactory1
);
impl IDWriteFactory2 {
    pub unsafe fn TranslateColorGlyphRun(
        &self,
        baselineoriginx: f32,
        baselineoriginy: f32,
        glyphrun: *const DWRITE_GLYPH_RUN,
        glyphrundescription: Option<*const DWRITE_GLYPH_RUN_DESCRIPTION>,
        measuringmode: DWRITE_MEASURING_MODE,
        worldtodevicetransform: Option<*const DWRITE_MATRIX>,
        colorpaletteindex: u32,
    ) -> windows_core::Result<IDWriteColorGlyphRunEnumerator> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TranslateColorGlyphRun)(
                windows_core::Interface::as_raw(self),
                baselineoriginx,
                baselineoriginy,
                glyphrun,
                glyphrundescription.unwrap_or(core::mem::zeroed()) as _,
                measuringmode,
                worldtodevicetransform.unwrap_or(core::mem::zeroed()) as _,
                colorpaletteindex,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateGlyphRunAnalysis(
        &self,
        glyphrun: *const DWRITE_GLYPH_RUN,
        transform: Option<*const DWRITE_MATRIX>,
        renderingmode: DWRITE_RENDERING_MODE,
        measuringmode: DWRITE_MEASURING_MODE,
        gridfitmode: DWRITE_GRID_FIT_MODE,
        antialiasmode: DWRITE_TEXT_ANTIALIAS_MODE,
        baselineoriginx: f32,
        baselineoriginy: f32,
    ) -> windows_core::Result<IDWriteGlyphRunAnalysis> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateGlyphRunAnalysis)(
                windows_core::Interface::as_raw(self),
                glyphrun,
                transform.unwrap_or(core::mem::zeroed()) as _,
                renderingmode,
                measuringmode,
                gridfitmode,
                antialiasmode,
                baselineoriginx,
                baselineoriginy,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDWriteFactory2_Vtbl {
    pub base__: IDWriteFactory1_Vtbl,
    GetSystemFontFallback: usize,
    CreateFontFallbackBuilder: usize,
    pub TranslateColorGlyphRun: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        f32,
        *const DWRITE_GLYPH_RUN,
        *const DWRITE_GLYPH_RUN_DESCRIPTION,
        DWRITE_MEASURING_MODE,
        *const DWRITE_MATRIX,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateCustomRenderingParams: usize,
    pub CreateGlyphRunAnalysis: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const DWRITE_GLYPH_RUN,
        *const DWRITE_MATRIX,
        DWRITE_RENDERING_MODE,
        DWRITE_MEASURING_MODE,
        DWRITE_GRID_FIT_MODE,
        DWRITE_TEXT_ANTIALIAS_MODE,
        f32,
        f32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteFactory3,
    IDWriteFactory3_Vtbl,
    0x9a1b41c3_d3bb_466a_87fc_fe67556a3b65
);
impl core::ops::Deref for IDWriteFactory3 {
    type Target = IDWriteFactory2;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteFactory3,
    windows_core::IUnknown,
    IDWriteFactory,
    IDWriteFactory1,
    IDWriteFactory2
);
impl IDWriteFactory3 {
    pub unsafe fn CreateGlyphRunAnalysis(
        &self,
        glyphrun: *const DWRITE_GLYPH_RUN,
        transform: Option<*const DWRITE_MATRIX>,
        renderingmode: DWRITE_RENDERING_MODE1,
        measuringmode: DWRITE_MEASURING_MODE,
        gridfitmode: DWRITE_GRID_FIT_MODE,
        antialiasmode: DWRITE_TEXT_ANTIALIAS_MODE,
        baselineoriginx: f32,
        baselineoriginy: f32,
    ) -> windows_core::Result<IDWriteGlyphRunAnalysis> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateGlyphRunAnalysis)(
                windows_core::Interface::as_raw(self),
                glyphrun,
                transform.unwrap_or(core::mem::zeroed()) as _,
                renderingmode,
                measuringmode,
                gridfitmode,
                antialiasmode,
                baselineoriginx,
                baselineoriginy,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateCustomRenderingParams(
        &self,
        gamma: f32,
        enhancedcontrast: f32,
        grayscaleenhancedcontrast: f32,
        cleartypelevel: f32,
        pixelgeometry: DWRITE_PIXEL_GEOMETRY,
        renderingmode: DWRITE_RENDERING_MODE1,
        gridfitmode: DWRITE_GRID_FIT_MODE,
    ) -> windows_core::Result<IDWriteRenderingParams3> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateCustomRenderingParams)(
                windows_core::Interface::as_raw(self),
                gamma,
                enhancedcontrast,
                grayscaleenhancedcontrast,
                cleartypelevel,
                pixelgeometry,
                renderingmode,
                gridfitmode,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateFontFaceReference<P0>(
        &self,
        fontfile: P0,
        faceindex: u32,
        fontsimulations: DWRITE_FONT_SIMULATIONS,
    ) -> windows_core::Result<IDWriteFontFaceReference>
    where
        P0: windows_core::Param<IDWriteFontFile>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateFontFaceReference)(
                windows_core::Interface::as_raw(self),
                fontfile.param().abi(),
                faceindex,
                fontsimulations,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateFontFaceReference2<P0>(
        &self,
        filepath: P0,
        lastwritetime: Option<*const FILETIME>,
        faceindex: u32,
        fontsimulations: DWRITE_FONT_SIMULATIONS,
    ) -> windows_core::Result<IDWriteFontFaceReference>
    where
        P0: windows_core::Param<windows_core::PCWSTR>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateFontFaceReference2)(
                windows_core::Interface::as_raw(self),
                filepath.param().abi(),
                lastwritetime.unwrap_or(core::mem::zeroed()) as _,
                faceindex,
                fontsimulations,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetSystemFontSet(&self) -> windows_core::Result<IDWriteFontSet> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetSystemFontSet)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateFontSetBuilder(&self) -> windows_core::Result<IDWriteFontSetBuilder> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateFontSetBuilder)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateFontCollectionFromFontSet<P0>(
        &self,
        fontset: P0,
    ) -> windows_core::Result<IDWriteFontCollection1>
    where
        P0: windows_core::Param<IDWriteFontSet>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateFontCollectionFromFontSet)(
                windows_core::Interface::as_raw(self),
                fontset.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetSystemFontCollection(
        &self,
        includedownloadablefonts: bool,
        fontcollection: *mut Option<IDWriteFontCollection1>,
        checkforupdates: bool,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetSystemFontCollection)(
                windows_core::Interface::as_raw(self),
                includedownloadablefonts.into(),
                core::mem::transmute(fontcollection),
                checkforupdates.into(),
            )
        }
    }
    pub unsafe fn GetFontDownloadQueue(&self) -> windows_core::Result<IDWriteFontDownloadQueue> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFontDownloadQueue)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDWriteFactory3_Vtbl {
    pub base__: IDWriteFactory2_Vtbl,
    pub CreateGlyphRunAnalysis: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const DWRITE_GLYPH_RUN,
        *const DWRITE_MATRIX,
        DWRITE_RENDERING_MODE1,
        DWRITE_MEASURING_MODE,
        DWRITE_GRID_FIT_MODE,
        DWRITE_TEXT_ANTIALIAS_MODE,
        f32,
        f32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateCustomRenderingParams: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        f32,
        f32,
        f32,
        DWRITE_PIXEL_GEOMETRY,
        DWRITE_RENDERING_MODE1,
        DWRITE_GRID_FIT_MODE,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateFontFaceReference: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        u32,
        DWRITE_FONT_SIMULATIONS,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateFontFaceReference2: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::PCWSTR,
        *const FILETIME,
        u32,
        DWRITE_FONT_SIMULATIONS,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetSystemFontSet: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateFontSetBuilder: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateFontCollectionFromFontSet: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetSystemFontCollection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::BOOL,
        *mut *mut core::ffi::c_void,
        windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub GetFontDownloadQueue: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteFactory4,
    IDWriteFactory4_Vtbl,
    0x4b0b5bd3_0797_4549_8ac5_fe915cc53856
);
impl core::ops::Deref for IDWriteFactory4 {
    type Target = IDWriteFactory3;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteFactory4,
    windows_core::IUnknown,
    IDWriteFactory,
    IDWriteFactory1,
    IDWriteFactory2,
    IDWriteFactory3
);
impl IDWriteFactory4 {
    pub unsafe fn TranslateColorGlyphRun(
        &self,
        baselineorigin: windows_numerics::Vector2,
        glyphrun: *const DWRITE_GLYPH_RUN,
        glyphrundescription: Option<*const DWRITE_GLYPH_RUN_DESCRIPTION>,
        desiredglyphimageformats: DWRITE_GLYPH_IMAGE_FORMATS,
        measuringmode: DWRITE_MEASURING_MODE,
        worldanddpitransform: Option<*const DWRITE_MATRIX>,
        colorpaletteindex: u32,
    ) -> windows_core::Result<IDWriteColorGlyphRunEnumerator1> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TranslateColorGlyphRun)(
                windows_core::Interface::as_raw(self),
                baselineorigin,
                glyphrun,
                glyphrundescription.unwrap_or(core::mem::zeroed()) as _,
                desiredglyphimageformats,
                measuringmode,
                worldanddpitransform.unwrap_or(core::mem::zeroed()) as _,
                colorpaletteindex,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn ComputeGlyphOrigins(
        &self,
        glyphrun: *const DWRITE_GLYPH_RUN,
        baselineorigin: windows_numerics::Vector2,
    ) -> windows_core::Result<windows_numerics::Vector2> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ComputeGlyphOrigins)(
                windows_core::Interface::as_raw(self),
                glyphrun,
                baselineorigin,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn ComputeGlyphOrigins2(
        &self,
        glyphrun: *const DWRITE_GLYPH_RUN,
        measuringmode: DWRITE_MEASURING_MODE,
        baselineorigin: windows_numerics::Vector2,
        worldanddpitransform: Option<*const DWRITE_MATRIX>,
    ) -> windows_core::Result<windows_numerics::Vector2> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ComputeGlyphOrigins2)(
                windows_core::Interface::as_raw(self),
                glyphrun,
                measuringmode,
                baselineorigin,
                worldanddpitransform.unwrap_or(core::mem::zeroed()) as _,
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IDWriteFactory4_Vtbl {
    pub base__: IDWriteFactory3_Vtbl,
    pub TranslateColorGlyphRun: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
        *const DWRITE_GLYPH_RUN,
        *const DWRITE_GLYPH_RUN_DESCRIPTION,
        DWRITE_GLYPH_IMAGE_FORMATS,
        DWRITE_MEASURING_MODE,
        *const DWRITE_MATRIX,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ComputeGlyphOrigins: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const DWRITE_GLYPH_RUN,
        windows_numerics::Vector2,
        *mut windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    pub ComputeGlyphOrigins2: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const DWRITE_GLYPH_RUN,
        DWRITE_MEASURING_MODE,
        windows_numerics::Vector2,
        *const DWRITE_MATRIX,
        *mut windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteFactory5,
    IDWriteFactory5_Vtbl,
    0x958db99a_be2a_4f09_af7d_65189803d1d3
);
impl core::ops::Deref for IDWriteFactory5 {
    type Target = IDWriteFactory4;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteFactory5,
    windows_core::IUnknown,
    IDWriteFactory,
    IDWriteFactory1,
    IDWriteFactory2,
    IDWriteFactory3,
    IDWriteFactory4
);
impl IDWriteFactory5 {
    pub unsafe fn CreateFontSetBuilder(&self) -> windows_core::Result<IDWriteFontSetBuilder1> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateFontSetBuilder)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateInMemoryFontFileLoader(
        &self,
    ) -> windows_core::Result<IDWriteInMemoryFontFileLoader> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateInMemoryFontFileLoader)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateHttpFontFileLoader<P0, P1>(
        &self,
        referrerurl: P0,
        extraheaders: P1,
    ) -> windows_core::Result<IDWriteRemoteFontFileLoader>
    where
        P0: windows_core::Param<windows_core::PCWSTR>,
        P1: windows_core::Param<windows_core::PCWSTR>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateHttpFontFileLoader)(
                windows_core::Interface::as_raw(self),
                referrerurl.param().abi(),
                extraheaders.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn AnalyzeContainerType(
        &self,
        filedata: *const core::ffi::c_void,
        filedatasize: u32,
    ) -> DWRITE_CONTAINER_TYPE {
        unsafe {
            (windows_core::Interface::vtable(self).AnalyzeContainerType)(
                windows_core::Interface::as_raw(self),
                filedata,
                filedatasize,
            )
        }
    }
    pub unsafe fn UnpackFontFile(
        &self,
        containertype: DWRITE_CONTAINER_TYPE,
        filedata: *const core::ffi::c_void,
        filedatasize: u32,
    ) -> windows_core::Result<IDWriteFontFileStream> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).UnpackFontFile)(
                windows_core::Interface::as_raw(self),
                containertype,
                filedata,
                filedatasize,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDWriteFactory5_Vtbl {
    pub base__: IDWriteFactory4_Vtbl,
    pub CreateFontSetBuilder: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateInMemoryFontFileLoader: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateHttpFontFileLoader: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::PCWSTR,
        windows_core::PCWSTR,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub AnalyzeContainerType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const core::ffi::c_void,
        u32,
    ) -> DWRITE_CONTAINER_TYPE,
    pub UnpackFontFile: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_CONTAINER_TYPE,
        *const core::ffi::c_void,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteFactory6,
    IDWriteFactory6_Vtbl,
    0xf3744d80_21f7_42eb_b35d_995bc72fc223
);
impl core::ops::Deref for IDWriteFactory6 {
    type Target = IDWriteFactory5;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteFactory6,
    windows_core::IUnknown,
    IDWriteFactory,
    IDWriteFactory1,
    IDWriteFactory2,
    IDWriteFactory3,
    IDWriteFactory4,
    IDWriteFactory5
);
impl IDWriteFactory6 {
    pub unsafe fn CreateFontFaceReference<P0>(
        &self,
        fontfile: P0,
        faceindex: u32,
        fontsimulations: DWRITE_FONT_SIMULATIONS,
        fontaxisvalues: &[DWRITE_FONT_AXIS_VALUE],
    ) -> windows_core::Result<IDWriteFontFaceReference1>
    where
        P0: windows_core::Param<IDWriteFontFile>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateFontFaceReference)(
                windows_core::Interface::as_raw(self),
                fontfile.param().abi(),
                faceindex,
                fontsimulations,
                fontaxisvalues.as_ptr(),
                fontaxisvalues.len().try_into().unwrap(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateFontResource<P0>(
        &self,
        fontfile: P0,
        faceindex: u32,
    ) -> windows_core::Result<IDWriteFontResource>
    where
        P0: windows_core::Param<IDWriteFontFile>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateFontResource)(
                windows_core::Interface::as_raw(self),
                fontfile.param().abi(),
                faceindex,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetSystemFontSet(
        &self,
        includedownloadablefonts: bool,
    ) -> windows_core::Result<IDWriteFontSet1> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetSystemFontSet)(
                windows_core::Interface::as_raw(self),
                includedownloadablefonts.into(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetSystemFontCollection(
        &self,
        includedownloadablefonts: bool,
        fontfamilymodel: DWRITE_FONT_FAMILY_MODEL,
    ) -> windows_core::Result<IDWriteFontCollection2> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetSystemFontCollection)(
                windows_core::Interface::as_raw(self),
                includedownloadablefonts.into(),
                fontfamilymodel,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateFontCollectionFromFontSet<P0>(
        &self,
        fontset: P0,
        fontfamilymodel: DWRITE_FONT_FAMILY_MODEL,
    ) -> windows_core::Result<IDWriteFontCollection2>
    where
        P0: windows_core::Param<IDWriteFontSet>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateFontCollectionFromFontSet)(
                windows_core::Interface::as_raw(self),
                fontset.param().abi(),
                fontfamilymodel,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateFontSetBuilder(&self) -> windows_core::Result<IDWriteFontSetBuilder2> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateFontSetBuilder)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateTextFormat<P0, P1, P5>(
        &self,
        fontfamilyname: P0,
        fontcollection: P1,
        fontaxisvalues: &[DWRITE_FONT_AXIS_VALUE],
        fontsize: f32,
        localename: P5,
    ) -> windows_core::Result<IDWriteTextFormat3>
    where
        P0: windows_core::Param<windows_core::PCWSTR>,
        P1: windows_core::Param<IDWriteFontCollection>,
        P5: windows_core::Param<windows_core::PCWSTR>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateTextFormat)(
                windows_core::Interface::as_raw(self),
                fontfamilyname.param().abi(),
                fontcollection.param().abi(),
                fontaxisvalues.as_ptr(),
                fontaxisvalues.len().try_into().unwrap(),
                fontsize,
                localename.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDWriteFactory6_Vtbl {
    pub base__: IDWriteFactory5_Vtbl,
    pub CreateFontFaceReference: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        u32,
        DWRITE_FONT_SIMULATIONS,
        *const DWRITE_FONT_AXIS_VALUE,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateFontResource: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetSystemFontSet: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::BOOL,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetSystemFontCollection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::BOOL,
        DWRITE_FONT_FAMILY_MODEL,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateFontCollectionFromFontSet: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        DWRITE_FONT_FAMILY_MODEL,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateFontSetBuilder: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateTextFormat: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::PCWSTR,
        *mut core::ffi::c_void,
        *const DWRITE_FONT_AXIS_VALUE,
        u32,
        f32,
        windows_core::PCWSTR,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteFactory7,
    IDWriteFactory7_Vtbl,
    0x35d0e0b3_9076_4d2e_a016_a91b568a06b4
);
impl core::ops::Deref for IDWriteFactory7 {
    type Target = IDWriteFactory6;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteFactory7,
    windows_core::IUnknown,
    IDWriteFactory,
    IDWriteFactory1,
    IDWriteFactory2,
    IDWriteFactory3,
    IDWriteFactory4,
    IDWriteFactory5,
    IDWriteFactory6
);
impl IDWriteFactory7 {
    pub unsafe fn GetSystemFontSet(
        &self,
        includedownloadablefonts: bool,
    ) -> windows_core::Result<IDWriteFontSet2> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetSystemFontSet)(
                windows_core::Interface::as_raw(self),
                includedownloadablefonts.into(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetSystemFontCollection(
        &self,
        includedownloadablefonts: bool,
        fontfamilymodel: DWRITE_FONT_FAMILY_MODEL,
    ) -> windows_core::Result<IDWriteFontCollection3> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetSystemFontCollection)(
                windows_core::Interface::as_raw(self),
                includedownloadablefonts.into(),
                fontfamilymodel,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDWriteFactory7_Vtbl {
    pub base__: IDWriteFactory6_Vtbl,
    pub GetSystemFontSet: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::BOOL,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetSystemFontCollection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::BOOL,
        DWRITE_FONT_FAMILY_MODEL,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteFont,
    IDWriteFont_Vtbl,
    0xacd16696_8c14_4f5d_877e_fe3fc1d32737
);
windows_core::imp::interface_hierarchy!(IDWriteFont, windows_core::IUnknown);
impl IDWriteFont {
    pub unsafe fn CreateFontFace(&self) -> windows_core::Result<IDWriteFontFace> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateFontFace)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDWriteFont_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetFontFamily: usize,
    GetWeight: usize,
    GetStretch: usize,
    GetStyle: usize,
    IsSymbolFont: usize,
    GetFaceNames: usize,
    GetInformationalStrings: usize,
    GetSimulations: usize,
    GetMetrics: usize,
    HasCharacter: usize,
    pub CreateFontFace: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteFontCollection,
    IDWriteFontCollection_Vtbl,
    0xa84cee02_3eea_4eee_a827_87c1a02a0fcc
);
windows_core::imp::interface_hierarchy!(IDWriteFontCollection, windows_core::IUnknown);
impl IDWriteFontCollection {
    pub unsafe fn GetFontFamily(&self, index: u32) -> windows_core::Result<IDWriteFontFamily> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFontFamily)(
                windows_core::Interface::as_raw(self),
                index,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn FindFamilyName<P0>(
        &self,
        familyname: P0,
        index: *mut u32,
        exists: *mut windows_core::BOOL,
    ) -> windows_core::HRESULT
    where
        P0: windows_core::Param<windows_core::PCWSTR>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).FindFamilyName)(
                windows_core::Interface::as_raw(self),
                familyname.param().abi(),
                index as _,
                exists as _,
            )
        }
    }
}
#[repr(C)]
pub struct IDWriteFontCollection_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetFontFamilyCount: usize,
    pub GetFontFamily: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub FindFamilyName: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::PCWSTR,
        *mut u32,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    GetFontFromFontFace: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontCollection1,
    IDWriteFontCollection1_Vtbl,
    0x53585141_d9f8_4095_8321_d73cf6bd116c
);
impl core::ops::Deref for IDWriteFontCollection1 {
    type Target = IDWriteFontCollection;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteFontCollection1,
    windows_core::IUnknown,
    IDWriteFontCollection
);
#[repr(C)]
pub struct IDWriteFontCollection1_Vtbl {
    pub base__: IDWriteFontCollection_Vtbl,
    GetFontSet: usize,
    GetFontFamily: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontCollection2,
    IDWriteFontCollection2_Vtbl,
    0x514039c6_4617_4064_bf8b_92ea83e506e0
);
impl core::ops::Deref for IDWriteFontCollection2 {
    type Target = IDWriteFontCollection1;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteFontCollection2,
    windows_core::IUnknown,
    IDWriteFontCollection,
    IDWriteFontCollection1
);
#[repr(C)]
pub struct IDWriteFontCollection2_Vtbl {
    pub base__: IDWriteFontCollection1_Vtbl,
    GetFontFamily: usize,
    GetMatchingFonts: usize,
    GetFontFamilyModel: usize,
    GetFontSet: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontCollection3,
    IDWriteFontCollection3_Vtbl,
    0xa4d055a6_f9e3_4e25_93b7_9e309f3af8e9
);
impl core::ops::Deref for IDWriteFontCollection3 {
    type Target = IDWriteFontCollection2;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteFontCollection3,
    windows_core::IUnknown,
    IDWriteFontCollection,
    IDWriteFontCollection1,
    IDWriteFontCollection2
);
#[repr(C)]
pub struct IDWriteFontCollection3_Vtbl {
    pub base__: IDWriteFontCollection2_Vtbl,
    GetExpirationEvent: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontDownloadQueue,
    IDWriteFontDownloadQueue_Vtbl,
    0xb71e6052_5aea_4fa3_832e_f60d431f7e91
);
windows_core::imp::interface_hierarchy!(IDWriteFontDownloadQueue, windows_core::IUnknown);
#[repr(C)]
pub struct IDWriteFontDownloadQueue_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    AddListener: usize,
    RemoveListener: usize,
    IsEmpty: usize,
    BeginDownload: usize,
    CancelDownload: usize,
    GetGenerationCount: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontFace,
    IDWriteFontFace_Vtbl,
    0x5f49804d_7024_4d43_bfa9_d25984f53849
);
windows_core::imp::interface_hierarchy!(IDWriteFontFace, windows_core::IUnknown);
impl IDWriteFontFace {
    pub unsafe fn GetMetrics(&self, fontfacemetrics: *mut DWRITE_FONT_METRICS) {
        unsafe {
            (windows_core::Interface::vtable(self).GetMetrics)(
                windows_core::Interface::as_raw(self),
                fontfacemetrics as _,
            );
        }
    }
    pub unsafe fn GetDesignGlyphMetrics(
        &self,
        glyphindices: *const u16,
        glyphcount: u32,
        glyphmetrics: *mut DWRITE_GLYPH_METRICS,
        issideways: bool,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetDesignGlyphMetrics)(
                windows_core::Interface::as_raw(self),
                glyphindices,
                glyphcount,
                glyphmetrics as _,
                issideways.into(),
            )
        }
    }
    pub unsafe fn GetGlyphIndices(
        &self,
        codepoints: *const u32,
        codepointcount: u32,
        glyphindices: *mut u16,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetGlyphIndices)(
                windows_core::Interface::as_raw(self),
                codepoints,
                codepointcount,
                glyphindices as _,
            )
        }
    }
}
#[repr(C)]
pub struct IDWriteFontFace_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetType: usize,
    GetFiles: usize,
    GetIndex: usize,
    GetSimulations: usize,
    IsSymbolFont: usize,
    pub GetMetrics: unsafe extern "system" fn(*mut core::ffi::c_void, *mut DWRITE_FONT_METRICS),
    GetGlyphCount: usize,
    pub GetDesignGlyphMetrics: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const u16,
        u32,
        *mut DWRITE_GLYPH_METRICS,
        windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub GetGlyphIndices: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const u32,
        u32,
        *mut u16,
    ) -> windows_core::HRESULT,
    TryGetFontTable: usize,
    ReleaseFontTable: usize,
    GetGlyphRunOutline: usize,
    GetRecommendedRenderingMode: usize,
    GetGdiCompatibleMetrics: usize,
    GetGdiCompatibleGlyphMetrics: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontFaceReference,
    IDWriteFontFaceReference_Vtbl,
    0x5e7fa7ca_dde3_424c_89f0_9fcd6fed58cd
);
windows_core::imp::interface_hierarchy!(IDWriteFontFaceReference, windows_core::IUnknown);
#[repr(C)]
pub struct IDWriteFontFaceReference_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    CreateFontFace: usize,
    CreateFontFaceWithSimulations: usize,
    Equals: usize,
    GetFontFaceIndex: usize,
    GetSimulations: usize,
    GetFontFile: usize,
    GetLocalFileSize: usize,
    GetFileSize: usize,
    GetFileTime: usize,
    GetLocality: usize,
    EnqueueFontDownloadRequest: usize,
    EnqueueCharacterDownloadRequest: usize,
    EnqueueGlyphDownloadRequest: usize,
    EnqueueFileFragmentDownloadRequest: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontFaceReference1,
    IDWriteFontFaceReference1_Vtbl,
    0xc081fe77_2fd1_41ac_a5a3_34983c4ba61a
);
impl core::ops::Deref for IDWriteFontFaceReference1 {
    type Target = IDWriteFontFaceReference;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteFontFaceReference1,
    windows_core::IUnknown,
    IDWriteFontFaceReference
);
#[repr(C)]
pub struct IDWriteFontFaceReference1_Vtbl {
    pub base__: IDWriteFontFaceReference_Vtbl,
    CreateFontFace: usize,
    GetFontAxisValueCount: usize,
    GetFontAxisValues: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontFallback,
    IDWriteFontFallback_Vtbl,
    0xefa008f9_f7a1_48bf_b05c_f224713cc0ff
);
windows_core::imp::interface_hierarchy!(IDWriteFontFallback, windows_core::IUnknown);
#[repr(C)]
pub struct IDWriteFontFallback_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    MapCharacters: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontFamily,
    IDWriteFontFamily_Vtbl,
    0xda20d8ef_812a_4c43_9802_62ec4abd7add
);
impl core::ops::Deref for IDWriteFontFamily {
    type Target = IDWriteFontList;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IDWriteFontFamily, windows_core::IUnknown, IDWriteFontList);
impl IDWriteFontFamily {
    pub unsafe fn GetFirstMatchingFont(
        &self,
        weight: DWRITE_FONT_WEIGHT,
        stretch: DWRITE_FONT_STRETCH,
        style: DWRITE_FONT_STYLE,
    ) -> windows_core::Result<IDWriteFont> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFirstMatchingFont)(
                windows_core::Interface::as_raw(self),
                weight,
                stretch,
                style,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDWriteFontFamily_Vtbl {
    pub base__: IDWriteFontList_Vtbl,
    GetFamilyNames: usize,
    pub GetFirstMatchingFont: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_FONT_WEIGHT,
        DWRITE_FONT_STRETCH,
        DWRITE_FONT_STYLE,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    GetMatchingFonts: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontFile,
    IDWriteFontFile_Vtbl,
    0x739d886a_cef5_47dc_8769_1a8b41bebbb0
);
windows_core::imp::interface_hierarchy!(IDWriteFontFile, windows_core::IUnknown);
#[repr(C)]
pub struct IDWriteFontFile_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetReferenceKey: usize,
    GetLoader: usize,
    Analyze: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontFileLoader,
    IDWriteFontFileLoader_Vtbl,
    0x727cad4e_d6af_4c9e_8a08_d695b11caa49
);
windows_core::imp::interface_hierarchy!(IDWriteFontFileLoader, windows_core::IUnknown);
#[repr(C)]
pub struct IDWriteFontFileLoader_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    CreateStreamFromKey: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontFileStream,
    IDWriteFontFileStream_Vtbl,
    0x6d4865fe_0ab8_4d91_8f62_5dd6be34a3e0
);
windows_core::imp::interface_hierarchy!(IDWriteFontFileStream, windows_core::IUnknown);
#[repr(C)]
pub struct IDWriteFontFileStream_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    ReadFileFragment: usize,
    ReleaseFileFragment: usize,
    GetFileSize: usize,
    GetLastWriteTime: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontList,
    IDWriteFontList_Vtbl,
    0x1a0d8438_1d97_4ec1_aef9_a2fb86ed6acb
);
windows_core::imp::interface_hierarchy!(IDWriteFontList, windows_core::IUnknown);
impl IDWriteFontList {
    pub unsafe fn GetFontCollection(&self) -> windows_core::Result<IDWriteFontCollection> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFontCollection)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetFontCount(&self) -> u32 {
        unsafe {
            (windows_core::Interface::vtable(self).GetFontCount)(windows_core::Interface::as_raw(
                self,
            ))
        }
    }
    pub unsafe fn GetFont(&self, index: u32) -> windows_core::Result<IDWriteFont> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFont)(
                windows_core::Interface::as_raw(self),
                index,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDWriteFontList_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetFontCollection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetFontCount: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
    pub GetFont: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteFontResource,
    IDWriteFontResource_Vtbl,
    0x1f803a76_6871_48e8_987f_b975551c50f2
);
windows_core::imp::interface_hierarchy!(IDWriteFontResource, windows_core::IUnknown);
#[repr(C)]
pub struct IDWriteFontResource_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetFontFile: usize,
    GetFontFaceIndex: usize,
    GetFontAxisCount: usize,
    GetDefaultFontAxisValues: usize,
    GetFontAxisRanges: usize,
    GetFontAxisAttributes: usize,
    GetAxisNames: usize,
    GetAxisValueNameCount: usize,
    GetAxisValueNames: usize,
    HasVariations: usize,
    CreateFontFace: usize,
    CreateFontFaceReference: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontSet,
    IDWriteFontSet_Vtbl,
    0x53585141_d9f8_4095_8321_d73cf6bd116b
);
windows_core::imp::interface_hierarchy!(IDWriteFontSet, windows_core::IUnknown);
#[repr(C)]
pub struct IDWriteFontSet_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetFontCount: usize,
    GetFontFaceReference: usize,
    FindFontFaceReference: usize,
    FindFontFace: usize,
    GetPropertyValues: usize,
    GetPropertyValues2: usize,
    GetPropertyValues3: usize,
    GetPropertyOccurrenceCount: usize,
    GetMatchingFonts: usize,
    GetMatchingFonts2: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontSet1,
    IDWriteFontSet1_Vtbl,
    0x7e9fda85_6c92_4053_bc47_7ae3530db4d3
);
impl core::ops::Deref for IDWriteFontSet1 {
    type Target = IDWriteFontSet;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IDWriteFontSet1, windows_core::IUnknown, IDWriteFontSet);
#[repr(C)]
pub struct IDWriteFontSet1_Vtbl {
    pub base__: IDWriteFontSet_Vtbl,
    GetMatchingFonts: usize,
    GetFirstFontResources: usize,
    GetFilteredFonts: usize,
    GetFilteredFonts2: usize,
    GetFilteredFonts3: usize,
    GetFilteredFontIndices: usize,
    GetFilteredFontIndices2: usize,
    GetFontAxisRanges: usize,
    GetFontAxisRanges2: usize,
    GetFontFaceReference: usize,
    CreateFontResource: usize,
    CreateFontFace: usize,
    GetFontLocality: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontSet2,
    IDWriteFontSet2_Vtbl,
    0xdc7ead19_e54c_43af_b2da_4e2b79ba3f7f
);
impl core::ops::Deref for IDWriteFontSet2 {
    type Target = IDWriteFontSet1;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteFontSet2,
    windows_core::IUnknown,
    IDWriteFontSet,
    IDWriteFontSet1
);
#[repr(C)]
pub struct IDWriteFontSet2_Vtbl {
    pub base__: IDWriteFontSet1_Vtbl,
    GetExpirationEvent: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontSetBuilder,
    IDWriteFontSetBuilder_Vtbl,
    0x2f642afe_9c68_4f40_b8be_457401afcb3d
);
windows_core::imp::interface_hierarchy!(IDWriteFontSetBuilder, windows_core::IUnknown);
#[repr(C)]
pub struct IDWriteFontSetBuilder_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    AddFontFaceReference: usize,
    AddFontFaceReference2: usize,
    AddFontSet: usize,
    CreateFontSet: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontSetBuilder1,
    IDWriteFontSetBuilder1_Vtbl,
    0x3ff7715f_3cdc_4dc6_9b72_ec5621dccafd
);
impl core::ops::Deref for IDWriteFontSetBuilder1 {
    type Target = IDWriteFontSetBuilder;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteFontSetBuilder1,
    windows_core::IUnknown,
    IDWriteFontSetBuilder
);
#[repr(C)]
pub struct IDWriteFontSetBuilder1_Vtbl {
    pub base__: IDWriteFontSetBuilder_Vtbl,
    AddFontFile: usize,
}
windows_core::imp::define_interface!(
    IDWriteFontSetBuilder2,
    IDWriteFontSetBuilder2_Vtbl,
    0xee5ba612_b131_463c_8f4f_3189b9401e45
);
impl core::ops::Deref for IDWriteFontSetBuilder2 {
    type Target = IDWriteFontSetBuilder1;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteFontSetBuilder2,
    windows_core::IUnknown,
    IDWriteFontSetBuilder,
    IDWriteFontSetBuilder1
);
#[repr(C)]
pub struct IDWriteFontSetBuilder2_Vtbl {
    pub base__: IDWriteFontSetBuilder1_Vtbl,
    AddFont: usize,
    AddFontFile: usize,
}
windows_core::imp::define_interface!(
    IDWriteGlyphRunAnalysis,
    IDWriteGlyphRunAnalysis_Vtbl,
    0x7d97dbf7_e085_42d4_81e3_6a883bded118
);
windows_core::imp::interface_hierarchy!(IDWriteGlyphRunAnalysis, windows_core::IUnknown);
impl IDWriteGlyphRunAnalysis {
    pub unsafe fn GetAlphaTextureBounds(
        &self,
        texturetype: DWRITE_TEXTURE_TYPE,
    ) -> windows_core::Result<RECT> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetAlphaTextureBounds)(
                windows_core::Interface::as_raw(self),
                texturetype,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn CreateAlphaTexture(
        &self,
        texturetype: DWRITE_TEXTURE_TYPE,
        texturebounds: *const RECT,
        alphavalues: &mut [u8],
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).CreateAlphaTexture)(
                windows_core::Interface::as_raw(self),
                texturetype,
                texturebounds,
                alphavalues.as_mut_ptr(),
                alphavalues.len().try_into().unwrap(),
            )
        }
    }
}
#[repr(C)]
pub struct IDWriteGlyphRunAnalysis_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetAlphaTextureBounds: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_TEXTURE_TYPE,
        *mut RECT,
    ) -> windows_core::HRESULT,
    pub CreateAlphaTexture: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_TEXTURE_TYPE,
        *const RECT,
        *mut u8,
        u32,
    ) -> windows_core::HRESULT,
    GetAlphaBlendParams: usize,
}
windows_core::imp::define_interface!(
    IDWriteInMemoryFontFileLoader,
    IDWriteInMemoryFontFileLoader_Vtbl,
    0xdc102f47_a12d_4b1c_822d_9e117e33043f
);
impl core::ops::Deref for IDWriteInMemoryFontFileLoader {
    type Target = IDWriteFontFileLoader;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteInMemoryFontFileLoader,
    windows_core::IUnknown,
    IDWriteFontFileLoader
);
#[repr(C)]
pub struct IDWriteInMemoryFontFileLoader_Vtbl {
    pub base__: IDWriteFontFileLoader_Vtbl,
    CreateInMemoryFontFileReference: usize,
    GetFileCount: usize,
}
windows_core::imp::define_interface!(
    IDWriteInlineObject,
    IDWriteInlineObject_Vtbl,
    0x8339fde3_106f_47ab_8373_1c6295eb10b3
);
windows_core::imp::interface_hierarchy!(IDWriteInlineObject, windows_core::IUnknown);
impl IDWriteInlineObject {
    pub unsafe fn Draw<P1, P6>(
        &self,
        clientdrawingcontext: Option<*const core::ffi::c_void>,
        renderer: P1,
        originx: f32,
        originy: f32,
        issideways: bool,
        isrighttoleft: bool,
        clientdrawingeffect: P6,
    ) -> windows_core::HRESULT
    where
        P1: windows_core::Param<IDWriteTextRenderer>,
        P6: windows_core::Param<windows_core::IUnknown>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).Draw)(
                windows_core::Interface::as_raw(self),
                clientdrawingcontext.unwrap_or(core::mem::zeroed()) as _,
                renderer.param().abi(),
                originx,
                originy,
                issideways.into(),
                isrighttoleft.into(),
                clientdrawingeffect.param().abi(),
            )
        }
    }
}
#[repr(C)]
pub struct IDWriteInlineObject_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Draw: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const core::ffi::c_void,
        *mut core::ffi::c_void,
        f32,
        f32,
        windows_core::BOOL,
        windows_core::BOOL,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    GetMetrics: usize,
    GetOverhangMetrics: usize,
    GetBreakConditions: usize,
}
windows_core::imp::define_interface!(
    IDWritePixelSnapping,
    IDWritePixelSnapping_Vtbl,
    0xeaf3a2da_ecf4_4d24_b644_b34f6842024b
);
windows_core::imp::interface_hierarchy!(IDWritePixelSnapping, windows_core::IUnknown);
#[repr(C)]
pub struct IDWritePixelSnapping_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub IsPixelSnappingDisabled: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const core::ffi::c_void,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub GetCurrentTransform: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const core::ffi::c_void,
        *mut DWRITE_MATRIX,
    ) -> windows_core::HRESULT,
    pub GetPixelsPerDip: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const core::ffi::c_void,
        *mut f32,
    ) -> windows_core::HRESULT,
}
pub trait IDWritePixelSnapping_Impl: windows_core::IUnknownImpl {
    fn IsPixelSnappingDisabled(
        &self,
        clientdrawingcontext: *const core::ffi::c_void,
    ) -> windows_core::Result<windows_core::BOOL>;
    fn GetCurrentTransform(
        &self,
        clientdrawingcontext: *const core::ffi::c_void,
        transform: *mut DWRITE_MATRIX,
    ) -> windows_core::Result<()>;
    fn GetPixelsPerDip(
        &self,
        clientdrawingcontext: *const core::ffi::c_void,
    ) -> windows_core::Result<f32>;
}
impl IDWritePixelSnapping_Vtbl {
    pub const fn new<Identity: IDWritePixelSnapping_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn IsPixelSnappingDisabled<
            Identity: IDWritePixelSnapping_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            clientdrawingcontext: *const core::ffi::c_void,
            isdisabled: *mut windows_core::BOOL,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IDWritePixelSnapping_Impl::IsPixelSnappingDisabled(
                    this,
                    core::mem::transmute_copy(&clientdrawingcontext),
                ) {
                    Ok(ok__) => {
                        isdisabled.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetCurrentTransform<
            Identity: IDWritePixelSnapping_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            clientdrawingcontext: *const core::ffi::c_void,
            transform: *mut DWRITE_MATRIX,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IDWritePixelSnapping_Impl::GetCurrentTransform(
                    this,
                    core::mem::transmute_copy(&clientdrawingcontext),
                    core::mem::transmute_copy(&transform),
                )
                .into()
            }
        }
        unsafe extern "system" fn GetPixelsPerDip<
            Identity: IDWritePixelSnapping_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            clientdrawingcontext: *const core::ffi::c_void,
            pixelsperdip: *mut f32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IDWritePixelSnapping_Impl::GetPixelsPerDip(
                    this,
                    core::mem::transmute_copy(&clientdrawingcontext),
                ) {
                    Ok(ok__) => {
                        pixelsperdip.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            IsPixelSnappingDisabled: IsPixelSnappingDisabled::<Identity, OFFSET>,
            GetCurrentTransform: GetCurrentTransform::<Identity, OFFSET>,
            GetPixelsPerDip: GetPixelsPerDip::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IDWritePixelSnapping as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IDWritePixelSnapping {}
windows_core::imp::define_interface!(
    IDWriteRemoteFontFileLoader,
    IDWriteRemoteFontFileLoader_Vtbl,
    0x68648c83_6ede_46c0_ab46_20083a887fde
);
impl core::ops::Deref for IDWriteRemoteFontFileLoader {
    type Target = IDWriteFontFileLoader;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteRemoteFontFileLoader,
    windows_core::IUnknown,
    IDWriteFontFileLoader
);
#[repr(C)]
pub struct IDWriteRemoteFontFileLoader_Vtbl {
    pub base__: IDWriteFontFileLoader_Vtbl,
    CreateRemoteStreamFromKey: usize,
    GetLocalityFromKey: usize,
    CreateFontFileReferenceFromUrl: usize,
}
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
windows_core::imp::define_interface!(
    IDWriteRenderingParams1,
    IDWriteRenderingParams1_Vtbl,
    0x94413cf4_a6fc_4248_8b50_6674348fcad3
);
impl core::ops::Deref for IDWriteRenderingParams1 {
    type Target = IDWriteRenderingParams;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteRenderingParams1,
    windows_core::IUnknown,
    IDWriteRenderingParams
);
#[repr(C)]
pub struct IDWriteRenderingParams1_Vtbl {
    pub base__: IDWriteRenderingParams_Vtbl,
    GetGrayscaleEnhancedContrast: usize,
}
windows_core::imp::define_interface!(
    IDWriteRenderingParams2,
    IDWriteRenderingParams2_Vtbl,
    0xf9d711c3_9777_40ae_87e8_3e5af9bf0948
);
impl core::ops::Deref for IDWriteRenderingParams2 {
    type Target = IDWriteRenderingParams1;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteRenderingParams2,
    windows_core::IUnknown,
    IDWriteRenderingParams,
    IDWriteRenderingParams1
);
#[repr(C)]
pub struct IDWriteRenderingParams2_Vtbl {
    pub base__: IDWriteRenderingParams1_Vtbl,
    GetGridFitMode: usize,
}
windows_core::imp::define_interface!(
    IDWriteRenderingParams3,
    IDWriteRenderingParams3_Vtbl,
    0xb7924baa_391b_412a_8c5c_e44cc2d867dc
);
impl core::ops::Deref for IDWriteRenderingParams3 {
    type Target = IDWriteRenderingParams2;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteRenderingParams3,
    windows_core::IUnknown,
    IDWriteRenderingParams,
    IDWriteRenderingParams1,
    IDWriteRenderingParams2
);
#[repr(C)]
pub struct IDWriteRenderingParams3_Vtbl {
    pub base__: IDWriteRenderingParams2_Vtbl,
    GetRenderingMode1: usize,
}
windows_core::imp::define_interface!(
    IDWriteTextFormat,
    IDWriteTextFormat_Vtbl,
    0x9c906818_31d7_4fd3_a151_7c5e225db55a
);
windows_core::imp::interface_hierarchy!(IDWriteTextFormat, windows_core::IUnknown);
impl IDWriteTextFormat {
    pub unsafe fn SetTextAlignment(
        &self,
        textalignment: DWRITE_TEXT_ALIGNMENT,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetTextAlignment)(
                windows_core::Interface::as_raw(self),
                textalignment,
            )
        }
    }
    pub unsafe fn SetParagraphAlignment(
        &self,
        paragraphalignment: DWRITE_PARAGRAPH_ALIGNMENT,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetParagraphAlignment)(
                windows_core::Interface::as_raw(self),
                paragraphalignment,
            )
        }
    }
    pub unsafe fn SetWordWrapping(
        &self,
        wordwrapping: DWRITE_WORD_WRAPPING,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetWordWrapping)(
                windows_core::Interface::as_raw(self),
                wordwrapping,
            )
        }
    }
    pub unsafe fn SetTrimming<P1>(
        &self,
        trimmingoptions: *const DWRITE_TRIMMING,
        trimmingsign: P1,
    ) -> windows_core::HRESULT
    where
        P1: windows_core::Param<IDWriteInlineObject>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetTrimming)(
                windows_core::Interface::as_raw(self),
                trimmingoptions,
                trimmingsign.param().abi(),
            )
        }
    }
}
#[repr(C)]
pub struct IDWriteTextFormat_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub SetTextAlignment: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_TEXT_ALIGNMENT,
    ) -> windows_core::HRESULT,
    pub SetParagraphAlignment: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_PARAGRAPH_ALIGNMENT,
    ) -> windows_core::HRESULT,
    pub SetWordWrapping: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_WORD_WRAPPING,
    ) -> windows_core::HRESULT,
    SetReadingDirection: usize,
    SetFlowDirection: usize,
    SetIncrementalTabStop: usize,
    pub SetTrimming: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const DWRITE_TRIMMING,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
windows_core::imp::define_interface!(
    IDWriteTextFormat1,
    IDWriteTextFormat1_Vtbl,
    0x5f174b49_0d8b_4cfb_8bca_f1cce9d06c67
);
impl core::ops::Deref for IDWriteTextFormat1 {
    type Target = IDWriteTextFormat;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteTextFormat1,
    windows_core::IUnknown,
    IDWriteTextFormat
);
impl IDWriteTextFormat1 {
    pub unsafe fn SetVerticalGlyphOrientation(
        &self,
        glyphorientation: DWRITE_VERTICAL_GLYPH_ORIENTATION,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetVerticalGlyphOrientation)(
                windows_core::Interface::as_raw(self),
                glyphorientation,
            )
        }
    }
    pub unsafe fn GetVerticalGlyphOrientation(&self) -> DWRITE_VERTICAL_GLYPH_ORIENTATION {
        unsafe {
            (windows_core::Interface::vtable(self).GetVerticalGlyphOrientation)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn SetLastLineWrapping(
        &self,
        islastlinewrappingenabled: bool,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetLastLineWrapping)(
                windows_core::Interface::as_raw(self),
                islastlinewrappingenabled.into(),
            )
        }
    }
    pub unsafe fn GetLastLineWrapping(&self) -> windows_core::BOOL {
        unsafe {
            (windows_core::Interface::vtable(self).GetLastLineWrapping)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn SetOpticalAlignment(
        &self,
        opticalalignment: DWRITE_OPTICAL_ALIGNMENT,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetOpticalAlignment)(
                windows_core::Interface::as_raw(self),
                opticalalignment,
            )
        }
    }
    pub unsafe fn GetOpticalAlignment(&self) -> DWRITE_OPTICAL_ALIGNMENT {
        unsafe {
            (windows_core::Interface::vtable(self).GetOpticalAlignment)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn SetFontFallback<P0>(&self, fontfallback: P0) -> windows_core::HRESULT
    where
        P0: windows_core::Param<IDWriteFontFallback>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontFallback)(
                windows_core::Interface::as_raw(self),
                fontfallback.param().abi(),
            )
        }
    }
    pub unsafe fn GetFontFallback(&self) -> windows_core::Result<IDWriteFontFallback> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFontFallback)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDWriteTextFormat1_Vtbl {
    pub base__: IDWriteTextFormat_Vtbl,
    pub SetVerticalGlyphOrientation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_VERTICAL_GLYPH_ORIENTATION,
    ) -> windows_core::HRESULT,
    pub GetVerticalGlyphOrientation:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> DWRITE_VERTICAL_GLYPH_ORIENTATION,
    pub SetLastLineWrapping: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub GetLastLineWrapping:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::BOOL,
    pub SetOpticalAlignment: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_OPTICAL_ALIGNMENT,
    ) -> windows_core::HRESULT,
    pub GetOpticalAlignment:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> DWRITE_OPTICAL_ALIGNMENT,
    pub SetFontFallback: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetFontFallback: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteTextFormat2,
    IDWriteTextFormat2_Vtbl,
    0xf67e0edd_9e3d_4ecc_8c32_4183253dfe70
);
impl core::ops::Deref for IDWriteTextFormat2 {
    type Target = IDWriteTextFormat1;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteTextFormat2,
    windows_core::IUnknown,
    IDWriteTextFormat,
    IDWriteTextFormat1
);
impl IDWriteTextFormat2 {
    pub unsafe fn SetLineSpacing(
        &self,
        linespacingoptions: *const DWRITE_LINE_SPACING,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetLineSpacing)(
                windows_core::Interface::as_raw(self),
                linespacingoptions,
            )
        }
    }
    pub unsafe fn GetLineSpacing(
        &self,
        linespacingoptions: *mut DWRITE_LINE_SPACING,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetLineSpacing)(
                windows_core::Interface::as_raw(self),
                linespacingoptions as _,
            )
        }
    }
}
#[repr(C)]
pub struct IDWriteTextFormat2_Vtbl {
    pub base__: IDWriteTextFormat1_Vtbl,
    pub SetLineSpacing: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const DWRITE_LINE_SPACING,
    ) -> windows_core::HRESULT,
    pub GetLineSpacing: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut DWRITE_LINE_SPACING,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteTextFormat3,
    IDWriteTextFormat3_Vtbl,
    0x6d3b5641_e550_430d_a85b_b7bf48a93427
);
impl core::ops::Deref for IDWriteTextFormat3 {
    type Target = IDWriteTextFormat2;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteTextFormat3,
    windows_core::IUnknown,
    IDWriteTextFormat,
    IDWriteTextFormat1,
    IDWriteTextFormat2
);
impl IDWriteTextFormat3 {
    pub unsafe fn SetFontAxisValues(
        &self,
        fontaxisvalues: &[DWRITE_FONT_AXIS_VALUE],
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontAxisValues)(
                windows_core::Interface::as_raw(self),
                fontaxisvalues.as_ptr(),
                fontaxisvalues.len().try_into().unwrap(),
            )
        }
    }
    pub unsafe fn SetAutomaticFontAxes(
        &self,
        automaticfontaxes: DWRITE_AUTOMATIC_FONT_AXES,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetAutomaticFontAxes)(
                windows_core::Interface::as_raw(self),
                automaticfontaxes,
            )
        }
    }
}
#[repr(C)]
pub struct IDWriteTextFormat3_Vtbl {
    pub base__: IDWriteTextFormat2_Vtbl,
    pub SetFontAxisValues: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const DWRITE_FONT_AXIS_VALUE,
        u32,
    ) -> windows_core::HRESULT,
    GetFontAxisValueCount: usize,
    GetFontAxisValues: usize,
    GetAutomaticFontAxes: usize,
    pub SetAutomaticFontAxes: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_AUTOMATIC_FONT_AXES,
    ) -> windows_core::HRESULT,
}
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
impl IDWriteTextLayout {
    pub unsafe fn SetMaxWidth(&self, maxwidth: f32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetMaxWidth)(
                windows_core::Interface::as_raw(self),
                maxwidth,
            )
        }
    }
    pub unsafe fn SetMaxHeight(&self, maxheight: f32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetMaxHeight)(
                windows_core::Interface::as_raw(self),
                maxheight,
            )
        }
    }
    pub unsafe fn SetFontWeight(
        &self,
        fontweight: DWRITE_FONT_WEIGHT,
        textrange: DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontWeight)(
                windows_core::Interface::as_raw(self),
                fontweight,
                textrange,
            )
        }
    }
    pub unsafe fn SetFontStyle(
        &self,
        fontstyle: DWRITE_FONT_STYLE,
        textrange: DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontStyle)(
                windows_core::Interface::as_raw(self),
                fontstyle,
                textrange,
            )
        }
    }
    pub unsafe fn SetFontStretch(
        &self,
        fontstretch: DWRITE_FONT_STRETCH,
        textrange: DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontStretch)(
                windows_core::Interface::as_raw(self),
                fontstretch,
                textrange,
            )
        }
    }
    pub unsafe fn SetUnderline(
        &self,
        hasunderline: bool,
        textrange: DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetUnderline)(
                windows_core::Interface::as_raw(self),
                hasunderline.into(),
                textrange,
            )
        }
    }
    pub unsafe fn SetStrikethrough(
        &self,
        hasstrikethrough: bool,
        textrange: DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetStrikethrough)(
                windows_core::Interface::as_raw(self),
                hasstrikethrough.into(),
                textrange,
            )
        }
    }
    pub unsafe fn SetDrawingEffect<P0>(
        &self,
        drawingeffect: P0,
        textrange: DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT
    where
        P0: windows_core::Param<windows_core::IUnknown>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetDrawingEffect)(
                windows_core::Interface::as_raw(self),
                drawingeffect.param().abi(),
                textrange,
            )
        }
    }
    pub unsafe fn Draw<P1>(
        &self,
        clientdrawingcontext: Option<*const core::ffi::c_void>,
        renderer: P1,
        originx: f32,
        originy: f32,
    ) -> windows_core::HRESULT
    where
        P1: windows_core::Param<IDWriteTextRenderer>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).Draw)(
                windows_core::Interface::as_raw(self),
                clientdrawingcontext.unwrap_or(core::mem::zeroed()) as _,
                renderer.param().abi(),
                originx,
                originy,
            )
        }
    }
    pub unsafe fn GetMetrics(
        &self,
        textmetrics: *mut DWRITE_TEXT_METRICS,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetMetrics)(
                windows_core::Interface::as_raw(self),
                textmetrics as _,
            )
        }
    }
    pub unsafe fn HitTestPoint(
        &self,
        pointx: f32,
        pointy: f32,
        istrailinghit: *mut windows_core::BOOL,
        isinside: *mut windows_core::BOOL,
        hittestmetrics: *mut DWRITE_HIT_TEST_METRICS,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).HitTestPoint)(
                windows_core::Interface::as_raw(self),
                pointx,
                pointy,
                istrailinghit as _,
                isinside as _,
                hittestmetrics as _,
            )
        }
    }
    pub unsafe fn HitTestTextPosition(
        &self,
        textposition: u32,
        istrailinghit: bool,
        pointx: *mut f32,
        pointy: *mut f32,
        hittestmetrics: *mut DWRITE_HIT_TEST_METRICS,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).HitTestTextPosition)(
                windows_core::Interface::as_raw(self),
                textposition,
                istrailinghit.into(),
                pointx as _,
                pointy as _,
                hittestmetrics as _,
            )
        }
    }
    pub unsafe fn HitTestTextRange(
        &self,
        textposition: u32,
        textlength: u32,
        originx: f32,
        originy: f32,
        hittestmetrics: Option<&mut [DWRITE_HIT_TEST_METRICS]>,
        actualhittestmetricscount: *mut u32,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).HitTestTextRange)(
                windows_core::Interface::as_raw(self),
                textposition,
                textlength,
                originx,
                originy,
                hittestmetrics
                    .as_deref()
                    .map_or(core::ptr::null_mut(), |slice| slice.as_ptr().cast_mut()),
                hittestmetrics
                    .as_deref()
                    .map_or(0, |slice| slice.len().try_into().unwrap()),
                actualhittestmetricscount as _,
            )
        }
    }
}
#[repr(C)]
pub struct IDWriteTextLayout_Vtbl {
    pub base__: IDWriteTextFormat_Vtbl,
    pub SetMaxWidth:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    pub SetMaxHeight:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    SetFontCollection: usize,
    SetFontFamilyName: usize,
    pub SetFontWeight: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_FONT_WEIGHT,
        DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT,
    pub SetFontStyle: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_FONT_STYLE,
        DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT,
    pub SetFontStretch: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_FONT_STRETCH,
        DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT,
    SetFontSize: usize,
    pub SetUnderline: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::BOOL,
        DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT,
    pub SetStrikethrough: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::BOOL,
        DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT,
    pub SetDrawingEffect: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT,
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
    pub Draw: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const core::ffi::c_void,
        *mut core::ffi::c_void,
        f32,
        f32,
    ) -> windows_core::HRESULT,
    GetLineMetrics: usize,
    pub GetMetrics: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut DWRITE_TEXT_METRICS,
    ) -> windows_core::HRESULT,
    GetOverhangMetrics: usize,
    GetClusterMetrics: usize,
    DetermineMinWidth: usize,
    pub HitTestPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        f32,
        *mut windows_core::BOOL,
        *mut windows_core::BOOL,
        *mut DWRITE_HIT_TEST_METRICS,
    ) -> windows_core::HRESULT,
    pub HitTestTextPosition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        windows_core::BOOL,
        *mut f32,
        *mut f32,
        *mut DWRITE_HIT_TEST_METRICS,
    ) -> windows_core::HRESULT,
    pub HitTestTextRange: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        u32,
        f32,
        f32,
        *mut DWRITE_HIT_TEST_METRICS,
        u32,
        *mut u32,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteTextLayout1,
    IDWriteTextLayout1_Vtbl,
    0x9064d822_80a7_465c_a986_df65f78b8feb
);
impl core::ops::Deref for IDWriteTextLayout1 {
    type Target = IDWriteTextLayout;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteTextLayout1,
    windows_core::IUnknown,
    IDWriteTextFormat,
    IDWriteTextLayout
);
impl IDWriteTextLayout1 {
    pub unsafe fn SetCharacterSpacing(
        &self,
        leadingspacing: f32,
        trailingspacing: f32,
        minimumadvancewidth: f32,
        textrange: DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetCharacterSpacing)(
                windows_core::Interface::as_raw(self),
                leadingspacing,
                trailingspacing,
                minimumadvancewidth,
                textrange,
            )
        }
    }
}
#[repr(C)]
pub struct IDWriteTextLayout1_Vtbl {
    pub base__: IDWriteTextLayout_Vtbl,
    SetPairKerning: usize,
    GetPairKerning: usize,
    pub SetCharacterSpacing: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        f32,
        f32,
        DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT,
    GetCharacterSpacing: usize,
}
windows_core::imp::define_interface!(
    IDWriteTextLayout2,
    IDWriteTextLayout2_Vtbl,
    0x1093c18f_8d5e_43f0_b064_0917311b525e
);
impl core::ops::Deref for IDWriteTextLayout2 {
    type Target = IDWriteTextLayout1;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteTextLayout2,
    windows_core::IUnknown,
    IDWriteTextFormat,
    IDWriteTextLayout,
    IDWriteTextLayout1
);
impl IDWriteTextLayout2 {
    pub unsafe fn GetMetrics(
        &self,
        textmetrics: *mut DWRITE_TEXT_METRICS1,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetMetrics)(
                windows_core::Interface::as_raw(self),
                textmetrics as _,
            )
        }
    }
    pub unsafe fn SetVerticalGlyphOrientation(
        &self,
        glyphorientation: DWRITE_VERTICAL_GLYPH_ORIENTATION,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetVerticalGlyphOrientation)(
                windows_core::Interface::as_raw(self),
                glyphorientation,
            )
        }
    }
    pub unsafe fn GetVerticalGlyphOrientation(&self) -> DWRITE_VERTICAL_GLYPH_ORIENTATION {
        unsafe {
            (windows_core::Interface::vtable(self).GetVerticalGlyphOrientation)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn SetLastLineWrapping(
        &self,
        islastlinewrappingenabled: bool,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetLastLineWrapping)(
                windows_core::Interface::as_raw(self),
                islastlinewrappingenabled.into(),
            )
        }
    }
    pub unsafe fn GetLastLineWrapping(&self) -> windows_core::BOOL {
        unsafe {
            (windows_core::Interface::vtable(self).GetLastLineWrapping)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn SetOpticalAlignment(
        &self,
        opticalalignment: DWRITE_OPTICAL_ALIGNMENT,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetOpticalAlignment)(
                windows_core::Interface::as_raw(self),
                opticalalignment,
            )
        }
    }
    pub unsafe fn GetOpticalAlignment(&self) -> DWRITE_OPTICAL_ALIGNMENT {
        unsafe {
            (windows_core::Interface::vtable(self).GetOpticalAlignment)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn SetFontFallback<P0>(&self, fontfallback: P0) -> windows_core::HRESULT
    where
        P0: windows_core::Param<IDWriteFontFallback>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontFallback)(
                windows_core::Interface::as_raw(self),
                fontfallback.param().abi(),
            )
        }
    }
    pub unsafe fn GetFontFallback(&self) -> windows_core::Result<IDWriteFontFallback> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFontFallback)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDWriteTextLayout2_Vtbl {
    pub base__: IDWriteTextLayout1_Vtbl,
    pub GetMetrics: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut DWRITE_TEXT_METRICS1,
    ) -> windows_core::HRESULT,
    pub SetVerticalGlyphOrientation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_VERTICAL_GLYPH_ORIENTATION,
    ) -> windows_core::HRESULT,
    pub GetVerticalGlyphOrientation:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> DWRITE_VERTICAL_GLYPH_ORIENTATION,
    pub SetLastLineWrapping: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub GetLastLineWrapping:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::BOOL,
    pub SetOpticalAlignment: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_OPTICAL_ALIGNMENT,
    ) -> windows_core::HRESULT,
    pub GetOpticalAlignment:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> DWRITE_OPTICAL_ALIGNMENT,
    pub SetFontFallback: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetFontFallback: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteTextLayout3,
    IDWriteTextLayout3_Vtbl,
    0x07ddcd52_020e_4de8_ac33_6c953d83f92d
);
impl core::ops::Deref for IDWriteTextLayout3 {
    type Target = IDWriteTextLayout2;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteTextLayout3,
    windows_core::IUnknown,
    IDWriteTextFormat,
    IDWriteTextLayout,
    IDWriteTextLayout1,
    IDWriteTextLayout2
);
impl IDWriteTextLayout3 {
    pub unsafe fn InvalidateLayout(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).InvalidateLayout)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn SetLineSpacing(
        &self,
        linespacingoptions: *const DWRITE_LINE_SPACING,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetLineSpacing)(
                windows_core::Interface::as_raw(self),
                linespacingoptions,
            )
        }
    }
    pub unsafe fn GetLineSpacing(
        &self,
        linespacingoptions: *mut DWRITE_LINE_SPACING,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetLineSpacing)(
                windows_core::Interface::as_raw(self),
                linespacingoptions as _,
            )
        }
    }
    pub unsafe fn GetLineMetrics(
        &self,
        linemetrics: Option<&mut [DWRITE_LINE_METRICS1]>,
        actuallinecount: *mut u32,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetLineMetrics)(
                windows_core::Interface::as_raw(self),
                linemetrics
                    .as_deref()
                    .map_or(core::ptr::null_mut(), |slice| slice.as_ptr().cast_mut()),
                linemetrics
                    .as_deref()
                    .map_or(0, |slice| slice.len().try_into().unwrap()),
                actuallinecount as _,
            )
        }
    }
}
#[repr(C)]
pub struct IDWriteTextLayout3_Vtbl {
    pub base__: IDWriteTextLayout2_Vtbl,
    pub InvalidateLayout:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetLineSpacing: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const DWRITE_LINE_SPACING,
    ) -> windows_core::HRESULT,
    pub GetLineSpacing: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut DWRITE_LINE_SPACING,
    ) -> windows_core::HRESULT,
    pub GetLineMetrics: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut DWRITE_LINE_METRICS1,
        u32,
        *mut u32,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteTextLayout4,
    IDWriteTextLayout4_Vtbl,
    0x05a9bf42_223f_4441_b5fb_8263685f55e9
);
impl core::ops::Deref for IDWriteTextLayout4 {
    type Target = IDWriteTextLayout3;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteTextLayout4,
    windows_core::IUnknown,
    IDWriteTextFormat,
    IDWriteTextLayout,
    IDWriteTextLayout1,
    IDWriteTextLayout2,
    IDWriteTextLayout3
);
impl IDWriteTextLayout4 {
    pub unsafe fn SetFontAxisValues(
        &self,
        fontaxisvalues: &[DWRITE_FONT_AXIS_VALUE],
        textrange: DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetFontAxisValues)(
                windows_core::Interface::as_raw(self),
                fontaxisvalues.as_ptr(),
                fontaxisvalues.len().try_into().unwrap(),
                textrange,
            )
        }
    }
    pub unsafe fn GetFontAxisValueCount(&self, currentposition: u32) -> u32 {
        unsafe {
            (windows_core::Interface::vtable(self).GetFontAxisValueCount)(
                windows_core::Interface::as_raw(self),
                currentposition,
            )
        }
    }
    pub unsafe fn GetFontAxisValues(
        &self,
        currentposition: u32,
        fontaxisvalues: &mut [DWRITE_FONT_AXIS_VALUE],
        textrange: Option<*mut DWRITE_TEXT_RANGE>,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetFontAxisValues)(
                windows_core::Interface::as_raw(self),
                currentposition,
                fontaxisvalues.as_mut_ptr(),
                fontaxisvalues.len().try_into().unwrap(),
                textrange.unwrap_or(core::mem::zeroed()) as _,
            )
        }
    }
    pub unsafe fn GetAutomaticFontAxes(&self) -> DWRITE_AUTOMATIC_FONT_AXES {
        unsafe {
            (windows_core::Interface::vtable(self).GetAutomaticFontAxes)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn SetAutomaticFontAxes(
        &self,
        automaticfontaxes: DWRITE_AUTOMATIC_FONT_AXES,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetAutomaticFontAxes)(
                windows_core::Interface::as_raw(self),
                automaticfontaxes,
            )
        }
    }
}
#[repr(C)]
pub struct IDWriteTextLayout4_Vtbl {
    pub base__: IDWriteTextLayout3_Vtbl,
    pub SetFontAxisValues: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const DWRITE_FONT_AXIS_VALUE,
        u32,
        DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT,
    pub GetFontAxisValueCount: unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> u32,
    pub GetFontAxisValues: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut DWRITE_FONT_AXIS_VALUE,
        u32,
        *mut DWRITE_TEXT_RANGE,
    ) -> windows_core::HRESULT,
    pub GetAutomaticFontAxes:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> DWRITE_AUTOMATIC_FONT_AXES,
    pub SetAutomaticFontAxes: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        DWRITE_AUTOMATIC_FONT_AXES,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDWriteTextRenderer,
    IDWriteTextRenderer_Vtbl,
    0xef8a8135_5cc6_45fe_8825_c5a0724eb819
);
impl core::ops::Deref for IDWriteTextRenderer {
    type Target = IDWritePixelSnapping;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IDWriteTextRenderer,
    windows_core::IUnknown,
    IDWritePixelSnapping
);
#[repr(C)]
pub struct IDWriteTextRenderer_Vtbl {
    pub base__: IDWritePixelSnapping_Vtbl,
    pub DrawGlyphRun: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const core::ffi::c_void,
        f32,
        f32,
        DWRITE_MEASURING_MODE,
        *const DWRITE_GLYPH_RUN,
        *const DWRITE_GLYPH_RUN_DESCRIPTION,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub DrawUnderline: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const core::ffi::c_void,
        f32,
        f32,
        *const DWRITE_UNDERLINE,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub DrawStrikethrough: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const core::ffi::c_void,
        f32,
        f32,
        *const DWRITE_STRIKETHROUGH,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub DrawInlineObject: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const core::ffi::c_void,
        f32,
        f32,
        *mut core::ffi::c_void,
        windows_core::BOOL,
        windows_core::BOOL,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
pub trait IDWriteTextRenderer_Impl: IDWritePixelSnapping_Impl {
    fn DrawGlyphRun(
        &self,
        clientdrawingcontext: *const core::ffi::c_void,
        baselineoriginx: f32,
        baselineoriginy: f32,
        measuringmode: DWRITE_MEASURING_MODE,
        glyphrun: *const DWRITE_GLYPH_RUN,
        glyphrundescription: *const DWRITE_GLYPH_RUN_DESCRIPTION,
        clientdrawingeffect: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()>;
    fn DrawUnderline(
        &self,
        clientdrawingcontext: *const core::ffi::c_void,
        baselineoriginx: f32,
        baselineoriginy: f32,
        underline: *const DWRITE_UNDERLINE,
        clientdrawingeffect: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()>;
    fn DrawStrikethrough(
        &self,
        clientdrawingcontext: *const core::ffi::c_void,
        baselineoriginx: f32,
        baselineoriginy: f32,
        strikethrough: *const DWRITE_STRIKETHROUGH,
        clientdrawingeffect: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()>;
    fn DrawInlineObject(
        &self,
        clientdrawingcontext: *const core::ffi::c_void,
        originx: f32,
        originy: f32,
        inlineobject: windows_core::Ref<IDWriteInlineObject>,
        issideways: windows_core::BOOL,
        isrighttoleft: windows_core::BOOL,
        clientdrawingeffect: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()>;
}
impl IDWriteTextRenderer_Vtbl {
    pub const fn new<Identity: IDWriteTextRenderer_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn DrawGlyphRun<
            Identity: IDWriteTextRenderer_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            clientdrawingcontext: *const core::ffi::c_void,
            baselineoriginx: f32,
            baselineoriginy: f32,
            measuringmode: DWRITE_MEASURING_MODE,
            glyphrun: *const DWRITE_GLYPH_RUN,
            glyphrundescription: *const DWRITE_GLYPH_RUN_DESCRIPTION,
            clientdrawingeffect: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IDWriteTextRenderer_Impl::DrawGlyphRun(
                    this,
                    core::mem::transmute_copy(&clientdrawingcontext),
                    core::mem::transmute_copy(&baselineoriginx),
                    core::mem::transmute_copy(&baselineoriginy),
                    core::mem::transmute_copy(&measuringmode),
                    core::mem::transmute_copy(&glyphrun),
                    core::mem::transmute_copy(&glyphrundescription),
                    core::mem::transmute_copy(&clientdrawingeffect),
                )
                .into()
            }
        }
        unsafe extern "system" fn DrawUnderline<
            Identity: IDWriteTextRenderer_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            clientdrawingcontext: *const core::ffi::c_void,
            baselineoriginx: f32,
            baselineoriginy: f32,
            underline: *const DWRITE_UNDERLINE,
            clientdrawingeffect: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IDWriteTextRenderer_Impl::DrawUnderline(
                    this,
                    core::mem::transmute_copy(&clientdrawingcontext),
                    core::mem::transmute_copy(&baselineoriginx),
                    core::mem::transmute_copy(&baselineoriginy),
                    core::mem::transmute_copy(&underline),
                    core::mem::transmute_copy(&clientdrawingeffect),
                )
                .into()
            }
        }
        unsafe extern "system" fn DrawStrikethrough<
            Identity: IDWriteTextRenderer_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            clientdrawingcontext: *const core::ffi::c_void,
            baselineoriginx: f32,
            baselineoriginy: f32,
            strikethrough: *const DWRITE_STRIKETHROUGH,
            clientdrawingeffect: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IDWriteTextRenderer_Impl::DrawStrikethrough(
                    this,
                    core::mem::transmute_copy(&clientdrawingcontext),
                    core::mem::transmute_copy(&baselineoriginx),
                    core::mem::transmute_copy(&baselineoriginy),
                    core::mem::transmute_copy(&strikethrough),
                    core::mem::transmute_copy(&clientdrawingeffect),
                )
                .into()
            }
        }
        unsafe extern "system" fn DrawInlineObject<
            Identity: IDWriteTextRenderer_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            clientdrawingcontext: *const core::ffi::c_void,
            originx: f32,
            originy: f32,
            inlineobject: *mut core::ffi::c_void,
            issideways: windows_core::BOOL,
            isrighttoleft: windows_core::BOOL,
            clientdrawingeffect: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IDWriteTextRenderer_Impl::DrawInlineObject(
                    this,
                    core::mem::transmute_copy(&clientdrawingcontext),
                    core::mem::transmute_copy(&originx),
                    core::mem::transmute_copy(&originy),
                    core::mem::transmute_copy(&inlineobject),
                    core::mem::transmute_copy(&issideways),
                    core::mem::transmute_copy(&isrighttoleft),
                    core::mem::transmute_copy(&clientdrawingeffect),
                )
                .into()
            }
        }
        Self {
            base__: IDWritePixelSnapping_Vtbl::new::<Identity, OFFSET>(),
            DrawGlyphRun: DrawGlyphRun::<Identity, OFFSET>,
            DrawUnderline: DrawUnderline::<Identity, OFFSET>,
            DrawStrikethrough: DrawStrikethrough::<Identity, OFFSET>,
            DrawInlineObject: DrawInlineObject::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IDWriteTextRenderer as windows_core::Interface>::IID
            || iid == &<IDWritePixelSnapping as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IDWriteTextRenderer {}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
