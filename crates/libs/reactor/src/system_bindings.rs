windows_core::link!("user32.dll" "system" fn BeginPaint(hwnd : HWND, lppaint : *mut PAINTSTRUCT) -> HDC);
windows_core::link!("user32.dll" "system" fn ClientToScreen(hwnd : HWND, lppoint : *mut POINT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn CloseClipboard() -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn CloseHandle(hobject : HANDLE) -> windows_core::BOOL);
windows_core::link!("coremessaging.dll" "system" fn CreateDispatcherQueueController(options : DispatcherQueueOptions, dispatcherqueuecontroller : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("kernel32.dll" "system" fn CreateEventW(lpeventattributes : *const SECURITY_ATTRIBUTES, bmanualreset : windows_core::BOOL, binitialstate : windows_core::BOOL, lpname : windows_core::PCWSTR) -> HANDLE);
windows_core::link!("kernel32.dll" "system" fn CreateWaitableTimerExW(lptimerattributes : *const SECURITY_ATTRIBUTES, lptimername : windows_core::PCWSTR, dwflags : u32, dwdesiredaccess : u32) -> HANDLE);
windows_core::link!("user32.dll" "system" fn CreateWindowExW(dwexstyle : u32, lpclassname : windows_core::PCWSTR, lpwindowname : windows_core::PCWSTR, dwstyle : u32, x : i32, y : i32, nwidth : i32, nheight : i32, hwndparent : HWND, hmenu : HMENU, hinstance : HINSTANCE, lpparam : *const core::ffi::c_void) -> HWND);
windows_core::link!("dcomp.dll" "system" fn DCompositionWaitForCompositorClock(count : u32, handles : *const HANDLE, timeoutinms : u32) -> u32);
windows_core::link!("user32.dll" "system" fn DefWindowProcW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> LRESULT);
windows_core::link!("user32.dll" "system" fn DestroyWindow(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn DispatchMessageW(lpmsg : *const MSG) -> LRESULT);
windows_core::link!("user32.dll" "system" fn EmptyClipboard() -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn EnableMouseInPointer(fenable : windows_core::BOOL) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn EndPaint(hwnd : HWND, lppaint : *const PAINTSTRUCT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetAsyncKeyState(vkey : i32) -> i16);
windows_core::link!("user32.dll" "system" fn GetCapture() -> HWND);
windows_core::link!("user32.dll" "system" fn GetCaretBlinkTime() -> u32);
windows_core::link!("user32.dll" "system" fn GetClientRect(hwnd : HWND, lprect : *mut RECT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetClipboardData(uformat : u32) -> HANDLE);
windows_core::link!("user32.dll" "system" fn GetDpiForWindow(hwnd : HWND) -> u32);
windows_core::link!("user32.dll" "system" fn GetKeyState(nvirtkey : i32) -> i16);
windows_core::link!("user32.dll" "system" fn GetMessageW(lpmsg : *mut MSG, hwnd : HWND, wmsgfiltermin : u32, wmsgfiltermax : u32) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn GetModuleHandleW(lpmodulename : windows_core::PCWSTR) -> HMODULE);
windows_core::link!("user32.dll" "system" fn GetMonitorInfoW(hmonitor : HMONITOR, lpmi : *mut MONITORINFO) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetPointerInfo(pointerid : u32, pointerinfo : *mut POINTER_INFO) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetSystemMetricsForDpi(nindex : i32, dpi : u32) -> i32);
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "x86_64"
))]
windows_core::link!("user32.dll" "system" fn GetWindowLongPtrW(hwnd : HWND, nindex : i32) -> isize);
#[cfg(target_pointer_width = "32")]
pub use GetWindowLongW as GetWindowLongPtrW;
windows_core::link!("user32.dll" "system" fn GetWindowLongW(hwnd : HWND, nindex : i32) -> i32);
windows_core::link!("user32.dll" "system" fn GetWindowRect(hwnd : HWND, lprect : *mut RECT) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn GlobalAlloc(uflags : u32, dwbytes : usize) -> HGLOBAL);
windows_core::link!("kernel32.dll" "system" fn GlobalFree(hmem : HGLOBAL) -> HGLOBAL);
windows_core::link!("kernel32.dll" "system" fn GlobalLock(hmem : HGLOBAL) -> *mut core::ffi::c_void);
windows_core::link!("kernel32.dll" "system" fn GlobalUnlock(hmem : HGLOBAL) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn IsZoomed(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn LoadCursorW(hinstance : HINSTANCE, lpcursorname : windows_core::PCWSTR) -> HCURSOR);
windows_core::link!("user32.dll" "system" fn MonitorFromPoint(pt : POINT, dwflags : u32) -> HMONITOR);
windows_core::link!("user32.dll" "system" fn MonitorFromWindow(hwnd : HWND, dwflags : u32) -> HMONITOR);
windows_core::link!("user32.dll" "system" fn OpenClipboard(hwndnewowner : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn PeekMessageW(lpmsg : *mut MSG, hwnd : HWND, wmsgfiltermin : u32, wmsgfiltermax : u32, wremovemsg : u32) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn PostMessageW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn PostQuitMessage(nexitcode : i32));
windows_core::link!("user32.dll" "system" fn RegisterClassExW(param0 : *const WNDCLASSEXW) -> ATOM);
windows_core::link!("user32.dll" "system" fn RegisterClassW(lpwndclass : *const WNDCLASSW) -> ATOM);
windows_core::link!("user32.dll" "system" fn ReleaseCapture() -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ScreenToClient(hwnd : HWND, lppoint : *mut POINT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn SendMessageW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> LRESULT);
windows_core::link!("user32.dll" "system" fn SetCapture(hwnd : HWND) -> HWND);
windows_core::link!("user32.dll" "system" fn SetClipboardData(uformat : u32, hmem : HANDLE) -> HANDLE);
windows_core::link!("user32.dll" "system" fn SetCursor(hcursor : HCURSOR) -> HCURSOR);
windows_core::link!("kernel32.dll" "system" fn SetEvent(hevent : HANDLE) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn SetProcessDpiAwarenessContext(value : DPI_AWARENESS_CONTEXT) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn SetWaitableTimerEx(htimer : HANDLE, lpduetime : *const i64, lperiod : i32, pfncompletionroutine : PTIMERAPCROUTINE, lpargtocompletionroutine : *const core::ffi::c_void, wakecontext : *const REASON_CONTEXT, tolerabledelay : u32) -> windows_core::BOOL);
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "x86_64"
))]
windows_core::link!("user32.dll" "system" fn SetWindowLongPtrW(hwnd : HWND, nindex : i32, dwnewlong : isize) -> isize);
#[cfg(target_pointer_width = "32")]
pub use SetWindowLongW as SetWindowLongPtrW;
windows_core::link!("user32.dll" "system" fn SetWindowLongW(hwnd : HWND, nindex : i32, dwnewlong : i32) -> i32);
windows_core::link!("user32.dll" "system" fn SetWindowPos(hwnd : HWND, hwndinsertafter : HWND, x : i32, y : i32, cx : i32, cy : i32, uflags : u32) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ShowWindow(hwnd : HWND, ncmdshow : i32) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn TrackMouseEvent(lpeventtrack : *mut TRACKMOUSEEVENT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn TranslateMessage(lpmsg : *const MSG) -> windows_core::BOOL);
windows_core::link!("uiautomationcore.dll" "system" fn UiaDisconnectProvider(pprovider : *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaHostProviderFromHwnd(hwnd : HWND, ppprovider : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaRaiseAutomationEvent(pprovider : *mut core::ffi::c_void, id : EVENTID) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaRaiseAutomationPropertyChangedEvent(pprovider : *mut core::ffi::c_void, id : PROPERTYID, oldvalue : VARIANT, newvalue : VARIANT) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaReturnRawElementProvider(hwnd : HWND, wparam : WPARAM, lparam : LPARAM, el : *mut core::ffi::c_void) -> LRESULT);
windows_core::link!("user32.dll" "system" fn UnregisterClassW(lpclassname : windows_core::PCWSTR, hinstance : HINSTANCE) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn UpdateWindow(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ValidateRect(hwnd : HWND, lprect : *const RECT) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn WaitForMultipleObjects(ncount : u32, lphandles : *const HANDLE, bwaitall : windows_core::BOOL, dwmilliseconds : u32) -> u32);
pub type ATOM = u16;
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
pub const CF_UNICODETEXT: u32 = 13;
pub type CLIPFORMAT = u16;
pub type COLORREF = u32;
pub type CONTROLTYPEID = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    pub dwExStyle: u32,
}
impl Default for CREATESTRUCTW {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub const CS_DBLCLKS: u32 = 8;
pub const CS_HREDRAW: u32 = 2;
pub const CS_OWNDC: u32 = 32;
pub const CS_VREDRAW: u32 = 1;
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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
windows_core::imp::required_hierarchy!(
    CompositionAnimation,
    ICompositionAnimationBase,
    CompositionObject
);
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
pub struct CompositionBatchCompletedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionBatchCompletedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionBatchCompletedEventArgs, CompositionObject);
impl windows_core::RuntimeType for CompositionBatchCompletedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionBatchCompletedEventArgs>();
}
unsafe impl windows_core::Interface for CompositionBatchCompletedEventArgs {
    type Vtable = <ICompositionBatchCompletedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <ICompositionBatchCompletedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionBatchCompletedEventArgs {
    type Target = ICompositionBatchCompletedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionBatchCompletedEventArgs {
    const NAME: &'static str = "Windows.UI.Composition.CompositionBatchCompletedEventArgs";
}
unsafe impl Send for CompositionBatchCompletedEventArgs {}
unsafe impl Sync for CompositionBatchCompletedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompositionBatchTypes(pub u32);
impl CompositionBatchTypes {
    pub const None: Self = Self(0);
    pub const Animation: Self = Self(1);
    pub const Effect: Self = Self(2);
    pub const InfiniteAnimation: Self = Self(4);
    pub const AllAnimations: Self = Self(5);
}
impl windows_core::TypeKind for CompositionBatchTypes {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for CompositionBatchTypes {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.CompositionBatchTypes;u4)",
    );
}
impl CompositionBatchTypes {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for CompositionBatchTypes {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for CompositionBatchTypes {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for CompositionBatchTypes {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0);
    }
}
impl core::ops::BitAndAssign for CompositionBatchTypes {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0);
    }
}
impl core::ops::Not for CompositionBatchTypes {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
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
pub struct CompositionColorGradientStop(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionColorGradientStop,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionColorGradientStop, CompositionObject);
impl windows_core::RuntimeType for CompositionColorGradientStop {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionColorGradientStop>();
}
unsafe impl windows_core::Interface for CompositionColorGradientStop {
    type Vtable = <ICompositionColorGradientStop as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionColorGradientStop as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionColorGradientStop {
    type Target = ICompositionColorGradientStop;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionColorGradientStop {
    const NAME: &'static str = "Windows.UI.Composition.CompositionColorGradientStop";
}
unsafe impl Send for CompositionColorGradientStop {}
unsafe impl Sync for CompositionColorGradientStop {}
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
pub struct CompositionEllipseGeometry(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionEllipseGeometry,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CompositionEllipseGeometry,
    CompositionGeometry,
    CompositionObject
);
impl windows_core::RuntimeType for CompositionEllipseGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionEllipseGeometry>();
}
unsafe impl windows_core::Interface for CompositionEllipseGeometry {
    type Vtable = <ICompositionEllipseGeometry as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionEllipseGeometry as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionEllipseGeometry {
    type Target = ICompositionEllipseGeometry;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionEllipseGeometry {
    const NAME: &'static str = "Windows.UI.Composition.CompositionEllipseGeometry";
}
unsafe impl Send for CompositionEllipseGeometry {}
unsafe impl Sync for CompositionEllipseGeometry {}
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
pub struct CompositionGradientBrush(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionGradientBrush,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CompositionGradientBrush,
    CompositionBrush,
    CompositionObject
);
impl windows_core::RuntimeType for CompositionGradientBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionGradientBrush>();
}
unsafe impl windows_core::Interface for CompositionGradientBrush {
    type Vtable = <ICompositionGradientBrush as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionGradientBrush as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionGradientBrush {
    type Target = ICompositionGradientBrush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionGradientBrush {
    const NAME: &'static str = "Windows.UI.Composition.CompositionGradientBrush";
}
unsafe impl Send for CompositionGradientBrush {}
unsafe impl Sync for CompositionGradientBrush {}
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
pub struct CompositionLinearGradientBrush(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionLinearGradientBrush,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CompositionLinearGradientBrush,
    CompositionGradientBrush,
    CompositionBrush,
    CompositionObject
);
impl windows_core::RuntimeType for CompositionLinearGradientBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionLinearGradientBrush>();
}
unsafe impl windows_core::Interface for CompositionLinearGradientBrush {
    type Vtable = <ICompositionLinearGradientBrush as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <ICompositionLinearGradientBrush as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionLinearGradientBrush {
    type Target = ICompositionLinearGradientBrush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionLinearGradientBrush {
    const NAME: &'static str = "Windows.UI.Composition.CompositionLinearGradientBrush";
}
unsafe impl Send for CompositionLinearGradientBrush {}
unsafe impl Sync for CompositionLinearGradientBrush {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionMaskBrush(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionMaskBrush,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionMaskBrush, CompositionBrush, CompositionObject);
impl windows_core::RuntimeType for CompositionMaskBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionMaskBrush>();
}
unsafe impl windows_core::Interface for CompositionMaskBrush {
    type Vtable = <ICompositionMaskBrush as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionMaskBrush as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionMaskBrush {
    type Target = ICompositionMaskBrush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionMaskBrush {
    const NAME: &'static str = "Windows.UI.Composition.CompositionMaskBrush";
}
unsafe impl Send for CompositionMaskBrush {}
unsafe impl Sync for CompositionMaskBrush {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionNineGridBrush(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionNineGridBrush,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CompositionNineGridBrush,
    CompositionBrush,
    CompositionObject
);
impl windows_core::RuntimeType for CompositionNineGridBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionNineGridBrush>();
}
unsafe impl windows_core::Interface for CompositionNineGridBrush {
    type Vtable = <ICompositionNineGridBrush as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionNineGridBrush as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionNineGridBrush {
    type Target = ICompositionNineGridBrush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionNineGridBrush {
    const NAME: &'static str = "Windows.UI.Composition.CompositionNineGridBrush";
}
unsafe impl Send for CompositionNineGridBrush {}
unsafe impl Sync for CompositionNineGridBrush {}
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
pub struct CompositionPath(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionPath,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionPath, IGeometrySource2D);
impl CompositionPath {
    pub(crate) fn Create<P0>(source: P0) -> windows_core::Result<Self>
    where
        P0: windows_core::Param<IGeometrySource2D>,
    {
        Self::ICompositionPathFactory(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).Create)(
                windows_core::Interface::as_raw(this),
                source.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    fn ICompositionPathFactory<
        R,
        F: FnOnce(&ICompositionPathFactory) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<CompositionPath, ICompositionPathFactory> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for CompositionPath {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionPath>();
}
unsafe impl windows_core::Interface for CompositionPath {
    type Vtable = <ICompositionPath as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionPath as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionPath {
    type Target = ICompositionPath;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionPath {
    const NAME: &'static str = "Windows.UI.Composition.CompositionPath";
}
unsafe impl Send for CompositionPath {}
unsafe impl Sync for CompositionPath {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionPathGeometry(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionPathGeometry,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CompositionPathGeometry,
    CompositionGeometry,
    CompositionObject
);
impl windows_core::RuntimeType for CompositionPathGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionPathGeometry>();
}
unsafe impl windows_core::Interface for CompositionPathGeometry {
    type Vtable = <ICompositionPathGeometry as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionPathGeometry as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionPathGeometry {
    type Target = ICompositionPathGeometry;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionPathGeometry {
    const NAME: &'static str = "Windows.UI.Composition.CompositionPathGeometry";
}
unsafe impl Send for CompositionPathGeometry {}
unsafe impl Sync for CompositionPathGeometry {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionRoundedRectangleGeometry(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionRoundedRectangleGeometry,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CompositionRoundedRectangleGeometry,
    CompositionGeometry,
    CompositionObject
);
impl windows_core::RuntimeType for CompositionRoundedRectangleGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionRoundedRectangleGeometry>();
}
unsafe impl windows_core::Interface for CompositionRoundedRectangleGeometry {
    type Vtable = <ICompositionRoundedRectangleGeometry as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <ICompositionRoundedRectangleGeometry as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionRoundedRectangleGeometry {
    type Target = ICompositionRoundedRectangleGeometry;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionRoundedRectangleGeometry {
    const NAME: &'static str = "Windows.UI.Composition.CompositionRoundedRectangleGeometry";
}
unsafe impl Send for CompositionRoundedRectangleGeometry {}
unsafe impl Sync for CompositionRoundedRectangleGeometry {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionScopedBatch(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionScopedBatch,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionScopedBatch, CompositionObject);
impl windows_core::RuntimeType for CompositionScopedBatch {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionScopedBatch>();
}
unsafe impl windows_core::Interface for CompositionScopedBatch {
    type Vtable = <ICompositionScopedBatch as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionScopedBatch as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionScopedBatch {
    type Target = ICompositionScopedBatch;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionScopedBatch {
    const NAME: &'static str = "Windows.UI.Composition.CompositionScopedBatch";
}
unsafe impl Send for CompositionScopedBatch {}
unsafe impl Sync for CompositionScopedBatch {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionShape(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionShape,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionShape, CompositionObject);
impl windows_core::RuntimeType for CompositionShape {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionShape>();
}
unsafe impl windows_core::Interface for CompositionShape {
    type Vtable = <ICompositionShape as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionShape as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionShape {
    type Target = ICompositionShape;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionShape {
    const NAME: &'static str = "Windows.UI.Composition.CompositionShape";
}
unsafe impl Send for CompositionShape {}
unsafe impl Sync for CompositionShape {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionShapeCollection(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionShapeCollection,
    windows_core::IUnknown,
    windows_core::IInspectable,
    windows_collections::IVector<CompositionShape>
);
windows_core::imp::required_hierarchy!(CompositionShapeCollection, CompositionObject);
impl windows_core::RuntimeType for CompositionShapeCollection {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<
        Self,
        windows_collections::IVector<CompositionShape>,
    >();
}
unsafe impl windows_core::Interface for CompositionShapeCollection {
    type Vtable =
        <windows_collections::IVector<CompositionShape> as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <windows_collections::IVector<CompositionShape> as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionShapeCollection {
    type Target = windows_collections::IVector<CompositionShape>;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionShapeCollection {
    const NAME: &'static str = "Windows.UI.Composition.CompositionShapeCollection";
}
unsafe impl Send for CompositionShapeCollection {}
unsafe impl Sync for CompositionShapeCollection {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionSpriteShape(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionSpriteShape,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionSpriteShape, CompositionShape, CompositionObject);
impl windows_core::RuntimeType for CompositionSpriteShape {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionSpriteShape>();
}
unsafe impl windows_core::Interface for CompositionSpriteShape {
    type Vtable = <ICompositionSpriteShape as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionSpriteShape as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionSpriteShape {
    type Target = ICompositionSpriteShape;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionSpriteShape {
    const NAME: &'static str = "Windows.UI.Composition.CompositionSpriteShape";
}
unsafe impl Send for CompositionSpriteShape {}
unsafe impl Sync for CompositionSpriteShape {}
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompositionStrokeCap(pub i32);
impl CompositionStrokeCap {
    pub const Flat: Self = Self(0);
    pub const Square: Self = Self(1);
    pub const Round: Self = Self(2);
    pub const Triangle: Self = Self(3);
}
impl windows_core::TypeKind for CompositionStrokeCap {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for CompositionStrokeCap {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.CompositionStrokeCap;i4)",
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
pub struct CompositionVisualSurface(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionVisualSurface,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CompositionVisualSurface,
    ICompositionSurface,
    CompositionObject
);
impl windows_core::RuntimeType for CompositionVisualSurface {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionVisualSurface>();
}
unsafe impl windows_core::Interface for CompositionVisualSurface {
    type Vtable = <ICompositionVisualSurface as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionVisualSurface as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionVisualSurface {
    type Target = ICompositionVisualSurface;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionVisualSurface {
    const NAME: &'static str = "Windows.UI.Composition.CompositionVisualSurface";
}
unsafe impl Send for CompositionVisualSurface {}
unsafe impl Sync for CompositionVisualSurface {}
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
    pub Anonymous: DECIMAL_0,
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DECIMAL_1_0 {
    pub Lo32: u32,
    pub Mid32: u32,
}
pub type DISPATCHERQUEUE_THREAD_APARTMENTTYPE = i32;
pub type DISPATCHERQUEUE_THREAD_TYPE = i32;
pub type DPI_AWARENESS_CONTEXT = *mut core::ffi::c_void;
pub const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: DPI_AWARENESS_CONTEXT = -4 as _;
pub const DQTAT_COM_ASTA: DISPATCHERQUEUE_THREAD_APARTMENTTYPE = 1;
pub const DQTAT_COM_NONE: DISPATCHERQUEUE_THREAD_APARTMENTTYPE = 0;
pub const DQTAT_COM_STA: DISPATCHERQUEUE_THREAD_APARTMENTTYPE = 2;
pub const DQTYPE_THREAD_CURRENT: DISPATCHERQUEUE_THREAD_TYPE = 2;
pub const DQTYPE_THREAD_DEDICATED: DISPATCHERQUEUE_THREAD_TYPE = 1;
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DispatcherQueueOptions {
    pub dwSize: u32,
    pub threadType: DISPATCHERQUEUE_THREAD_TYPE,
    pub apartmentType: DISPATCHERQUEUE_THREAD_APARTMENTTYPE,
}
pub type EVENTID = i32;
pub type ExpandCollapseState = i32;
pub const ExpandCollapseState_Collapsed: ExpandCollapseState = 0;
pub const ExpandCollapseState_Expanded: ExpandCollapseState = 1;
pub const ExpandCollapseState_LeafNode: ExpandCollapseState = 3;
pub const ExpandCollapseState_PartiallyExpanded: ExpandCollapseState = 2;
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
    ICompositionAnimationBase,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FORMATETC {
    pub cfFormat: CLIPFORMAT,
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
pub const GMEM_MOVEABLE: u32 = 2;
pub const GMEM_ZEROINIT: u32 = 64;
pub const GWLP_USERDATA: i32 = -21;
pub const GWLP_WNDPROC: i32 = -4;
pub type HANDLE = *mut core::ffi::c_void;
pub type HBRUSH = *mut core::ffi::c_void;
pub type HCURSOR = HICON;
pub type HDC = *mut core::ffi::c_void;
pub type HGLOBAL = HANDLE;
pub type HICON = *mut core::ffi::c_void;
pub type HINSTANCE = *mut core::ffi::c_void;
pub type HMENU = *mut core::ffi::c_void;
pub type HMODULE = HINSTANCE;
pub type HMONITOR = *mut core::ffi::c_void;
pub const HTBOTTOM: u32 = 15;
pub const HTBOTTOMLEFT: u32 = 16;
pub const HTBOTTOMRIGHT: u32 = 17;
pub const HTCAPTION: u32 = 2;
pub const HTCLIENT: u32 = 1;
pub const HTCLOSE: u32 = 20;
pub const HTLEFT: u32 = 10;
pub const HTMAXBUTTON: u32 = 9;
pub const HTMINBUTTON: u32 = 8;
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
    ICompositionAnimation2,
    ICompositionAnimation2_Vtbl,
    0x369b603e_a80f_4948_93e3_ed23fb38c6cb
);
impl windows_core::RuntimeType for ICompositionAnimation2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionAnimation2 {
    pub(crate) fn SetTarget(&self, value: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTarget)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(value)),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionAnimation2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    SetBooleanParameter: usize,
    Target: usize,
    pub SetTarget: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionAnimationBase,
    ICompositionAnimationBase_Vtbl,
    0x1c2c2999_e818_48d3_a6dd_d78c82f8ace9
);
impl windows_core::RuntimeType for ICompositionAnimationBase {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(
    ICompositionAnimationBase,
    windows_core::IUnknown,
    windows_core::IInspectable
);
#[repr(C)]
pub struct ICompositionAnimationBase_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionBatchCompletedEventArgs,
    ICompositionBatchCompletedEventArgs_Vtbl,
    0x0d00dad0_9464_450a_a562_2e2698b0a812
);
impl windows_core::RuntimeType for ICompositionBatchCompletedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionBatchCompletedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
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
    ICompositionClip2,
    ICompositionClip2_Vtbl,
    0x5893e069_3516_40e1_89e0_5ba924927235
);
impl windows_core::RuntimeType for ICompositionClip2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionClip2 {
    pub(crate) fn SetCenterPoint(
        &self,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetCenterPoint)(
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
}
#[repr(C)]
pub struct ICompositionClip2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    AnchorPoint: usize,
    SetAnchorPoint: usize,
    CenterPoint: usize,
    pub SetCenterPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    Offset: usize,
    SetOffset: usize,
    RotationAngle: usize,
    pub SetRotationAngle:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
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
    ICompositionColorGradientStop,
    ICompositionColorGradientStop_Vtbl,
    0x6f00ca92_c801_4e41_9a8f_a53e20f57778
);
impl windows_core::RuntimeType for ICompositionColorGradientStop {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionColorGradientStop_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
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
    pub(crate) unsafe fn EndDraw(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).EndDraw)(windows_core::Interface::as_raw(self))
        }
    }
    pub(crate) unsafe fn Resize(&self, sizepixels: SIZE) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Resize)(
                windows_core::Interface::as_raw(self),
                sizepixels,
            )
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
    ICompositionEllipseGeometry,
    ICompositionEllipseGeometry_Vtbl,
    0x4801f884_f6ad_4b93_afa9_897b64e57b1f
);
impl windows_core::RuntimeType for ICompositionEllipseGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionEllipseGeometry {
    pub(crate) fn SetCenter(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetCenter)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetRadius(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionEllipseGeometry_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Center: usize,
    pub SetCenter: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    Radius: usize,
    pub SetRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
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
impl ICompositionGeometry {
    pub(crate) fn SetTrimEnd(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTrimEnd)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetTrimStart(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTrimStart)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionGeometry_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    TrimEnd: usize,
    pub SetTrimEnd: unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    TrimOffset: usize,
    SetTrimOffset: usize,
    TrimStart: usize,
    pub SetTrimStart:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionGradientBrush,
    ICompositionGradientBrush_Vtbl,
    0x1d9709e0_ffc6_4c0e_a9ab_34144d4c9098
);
impl windows_core::RuntimeType for ICompositionGradientBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionGradientBrush_Vtbl {
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
    ICompositionLinearGradientBrush,
    ICompositionLinearGradientBrush_Vtbl,
    0x983bc519_a9db_413c_a2d8_2a9056fc525e
);
impl windows_core::RuntimeType for ICompositionLinearGradientBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionLinearGradientBrush_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionMaskBrush,
    ICompositionMaskBrush_Vtbl,
    0x522cf09e_be6b_4f41_be49_f9226d471b4a
);
impl windows_core::RuntimeType for ICompositionMaskBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionMaskBrush {
    pub(crate) fn SetMask<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<CompositionBrush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetMask)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn SetSource<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<CompositionBrush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetSource)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionMaskBrush_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Mask: usize,
    pub SetMask: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    Source: usize,
    pub SetSource: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionNineGridBrush,
    ICompositionNineGridBrush_Vtbl,
    0xf25154e4_bc8c_4be7_b80f_8685b83c0186
);
impl windows_core::RuntimeType for ICompositionNineGridBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionNineGridBrush {
    pub(crate) fn SetSource<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<CompositionBrush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetSource)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn SetInsets(&self, inset: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetInsets)(
                windows_core::Interface::as_raw(self),
                inset,
            )
            .ok()
        }
    }
    pub(crate) fn SetInsetsWithValues(
        &self,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetInsetsWithValues)(
                windows_core::Interface::as_raw(self),
                left,
                top,
                right,
                bottom,
            )
            .ok()
        }
    }
    pub(crate) fn SetInsetScales(&self, scale: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetInsetScales)(
                windows_core::Interface::as_raw(self),
                scale,
            )
            .ok()
        }
    }
    pub(crate) fn SetInsetScalesWithValues(
        &self,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetInsetScalesWithValues)(
                windows_core::Interface::as_raw(self),
                left,
                top,
                right,
                bottom,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionNineGridBrush_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    BottomInset: usize,
    SetBottomInset: usize,
    BottomInsetScale: usize,
    SetBottomInsetScale: usize,
    IsCenterHollow: usize,
    SetIsCenterHollow: usize,
    LeftInset: usize,
    SetLeftInset: usize,
    LeftInsetScale: usize,
    SetLeftInsetScale: usize,
    RightInset: usize,
    SetRightInset: usize,
    RightInsetScale: usize,
    SetRightInsetScale: usize,
    Source: usize,
    pub SetSource: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    TopInset: usize,
    SetTopInset: usize,
    TopInsetScale: usize,
    SetTopInsetScale: usize,
    pub SetInsets: unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    pub SetInsetsWithValues: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        f32,
        f32,
        f32,
    ) -> windows_core::HRESULT,
    pub SetInsetScales:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    pub SetInsetScalesWithValues: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        f32,
        f32,
        f32,
    ) -> windows_core::HRESULT,
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
    ICompositionObject2,
    ICompositionObject2_Vtbl,
    0xef874ea1_5cff_4b68_9e30_a1519d08ba03
);
impl windows_core::RuntimeType for ICompositionObject2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionObject2 {
    pub(crate) fn SetImplicitAnimations<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<ImplicitAnimationCollection>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetImplicitAnimations)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionObject2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Comment: usize,
    SetComment: usize,
    ImplicitAnimations: usize,
    pub SetImplicitAnimations: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionPath,
    ICompositionPath_Vtbl,
    0x66da1d5f_2e10_4f22_8a06_0a8151919e60
);
impl windows_core::RuntimeType for ICompositionPath {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionPath_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionPathFactory,
    ICompositionPathFactory_Vtbl,
    0x9c1e8c6a_0f33_4751_9437_eb3fb9d3ab07
);
impl windows_core::RuntimeType for ICompositionPathFactory {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionPathFactory_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Create: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionPathGeometry,
    ICompositionPathGeometry_Vtbl,
    0x0b6a417e_2c77_4c23_af5e_6304c147bb61
);
impl windows_core::RuntimeType for ICompositionPathGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionPathGeometry_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionRoundedRectangleGeometry,
    ICompositionRoundedRectangleGeometry_Vtbl,
    0x8770c822_1d50_4b8b_b013_7c9a0e46935f
);
impl windows_core::RuntimeType for ICompositionRoundedRectangleGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionRoundedRectangleGeometry {
    pub(crate) fn SetCornerRadius(
        &self,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetCornerRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetOffset(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetOffset)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
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
pub struct ICompositionRoundedRectangleGeometry_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    CornerRadius: usize,
    pub SetCornerRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    Offset: usize,
    pub SetOffset: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    Size: usize,
    pub SetSize: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionScopedBatch,
    ICompositionScopedBatch_Vtbl,
    0x0d00dad0_fb07_46fd_8c72_6280d1a3d1dd
);
impl windows_core::RuntimeType for ICompositionScopedBatch {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionScopedBatch {
    pub(crate) fn End(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).End)(windows_core::Interface::as_raw(self)).ok()
        }
    }
    pub(crate) fn Completed<F>(
        &self,
        handler: F,
    ) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<windows_core::IInspectable>,
                windows_core::Ref<CompositionBatchCompletedEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<
            windows_core::IInspectable,
            CompositionBatchCompletedEventArgs,
        > = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<windows_core::IInspectable, CompositionBatchCompletedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<
                    windows_core::IInspectable,
                    CompositionBatchCompletedEventArgs,
                    F,
                >::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Completed)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveCompleted,
            ))
        }
    }
}
#[repr(C)]
pub struct ICompositionScopedBatch_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    IsActive: usize,
    IsEnded: usize,
    pub End: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    Resume: usize,
    Suspend: usize,
    pub Completed: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveCompleted:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionShape,
    ICompositionShape_Vtbl,
    0xb47ce2f7_9a88_42c4_9e87_2e500ca8688c
);
impl windows_core::RuntimeType for ICompositionShape {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionShape_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionSpriteShape,
    ICompositionSpriteShape_Vtbl,
    0x401b61bb_0007_4363_b1f3_6bcc003fb83e
);
impl windows_core::RuntimeType for ICompositionSpriteShape {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionSpriteShape {
    pub(crate) fn SetFillBrush<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<CompositionBrush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetFillBrush)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn SetGeometry<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<CompositionGeometry>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetGeometry)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn SetStrokeBrush<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<CompositionBrush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetStrokeBrush)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn SetStrokeEndCap(&self, value: CompositionStrokeCap) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetStrokeEndCap)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetStrokeStartCap(
        &self,
        value: CompositionStrokeCap,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetStrokeStartCap)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetStrokeThickness(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetStrokeThickness)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionSpriteShape_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    FillBrush: usize,
    pub SetFillBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    Geometry: usize,
    pub SetGeometry: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    IsStrokeNonScaling: usize,
    SetIsStrokeNonScaling: usize,
    StrokeBrush: usize,
    pub SetStrokeBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    StrokeDashArray: usize,
    StrokeDashCap: usize,
    SetStrokeDashCap: usize,
    StrokeDashOffset: usize,
    SetStrokeDashOffset: usize,
    StrokeEndCap: usize,
    pub SetStrokeEndCap: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        CompositionStrokeCap,
    ) -> windows_core::HRESULT,
    StrokeLineJoin: usize,
    SetStrokeLineJoin: usize,
    StrokeMiterLimit: usize,
    SetStrokeMiterLimit: usize,
    StrokeStartCap: usize,
    pub SetStrokeStartCap: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        CompositionStrokeCap,
    ) -> windows_core::HRESULT,
    StrokeThickness: usize,
    pub SetStrokeThickness:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
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
    ICompositionVisualSurface,
    ICompositionVisualSurface_Vtbl,
    0xb224d803_4f6e_4a3f_8cae_3dc1cda74fc6
);
impl windows_core::RuntimeType for ICompositionVisualSurface {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionVisualSurface {
    pub(crate) fn SetSourceVisual<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<Visual>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetSourceVisual)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn SetSourceOffset(
        &self,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSourceOffset)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetSourceSize(
        &self,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSourceSize)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionVisualSurface_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    SourceVisual: usize,
    pub SetSourceVisual: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    SourceOffset: usize,
    pub SetSourceOffset: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    SourceSize: usize,
    pub SetSourceSize: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
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
    pub(crate) fn CreateScopedBatch(
        &self,
        batchtype: CompositionBatchTypes,
    ) -> windows_core::Result<CompositionScopedBatch> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateScopedBatch)(
                windows_core::Interface::as_raw(self),
                batchtype,
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
    CreateEffectFactory: usize,
    CreateEffectFactoryWithProperties: usize,
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
    CreatePropertySet: usize,
    CreateQuaternionKeyFrameAnimation: usize,
    pub CreateScalarKeyFrameAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateScopedBatch: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        CompositionBatchTypes,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
    ICompositor2,
    ICompositor2_Vtbl,
    0x735081dc_5e24_45da_a38f_e32cc349a9a0
);
impl windows_core::RuntimeType for ICompositor2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositor2 {
    pub(crate) fn CreateImplicitAnimationCollection(
        &self,
    ) -> windows_core::Result<ImplicitAnimationCollection> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateImplicitAnimationCollection)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateMaskBrush(&self) -> windows_core::Result<CompositionMaskBrush> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateMaskBrush)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateNineGridBrush(&self) -> windows_core::Result<CompositionNineGridBrush> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateNineGridBrush)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateStepEasingFunction(&self) -> windows_core::Result<StepEasingFunction> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateStepEasingFunction)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositor2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    CreateAmbientLight: usize,
    CreateAnimationGroup: usize,
    CreateBackdropBrush: usize,
    CreateDistantLight: usize,
    CreateDropShadow: usize,
    pub CreateImplicitAnimationCollection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateLayerVisual: usize,
    pub CreateMaskBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateNineGridBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreatePointLight: usize,
    CreateSpotLight: usize,
    pub CreateStepEasingFunction: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositor4,
    ICompositor4_Vtbl,
    0xae47e78a_7910_4425_a482_a05b758adce9
);
impl windows_core::RuntimeType for ICompositor4 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositor4 {
    pub(crate) fn CreateColorGradientStopWithOffsetAndColor(
        &self,
        offset: f32,
        color: Color,
    ) -> windows_core::Result<CompositionColorGradientStop> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateColorGradientStopWithOffsetAndColor)(
                windows_core::Interface::as_raw(self),
                offset,
                color,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateLinearGradientBrush(
        &self,
    ) -> windows_core::Result<CompositionLinearGradientBrush> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateLinearGradientBrush)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateSpringScalarAnimation(
        &self,
    ) -> windows_core::Result<SpringScalarNaturalMotionAnimation> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateSpringScalarAnimation)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateSpringVector2Animation(
        &self,
    ) -> windows_core::Result<SpringVector2NaturalMotionAnimation> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateSpringVector2Animation)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateSpringVector3Animation(
        &self,
    ) -> windows_core::Result<SpringVector3NaturalMotionAnimation> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateSpringVector3Animation)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositor4_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    CreateColorGradientStop: usize,
    pub CreateColorGradientStopWithOffsetAndColor:
        unsafe extern "system" fn(
            *mut core::ffi::c_void,
            f32,
            Color,
            *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT,
    pub CreateLinearGradientBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateSpringScalarAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateSpringVector2Animation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateSpringVector3Animation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositor5,
    ICompositor5_Vtbl,
    0x48ea31ad_7fcd_4076_a79c_90cc4b852c9b
);
impl windows_core::RuntimeType for ICompositor5 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositor5 {
    pub(crate) fn CreateEllipseGeometry(&self) -> windows_core::Result<CompositionEllipseGeometry> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateEllipseGeometry)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreatePathGeometryWithPath<P0>(
        &self,
        path: P0,
    ) -> windows_core::Result<CompositionPathGeometry>
    where
        P0: windows_core::Param<CompositionPath>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreatePathGeometryWithPath)(
                windows_core::Interface::as_raw(self),
                path.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateRoundedRectangleGeometry(
        &self,
    ) -> windows_core::Result<CompositionRoundedRectangleGeometry> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateRoundedRectangleGeometry)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateShapeVisual(&self) -> windows_core::Result<ShapeVisual> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateShapeVisual)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateSpriteShape(&self) -> windows_core::Result<CompositionSpriteShape> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateSpriteShape)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateSpriteShapeWithGeometry<P0>(
        &self,
        geometry: P0,
    ) -> windows_core::Result<CompositionSpriteShape>
    where
        P0: windows_core::Param<CompositionGeometry>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateSpriteShapeWithGeometry)(
                windows_core::Interface::as_raw(self),
                geometry.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositor5_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Comment: usize,
    SetComment: usize,
    GlobalPlaybackRate: usize,
    SetGlobalPlaybackRate: usize,
    CreateBounceScalarAnimation: usize,
    CreateBounceVector2Animation: usize,
    CreateBounceVector3Animation: usize,
    CreateContainerShape: usize,
    pub CreateEllipseGeometry: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateLineGeometry: usize,
    CreatePathGeometry: usize,
    pub CreatePathGeometryWithPath: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreatePathKeyFrameAnimation: usize,
    CreateRectangleGeometry: usize,
    pub CreateRoundedRectangleGeometry: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateShapeVisual: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateSpriteShape: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateSpriteShapeWithGeometry: unsafe extern "system" fn(
        *mut core::ffi::c_void,
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
    ) -> windows_core::Result<IDesktopWindowTarget> {
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
    ) -> windows_core::Result<ICompositionGraphicsDevice>
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
    ICompositorWithVisualSurface,
    ICompositorWithVisualSurface_Vtbl,
    0xcfa1658b_0123_4551_8891_89bdcc40322b
);
impl windows_core::RuntimeType for ICompositorWithVisualSurface {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositorWithVisualSurface {
    pub(crate) fn CreateVisualSurface(&self) -> windows_core::Result<CompositionVisualSurface> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateVisualSurface)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositorWithVisualSurface_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateVisualSurface: unsafe extern "system" fn(
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
#[repr(C)]
pub struct ID2D1Geometry_Vtbl {
    pub base__: ID2D1Resource_Vtbl,
    GetBounds: usize,
    GetWidenedBounds: usize,
    StrokeContainsPoint: usize,
    FillContainsPoint: usize,
    CompareWithGeometry: usize,
    Simplify: usize,
    Tessellate: usize,
    CombineWithGeometry: usize,
    Outline: usize,
    ComputeArea: usize,
    ComputeLength: usize,
    ComputePointAtLength: usize,
    Widen: usize,
}
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
    IEnumTfContextViews,
    IEnumTfContextViews_Vtbl,
    0xf0c0f8dd_cf38_44e1_bb0f_68cf0d551c78
);
windows_core::imp::interface_hierarchy!(IEnumTfContextViews, windows_core::IUnknown);
#[repr(C)]
pub struct IEnumTfContextViews_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    Clone: usize,
    Next: usize,
    Reset: usize,
    Skip: usize,
}
windows_core::imp::define_interface!(
    IEnumTfDisplayAttributeInfo,
    IEnumTfDisplayAttributeInfo_Vtbl,
    0x7cef04d7_cb75_4e80_a7ab_5f5bc7d332de
);
windows_core::imp::interface_hierarchy!(IEnumTfDisplayAttributeInfo, windows_core::IUnknown);
#[repr(C)]
pub struct IEnumTfDisplayAttributeInfo_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    Clone: usize,
    Next: usize,
    Reset: usize,
    Skip: usize,
}
windows_core::imp::define_interface!(
    IEnumTfProperties,
    IEnumTfProperties_Vtbl,
    0x19188cb0_aca9_11d2_afc5_00105a2799b5
);
windows_core::imp::interface_hierarchy!(IEnumTfProperties, windows_core::IUnknown);
#[repr(C)]
pub struct IEnumTfProperties_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    Clone: usize,
    Next: usize,
    Reset: usize,
    Skip: usize,
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
windows_core::imp::define_interface!(
    IGeometrySource2D,
    IGeometrySource2D_Vtbl,
    0xcaff7902_670c_4181_a624_da977203b845
);
impl windows_core::RuntimeType for IGeometrySource2D {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
    const NAME: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"Windows.Graphics.IGeometrySource2D");
}
windows_core::imp::interface_hierarchy!(
    IGeometrySource2D,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeName for IGeometrySource2D {
    const NAME: &'static str = "Windows.Graphics.IGeometrySource2D";
}
pub trait IGeometrySource2D_Impl: windows_core::IUnknownImpl {}
impl IGeometrySource2D_Vtbl {
    pub const fn new<Identity: IGeometrySource2D_Impl, const OFFSET: isize>() -> Self {
        Self {
            base__: windows_core::IInspectable_Vtbl::new::<Identity, IGeometrySource2D, OFFSET>(),
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IGeometrySource2D as windows_core::Interface>::IID
    }
}
#[repr(C)]
pub struct IGeometrySource2D_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IGeometrySource2DInterop,
    IGeometrySource2DInterop_Vtbl,
    0x0657af73_53fd_47cf_84ff_c8492d2a80a3
);
windows_core::imp::interface_hierarchy!(IGeometrySource2DInterop, windows_core::IUnknown);
#[repr(C)]
pub struct IGeometrySource2DInterop_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetGeometry: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub TryGetGeometryUsingFactory: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
pub trait IGeometrySource2DInterop_Impl: windows_core::IUnknownImpl {
    fn GetGeometry(&self) -> windows_core::Result<ID2D1Geometry>;
    fn TryGetGeometryUsingFactory(
        &self,
        factory: windows_core::Ref<ID2D1Factory>,
    ) -> windows_core::Result<ID2D1Geometry>;
}
impl IGeometrySource2DInterop_Vtbl {
    pub const fn new<Identity: IGeometrySource2DInterop_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn GetGeometry<
            Identity: IGeometrySource2DInterop_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            value: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IGeometrySource2DInterop_Impl::GetGeometry(this) {
                    Ok(ok__) => {
                        value.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn TryGetGeometryUsingFactory<
            Identity: IGeometrySource2DInterop_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            factory: *mut core::ffi::c_void,
            value: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IGeometrySource2DInterop_Impl::TryGetGeometryUsingFactory(
                    this,
                    core::mem::transmute_copy(&factory),
                ) {
                    Ok(ok__) => {
                        value.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            GetGeometry: GetGeometry::<Identity, OFFSET>,
            TryGetGeometryUsingFactory: TryGetGeometryUsingFactory::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IGeometrySource2DInterop as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IGeometrySource2DInterop {}
windows_core::imp::define_interface!(
    IImplicitAnimationCollection,
    IImplicitAnimationCollection_Vtbl,
    0x0598a3ff_0a92_4c9d_a427_b25519250dbf
);
impl windows_core::RuntimeType for IImplicitAnimationCollection {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IImplicitAnimationCollection_Vtbl {
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
windows_core::imp::define_interface!(
    INaturalMotionAnimation,
    INaturalMotionAnimation_Vtbl,
    0x438de12d_769b_4821_a949_284a6547e873
);
impl windows_core::RuntimeType for INaturalMotionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct INaturalMotionAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
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
    IRawElementProviderAdviseEvents,
    IRawElementProviderAdviseEvents_Vtbl,
    0xa407b27b_0f6d_4427_9292_473c7bf93258
);
windows_core::imp::interface_hierarchy!(IRawElementProviderAdviseEvents, windows_core::IUnknown);
#[repr(C)]
pub struct IRawElementProviderAdviseEvents_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub AdviseEventAdded: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        EVENTID,
        *const SAFEARRAY,
    ) -> windows_core::HRESULT,
    pub AdviseEventRemoved: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        EVENTID,
        *const SAFEARRAY,
    ) -> windows_core::HRESULT,
}
pub trait IRawElementProviderAdviseEvents_Impl: windows_core::IUnknownImpl {
    fn AdviseEventAdded(
        &self,
        eventid: EVENTID,
        propertyids: *const SAFEARRAY,
    ) -> windows_core::Result<()>;
    fn AdviseEventRemoved(
        &self,
        eventid: EVENTID,
        propertyids: *const SAFEARRAY,
    ) -> windows_core::Result<()>;
}
impl IRawElementProviderAdviseEvents_Vtbl {
    pub const fn new<Identity: IRawElementProviderAdviseEvents_Impl, const OFFSET: isize>() -> Self
    {
        unsafe extern "system" fn AdviseEventAdded<
            Identity: IRawElementProviderAdviseEvents_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            eventid: EVENTID,
            propertyids: *const SAFEARRAY,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IRawElementProviderAdviseEvents_Impl::AdviseEventAdded(
                    this,
                    core::mem::transmute_copy(&eventid),
                    core::mem::transmute_copy(&propertyids),
                )
                .into()
            }
        }
        unsafe extern "system" fn AdviseEventRemoved<
            Identity: IRawElementProviderAdviseEvents_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            eventid: EVENTID,
            propertyids: *const SAFEARRAY,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IRawElementProviderAdviseEvents_Impl::AdviseEventRemoved(
                    this,
                    core::mem::transmute_copy(&eventid),
                    core::mem::transmute_copy(&propertyids),
                )
                .into()
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            AdviseEventAdded: AdviseEventAdded::<Identity, OFFSET>,
            AdviseEventRemoved: AdviseEventRemoved::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IRawElementProviderAdviseEvents as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IRawElementProviderAdviseEvents {}
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
    pub get_BoundingRectangle:
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
    fn get_BoundingRectangle(&self) -> windows_core::Result<UiaRect>;
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
        unsafe extern "system" fn get_BoundingRectangle<
            Identity: IRawElementProviderFragment_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut UiaRect,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IRawElementProviderFragment_Impl::get_BoundingRectangle(this) {
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
            get_BoundingRectangle: get_BoundingRectangle::<Identity, OFFSET>,
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
        PATTERNID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetPropertyValue: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        PROPERTYID,
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
        patternid: PATTERNID,
    ) -> windows_core::Result<windows_core::IUnknown>;
    fn GetPropertyValue(&self, propertyid: PROPERTYID) -> windows_core::Result<VARIANT>;
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
            patternid: PATTERNID,
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
            propertyid: PROPERTYID,
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
    IScalarNaturalMotionAnimation,
    IScalarNaturalMotionAnimation_Vtbl,
    0x94a94581_bf92_495b_b5bd_d2c659430737
);
impl windows_core::RuntimeType for IScalarNaturalMotionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IScalarNaturalMotionAnimation {
    pub(crate) fn SetFinalValue(&self, value: Option<f32>) -> windows_core::Result<()> {
        let value__ = value.map(<windows_reference::IReference<f32> as From<_>>::from);
        unsafe {
            (windows_core::Interface::vtable(self).SetFinalValue)(
                windows_core::Interface::as_raw(self),
                windows_core::Param::param(value__.as_ref()).abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IScalarNaturalMotionAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    FinalValue: usize,
    pub SetFinalValue: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IScrollItemProvider,
    IScrollItemProvider_Vtbl,
    0x2360c714_4bf1_4b26_ba65_9b21316127eb
);
windows_core::imp::interface_hierarchy!(IScrollItemProvider, windows_core::IUnknown);
#[repr(C)]
pub struct IScrollItemProvider_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub ScrollIntoView: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
pub trait IScrollItemProvider_Impl: windows_core::IUnknownImpl {
    fn ScrollIntoView(&self) -> windows_core::Result<()>;
}
impl IScrollItemProvider_Vtbl {
    pub const fn new<Identity: IScrollItemProvider_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn ScrollIntoView<
            Identity: IScrollItemProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IScrollItemProvider_Impl::ScrollIntoView(this).into()
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            ScrollIntoView: ScrollIntoView::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IScrollItemProvider as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IScrollItemProvider {}
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
    IShapeVisual,
    IShapeVisual_Vtbl,
    0xf2bd13c3_ba7e_4b0f_9126_ffb7536b8176
);
impl windows_core::RuntimeType for IShapeVisual {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IShapeVisual {
    pub(crate) fn Shapes(&self) -> windows_core::Result<CompositionShapeCollection> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Shapes)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IShapeVisual_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Shapes: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ISpringScalarNaturalMotionAnimation,
    ISpringScalarNaturalMotionAnimation_Vtbl,
    0x0572a95f_37f9_4fbe_b87b_5cd03a89501c
);
impl windows_core::RuntimeType for ISpringScalarNaturalMotionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ISpringScalarNaturalMotionAnimation {
    pub(crate) fn SetDampingRatio(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetDampingRatio)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetPeriod(&self, value: TimeSpan) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPeriod)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ISpringScalarNaturalMotionAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    DampingRatio: usize,
    pub SetDampingRatio:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    Period: usize,
    pub SetPeriod:
        unsafe extern "system" fn(*mut core::ffi::c_void, TimeSpan) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ISpringVector2NaturalMotionAnimation,
    ISpringVector2NaturalMotionAnimation_Vtbl,
    0x23f494b5_ee73_4f0f_a423_402b946df4b3
);
impl windows_core::RuntimeType for ISpringVector2NaturalMotionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ISpringVector2NaturalMotionAnimation {
    pub(crate) fn SetDampingRatio(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetDampingRatio)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetPeriod(&self, value: TimeSpan) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPeriod)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ISpringVector2NaturalMotionAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    DampingRatio: usize,
    pub SetDampingRatio:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    Period: usize,
    pub SetPeriod:
        unsafe extern "system" fn(*mut core::ffi::c_void, TimeSpan) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ISpringVector3NaturalMotionAnimation,
    ISpringVector3NaturalMotionAnimation_Vtbl,
    0x6c8749df_d57b_4794_8e2d_cecb11e194e5
);
impl windows_core::RuntimeType for ISpringVector3NaturalMotionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ISpringVector3NaturalMotionAnimation {
    pub(crate) fn SetDampingRatio(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetDampingRatio)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetPeriod(&self, value: TimeSpan) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPeriod)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ISpringVector3NaturalMotionAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    DampingRatio: usize,
    pub SetDampingRatio:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    Period: usize,
    pub SetPeriod:
        unsafe extern "system" fn(*mut core::ffi::c_void, TimeSpan) -> windows_core::HRESULT,
}
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
    IStepEasingFunction,
    IStepEasingFunction_Vtbl,
    0xd0caa74b_560c_4a0b_a5f6_206ca8c3ecd6
);
impl windows_core::RuntimeType for IStepEasingFunction {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IStepEasingFunction_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ITextProvider,
    ITextProvider_Vtbl,
    0x3589c92c_63f3_4367_99bb_ada653b77cf2
);
windows_core::imp::interface_hierarchy!(ITextProvider, windows_core::IUnknown);
#[repr(C)]
pub struct ITextProvider_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetSelection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut SAFEARRAY,
    ) -> windows_core::HRESULT,
    pub GetVisibleRanges: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut SAFEARRAY,
    ) -> windows_core::HRESULT,
    pub RangeFromChild: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub RangeFromPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        UiaPoint,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub DocumentRange: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SupportedTextSelection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut SupportedTextSelection,
    ) -> windows_core::HRESULT,
}
pub trait ITextProvider_Impl: windows_core::IUnknownImpl {
    fn GetSelection(&self) -> windows_core::Result<*mut SAFEARRAY>;
    fn GetVisibleRanges(&self) -> windows_core::Result<*mut SAFEARRAY>;
    fn RangeFromChild(
        &self,
        childelement: windows_core::Ref<IRawElementProviderSimple>,
    ) -> windows_core::Result<ITextRangeProvider>;
    fn RangeFromPoint(&self, point: &UiaPoint) -> windows_core::Result<ITextRangeProvider>;
    fn DocumentRange(&self) -> windows_core::Result<ITextRangeProvider>;
    fn SupportedTextSelection(&self) -> windows_core::Result<SupportedTextSelection>;
}
impl ITextProvider_Vtbl {
    pub const fn new<Identity: ITextProvider_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn GetSelection<
            Identity: ITextProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut SAFEARRAY,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextProvider_Impl::GetSelection(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetVisibleRanges<
            Identity: ITextProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut SAFEARRAY,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextProvider_Impl::GetVisibleRanges(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn RangeFromChild<
            Identity: ITextProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            childelement: *mut core::ffi::c_void,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextProvider_Impl::RangeFromChild(
                    this,
                    core::mem::transmute_copy(&childelement),
                ) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn RangeFromPoint<
            Identity: ITextProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            point: UiaPoint,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextProvider_Impl::RangeFromPoint(this, core::mem::transmute(&point)) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn DocumentRange<
            Identity: ITextProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextProvider_Impl::DocumentRange(this) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn SupportedTextSelection<
            Identity: ITextProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut SupportedTextSelection,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextProvider_Impl::SupportedTextSelection(this) {
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
            GetVisibleRanges: GetVisibleRanges::<Identity, OFFSET>,
            RangeFromChild: RangeFromChild::<Identity, OFFSET>,
            RangeFromPoint: RangeFromPoint::<Identity, OFFSET>,
            DocumentRange: DocumentRange::<Identity, OFFSET>,
            SupportedTextSelection: SupportedTextSelection::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<ITextProvider as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for ITextProvider {}
windows_core::imp::define_interface!(
    ITextRangeProvider,
    ITextRangeProvider_Vtbl,
    0x5347ad7b_c355_46f8_aff5_909033582f63
);
windows_core::imp::interface_hierarchy!(ITextRangeProvider, windows_core::IUnknown);
#[repr(C)]
pub struct ITextRangeProvider_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Clone: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub Compare: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub CompareEndpoints: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TextPatternRangeEndpoint,
        *mut core::ffi::c_void,
        TextPatternRangeEndpoint,
        *mut i32,
    ) -> windows_core::HRESULT,
    pub ExpandToEnclosingUnit:
        unsafe extern "system" fn(*mut core::ffi::c_void, TextUnit) -> windows_core::HRESULT,
    pub FindAttribute: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TEXTATTRIBUTEID,
        VARIANT,
        windows_core::BOOL,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub FindText: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        windows_core::BOOL,
        windows_core::BOOL,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetAttributeValue: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TEXTATTRIBUTEID,
        *mut VARIANT,
    ) -> windows_core::HRESULT,
    pub GetBoundingRectangles: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut SAFEARRAY,
    ) -> windows_core::HRESULT,
    pub GetEnclosingElement: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetText: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub Move: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TextUnit,
        i32,
        *mut i32,
    ) -> windows_core::HRESULT,
    pub MoveEndpointByUnit: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TextPatternRangeEndpoint,
        TextUnit,
        i32,
        *mut i32,
    ) -> windows_core::HRESULT,
    pub MoveEndpointByRange: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TextPatternRangeEndpoint,
        *mut core::ffi::c_void,
        TextPatternRangeEndpoint,
    ) -> windows_core::HRESULT,
    pub Select: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub AddToSelection: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub RemoveFromSelection:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub ScrollIntoView: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub GetChildren: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut SAFEARRAY,
    ) -> windows_core::HRESULT,
}
pub trait ITextRangeProvider_Impl: windows_core::IUnknownImpl {
    fn Clone(&self) -> windows_core::Result<ITextRangeProvider>;
    fn Compare(
        &self,
        range: windows_core::Ref<ITextRangeProvider>,
    ) -> windows_core::Result<windows_core::BOOL>;
    fn CompareEndpoints(
        &self,
        endpoint: TextPatternRangeEndpoint,
        targetrange: windows_core::Ref<ITextRangeProvider>,
        targetendpoint: TextPatternRangeEndpoint,
    ) -> windows_core::Result<i32>;
    fn ExpandToEnclosingUnit(&self, unit: TextUnit) -> windows_core::Result<()>;
    fn FindAttribute(
        &self,
        attributeid: TEXTATTRIBUTEID,
        val: &VARIANT,
        backward: windows_core::BOOL,
    ) -> windows_core::Result<ITextRangeProvider>;
    fn FindText(
        &self,
        text: &windows_core::BSTR,
        backward: windows_core::BOOL,
        ignorecase: windows_core::BOOL,
    ) -> windows_core::Result<ITextRangeProvider>;
    fn GetAttributeValue(&self, attributeid: TEXTATTRIBUTEID) -> windows_core::Result<VARIANT>;
    fn GetBoundingRectangles(&self) -> windows_core::Result<*mut SAFEARRAY>;
    fn GetEnclosingElement(&self) -> windows_core::Result<IRawElementProviderSimple>;
    fn GetText(&self, maxlength: i32) -> windows_core::Result<windows_core::BSTR>;
    fn Move(&self, unit: TextUnit, count: i32) -> windows_core::Result<i32>;
    fn MoveEndpointByUnit(
        &self,
        endpoint: TextPatternRangeEndpoint,
        unit: TextUnit,
        count: i32,
    ) -> windows_core::Result<i32>;
    fn MoveEndpointByRange(
        &self,
        endpoint: TextPatternRangeEndpoint,
        targetrange: windows_core::Ref<ITextRangeProvider>,
        targetendpoint: TextPatternRangeEndpoint,
    ) -> windows_core::Result<()>;
    fn Select(&self) -> windows_core::Result<()>;
    fn AddToSelection(&self) -> windows_core::Result<()>;
    fn RemoveFromSelection(&self) -> windows_core::Result<()>;
    fn ScrollIntoView(&self, aligntotop: windows_core::BOOL) -> windows_core::Result<()>;
    fn GetChildren(&self) -> windows_core::Result<*mut SAFEARRAY>;
}
impl ITextRangeProvider_Vtbl {
    pub const fn new<Identity: ITextRangeProvider_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn Clone<Identity: ITextRangeProvider_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextRangeProvider_Impl::Clone(this) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn Compare<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            range: *mut core::ffi::c_void,
            pretval: *mut windows_core::BOOL,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextRangeProvider_Impl::Compare(this, core::mem::transmute_copy(&range)) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn CompareEndpoints<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            endpoint: TextPatternRangeEndpoint,
            targetrange: *mut core::ffi::c_void,
            targetendpoint: TextPatternRangeEndpoint,
            pretval: *mut i32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextRangeProvider_Impl::CompareEndpoints(
                    this,
                    core::mem::transmute_copy(&endpoint),
                    core::mem::transmute_copy(&targetrange),
                    core::mem::transmute_copy(&targetendpoint),
                ) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn ExpandToEnclosingUnit<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            unit: TextUnit,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextRangeProvider_Impl::ExpandToEnclosingUnit(
                    this,
                    core::mem::transmute_copy(&unit),
                )
                .into()
            }
        }
        unsafe extern "system" fn FindAttribute<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            attributeid: TEXTATTRIBUTEID,
            val: VARIANT,
            backward: windows_core::BOOL,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextRangeProvider_Impl::FindAttribute(
                    this,
                    core::mem::transmute_copy(&attributeid),
                    core::mem::transmute(&val),
                    core::mem::transmute_copy(&backward),
                ) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn FindText<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            text: *mut core::ffi::c_void,
            backward: windows_core::BOOL,
            ignorecase: windows_core::BOOL,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextRangeProvider_Impl::FindText(
                    this,
                    core::mem::transmute(&text),
                    core::mem::transmute_copy(&backward),
                    core::mem::transmute_copy(&ignorecase),
                ) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetAttributeValue<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            attributeid: TEXTATTRIBUTEID,
            pretval: *mut VARIANT,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextRangeProvider_Impl::GetAttributeValue(
                    this,
                    core::mem::transmute_copy(&attributeid),
                ) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetBoundingRectangles<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut SAFEARRAY,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextRangeProvider_Impl::GetBoundingRectangles(this) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetEnclosingElement<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextRangeProvider_Impl::GetEnclosingElement(this) {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn GetText<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            maxlength: i32,
            pretval: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextRangeProvider_Impl::GetText(this, core::mem::transmute_copy(&maxlength))
                {
                    Ok(ok__) => {
                        pretval.write(core::mem::transmute(ok__));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn Move<Identity: ITextRangeProvider_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
            unit: TextUnit,
            count: i32,
            pretval: *mut i32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextRangeProvider_Impl::Move(
                    this,
                    core::mem::transmute_copy(&unit),
                    core::mem::transmute_copy(&count),
                ) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn MoveEndpointByUnit<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            endpoint: TextPatternRangeEndpoint,
            unit: TextUnit,
            count: i32,
            pretval: *mut i32,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextRangeProvider_Impl::MoveEndpointByUnit(
                    this,
                    core::mem::transmute_copy(&endpoint),
                    core::mem::transmute_copy(&unit),
                    core::mem::transmute_copy(&count),
                ) {
                    Ok(ok__) => {
                        pretval.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn MoveEndpointByRange<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            endpoint: TextPatternRangeEndpoint,
            targetrange: *mut core::ffi::c_void,
            targetendpoint: TextPatternRangeEndpoint,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextRangeProvider_Impl::MoveEndpointByRange(
                    this,
                    core::mem::transmute_copy(&endpoint),
                    core::mem::transmute_copy(&targetrange),
                    core::mem::transmute_copy(&targetendpoint),
                )
                .into()
            }
        }
        unsafe extern "system" fn Select<Identity: ITextRangeProvider_Impl, const OFFSET: isize>(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextRangeProvider_Impl::Select(this).into()
            }
        }
        unsafe extern "system" fn AddToSelection<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextRangeProvider_Impl::AddToSelection(this).into()
            }
        }
        unsafe extern "system" fn RemoveFromSelection<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextRangeProvider_Impl::RemoveFromSelection(this).into()
            }
        }
        unsafe extern "system" fn ScrollIntoView<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            aligntotop: windows_core::BOOL,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITextRangeProvider_Impl::ScrollIntoView(
                    this,
                    core::mem::transmute_copy(&aligntotop),
                )
                .into()
            }
        }
        unsafe extern "system" fn GetChildren<
            Identity: ITextRangeProvider_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pretval: *mut *mut SAFEARRAY,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITextRangeProvider_Impl::GetChildren(this) {
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
            Clone: Clone::<Identity, OFFSET>,
            Compare: Compare::<Identity, OFFSET>,
            CompareEndpoints: CompareEndpoints::<Identity, OFFSET>,
            ExpandToEnclosingUnit: ExpandToEnclosingUnit::<Identity, OFFSET>,
            FindAttribute: FindAttribute::<Identity, OFFSET>,
            FindText: FindText::<Identity, OFFSET>,
            GetAttributeValue: GetAttributeValue::<Identity, OFFSET>,
            GetBoundingRectangles: GetBoundingRectangles::<Identity, OFFSET>,
            GetEnclosingElement: GetEnclosingElement::<Identity, OFFSET>,
            GetText: GetText::<Identity, OFFSET>,
            Move: Move::<Identity, OFFSET>,
            MoveEndpointByUnit: MoveEndpointByUnit::<Identity, OFFSET>,
            MoveEndpointByRange: MoveEndpointByRange::<Identity, OFFSET>,
            Select: Select::<Identity, OFFSET>,
            AddToSelection: AddToSelection::<Identity, OFFSET>,
            RemoveFromSelection: RemoveFromSelection::<Identity, OFFSET>,
            ScrollIntoView: ScrollIntoView::<Identity, OFFSET>,
            GetChildren: GetChildren::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<ITextRangeProvider as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for ITextRangeProvider {}
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
        *mut u16,
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
        *const u16,
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
        *const u16,
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
        *const TS_ATTRID,
    ) -> windows_core::HRESULT,
    pub RequestAttrsAtPosition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        u32,
        *const TS_ATTRID,
        u32,
    ) -> windows_core::HRESULT,
    pub RequestAttrsTransitioningAtPosition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        u32,
        *const TS_ATTRID,
        u32,
    )
        -> windows_core::HRESULT,
    pub FindNextAttrTransition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        i32,
        u32,
        *const TS_ATTRID,
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
    pub GetActiveView: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut TsViewCookie,
    ) -> windows_core::HRESULT,
    pub GetACPFromPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TsViewCookie,
        *const POINT,
        u32,
        *mut i32,
    ) -> windows_core::HRESULT,
    pub GetTextExt: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TsViewCookie,
        i32,
        i32,
        *mut RECT,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub GetScreenExt: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TsViewCookie,
        *mut RECT,
    ) -> windows_core::HRESULT,
    pub GetWnd: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TsViewCookie,
        *mut HWND,
    ) -> windows_core::HRESULT,
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
        pchplain: *mut u16,
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
        pchtext: *const u16,
        cch: u32,
    ) -> windows_core::Result<TS_TEXTCHANGE>;
    fn GetFormattedText(&self, acpstart: i32, acpend: i32) -> windows_core::Result<IDataObject>;
    fn GetEmbedded(
        &self,
        acppos: i32,
        rguidservice: *const windows_core::GUID,
        riid: *const windows_core::GUID,
        ppunk: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()>;
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
        pchtext: *const u16,
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
        pafilterattrs: *const TS_ATTRID,
    ) -> windows_core::Result<()>;
    fn RequestAttrsAtPosition(
        &self,
        acppos: i32,
        cfilterattrs: u32,
        pafilterattrs: *const TS_ATTRID,
        dwflags: u32,
    ) -> windows_core::Result<()>;
    fn RequestAttrsTransitioningAtPosition(
        &self,
        acppos: i32,
        cfilterattrs: u32,
        pafilterattrs: *const TS_ATTRID,
        dwflags: u32,
    ) -> windows_core::Result<()>;
    fn FindNextAttrTransition(
        &self,
        acpstart: i32,
        acphalt: i32,
        cfilterattrs: u32,
        pafilterattrs: *const TS_ATTRID,
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
    fn GetActiveView(&self) -> windows_core::Result<TsViewCookie>;
    fn GetACPFromPoint(
        &self,
        vcview: TsViewCookie,
        ptscreen: *const POINT,
        dwflags: u32,
    ) -> windows_core::Result<i32>;
    fn GetTextExt(
        &self,
        vcview: TsViewCookie,
        acpstart: i32,
        acpend: i32,
        prc: *mut RECT,
        pfclipped: *mut windows_core::BOOL,
    ) -> windows_core::Result<()>;
    fn GetScreenExt(&self, vcview: TsViewCookie) -> windows_core::Result<RECT>;
    fn GetWnd(&self, vcview: TsViewCookie) -> windows_core::Result<HWND>;
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
            pchplain: *mut u16,
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
            pchtext: *const u16,
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
                    core::mem::transmute_copy(&pchtext),
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
                ITextStoreACP_Impl::GetEmbedded(
                    this,
                    core::mem::transmute_copy(&acppos),
                    core::mem::transmute_copy(&rguidservice),
                    core::mem::transmute_copy(&riid),
                    core::mem::transmute_copy(&ppunk),
                )
                .into()
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
            pchtext: *const u16,
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
                    core::mem::transmute_copy(&pchtext),
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
            pafilterattrs: *const TS_ATTRID,
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
            pafilterattrs: *const TS_ATTRID,
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
            pafilterattrs: *const TS_ATTRID,
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
            pafilterattrs: *const TS_ATTRID,
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
            pvcview: *mut TsViewCookie,
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
            vcview: TsViewCookie,
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
            vcview: TsViewCookie,
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
            vcview: TsViewCookie,
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
            vcview: TsViewCookie,
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
        dwflags: u32,
        pchange: *const TS_TEXTCHANGE,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).OnTextChange)(
                windows_core::Interface::as_raw(self),
                dwflags,
                pchange,
            )
        }
    }
    pub(crate) unsafe fn OnSelectionChange(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).OnSelectionChange)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub(crate) unsafe fn OnLayoutChange(
        &self,
        lcode: TsLayoutCode,
        vcview: TsViewCookie,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).OnLayoutChange)(
                windows_core::Interface::as_raw(self),
                lcode,
                vcview,
            )
        }
    }
    pub(crate) unsafe fn OnStatusChange(&self, dwflags: u32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).OnStatusChange)(
                windows_core::Interface::as_raw(self),
                dwflags,
            )
        }
    }
    pub(crate) unsafe fn OnAttrsChange(
        &self,
        acpstart: i32,
        acpend: i32,
        cattrs: u32,
        paattrs: *const TS_ATTRID,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).OnAttrsChange)(
                windows_core::Interface::as_raw(self),
                acpstart,
                acpend,
                cattrs,
                paattrs,
            )
        }
    }
    pub(crate) unsafe fn OnLockGranted(&self, dwlockflags: u32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).OnLockGranted)(
                windows_core::Interface::as_raw(self),
                dwlockflags,
            )
        }
    }
    pub(crate) unsafe fn OnStartEditTransaction(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).OnStartEditTransaction)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub(crate) unsafe fn OnEndEditTransaction(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).OnEndEditTransaction)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
}
#[repr(C)]
pub struct ITextStoreACPSink_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub OnTextChange: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *const TS_TEXTCHANGE,
    ) -> windows_core::HRESULT,
    pub OnSelectionChange:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub OnLayoutChange: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TsLayoutCode,
        TsViewCookie,
    ) -> windows_core::HRESULT,
    pub OnStatusChange:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    pub OnAttrsChange: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        i32,
        u32,
        *const TS_ATTRID,
    ) -> windows_core::HRESULT,
    pub OnLockGranted:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    pub OnStartEditTransaction:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub OnEndEditTransaction:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ITfCompositionView,
    ITfCompositionView_Vtbl,
    0xd7540241_f9a1_4364_befc_dbcd2c4395b7
);
windows_core::imp::interface_hierarchy!(ITfCompositionView, windows_core::IUnknown);
impl ITfCompositionView {
    pub(crate) unsafe fn GetRange(&self) -> windows_core::Result<ITfRange> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetRange)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ITfCompositionView_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetOwnerClsid: usize,
    pub GetRange: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ITfContext,
    ITfContext_Vtbl,
    0xaa80e7fd_2021_11d2_93e0_0060b067b86e
);
windows_core::imp::interface_hierarchy!(ITfContext, windows_core::IUnknown);
impl ITfContext {
    pub(crate) unsafe fn RequestEditSession<P1>(
        &self,
        tid: TfClientId,
        pes: P1,
        dwflags: u32,
    ) -> windows_core::Result<windows_core::HRESULT>
    where
        P1: windows_core::Param<ITfEditSession>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).RequestEditSession)(
                windows_core::Interface::as_raw(self),
                tid,
                pes.param().abi(),
                dwflags,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn InWriteSession(
        &self,
        tid: TfClientId,
    ) -> windows_core::Result<windows_core::BOOL> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).InWriteSession)(
                windows_core::Interface::as_raw(self),
                tid,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn GetSelection(
        &self,
        ec: TfEditCookie,
        ulindex: u32,
        ulcount: u32,
        pselection: *mut TF_SELECTION,
        pcfetched: *mut u32,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetSelection)(
                windows_core::Interface::as_raw(self),
                ec,
                ulindex,
                ulcount,
                pselection,
                pcfetched as _,
            )
        }
    }
    pub(crate) unsafe fn SetSelection(
        &self,
        ec: TfEditCookie,
        ulcount: u32,
        pselection: *const TF_SELECTION,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetSelection)(
                windows_core::Interface::as_raw(self),
                ec,
                ulcount,
                pselection,
            )
        }
    }
    pub(crate) unsafe fn GetStart(&self, ec: TfEditCookie) -> windows_core::Result<ITfRange> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetStart)(
                windows_core::Interface::as_raw(self),
                ec,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn GetEnd(&self, ec: TfEditCookie) -> windows_core::Result<ITfRange> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetEnd)(
                windows_core::Interface::as_raw(self),
                ec,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn GetActiveView(&self) -> windows_core::Result<ITfContextView> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetActiveView)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn EnumViews(&self) -> windows_core::Result<IEnumTfContextViews> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).EnumViews)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn GetStatus(&self) -> windows_core::Result<TF_STATUS> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetStatus)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn GetProperty(
        &self,
        guidprop: *const windows_core::GUID,
    ) -> windows_core::Result<ITfProperty> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetProperty)(
                windows_core::Interface::as_raw(self),
                guidprop,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn GetAppProperty(
        &self,
        guidprop: *const windows_core::GUID,
    ) -> windows_core::Result<ITfReadOnlyProperty> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetAppProperty)(
                windows_core::Interface::as_raw(self),
                guidprop,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn TrackProperties(
        &self,
        prgprop: *const *const windows_core::GUID,
        cprop: u32,
        prgappprop: *const *const windows_core::GUID,
        cappprop: u32,
    ) -> windows_core::Result<ITfReadOnlyProperty> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TrackProperties)(
                windows_core::Interface::as_raw(self),
                prgprop,
                cprop,
                prgappprop,
                cappprop,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn EnumProperties(&self) -> windows_core::Result<IEnumTfProperties> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).EnumProperties)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn GetDocumentMgr(&self) -> windows_core::Result<ITfDocumentMgr> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetDocumentMgr)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn CreateRangeBackup<P1>(
        &self,
        ec: TfEditCookie,
        prange: P1,
    ) -> windows_core::Result<ITfRangeBackup>
    where
        P1: windows_core::Param<ITfRange>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateRangeBackup)(
                windows_core::Interface::as_raw(self),
                ec,
                prange.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ITfContext_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub RequestEditSession: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TfClientId,
        *mut core::ffi::c_void,
        u32,
        *mut windows_core::HRESULT,
    ) -> windows_core::HRESULT,
    pub InWriteSession: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TfClientId,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub GetSelection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TfEditCookie,
        u32,
        u32,
        *mut TF_SELECTION,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub SetSelection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TfEditCookie,
        u32,
        *const TF_SELECTION,
    ) -> windows_core::HRESULT,
    pub GetStart: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TfEditCookie,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetEnd: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TfEditCookie,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetActiveView: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub EnumViews: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetStatus:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut TF_STATUS) -> windows_core::HRESULT,
    pub GetProperty: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetAppProperty: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub TrackProperties: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const *const windows_core::GUID,
        u32,
        *const *const windows_core::GUID,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub EnumProperties: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetDocumentMgr: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateRangeBackup: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TfEditCookie,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ITfContextOwnerCompositionSink,
    ITfContextOwnerCompositionSink_Vtbl,
    0x5f20aa40_b57a_4f34_96ab_3576f377cc79
);
windows_core::imp::interface_hierarchy!(ITfContextOwnerCompositionSink, windows_core::IUnknown);
#[repr(C)]
pub struct ITfContextOwnerCompositionSink_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub OnStartComposition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub OnUpdateComposition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub OnEndComposition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
pub trait ITfContextOwnerCompositionSink_Impl: windows_core::IUnknownImpl {
    fn OnStartComposition(
        &self,
        pcomposition: windows_core::Ref<ITfCompositionView>,
    ) -> windows_core::Result<windows_core::BOOL>;
    fn OnUpdateComposition(
        &self,
        pcomposition: windows_core::Ref<ITfCompositionView>,
        prangenew: windows_core::Ref<ITfRange>,
    ) -> windows_core::Result<()>;
    fn OnEndComposition(
        &self,
        pcomposition: windows_core::Ref<ITfCompositionView>,
    ) -> windows_core::Result<()>;
}
impl ITfContextOwnerCompositionSink_Vtbl {
    pub const fn new<Identity: ITfContextOwnerCompositionSink_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn OnStartComposition<
            Identity: ITfContextOwnerCompositionSink_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pcomposition: *mut core::ffi::c_void,
            pfok: *mut windows_core::BOOL,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match ITfContextOwnerCompositionSink_Impl::OnStartComposition(
                    this,
                    core::mem::transmute_copy(&pcomposition),
                ) {
                    Ok(ok__) => {
                        pfok.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn OnUpdateComposition<
            Identity: ITfContextOwnerCompositionSink_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pcomposition: *mut core::ffi::c_void,
            prangenew: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITfContextOwnerCompositionSink_Impl::OnUpdateComposition(
                    this,
                    core::mem::transmute_copy(&pcomposition),
                    core::mem::transmute_copy(&prangenew),
                )
                .into()
            }
        }
        unsafe extern "system" fn OnEndComposition<
            Identity: ITfContextOwnerCompositionSink_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            pcomposition: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ITfContextOwnerCompositionSink_Impl::OnEndComposition(
                    this,
                    core::mem::transmute_copy(&pcomposition),
                )
                .into()
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            OnStartComposition: OnStartComposition::<Identity, OFFSET>,
            OnUpdateComposition: OnUpdateComposition::<Identity, OFFSET>,
            OnEndComposition: OnEndComposition::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<ITfContextOwnerCompositionSink as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for ITfContextOwnerCompositionSink {}
windows_core::imp::define_interface!(
    ITfContextView,
    ITfContextView_Vtbl,
    0x2433bf8e_0f9b_435c_ba2c_180611978c30
);
windows_core::imp::interface_hierarchy!(ITfContextView, windows_core::IUnknown);
#[repr(C)]
pub struct ITfContextView_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetRangeFromPoint: usize,
    GetTextExt: usize,
    GetScreenExt: usize,
    GetWnd: usize,
}
windows_core::imp::define_interface!(
    ITfDisplayAttributeInfo,
    ITfDisplayAttributeInfo_Vtbl,
    0x70528852_2f26_4aea_8c96_215150578932
);
windows_core::imp::interface_hierarchy!(ITfDisplayAttributeInfo, windows_core::IUnknown);
impl ITfDisplayAttributeInfo {
    pub(crate) unsafe fn GetGUID(&self) -> windows_core::Result<windows_core::GUID> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetGUID)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn GetDescription(&self) -> windows_core::Result<windows_core::BSTR> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetDescription)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| core::mem::transmute(result__))
        }
    }
    pub(crate) unsafe fn GetAttributeInfo(
        &self,
        pda: *mut TF_DISPLAYATTRIBUTE,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetAttributeInfo)(
                windows_core::Interface::as_raw(self),
                pda as _,
            )
        }
    }
    pub(crate) unsafe fn SetAttributeInfo(
        &self,
        pda: *const TF_DISPLAYATTRIBUTE,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetAttributeInfo)(
                windows_core::Interface::as_raw(self),
                pda,
            )
        }
    }
    pub(crate) unsafe fn Reset(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Reset)(windows_core::Interface::as_raw(self))
        }
    }
}
#[repr(C)]
pub struct ITfDisplayAttributeInfo_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetGUID: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::GUID,
    ) -> windows_core::HRESULT,
    pub GetDescription: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetAttributeInfo: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut TF_DISPLAYATTRIBUTE,
    ) -> windows_core::HRESULT,
    pub SetAttributeInfo: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const TF_DISPLAYATTRIBUTE,
    ) -> windows_core::HRESULT,
    pub Reset: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ITfDisplayAttributeMgr,
    ITfDisplayAttributeMgr_Vtbl,
    0x8ded7393_5db1_475c_9e71_a39111b0ff67
);
windows_core::imp::interface_hierarchy!(ITfDisplayAttributeMgr, windows_core::IUnknown);
impl ITfDisplayAttributeMgr {
    pub(crate) unsafe fn OnUpdateInfo(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).OnUpdateInfo)(windows_core::Interface::as_raw(
                self,
            ))
        }
    }
    pub(crate) unsafe fn EnumDisplayAttributeInfo(
        &self,
    ) -> windows_core::Result<IEnumTfDisplayAttributeInfo> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).EnumDisplayAttributeInfo)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn GetDisplayAttributeInfo(
        &self,
        guid: *const windows_core::GUID,
        ppinfo: *mut Option<ITfDisplayAttributeInfo>,
        pclsidowner: *mut windows_core::GUID,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetDisplayAttributeInfo)(
                windows_core::Interface::as_raw(self),
                guid,
                core::mem::transmute(ppinfo),
                pclsidowner as _,
            )
        }
    }
}
#[repr(C)]
pub struct ITfDisplayAttributeMgr_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub OnUpdateInfo: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub EnumDisplayAttributeInfo: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetDisplayAttributeInfo: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
        *mut windows_core::GUID,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ITfDisplayAttributeProvider,
    ITfDisplayAttributeProvider_Vtbl,
    0xfee47777_163c_4769_996a_6e9c50ad8f54
);
windows_core::imp::interface_hierarchy!(ITfDisplayAttributeProvider, windows_core::IUnknown);
impl ITfDisplayAttributeProvider {
    pub(crate) unsafe fn EnumDisplayAttributeInfo(
        &self,
    ) -> windows_core::Result<IEnumTfDisplayAttributeInfo> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).EnumDisplayAttributeInfo)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn GetDisplayAttributeInfo(
        &self,
        guid: *const windows_core::GUID,
    ) -> windows_core::Result<ITfDisplayAttributeInfo> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetDisplayAttributeInfo)(
                windows_core::Interface::as_raw(self),
                guid,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ITfDisplayAttributeProvider_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub EnumDisplayAttributeInfo: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetDisplayAttributeInfo: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
        tidowner: TfClientId,
        dwflags: u32,
        punk: P2,
        ppic: *mut Option<ITfContext>,
        pectextstore: *mut TfEditCookie,
    ) -> windows_core::HRESULT
    where
        P2: windows_core::Param<windows_core::IUnknown>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).CreateContext)(
                windows_core::Interface::as_raw(self),
                tidowner,
                dwflags,
                punk.param().abi(),
                core::mem::transmute(ppic),
                pectextstore as _,
            )
        }
    }
    pub(crate) unsafe fn Push<P0>(&self, pic: P0) -> windows_core::HRESULT
    where
        P0: windows_core::Param<ITfContext>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).Push)(
                windows_core::Interface::as_raw(self),
                pic.param().abi(),
            )
        }
    }
    pub(crate) unsafe fn Pop(&self, dwflags: u32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Pop)(
                windows_core::Interface::as_raw(self),
                dwflags,
            )
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
        TfClientId,
        u32,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
        *mut TfEditCookie,
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
    ITfEditSession,
    ITfEditSession_Vtbl,
    0xaa80e803_2021_11d2_93e0_0060b067b86e
);
windows_core::imp::interface_hierarchy!(ITfEditSession, windows_core::IUnknown);
#[repr(C)]
pub struct ITfEditSession_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    DoEditSession: usize,
}
windows_core::imp::define_interface!(
    ITfKeyEventSink,
    ITfKeyEventSink_Vtbl,
    0xaa80e7f5_2021_11d2_93e0_0060b067b86e
);
windows_core::imp::interface_hierarchy!(ITfKeyEventSink, windows_core::IUnknown);
#[repr(C)]
pub struct ITfKeyEventSink_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    OnSetFocus: usize,
    OnTestKeyDown: usize,
    OnTestKeyUp: usize,
    OnKeyDown: usize,
    OnKeyUp: usize,
    OnPreservedKey: usize,
}
windows_core::imp::define_interface!(
    ITfKeystrokeMgr,
    ITfKeystrokeMgr_Vtbl,
    0xaa80e7f0_2021_11d2_93e0_0060b067b86e
);
windows_core::imp::interface_hierarchy!(ITfKeystrokeMgr, windows_core::IUnknown);
impl ITfKeystrokeMgr {
    pub(crate) unsafe fn AdviseKeyEventSink<P1>(
        &self,
        tid: TfClientId,
        psink: P1,
        fforeground: bool,
    ) -> windows_core::HRESULT
    where
        P1: windows_core::Param<ITfKeyEventSink>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).AdviseKeyEventSink)(
                windows_core::Interface::as_raw(self),
                tid,
                psink.param().abi(),
                fforeground.into(),
            )
        }
    }
    pub(crate) unsafe fn UnadviseKeyEventSink(&self, tid: TfClientId) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).UnadviseKeyEventSink)(
                windows_core::Interface::as_raw(self),
                tid,
            )
        }
    }
    pub(crate) unsafe fn GetForeground(&self) -> windows_core::Result<windows_core::GUID> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetForeground)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn TestKeyDown(
        &self,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> windows_core::Result<windows_core::BOOL> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TestKeyDown)(
                windows_core::Interface::as_raw(self),
                wparam,
                lparam,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn TestKeyUp(
        &self,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> windows_core::Result<windows_core::BOOL> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TestKeyUp)(
                windows_core::Interface::as_raw(self),
                wparam,
                lparam,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn KeyDown(
        &self,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> windows_core::Result<windows_core::BOOL> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).KeyDown)(
                windows_core::Interface::as_raw(self),
                wparam,
                lparam,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn KeyUp(
        &self,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> windows_core::Result<windows_core::BOOL> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).KeyUp)(
                windows_core::Interface::as_raw(self),
                wparam,
                lparam,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn GetPreservedKey<P0>(
        &self,
        pic: P0,
        pprekey: *const TF_PRESERVEDKEY,
    ) -> windows_core::Result<windows_core::GUID>
    where
        P0: windows_core::Param<ITfContext>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetPreservedKey)(
                windows_core::Interface::as_raw(self),
                pic.param().abi(),
                pprekey,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn IsPreservedKey(
        &self,
        rguid: *const windows_core::GUID,
        pprekey: *const TF_PRESERVEDKEY,
    ) -> windows_core::Result<windows_core::BOOL> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsPreservedKey)(
                windows_core::Interface::as_raw(self),
                rguid,
                pprekey,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn PreserveKey(
        &self,
        tid: TfClientId,
        rguid: *const windows_core::GUID,
        prekey: *const TF_PRESERVEDKEY,
        pchdesc: *const u16,
        cchdesc: u32,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).PreserveKey)(
                windows_core::Interface::as_raw(self),
                tid,
                rguid,
                prekey,
                pchdesc,
                cchdesc,
            )
        }
    }
    pub(crate) unsafe fn UnpreserveKey(
        &self,
        rguid: *const windows_core::GUID,
        pprekey: *const TF_PRESERVEDKEY,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).UnpreserveKey)(
                windows_core::Interface::as_raw(self),
                rguid,
                pprekey,
            )
        }
    }
    pub(crate) unsafe fn SetPreservedKeyDescription(
        &self,
        rguid: *const windows_core::GUID,
        pchdesc: *const u16,
        cchdesc: u32,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetPreservedKeyDescription)(
                windows_core::Interface::as_raw(self),
                rguid,
                pchdesc,
                cchdesc,
            )
        }
    }
    pub(crate) unsafe fn GetPreservedKeyDescription(
        &self,
        rguid: *const windows_core::GUID,
    ) -> windows_core::Result<windows_core::BSTR> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetPreservedKeyDescription)(
                windows_core::Interface::as_raw(self),
                rguid,
                &mut result__,
            )
            .map(|| core::mem::transmute(result__))
        }
    }
    pub(crate) unsafe fn SimulatePreservedKey<P0>(
        &self,
        pic: P0,
        rguid: *const windows_core::GUID,
    ) -> windows_core::Result<windows_core::BOOL>
    where
        P0: windows_core::Param<ITfContext>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).SimulatePreservedKey)(
                windows_core::Interface::as_raw(self),
                pic.param().abi(),
                rguid,
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct ITfKeystrokeMgr_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub AdviseKeyEventSink: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TfClientId,
        *mut core::ffi::c_void,
        windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub UnadviseKeyEventSink:
        unsafe extern "system" fn(*mut core::ffi::c_void, TfClientId) -> windows_core::HRESULT,
    pub GetForeground: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::GUID,
    ) -> windows_core::HRESULT,
    pub TestKeyDown: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        WPARAM,
        LPARAM,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub TestKeyUp: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        WPARAM,
        LPARAM,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub KeyDown: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        WPARAM,
        LPARAM,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub KeyUp: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        WPARAM,
        LPARAM,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub GetPreservedKey: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *const TF_PRESERVEDKEY,
        *mut windows_core::GUID,
    ) -> windows_core::HRESULT,
    pub IsPreservedKey: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *const TF_PRESERVEDKEY,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub PreserveKey: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        TfClientId,
        *const windows_core::GUID,
        *const TF_PRESERVEDKEY,
        *const u16,
        u32,
    ) -> windows_core::HRESULT,
    pub UnpreserveKey: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *const TF_PRESERVEDKEY,
    ) -> windows_core::HRESULT,
    pub SetPreservedKeyDescription: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *const u16,
        u32,
    ) -> windows_core::HRESULT,
    pub GetPreservedKeyDescription: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SimulatePreservedKey: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ITfProperty,
    ITfProperty_Vtbl,
    0xe2449660_9542_11d2_bf46_00105a2799b5
);
impl core::ops::Deref for ITfProperty {
    type Target = ITfReadOnlyProperty;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ITfProperty, windows_core::IUnknown, ITfReadOnlyProperty);
#[repr(C)]
pub struct ITfProperty_Vtbl {
    pub base__: ITfReadOnlyProperty_Vtbl,
    FindRange: usize,
    SetValueStore: usize,
    SetValue: usize,
    Clear: usize,
}
windows_core::imp::define_interface!(
    ITfRange,
    ITfRange_Vtbl,
    0xaa80e7ff_2021_11d2_93e0_0060b067b86e
);
windows_core::imp::interface_hierarchy!(ITfRange, windows_core::IUnknown);
#[repr(C)]
pub struct ITfRange_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetText: usize,
    SetText: usize,
    GetFormattedText: usize,
    GetEmbedded: usize,
    InsertEmbedded: usize,
    ShiftStart: usize,
    ShiftEnd: usize,
    ShiftStartToRange: usize,
    ShiftEndToRange: usize,
    ShiftStartRegion: usize,
    ShiftEndRegion: usize,
    IsEmpty: usize,
    Collapse: usize,
    IsEqualStart: usize,
    IsEqualEnd: usize,
    CompareStart: usize,
    CompareEnd: usize,
    AdjustForInsert: usize,
    GetGravity: usize,
    SetGravity: usize,
    Clone: usize,
    GetContext: usize,
}
windows_core::imp::define_interface!(
    ITfRangeACP,
    ITfRangeACP_Vtbl,
    0x057a6296_029b_4154_b79a_0d461d4ea94c
);
impl core::ops::Deref for ITfRangeACP {
    type Target = ITfRange;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ITfRangeACP, windows_core::IUnknown, ITfRange);
impl ITfRangeACP {
    pub(crate) unsafe fn GetExtent(
        &self,
        pacpanchor: *mut i32,
        pcch: *mut i32,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetExtent)(
                windows_core::Interface::as_raw(self),
                pacpanchor as _,
                pcch as _,
            )
        }
    }
    pub(crate) unsafe fn SetExtent(&self, acpanchor: i32, cch: i32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SetExtent)(
                windows_core::Interface::as_raw(self),
                acpanchor,
                cch,
            )
        }
    }
}
#[repr(C)]
pub struct ITfRangeACP_Vtbl {
    pub base__: ITfRange_Vtbl,
    pub GetExtent: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut i32,
        *mut i32,
    ) -> windows_core::HRESULT,
    pub SetExtent:
        unsafe extern "system" fn(*mut core::ffi::c_void, i32, i32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ITfRangeBackup,
    ITfRangeBackup_Vtbl,
    0x463a506d_6992_49d2_9b88_93d55e70bb16
);
windows_core::imp::interface_hierarchy!(ITfRangeBackup, windows_core::IUnknown);
#[repr(C)]
pub struct ITfRangeBackup_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    Restore: usize,
}
windows_core::imp::define_interface!(
    ITfReadOnlyProperty,
    ITfReadOnlyProperty_Vtbl,
    0x17d49a3d_f8b8_4b2f_b254_52319dd64c53
);
windows_core::imp::interface_hierarchy!(ITfReadOnlyProperty, windows_core::IUnknown);
#[repr(C)]
pub struct ITfReadOnlyProperty_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetType: usize,
    EnumRanges: usize,
    GetValue: usize,
    GetContext: usize,
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
    pub(crate) unsafe fn UnadviseSink(&self, dwcookie: u32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).UnadviseSink)(
                windows_core::Interface::as_raw(self),
                dwcookie,
            )
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
    pub(crate) unsafe fn Activate(&self) -> windows_core::Result<TfClientId> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Activate)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn Deactivate(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Deactivate)(windows_core::Interface::as_raw(
                self,
            ))
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
    pub(crate) unsafe fn SetFocus<P0>(&self, pdimfocus: P0) -> windows_core::HRESULT
    where
        P0: windows_core::Param<ITfDocumentMgr>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetFocus)(
                windows_core::Interface::as_raw(self),
                pdimfocus.param().abi(),
            )
        }
    }
}
#[repr(C)]
pub struct ITfThreadMgr_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Activate:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut TfClientId) -> windows_core::HRESULT,
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
    pub(crate) unsafe fn Activate(&self) -> windows_core::Result<TfClientId> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Activate)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) unsafe fn Deactivate(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Deactivate)(windows_core::Interface::as_raw(
                self,
            ))
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
    pub(crate) unsafe fn SetFocus<P0>(&self, pdimfocus: P0) -> windows_core::HRESULT
    where
        P0: windows_core::Param<ITfDocumentMgr>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetFocus)(
                windows_core::Interface::as_raw(self),
                pdimfocus.param().abi(),
            )
        }
    }
}
#[repr(C)]
pub struct ITfThreadMgr2_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub Activate:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut TfClientId) -> windows_core::HRESULT,
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
    IVector2NaturalMotionAnimation,
    IVector2NaturalMotionAnimation_Vtbl,
    0x0f3e0b7d_e512_479d_a00c_77c93a30a395
);
impl windows_core::RuntimeType for IVector2NaturalMotionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IVector2NaturalMotionAnimation {
    pub(crate) fn SetFinalValue(
        &self,
        value: Option<windows_numerics::Vector2>,
    ) -> windows_core::Result<()> {
        let value__ =
            value.map(<windows_reference::IReference<windows_numerics::Vector2> as From<_>>::from);
        unsafe {
            (windows_core::Interface::vtable(self).SetFinalValue)(
                windows_core::Interface::as_raw(self),
                windows_core::Param::param(value__.as_ref()).abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IVector2NaturalMotionAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    FinalValue: usize,
    pub SetFinalValue: unsafe extern "system" fn(
        *mut core::ffi::c_void,
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
    IVector3NaturalMotionAnimation,
    IVector3NaturalMotionAnimation_Vtbl,
    0x9c17042c_e2ca_45ad_969e_4e78b7b9ad41
);
impl windows_core::RuntimeType for IVector3NaturalMotionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IVector3NaturalMotionAnimation {
    pub(crate) fn SetFinalValue(
        &self,
        value: Option<windows_numerics::Vector3>,
    ) -> windows_core::Result<()> {
        let value__ =
            value.map(<windows_reference::IReference<windows_numerics::Vector3> as From<_>>::from);
        unsafe {
            (windows_core::Interface::vtable(self).SetFinalValue)(
                windows_core::Interface::as_raw(self),
                windows_core::Param::param(value__.as_ref()).abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IVector3NaturalMotionAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    FinalValue: usize,
    pub SetFinalValue: unsafe extern "system" fn(
        *mut core::ffi::c_void,
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
    pub(crate) fn Parent(&self) -> windows_core::Result<ContainerVisual> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Parent)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
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
    pub Parent: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
pub struct ImplicitAnimationCollection(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ImplicitAnimationCollection,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(ImplicitAnimationCollection, CompositionObject);
impl windows_core::RuntimeType for ImplicitAnimationCollection {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IImplicitAnimationCollection>();
}
unsafe impl windows_core::Interface for ImplicitAnimationCollection {
    type Vtable = <IImplicitAnimationCollection as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IImplicitAnimationCollection as windows_core::Interface>::IID;
}
impl core::ops::Deref for ImplicitAnimationCollection {
    type Target = IImplicitAnimationCollection;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ImplicitAnimationCollection {
    const NAME: &'static str = "Windows.UI.Composition.ImplicitAnimationCollection";
}
unsafe impl Send for ImplicitAnimationCollection {}
unsafe impl Sync for ImplicitAnimationCollection {}
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
windows_core::imp::required_hierarchy!(
    KeyFrameAnimation,
    ICompositionAnimationBase,
    CompositionAnimation,
    CompositionObject
);
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MINMAXINFO {
    pub ptReserved: POINT,
    pub ptMaxSize: POINT,
    pub ptMaxPosition: POINT,
    pub ptMinTrackSize: POINT,
    pub ptMaxTrackSize: POINT,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MONITORINFO {
    pub cbSize: u32,
    pub rcMonitor: RECT,
    pub rcWork: RECT,
    pub dwFlags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MONITORINFOEXW {
    pub Base: MONITORINFO,
    pub szDevice: [u16; 32],
}
impl Default for MONITORINFOEXW {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub const MONITOR_DEFAULTTONEAREST: u32 = 2;
pub const MONITOR_DEFAULTTOPRIMARY: u32 = 1;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: u32,
    pub pt: POINT,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NCCALCSIZE_PARAMS {
    pub rgrc: [RECT; 3],
    pub lppos: PWINDOWPOS,
}
impl Default for NCCALCSIZE_PARAMS {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NaturalMotionAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    NaturalMotionAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    NaturalMotionAnimation,
    ICompositionAnimationBase,
    CompositionAnimation,
    CompositionObject
);
impl windows_core::RuntimeType for NaturalMotionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, INaturalMotionAnimation>();
}
unsafe impl windows_core::Interface for NaturalMotionAnimation {
    type Vtable = <INaturalMotionAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <INaturalMotionAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for NaturalMotionAnimation {
    type Target = INaturalMotionAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for NaturalMotionAnimation {
    const NAME: &'static str = "Windows.UI.Composition.NaturalMotionAnimation";
}
unsafe impl Send for NaturalMotionAnimation {}
unsafe impl Sync for NaturalMotionAnimation {}
pub type NavigateDirection = i32;
pub const NavigateDirection_FirstChild: NavigateDirection = 3;
pub const NavigateDirection_LastChild: NavigateDirection = 4;
pub const NavigateDirection_NextSibling: NavigateDirection = 1;
pub const NavigateDirection_Parent: NavigateDirection = 0;
pub const NavigateDirection_PreviousSibling: NavigateDirection = 2;
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
pub type PATTERNID = i32;
pub const PM_REMOVE: u32 = 1;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}
pub type POINTER_BUTTON_CHANGE_TYPE = i32;
pub type POINTER_FLAGS = u32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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
pub type POINTER_INPUT_TYPE = u32;
pub type PROPERTYID = i32;
pub type PTIMERAPCROUTINE = Option<
    unsafe extern "system" fn(
        lpargtocompletionroutine: *const core::ffi::c_void,
        dwtimerlowvalue: u32,
        dwtimerhighvalue: u32,
    ),
>;
pub type PWINDOWPOS = *mut WINDOWPOS;
pub type ProviderOptions = u32;
pub const ProviderOptions_ClientSideProvider: ProviderOptions = 1;
pub const ProviderOptions_HasNativeIAccessible: ProviderOptions = 128;
pub const ProviderOptions_NonClientAreaProvider: ProviderOptions = 4;
pub const ProviderOptions_OverrideProvider: ProviderOptions = 8;
pub const ProviderOptions_ProviderOwnsSetFocus: ProviderOptions = 16;
pub const ProviderOptions_RefuseNonClientSupport: ProviderOptions = 64;
pub const ProviderOptions_ServerSideProvider: ProviderOptions = 2;
pub const ProviderOptions_UseClientCoordinates: ProviderOptions = 256;
pub const ProviderOptions_UseComThreading: ProviderOptions = 32;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct REASON_CONTEXT {
    pub Version: u32,
    pub Flags: u32,
    pub Reason: REASON_CONTEXT_0,
}
impl Default for REASON_CONTEXT {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union REASON_CONTEXT_0 {
    pub Detailed: REASON_CONTEXT_0_0,
    pub SimpleReasonString: windows_core::PWSTR,
}
impl Default for REASON_CONTEXT_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct REASON_CONTEXT_0_0 {
    pub LocalizedReasonModule: HMODULE,
    pub LocalizedReasonId: u32,
    pub ReasonStringCount: u32,
    pub ReasonStrings: *mut windows_core::PWSTR,
}
impl Default for REASON_CONTEXT_0_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SAFEARRAY {
    pub cDims: u16,
    pub fFeatures: u16,
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SAFEARRAYBOUND {
    pub cElements: u32,
    pub lLbound: i32,
}
pub type SCODE = i32;
pub const SC_CLOSE: u32 = 61536;
pub const SC_MAXIMIZE: u32 = 61488;
pub const SC_MINIMIZE: u32 = 61472;
pub const SC_RESTORE: u32 = 61728;
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SECURITY_ATTRIBUTES {
    pub nLength: u32,
    pub lpSecurityDescriptor: *mut core::ffi::c_void,
    pub bInheritHandle: windows_core::BOOL,
}
impl Default for SECURITY_ATTRIBUTES {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SIZE {
    pub cx: i32,
    pub cy: i32,
}
pub const SIZE_MAXIMIZED: u32 = 2;
pub const SIZE_MINIMIZED: u32 = 1;
pub const SM_CXPADDEDBORDER: u32 = 92;
pub const SM_CYFRAME: u32 = 33;
pub const SWP_FRAMECHANGED: u32 = 32;
pub const SWP_NOACTIVATE: u32 = 16;
pub const SWP_NOMOVE: u32 = 2;
pub const SWP_NOSIZE: u32 = 1;
pub const SWP_NOZORDER: u32 = 4;
pub const SWP_SHOWWINDOW: u32 = 64;
pub const SW_HIDE: u32 = 0;
pub const SW_MAXIMIZE: u32 = 3;
pub const SW_MINIMIZE: u32 = 6;
pub const SW_RESTORE: u32 = 9;
pub const SW_SHOW: u32 = 5;
pub const SW_SHOWDEFAULT: u32 = 10;
pub const SW_SHOWNORMAL: u32 = 1;
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
    ICompositionAnimationBase,
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
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarNaturalMotionAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ScalarNaturalMotionAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    ScalarNaturalMotionAnimation,
    ICompositionAnimationBase,
    NaturalMotionAnimation,
    CompositionAnimation,
    CompositionObject
);
impl windows_core::RuntimeType for ScalarNaturalMotionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IScalarNaturalMotionAnimation>();
}
unsafe impl windows_core::Interface for ScalarNaturalMotionAnimation {
    type Vtable = <IScalarNaturalMotionAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IScalarNaturalMotionAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for ScalarNaturalMotionAnimation {
    type Target = IScalarNaturalMotionAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ScalarNaturalMotionAnimation {
    const NAME: &'static str = "Windows.UI.Composition.ScalarNaturalMotionAnimation";
}
unsafe impl Send for ScalarNaturalMotionAnimation {}
unsafe impl Sync for ScalarNaturalMotionAnimation {}
pub type ScrollAmount = i32;
pub const ScrollAmount_LargeDecrement: ScrollAmount = 0;
pub const ScrollAmount_LargeIncrement: ScrollAmount = 3;
pub const ScrollAmount_NoAmount: ScrollAmount = 2;
pub const ScrollAmount_SmallDecrement: ScrollAmount = 1;
pub const ScrollAmount_SmallIncrement: ScrollAmount = 4;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShapeVisual(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ShapeVisual,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(ShapeVisual, ContainerVisual, Visual, CompositionObject);
impl windows_core::RuntimeType for ShapeVisual {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IShapeVisual>();
}
unsafe impl windows_core::Interface for ShapeVisual {
    type Vtable = <IShapeVisual as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IShapeVisual as windows_core::Interface>::IID;
}
impl core::ops::Deref for ShapeVisual {
    type Target = IShapeVisual;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ShapeVisual {
    const NAME: &'static str = "Windows.UI.Composition.ShapeVisual";
}
unsafe impl Send for ShapeVisual {}
unsafe impl Sync for ShapeVisual {}
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
pub struct SpringScalarNaturalMotionAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    SpringScalarNaturalMotionAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    SpringScalarNaturalMotionAnimation,
    ICompositionAnimationBase,
    ScalarNaturalMotionAnimation,
    NaturalMotionAnimation,
    CompositionAnimation,
    CompositionObject
);
impl windows_core::RuntimeType for SpringScalarNaturalMotionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ISpringScalarNaturalMotionAnimation>();
}
unsafe impl windows_core::Interface for SpringScalarNaturalMotionAnimation {
    type Vtable = <ISpringScalarNaturalMotionAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <ISpringScalarNaturalMotionAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for SpringScalarNaturalMotionAnimation {
    type Target = ISpringScalarNaturalMotionAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for SpringScalarNaturalMotionAnimation {
    const NAME: &'static str = "Windows.UI.Composition.SpringScalarNaturalMotionAnimation";
}
unsafe impl Send for SpringScalarNaturalMotionAnimation {}
unsafe impl Sync for SpringScalarNaturalMotionAnimation {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpringVector2NaturalMotionAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    SpringVector2NaturalMotionAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    SpringVector2NaturalMotionAnimation,
    ICompositionAnimationBase,
    Vector2NaturalMotionAnimation,
    NaturalMotionAnimation,
    CompositionAnimation,
    CompositionObject
);
impl windows_core::RuntimeType for SpringVector2NaturalMotionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ISpringVector2NaturalMotionAnimation>();
}
unsafe impl windows_core::Interface for SpringVector2NaturalMotionAnimation {
    type Vtable = <ISpringVector2NaturalMotionAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <ISpringVector2NaturalMotionAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for SpringVector2NaturalMotionAnimation {
    type Target = ISpringVector2NaturalMotionAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for SpringVector2NaturalMotionAnimation {
    const NAME: &'static str = "Windows.UI.Composition.SpringVector2NaturalMotionAnimation";
}
unsafe impl Send for SpringVector2NaturalMotionAnimation {}
unsafe impl Sync for SpringVector2NaturalMotionAnimation {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpringVector3NaturalMotionAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    SpringVector3NaturalMotionAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    SpringVector3NaturalMotionAnimation,
    ICompositionAnimationBase,
    Vector3NaturalMotionAnimation,
    NaturalMotionAnimation,
    CompositionAnimation,
    CompositionObject
);
impl windows_core::RuntimeType for SpringVector3NaturalMotionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ISpringVector3NaturalMotionAnimation>();
}
unsafe impl windows_core::Interface for SpringVector3NaturalMotionAnimation {
    type Vtable = <ISpringVector3NaturalMotionAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <ISpringVector3NaturalMotionAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for SpringVector3NaturalMotionAnimation {
    type Target = ISpringVector3NaturalMotionAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for SpringVector3NaturalMotionAnimation {
    const NAME: &'static str = "Windows.UI.Composition.SpringVector3NaturalMotionAnimation";
}
unsafe impl Send for SpringVector3NaturalMotionAnimation {}
unsafe impl Sync for SpringVector3NaturalMotionAnimation {}
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
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepEasingFunction(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    StepEasingFunction,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    StepEasingFunction,
    CompositionEasingFunction,
    CompositionObject
);
impl windows_core::RuntimeType for StepEasingFunction {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IStepEasingFunction>();
}
unsafe impl windows_core::Interface for StepEasingFunction {
    type Vtable = <IStepEasingFunction as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IStepEasingFunction as windows_core::Interface>::IID;
}
impl core::ops::Deref for StepEasingFunction {
    type Target = IStepEasingFunction;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for StepEasingFunction {
    const NAME: &'static str = "Windows.UI.Composition.StepEasingFunction";
}
unsafe impl Send for StepEasingFunction {}
unsafe impl Sync for StepEasingFunction {}
pub type SupportedTextSelection = i32;
pub const SupportedTextSelection_Multiple: SupportedTextSelection = 2;
pub const SupportedTextSelection_None: SupportedTextSelection = 0;
pub const SupportedTextSelection_Single: SupportedTextSelection = 1;
pub type TEXTATTRIBUTEID = i32;
pub const TF_ATTR_CONVERTED: TF_DA_ATTR_INFO = 2;
pub const TF_ATTR_FIXEDCONVERTED: TF_DA_ATTR_INFO = 5;
pub const TF_ATTR_INPUT: TF_DA_ATTR_INFO = 0;
pub const TF_ATTR_INPUT_ERROR: TF_DA_ATTR_INFO = 4;
pub const TF_ATTR_OTHER: TF_DA_ATTR_INFO = -1;
pub const TF_ATTR_TARGET_CONVERTED: TF_DA_ATTR_INFO = 1;
pub const TF_ATTR_TARGET_NOTCONVERTED: TF_DA_ATTR_INFO = 3;
pub const TF_CT_COLORREF: TF_DA_COLORTYPE = 2;
pub const TF_CT_NONE: TF_DA_COLORTYPE = 0;
pub const TF_CT_SYSCOLOR: TF_DA_COLORTYPE = 1;
pub type TF_DA_ATTR_INFO = i32;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TF_DA_COLOR {
    pub r#type: TF_DA_COLORTYPE,
    pub Anonymous: TF_DA_COLOR_0,
}
impl Default for TF_DA_COLOR {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union TF_DA_COLOR_0 {
    pub nIndex: i32,
    pub cr: COLORREF,
}
impl Default for TF_DA_COLOR_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub type TF_DA_COLORTYPE = i32;
pub type TF_DA_LINESTYLE = i32;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TF_DISPLAYATTRIBUTE {
    pub crText: TF_DA_COLOR,
    pub crBk: TF_DA_COLOR,
    pub lsStyle: TF_DA_LINESTYLE,
    pub fBoldLine: windows_core::BOOL,
    pub crLine: TF_DA_COLOR,
    pub bAttr: TF_DA_ATTR_INFO,
}
impl Default for TF_DISPLAYATTRIBUTE {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub const TF_ES_ASYNCDONTCARE: u32 = 0;
pub const TF_ES_READ: u32 = 2;
pub const TF_ES_READWRITE: u32 = 6;
pub const TF_ES_SYNC: u32 = 1;
pub const TF_LS_DASH: TF_DA_LINESTYLE = 3;
pub const TF_LS_DOT: TF_DA_LINESTYLE = 2;
pub const TF_LS_NONE: TF_DA_LINESTYLE = 0;
pub const TF_LS_SOLID: TF_DA_LINESTYLE = 1;
pub const TF_LS_SQUIGGLE: TF_DA_LINESTYLE = 4;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TF_PRESERVEDKEY {
    pub uVKey: u32,
    pub uModifiers: u32,
}
#[repr(C)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TF_SELECTION {
    pub range: core::mem::ManuallyDrop<Option<ITfRange>>,
    pub style: TF_SELECTIONSTYLE,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TF_SELECTIONSTYLE {
    pub ase: TfActiveSelEnd,
    pub fInterimChar: windows_core::BOOL,
}
pub type TF_STATUS = TS_STATUS;
pub const TIMER_ALL_ACCESS: u32 = 2031619;
pub const TME_LEAVE: u32 = 2;
pub const TME_NONCLIENT: u32 = 16;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TRACKMOUSEEVENT {
    pub cbSize: u32,
    pub dwFlags: u32,
    pub hwndTrack: HWND,
    pub dwHoverTime: u32,
}
pub type TS_ATTRID = windows_core::GUID;
#[repr(C)]
pub struct TS_ATTRVAL {
    pub idAttr: TS_ATTRID,
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TS_RUNINFO {
    pub uCount: u32,
    pub r#type: TsRunType,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TS_SELECTIONSTYLE {
    pub ase: TsActiveSelEnd,
    pub fInterimChar: windows_core::BOOL,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TS_SELECTION_ACP {
    pub acpStart: i32,
    pub acpEnd: i32,
    pub style: TS_SELECTIONSTYLE,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TS_STATUS {
    pub dwDynamicFlags: u32,
    pub dwStaticFlags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TS_TEXTCHANGE {
    pub acpStart: i32,
    pub acpOldEnd: i32,
    pub acpNewEnd: i32,
}
pub type TextPatternRangeEndpoint = i32;
pub const TextPatternRangeEndpoint_End: TextPatternRangeEndpoint = 1;
pub const TextPatternRangeEndpoint_Start: TextPatternRangeEndpoint = 0;
pub type TextUnit = i32;
pub const TextUnit_Character: TextUnit = 0;
pub const TextUnit_Document: TextUnit = 6;
pub const TextUnit_Format: TextUnit = 1;
pub const TextUnit_Line: TextUnit = 3;
pub const TextUnit_Page: TextUnit = 5;
pub const TextUnit_Paragraph: TextUnit = 4;
pub const TextUnit_Word: TextUnit = 2;
pub type TfActiveSelEnd = i32;
pub type TfClientId = u32;
pub type TfEditCookie = u32;
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
pub const ToggleState_Indeterminate: ToggleState = 2;
pub const ToggleState_Off: ToggleState = 0;
pub const ToggleState_On: ToggleState = 1;
pub type TsActiveSelEnd = i32;
pub type TsLayoutCode = i32;
pub type TsRunType = i32;
pub type TsViewCookie = u32;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedEventHandler<TSender, TResult>(
    windows_core::IUnknown,
    core::marker::PhantomData<TSender>,
    core::marker::PhantomData<TResult>,
)
where
    TSender: windows_core::RuntimeType + 'static,
    TResult: windows_core::RuntimeType + 'static;
unsafe impl<
    TSender: windows_core::RuntimeType + 'static,
    TResult: windows_core::RuntimeType + 'static,
> windows_core::Interface for TypedEventHandler<TSender, TResult>
{
    type Vtable = TypedEventHandler_Vtbl<TSender, TResult>;
    const IID: windows_core::GUID =
        windows_core::GUID::from_signature(<Self as windows_core::RuntimeType>::SIGNATURE);
}
impl<TSender: windows_core::RuntimeType + 'static, TResult: windows_core::RuntimeType + 'static>
    windows_core::RuntimeType for TypedEventHandler<TSender, TResult>
{
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::new()
        .push_slice(b"pinterface({9de1c534-6ae1-11e0-84e1-18a905bcc53f}")
        .push_slice(b";")
        .push_other(TSender::SIGNATURE)
        .push_slice(b";")
        .push_other(TResult::SIGNATURE)
        .push_slice(b")");
}
#[repr(C)]
pub struct TypedEventHandler_Vtbl<TSender, TResult>
where
    TSender: windows_core::RuntimeType + 'static,
    TResult: windows_core::RuntimeType + 'static,
{
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: windows_core::AbiType<TSender>,
        args: windows_core::AbiType<TResult>,
    ) -> windows_core::HRESULT,
    TSender: core::marker::PhantomData<TSender>,
    TResult: core::marker::PhantomData<TResult>,
}
struct TypedEventHandlerBox<
    TSender,
    TResult,
    F: Fn(windows_core::Ref<TSender>, windows_core::Ref<TResult>) + 'static,
>(core::marker::PhantomData<(TSender, TResult, fn() -> F)>)
where
    TSender: windows_core::RuntimeType + 'static,
    TResult: windows_core::RuntimeType + 'static;
impl<
    TSender: windows_core::RuntimeType + 'static,
    TResult: windows_core::RuntimeType + 'static,
    F: Fn(windows_core::Ref<TSender>, windows_core::Ref<TResult>) + 'static,
> TypedEventHandlerBox<TSender, TResult, F>
{
    const VTABLE : TypedEventHandler_Vtbl < TSender , TResult , > = TypedEventHandler_Vtbl::< TSender , TResult , > { base__ : windows_core::IUnknown_Vtbl { QueryInterface : windows_core::imp::DelegateBox::< TypedEventHandler < TSender , TResult > , F >::QueryInterface , AddRef : windows_core::imp::DelegateBox::< TypedEventHandler < TSender , TResult > , F >::AddRef , Release : windows_core::imp::DelegateBox::< TypedEventHandler < TSender , TResult > , F >::Release , } , Invoke : Self::Invoke , TSender : core::marker::PhantomData::< TSender > , TResult : core::marker::PhantomData::< TResult > } ;
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: windows_core::AbiType<TSender>,
        args: windows_core::AbiType<TResult>,
    ) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<TypedEventHandler<TSender, TResult>, F>);
            (this.invoke)(
                core::mem::transmute_copy(&sender),
                core::mem::transmute_copy(&args),
            );
            windows_core::HRESULT(0)
        }
    }
}
pub const UIA_AutomationFocusChangedEventId: i32 = 20005;
pub const UIA_AutomationIdPropertyId: i32 = 30011;
pub const UIA_AutomationPropertyChangedEventId: i32 = 20004;
pub const UIA_BoundingRectanglePropertyId: i32 = 30001;
pub const UIA_ButtonControlTypeId: i32 = 50000;
pub const UIA_CheckBoxControlTypeId: i32 = 50002;
pub const UIA_ComboBoxControlTypeId: i32 = 50003;
pub const UIA_ControlTypePropertyId: i32 = 30003;
pub const UIA_CustomControlTypeId: i32 = 50025;
pub const UIA_EditControlTypeId: i32 = 50004;
pub const UIA_ExpandCollapseExpandCollapseStatePropertyId: i32 = 30070;
pub const UIA_ExpandCollapsePatternId: i32 = 10005;
pub const UIA_GroupControlTypeId: i32 = 50026;
pub const UIA_HasKeyboardFocusPropertyId: i32 = 30008;
pub const UIA_HelpTextPropertyId: i32 = 30013;
pub const UIA_HyperlinkControlTypeId: i32 = 50005;
pub const UIA_ImageControlTypeId: i32 = 50006;
pub const UIA_InvokePatternId: i32 = 10000;
pub const UIA_Invoke_InvokedEventId: i32 = 20009;
pub const UIA_IsContentElementPropertyId: i32 = 30017;
pub const UIA_IsControlElementPropertyId: i32 = 30016;
pub const UIA_IsEnabledPropertyId: i32 = 30010;
pub const UIA_IsKeyboardFocusablePropertyId: i32 = 30009;
pub const UIA_IsOffscreenPropertyId: i32 = 30022;
pub const UIA_IsPasswordPropertyId: i32 = 30019;
pub const UIA_IsTextPatternAvailablePropertyId: i32 = 30040;
pub const UIA_ListControlTypeId: i32 = 50008;
pub const UIA_ListItemControlTypeId: i32 = 50007;
pub const UIA_LiveRegionChangedEventId: i32 = 20024;
pub const UIA_LiveSettingPropertyId: i32 = 30135;
pub const UIA_MenuControlTypeId: i32 = 50009;
pub const UIA_MenuItemControlTypeId: i32 = 50011;
pub const UIA_NamePropertyId: i32 = 30005;
pub const UIA_PaneControlTypeId: i32 = 50033;
pub const UIA_ProgressBarControlTypeId: i32 = 50012;
pub const UIA_RadioButtonControlTypeId: i32 = 50013;
pub const UIA_RangeValuePatternId: i32 = 10003;
pub const UIA_RangeValueValuePropertyId: i32 = 30047;
pub const UIA_RuntimeIdPropertyId: i32 = 30000;
pub const UIA_ScrollBarControlTypeId: i32 = 50014;
pub const UIA_ScrollItemPatternId: i32 = 10017;
pub const UIA_ScrollPatternId: i32 = 10004;
pub const UIA_SelectionItemPatternId: i32 = 10010;
pub const UIA_SelectionItem_ElementSelectedEventId: i32 = 20012;
pub const UIA_SelectionPatternId: i32 = 10001;
pub const UIA_SliderControlTypeId: i32 = 50015;
pub const UIA_StatusBarControlTypeId: i32 = 50017;
pub const UIA_StructureChangedEventId: i32 = 20002;
pub const UIA_TabControlTypeId: i32 = 50018;
pub const UIA_TabItemControlTypeId: i32 = 50019;
pub const UIA_TextControlTypeId: i32 = 50020;
pub const UIA_TextPatternId: i32 = 10014;
pub const UIA_TitleBarControlTypeId: i32 = 50037;
pub const UIA_TogglePatternId: i32 = 10015;
pub const UIA_ToggleToggleStatePropertyId: i32 = 30086;
pub const UIA_ValuePatternId: i32 = 10002;
pub const UIA_ValueValuePropertyId: i32 = 30045;
pub const UIA_WindowControlTypeId: i32 = 50032;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiaPoint {
    pub x: f64,
    pub y: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiaRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}
#[repr(C)]
pub struct VARIANT {
    pub Anonymous: VARIANT_0,
}
impl Clone for VARIANT {
    fn clone(&self) -> Self {
        unsafe { core::mem::transmute_copy(self) }
    }
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
    pub vt: VARTYPE,
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
    pub scode: SCODE,
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
    pub pscode: *mut SCODE,
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
    pub pcVal: *mut i8,
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
#[derive(Clone, Debug, Eq, PartialEq)]
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
pub type VARTYPE = u16;
pub const VK_BACK: u32 = 8;
pub const VK_CONTROL: u32 = 17;
pub const VK_DELETE: u32 = 46;
pub const VK_DOWN: u32 = 40;
pub const VK_END: u32 = 35;
pub const VK_ESCAPE: u32 = 27;
pub const VK_HOME: u32 = 36;
pub const VK_LEFT: u32 = 37;
pub const VK_MENU: u32 = 18;
pub const VK_NEXT: u32 = 34;
pub const VK_PRIOR: u32 = 33;
pub const VK_RETURN: u32 = 13;
pub const VK_RIGHT: u32 = 39;
pub const VK_SHIFT: u32 = 16;
pub const VK_SPACE: u32 = 32;
pub const VK_TAB: u32 = 9;
pub const VK_UP: u32 = 38;
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
    ICompositionAnimationBase,
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
pub struct Vector2NaturalMotionAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    Vector2NaturalMotionAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    Vector2NaturalMotionAnimation,
    ICompositionAnimationBase,
    NaturalMotionAnimation,
    CompositionAnimation,
    CompositionObject
);
impl windows_core::RuntimeType for Vector2NaturalMotionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IVector2NaturalMotionAnimation>();
}
unsafe impl windows_core::Interface for Vector2NaturalMotionAnimation {
    type Vtable = <IVector2NaturalMotionAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IVector2NaturalMotionAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for Vector2NaturalMotionAnimation {
    type Target = IVector2NaturalMotionAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Vector2NaturalMotionAnimation {
    const NAME: &'static str = "Windows.UI.Composition.Vector2NaturalMotionAnimation";
}
unsafe impl Send for Vector2NaturalMotionAnimation {}
unsafe impl Sync for Vector2NaturalMotionAnimation {}
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
    ICompositionAnimationBase,
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
pub struct Vector3NaturalMotionAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    Vector3NaturalMotionAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    Vector3NaturalMotionAnimation,
    ICompositionAnimationBase,
    NaturalMotionAnimation,
    CompositionAnimation,
    CompositionObject
);
impl windows_core::RuntimeType for Vector3NaturalMotionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IVector3NaturalMotionAnimation>();
}
unsafe impl windows_core::Interface for Vector3NaturalMotionAnimation {
    type Vtable = <IVector3NaturalMotionAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IVector3NaturalMotionAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for Vector3NaturalMotionAnimation {
    type Target = IVector3NaturalMotionAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Vector3NaturalMotionAnimation {
    const NAME: &'static str = "Windows.UI.Composition.Vector3NaturalMotionAnimation";
}
unsafe impl Send for Vector3NaturalMotionAnimation {}
unsafe impl Sync for Vector3NaturalMotionAnimation {}
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
pub const WAIT_FAILED: u32 = 4294967295;
pub const WHEEL_DELTA: u32 = 120;
pub type WIN32_ERROR = u32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WINDOWPOS {
    pub hwnd: HWND,
    pub hwndInsertAfter: HWND,
    pub x: i32,
    pub y: i32,
    pub cx: i32,
    pub cy: i32,
    pub flags: u32,
}
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
pub const WM_NCLBUTTONDOWN: u32 = 161;
pub const WM_NCLBUTTONUP: u32 = 162;
pub const WM_NCMOUSELEAVE: u32 = 674;
pub const WM_NCMOUSEMOVE: u32 = 160;
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
pub const WM_SYSCOMMAND: u32 = 274;
pub const WM_SYSKEYDOWN: u32 = 260;
pub const WM_SYSKEYUP: u32 = 261;
pub const WM_WINDOWPOSCHANGED: u32 = 71;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: u32,
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
    pub style: u32,
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
pub type WNDPROC = Option<
    unsafe extern "system" fn(param0: HWND, param1: u32, param2: WPARAM, param3: LPARAM) -> LRESULT,
>;
pub type WPARAM = usize;
pub const WS_CAPTION: u32 = 12582912;
pub const WS_CHILD: u32 = 1073741824;
pub const WS_CLIPCHILDREN: u32 = 33554432;
pub const WS_CLIPSIBLINGS: u32 = 67108864;
pub const WS_EX_APPWINDOW: u32 = 262144;
pub const WS_EX_LAYERED: u32 = 524288;
pub const WS_EX_NOACTIVATE: u32 = 134217728;
pub const WS_EX_NOREDIRECTIONBITMAP: u32 = 2097152;
pub const WS_EX_TOOLWINDOW: u32 = 128;
pub const WS_EX_TOPMOST: u32 = 8;
pub const WS_MAXIMIZEBOX: u32 = 65536;
pub const WS_MINIMIZEBOX: u32 = 131072;
pub const WS_OVERLAPPED: u32 = 0;
pub const WS_OVERLAPPEDWINDOW: u32 = 13565952;
pub const WS_POPUP: u32 = 2147483648;
pub const WS_SYSMENU: u32 = 524288;
pub const WS_THICKFRAME: u32 = 262144;
pub const WS_VISIBLE: u32 = 268435456;
