windows_core::link!("user32.dll" "system" fn BeginPaint(hwnd : HWND, lppaint : *mut PAINTSTRUCT) -> HDC);
windows_core::link!("user32.dll" "system" fn ClientToScreen(hwnd : HWND, lppoint : *mut POINT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn CloseClipboard() -> windows_core::BOOL);
windows_core::link!("coremessaging.dll" "system" fn CreateDispatcherQueueController(options : DispatcherQueueOptions, dispatcherqueuecontroller : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("user32.dll" "system" fn CreateWindowExW(dwexstyle : WINDOW_EX_STYLE, lpclassname : windows_core::PCWSTR, lpwindowname : windows_core::PCWSTR, dwstyle : WINDOW_STYLE, x : i32, y : i32, nwidth : i32, nheight : i32, hwndparent : HWND, hmenu : HMENU, hinstance : HINSTANCE, lpparam : *const core::ffi::c_void) -> HWND);
windows_core::link!("user32.dll" "system" fn DefWindowProcW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> LRESULT);
windows_core::link!("user32.dll" "system" fn DestroyWindow(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn DispatchMessageW(lpmsg : *const MSG) -> LRESULT);
windows_core::link!("user32.dll" "system" fn DisplayConfigGetDeviceInfo(requestpacket : *mut DISPLAYCONFIG_DEVICE_INFO_HEADER) -> i32);
windows_core::link!("user32.dll" "system" fn EmptyClipboard() -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn EnableMouseInPointer(fenable : windows_core::BOOL) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn EndPaint(hwnd : HWND, lppaint : *const PAINTSTRUCT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetAsyncKeyState(vkey : i32) -> i16);
windows_core::link!("user32.dll" "system" fn GetCapture() -> HWND);
windows_core::link!("user32.dll" "system" fn GetCaretBlinkTime() -> u32);
windows_core::link!("user32.dll" "system" fn GetClientRect(hwnd : HWND, lprect : *mut RECT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetClipboardData(uformat : u32) -> HANDLE);
windows_core::link!("user32.dll" "system" fn GetDisplayConfigBufferSizes(flags : QUERY_DISPLAY_CONFIG_FLAGS, numpatharrayelements : *mut u32, nummodeinfoarrayelements : *mut u32) -> windows_core::WIN32_ERROR);
windows_core::link!("user32.dll" "system" fn GetDpiForWindow(hwnd : HWND) -> u32);
windows_core::link!("user32.dll" "system" fn GetKeyState(nvirtkey : i32) -> i16);
windows_core::link!("user32.dll" "system" fn GetMessageW(lpmsg : *mut MSG, hwnd : HWND, wmsgfiltermin : u32, wmsgfiltermax : u32) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn GetModuleHandleW(lpmodulename : windows_core::PCWSTR) -> HMODULE);
windows_core::link!("user32.dll" "system" fn GetMonitorInfoW(hmonitor : HMONITOR, lpmi : *mut MONITORINFO) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetPointerInfo(pointerid : u32, pointerinfo : *mut POINTER_INFO) -> windows_core::BOOL);
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "x86_64"
))]
windows_core::link!("user32.dll" "system" fn GetWindowLongPtrW(hwnd : HWND, nindex : WINDOW_LONG_PTR_INDEX) -> isize);
#[cfg(target_pointer_width = "32")]
pub use GetWindowLongW as GetWindowLongPtrW;
windows_core::link!("user32.dll" "system" fn GetWindowLongW(hwnd : HWND, nindex : WINDOW_LONG_PTR_INDEX) -> i32);
windows_core::link!("user32.dll" "system" fn GetWindowRect(hwnd : HWND, lprect : *mut RECT) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn GlobalAlloc(uflags : GLOBAL_ALLOC_FLAGS, dwbytes : usize) -> HGLOBAL);
windows_core::link!("kernel32.dll" "system" fn GlobalFree(hmem : HGLOBAL) -> HGLOBAL);
windows_core::link!("kernel32.dll" "system" fn GlobalLock(hmem : HGLOBAL) -> *mut core::ffi::c_void);
windows_core::link!("kernel32.dll" "system" fn GlobalUnlock(hmem : HGLOBAL) -> windows_core::BOOL);
windows_core::link!("imm32.dll" "system" fn ImmGetCompositionStringW(param0 : HIMC, param1 : IME_COMPOSITION_STRING, lpbuf : *mut core::ffi::c_void, dwbuflen : u32) -> i32);
windows_core::link!("imm32.dll" "system" fn ImmGetContext(param0 : HWND) -> HIMC);
windows_core::link!("imm32.dll" "system" fn ImmGetDefaultIMEWnd(param0 : HWND) -> HWND);
windows_core::link!("imm32.dll" "system" fn ImmNotifyIME(param0 : HIMC, dwaction : NOTIFY_IME_ACTION, dwindex : NOTIFY_IME_INDEX, dwvalue : u32) -> windows_core::BOOL);
windows_core::link!("imm32.dll" "system" fn ImmReleaseContext(param0 : HWND, param1 : HIMC) -> windows_core::BOOL);
windows_core::link!("imm32.dll" "system" fn ImmSetCompositionWindow(param0 : HIMC, lpcompform : *const COMPOSITIONFORM) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn KillTimer(hwnd : HWND, uidevent : usize) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn LoadCursorW(hinstance : HINSTANCE, lpcursorname : windows_core::PCWSTR) -> HCURSOR);
windows_core::link!("user32.dll" "system" fn MonitorFromPoint(pt : POINT, dwflags : MONITOR_FROM_FLAGS) -> HMONITOR);
windows_core::link!("user32.dll" "system" fn MonitorFromWindow(hwnd : HWND, dwflags : MONITOR_FROM_FLAGS) -> HMONITOR);
windows_core::link!("user32.dll" "system" fn OpenClipboard(hwndnewowner : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn PeekMessageW(lpmsg : *mut MSG, hwnd : HWND, wmsgfiltermin : u32, wmsgfiltermax : u32, wremovemsg : PEEK_MESSAGE_REMOVE_TYPE) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn PostMessageW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn PostQuitMessage(nexitcode : i32));
windows_core::link!("user32.dll" "system" fn QueryDisplayConfig(flags : QUERY_DISPLAY_CONFIG_FLAGS, numpatharrayelements : *mut u32, patharray : *mut DISPLAYCONFIG_PATH_INFO, nummodeinfoarrayelements : *mut u32, modeinfoarray : *mut DISPLAYCONFIG_MODE_INFO, currenttopologyid : *mut DISPLAYCONFIG_TOPOLOGY_ID) -> windows_core::WIN32_ERROR);
windows_core::link!("user32.dll" "system" fn RegisterClassExW(param0 : *const WNDCLASSEXW) -> u16);
windows_core::link!("user32.dll" "system" fn RegisterClassW(lpwndclass : *const WNDCLASSW) -> u16);
windows_core::link!("user32.dll" "system" fn ReleaseCapture() -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ScreenToClient(hwnd : HWND, lppoint : *mut POINT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn SendMessageW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> LRESULT);
windows_core::link!("user32.dll" "system" fn SetCapture(hwnd : HWND) -> HWND);
windows_core::link!("user32.dll" "system" fn SetClipboardData(uformat : u32, hmem : HANDLE) -> HANDLE);
windows_core::link!("user32.dll" "system" fn SetCursor(hcursor : HCURSOR) -> HCURSOR);
windows_core::link!("user32.dll" "system" fn SetProcessDpiAwarenessContext(value : DPI_AWARENESS_CONTEXT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn SetTimer(hwnd : HWND, nidevent : usize, uelapse : u32, lptimerfunc : TIMERPROC) -> usize);
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "x86_64"
))]
windows_core::link!("user32.dll" "system" fn SetWindowLongPtrW(hwnd : HWND, nindex : WINDOW_LONG_PTR_INDEX, dwnewlong : isize) -> isize);
#[cfg(target_pointer_width = "32")]
pub use SetWindowLongW as SetWindowLongPtrW;
windows_core::link!("user32.dll" "system" fn SetWindowLongW(hwnd : HWND, nindex : WINDOW_LONG_PTR_INDEX, dwnewlong : i32) -> i32);
windows_core::link!("user32.dll" "system" fn SetWindowPos(hwnd : HWND, hwndinsertafter : HWND, x : i32, y : i32, cx : i32, cy : i32, uflags : SET_WINDOW_POS_FLAGS) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ShowWindow(hwnd : HWND, ncmdshow : SHOW_WINDOW_CMD) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn TrackMouseEvent(lpeventtrack : *mut TRACKMOUSEEVENT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn TranslateMessage(lpmsg : *const MSG) -> windows_core::BOOL);
windows_core::link!("uiautomationcore.dll" "system" fn UiaDisconnectProvider(pprovider : *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaHostProviderFromHwnd(hwnd : HWND, ppprovider : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaRaiseAutomationEvent(pprovider : *mut core::ffi::c_void, id : UIA_EVENT_ID) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaRaiseAutomationPropertyChangedEvent(pprovider : *mut core::ffi::c_void, id : UIA_PROPERTY_ID, oldvalue : VARIANT, newvalue : VARIANT) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaReturnRawElementProvider(hwnd : HWND, wparam : WPARAM, lparam : LPARAM, el : *mut core::ffi::c_void) -> LRESULT);
windows_core::link!("user32.dll" "system" fn UnregisterClassW(lpclassname : windows_core::PCWSTR, hinstance : HINSTANCE) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn UpdateWindow(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ValidateRect(hwnd : HWND, lprect : *const RECT) -> windows_core::BOOL);
pub type ADVANCED_FEATURE_FLAGS = u16;
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AnimationIterationBehavior(pub i32);
impl AnimationIterationBehavior {
    pub const Count: Self = Self(0);
    pub const Forever: Self = Self(1);
}
impl windows_core::TypeKind for AnimationIterationBehavior {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for AnimationIterationBehavior {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.AnimationIterationBehavior;i4)",
    );
}
pub const CF_UNICODETEXT: CLIPBOARD_FORMAT = 13;
pub type CLIPBOARD_FORMAT = u16;
pub const CLSID_D2D1Exposure: windows_core::GUID =
    windows_core::GUID::from_u128(0xb56c8cfa_f634_41ee_bee0_ffa617106004);
pub const CLSID_TF_ThreadMgr: windows_core::GUID =
    windows_core::GUID::from_u128(0x529a9e6b_6587_4f23_ab9e_9c7d683e3c50);
pub type COLORREF = u32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct COMPOSITIONFORM {
    pub dwStyle: u32,
    pub ptCurrentPos: POINT,
    pub rcArea: RECT,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CREATESTRUCTW {
    pub lpCreateParams: *mut core::ffi::c_void,
    pub hInstance: HINSTANCE,
    pub hMenu: HMENU,
    pub hwndParent: HWND,
    pub cy: i32,
    pub cx: i32,
    pub y: i32,
    pub x: i32,
    pub style: i32,
    pub lpszName: windows_core::PCWSTR,
    pub lpszClass: windows_core::PCWSTR,
    pub dwExStyle: WINDOW_EX_STYLE,
}
impl Default for CREATESTRUCTW {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub const CS_DBLCLKS: WNDCLASS_STYLES = 8;
pub const CS_HREDRAW: WNDCLASS_STYLES = 2;
pub const CS_OWNDC: WNDCLASS_STYLES = 32;
pub const CS_VREDRAW: WNDCLASS_STYLES = 1;
pub const CW_USEDEFAULT: i32 = -2147483648;
#[repr(C)]
#[derive(Clone, Copy)]
pub union CY {
    pub Anonymous: CY_0,
    pub int64: i64,
}
impl Default for CY {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CY_0 {
    pub Lo: u32,
    pub Hi: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Color {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
impl windows_core::TypeKind for Color {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for Color {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.UI.Color;u1;u1;u1;u1)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionAnimation, CompositionObject);
impl windows_core::RuntimeType for CompositionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionAnimation>();
}
unsafe impl windows_core::Interface for CompositionAnimation {
    type Vtable = <ICompositionAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionAnimation {
    type Target = ICompositionAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionAnimation {
    const NAME: &'static str = "Windows.UI.Composition.CompositionAnimation";
}
unsafe impl Send for CompositionAnimation {}
unsafe impl Sync for CompositionAnimation {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionBrush(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionBrush,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionBrush, CompositionObject);
impl windows_core::RuntimeType for CompositionBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionBrush>();
}
unsafe impl windows_core::Interface for CompositionBrush {
    type Vtable = <ICompositionBrush as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionBrush as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionBrush {
    type Target = ICompositionBrush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionBrush {
    const NAME: &'static str = "Windows.UI.Composition.CompositionBrush";
}
unsafe impl Send for CompositionBrush {}
unsafe impl Sync for CompositionBrush {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionClip(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionClip,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionClip, CompositionObject);
impl windows_core::RuntimeType for CompositionClip {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionClip>();
}
unsafe impl windows_core::Interface for CompositionClip {
    type Vtable = <ICompositionClip as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionClip as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionClip {
    type Target = ICompositionClip;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionClip {
    const NAME: &'static str = "Windows.UI.Composition.CompositionClip";
}
unsafe impl Send for CompositionClip {}
unsafe impl Sync for CompositionClip {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionColorBrush(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionColorBrush,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionColorBrush, CompositionBrush, CompositionObject);
impl windows_core::RuntimeType for CompositionColorBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionColorBrush>();
}
unsafe impl windows_core::Interface for CompositionColorBrush {
    type Vtable = <ICompositionColorBrush as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionColorBrush as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionColorBrush {
    type Target = ICompositionColorBrush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionColorBrush {
    const NAME: &'static str = "Windows.UI.Composition.CompositionColorBrush";
}
unsafe impl Send for CompositionColorBrush {}
unsafe impl Sync for CompositionColorBrush {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionDrawingSurface(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionDrawingSurface,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CompositionDrawingSurface,
    ICompositionSurface,
    CompositionObject
);
impl windows_core::RuntimeType for CompositionDrawingSurface {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionDrawingSurface>();
}
unsafe impl windows_core::Interface for CompositionDrawingSurface {
    type Vtable = <ICompositionDrawingSurface as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionDrawingSurface as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionDrawingSurface {
    type Target = ICompositionDrawingSurface;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionDrawingSurface {
    const NAME: &'static str = "Windows.UI.Composition.CompositionDrawingSurface";
}
unsafe impl Send for CompositionDrawingSurface {}
unsafe impl Sync for CompositionDrawingSurface {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionEasingFunction(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionEasingFunction,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionEasingFunction, CompositionObject);
impl windows_core::RuntimeType for CompositionEasingFunction {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionEasingFunction>();
}
unsafe impl windows_core::Interface for CompositionEasingFunction {
    type Vtable = <ICompositionEasingFunction as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionEasingFunction as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionEasingFunction {
    type Target = ICompositionEasingFunction;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionEasingFunction {
    const NAME: &'static str = "Windows.UI.Composition.CompositionEasingFunction";
}
unsafe impl Send for CompositionEasingFunction {}
unsafe impl Sync for CompositionEasingFunction {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionEffectBrush(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionEffectBrush,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionEffectBrush, CompositionBrush, CompositionObject);
impl windows_core::RuntimeType for CompositionEffectBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionEffectBrush>();
}
unsafe impl windows_core::Interface for CompositionEffectBrush {
    type Vtable = <ICompositionEffectBrush as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionEffectBrush as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionEffectBrush {
    type Target = ICompositionEffectBrush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionEffectBrush {
    const NAME: &'static str = "Windows.UI.Composition.CompositionEffectBrush";
}
unsafe impl Send for CompositionEffectBrush {}
unsafe impl Sync for CompositionEffectBrush {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionEffectFactory(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionEffectFactory,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionEffectFactory, CompositionObject);
impl windows_core::RuntimeType for CompositionEffectFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionEffectFactory>();
}
unsafe impl windows_core::Interface for CompositionEffectFactory {
    type Vtable = <ICompositionEffectFactory as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionEffectFactory as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionEffectFactory {
    type Target = ICompositionEffectFactory;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionEffectFactory {
    const NAME: &'static str = "Windows.UI.Composition.CompositionEffectFactory";
}
unsafe impl Send for CompositionEffectFactory {}
unsafe impl Sync for CompositionEffectFactory {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionEffectSourceParameter(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionEffectSourceParameter,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionEffectSourceParameter, IGraphicsEffectSource);
impl CompositionEffectSourceParameter {
    pub(crate) fn Create(name: &str) -> windows_core::Result<Self> {
        Self::ICompositionEffectSourceParameterFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).Create)(
                windows_core::Interface::as_raw(this),
                core::mem::transmute_copy(&windows_core::HSTRING::from(name)),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    fn ICompositionEffectSourceParameterFactory<
        R,
        F: FnOnce(&ICompositionEffectSourceParameterFactory) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<
            CompositionEffectSourceParameter,
            ICompositionEffectSourceParameterFactory,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for CompositionEffectSourceParameter {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionEffectSourceParameter>();
}
unsafe impl windows_core::Interface for CompositionEffectSourceParameter {
    type Vtable = <ICompositionEffectSourceParameter as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <ICompositionEffectSourceParameter as windows_core::Interface>::IID;
}
impl windows_core::RuntimeName for CompositionEffectSourceParameter {
    const NAME: &'static str = "Windows.UI.Composition.CompositionEffectSourceParameter";
}
unsafe impl Send for CompositionEffectSourceParameter {}
unsafe impl Sync for CompositionEffectSourceParameter {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionGeometricClip(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionGeometricClip,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CompositionGeometricClip,
    CompositionClip,
    CompositionObject
);
impl windows_core::RuntimeType for CompositionGeometricClip {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionGeometricClip>();
}
unsafe impl windows_core::Interface for CompositionGeometricClip {
    type Vtable = <ICompositionGeometricClip as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionGeometricClip as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionGeometricClip {
    type Target = ICompositionGeometricClip;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionGeometricClip {
    const NAME: &'static str = "Windows.UI.Composition.CompositionGeometricClip";
}
unsafe impl Send for CompositionGeometricClip {}
unsafe impl Sync for CompositionGeometricClip {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionGeometry(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionGeometry,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionGeometry, CompositionObject);
impl windows_core::RuntimeType for CompositionGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionGeometry>();
}
unsafe impl windows_core::Interface for CompositionGeometry {
    type Vtable = <ICompositionGeometry as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionGeometry as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionGeometry {
    type Target = ICompositionGeometry;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionGeometry {
    const NAME: &'static str = "Windows.UI.Composition.CompositionGeometry";
}
unsafe impl Send for CompositionGeometry {}
unsafe impl Sync for CompositionGeometry {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionGraphicsDevice(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionGraphicsDevice,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionGraphicsDevice, CompositionObject);
impl windows_core::RuntimeType for CompositionGraphicsDevice {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionGraphicsDevice>();
}
unsafe impl windows_core::Interface for CompositionGraphicsDevice {
    type Vtable = <ICompositionGraphicsDevice as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionGraphicsDevice as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionGraphicsDevice {
    type Target = ICompositionGraphicsDevice;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionGraphicsDevice {
    const NAME: &'static str = "Windows.UI.Composition.CompositionGraphicsDevice";
}
unsafe impl Send for CompositionGraphicsDevice {}
unsafe impl Sync for CompositionGraphicsDevice {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionObject(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionObject,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for CompositionObject {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionObject>();
}
unsafe impl windows_core::Interface for CompositionObject {
    type Vtable = <ICompositionObject as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionObject as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionObject {
    type Target = ICompositionObject;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionObject {
    const NAME: &'static str = "Windows.UI.Composition.CompositionObject";
}
unsafe impl Send for CompositionObject {}
unsafe impl Sync for CompositionObject {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionPropertySet(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionPropertySet,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionPropertySet, CompositionObject);
impl windows_core::RuntimeType for CompositionPropertySet {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionPropertySet>();
}
unsafe impl windows_core::Interface for CompositionPropertySet {
    type Vtable = <ICompositionPropertySet as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionPropertySet as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionPropertySet {
    type Target = ICompositionPropertySet;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionPropertySet {
    const NAME: &'static str = "Windows.UI.Composition.CompositionPropertySet";
}
unsafe impl Send for CompositionPropertySet {}
unsafe impl Sync for CompositionPropertySet {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompositionStretch(pub i32);
impl CompositionStretch {
    pub const None: Self = Self(0);
    pub const Fill: Self = Self(1);
    pub const Uniform: Self = Self(2);
    pub const UniformToFill: Self = Self(3);
}
impl windows_core::TypeKind for CompositionStretch {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for CompositionStretch {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.CompositionStretch;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionSurfaceBrush(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionSurfaceBrush,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CompositionSurfaceBrush,
    CompositionBrush,
    CompositionObject
);
impl windows_core::RuntimeType for CompositionSurfaceBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionSurfaceBrush>();
}
unsafe impl windows_core::Interface for CompositionSurfaceBrush {
    type Vtable = <ICompositionSurfaceBrush as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionSurfaceBrush as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionSurfaceBrush {
    type Target = ICompositionSurfaceBrush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionSurfaceBrush {
    const NAME: &'static str = "Windows.UI.Composition.CompositionSurfaceBrush";
}
unsafe impl Send for CompositionSurfaceBrush {}
unsafe impl Sync for CompositionSurfaceBrush {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionTarget(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionTarget,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionTarget, CompositionObject);
impl windows_core::RuntimeType for CompositionTarget {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionTarget>();
}
unsafe impl windows_core::Interface for CompositionTarget {
    type Vtable = <ICompositionTarget as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionTarget as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionTarget {
    type Target = ICompositionTarget;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionTarget {
    const NAME: &'static str = "Windows.UI.Composition.CompositionTarget";
}
unsafe impl Send for CompositionTarget {}
unsafe impl Sync for CompositionTarget {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Compositor(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    Compositor,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl Compositor {
    pub(crate) fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<
        R,
        F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<
            Compositor,
            windows_core::imp::IGenericFactory,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for Compositor {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositor>();
}
unsafe impl windows_core::Interface for Compositor {
    type Vtable = <ICompositor as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositor as windows_core::Interface>::IID;
}
impl core::ops::Deref for Compositor {
    type Target = ICompositor;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Compositor {
    const NAME: &'static str = "Windows.UI.Composition.Compositor";
}
unsafe impl Send for Compositor {}
unsafe impl Sync for Compositor {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerVisual(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ContainerVisual,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(ContainerVisual, Visual, CompositionObject);
impl windows_core::RuntimeType for ContainerVisual {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IContainerVisual>();
}
unsafe impl windows_core::Interface for ContainerVisual {
    type Vtable = <IContainerVisual as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IContainerVisual as windows_core::Interface>::IID;
}
impl core::ops::Deref for ContainerVisual {
    type Target = IContainerVisual;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ContainerVisual {
    const NAME: &'static str = "Windows.UI.Composition.ContainerVisual";
}
unsafe impl Send for ContainerVisual {}
unsafe impl Sync for ContainerVisual {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CubicBezierEasingFunction(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CubicBezierEasingFunction,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CubicBezierEasingFunction,
    CompositionEasingFunction,
    CompositionObject
);
impl windows_core::RuntimeType for CubicBezierEasingFunction {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICubicBezierEasingFunction>();
}
unsafe impl windows_core::Interface for CubicBezierEasingFunction {
    type Vtable = <ICubicBezierEasingFunction as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICubicBezierEasingFunction as windows_core::Interface>::IID;
}
impl core::ops::Deref for CubicBezierEasingFunction {
    type Target = ICubicBezierEasingFunction;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CubicBezierEasingFunction {
    const NAME: &'static str = "Windows.UI.Composition.CubicBezierEasingFunction";
}
unsafe impl Send for CubicBezierEasingFunction {}
unsafe impl Sync for CubicBezierEasingFunction {}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DECIMAL {
    pub wReserved: u16,
    pub Anonymous1: DECIMAL_0,
    pub Hi32: u32,
    pub Anonymous2: DECIMAL_1,
}
impl Default for DECIMAL {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union DECIMAL_0 {
    pub Anonymous: DECIMAL_0_0,
    pub signscale: u16,
}
impl Default for DECIMAL_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DECIMAL_0_0 {
    pub scale: u8,
    pub sign: u8,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union DECIMAL_1 {
    pub Anonymous: DECIMAL_1_0,
    pub Lo64: u64,
}
impl Default for DECIMAL_1 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DECIMAL_1_0 {
    pub Lo32: u32,
    pub Mid32: u32,
}
pub type DISPATCHERQUEUE_THREAD_APARTMENTTYPE = i32;
pub type DISPATCHERQUEUE_THREAD_TYPE = i32;
pub type DPI_AWARENESS_CONTEXT = *mut core::ffi::c_void;
pub const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: DPI_AWARENESS_CONTEXT = -4 as _;
// ── displayconfig (SDR white level query) — copied verbatim from the generated
// Win32 bindings in crates/libs/windows (Windows/Win32/Devices/Display/mod.rs,
// with LUID/POINTL/RECTL from Windows/Win32/Foundation/mod.rs). ──────────────
pub const DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL: DISPLAYCONFIG_DEVICE_INFO_TYPE =
    DISPLAYCONFIG_DEVICE_INFO_TYPE(11);
pub const DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME: DISPLAYCONFIG_DEVICE_INFO_TYPE =
    DISPLAYCONFIG_DEVICE_INFO_TYPE(1);
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DISPLAYCONFIG_DEVICE_INFO_HEADER {
    pub r#type: DISPLAYCONFIG_DEVICE_INFO_TYPE,
    pub size: u32,
    pub adapterId: LUID,
    pub id: u32,
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DISPLAYCONFIG_DEVICE_INFO_TYPE(pub i32);
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DISPLAYCONFIG_DESKTOP_IMAGE_INFO {
    pub PathSourceSize: POINTL,
    pub DesktopImageRegion: RECTL,
    pub DesktopImageClip: RECTL,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DISPLAYCONFIG_2DREGION {
    pub cx: u32,
    pub cy: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DISPLAYCONFIG_MODE_INFO {
    pub infoType: DISPLAYCONFIG_MODE_INFO_TYPE,
    pub id: u32,
    pub adapterId: LUID,
    pub Anonymous: DISPLAYCONFIG_MODE_INFO_0,
}
impl Default for DISPLAYCONFIG_MODE_INFO {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union DISPLAYCONFIG_MODE_INFO_0 {
    pub targetMode: DISPLAYCONFIG_TARGET_MODE,
    pub sourceMode: DISPLAYCONFIG_SOURCE_MODE,
    pub desktopImageInfo: DISPLAYCONFIG_DESKTOP_IMAGE_INFO,
}
impl Default for DISPLAYCONFIG_MODE_INFO_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DISPLAYCONFIG_MODE_INFO_TYPE(pub i32);
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DISPLAYCONFIG_PATH_INFO {
    pub sourceInfo: DISPLAYCONFIG_PATH_SOURCE_INFO,
    pub targetInfo: DISPLAYCONFIG_PATH_TARGET_INFO,
    pub flags: u32,
}
impl Default for DISPLAYCONFIG_PATH_INFO {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DISPLAYCONFIG_PATH_SOURCE_INFO {
    pub adapterId: LUID,
    pub id: u32,
    pub Anonymous: DISPLAYCONFIG_PATH_SOURCE_INFO_0,
    pub statusFlags: u32,
}
impl Default for DISPLAYCONFIG_PATH_SOURCE_INFO {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union DISPLAYCONFIG_PATH_SOURCE_INFO_0 {
    pub modeInfoIdx: u32,
    pub Anonymous: DISPLAYCONFIG_PATH_SOURCE_INFO_0_0,
}
impl Default for DISPLAYCONFIG_PATH_SOURCE_INFO_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DISPLAYCONFIG_PATH_SOURCE_INFO_0_0 {
    pub _bitfield: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DISPLAYCONFIG_PATH_TARGET_INFO {
    pub adapterId: LUID,
    pub id: u32,
    pub Anonymous: DISPLAYCONFIG_PATH_TARGET_INFO_0,
    pub outputTechnology: DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY,
    pub rotation: DISPLAYCONFIG_ROTATION,
    pub scaling: DISPLAYCONFIG_SCALING,
    pub refreshRate: DISPLAYCONFIG_RATIONAL,
    pub scanLineOrdering: DISPLAYCONFIG_SCANLINE_ORDERING,
    pub targetAvailable: windows_core::BOOL,
    pub statusFlags: u32,
}
impl Default for DISPLAYCONFIG_PATH_TARGET_INFO {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union DISPLAYCONFIG_PATH_TARGET_INFO_0 {
    pub modeInfoIdx: u32,
    pub Anonymous: DISPLAYCONFIG_PATH_TARGET_INFO_0_0,
}
impl Default for DISPLAYCONFIG_PATH_TARGET_INFO_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DISPLAYCONFIG_PATH_TARGET_INFO_0_0 {
    pub _bitfield: u32,
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DISPLAYCONFIG_PIXELFORMAT(pub i32);
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DISPLAYCONFIG_RATIONAL {
    pub Numerator: u32,
    pub Denominator: u32,
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DISPLAYCONFIG_ROTATION(pub i32);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DISPLAYCONFIG_SCALING(pub i32);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DISPLAYCONFIG_SCANLINE_ORDERING(pub i32);
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DISPLAYCONFIG_SDR_WHITE_LEVEL {
    pub header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    pub SDRWhiteLevel: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DISPLAYCONFIG_SOURCE_DEVICE_NAME {
    pub header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    pub viewGdiDeviceName: [u16; 32],
}
impl Default for DISPLAYCONFIG_SOURCE_DEVICE_NAME {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DISPLAYCONFIG_SOURCE_MODE {
    pub width: u32,
    pub height: u32,
    pub pixelFormat: DISPLAYCONFIG_PIXELFORMAT,
    pub position: POINTL,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DISPLAYCONFIG_TARGET_MODE {
    pub targetVideoSignalInfo: DISPLAYCONFIG_VIDEO_SIGNAL_INFO,
}
impl Default for DISPLAYCONFIG_TARGET_MODE {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DISPLAYCONFIG_TOPOLOGY_ID(pub i32);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY(pub i32);
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DISPLAYCONFIG_VIDEO_SIGNAL_INFO {
    pub pixelRate: u64,
    pub hSyncFreq: DISPLAYCONFIG_RATIONAL,
    pub vSyncFreq: DISPLAYCONFIG_RATIONAL,
    pub activeSize: DISPLAYCONFIG_2DREGION,
    pub totalSize: DISPLAYCONFIG_2DREGION,
    pub Anonymous: DISPLAYCONFIG_VIDEO_SIGNAL_INFO_0,
    pub scanLineOrdering: DISPLAYCONFIG_SCANLINE_ORDERING,
}
impl Default for DISPLAYCONFIG_VIDEO_SIGNAL_INFO {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union DISPLAYCONFIG_VIDEO_SIGNAL_INFO_0 {
    pub AdditionalSignalInfo: DISPLAYCONFIG_VIDEO_SIGNAL_INFO_0_0,
    pub videoStandard: u32,
}
impl Default for DISPLAYCONFIG_VIDEO_SIGNAL_INFO_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DISPLAYCONFIG_VIDEO_SIGNAL_INFO_0_0 {
    pub _bitfield: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LUID {
    pub LowPart: u32,
    pub HighPart: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct POINTL {
    pub x: i32,
    pub y: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RECTL {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
pub const QDC_ONLY_ACTIVE_PATHS: QUERY_DISPLAY_CONFIG_FLAGS = QUERY_DISPLAY_CONFIG_FLAGS(2);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct QUERY_DISPLAY_CONFIG_FLAGS(pub u32);
// ── end displayconfig ────────────────────────────────────────────────────────
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DVTARGETDEVICE {
    pub tdSize: u32,
    pub tdDriverNameOffset: u16,
    pub tdDeviceNameOffset: u16,
    pub tdPortNameOffset: u16,
    pub tdExtDevmodeOffset: u16,
    pub tdData: [u8; 1],
}
impl Default for DVTARGETDEVICE {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopWindowTarget(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    DesktopWindowTarget,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(DesktopWindowTarget, CompositionTarget, CompositionObject);
impl windows_core::RuntimeType for DesktopWindowTarget {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDesktopWindowTarget>();
}
unsafe impl windows_core::Interface for DesktopWindowTarget {
    type Vtable = <IDesktopWindowTarget as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDesktopWindowTarget as windows_core::Interface>::IID;
}
impl core::ops::Deref for DesktopWindowTarget {
    type Target = IDesktopWindowTarget;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DesktopWindowTarget {
    const NAME: &'static str = "Windows.UI.Composition.Desktop.DesktopWindowTarget";
}
unsafe impl Send for DesktopWindowTarget {}
unsafe impl Sync for DesktopWindowTarget {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DirectXAlphaMode(pub i32);
impl DirectXAlphaMode {
    pub const Unspecified: Self = Self(0);
    pub const Premultiplied: Self = Self(1);
    pub const Straight: Self = Self(2);
    pub const Ignore: Self = Self(3);
}
impl windows_core::TypeKind for DirectXAlphaMode {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for DirectXAlphaMode {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.Graphics.DirectX.DirectXAlphaMode;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DirectXPixelFormat(pub i32);
impl DirectXPixelFormat {
    pub const Unknown: Self = Self(0);
    pub const R32G32B32A32Typeless: Self = Self(1);
    pub const R32G32B32A32Float: Self = Self(2);
    pub const R32G32B32A32UInt: Self = Self(3);
    pub const R32G32B32A32Int: Self = Self(4);
    pub const R32G32B32Typeless: Self = Self(5);
    pub const R32G32B32Float: Self = Self(6);
    pub const R32G32B32UInt: Self = Self(7);
    pub const R32G32B32Int: Self = Self(8);
    pub const R16G16B16A16Typeless: Self = Self(9);
    pub const R16G16B16A16Float: Self = Self(10);
    pub const R16G16B16A16UIntNormalized: Self = Self(11);
    pub const R16G16B16A16UInt: Self = Self(12);
    pub const R16G16B16A16IntNormalized: Self = Self(13);
    pub const R16G16B16A16Int: Self = Self(14);
    pub const R32G32Typeless: Self = Self(15);
    pub const R32G32Float: Self = Self(16);
    pub const R32G32UInt: Self = Self(17);
    pub const R32G32Int: Self = Self(18);
    pub const R32G8X24Typeless: Self = Self(19);
    pub const D32FloatS8X24UInt: Self = Self(20);
    pub const R32FloatX8X24Typeless: Self = Self(21);
    pub const X32TypelessG8X24UInt: Self = Self(22);
    pub const R10G10B10A2Typeless: Self = Self(23);
    pub const R10G10B10A2UIntNormalized: Self = Self(24);
    pub const R10G10B10A2UInt: Self = Self(25);
    pub const R11G11B10Float: Self = Self(26);
    pub const R8G8B8A8Typeless: Self = Self(27);
    pub const R8G8B8A8UIntNormalized: Self = Self(28);
    pub const R8G8B8A8UIntNormalizedSrgb: Self = Self(29);
    pub const R8G8B8A8UInt: Self = Self(30);
    pub const R8G8B8A8IntNormalized: Self = Self(31);
    pub const R8G8B8A8Int: Self = Self(32);
    pub const R16G16Typeless: Self = Self(33);
    pub const R16G16Float: Self = Self(34);
    pub const R16G16UIntNormalized: Self = Self(35);
    pub const R16G16UInt: Self = Self(36);
    pub const R16G16IntNormalized: Self = Self(37);
    pub const R16G16Int: Self = Self(38);
    pub const R32Typeless: Self = Self(39);
    pub const D32Float: Self = Self(40);
    pub const R32Float: Self = Self(41);
    pub const R32UInt: Self = Self(42);
    pub const R32Int: Self = Self(43);
    pub const R24G8Typeless: Self = Self(44);
    pub const D24UIntNormalizedS8UInt: Self = Self(45);
    pub const R24UIntNormalizedX8Typeless: Self = Self(46);
    pub const X24TypelessG8UInt: Self = Self(47);
    pub const R8G8Typeless: Self = Self(48);
    pub const R8G8UIntNormalized: Self = Self(49);
    pub const R8G8UInt: Self = Self(50);
    pub const R8G8IntNormalized: Self = Self(51);
    pub const R8G8Int: Self = Self(52);
    pub const R16Typeless: Self = Self(53);
    pub const R16Float: Self = Self(54);
    pub const D16UIntNormalized: Self = Self(55);
    pub const R16UIntNormalized: Self = Self(56);
    pub const R16UInt: Self = Self(57);
    pub const R16IntNormalized: Self = Self(58);
    pub const R16Int: Self = Self(59);
    pub const R8Typeless: Self = Self(60);
    pub const R8UIntNormalized: Self = Self(61);
    pub const R8UInt: Self = Self(62);
    pub const R8IntNormalized: Self = Self(63);
    pub const R8Int: Self = Self(64);
    pub const A8UIntNormalized: Self = Self(65);
    pub const R1UIntNormalized: Self = Self(66);
    pub const R9G9B9E5SharedExponent: Self = Self(67);
    pub const R8G8B8G8UIntNormalized: Self = Self(68);
    pub const G8R8G8B8UIntNormalized: Self = Self(69);
    pub const BC1Typeless: Self = Self(70);
    pub const BC1UIntNormalized: Self = Self(71);
    pub const BC1UIntNormalizedSrgb: Self = Self(72);
    pub const BC2Typeless: Self = Self(73);
    pub const BC2UIntNormalized: Self = Self(74);
    pub const BC2UIntNormalizedSrgb: Self = Self(75);
    pub const BC3Typeless: Self = Self(76);
    pub const BC3UIntNormalized: Self = Self(77);
    pub const BC3UIntNormalizedSrgb: Self = Self(78);
    pub const BC4Typeless: Self = Self(79);
    pub const BC4UIntNormalized: Self = Self(80);
    pub const BC4IntNormalized: Self = Self(81);
    pub const BC5Typeless: Self = Self(82);
    pub const BC5UIntNormalized: Self = Self(83);
    pub const BC5IntNormalized: Self = Self(84);
    pub const B5G6R5UIntNormalized: Self = Self(85);
    pub const B5G5R5A1UIntNormalized: Self = Self(86);
    pub const B8G8R8A8UIntNormalized: Self = Self(87);
    pub const B8G8R8X8UIntNormalized: Self = Self(88);
    pub const R10G10B10XRBiasA2UIntNormalized: Self = Self(89);
    pub const B8G8R8A8Typeless: Self = Self(90);
    pub const B8G8R8A8UIntNormalizedSrgb: Self = Self(91);
    pub const B8G8R8X8Typeless: Self = Self(92);
    pub const B8G8R8X8UIntNormalizedSrgb: Self = Self(93);
    pub const BC6HTypeless: Self = Self(94);
    pub const BC6H16UnsignedFloat: Self = Self(95);
    pub const BC6H16Float: Self = Self(96);
    pub const BC7Typeless: Self = Self(97);
    pub const BC7UIntNormalized: Self = Self(98);
    pub const BC7UIntNormalizedSrgb: Self = Self(99);
    pub const Ayuv: Self = Self(100);
    pub const Y410: Self = Self(101);
    pub const Y416: Self = Self(102);
    pub const NV12: Self = Self(103);
    pub const P010: Self = Self(104);
    pub const P016: Self = Self(105);
    pub const Opaque420: Self = Self(106);
    pub const Yuy2: Self = Self(107);
    pub const Y210: Self = Self(108);
    pub const Y216: Self = Self(109);
    pub const NV11: Self = Self(110);
    pub const AI44: Self = Self(111);
    pub const IA44: Self = Self(112);
    pub const P8: Self = Self(113);
    pub const A8P8: Self = Self(114);
    pub const B4G4R4A4UIntNormalized: Self = Self(115);
    pub const P208: Self = Self(130);
    pub const V208: Self = Self(131);
    pub const V408: Self = Self(132);
    pub const SamplerFeedbackMinMipOpaque: Self = Self(189);
    pub const SamplerFeedbackMipRegionUsedOpaque: Self = Self(190);
    pub const A4B4G4R4: Self = Self(191);
}
impl windows_core::TypeKind for DirectXPixelFormat {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for DirectXPixelFormat {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.Graphics.DirectX.DirectXPixelFormat;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatcherQueue(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    DispatcherQueue,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for DispatcherQueue {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDispatcherQueue>();
}
unsafe impl windows_core::Interface for DispatcherQueue {
    type Vtable = <IDispatcherQueue as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDispatcherQueue as windows_core::Interface>::IID;
}
impl core::ops::Deref for DispatcherQueue {
    type Target = IDispatcherQueue;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DispatcherQueue {
    const NAME: &'static str = "Windows.System.DispatcherQueue";
}
unsafe impl Send for DispatcherQueue {}
unsafe impl Sync for DispatcherQueue {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatcherQueueController(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    DispatcherQueueController,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for DispatcherQueueController {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDispatcherQueueController>();
}
unsafe impl windows_core::Interface for DispatcherQueueController {
    type Vtable = <IDispatcherQueueController as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDispatcherQueueController as windows_core::Interface>::IID;
}
impl core::ops::Deref for DispatcherQueueController {
    type Target = IDispatcherQueueController;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DispatcherQueueController {
    const NAME: &'static str = "Windows.System.DispatcherQueueController";
}
unsafe impl Send for DispatcherQueueController {}
unsafe impl Sync for DispatcherQueueController {}
windows_core::imp::define_interface!(
    DispatcherQueueHandler,
    DispatcherQueueHandler_Vtbl,
    0xdfa2dc9c_1a2d_4917_98f2_939af1d6e0c8
);
impl windows_core::RuntimeType for DispatcherQueueHandler {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl DispatcherQueueHandler {
    pub(crate) fn new<F: Fn() + 'static>(invoke: F) -> Self {
        let com = windows_core::imp::DelegateBox::<Self, F>::new(
            &DispatcherQueueHandlerBox::<F>::VTABLE,
            invoke,
        );
        unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
    }
}
#[repr(C)]
pub struct DispatcherQueueHandler_Vtbl {
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> windows_core::HRESULT,
}
struct DispatcherQueueHandlerBox<F: Fn() + 'static>(core::marker::PhantomData<(fn() -> F,)>);
impl<F: Fn() + 'static> DispatcherQueueHandlerBox<F> {
    const VTABLE: DispatcherQueueHandler_Vtbl = DispatcherQueueHandler_Vtbl {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface:
                windows_core::imp::DelegateBox::<DispatcherQueueHandler, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<DispatcherQueueHandler, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<DispatcherQueueHandler, F>::Release,
        },
        Invoke: Self::Invoke,
    };
    unsafe extern "system" fn Invoke(this: *mut core::ffi::c_void) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<DispatcherQueueHandler, F>);
            (this.invoke)();
            windows_core::HRESULT(0)
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DispatcherQueueOptions {
    pub dwSize: u32,
    pub threadType: DISPATCHERQUEUE_THREAD_TYPE,
    pub apartmentType: DISPATCHERQUEUE_THREAD_APARTMENTTYPE,
}
pub type ExpandCollapseState = i32;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ExpressionAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    ExpressionAnimation,
    CompositionAnimation,
    CompositionObject
);
impl windows_core::RuntimeType for ExpressionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IExpressionAnimation>();
}
unsafe impl windows_core::Interface for ExpressionAnimation {
    type Vtable = <IExpressionAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IExpressionAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for ExpressionAnimation {
    type Target = IExpressionAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ExpressionAnimation {
    const NAME: &'static str = "Windows.UI.Composition.ExpressionAnimation";
}
unsafe impl Send for ExpressionAnimation {}
unsafe impl Sync for ExpressionAnimation {}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FORMATETC {
    pub cfFormat: u16,
    pub ptd: *mut DVTARGETDEVICE,
    pub dwAspect: u32,
    pub lindex: i32,
    pub tymed: u32,
}
impl Default for FORMATETC {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub const GCS_COMPSTR: IME_COMPOSITION_STRING = 8;
pub const GCS_RESULTSTR: IME_COMPOSITION_STRING = 2048;
pub type GLOBAL_ALLOC_FLAGS = u32;
pub const GMEM_MOVEABLE: GLOBAL_ALLOC_FLAGS = 2;
pub const GMEM_ZEROINIT: GLOBAL_ALLOC_FLAGS = 64;
pub const GWLP_USERDATA: WINDOW_LONG_PTR_INDEX = -21;
pub const GWLP_WNDPROC: WINDOW_LONG_PTR_INDEX = -4;
pub type HANDLE = *mut core::ffi::c_void;
pub type HBRUSH = *mut core::ffi::c_void;
pub type HCURSOR = *mut core::ffi::c_void;
pub type HDC = *mut core::ffi::c_void;
pub type HGLOBAL = *mut core::ffi::c_void;
pub type HICON = *mut core::ffi::c_void;
pub type HIMC = *mut core::ffi::c_void;
pub type HINSTANCE = *mut core::ffi::c_void;
pub type HMENU = *mut core::ffi::c_void;
pub type HMODULE = *mut core::ffi::c_void;
pub type HMONITOR = *mut core::ffi::c_void;
pub const HTBOTTOM: u32 = 15;
pub const HTBOTTOMLEFT: u32 = 16;
pub const HTBOTTOMRIGHT: u32 = 17;
pub const HTCAPTION: u32 = 2;
pub const HTCLIENT: u32 = 1;
pub const HTLEFT: u32 = 10;
pub const HTNOWHERE: u32 = 0;
pub const HTRIGHT: u32 = 11;
pub const HTTOP: u32 = 12;
pub const HTTOPLEFT: u32 = 13;
pub const HTTOPRIGHT: u32 = 14;
pub const HTTRANSPARENT: i32 = -1;
pub type HWND = *mut core::ffi::c_void;
windows_core::imp::define_interface!(
    ICompositionAnimation,
    ICompositionAnimation_Vtbl,
    0x464c4c2c_1caa_4061_9b40_e13fde1503ca
);
impl windows_core::RuntimeType for ICompositionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionAnimation {
    pub(crate) fn SetColorParameter(&self, key: &str, value: Color) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetColorParameter)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(key)),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetReferenceParameter<P1>(
        &self,
        key: &str,
        compositionobject: P1,
    ) -> windows_core::Result<()>
    where
        P1: windows_core::Param<CompositionObject>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetReferenceParameter)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(key)),
                compositionobject.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn SetScalarParameter(&self, key: &str, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetScalarParameter)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(key)),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetVector2Parameter(
        &self,
        key: &str,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetVector2Parameter)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(key)),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetVector3Parameter(
        &self,
        key: &str,
        value: windows_numerics::Vector3,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetVector3Parameter)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(key)),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    ClearAllParameters: usize,
    ClearParameter: usize,
    pub SetColorParameter: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        Color,
    ) -> windows_core::HRESULT,
    SetMatrix3x2Parameter: usize,
    SetMatrix4x4Parameter: usize,
    SetQuaternionParameter: usize,
    pub SetReferenceParameter: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetScalarParameter: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        f32,
    ) -> windows_core::HRESULT,
    pub SetVector2Parameter: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    pub SetVector3Parameter: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionBrush,
    ICompositionBrush_Vtbl,
    0xab0d7608_30c0_40e9_b568_b60a6bd1fb46
);
impl windows_core::RuntimeType for ICompositionBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionBrush_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionClip,
    ICompositionClip_Vtbl,
    0x1ccd2a52_cfc7_4ace_9983_146bb8eb6a3c
);
impl windows_core::RuntimeType for ICompositionClip {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionClip_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionColorBrush,
    ICompositionColorBrush_Vtbl,
    0x2b264c5e_bf35_4831_8642_cf70c20fff2f
);
impl windows_core::RuntimeType for ICompositionColorBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionColorBrush {
    pub(crate) fn SetColor(&self, value: Color) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetColor)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionColorBrush_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Color: usize,
    pub SetColor: unsafe extern "system" fn(*mut core::ffi::c_void, Color) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionDrawingSurface,
    ICompositionDrawingSurface_Vtbl,
    0xa166c300_fad0_4d11_9e67_e433162ff49e
);
impl windows_core::RuntimeType for ICompositionDrawingSurface {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionDrawingSurface {
    pub(crate) fn Size(&self) -> windows_core::Result<Size> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Size)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct ICompositionDrawingSurface_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    AlphaMode: usize,
    PixelFormat: usize,
    pub Size: unsafe extern "system" fn(*mut core::ffi::c_void, *mut Size) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionDrawingSurface2,
    ICompositionDrawingSurface2_Vtbl,
    0xfad0e88b_e354_44e8_8e3d_c4880d5a213f
);
impl windows_core::RuntimeType for ICompositionDrawingSurface2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionDrawingSurface2 {
    pub(crate) fn Resize(&self, sizepixels: SizeInt32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).Resize)(
                windows_core::Interface::as_raw(self),
                sizepixels,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionDrawingSurface2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    SizeInt32: usize,
    pub Resize:
        unsafe extern "system" fn(*mut core::ffi::c_void, SizeInt32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionDrawingSurfaceInterop,
    ICompositionDrawingSurfaceInterop_Vtbl,
    0xfd04e6e3_fe0c_4c3c_ab19_a07601a576ee
);
windows_core::imp::interface_hierarchy!(ICompositionDrawingSurfaceInterop, windows_core::IUnknown);
impl ICompositionDrawingSurfaceInterop {
    pub(crate) unsafe fn BeginDraw<T>(
        &self,
        updaterect: Option<*const RECT>,
        updateoffset: *mut POINT,
    ) -> windows_core::Result<T>
    where
        T: windows_core::Interface,
    {
        let mut result__ = core::ptr::null_mut();
        unsafe {
            (windows_core::Interface::vtable(self).BeginDraw)(
                windows_core::Interface::as_raw(self),
                updaterect.unwrap_or(core::mem::zeroed()) as _,
                &T::IID,
                &mut result__,
                updateoffset as _,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn EndDraw(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).EndDraw)(windows_core::Interface::as_raw(self))
                .ok()
        }
    }
    pub(crate) unsafe fn Resize(&self, sizepixels: SIZE) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).Resize)(
                windows_core::Interface::as_raw(self),
                sizepixels,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionDrawingSurfaceInterop_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub BeginDraw: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const RECT,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
        *mut POINT,
    ) -> windows_core::HRESULT,
    pub EndDraw: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Resize: unsafe extern "system" fn(*mut core::ffi::c_void, SIZE) -> windows_core::HRESULT,
    Scroll: usize,
    ResumeDraw: usize,
    SuspendDraw: usize,
}
windows_core::imp::define_interface!(
    ICompositionEasingFunction,
    ICompositionEasingFunction_Vtbl,
    0x5145e356_bf79_4ea8_8cc2_6b5b472e6c9a
);
impl windows_core::RuntimeType for ICompositionEasingFunction {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionEasingFunction_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionEffectBrush,
    ICompositionEffectBrush_Vtbl,
    0xbf7f795e_83cc_44bf_a447_3e3c071789ec
);
impl windows_core::RuntimeType for ICompositionEffectBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionEffectBrush {
    pub(crate) fn SetSourceParameter<P1>(&self, name: &str, source: P1) -> windows_core::Result<()>
    where
        P1: windows_core::Param<CompositionBrush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetSourceParameter)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(name)),
                source.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionEffectBrush_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    GetSourceParameter: usize,
    pub SetSourceParameter: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionEffectFactory,
    ICompositionEffectFactory_Vtbl,
    0xbe5624af_ba7e_4510_9850_41c0b4ff74df
);
impl windows_core::RuntimeType for ICompositionEffectFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionEffectFactory {
    pub(crate) fn CreateBrush(&self) -> windows_core::Result<CompositionEffectBrush> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateBrush)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositionEffectFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    ExtendedError: usize,
    LoadStatus: usize,
}
windows_core::imp::define_interface!(
    ICompositionEffectSourceParameter,
    ICompositionEffectSourceParameter_Vtbl,
    0x858ab13a_3292_4e4e_b3bb_2b6c6544a6ee
);
impl windows_core::RuntimeType for ICompositionEffectSourceParameter {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionEffectSourceParameter_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Name: usize,
}
windows_core::imp::define_interface!(
    ICompositionEffectSourceParameterFactory,
    ICompositionEffectSourceParameterFactory_Vtbl,
    0xb3d9f276_aba3_4724_acf3_d0397464db1c
);
impl windows_core::RuntimeType for ICompositionEffectSourceParameterFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionEffectSourceParameterFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Create: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionGeometricClip,
    ICompositionGeometricClip_Vtbl,
    0xc840b581_81c9_4444_a2c1_ccaece3a50e5
);
impl windows_core::RuntimeType for ICompositionGeometricClip {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionGeometricClip_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionGeometry,
    ICompositionGeometry_Vtbl,
    0xe985217c_6a17_4207_abd8_5fd3dd612a9d
);
impl windows_core::RuntimeType for ICompositionGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionGeometry_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionGraphicsDevice,
    ICompositionGraphicsDevice_Vtbl,
    0xfb22c6e1_80a2_4667_9936_dbeaf6eefe95
);
impl windows_core::RuntimeType for ICompositionGraphicsDevice {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionGraphicsDevice {
    pub(crate) fn CreateDrawingSurface(
        &self,
        sizepixels: Size,
        pixelformat: DirectXPixelFormat,
        alphamode: DirectXAlphaMode,
    ) -> windows_core::Result<CompositionDrawingSurface> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateDrawingSurface)(
                windows_core::Interface::as_raw(self),
                sizepixels,
                pixelformat,
                alphamode,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositionGraphicsDevice_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateDrawingSurface: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        Size,
        DirectXPixelFormat,
        DirectXAlphaMode,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionGraphicsDevice2,
    ICompositionGraphicsDevice2_Vtbl,
    0x0fb8bdf6_c0f0_4bcc_9fb8_084982490d7d
);
impl windows_core::RuntimeType for ICompositionGraphicsDevice2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionGraphicsDevice2 {
    pub(crate) fn CreateDrawingSurface2(
        &self,
        sizepixels: SizeInt32,
        pixelformat: DirectXPixelFormat,
        alphamode: DirectXAlphaMode,
    ) -> windows_core::Result<CompositionDrawingSurface> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateDrawingSurface2)(
                windows_core::Interface::as_raw(self),
                sizepixels,
                pixelformat,
                alphamode,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositionGraphicsDevice2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateDrawingSurface2: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        SizeInt32,
        DirectXPixelFormat,
        DirectXAlphaMode,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionGraphicsDeviceInterop,
    ICompositionGraphicsDeviceInterop_Vtbl,
    0xa116ff71_f8bf_4c8a_9c98_70779a32a9c8
);
windows_core::imp::interface_hierarchy!(ICompositionGraphicsDeviceInterop, windows_core::IUnknown);
impl ICompositionGraphicsDeviceInterop {
    pub(crate) unsafe fn GetRenderingDevice(&self) -> windows_core::Result<windows_core::IUnknown> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetRenderingDevice)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositionGraphicsDeviceInterop_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetRenderingDevice: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    SetRenderingDevice: usize,
}
windows_core::imp::define_interface!(
    ICompositionObject,
    ICompositionObject_Vtbl,
    0xbcb4ad45_7609_4550_934f_16002a68fded
);
impl windows_core::RuntimeType for ICompositionObject {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionObject {
    pub(crate) fn Compositor(&self) -> windows_core::Result<Compositor> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Compositor)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn StartAnimation<P1>(
        &self,
        propertyname: &str,
        animation: P1,
    ) -> windows_core::Result<()>
    where
        P1: windows_core::Param<CompositionAnimation>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).StartAnimation)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(propertyname)),
                animation.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn StopAnimation(&self, propertyname: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).StopAnimation)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(propertyname)),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionObject_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Compositor: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    Dispatcher: usize,
    Properties: usize,
    pub StartAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub StopAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionPropertySet,
    ICompositionPropertySet_Vtbl,
    0xc9d6d202_5f67_4453_9117_9eadd430d3c2
);
impl windows_core::RuntimeType for ICompositionPropertySet {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionPropertySet {
    pub(crate) fn InsertScalar(&self, propertyname: &str, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).InsertScalar)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(propertyname)),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionPropertySet_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    InsertColor: usize,
    InsertMatrix3x2: usize,
    InsertMatrix4x4: usize,
    InsertQuaternion: usize,
    pub InsertScalar: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        f32,
    ) -> windows_core::HRESULT,
    InsertVector2: usize,
    InsertVector3: usize,
    InsertVector4: usize,
    TryGetColor: usize,
    TryGetMatrix3x2: usize,
    TryGetMatrix4x4: usize,
    TryGetQuaternion: usize,
    TryGetScalar: usize,
    TryGetVector2: usize,
    TryGetVector3: usize,
    TryGetVector4: usize,
}
windows_core::imp::define_interface!(
    ICompositionSurface,
    ICompositionSurface_Vtbl,
    0x1527540d_42c7_47a6_a408_668f79a90dfb
);
impl windows_core::RuntimeType for ICompositionSurface {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(
    ICompositionSurface,
    windows_core::IUnknown,
    windows_core::IInspectable
);
#[repr(C)]
pub struct ICompositionSurface_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionSurfaceBrush,
    ICompositionSurfaceBrush_Vtbl,
    0xad016d79_1e4c_4c0d_9c29_83338c87c162
);
impl windows_core::RuntimeType for ICompositionSurfaceBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionSurfaceBrush {
    pub(crate) fn SetStretch(&self, value: CompositionStretch) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetStretch)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetSurface<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<ICompositionSurface>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetSurface)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionSurfaceBrush_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    BitmapInterpolationMode: usize,
    SetBitmapInterpolationMode: usize,
    HorizontalAlignmentRatio: usize,
    SetHorizontalAlignmentRatio: usize,
    Stretch: usize,
    pub SetStretch: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        CompositionStretch,
    ) -> windows_core::HRESULT,
    Surface: usize,
    pub SetSurface: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionTarget,
    ICompositionTarget_Vtbl,
    0xa1bea8ba_d726_4663_8129_6b5e7927ffa6
);
impl windows_core::RuntimeType for ICompositionTarget {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionTarget {
    pub(crate) fn SetRoot<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<Visual>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetRoot)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionTarget_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Root: usize,
    pub SetRoot: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositor,
    ICompositor_Vtbl,
    0xb403ca50_7f8c_4e83_985f_cc45060036d8
);
impl windows_core::RuntimeType for ICompositor {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositor {
    pub(crate) fn CreateColorBrush(&self) -> windows_core::Result<CompositionColorBrush> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateColorBrush)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateColorBrushWithColor(
        &self,
        color: Color,
    ) -> windows_core::Result<CompositionColorBrush> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateColorBrushWithColor)(
                windows_core::Interface::as_raw(self),
                color,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateContainerVisual(&self) -> windows_core::Result<ContainerVisual> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateContainerVisual)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateCubicBezierEasingFunction(
        &self,
        controlpoint1: windows_numerics::Vector2,
        controlpoint2: windows_numerics::Vector2,
    ) -> windows_core::Result<CubicBezierEasingFunction> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateCubicBezierEasingFunction)(
                windows_core::Interface::as_raw(self),
                controlpoint1,
                controlpoint2,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateEffectFactory<P0>(
        &self,
        graphicseffect: P0,
    ) -> windows_core::Result<CompositionEffectFactory>
    where
        P0: windows_core::Param<IGraphicsEffect>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateEffectFactory)(
                windows_core::Interface::as_raw(self),
                graphicseffect.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateEffectFactoryWithProperties<P0, P1>(
        &self,
        graphicseffect: P0,
        animatableproperties: P1,
    ) -> windows_core::Result<CompositionEffectFactory>
    where
        P0: windows_core::Param<IGraphicsEffect>,
        P1: windows_core::Param<windows_collections::IIterable<windows_core::HSTRING>>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateEffectFactoryWithProperties)(
                windows_core::Interface::as_raw(self),
                graphicseffect.param().abi(),
                animatableproperties.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateExpressionAnimation(&self) -> windows_core::Result<ExpressionAnimation> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateExpressionAnimation)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateExpressionAnimationWithExpression(
        &self,
        expression: &str,
    ) -> windows_core::Result<ExpressionAnimation> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateExpressionAnimationWithExpression)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(expression)),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateInsetClip(&self) -> windows_core::Result<InsetClip> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateInsetClip)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateInsetClipWithInsets(
        &self,
        leftinset: f32,
        topinset: f32,
        rightinset: f32,
        bottominset: f32,
    ) -> windows_core::Result<InsetClip> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateInsetClipWithInsets)(
                windows_core::Interface::as_raw(self),
                leftinset,
                topinset,
                rightinset,
                bottominset,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateLinearEasingFunction(&self) -> windows_core::Result<LinearEasingFunction> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateLinearEasingFunction)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreatePropertySet(&self) -> windows_core::Result<CompositionPropertySet> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreatePropertySet)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateScalarKeyFrameAnimation(
        &self,
    ) -> windows_core::Result<ScalarKeyFrameAnimation> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateScalarKeyFrameAnimation)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateSpriteVisual(&self) -> windows_core::Result<SpriteVisual> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateSpriteVisual)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateSurfaceBrush(&self) -> windows_core::Result<CompositionSurfaceBrush> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateSurfaceBrush)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateSurfaceBrushWithSurface<P0>(
        &self,
        surface: P0,
    ) -> windows_core::Result<CompositionSurfaceBrush>
    where
        P0: windows_core::Param<ICompositionSurface>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateSurfaceBrushWithSurface)(
                windows_core::Interface::as_raw(self),
                surface.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateVector2KeyFrameAnimation(
        &self,
    ) -> windows_core::Result<Vector2KeyFrameAnimation> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateVector2KeyFrameAnimation)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateVector3KeyFrameAnimation(
        &self,
    ) -> windows_core::Result<Vector3KeyFrameAnimation> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateVector3KeyFrameAnimation)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositor_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    CreateColorKeyFrameAnimation: usize,
    pub CreateColorBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateColorBrushWithColor: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        Color,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateContainerVisual: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateCubicBezierEasingFunction: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
        windows_numerics::Vector2,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateEffectFactory: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateEffectFactoryWithProperties: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateExpressionAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateExpressionAnimationWithExpression: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    )
        -> windows_core::HRESULT,
    pub CreateInsetClip: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateInsetClipWithInsets: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        f32,
        f32,
        f32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateLinearEasingFunction: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreatePropertySet: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateQuaternionKeyFrameAnimation: usize,
    pub CreateScalarKeyFrameAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateScopedBatch: usize,
    pub CreateSpriteVisual: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateSurfaceBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateSurfaceBrushWithSurface: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateTargetForCurrentView: usize,
    pub CreateVector2KeyFrameAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateVector3KeyFrameAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositor6,
    ICompositor6_Vtbl,
    0x7a38b2bd_cec8_4eeb_830f_d8d07aedebc3
);
impl windows_core::RuntimeType for ICompositor6 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositor6 {
    pub(crate) fn CreateGeometricClip(&self) -> windows_core::Result<CompositionGeometricClip> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateGeometricClip)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateGeometricClipWithGeometry<P0>(
        &self,
        geometry: P0,
    ) -> windows_core::Result<CompositionGeometricClip>
    where
        P0: windows_core::Param<CompositionGeometry>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateGeometricClipWithGeometry)(
                windows_core::Interface::as_raw(self),
                geometry.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositor6_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateGeometricClip: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateGeometricClipWithGeometry: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositorDesktopInterop,
    ICompositorDesktopInterop_Vtbl,
    0x29e691fa_4567_4dca_b319_d0f207eb6807
);
windows_core::imp::interface_hierarchy!(ICompositorDesktopInterop, windows_core::IUnknown);
impl ICompositorDesktopInterop {
    pub(crate) unsafe fn CreateDesktopWindowTarget(
        &self,
        hwndtarget: HWND,
        istopmost: bool,
    ) -> windows_core::Result<DesktopWindowTarget> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateDesktopWindowTarget)(
                windows_core::Interface::as_raw(self),
                hwndtarget,
                istopmost.into(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositorDesktopInterop_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub CreateDesktopWindowTarget: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        HWND,
        windows_core::BOOL,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    EnsureOnThread: usize,
}
windows_core::imp::define_interface!(
    ICompositorInterop,
    ICompositorInterop_Vtbl,
    0x25297d5c_3ad4_4c9c_b5cf_e36a38512330
);
windows_core::imp::interface_hierarchy!(ICompositorInterop, windows_core::IUnknown);
impl ICompositorInterop {
    pub(crate) unsafe fn CreateGraphicsDevice<P0>(
        &self,
        renderingdevice: P0,
    ) -> windows_core::Result<CompositionGraphicsDevice>
    where
        P0: windows_core::Param<windows_core::IUnknown>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateGraphicsDevice)(
                windows_core::Interface::as_raw(self),
                renderingdevice.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositorInterop_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    CreateCompositionSurfaceForHandle: usize,
    CreateCompositionSurfaceForSwapChain: usize,
    pub CreateGraphicsDevice: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IContainerVisual,
    IContainerVisual_Vtbl,
    0x02f6bc74_ed20_4773_afe6_d49b4a93db32
);
impl windows_core::RuntimeType for IContainerVisual {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IContainerVisual {
    pub(crate) fn Children(&self) -> windows_core::Result<VisualCollection> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Children)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IContainerVisual_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Children: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICubicBezierEasingFunction,
    ICubicBezierEasingFunction_Vtbl,
    0x32350666_c1e8_44f9_96b8_c98acf0ae698
);
impl windows_core::RuntimeType for ICubicBezierEasingFunction {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICubicBezierEasingFunction_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
pub const IDC_ARROW: windows_core::PCWSTR = windows_core::PCWSTR(32512 as _);
pub const IDC_HAND: windows_core::PCWSTR = windows_core::PCWSTR(32649 as _);
pub const IDC_IBEAM: windows_core::PCWSTR = windows_core::PCWSTR(32513 as _);
pub const IDC_SIZEALL: windows_core::PCWSTR = windows_core::PCWSTR(32646 as _);
pub const IDC_SIZENESW: windows_core::PCWSTR = windows_core::PCWSTR(32643 as _);
pub const IDC_SIZENS: windows_core::PCWSTR = windows_core::PCWSTR(32645 as _);
pub const IDC_SIZENWSE: windows_core::PCWSTR = windows_core::PCWSTR(32642 as _);
pub const IDC_SIZEWE: windows_core::PCWSTR = windows_core::PCWSTR(32644 as _);
windows_core::imp::define_interface!(
    IDataObject,
    IDataObject_Vtbl,
    0x0000010e_0000_0000_c000_000000000046
);
windows_core::imp::interface_hierarchy!(IDataObject, windows_core::IUnknown);
#[repr(C)]
pub struct IDataObject_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetData: usize,
    GetDataHere: usize,
    QueryGetData: usize,
    GetCanonicalFormatEtc: usize,
    SetData: usize,
    EnumFormatEtc: usize,
    DAdvise: usize,
    DUnadvise: usize,
    EnumDAdvise: usize,
}
windows_core::imp::define_interface!(
    IDesktopWindowTarget,
    IDesktopWindowTarget_Vtbl,
    0x6329d6ca_3366_490e_9db3_25312929ac51
);
impl windows_core::RuntimeType for IDesktopWindowTarget {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IDesktopWindowTarget_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IDispatch,
    IDispatch_Vtbl,
    0x00020400_0000_0000_c000_000000000046
);
windows_core::imp::interface_hierarchy!(IDispatch, windows_core::IUnknown);
#[repr(C)]
pub struct IDispatch_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetTypeInfoCount: usize,
    GetTypeInfo: usize,
    GetIDsOfNames: usize,
    Invoke: usize,
}
windows_core::imp::define_interface!(
    IDispatcherQueue,
    IDispatcherQueue_Vtbl,
    0x603e88e4_a338_4ffe_a457_a5cfb9ceb899
);
impl windows_core::RuntimeType for IDispatcherQueue {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDispatcherQueue {
    pub(crate) fn TryEnqueue<P0>(&self, callback: P0) -> windows_core::Result<bool>
    where
        P0: windows_core::Param<DispatcherQueueHandler>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TryEnqueue)(
                windows_core::Interface::as_raw(self),
                callback.param().abi(),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IDispatcherQueue_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    CreateTimer: usize,
    pub TryEnqueue: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut bool,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDispatcherQueueController,
    IDispatcherQueueController_Vtbl,
    0x22f34e66_50db_4e36_a98d_61c01b384d20
);
impl windows_core::RuntimeType for IDispatcherQueueController {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDispatcherQueueController {
    pub(crate) fn DispatcherQueue(&self) -> windows_core::Result<DispatcherQueue> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).DispatcherQueue)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn ShutdownQueueAsync(&self) -> windows_core::Result<windows_future::IAsyncAction> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ShutdownQueueAsync)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDispatcherQueueController_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub DispatcherQueue: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ShutdownQueueAsync: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IExpandCollapseProvider,
    IExpandCollapseProvider_Vtbl,
    0xd847d3a5_cab0_4a98_8c32_ecb45c59ad24
);
windows_core::imp::interface_hierarchy!(IExpandCollapseProvider, windows_core::IUnknown);
#[repr(C)]
pub struct IExpandCollapseProvider_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Expand: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Collapse: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub ExpandCollapseState: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut ExpandCollapseState,
    ) -> windows_core::HRESULT,
}
pub trait IExpandCollapseProvider_Impl: windows_core::IUnknownImpl {
    fn Expand(&self) -> windows_core::Result<()>;
    fn Collapse(&self) -> windows_core::Result<()>;
    fn ExpandCollapseState(&self) -> windows_core::Result<ExpandCollapseState>;
}
impl IExpandCollapseProvider_Vtbl {
    pub const fn new<Identity: IExpandCollapseProvider_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn Expand<
            Identity: IExpandCollapseProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IExpandCollapseProvider_Impl::Expand(this).into()
            }
        }
        unsafe extern "system" fn Collapse<
            Identity: IExpandCollapseProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IExpandCollapseProvider_Impl::Collapse(this).into()
            }
        }
        unsafe extern "system" fn ExpandCollapseState<
            Identity: IExpandCollapseProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut ExpandCollapseState,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IExpandCollapseProvider_Impl::ExpandCollapseState(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            Expand: Expand::<Identity, OFFSET>,
            Collapse: Collapse::<Identity, OFFSET>,
            ExpandCollapseState: ExpandCollapseState::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IExpandCollapseProvider as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IExpandCollapseProvider {}
windows_core::imp::define_interface!(
    IExpressionAnimation,
    IExpressionAnimation_Vtbl,
    0x6acc5431_7d3d_4bf3_abb6_f44bdc4888c1
);
impl windows_core::RuntimeType for IExpressionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IExpressionAnimation {
    pub(crate) fn SetExpression(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetExpression)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IExpressionAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Expression: usize,
    pub SetExpression: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
// ── Windows.Graphics.Effects + the D2D effect interop — copied verbatim from
// the generated bindings (Windows/Graphics/Effects/mod.rs and
// Windows/Win32/System/WinRT/Graphics/Direct2D/mod.rs). `GetProperty` is typed
// `IInspectable` here instead of `IPropertyValue` (ABI-identical transparent
// interface pointer); only boxed `PropertyValue` instances are ever returned. ──
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GRAPHICS_EFFECT_PROPERTY_MAPPING(pub i32);
pub const GRAPHICS_EFFECT_PROPERTY_MAPPING_DIRECT: GRAPHICS_EFFECT_PROPERTY_MAPPING =
    GRAPHICS_EFFECT_PROPERTY_MAPPING(1);
pub const GRAPHICS_EFFECT_PROPERTY_MAPPING_UNKNOWN: GRAPHICS_EFFECT_PROPERTY_MAPPING =
    GRAPHICS_EFFECT_PROPERTY_MAPPING(0);
windows_core::imp::define_interface!(
    IGraphicsEffect,
    IGraphicsEffect_Vtbl,
    0xcb51c0ce_8fe6_4636_b202_861faa07d8f3
);
impl windows_core::RuntimeType for IGraphicsEffect {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
    const NAME: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"Windows.Graphics.Effects.IGraphicsEffect");
}
windows_core::imp::interface_hierarchy!(
    IGraphicsEffect,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(IGraphicsEffect, IGraphicsEffectSource);
impl windows_core::RuntimeName for IGraphicsEffect {
    const NAME: &'static str = "Windows.Graphics.Effects.IGraphicsEffect";
}
pub trait IGraphicsEffect_Impl: IGraphicsEffectSource_Impl {
    fn Name(&self) -> windows_core::Result<windows_core::HSTRING>;
    fn SetName(&self, name: &windows_core::HSTRING) -> windows_core::Result<()>;
}
impl IGraphicsEffect_Vtbl {
    pub const fn new<Identity: IGraphicsEffect_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn Name<Identity: IGraphicsEffect_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            result__: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IGraphicsEffect_Impl::Name(this) {
                    Ok(ok__) => {
                        result__.write(core::mem::transmute_copy(&ok__));
                        core::mem::forget(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn SetName<Identity: IGraphicsEffect_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            name: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IGraphicsEffect_Impl::SetName(this, core::mem::transmute(&name)).into()
            }
        }
        Self {
            base__: windows_core::IInspectable_Vtbl::new::<Identity, IGraphicsEffect, OFFSET>(),
            Name: Name::<Identity, OFFSET>,
            SetName: SetName::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IGraphicsEffect as windows_core::Interface>::IID
    }
}
#[repr(C)]
pub struct IGraphicsEffect_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Name: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetName: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IGraphicsEffectD2D1Interop,
    IGraphicsEffectD2D1Interop_Vtbl,
    0x2fc57384_a068_44d7_a331_30982fcf7177
);
windows_core::imp::interface_hierarchy!(IGraphicsEffectD2D1Interop, windows_core::IUnknown);
#[repr(C)]
pub struct IGraphicsEffectD2D1Interop_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetEffectId: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::GUID,
    ) -> windows_core::HRESULT,
    pub GetNamedPropertyMapping: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::PCWSTR,
        *mut u32,
        *mut GRAPHICS_EFFECT_PROPERTY_MAPPING,
    ) -> windows_core::HRESULT,
    pub GetPropertyCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub GetProperty: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetSource: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetSourceCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
}
pub trait IGraphicsEffectD2D1Interop_Impl: windows_core::IUnknownImpl {
    fn GetEffectId(&self) -> windows_core::Result<windows_core::GUID>;
    fn GetNamedPropertyMapping(
        &self,
        name: &windows_core::PCWSTR,
        index: *mut u32,
        mapping: *mut GRAPHICS_EFFECT_PROPERTY_MAPPING,
    ) -> windows_core::Result<()>;
    fn GetPropertyCount(&self) -> windows_core::Result<u32>;
    fn GetProperty(&self, index: u32) -> windows_core::Result<windows_core::IInspectable>;
    fn GetSource(&self, index: u32) -> windows_core::Result<IGraphicsEffectSource>;
    fn GetSourceCount(&self) -> windows_core::Result<u32>;
}
impl IGraphicsEffectD2D1Interop_Vtbl {
    pub const fn new<Identity: IGraphicsEffectD2D1Interop_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn GetEffectId<
            Identity: IGraphicsEffectD2D1Interop_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            id: *mut windows_core::GUID,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IGraphicsEffectD2D1Interop_Impl::GetEffectId(this) {
                    Ok(ok__) => {
                        id.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetNamedPropertyMapping<
            Identity: IGraphicsEffectD2D1Interop_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            name: windows_core::PCWSTR,
            index: *mut u32,
            mapping: *mut GRAPHICS_EFFECT_PROPERTY_MAPPING,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IGraphicsEffectD2D1Interop_Impl::GetNamedPropertyMapping(
                    this,
                    core::mem::transmute(&name),
                    core::mem::transmute_copy(&index),
                    core::mem::transmute_copy(&mapping),
                )
                .into()
            }
        }
        unsafe extern "system" fn GetPropertyCount<
            Identity: IGraphicsEffectD2D1Interop_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            count: *mut u32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IGraphicsEffectD2D1Interop_Impl::GetPropertyCount(this) {
                    Ok(ok__) => {
                        count.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetProperty<
            Identity: IGraphicsEffectD2D1Interop_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            index: u32,
            value: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IGraphicsEffectD2D1Interop_Impl::GetProperty(
                    this,
                    core::mem::transmute_copy(&index),
                ) {
                    Ok(ok__) => {
                        value.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetSource<
            Identity: IGraphicsEffectD2D1Interop_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            index: u32,
            source: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IGraphicsEffectD2D1Interop_Impl::GetSource(
                    this,
                    core::mem::transmute_copy(&index),
                ) {
                    Ok(ok__) => {
                        source.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetSourceCount<
            Identity: IGraphicsEffectD2D1Interop_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            count: *mut u32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IGraphicsEffectD2D1Interop_Impl::GetSourceCount(this) {
                    Ok(ok__) => {
                        count.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            GetEffectId: GetEffectId::<Identity, OFFSET>,
            GetNamedPropertyMapping: GetNamedPropertyMapping::<Identity, OFFSET>,
            GetPropertyCount: GetPropertyCount::<Identity, OFFSET>,
            GetProperty: GetProperty::<Identity, OFFSET>,
            GetSource: GetSource::<Identity, OFFSET>,
            GetSourceCount: GetSourceCount::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IGraphicsEffectD2D1Interop as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IGraphicsEffectD2D1Interop {}
windows_core::imp::define_interface!(
    IGraphicsEffectSource,
    IGraphicsEffectSource_Vtbl,
    0x2d8f9ddc_4339_4eb9_9216_f9deb75658a2
);
impl windows_core::RuntimeType for IGraphicsEffectSource {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
    const NAME: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"Windows.Graphics.Effects.IGraphicsEffectSource",
    );
}
windows_core::imp::interface_hierarchy!(
    IGraphicsEffectSource,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeName for IGraphicsEffectSource {
    const NAME: &'static str = "Windows.Graphics.Effects.IGraphicsEffectSource";
}
pub trait IGraphicsEffectSource_Impl: windows_core::IUnknownImpl {}
impl IGraphicsEffectSource_Vtbl {
    pub const fn new<Identity: IGraphicsEffectSource_Impl, const OFFSET: isize>() -> Self {
        Self {
            base__: windows_core::IInspectable_Vtbl::new::<Identity, IGraphicsEffectSource, OFFSET>(
            ),
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IGraphicsEffectSource as windows_core::Interface>::IID
    }
}
#[repr(C)]
pub struct IGraphicsEffectSource_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IInsetClip,
    IInsetClip_Vtbl,
    0x1e73e647_84c7_477a_b474_5880e0442e15
);
impl windows_core::RuntimeType for IInsetClip {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInsetClip {
    pub(crate) fn SetBottomInset(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetBottomInset)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetLeftInset(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetLeftInset)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetRightInset(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetRightInset)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetTopInset(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTopInset)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IInsetClip_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    BottomInset: usize,
    pub SetBottomInset:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    LeftInset: usize,
    pub SetLeftInset:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    RightInset: usize,
    pub SetRightInset:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    TopInset: usize,
    pub SetTopInset:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInvokeProvider,
    IInvokeProvider_Vtbl,
    0x54fcb24b_e18e_47a2_b4d3_eccbe77599a2
);
windows_core::imp::interface_hierarchy!(IInvokeProvider, windows_core::IUnknown);
#[repr(C)]
pub struct IInvokeProvider_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Invoke: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
pub trait IInvokeProvider_Impl: windows_core::IUnknownImpl {
    fn Invoke(&self) -> windows_core::Result<()>;
}
impl IInvokeProvider_Vtbl {
    pub const fn new<Identity: IInvokeProvider_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn Invoke<Identity: IInvokeProvider_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IInvokeProvider_Impl::Invoke(this).into()
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            Invoke: Invoke::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IInvokeProvider as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IInvokeProvider {}
windows_core::imp::define_interface!(
    IKeyFrameAnimation,
    IKeyFrameAnimation_Vtbl,
    0x126e7f22_3ae9_4540_9a8a_deae8a4a4a84
);
impl windows_core::RuntimeType for IKeyFrameAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IKeyFrameAnimation {
    pub(crate) fn SetDelayTime(&self, value: TimeSpan) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetDelayTime)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetDuration(&self, value: TimeSpan) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetDuration)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetIterationBehavior(
        &self,
        value: AnimationIterationBehavior,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIterationBehavior)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetIterationCount(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIterationCount)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn InsertExpressionKeyFrameWithEasingFunction<P2>(
        &self,
        normalizedprogresskey: f32,
        value: &str,
        easingfunction: P2,
    ) -> windows_core::Result<()>
    where
        P2: windows_core::Param<CompositionEasingFunction>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InsertExpressionKeyFrameWithEasingFunction)(
                windows_core::Interface::as_raw(self),
                normalizedprogresskey,
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
                easingfunction.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IKeyFrameAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    DelayTime: usize,
    pub SetDelayTime:
        unsafe extern "system" fn(*mut core::ffi::c_void, TimeSpan) -> windows_core::HRESULT,
    Duration: usize,
    pub SetDuration:
        unsafe extern "system" fn(*mut core::ffi::c_void, TimeSpan) -> windows_core::HRESULT,
    IterationBehavior: usize,
    pub SetIterationBehavior: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        AnimationIterationBehavior,
    ) -> windows_core::HRESULT,
    IterationCount: usize,
    pub SetIterationCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    KeyFrameCount: usize,
    StopBehavior: usize,
    SetStopBehavior: usize,
    InsertExpressionKeyFrame: usize,
    pub InsertExpressionKeyFrameWithEasingFunction:
        unsafe extern "system" fn(
            *mut core::ffi::c_void,
            f32,
            *mut core::ffi::c_void,
            *mut core::ffi::c_void,
        ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ILinearEasingFunction,
    ILinearEasingFunction_Vtbl,
    0x9400975a_c7a6_46b3_acf7_1a268a0a117d
);
impl windows_core::RuntimeType for ILinearEasingFunction {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ILinearEasingFunction_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
pub type IME_COMPOSITION_STRING = u32;
windows_core::imp::define_interface!(
    IRangeValueProvider,
    IRangeValueProvider_Vtbl,
    0x36dc7aef_33e6_4691_afe1_2be7274b3d33
);
windows_core::imp::interface_hierarchy!(IRangeValueProvider, windows_core::IUnknown);
#[repr(C)]
pub struct IRangeValueProvider_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub SetValue: unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    pub Value: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub IsReadOnly: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub Maximum:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub Minimum:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub LargeChange:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub SmallChange:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
}
pub trait IRangeValueProvider_Impl: windows_core::IUnknownImpl {
    fn SetValue(&self, val: f64) -> windows_core::Result<()>;
    fn Value(&self) -> windows_core::Result<f64>;
    fn IsReadOnly(&self) -> windows_core::Result<windows_core::BOOL>;
    fn Maximum(&self) -> windows_core::Result<f64>;
    fn Minimum(&self) -> windows_core::Result<f64>;
    fn LargeChange(&self) -> windows_core::Result<f64>;
    fn SmallChange(&self) -> windows_core::Result<f64>;
}
impl IRangeValueProvider_Vtbl {
    pub const fn new<Identity: IRangeValueProvider_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn SetValue<
            Identity: IRangeValueProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            val: f64,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IRangeValueProvider_Impl::SetValue(this, core::mem::transmute_copy(&val)).into()
            }
        }
        unsafe extern "system" fn Value<Identity: IRangeValueProvider_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            pretval: *mut f64,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRangeValueProvider_Impl::Value(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn IsReadOnly<
            Identity: IRangeValueProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut windows_core::BOOL,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRangeValueProvider_Impl::IsReadOnly(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn Maximum<
            Identity: IRangeValueProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut f64,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRangeValueProvider_Impl::Maximum(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn Minimum<
            Identity: IRangeValueProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut f64,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRangeValueProvider_Impl::Minimum(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn LargeChange<
            Identity: IRangeValueProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut f64,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRangeValueProvider_Impl::LargeChange(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn SmallChange<
            Identity: IRangeValueProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut f64,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRangeValueProvider_Impl::SmallChange(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            SetValue: SetValue::<Identity, OFFSET>,
            Value: Value::<Identity, OFFSET>,
            IsReadOnly: IsReadOnly::<Identity, OFFSET>,
            Maximum: Maximum::<Identity, OFFSET>,
            Minimum: Minimum::<Identity, OFFSET>,
            LargeChange: LargeChange::<Identity, OFFSET>,
            SmallChange: SmallChange::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IRangeValueProvider as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IRangeValueProvider {}
windows_core::imp::define_interface!(
    IRawElementProviderFragment,
    IRawElementProviderFragment_Vtbl,
    0xf7063da8_8359_439c_9297_bbc5299a7d87
);
windows_core::imp::interface_hierarchy!(IRawElementProviderFragment, windows_core::IUnknown);
#[repr(C)]
pub struct IRawElementProviderFragment_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Navigate: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        NavigateDirection,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetRuntimeId: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut SAFEARRAY,
    ) -> windows_core::HRESULT,
    pub BoundingRectangle:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut UiaRect) -> windows_core::HRESULT,
    pub GetEmbeddedFragmentRoots: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut SAFEARRAY,
    ) -> windows_core::HRESULT,
    pub SetFocus: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub FragmentRoot: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
pub trait IRawElementProviderFragment_Impl: windows_core::IUnknownImpl {
    fn Navigate(
        &self,
        direction: NavigateDirection,
    ) -> windows_core::Result<IRawElementProviderFragment>;
    fn GetRuntimeId(&self) -> windows_core::Result<*mut SAFEARRAY>;
    fn BoundingRectangle(&self) -> windows_core::Result<UiaRect>;
    fn GetEmbeddedFragmentRoots(&self) -> windows_core::Result<*mut SAFEARRAY>;
    fn SetFocus(&self) -> windows_core::Result<()>;
    fn FragmentRoot(&self) -> windows_core::Result<IRawElementProviderFragmentRoot>;
}
impl IRawElementProviderFragment_Vtbl {
    pub const fn new<Identity: IRawElementProviderFragment_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn Navigate<
            Identity: IRawElementProviderFragment_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            direction: NavigateDirection,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRawElementProviderFragment_Impl::Navigate(
                    this,
                    core::mem::transmute_copy(&direction),
                ) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetRuntimeId<
            Identity: IRawElementProviderFragment_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut SAFEARRAY,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRawElementProviderFragment_Impl::GetRuntimeId(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn BoundingRectangle<
            Identity: IRawElementProviderFragment_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut UiaRect,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRawElementProviderFragment_Impl::BoundingRectangle(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetEmbeddedFragmentRoots<
            Identity: IRawElementProviderFragment_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut SAFEARRAY,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRawElementProviderFragment_Impl::GetEmbeddedFragmentRoots(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn SetFocus<
            Identity: IRawElementProviderFragment_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IRawElementProviderFragment_Impl::SetFocus(this).into()
            }
        }
        unsafe extern "system" fn FragmentRoot<
            Identity: IRawElementProviderFragment_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRawElementProviderFragment_Impl::FragmentRoot(this) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            Navigate: Navigate::<Identity, OFFSET>,
            GetRuntimeId: GetRuntimeId::<Identity, OFFSET>,
            BoundingRectangle: BoundingRectangle::<Identity, OFFSET>,
            GetEmbeddedFragmentRoots: GetEmbeddedFragmentRoots::<Identity, OFFSET>,
            SetFocus: SetFocus::<Identity, OFFSET>,
            FragmentRoot: FragmentRoot::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IRawElementProviderFragment as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IRawElementProviderFragment {}
windows_core::imp::define_interface!(
    IRawElementProviderFragmentRoot,
    IRawElementProviderFragmentRoot_Vtbl,
    0x620ce2a5_ab8f_40a9_86cb_de3c75599b58
);
windows_core::imp::interface_hierarchy!(IRawElementProviderFragmentRoot, windows_core::IUnknown);
#[repr(C)]
pub struct IRawElementProviderFragmentRoot_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub ElementProviderFromPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f64,
        f64,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetFocus: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
pub trait IRawElementProviderFragmentRoot_Impl: windows_core::IUnknownImpl {
    fn ElementProviderFromPoint(
        &self,
        x: f64,
        y: f64,
    ) -> windows_core::Result<IRawElementProviderFragment>;
    fn GetFocus(&self) -> windows_core::Result<IRawElementProviderFragment>;
}
impl IRawElementProviderFragmentRoot_Vtbl {
    pub const fn new<Identity: IRawElementProviderFragmentRoot_Impl, const OFFSET: isize>() -> Self
    {
        unsafe extern "system" fn ElementProviderFromPoint<
            Identity: IRawElementProviderFragmentRoot_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            x: f64,
            y: f64,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRawElementProviderFragmentRoot_Impl::ElementProviderFromPoint(
                    this,
                    core::mem::transmute_copy(&x),
                    core::mem::transmute_copy(&y),
                ) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetFocus<
            Identity: IRawElementProviderFragmentRoot_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRawElementProviderFragmentRoot_Impl::GetFocus(this) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            ElementProviderFromPoint: ElementProviderFromPoint::<Identity, OFFSET>,
            GetFocus: GetFocus::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IRawElementProviderFragmentRoot as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IRawElementProviderFragmentRoot {}
windows_core::imp::define_interface!(
    IRawElementProviderSimple,
    IRawElementProviderSimple_Vtbl,
    0xd6dd68d1_86fd_4332_8666_9abedea2d24c
);
windows_core::imp::interface_hierarchy!(IRawElementProviderSimple, windows_core::IUnknown);
#[repr(C)]
pub struct IRawElementProviderSimple_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub ProviderOptions: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut ProviderOptions,
    ) -> windows_core::HRESULT,
    pub GetPatternProvider: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        UIA_PATTERN_ID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetPropertyValue: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        UIA_PROPERTY_ID,
        *mut VARIANT,
    ) -> windows_core::HRESULT,
    pub HostRawElementProvider: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
pub trait IRawElementProviderSimple_Impl: windows_core::IUnknownImpl {
    fn ProviderOptions(&self) -> windows_core::Result<ProviderOptions>;
    fn GetPatternProvider(
        &self,
        patternid: UIA_PATTERN_ID,
    ) -> windows_core::Result<windows_core::IUnknown>;
    fn GetPropertyValue(&self, propertyid: UIA_PROPERTY_ID) -> windows_core::Result<VARIANT>;
    fn HostRawElementProvider(&self) -> windows_core::Result<IRawElementProviderSimple>;
}
impl IRawElementProviderSimple_Vtbl {
    pub const fn new<Identity: IRawElementProviderSimple_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn ProviderOptions<
            Identity: IRawElementProviderSimple_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut ProviderOptions,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRawElementProviderSimple_Impl::ProviderOptions(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetPatternProvider<
            Identity: IRawElementProviderSimple_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            patternid: UIA_PATTERN_ID,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRawElementProviderSimple_Impl::GetPatternProvider(
                    this,
                    core::mem::transmute_copy(&patternid),
                ) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetPropertyValue<
            Identity: IRawElementProviderSimple_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            propertyid: UIA_PROPERTY_ID,
            pretval: *mut VARIANT,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRawElementProviderSimple_Impl::GetPropertyValue(
                    this,
                    core::mem::transmute_copy(&propertyid),
                ) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn HostRawElementProvider<
            Identity: IRawElementProviderSimple_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRawElementProviderSimple_Impl::HostRawElementProvider(this) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            ProviderOptions: ProviderOptions::<Identity, OFFSET>,
            GetPatternProvider: GetPatternProvider::<Identity, OFFSET>,
            GetPropertyValue: GetPropertyValue::<Identity, OFFSET>,
            HostRawElementProvider: HostRawElementProvider::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IRawElementProviderSimple as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IRawElementProviderSimple {}
windows_core::imp::define_interface!(
    IRecordInfo,
    IRecordInfo_Vtbl,
    0x0000002f_0000_0000_c000_000000000046
);
windows_core::imp::interface_hierarchy!(IRecordInfo, windows_core::IUnknown);
#[repr(C)]
pub struct IRecordInfo_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    RecordInit: usize,
    RecordClear: usize,
    RecordCopy: usize,
    GetGuid: usize,
    GetName: usize,
    GetSize: usize,
    GetTypeInfo: usize,
    GetField: usize,
    GetFieldNoCopy: usize,
    PutField: usize,
    PutFieldNoCopy: usize,
    GetFieldNames: usize,
    IsMatchingType: usize,
    RecordCreate: usize,
    RecordCreateCopy: usize,
    RecordDestroy: usize,
}
windows_core::imp::define_interface!(
    IScalarKeyFrameAnimation,
    IScalarKeyFrameAnimation_Vtbl,
    0xae288fa9_252c_4b95_a725_bf85e38000a1
);
impl windows_core::RuntimeType for IScalarKeyFrameAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IScalarKeyFrameAnimation {
    pub(crate) fn InsertKeyFrame(
        &self,
        normalizedprogresskey: f32,
        value: f32,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).InsertKeyFrame)(
                windows_core::Interface::as_raw(self),
                normalizedprogresskey,
                value,
            )
            .ok()
        }
    }
    pub(crate) fn InsertKeyFrameWithEasingFunction<P2>(
        &self,
        normalizedprogresskey: f32,
        value: f32,
        easingfunction: P2,
    ) -> windows_core::Result<()>
    where
        P2: windows_core::Param<CompositionEasingFunction>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InsertKeyFrameWithEasingFunction)(
                windows_core::Interface::as_raw(self),
                normalizedprogresskey,
                value,
                easingfunction.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IScalarKeyFrameAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub InsertKeyFrame:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32, f32) -> windows_core::HRESULT,
    pub InsertKeyFrameWithEasingFunction: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        f32,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IScrollProvider,
    IScrollProvider_Vtbl,
    0xb38b8077_1fc3_42a5_8cae_d40c2215055a
);
windows_core::imp::interface_hierarchy!(IScrollProvider, windows_core::IUnknown);
#[repr(C)]
pub struct IScrollProvider_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Scroll: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        ScrollAmount,
        ScrollAmount,
    ) -> windows_core::HRESULT,
    pub SetScrollPercent:
        unsafe extern "system" fn(*mut core::ffi::c_void, f64, f64) -> windows_core::HRESULT,
    pub HorizontalScrollPercent:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub VerticalScrollPercent:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub HorizontalViewSize:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub VerticalViewSize:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub HorizontallyScrollable: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub VerticallyScrollable: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
}
pub trait IScrollProvider_Impl: windows_core::IUnknownImpl {
    fn Scroll(
        &self,
        horizontalamount: ScrollAmount,
        verticalamount: ScrollAmount,
    ) -> windows_core::Result<()>;
    fn SetScrollPercent(
        &self,
        horizontalpercent: f64,
        verticalpercent: f64,
    ) -> windows_core::Result<()>;
    fn HorizontalScrollPercent(&self) -> windows_core::Result<f64>;
    fn VerticalScrollPercent(&self) -> windows_core::Result<f64>;
    fn HorizontalViewSize(&self) -> windows_core::Result<f64>;
    fn VerticalViewSize(&self) -> windows_core::Result<f64>;
    fn HorizontallyScrollable(&self) -> windows_core::Result<windows_core::BOOL>;
    fn VerticallyScrollable(&self) -> windows_core::Result<windows_core::BOOL>;
}
impl IScrollProvider_Vtbl {
    pub const fn new<Identity: IScrollProvider_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn Scroll<Identity: IScrollProvider_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            horizontalamount: ScrollAmount,
            verticalamount: ScrollAmount,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IScrollProvider_Impl::Scroll(
                    this,
                    core::mem::transmute_copy(&horizontalamount),
                    core::mem::transmute_copy(&verticalamount),
                )
                .into()
            }
        }
        unsafe extern "system" fn SetScrollPercent<
            Identity: IScrollProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            horizontalpercent: f64,
            verticalpercent: f64,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IScrollProvider_Impl::SetScrollPercent(
                    this,
                    core::mem::transmute_copy(&horizontalpercent),
                    core::mem::transmute_copy(&verticalpercent),
                )
                .into()
            }
        }
        unsafe extern "system" fn HorizontalScrollPercent<
            Identity: IScrollProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut f64,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IScrollProvider_Impl::HorizontalScrollPercent(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn VerticalScrollPercent<
            Identity: IScrollProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut f64,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IScrollProvider_Impl::VerticalScrollPercent(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn HorizontalViewSize<
            Identity: IScrollProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut f64,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IScrollProvider_Impl::HorizontalViewSize(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn VerticalViewSize<
            Identity: IScrollProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut f64,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IScrollProvider_Impl::VerticalViewSize(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn HorizontallyScrollable<
            Identity: IScrollProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut windows_core::BOOL,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IScrollProvider_Impl::HorizontallyScrollable(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn VerticallyScrollable<
            Identity: IScrollProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut windows_core::BOOL,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IScrollProvider_Impl::VerticallyScrollable(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            Scroll: Scroll::<Identity, OFFSET>,
            SetScrollPercent: SetScrollPercent::<Identity, OFFSET>,
            HorizontalScrollPercent: HorizontalScrollPercent::<Identity, OFFSET>,
            VerticalScrollPercent: VerticalScrollPercent::<Identity, OFFSET>,
            HorizontalViewSize: HorizontalViewSize::<Identity, OFFSET>,
            VerticalViewSize: VerticalViewSize::<Identity, OFFSET>,
            HorizontallyScrollable: HorizontallyScrollable::<Identity, OFFSET>,
            VerticallyScrollable: VerticallyScrollable::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IScrollProvider as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IScrollProvider {}
windows_core::imp::define_interface!(
    ISelectionItemProvider,
    ISelectionItemProvider_Vtbl,
    0x2acad808_b2d4_452d_a407_91ff1ad167b2
);
windows_core::imp::interface_hierarchy!(ISelectionItemProvider, windows_core::IUnknown);
#[repr(C)]
pub struct ISelectionItemProvider_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Select: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub AddToSelection: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub RemoveFromSelection:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub IsSelected: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub SelectionContainer: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
pub trait ISelectionItemProvider_Impl: windows_core::IUnknownImpl {
    fn Select(&self) -> windows_core::Result<()>;
    fn AddToSelection(&self) -> windows_core::Result<()>;
    fn RemoveFromSelection(&self) -> windows_core::Result<()>;
    fn IsSelected(&self) -> windows_core::Result<windows_core::BOOL>;
    fn SelectionContainer(&self) -> windows_core::Result<IRawElementProviderSimple>;
}
impl ISelectionItemProvider_Vtbl {
    pub const fn new<Identity: ISelectionItemProvider_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn Select<
            Identity: ISelectionItemProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ISelectionItemProvider_Impl::Select(this).into()
            }
        }
        unsafe extern "system" fn AddToSelection<
            Identity: ISelectionItemProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ISelectionItemProvider_Impl::AddToSelection(this).into()
            }
        }
        unsafe extern "system" fn RemoveFromSelection<
            Identity: ISelectionItemProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ISelectionItemProvider_Impl::RemoveFromSelection(this).into()
            }
        }
        unsafe extern "system" fn IsSelected<
            Identity: ISelectionItemProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut windows_core::BOOL,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ISelectionItemProvider_Impl::IsSelected(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn SelectionContainer<
            Identity: ISelectionItemProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ISelectionItemProvider_Impl::SelectionContainer(this) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            Select: Select::<Identity, OFFSET>,
            AddToSelection: AddToSelection::<Identity, OFFSET>,
            RemoveFromSelection: RemoveFromSelection::<Identity, OFFSET>,
            IsSelected: IsSelected::<Identity, OFFSET>,
            SelectionContainer: SelectionContainer::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<ISelectionItemProvider as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for ISelectionItemProvider {}
windows_core::imp::define_interface!(
    ISelectionProvider,
    ISelectionProvider_Vtbl,
    0xfb8b03af_3bdf_48d4_bd36_1a65793be168
);
windows_core::imp::interface_hierarchy!(ISelectionProvider, windows_core::IUnknown);
#[repr(C)]
pub struct ISelectionProvider_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetSelection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut SAFEARRAY,
    ) -> windows_core::HRESULT,
    pub CanSelectMultiple: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub IsSelectionRequired: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
}
pub trait ISelectionProvider_Impl: windows_core::IUnknownImpl {
    fn GetSelection(&self) -> windows_core::Result<*mut SAFEARRAY>;
    fn CanSelectMultiple(&self) -> windows_core::Result<windows_core::BOOL>;
    fn IsSelectionRequired(&self) -> windows_core::Result<windows_core::BOOL>;
}
impl ISelectionProvider_Vtbl {
    pub const fn new<Identity: ISelectionProvider_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn GetSelection<
            Identity: ISelectionProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut SAFEARRAY,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ISelectionProvider_Impl::GetSelection(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn CanSelectMultiple<
            Identity: ISelectionProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut windows_core::BOOL,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ISelectionProvider_Impl::CanSelectMultiple(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn IsSelectionRequired<
            Identity: ISelectionProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut windows_core::BOOL,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ISelectionProvider_Impl::IsSelectionRequired(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            GetSelection: GetSelection::<Identity, OFFSET>,
            CanSelectMultiple: CanSelectMultiple::<Identity, OFFSET>,
            IsSelectionRequired: IsSelectionRequired::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<ISelectionProvider as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for ISelectionProvider {}
windows_core::imp::define_interface!(
    ISpriteVisual,
    ISpriteVisual_Vtbl,
    0x08e05581_1ad1_4f97_9757_402d76e4233b
);
impl windows_core::RuntimeType for ISpriteVisual {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ISpriteVisual {
    pub(crate) fn SetBrush<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<CompositionBrush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetBrush)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ISpriteVisual_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Brush: usize,
    pub SetBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ITextStoreACP,
    ITextStoreACP_Vtbl,
    0x28888fe3_c2a0_483a_a3ea_8cb1ce51ff3d
);
windows_core::imp::interface_hierarchy!(ITextStoreACP, windows_core::IUnknown);
#[repr(C)]
pub struct ITextStoreACP_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub AdviseSink: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut core::ffi::c_void,
        u32,
    ) -> windows_core::HRESULT,
    pub UnadviseSink: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub RequestLock: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut windows_core::HRESULT,
    ) -> windows_core::HRESULT,
    pub GetStatus:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut TS_STATUS) -> windows_core::HRESULT,
    pub QueryInsert: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        i32,
        u32,
        *mut i32,
        *mut i32,
    ) -> windows_core::HRESULT,
    pub GetSelection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        u32,
        *mut TS_SELECTION_ACP,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub SetSelection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *const TS_SELECTION_ACP,
    ) -> windows_core::HRESULT,
    pub GetText: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        i32,
        windows_core::PWSTR,
        u32,
        *mut u32,
        *mut TS_RUNINFO,
        u32,
        *mut u32,
        *mut i32,
    ) -> windows_core::HRESULT,
    pub SetText: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        i32,
        i32,
        windows_core::PCWSTR,
        u32,
        *mut TS_TEXTCHANGE,
    ) -> windows_core::HRESULT,
    pub GetFormattedText: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        i32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetEmbedded: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        *const windows_core::GUID,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub QueryInsertEmbedded: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *const FORMATETC,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub InsertEmbedded: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        i32,
        i32,
        *mut core::ffi::c_void,
        *mut TS_TEXTCHANGE,
    ) -> windows_core::HRESULT,
    pub InsertTextAtSelection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        windows_core::PCWSTR,
        u32,
        *mut i32,
        *mut i32,
        *mut TS_TEXTCHANGE,
    ) -> windows_core::HRESULT,
    pub InsertEmbeddedAtSelection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut core::ffi::c_void,
        *mut i32,
        *mut i32,
        *mut TS_TEXTCHANGE,
    ) -> windows_core::HRESULT,
    pub RequestSupportedAttrs: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        u32,
        *const windows_core::GUID,
    ) -> windows_core::HRESULT,
    pub RequestAttrsAtPosition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        u32,
        *const windows_core::GUID,
        u32,
    ) -> windows_core::HRESULT,
    pub RequestAttrsTransitioningAtPosition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        u32,
        *const windows_core::GUID,
        u32,
    )
        -> windows_core::HRESULT,
    pub FindNextAttrTransition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        i32,
        u32,
        *const windows_core::GUID,
        u32,
        *mut i32,
        *mut windows_core::BOOL,
        *mut i32,
    ) -> windows_core::HRESULT,
    pub RetrieveRequestedAttrs: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut TS_ATTRVAL,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub GetEndACP:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub GetActiveView:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub GetACPFromPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *const POINT,
        u32,
        *mut i32,
    ) -> windows_core::HRESULT,
    pub GetTextExt: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        i32,
        i32,
        *mut RECT,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub GetScreenExt:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32, *mut RECT) -> windows_core::HRESULT,
    pub GetWnd:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32, *mut HWND) -> windows_core::HRESULT,
}
pub trait ITextStoreACP_Impl: windows_core::IUnknownImpl {
    fn AdviseSink(
        &self,
        riid: *const windows_core::GUID,
        punk: windows_core::Ref<windows_core::IUnknown>,
        dwmask: u32,
    ) -> windows_core::Result<()>;
    fn UnadviseSink(
        &self,
        punk: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()>;
    fn RequestLock(&self, dwlockflags: u32) -> windows_core::Result<windows_core::HRESULT>;
    fn GetStatus(&self) -> windows_core::Result<TS_STATUS>;
    fn QueryInsert(
        &self,
        acpteststart: i32,
        acptestend: i32,
        cch: u32,
        pacpresultstart: *mut i32,
        pacpresultend: *mut i32,
    ) -> windows_core::Result<()>;
    fn GetSelection(
        &self,
        ulindex: u32,
        ulcount: u32,
        pselection: *mut TS_SELECTION_ACP,
        pcfetched: *mut u32,
    ) -> windows_core::Result<()>;
    fn SetSelection(
        &self,
        ulcount: u32,
        pselection: *const TS_SELECTION_ACP,
    ) -> windows_core::Result<()>;
    fn GetText(
        &self,
        acpstart: i32,
        acpend: i32,
        pchplain: windows_core::PWSTR,
        cchplainreq: u32,
        pcchplainret: *mut u32,
        prgruninfo: *mut TS_RUNINFO,
        cruninforeq: u32,
        pcruninforet: *mut u32,
        pacpnext: *mut i32,
    ) -> windows_core::Result<()>;
    fn SetText(
        &self,
        dwflags: u32,
        acpstart: i32,
        acpend: i32,
        pchtext: &windows_core::PCWSTR,
        cch: u32,
    ) -> windows_core::Result<TS_TEXTCHANGE>;
    fn GetFormattedText(&self, acpstart: i32, acpend: i32) -> windows_core::Result<IDataObject>;
    fn GetEmbedded(
        &self,
        acppos: i32,
        rguidservice: *const windows_core::GUID,
        riid: *const windows_core::GUID,
    ) -> windows_core::Result<windows_core::IUnknown>;
    fn QueryInsertEmbedded(
        &self,
        pguidservice: *const windows_core::GUID,
        pformatetc: *const FORMATETC,
    ) -> windows_core::Result<windows_core::BOOL>;
    fn InsertEmbedded(
        &self,
        dwflags: u32,
        acpstart: i32,
        acpend: i32,
        pdataobject: windows_core::Ref<IDataObject>,
    ) -> windows_core::Result<TS_TEXTCHANGE>;
    fn InsertTextAtSelection(
        &self,
        dwflags: u32,
        pchtext: &windows_core::PCWSTR,
        cch: u32,
        pacpstart: *mut i32,
        pacpend: *mut i32,
        pchange: *mut TS_TEXTCHANGE,
    ) -> windows_core::Result<()>;
    fn InsertEmbeddedAtSelection(
        &self,
        dwflags: u32,
        pdataobject: windows_core::Ref<IDataObject>,
        pacpstart: *mut i32,
        pacpend: *mut i32,
        pchange: *mut TS_TEXTCHANGE,
    ) -> windows_core::Result<()>;
    fn RequestSupportedAttrs(
        &self,
        dwflags: u32,
        cfilterattrs: u32,
        pafilterattrs: *const windows_core::GUID,
    ) -> windows_core::Result<()>;
    fn RequestAttrsAtPosition(
        &self,
        acppos: i32,
        cfilterattrs: u32,
        pafilterattrs: *const windows_core::GUID,
        dwflags: u32,
    ) -> windows_core::Result<()>;
    fn RequestAttrsTransitioningAtPosition(
        &self,
        acppos: i32,
        cfilterattrs: u32,
        pafilterattrs: *const windows_core::GUID,
        dwflags: u32,
    ) -> windows_core::Result<()>;
    fn FindNextAttrTransition(
        &self,
        acpstart: i32,
        acphalt: i32,
        cfilterattrs: u32,
        pafilterattrs: *const windows_core::GUID,
        dwflags: u32,
        pacpnext: *mut i32,
        pffound: *mut windows_core::BOOL,
        plfoundoffset: *mut i32,
    ) -> windows_core::Result<()>;
    fn RetrieveRequestedAttrs(
        &self,
        ulcount: u32,
        paattrvals: *mut TS_ATTRVAL,
        pcfetched: *mut u32,
    ) -> windows_core::Result<()>;
    fn GetEndACP(&self) -> windows_core::Result<i32>;
    fn GetActiveView(&self) -> windows_core::Result<u32>;
    fn GetACPFromPoint(
        &self,
        vcview: u32,
        ptscreen: *const POINT,
        dwflags: u32,
    ) -> windows_core::Result<i32>;
    fn GetTextExt(
        &self,
        vcview: u32,
        acpstart: i32,
        acpend: i32,
        prc: *mut RECT,
        pfclipped: *mut windows_core::BOOL,
    ) -> windows_core::Result<()>;
    fn GetScreenExt(&self, vcview: u32) -> windows_core::Result<RECT>;
    fn GetWnd(&self, vcview: u32) -> windows_core::Result<HWND>;
}
impl ITextStoreACP_Vtbl {
    pub const fn new<Identity: ITextStoreACP_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn AdviseSink<Identity: ITextStoreACP_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            riid: *const windows_core::GUID,
            punk: *mut core::ffi::c_void,
            dwmask: u32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextStoreACP_Impl::AdviseSink(
                    this,
                    core::mem::transmute_copy(&riid),
                    core::mem::transmute_copy(&punk),
                    core::mem::transmute_copy(&dwmask),
                )
                .into()
            }
        }
        unsafe extern "system" fn UnadviseSink<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            punk: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextStoreACP_Impl::UnadviseSink(this, core::mem::transmute_copy(&punk)).into()
            }
        }
        unsafe extern "system" fn RequestLock<Identity: ITextStoreACP_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            dwlockflags: u32,
            phrsession: *mut windows_core::HRESULT,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextStoreACP_Impl::RequestLock(this, core::mem::transmute_copy(&dwlockflags))
                {
                    Ok(ok__) => {
                        phrsession.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetStatus<Identity: ITextStoreACP_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            pdcs: *mut TS_STATUS,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextStoreACP_Impl::GetStatus(this) {
                    Ok(ok__) => {
                        pdcs.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn QueryInsert<Identity: ITextStoreACP_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            acpteststart: i32,
            acptestend: i32,
            cch: u32,
            pacpresultstart: *mut i32,
            pacpresultend: *mut i32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextStoreACP_Impl::QueryInsert(
                    this,
                    core::mem::transmute_copy(&acpteststart),
                    core::mem::transmute_copy(&acptestend),
                    core::mem::transmute_copy(&cch),
                    core::mem::transmute_copy(&pacpresultstart),
                    core::mem::transmute_copy(&pacpresultend),
                )
                .into()
            }
        }
        unsafe extern "system" fn GetSelection<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            ulindex: u32,
            ulcount: u32,
            pselection: *mut TS_SELECTION_ACP,
            pcfetched: *mut u32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextStoreACP_Impl::GetSelection(
                    this,
                    core::mem::transmute_copy(&ulindex),
                    core::mem::transmute_copy(&ulcount),
                    core::mem::transmute_copy(&pselection),
                    core::mem::transmute_copy(&pcfetched),
                )
                .into()
            }
        }
        unsafe extern "system" fn SetSelection<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            ulcount: u32,
            pselection: *const TS_SELECTION_ACP,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextStoreACP_Impl::SetSelection(
                    this,
                    core::mem::transmute_copy(&ulcount),
                    core::mem::transmute_copy(&pselection),
                )
                .into()
            }
        }
        unsafe extern "system" fn GetText<Identity: ITextStoreACP_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            acpstart: i32,
            acpend: i32,
            pchplain: windows_core::PWSTR,
            cchplainreq: u32,
            pcchplainret: *mut u32,
            prgruninfo: *mut TS_RUNINFO,
            cruninforeq: u32,
            pcruninforet: *mut u32,
            pacpnext: *mut i32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextStoreACP_Impl::GetText(
                    this,
                    core::mem::transmute_copy(&acpstart),
                    core::mem::transmute_copy(&acpend),
                    core::mem::transmute_copy(&pchplain),
                    core::mem::transmute_copy(&cchplainreq),
                    core::mem::transmute_copy(&pcchplainret),
                    core::mem::transmute_copy(&prgruninfo),
                    core::mem::transmute_copy(&cruninforeq),
                    core::mem::transmute_copy(&pcruninforet),
                    core::mem::transmute_copy(&pacpnext),
                )
                .into()
            }
        }
        unsafe extern "system" fn SetText<Identity: ITextStoreACP_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            dwflags: u32,
            acpstart: i32,
            acpend: i32,
            pchtext: windows_core::PCWSTR,
            cch: u32,
            pchange: *mut TS_TEXTCHANGE,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextStoreACP_Impl::SetText(
                    this,
                    core::mem::transmute_copy(&dwflags),
                    core::mem::transmute_copy(&acpstart),
                    core::mem::transmute_copy(&acpend),
                    core::mem::transmute(&pchtext),
                    core::mem::transmute_copy(&cch),
                ) {
                    Ok(ok__) => {
                        pchange.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetFormattedText<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            acpstart: i32,
            acpend: i32,
            ppdataobject: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextStoreACP_Impl::GetFormattedText(
                    this,
                    core::mem::transmute_copy(&acpstart),
                    core::mem::transmute_copy(&acpend),
                ) {
                    Ok(ok__) => {
                        ppdataobject.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetEmbedded<Identity: ITextStoreACP_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            acppos: i32,
            rguidservice: *const windows_core::GUID,
            riid: *const windows_core::GUID,
            ppunk: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextStoreACP_Impl::GetEmbedded(
                    this,
                    core::mem::transmute_copy(&acppos),
                    core::mem::transmute_copy(&rguidservice),
                    core::mem::transmute_copy(&riid),
                ) {
                    Ok(ok__) => {
                        ppunk.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn QueryInsertEmbedded<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pguidservice: *const windows_core::GUID,
            pformatetc: *const FORMATETC,
            pfinsertable: *mut windows_core::BOOL,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextStoreACP_Impl::QueryInsertEmbedded(
                    this,
                    core::mem::transmute_copy(&pguidservice),
                    core::mem::transmute_copy(&pformatetc),
                ) {
                    Ok(ok__) => {
                        pfinsertable.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn InsertEmbedded<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            dwflags: u32,
            acpstart: i32,
            acpend: i32,
            pdataobject: *mut core::ffi::c_void,
            pchange: *mut TS_TEXTCHANGE,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextStoreACP_Impl::InsertEmbedded(
                    this,
                    core::mem::transmute_copy(&dwflags),
                    core::mem::transmute_copy(&acpstart),
                    core::mem::transmute_copy(&acpend),
                    core::mem::transmute_copy(&pdataobject),
                ) {
                    Ok(ok__) => {
                        pchange.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn InsertTextAtSelection<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            dwflags: u32,
            pchtext: windows_core::PCWSTR,
            cch: u32,
            pacpstart: *mut i32,
            pacpend: *mut i32,
            pchange: *mut TS_TEXTCHANGE,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextStoreACP_Impl::InsertTextAtSelection(
                    this,
                    core::mem::transmute_copy(&dwflags),
                    core::mem::transmute(&pchtext),
                    core::mem::transmute_copy(&cch),
                    core::mem::transmute_copy(&pacpstart),
                    core::mem::transmute_copy(&pacpend),
                    core::mem::transmute_copy(&pchange),
                )
                .into()
            }
        }
        unsafe extern "system" fn InsertEmbeddedAtSelection<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            dwflags: u32,
            pdataobject: *mut core::ffi::c_void,
            pacpstart: *mut i32,
            pacpend: *mut i32,
            pchange: *mut TS_TEXTCHANGE,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextStoreACP_Impl::InsertEmbeddedAtSelection(
                    this,
                    core::mem::transmute_copy(&dwflags),
                    core::mem::transmute_copy(&pdataobject),
                    core::mem::transmute_copy(&pacpstart),
                    core::mem::transmute_copy(&pacpend),
                    core::mem::transmute_copy(&pchange),
                )
                .into()
            }
        }
        unsafe extern "system" fn RequestSupportedAttrs<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            dwflags: u32,
            cfilterattrs: u32,
            pafilterattrs: *const windows_core::GUID,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextStoreACP_Impl::RequestSupportedAttrs(
                    this,
                    core::mem::transmute_copy(&dwflags),
                    core::mem::transmute_copy(&cfilterattrs),
                    core::mem::transmute_copy(&pafilterattrs),
                )
                .into()
            }
        }
        unsafe extern "system" fn RequestAttrsAtPosition<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            acppos: i32,
            cfilterattrs: u32,
            pafilterattrs: *const windows_core::GUID,
            dwflags: u32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextStoreACP_Impl::RequestAttrsAtPosition(
                    this,
                    core::mem::transmute_copy(&acppos),
                    core::mem::transmute_copy(&cfilterattrs),
                    core::mem::transmute_copy(&pafilterattrs),
                    core::mem::transmute_copy(&dwflags),
                )
                .into()
            }
        }
        unsafe extern "system" fn RequestAttrsTransitioningAtPosition<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            acppos: i32,
            cfilterattrs: u32,
            pafilterattrs: *const windows_core::GUID,
            dwflags: u32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextStoreACP_Impl::RequestAttrsTransitioningAtPosition(
                    this,
                    core::mem::transmute_copy(&acppos),
                    core::mem::transmute_copy(&cfilterattrs),
                    core::mem::transmute_copy(&pafilterattrs),
                    core::mem::transmute_copy(&dwflags),
                )
                .into()
            }
        }
        unsafe extern "system" fn FindNextAttrTransition<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            acpstart: i32,
            acphalt: i32,
            cfilterattrs: u32,
            pafilterattrs: *const windows_core::GUID,
            dwflags: u32,
            pacpnext: *mut i32,
            pffound: *mut windows_core::BOOL,
            plfoundoffset: *mut i32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextStoreACP_Impl::FindNextAttrTransition(
                    this,
                    core::mem::transmute_copy(&acpstart),
                    core::mem::transmute_copy(&acphalt),
                    core::mem::transmute_copy(&cfilterattrs),
                    core::mem::transmute_copy(&pafilterattrs),
                    core::mem::transmute_copy(&dwflags),
                    core::mem::transmute_copy(&pacpnext),
                    core::mem::transmute_copy(&pffound),
                    core::mem::transmute_copy(&plfoundoffset),
                )
                .into()
            }
        }
        unsafe extern "system" fn RetrieveRequestedAttrs<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            ulcount: u32,
            paattrvals: *mut TS_ATTRVAL,
            pcfetched: *mut u32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextStoreACP_Impl::RetrieveRequestedAttrs(
                    this,
                    core::mem::transmute_copy(&ulcount),
                    core::mem::transmute_copy(&paattrvals),
                    core::mem::transmute_copy(&pcfetched),
                )
                .into()
            }
        }
        unsafe extern "system" fn GetEndACP<Identity: ITextStoreACP_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            pacp: *mut i32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextStoreACP_Impl::GetEndACP(this) {
                    Ok(ok__) => {
                        pacp.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetActiveView<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pvcview: *mut u32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextStoreACP_Impl::GetActiveView(this) {
                    Ok(ok__) => {
                        pvcview.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetACPFromPoint<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            vcview: u32,
            ptscreen: *const POINT,
            dwflags: u32,
            pacp: *mut i32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextStoreACP_Impl::GetACPFromPoint(
                    this,
                    core::mem::transmute_copy(&vcview),
                    core::mem::transmute_copy(&ptscreen),
                    core::mem::transmute_copy(&dwflags),
                ) {
                    Ok(ok__) => {
                        pacp.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetTextExt<Identity: ITextStoreACP_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            vcview: u32,
            acpstart: i32,
            acpend: i32,
            prc: *mut RECT,
            pfclipped: *mut windows_core::BOOL,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextStoreACP_Impl::GetTextExt(
                    this,
                    core::mem::transmute_copy(&vcview),
                    core::mem::transmute_copy(&acpstart),
                    core::mem::transmute_copy(&acpend),
                    core::mem::transmute_copy(&prc),
                    core::mem::transmute_copy(&pfclipped),
                )
                .into()
            }
        }
        unsafe extern "system" fn GetScreenExt<
            Identity: ITextStoreACP_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            vcview: u32,
            prc: *mut RECT,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextStoreACP_Impl::GetScreenExt(this, core::mem::transmute_copy(&vcview)) {
                    Ok(ok__) => {
                        prc.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetWnd<Identity: ITextStoreACP_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            vcview: u32,
            phwnd: *mut HWND,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextStoreACP_Impl::GetWnd(this, core::mem::transmute_copy(&vcview)) {
                    Ok(ok__) => {
                        phwnd.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            AdviseSink: AdviseSink::<Identity, OFFSET>,
            UnadviseSink: UnadviseSink::<Identity, OFFSET>,
            RequestLock: RequestLock::<Identity, OFFSET>,
            GetStatus: GetStatus::<Identity, OFFSET>,
            QueryInsert: QueryInsert::<Identity, OFFSET>,
            GetSelection: GetSelection::<Identity, OFFSET>,
            SetSelection: SetSelection::<Identity, OFFSET>,
            GetText: GetText::<Identity, OFFSET>,
            SetText: SetText::<Identity, OFFSET>,
            GetFormattedText: GetFormattedText::<Identity, OFFSET>,
            GetEmbedded: GetEmbedded::<Identity, OFFSET>,
            QueryInsertEmbedded: QueryInsertEmbedded::<Identity, OFFSET>,
            InsertEmbedded: InsertEmbedded::<Identity, OFFSET>,
            InsertTextAtSelection: InsertTextAtSelection::<Identity, OFFSET>,
            InsertEmbeddedAtSelection: InsertEmbeddedAtSelection::<Identity, OFFSET>,
            RequestSupportedAttrs: RequestSupportedAttrs::<Identity, OFFSET>,
            RequestAttrsAtPosition: RequestAttrsAtPosition::<Identity, OFFSET>,
            RequestAttrsTransitioningAtPosition: RequestAttrsTransitioningAtPosition::<
                Identity,
                OFFSET,
            >,
            FindNextAttrTransition: FindNextAttrTransition::<Identity, OFFSET>,
            RetrieveRequestedAttrs: RetrieveRequestedAttrs::<Identity, OFFSET>,
            GetEndACP: GetEndACP::<Identity, OFFSET>,
            GetActiveView: GetActiveView::<Identity, OFFSET>,
            GetACPFromPoint: GetACPFromPoint::<Identity, OFFSET>,
            GetTextExt: GetTextExt::<Identity, OFFSET>,
            GetScreenExt: GetScreenExt::<Identity, OFFSET>,
            GetWnd: GetWnd::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<ITextStoreACP as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for ITextStoreACP {}
windows_core::imp::define_interface!(
    ITextStoreACPSink,
    ITextStoreACPSink_Vtbl,
    0x22d44c94_a419_4542_a272_ae26093ececf
);
windows_core::imp::interface_hierarchy!(ITextStoreACPSink, windows_core::IUnknown);
impl ITextStoreACPSink {
    pub(crate) unsafe fn OnTextChange(
        &self,
        dwflags: TEXT_STORE_TEXT_CHANGE_FLAGS,
        pchange: *const TS_TEXTCHANGE,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).OnTextChange)(
                windows_core::Interface::as_raw(self),
                dwflags,
                pchange,
            )
            .ok()
        }
    }
    pub(crate) unsafe fn OnSelectionChange(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).OnSelectionChange)(
                windows_core::Interface::as_raw(self),
            )
            .ok()
        }
    }
    pub(crate) unsafe fn OnLayoutChange(
        &self,
        lcode: TsLayoutCode,
        vcview: u32,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).OnLayoutChange)(
                windows_core::Interface::as_raw(self),
                lcode,
                vcview,
            )
            .ok()
        }
    }
    pub(crate) unsafe fn OnStatusChange(&self, dwflags: u32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).OnStatusChange)(
                windows_core::Interface::as_raw(self),
                dwflags,
            )
            .ok()
        }
    }
    pub(crate) unsafe fn OnAttrsChange(
        &self,
        acpstart: i32,
        acpend: i32,
        paattrs: &[windows_core::GUID],
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).OnAttrsChange)(
                windows_core::Interface::as_raw(self),
                acpstart,
                acpend,
                paattrs.len().try_into().unwrap(),
                paattrs.as_ptr(),
            )
            .ok()
        }
    }
    pub(crate) unsafe fn OnLockGranted(
        &self,
        dwlockflags: TEXT_STORE_LOCK_FLAGS,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).OnLockGranted)(
                windows_core::Interface::as_raw(self),
                dwlockflags,
            )
            .ok()
        }
    }
    pub(crate) unsafe fn OnStartEditTransaction(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).OnStartEditTransaction)(
                windows_core::Interface::as_raw(self),
            )
            .ok()
        }
    }
    pub(crate) unsafe fn OnEndEditTransaction(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).OnEndEditTransaction)(
                windows_core::Interface::as_raw(self),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ITextStoreACPSink_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub OnTextChange: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TEXT_STORE_TEXT_CHANGE_FLAGS,
        *const TS_TEXTCHANGE,
    ) -> windows_core::HRESULT,
    pub OnSelectionChange:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub OnLayoutChange: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TsLayoutCode,
        u32,
    ) -> windows_core::HRESULT,
    pub OnStatusChange:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    pub OnAttrsChange: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        i32,
        u32,
        *const windows_core::GUID,
    ) -> windows_core::HRESULT,
    pub OnLockGranted: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TEXT_STORE_LOCK_FLAGS,
    ) -> windows_core::HRESULT,
    pub OnStartEditTransaction:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub OnEndEditTransaction:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ITfContext,
    ITfContext_Vtbl,
    0xaa80e7fd_2021_11d2_93e0_0060b067b86e
);
windows_core::imp::interface_hierarchy!(ITfContext, windows_core::IUnknown);
#[repr(C)]
pub struct ITfContext_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    RequestEditSession: usize,
    InWriteSession: usize,
    GetSelection: usize,
    SetSelection: usize,
    GetStart: usize,
    GetEnd: usize,
    GetActiveView: usize,
    EnumViews: usize,
    GetStatus: usize,
    GetProperty: usize,
    GetAppProperty: usize,
    TrackProperties: usize,
    EnumProperties: usize,
    GetDocumentMgr: usize,
    CreateRangeBackup: usize,
}
windows_core::imp::define_interface!(
    ITfDocumentMgr,
    ITfDocumentMgr_Vtbl,
    0xaa80e7f4_2021_11d2_93e0_0060b067b86e
);
windows_core::imp::interface_hierarchy!(ITfDocumentMgr, windows_core::IUnknown);
impl ITfDocumentMgr {
    pub(crate) unsafe fn CreateContext<P2>(
        &self,
        tidowner: u32,
        dwflags: u32,
        punk: P2,
        ppic: *mut Option<ITfContext>,
        pectextstore: *mut u32,
    ) -> windows_core::Result<()>
    where
        P2: windows_core::Param<windows_core::IUnknown>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).CreateContext)(
                windows_core::Interface::as_raw(self),
                tidowner,
                dwflags,
                punk.param().abi(),
                ppic as _,
                pectextstore as _,
            )
            .ok()
        }
    }
    pub(crate) unsafe fn Push<P0>(&self, pic: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<ITfContext>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).Push)(
                windows_core::Interface::as_raw(self),
                pic.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) unsafe fn Pop(&self, dwflags: u32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).Pop)(
                windows_core::Interface::as_raw(self),
                dwflags,
            )
            .ok()
        }
    }
    pub(crate) unsafe fn GetTop(&self) -> windows_core::Result<ITfContext> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetTop)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn GetBase(&self) -> windows_core::Result<ITfContext> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetBase)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ITfDocumentMgr_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub CreateContext: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        u32,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub Push: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub Pop: unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    pub GetTop: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetBase: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    EnumContexts: usize,
}
windows_core::imp::define_interface!(
    ITfKeystrokeMgr,
    ITfKeystrokeMgr_Vtbl,
    0xaa80e7f0_2021_11d2_93e0_0060b067b86e
);
windows_core::imp::interface_hierarchy!(ITfKeystrokeMgr, windows_core::IUnknown);
#[repr(C)]
pub struct ITfKeystrokeMgr_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    AdviseKeyEventSink: usize,
    UnadviseKeyEventSink: usize,
    GetForeground: usize,
    TestKeyDown: usize,
    TestKeyUp: usize,
    KeyDown: usize,
    KeyUp: usize,
    GetPreservedKey: usize,
    IsPreservedKey: usize,
    PreserveKey: usize,
    UnpreserveKey: usize,
    SetPreservedKeyDescription: usize,
    GetPreservedKeyDescription: usize,
    SimulatePreservedKey: usize,
}
windows_core::imp::define_interface!(
    ITfSource,
    ITfSource_Vtbl,
    0x4ea48a35_60ae_446f_8fd6_e6a8d82459f7
);
windows_core::imp::interface_hierarchy!(ITfSource, windows_core::IUnknown);
impl ITfSource {
    pub(crate) unsafe fn AdviseSink<P1>(
        &self,
        riid: *const windows_core::GUID,
        punk: P1,
    ) -> windows_core::Result<u32>
    where
        P1: windows_core::Param<windows_core::IUnknown>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).AdviseSink)(
                windows_core::Interface::as_raw(self),
                riid,
                punk.param().abi(),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn UnadviseSink(&self, dwcookie: u32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).UnadviseSink)(
                windows_core::Interface::as_raw(self),
                dwcookie,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ITfSource_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub AdviseSink: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut core::ffi::c_void,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub UnadviseSink:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ITfThreadMgr,
    ITfThreadMgr_Vtbl,
    0xaa80e801_2021_11d2_93e0_0060b067b86e
);
windows_core::imp::interface_hierarchy!(ITfThreadMgr, windows_core::IUnknown);
impl ITfThreadMgr {
    pub(crate) unsafe fn Activate(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Activate)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn Deactivate(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).Deactivate)(windows_core::Interface::as_raw(
                self,
            ))
            .ok()
        }
    }
    pub(crate) unsafe fn CreateDocumentMgr(&self) -> windows_core::Result<ITfDocumentMgr> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateDocumentMgr)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn GetFocus(&self) -> windows_core::Result<ITfDocumentMgr> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFocus)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn SetFocus<P0>(&self, pdimfocus: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<ITfDocumentMgr>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetFocus)(
                windows_core::Interface::as_raw(self),
                pdimfocus.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ITfThreadMgr_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Activate:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub Deactivate: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub CreateDocumentMgr: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    EnumDocumentMgrs: usize,
    pub GetFocus: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetFocus: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    AssociateFocus: usize,
    IsThreadFocus: usize,
    GetFunctionProvider: usize,
    EnumFunctionProviders: usize,
    GetGlobalCompartment: usize,
}
windows_core::imp::define_interface!(
    ITfThreadMgr2,
    ITfThreadMgr2_Vtbl,
    0x0ab198ef_6477_4ee8_8812_6780edb82d5e
);
windows_core::imp::interface_hierarchy!(ITfThreadMgr2, windows_core::IUnknown);
impl ITfThreadMgr2 {
    pub(crate) unsafe fn Activate(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Activate)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn Deactivate(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).Deactivate)(windows_core::Interface::as_raw(
                self,
            ))
            .ok()
        }
    }
    pub(crate) unsafe fn CreateDocumentMgr(&self) -> windows_core::Result<ITfDocumentMgr> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateDocumentMgr)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn GetFocus(&self) -> windows_core::Result<ITfDocumentMgr> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFocus)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn SetFocus<P0>(&self, pdimfocus: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<ITfDocumentMgr>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetFocus)(
                windows_core::Interface::as_raw(self),
                pdimfocus.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ITfThreadMgr2_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Activate:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub Deactivate: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub CreateDocumentMgr: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    EnumDocumentMgrs: usize,
    pub GetFocus: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetFocus: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    IsThreadFocus: usize,
    GetFunctionProvider: usize,
    EnumFunctionProviders: usize,
    GetGlobalCompartment: usize,
    ActivateEx: usize,
    GetActiveFlags: usize,
    SuspendKeystrokeHandling: usize,
    ResumeKeystrokeHandling: usize,
}
windows_core::imp::define_interface!(
    IToggleProvider,
    IToggleProvider_Vtbl,
    0x56d00bd0_c4f4_433c_a836_1a52a57e0892
);
windows_core::imp::interface_hierarchy!(IToggleProvider, windows_core::IUnknown);
#[repr(C)]
pub struct IToggleProvider_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Toggle: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub ToggleState: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut ToggleState,
    ) -> windows_core::HRESULT,
}
pub trait IToggleProvider_Impl: windows_core::IUnknownImpl {
    fn Toggle(&self) -> windows_core::Result<()>;
    fn ToggleState(&self) -> windows_core::Result<ToggleState>;
}
impl IToggleProvider_Vtbl {
    pub const fn new<Identity: IToggleProvider_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn Toggle<Identity: IToggleProvider_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IToggleProvider_Impl::Toggle(this).into()
            }
        }
        unsafe extern "system" fn ToggleState<
            Identity: IToggleProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut ToggleState,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IToggleProvider_Impl::ToggleState(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            Toggle: Toggle::<Identity, OFFSET>,
            ToggleState: ToggleState::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IToggleProvider as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IToggleProvider {}
windows_core::imp::define_interface!(
    IValueProvider,
    IValueProvider_Vtbl,
    0xc7935180_6fb3_4201_b174_7df73adbf64a
);
windows_core::imp::interface_hierarchy!(IValueProvider, windows_core::IUnknown);
#[repr(C)]
pub struct IValueProvider_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub SetValue: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::PCWSTR,
    ) -> windows_core::HRESULT,
    pub Value: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub IsReadOnly: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
}
pub trait IValueProvider_Impl: windows_core::IUnknownImpl {
    fn SetValue(&self, val: &windows_core::PCWSTR) -> windows_core::Result<()>;
    fn Value(&self) -> windows_core::Result<windows_core::BSTR>;
    fn IsReadOnly(&self) -> windows_core::Result<windows_core::BOOL>;
}
impl IValueProvider_Vtbl {
    pub const fn new<Identity: IValueProvider_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn SetValue<Identity: IValueProvider_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            val: windows_core::PCWSTR,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IValueProvider_Impl::SetValue(this, core::mem::transmute(&val)).into()
            }
        }
        unsafe extern "system" fn Value<Identity: IValueProvider_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IValueProvider_Impl::Value(this) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn IsReadOnly<Identity: IValueProvider_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            pretval: *mut windows_core::BOOL,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IValueProvider_Impl::IsReadOnly(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            SetValue: SetValue::<Identity, OFFSET>,
            Value: Value::<Identity, OFFSET>,
            IsReadOnly: IsReadOnly::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IValueProvider as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IValueProvider {}
windows_core::imp::define_interface!(
    IVector2KeyFrameAnimation,
    IVector2KeyFrameAnimation_Vtbl,
    0xdf414515_4e29_4f11_b55e_bf2a6eb36294
);
impl windows_core::RuntimeType for IVector2KeyFrameAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IVector2KeyFrameAnimation {
    pub(crate) fn InsertKeyFrame(
        &self,
        normalizedprogresskey: f32,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).InsertKeyFrame)(
                windows_core::Interface::as_raw(self),
                normalizedprogresskey,
                value,
            )
            .ok()
        }
    }
    pub(crate) fn InsertKeyFrameWithEasingFunction<P2>(
        &self,
        normalizedprogresskey: f32,
        value: windows_numerics::Vector2,
        easingfunction: P2,
    ) -> windows_core::Result<()>
    where
        P2: windows_core::Param<CompositionEasingFunction>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InsertKeyFrameWithEasingFunction)(
                windows_core::Interface::as_raw(self),
                normalizedprogresskey,
                value,
                easingfunction.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IVector2KeyFrameAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub InsertKeyFrame: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    pub InsertKeyFrameWithEasingFunction: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        windows_numerics::Vector2,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IVector3KeyFrameAnimation,
    IVector3KeyFrameAnimation_Vtbl,
    0xc8039daa_a281_43c2_a73d_b68e3c533c40
);
impl windows_core::RuntimeType for IVector3KeyFrameAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IVector3KeyFrameAnimation {
    pub(crate) fn InsertKeyFrame(
        &self,
        normalizedprogresskey: f32,
        value: windows_numerics::Vector3,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).InsertKeyFrame)(
                windows_core::Interface::as_raw(self),
                normalizedprogresskey,
                value,
            )
            .ok()
        }
    }
    pub(crate) fn InsertKeyFrameWithEasingFunction<P2>(
        &self,
        normalizedprogresskey: f32,
        value: windows_numerics::Vector3,
        easingfunction: P2,
    ) -> windows_core::Result<()>
    where
        P2: windows_core::Param<CompositionEasingFunction>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InsertKeyFrameWithEasingFunction)(
                windows_core::Interface::as_raw(self),
                normalizedprogresskey,
                value,
                easingfunction.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IVector3KeyFrameAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub InsertKeyFrame: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    pub InsertKeyFrameWithEasingFunction: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        windows_numerics::Vector3,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IVisual,
    IVisual_Vtbl,
    0x117e202d_a859_4c89_873b_c2aa566788e3
);
impl windows_core::RuntimeType for IVisual {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IVisual {
    pub(crate) fn SetAnchorPoint(
        &self,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetAnchorPoint)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetCenterPoint(
        &self,
        value: windows_numerics::Vector3,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetCenterPoint)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetClip<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<CompositionClip>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetClip)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn SetIsVisible(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsVisible)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn Offset(&self) -> windows_core::Result<windows_numerics::Vector3> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Offset)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn SetOffset(&self, value: windows_numerics::Vector3) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetOffset)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetOpacity(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetOpacity)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetRotationAngle(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetRotationAngle)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetScale(&self, value: windows_numerics::Vector3) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetScale)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn Size(&self) -> windows_core::Result<windows_numerics::Vector2> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Size)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn SetSize(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSize)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IVisual_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    AnchorPoint: usize,
    pub SetAnchorPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    BackfaceVisibility: usize,
    SetBackfaceVisibility: usize,
    BorderMode: usize,
    SetBorderMode: usize,
    CenterPoint: usize,
    pub SetCenterPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    Clip: usize,
    pub SetClip: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CompositeMode: usize,
    SetCompositeMode: usize,
    IsVisible: usize,
    pub SetIsVisible:
        unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    pub Offset: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    pub SetOffset: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    Opacity: usize,
    pub SetOpacity: unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    Orientation: usize,
    SetOrientation: usize,
    Parent: usize,
    RotationAngle: usize,
    pub SetRotationAngle:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    RotationAngleInDegrees: usize,
    SetRotationAngleInDegrees: usize,
    RotationAxis: usize,
    SetRotationAxis: usize,
    Scale: usize,
    pub SetScale: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    pub Size: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    pub SetSize: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IVisualCollection,
    IVisualCollection_Vtbl,
    0x8b745505_fd3e_4a98_84a8_e949468c6bcb
);
impl windows_core::RuntimeType for IVisualCollection {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IVisualCollection {
    pub(crate) fn Count(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Count)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn InsertAbove<P0, P1>(&self, newchild: P0, sibling: P1) -> windows_core::Result<()>
    where
        P0: windows_core::Param<Visual>,
        P1: windows_core::Param<Visual>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InsertAbove)(
                windows_core::Interface::as_raw(self),
                newchild.param().abi(),
                sibling.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn InsertAtBottom<P0>(&self, newchild: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<Visual>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InsertAtBottom)(
                windows_core::Interface::as_raw(self),
                newchild.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn InsertAtTop<P0>(&self, newchild: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<Visual>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InsertAtTop)(
                windows_core::Interface::as_raw(self),
                newchild.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn InsertBelow<P0, P1>(&self, newchild: P0, sibling: P1) -> windows_core::Result<()>
    where
        P0: windows_core::Param<Visual>,
        P1: windows_core::Param<Visual>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InsertBelow)(
                windows_core::Interface::as_raw(self),
                newchild.param().abi(),
                sibling.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn Remove<P0>(&self, child: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<Visual>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).Remove)(
                windows_core::Interface::as_raw(self),
                child.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn RemoveAll(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).RemoveAll)(windows_core::Interface::as_raw(self))
                .ok()
        }
    }
}
#[repr(C)]
pub struct IVisualCollection_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Count: unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub InsertAbove: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub InsertAtBottom: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub InsertAtTop: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub InsertBelow: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub Remove: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub RemoveAll: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsetClip(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InsetClip,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(InsetClip, CompositionClip, CompositionObject);
impl windows_core::RuntimeType for InsetClip {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInsetClip>();
}
unsafe impl windows_core::Interface for InsetClip {
    type Vtable = <IInsetClip as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IInsetClip as windows_core::Interface>::IID;
}
impl core::ops::Deref for InsetClip {
    type Target = IInsetClip;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InsetClip {
    const NAME: &'static str = "Windows.UI.Composition.InsetClip";
}
unsafe impl Send for InsetClip {}
unsafe impl Sync for InsetClip {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyFrameAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    KeyFrameAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(KeyFrameAnimation, CompositionAnimation, CompositionObject);
impl windows_core::RuntimeType for KeyFrameAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IKeyFrameAnimation>();
}
unsafe impl windows_core::Interface for KeyFrameAnimation {
    type Vtable = <IKeyFrameAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IKeyFrameAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for KeyFrameAnimation {
    type Target = IKeyFrameAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for KeyFrameAnimation {
    const NAME: &'static str = "Windows.UI.Composition.KeyFrameAnimation";
}
unsafe impl Send for KeyFrameAnimation {}
unsafe impl Sync for KeyFrameAnimation {}
pub type LPARAM = isize;
pub type LRESULT = isize;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinearEasingFunction(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    LinearEasingFunction,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    LinearEasingFunction,
    CompositionEasingFunction,
    CompositionObject
);
impl windows_core::RuntimeType for LinearEasingFunction {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ILinearEasingFunction>();
}
unsafe impl windows_core::Interface for LinearEasingFunction {
    type Vtable = <ILinearEasingFunction as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ILinearEasingFunction as windows_core::Interface>::IID;
}
impl core::ops::Deref for LinearEasingFunction {
    type Target = ILinearEasingFunction;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for LinearEasingFunction {
    const NAME: &'static str = "Windows.UI.Composition.LinearEasingFunction";
}
unsafe impl Send for LinearEasingFunction {}
unsafe impl Sync for LinearEasingFunction {}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MINMAXINFO {
    pub ptReserved: POINT,
    pub ptMaxSize: POINT,
    pub ptMaxPosition: POINT,
    pub ptMinTrackSize: POINT,
    pub ptMaxTrackSize: POINT,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MONITORINFO {
    pub cbSize: u32,
    pub rcMonitor: RECT,
    pub rcWork: RECT,
    pub dwFlags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MONITORINFOEXW {
    pub monitorInfo: MONITORINFO,
    pub szDevice: [u16; 32],
}
impl Default for MONITORINFOEXW {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub const MONITOR_DEFAULTTONEAREST: MONITOR_FROM_FLAGS = 2;
pub const MONITOR_DEFAULTTOPRIMARY: MONITOR_FROM_FLAGS = 1;
pub type MONITOR_FROM_FLAGS = u32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: u32,
    pub pt: POINT,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NCCALCSIZE_PARAMS {
    pub rgrc: [RECT; 3],
    pub lppos: *mut WINDOWPOS,
}
impl Default for NCCALCSIZE_PARAMS {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub type NOTIFY_IME_ACTION = u32;
pub type NOTIFY_IME_INDEX = u32;
pub type NavigateDirection = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: windows_core::BOOL,
    pub rcPaint: RECT,
    pub fRestore: windows_core::BOOL,
    pub fIncUpdate: windows_core::BOOL,
    pub rgbReserved: [u8; 32],
}
impl Default for PAINTSTRUCT {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub type PEEK_MESSAGE_REMOVE_TYPE = u32;
pub const PM_REMOVE: PEEK_MESSAGE_REMOVE_TYPE = 1;
windows_core::imp::define_interface!(
    IPropertyValueStatics,
    IPropertyValueStatics_Vtbl,
    0x629bdbc8_d932_4ff4_96b9_8d96c5c1e858
);
impl windows_core::RuntimeType for IPropertyValueStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IPropertyValueStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    CreateEmpty: usize,
    CreateUInt8: usize,
    CreateInt16: usize,
    CreateUInt16: usize,
    CreateInt32: usize,
    CreateUInt32: usize,
    CreateInt64: usize,
    CreateUInt64: usize,
    pub CreateSingle: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateDouble: usize,
    CreateChar16: usize,
    CreateBoolean: usize,
    CreateString: usize,
    CreateInspectable: usize,
    CreateGuid: usize,
    CreateDateTime: usize,
    CreateTimeSpan: usize,
    CreatePoint: usize,
    CreateSize: usize,
    CreateRect: usize,
    CreateUInt8Array: usize,
    CreateInt16Array: usize,
    CreateUInt16Array: usize,
    CreateInt32Array: usize,
    CreateUInt32Array: usize,
    CreateInt64Array: usize,
    CreateUInt64Array: usize,
    CreateSingleArray: usize,
    CreateDoubleArray: usize,
    CreateChar16Array: usize,
    CreateBooleanArray: usize,
    CreateStringArray: usize,
    CreateInspectableArray: usize,
    CreateGuidArray: usize,
    CreateDateTimeArray: usize,
    CreateTimeSpanArray: usize,
    CreatePointArray: usize,
    CreateSizeArray: usize,
    CreateRectArray: usize,
}
pub struct PropertyValue;
impl PropertyValue {
    pub(crate) fn CreateSingle(value: f32) -> windows_core::Result<windows_core::IInspectable> {
        Self::IPropertyValueStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateSingle)(
                windows_core::Interface::as_raw(this),
                value,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    fn IPropertyValueStatics<R, F: FnOnce(&IPropertyValueStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<PropertyValue, IPropertyValueStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeName for PropertyValue {
    const NAME: &'static str = "Windows.Foundation.PropertyValue";
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}
pub type POINTER_BUTTON_CHANGE_TYPE = i32;
pub type POINTER_FLAGS = u32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct POINTER_INFO {
    pub pointerType: POINTER_INPUT_TYPE,
    pub pointerId: u32,
    pub frameId: u32,
    pub pointerFlags: POINTER_FLAGS,
    pub sourceDevice: HANDLE,
    pub hwndTarget: HWND,
    pub ptPixelLocation: POINT,
    pub ptHimetricLocation: POINT,
    pub ptPixelLocationRaw: POINT,
    pub ptHimetricLocationRaw: POINT,
    pub dwTime: u32,
    pub historyCount: u32,
    pub InputData: i32,
    pub dwKeyStates: u32,
    pub PerformanceCount: u64,
    pub ButtonChangeType: POINTER_BUTTON_CHANGE_TYPE,
}
pub type POINTER_INPUT_TYPE = i32;
pub type ProviderOptions = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SAFEARRAY {
    pub cDims: u16,
    pub fFeatures: ADVANCED_FEATURE_FLAGS,
    pub cbElements: u32,
    pub cLocks: u32,
    pub pvData: *mut core::ffi::c_void,
    pub rgsabound: [SAFEARRAYBOUND; 1],
}
impl Default for SAFEARRAY {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SAFEARRAYBOUND {
    pub cElements: u32,
    pub lLbound: i32,
}
pub type SET_WINDOW_POS_FLAGS = u32;
pub type SHOW_WINDOW_CMD = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SIZE {
    pub cx: i32,
    pub cy: i32,
}
pub const SWP_FRAMECHANGED: SET_WINDOW_POS_FLAGS = 32;
pub const SWP_NOACTIVATE: SET_WINDOW_POS_FLAGS = 16;
pub const SWP_NOMOVE: SET_WINDOW_POS_FLAGS = 2;
pub const SWP_NOSIZE: SET_WINDOW_POS_FLAGS = 1;
pub const SWP_NOZORDER: SET_WINDOW_POS_FLAGS = 4;
pub const SWP_SHOWWINDOW: SET_WINDOW_POS_FLAGS = 64;
pub const SW_HIDE: SHOW_WINDOW_CMD = 0;
pub const SW_MAXIMIZE: SHOW_WINDOW_CMD = 3;
pub const SW_MINIMIZE: SHOW_WINDOW_CMD = 6;
pub const SW_RESTORE: SHOW_WINDOW_CMD = 9;
pub const SW_SHOW: SHOW_WINDOW_CMD = 5;
pub const SW_SHOWDEFAULT: SHOW_WINDOW_CMD = 10;
pub const SW_SHOWNORMAL: SHOW_WINDOW_CMD = 1;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarKeyFrameAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ScalarKeyFrameAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    ScalarKeyFrameAnimation,
    KeyFrameAnimation,
    CompositionAnimation,
    CompositionObject
);
impl windows_core::RuntimeType for ScalarKeyFrameAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IScalarKeyFrameAnimation>();
}
unsafe impl windows_core::Interface for ScalarKeyFrameAnimation {
    type Vtable = <IScalarKeyFrameAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IScalarKeyFrameAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for ScalarKeyFrameAnimation {
    type Target = IScalarKeyFrameAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ScalarKeyFrameAnimation {
    const NAME: &'static str = "Windows.UI.Composition.ScalarKeyFrameAnimation";
}
unsafe impl Send for ScalarKeyFrameAnimation {}
unsafe impl Sync for ScalarKeyFrameAnimation {}
pub type ScrollAmount = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}
impl windows_core::TypeKind for Size {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for Size {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.Foundation.Size;f4;f4)");
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SizeInt32 {
    pub width: i32,
    pub height: i32,
}
impl windows_core::TypeKind for SizeInt32 {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for SizeInt32 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.Graphics.SizeInt32;i4;i4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpriteVisual(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    SpriteVisual,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(SpriteVisual, ContainerVisual, Visual, CompositionObject);
impl windows_core::RuntimeType for SpriteVisual {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ISpriteVisual>();
}
unsafe impl windows_core::Interface for SpriteVisual {
    type Vtable = <ISpriteVisual as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ISpriteVisual as windows_core::Interface>::IID;
}
impl core::ops::Deref for SpriteVisual {
    type Target = ISpriteVisual;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for SpriteVisual {
    const NAME: &'static str = "Windows.UI.Composition.SpriteVisual";
}
unsafe impl Send for SpriteVisual {}
unsafe impl Sync for SpriteVisual {}
pub type SupportedTextSelection = i32;
pub type TEXT_STORE_LOCK_FLAGS = u32;
pub type TEXT_STORE_TEXT_CHANGE_FLAGS = u32;
pub type TF_CONTEXT_EDIT_CONTEXT_FLAGS = u32;
pub const TF_ES_ASYNCDONTCARE: TF_CONTEXT_EDIT_CONTEXT_FLAGS = 0;
pub const TF_ES_READ: TF_CONTEXT_EDIT_CONTEXT_FLAGS = 2;
pub const TF_ES_READWRITE: TF_CONTEXT_EDIT_CONTEXT_FLAGS = 6;
pub const TF_ES_SYNC: TF_CONTEXT_EDIT_CONTEXT_FLAGS = 1;
pub type TIMERPROC =
    Option<unsafe extern "system" fn(param0: HWND, param1: u32, param2: usize, param3: u32)>;
pub const TME_LEAVE: TRACKMOUSEEVENT_FLAGS = 2;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TRACKMOUSEEVENT {
    pub cbSize: u32,
    pub dwFlags: TRACKMOUSEEVENT_FLAGS,
    pub hwndTrack: HWND,
    pub dwHoverTime: u32,
}
pub type TRACKMOUSEEVENT_FLAGS = u32;
#[repr(C)]
pub struct TS_ATTRVAL {
    pub idAttr: windows_core::GUID,
    pub dwOverlapId: u32,
    pub varValue: VARIANT,
}
impl Clone for TS_ATTRVAL {
    fn clone(&self) -> Self {
        unsafe { core::mem::transmute_copy(self) }
    }
}
impl Default for TS_ATTRVAL {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TS_RUNINFO {
    pub uCount: u32,
    pub r#type: TsRunType,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TS_SELECTIONSTYLE {
    pub ase: TsActiveSelEnd,
    pub fInterimChar: windows_core::BOOL,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TS_SELECTION_ACP {
    pub acpStart: i32,
    pub acpEnd: i32,
    pub style: TS_SELECTIONSTYLE,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TS_STATUS {
    pub dwDynamicFlags: u32,
    pub dwStaticFlags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TS_TEXTCHANGE {
    pub acpStart: i32,
    pub acpOldEnd: i32,
    pub acpNewEnd: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimeSpan {
    pub duration: i64,
}
impl windows_core::TypeKind for TimeSpan {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for TimeSpan {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.Foundation.TimeSpan;i8)");
}
pub type ToggleState = i32;
pub type TsActiveSelEnd = i32;
pub type TsLayoutCode = i32;
pub type TsRunType = i32;
pub const UIA_AutomationFocusChangedEventId: UIA_EVENT_ID = 20005;
pub const UIA_AutomationIdPropertyId: UIA_PROPERTY_ID = 30011;
pub const UIA_AutomationPropertyChangedEventId: UIA_EVENT_ID = 20004;
pub const UIA_BoundingRectanglePropertyId: UIA_PROPERTY_ID = 30001;
pub const UIA_ButtonControlTypeId: UIA_CONTROLTYPE_ID = 50000;
pub type UIA_CONTROLTYPE_ID = i32;
pub const UIA_CheckBoxControlTypeId: UIA_CONTROLTYPE_ID = 50002;
pub const UIA_ComboBoxControlTypeId: UIA_CONTROLTYPE_ID = 50003;
pub const UIA_ControlTypePropertyId: UIA_PROPERTY_ID = 30003;
pub const UIA_CustomControlTypeId: UIA_CONTROLTYPE_ID = 50025;
pub type UIA_EVENT_ID = i32;
pub const UIA_EditControlTypeId: UIA_CONTROLTYPE_ID = 50004;
pub const UIA_ExpandCollapsePatternId: UIA_PATTERN_ID = 10005;
pub const UIA_GroupControlTypeId: UIA_CONTROLTYPE_ID = 50026;
pub const UIA_HasKeyboardFocusPropertyId: UIA_PROPERTY_ID = 30008;
pub const UIA_HelpTextPropertyId: UIA_PROPERTY_ID = 30013;
pub const UIA_ImageControlTypeId: UIA_CONTROLTYPE_ID = 50006;
pub const UIA_InvokePatternId: UIA_PATTERN_ID = 10000;
pub const UIA_Invoke_InvokedEventId: UIA_EVENT_ID = 20009;
pub const UIA_IsContentElementPropertyId: UIA_PROPERTY_ID = 30017;
pub const UIA_IsControlElementPropertyId: UIA_PROPERTY_ID = 30016;
pub const UIA_IsEnabledPropertyId: UIA_PROPERTY_ID = 30010;
pub const UIA_IsKeyboardFocusablePropertyId: UIA_PROPERTY_ID = 30009;
pub const UIA_ListControlTypeId: UIA_CONTROLTYPE_ID = 50008;
pub const UIA_ListItemControlTypeId: UIA_CONTROLTYPE_ID = 50007;
pub const UIA_LiveRegionChangedEventId: UIA_EVENT_ID = 20024;
pub const UIA_MenuControlTypeId: UIA_CONTROLTYPE_ID = 50009;
pub const UIA_MenuItemControlTypeId: UIA_CONTROLTYPE_ID = 50011;
pub const UIA_NamePropertyId: UIA_PROPERTY_ID = 30005;
pub type UIA_PATTERN_ID = i32;
pub type UIA_PROPERTY_ID = i32;
pub const UIA_PaneControlTypeId: UIA_CONTROLTYPE_ID = 50033;
pub const UIA_RangeValuePatternId: UIA_PATTERN_ID = 10003;
pub const UIA_RangeValueValuePropertyId: UIA_PROPERTY_ID = 30047;
pub const UIA_RuntimeIdPropertyId: UIA_PROPERTY_ID = 30000;
pub const UIA_ScrollBarControlTypeId: UIA_CONTROLTYPE_ID = 50014;
pub const UIA_ScrollPatternId: UIA_PATTERN_ID = 10004;
pub const UIA_SelectionItemPatternId: UIA_PATTERN_ID = 10010;
pub const UIA_SelectionItem_ElementSelectedEventId: UIA_EVENT_ID = 20012;
pub const UIA_SelectionPatternId: UIA_PATTERN_ID = 10001;
pub const UIA_SliderControlTypeId: UIA_CONTROLTYPE_ID = 50015;
pub const UIA_StructureChangedEventId: UIA_EVENT_ID = 20002;
pub const UIA_TabControlTypeId: UIA_CONTROLTYPE_ID = 50018;
pub const UIA_TabItemControlTypeId: UIA_CONTROLTYPE_ID = 50019;
pub const UIA_TextControlTypeId: UIA_CONTROLTYPE_ID = 50020;
pub const UIA_TogglePatternId: UIA_PATTERN_ID = 10015;
pub const UIA_ToggleToggleStatePropertyId: UIA_PROPERTY_ID = 30086;
pub const UIA_ValuePatternId: UIA_PATTERN_ID = 10002;
pub const UIA_ValueValuePropertyId: UIA_PROPERTY_ID = 30045;
pub const UIA_WindowControlTypeId: UIA_CONTROLTYPE_ID = 50032;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiaRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}
pub type VARENUM = u16;
#[repr(C)]
pub struct VARIANT {
    pub Anonymous: VARIANT_0,
}
impl Default for VARIANT {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
pub union VARIANT_0 {
    pub Anonymous: core::mem::ManuallyDrop<VARIANT_0_0>,
    pub decVal: DECIMAL,
}
impl Clone for VARIANT_0 {
    fn clone(&self) -> Self {
        unsafe { core::mem::transmute_copy(self) }
    }
}
impl Default for VARIANT_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
pub struct VARIANT_0_0 {
    pub vt: VARENUM,
    pub wReserved1: u16,
    pub wReserved2: u16,
    pub wReserved3: u16,
    pub Anonymous: VARIANT_0_0_0,
}
impl Clone for VARIANT_0_0 {
    fn clone(&self) -> Self {
        unsafe { core::mem::transmute_copy(self) }
    }
}
impl Default for VARIANT_0_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
pub union VARIANT_0_0_0 {
    pub llVal: i64,
    pub lVal: i32,
    pub bVal: u8,
    pub iVal: i16,
    pub fltVal: f32,
    pub dblVal: f64,
    pub boolVal: VARIANT_BOOL,
    pub __OBSOLETE__VARIANT_BOOL: VARIANT_BOOL,
    pub scode: i32,
    pub cyVal: CY,
    pub date: f64,
    pub bstrVal: core::mem::ManuallyDrop<windows_core::BSTR>,
    pub punkVal: core::mem::ManuallyDrop<Option<windows_core::IUnknown>>,
    pub pdispVal: core::mem::ManuallyDrop<Option<IDispatch>>,
    pub parray: *mut SAFEARRAY,
    pub pbVal: *mut u8,
    pub piVal: *mut i16,
    pub plVal: *mut i32,
    pub pllVal: *mut i64,
    pub pfltVal: *mut f32,
    pub pdblVal: *mut f64,
    pub pboolVal: *mut VARIANT_BOOL,
    pub __OBSOLETE__VARIANT_PBOOL: *mut VARIANT_BOOL,
    pub pscode: *mut i32,
    pub pcyVal: *mut CY,
    pub pdate: *mut f64,
    pub pbstrVal: *mut windows_core::BSTR,
    pub ppunkVal: *mut Option<windows_core::IUnknown>,
    pub ppdispVal: *mut Option<IDispatch>,
    pub pparray: *mut *mut SAFEARRAY,
    pub pvarVal: *mut VARIANT,
    pub byref: *mut core::ffi::c_void,
    pub cVal: i8,
    pub uiVal: u16,
    pub ulVal: u32,
    pub ullVal: u64,
    pub intVal: i32,
    pub uintVal: u32,
    pub pdecVal: *mut DECIMAL,
    pub pcVal: windows_core::PSTR,
    pub puiVal: *mut u16,
    pub pulVal: *mut u32,
    pub pullVal: *mut u64,
    pub pintVal: *mut i32,
    pub puintVal: *mut u32,
    pub Anonymous: core::mem::ManuallyDrop<VARIANT_0_0_0_0>,
}
impl Clone for VARIANT_0_0_0 {
    fn clone(&self) -> Self {
        unsafe { core::mem::transmute_copy(self) }
    }
}
impl Default for VARIANT_0_0_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct VARIANT_0_0_0_0 {
    pub pvRecord: *mut core::ffi::c_void,
    pub pRecInfo: core::mem::ManuallyDrop<Option<IRecordInfo>>,
}
impl Default for VARIANT_0_0_0_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub type VARIANT_BOOL = i16;
pub type VIRTUAL_KEY = u16;
pub const VK_BACK: VIRTUAL_KEY = 8;
pub const VK_CONTROL: VIRTUAL_KEY = 17;
pub const VK_DELETE: VIRTUAL_KEY = 46;
pub const VK_DOWN: VIRTUAL_KEY = 40;
pub const VK_END: VIRTUAL_KEY = 35;
pub const VK_ESCAPE: VIRTUAL_KEY = 27;
pub const VK_HOME: VIRTUAL_KEY = 36;
pub const VK_LEFT: VIRTUAL_KEY = 37;
pub const VK_MENU: VIRTUAL_KEY = 18;
pub const VK_NEXT: VIRTUAL_KEY = 34;
pub const VK_PRIOR: VIRTUAL_KEY = 33;
pub const VK_RETURN: VIRTUAL_KEY = 13;
pub const VK_RIGHT: VIRTUAL_KEY = 39;
pub const VK_SHIFT: VIRTUAL_KEY = 16;
pub const VK_SPACE: VIRTUAL_KEY = 32;
pub const VK_TAB: VIRTUAL_KEY = 9;
pub const VK_UP: VIRTUAL_KEY = 38;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vector2KeyFrameAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    Vector2KeyFrameAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    Vector2KeyFrameAnimation,
    KeyFrameAnimation,
    CompositionAnimation,
    CompositionObject
);
impl windows_core::RuntimeType for Vector2KeyFrameAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IVector2KeyFrameAnimation>();
}
unsafe impl windows_core::Interface for Vector2KeyFrameAnimation {
    type Vtable = <IVector2KeyFrameAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IVector2KeyFrameAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for Vector2KeyFrameAnimation {
    type Target = IVector2KeyFrameAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Vector2KeyFrameAnimation {
    const NAME: &'static str = "Windows.UI.Composition.Vector2KeyFrameAnimation";
}
unsafe impl Send for Vector2KeyFrameAnimation {}
unsafe impl Sync for Vector2KeyFrameAnimation {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vector3KeyFrameAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    Vector3KeyFrameAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    Vector3KeyFrameAnimation,
    KeyFrameAnimation,
    CompositionAnimation,
    CompositionObject
);
impl windows_core::RuntimeType for Vector3KeyFrameAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IVector3KeyFrameAnimation>();
}
unsafe impl windows_core::Interface for Vector3KeyFrameAnimation {
    type Vtable = <IVector3KeyFrameAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IVector3KeyFrameAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for Vector3KeyFrameAnimation {
    type Target = IVector3KeyFrameAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Vector3KeyFrameAnimation {
    const NAME: &'static str = "Windows.UI.Composition.Vector3KeyFrameAnimation";
}
unsafe impl Send for Vector3KeyFrameAnimation {}
unsafe impl Sync for Vector3KeyFrameAnimation {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Visual(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Visual, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(Visual, CompositionObject);
impl windows_core::RuntimeType for Visual {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IVisual>();
}
unsafe impl windows_core::Interface for Visual {
    type Vtable = <IVisual as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IVisual as windows_core::Interface>::IID;
}
impl core::ops::Deref for Visual {
    type Target = IVisual;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Visual {
    const NAME: &'static str = "Windows.UI.Composition.Visual";
}
unsafe impl Send for Visual {}
unsafe impl Sync for Visual {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualCollection(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    VisualCollection,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(VisualCollection, CompositionObject);
impl windows_core::RuntimeType for VisualCollection {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IVisualCollection>();
}
unsafe impl windows_core::Interface for VisualCollection {
    type Vtable = <IVisualCollection as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IVisualCollection as windows_core::Interface>::IID;
}
impl core::ops::Deref for VisualCollection {
    type Target = IVisualCollection;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for VisualCollection {
    const NAME: &'static str = "Windows.UI.Composition.VisualCollection";
}
unsafe impl Send for VisualCollection {}
unsafe impl Sync for VisualCollection {}
pub const WHEEL_DELTA: u32 = 120;
pub type WIN32_ERROR = u32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WINDOWPOS {
    pub hwnd: HWND,
    pub hwndInsertAfter: HWND,
    pub x: i32,
    pub y: i32,
    pub cx: i32,
    pub cy: i32,
    pub flags: SET_WINDOW_POS_FLAGS,
}
pub type WINDOW_EX_STYLE = u32;
pub type WINDOW_LONG_PTR_INDEX = i32;
pub type WINDOW_STYLE = u32;
pub const WM_ACTIVATE: u32 = 6;
pub const WM_APP: u32 = 32768;
pub const WM_CAPTURECHANGED: u32 = 533;
pub const WM_CHAR: u32 = 258;
pub const WM_CLOSE: u32 = 16;
pub const WM_CREATE: u32 = 1;
pub const WM_DESTROY: u32 = 2;
pub const WM_DISPLAYCHANGE: u32 = 126;
pub const WM_DPICHANGED: u32 = 736;
pub const WM_ERASEBKGND: u32 = 20;
pub const WM_GETMINMAXINFO: u32 = 36;
pub const WM_GETOBJECT: u32 = 61;
pub const WM_IME_CHAR: u32 = 646;
pub const WM_IME_COMPOSITION: u32 = 271;
pub const WM_IME_ENDCOMPOSITION: u32 = 270;
pub const WM_IME_NOTIFY: u32 = 642;
pub const WM_IME_SETCONTEXT: u32 = 641;
pub const WM_IME_STARTCOMPOSITION: u32 = 269;
pub const WM_KEYDOWN: u32 = 256;
pub const WM_KEYUP: u32 = 257;
pub const WM_KILLFOCUS: u32 = 8;
pub const WM_LBUTTONDBLCLK: u32 = 515;
pub const WM_LBUTTONDOWN: u32 = 513;
pub const WM_LBUTTONUP: u32 = 514;
pub const WM_MBUTTONDOWN: u32 = 519;
pub const WM_MBUTTONUP: u32 = 520;
pub const WM_MOUSEHWHEEL: u32 = 526;
pub const WM_MOUSELEAVE: u32 = 675;
pub const WM_MOUSEMOVE: u32 = 512;
pub const WM_MOUSEWHEEL: u32 = 522;
pub const WM_NCCALCSIZE: u32 = 131;
pub const WM_NCDESTROY: u32 = 130;
pub const WM_NCHITTEST: u32 = 132;
pub const WM_PAINT: u32 = 15;
pub const WM_POINTERDOWN: u32 = 582;
pub const WM_POINTERENTER: u32 = 585;
pub const WM_POINTERHWHEEL: u32 = 591;
pub const WM_POINTERLEAVE: u32 = 586;
pub const WM_POINTERUP: u32 = 583;
pub const WM_POINTERUPDATE: u32 = 581;
pub const WM_POINTERWHEEL: u32 = 590;
pub const WM_QUIT: u32 = 18;
pub const WM_RBUTTONDOWN: u32 = 516;
pub const WM_RBUTTONUP: u32 = 517;
pub const WM_SETCURSOR: u32 = 32;
pub const WM_SETFOCUS: u32 = 7;
pub const WM_SETTINGCHANGE: u32 = 26;
pub const WM_SIZE: u32 = 5;
pub const WM_SYSKEYDOWN: u32 = 260;
pub const WM_SYSKEYUP: u32 = 261;
pub const WM_TIMER: u32 = 275;
pub const WM_WINDOWPOSCHANGED: u32 = 71;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: WNDCLASS_STYLES,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: windows_core::PCWSTR,
    pub lpszClassName: windows_core::PCWSTR,
    pub hIconSm: HICON,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WNDCLASSW {
    pub style: WNDCLASS_STYLES,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: windows_core::PCWSTR,
    pub lpszClassName: windows_core::PCWSTR,
}
pub type WNDCLASS_STYLES = u32;
pub type WNDPROC = Option<
    unsafe extern "system" fn(param0: HWND, param1: u32, param2: WPARAM, param3: LPARAM) -> LRESULT,
>;
pub type WPARAM = usize;
pub const WS_CAPTION: WINDOW_STYLE = 12582912;
pub const WS_CHILD: WINDOW_STYLE = 1073741824;
pub const WS_CLIPCHILDREN: WINDOW_STYLE = 33554432;
pub const WS_CLIPSIBLINGS: WINDOW_STYLE = 67108864;
pub const WS_EX_APPWINDOW: WINDOW_EX_STYLE = 262144;
pub const WS_EX_LAYERED: WINDOW_EX_STYLE = 524288;
pub const WS_EX_NOACTIVATE: WINDOW_EX_STYLE = 134217728;
pub const WS_EX_NOREDIRECTIONBITMAP: WINDOW_EX_STYLE = 2097152;
pub const WS_EX_TOOLWINDOW: WINDOW_EX_STYLE = 128;
pub const WS_EX_TOPMOST: WINDOW_EX_STYLE = 8;
pub const WS_MAXIMIZEBOX: WINDOW_STYLE = 65536;
pub const WS_MINIMIZEBOX: WINDOW_STYLE = 131072;
pub const WS_OVERLAPPED: WINDOW_STYLE = 0;
pub const WS_OVERLAPPEDWINDOW: WINDOW_STYLE = 13565952;
pub const WS_POPUP: WINDOW_STYLE = 2147483648;
pub const WS_SYSMENU: WINDOW_STYLE = 524288;
pub const WS_THICKFRAME: WINDOW_STYLE = 262144;
pub const WS_VISIBLE: WINDOW_STYLE = 268435456;
