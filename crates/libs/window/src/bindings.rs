windows_core::link!("user32.dll" "system" fn BeginPaint(hwnd : HWND, lppaint : *mut PAINTSTRUCT) -> HDC);
windows_core::link!("user32.dll" "system" fn ClientToScreen(hwnd : HWND, lppoint : *mut POINT) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn CloseHandle(hobject : HANDLE) -> windows_core::BOOL);
windows_core::link!("coremessaging.dll" "system" fn CreateDispatcherQueueController(options : DispatcherQueueOptions, dispatcherqueuecontroller : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("kernel32.dll" "system" fn CreateEventW(lpeventattributes : *const SECURITY_ATTRIBUTES, bmanualreset : windows_core::BOOL, binitialstate : windows_core::BOOL, lpname : windows_core::PCWSTR) -> HANDLE);
windows_core::link!("kernel32.dll" "system" fn CreateWaitableTimerExW(lptimerattributes : *const SECURITY_ATTRIBUTES, lptimername : windows_core::PCWSTR, dwflags : u32, dwdesiredaccess : u32) -> HANDLE);
windows_core::link!("user32.dll" "system" fn CreateWindowExW(dwexstyle : u32, lpclassname : windows_core::PCWSTR, lpwindowname : windows_core::PCWSTR, dwstyle : u32, x : i32, y : i32, nwidth : i32, nheight : i32, hwndparent : HWND, hmenu : HMENU, hinstance : HINSTANCE, lpparam : *const core::ffi::c_void) -> HWND);
windows_core::link!("user32.dll" "system" fn DefWindowProcW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> LRESULT);
windows_core::link!("user32.dll" "system" fn DestroyWindow(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn DispatchMessageW(lpmsg : *const MSG) -> LRESULT);
windows_core::link!("dwmapi.dll" "system" fn DwmSetWindowAttribute(hwnd : HWND, dwattribute : u32, pvattribute : *const core::ffi::c_void, cbattribute : u32) -> windows_core::HRESULT);
windows_core::link!("user32.dll" "system" fn EnableMouseInPointer(fenable : windows_core::BOOL) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn EndPaint(hwnd : HWND, lppaint : *const PAINTSTRUCT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetClientRect(hwnd : HWND, lprect : *mut RECT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetDpiForWindow(hwnd : HWND) -> u32);
windows_core::link!("user32.dll" "system" fn GetMessageW(lpmsg : *mut MSG, hwnd : HWND, wmsgfiltermin : u32, wmsgfiltermax : u32) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn GetModuleHandleW(lpmodulename : windows_core::PCWSTR) -> HMODULE);
windows_core::link!("user32.dll" "system" fn GetMonitorInfoW(hmonitor : HMONITOR, lpmi : *mut MONITORINFO) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetSystemMenu(hwnd : HWND, brevert : windows_core::BOOL) -> HMENU);
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
windows_core::link!("user32.dll" "system" fn IsWindow(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn IsZoomed(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn LoadCursorW(hinstance : HINSTANCE, lpcursorname : windows_core::PCWSTR) -> HCURSOR);
windows_core::link!("user32.dll" "system" fn MonitorFromPoint(pt : POINT, dwflags : u32) -> HMONITOR);
windows_core::link!("user32.dll" "system" fn MonitorFromWindow(hwnd : HWND, dwflags : u32) -> HMONITOR);
windows_core::link!("user32.dll" "system" fn PeekMessageW(lpmsg : *mut MSG, hwnd : HWND, wmsgfiltermin : u32, wmsgfiltermax : u32, wremovemsg : u32) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn PostMessageW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn PostQuitMessage(nexitcode : i32));
windows_core::link!("user32.dll" "system" fn RegisterClassExW(param0 : *const WNDCLASSEXW) -> ATOM);
windows_core::link!("user32.dll" "system" fn RegisterClassW(lpwndclass : *const WNDCLASSW) -> ATOM);
windows_core::link!("user32.dll" "system" fn ScreenToClient(hwnd : HWND, lppoint : *mut POINT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn SendMessageW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> LRESULT);
windows_core::link!("user32.dll" "system" fn SetCursor(hcursor : HCURSOR) -> HCURSOR);
windows_core::link!("kernel32.dll" "system" fn SetEvent(hevent : HANDLE) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn SetProcessDpiAwarenessContext(value : DPI_AWARENESS_CONTEXT) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn SetProcessInformation(hprocess : HANDLE, processinformationclass : PROCESS_INFORMATION_CLASS, processinformation : *const core::ffi::c_void, processinformationsize : u32) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn SetThreadInformation(hthread : HANDLE, threadinformationclass : THREAD_INFORMATION_CLASS, threadinformation : *const core::ffi::c_void, threadinformationsize : u32) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn SetWaitableTimerEx(htimer : HANDLE, lpduetime : *const i64, lperiod : i32, pfncompletionroutine : PTIMERAPCROUTINE, lpargtocompletionroutine : *const core::ffi::c_void, wakecontext : *const REASON_CONTEXT, tolerabledelay : u32) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn SetWindowFeedbackSetting(hwnd : HWND, feedback : FEEDBACK_TYPE, dwflags : u32, size : u32, configuration : *const core::ffi::c_void) -> windows_core::BOOL);
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
windows_core::link!("user32.dll" "system" fn TrackPopupMenu(hmenu : HMENU, uflags : u32, x : i32, y : i32, nreserved : i32, hwnd : HWND, prcrect : *const RECT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn TranslateMessage(lpmsg : *const MSG) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn UnregisterClassW(lpclassname : windows_core::PCWSTR, hinstance : HINSTANCE) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn UpdateWindow(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ValidateRect(hwnd : HWND, lprect : *const RECT) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn WaitForMultipleObjects(ncount : u32, lphandles : *const HANDLE, bwaitall : windows_core::BOOL, dwmilliseconds : u32) -> u32);
pub type ATOM = u16;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdvancedColorInfo(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    AdvancedColorInfo,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for AdvancedColorInfo {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IAdvancedColorInfo>();
}
unsafe impl windows_core::Interface for AdvancedColorInfo {
    type Vtable = <IAdvancedColorInfo as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAdvancedColorInfo as windows_core::Interface>::IID;
}
impl core::ops::Deref for AdvancedColorInfo {
    type Target = IAdvancedColorInfo;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AdvancedColorInfo {
    const NAME: &'static str = "Windows.Graphics.Display.AdvancedColorInfo";
}
unsafe impl Send for AdvancedColorInfo {}
unsafe impl Sync for AdvancedColorInfo {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AdvancedColorKind(pub i32);
impl AdvancedColorKind {
    pub const StandardDynamicRange: Self = Self(0);
    pub const WideColorGamut: Self = Self(1);
    pub const HighDynamicRange: Self = Self(2);
}
impl windows_core::TypeKind for AdvancedColorKind {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for AdvancedColorKind {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.Graphics.Display.AdvancedColorKind;i4)",
    );
}
pub const CS_HREDRAW: i32 = 2;
pub const CS_VREDRAW: i32 = 1;
pub const CW_USEDEFAULT: i32 = -2147483648;
pub type DISPATCHERQUEUE_THREAD_APARTMENTTYPE = i32;
pub type DISPATCHERQUEUE_THREAD_TYPE = i32;
pub type DPI_AWARENESS_CONTEXT = *mut core::ffi::c_void;
pub const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: DPI_AWARENESS_CONTEXT = -4 as _;
pub const DQTAT_COM_ASTA: DISPATCHERQUEUE_THREAD_APARTMENTTYPE = 1;
pub const DQTAT_COM_NONE: DISPATCHERQUEUE_THREAD_APARTMENTTYPE = 0;
pub const DQTAT_COM_STA: DISPATCHERQUEUE_THREAD_APARTMENTTYPE = 2;
pub const DQTYPE_THREAD_CURRENT: DISPATCHERQUEUE_THREAD_TYPE = 2;
pub const DQTYPE_THREAD_DEDICATED: DISPATCHERQUEUE_THREAD_TYPE = 1;
pub const DWMWA_ALLOW_NCPAINT: DWMWINDOWATTRIBUTE = 4;
pub const DWMWA_BORDER_COLOR: DWMWINDOWATTRIBUTE = 34;
pub const DWMWA_BORDER_MARGINS: DWMWINDOWATTRIBUTE = 40;
pub const DWMWA_CAPTION_BUTTON_BOUNDS: DWMWINDOWATTRIBUTE = 5;
pub const DWMWA_CAPTION_COLOR: DWMWINDOWATTRIBUTE = 35;
pub const DWMWA_CLOAK: DWMWINDOWATTRIBUTE = 13;
pub const DWMWA_CLOAKED: DWMWINDOWATTRIBUTE = 14;
pub const DWMWA_COLOR_NONE: u32 = 4294967294;
pub const DWMWA_DISALLOW_PEEK: DWMWINDOWATTRIBUTE = 11;
pub const DWMWA_EXCLUDED_FROM_PEEK: DWMWINDOWATTRIBUTE = 12;
pub const DWMWA_EXTENDED_FRAME_BOUNDS: DWMWINDOWATTRIBUTE = 9;
pub const DWMWA_FLIP3D_POLICY: DWMWINDOWATTRIBUTE = 8;
pub const DWMWA_FORCE_ICONIC_REPRESENTATION: DWMWINDOWATTRIBUTE = 7;
pub const DWMWA_FREEZE_REPRESENTATION: DWMWINDOWATTRIBUTE = 15;
pub const DWMWA_HAS_ICONIC_BITMAP: DWMWINDOWATTRIBUTE = 10;
pub const DWMWA_LAST: DWMWINDOWATTRIBUTE = 41;
pub const DWMWA_NCRENDERING_ENABLED: DWMWINDOWATTRIBUTE = 1;
pub const DWMWA_NCRENDERING_POLICY: DWMWINDOWATTRIBUTE = 2;
pub const DWMWA_NONCLIENT_RTL_LAYOUT: DWMWINDOWATTRIBUTE = 6;
pub const DWMWA_PASSIVE_UPDATE_MODE: DWMWINDOWATTRIBUTE = 16;
pub const DWMWA_REDIRECTIONBITMAP_ALPHA: DWMWINDOWATTRIBUTE = 39;
pub const DWMWA_SYSTEMBACKDROP_TYPE: DWMWINDOWATTRIBUTE = 38;
pub const DWMWA_TEXT_COLOR: DWMWINDOWATTRIBUTE = 36;
pub const DWMWA_TRANSITIONS_FORCEDISABLED: DWMWINDOWATTRIBUTE = 3;
pub const DWMWA_USE_HOSTBACKDROPBRUSH: DWMWINDOWATTRIBUTE = 17;
pub const DWMWA_USE_IMMERSIVE_DARK_MODE: DWMWINDOWATTRIBUTE = 20;
pub const DWMWA_VISIBLE_FRAME_BORDER_THICKNESS: DWMWINDOWATTRIBUTE = 37;
pub const DWMWA_WINDOW_CORNER_PREFERENCE: DWMWINDOWATTRIBUTE = 33;
pub const DWMWCP_DEFAULT: DWM_WINDOW_CORNER_PREFERENCE = 0;
pub const DWMWCP_DONOTROUND: DWM_WINDOW_CORNER_PREFERENCE = 1;
pub const DWMWCP_ROUND: DWM_WINDOW_CORNER_PREFERENCE = 2;
pub const DWMWCP_ROUNDSMALL: DWM_WINDOW_CORNER_PREFERENCE = 3;
pub type DWMWINDOWATTRIBUTE = i32;
pub type DWM_WINDOW_CORNER_PREFERENCE = i32;
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
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayInformation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    DisplayInformation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for DisplayInformation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDisplayInformation>();
}
unsafe impl windows_core::Interface for DisplayInformation {
    type Vtable = <IDisplayInformation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDisplayInformation as windows_core::Interface>::IID;
}
impl core::ops::Deref for DisplayInformation {
    type Target = IDisplayInformation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DisplayInformation {
    const NAME: &'static str = "Windows.Graphics.Display.DisplayInformation";
}
unsafe impl Send for DisplayInformation {}
unsafe impl Sync for DisplayInformation {}
pub const FEEDBACK_GESTURE_PRESSANDTAP: FEEDBACK_TYPE = 11;
pub const FEEDBACK_MAX: FEEDBACK_TYPE = -1;
pub const FEEDBACK_PEN_BARRELVISUALIZATION: FEEDBACK_TYPE = 2;
pub const FEEDBACK_PEN_DOUBLETAP: FEEDBACK_TYPE = 4;
pub const FEEDBACK_PEN_PRESSANDHOLD: FEEDBACK_TYPE = 5;
pub const FEEDBACK_PEN_RIGHTTAP: FEEDBACK_TYPE = 6;
pub const FEEDBACK_PEN_TAP: FEEDBACK_TYPE = 3;
pub const FEEDBACK_TOUCH_CONTACTVISUALIZATION: FEEDBACK_TYPE = 1;
pub const FEEDBACK_TOUCH_DOUBLETAP: FEEDBACK_TYPE = 8;
pub const FEEDBACK_TOUCH_PRESSANDHOLD: FEEDBACK_TYPE = 9;
pub const FEEDBACK_TOUCH_RIGHTTAP: FEEDBACK_TYPE = 10;
pub const FEEDBACK_TOUCH_TAP: FEEDBACK_TYPE = 7;
pub type FEEDBACK_TYPE = i32;
pub const GWLP_USERDATA: i32 = -21;
pub type HANDLE = *mut core::ffi::c_void;
pub type HBRUSH = *mut core::ffi::c_void;
pub type HCURSOR = HICON;
pub type HDC = *mut core::ffi::c_void;
pub type HICON = *mut core::ffi::c_void;
pub type HINSTANCE = *mut core::ffi::c_void;
pub type HMENU = *mut core::ffi::c_void;
pub type HMODULE = HINSTANCE;
pub type HMONITOR = *mut core::ffi::c_void;
pub const HTCAPTION: i32 = 2;
pub const HTCLIENT: i32 = 1;
pub const HTCLOSE: i32 = 20;
pub const HTMAXBUTTON: i32 = 9;
pub const HTMINBUTTON: i32 = 8;
pub const HTTOP: i32 = 12;
pub type HWND = *mut core::ffi::c_void;
windows_core::imp::define_interface!(
    IAdvancedColorInfo,
    IAdvancedColorInfo_Vtbl,
    0x8797dcfb_b229_4081_ae9a_2cc85e34ad6a
);
impl windows_core::RuntimeType for IAdvancedColorInfo {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IAdvancedColorInfo_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
pub const IDC_ARROW: windows_core::PCWSTR = windows_core::PCWSTR(32512 as _);
windows_core::imp::define_interface!(
    IDispatcherQueue,
    IDispatcherQueue_Vtbl,
    0x603e88e4_a338_4ffe_a457_a5cfb9ceb899
);
impl windows_core::RuntimeType for IDispatcherQueue {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IDispatcherQueue_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
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
#[repr(C)]
pub struct IDispatcherQueueController_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IDisplayInformation,
    IDisplayInformation_Vtbl,
    0xbed112ae_adc3_4dc9_ae65_851f4d7d4799
);
impl windows_core::RuntimeType for IDisplayInformation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IDisplayInformation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IDisplayInformationStaticsInterop,
    IDisplayInformationStaticsInterop_Vtbl,
    0x7449121c_382b_4705_8da7_a795ba482013
);
windows_core::imp::interface_hierarchy!(
    IDisplayInformationStaticsInterop,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl IDisplayInformationStaticsInterop {
    pub(crate) unsafe fn GetForWindow<T>(&self, window: HWND) -> windows_core::Result<T>
    where
        T: windows_core::Interface,
    {
        let mut result__ = core::ptr::null_mut();
        unsafe {
            (windows_core::Interface::vtable(self).GetForWindow)(
                windows_core::Interface::as_raw(self),
                window,
                &T::IID,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) unsafe fn GetForMonitor<T>(&self, monitor: HMONITOR) -> windows_core::Result<T>
    where
        T: windows_core::Interface,
    {
        let mut result__ = core::ptr::null_mut();
        unsafe {
            (windows_core::Interface::vtable(self).GetForMonitor)(
                windows_core::Interface::as_raw(self),
                monitor,
                &T::IID,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IDisplayInformationStaticsInterop_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub GetForWindow: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        HWND,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetForMonitor: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        HMONITOR,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
pub trait IDisplayInformationStaticsInterop_Impl: windows_core::IUnknownImpl {
    fn GetForWindow(
        &self,
        window: HWND,
        riid: *const windows_core::GUID,
        displayinfo: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()>;
    fn GetForMonitor(
        &self,
        monitor: HMONITOR,
        riid: *const windows_core::GUID,
        displayinfo: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()>;
}
impl IDisplayInformationStaticsInterop_Vtbl {
    pub const fn new<Identity: IDisplayInformationStaticsInterop_Impl, const OFFSET: isize>() -> Self
    {
        unsafe extern "system" fn GetForWindow<
            Identity: IDisplayInformationStaticsInterop_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            window: HWND,
            riid: *const windows_core::GUID,
            displayinfo: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IDisplayInformationStaticsInterop_Impl::GetForWindow(
                    this,
                    core::mem::transmute_copy(&window),
                    core::mem::transmute_copy(&riid),
                    core::mem::transmute_copy(&displayinfo),
                )
                .into()
            }
        }
        unsafe extern "system" fn GetForMonitor<
            Identity: IDisplayInformationStaticsInterop_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            monitor: HMONITOR,
            riid: *const windows_core::GUID,
            displayinfo: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IDisplayInformationStaticsInterop_Impl::GetForMonitor(
                    this,
                    core::mem::transmute_copy(&monitor),
                    core::mem::transmute_copy(&riid),
                    core::mem::transmute_copy(&displayinfo),
                )
                .into()
            }
        }
        Self {
            base__: windows_core::IInspectable_Vtbl::new::<
                Identity,
                IDisplayInformationStaticsInterop,
                OFFSET,
            >(),
            GetForWindow: GetForWindow::<Identity, OFFSET>,
            GetForMonitor: GetForMonitor::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IDisplayInformationStaticsInterop as windows_core::Interface>::IID
    }
}
impl windows_core::RuntimeName for IDisplayInformationStaticsInterop {}
windows_core::imp::define_interface!(
    IUISettings,
    IUISettings_Vtbl,
    0x85361600_1c63_4627_bcb1_3a89e0bc9c55
);
impl windows_core::RuntimeType for IUISettings {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IUISettings_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
pub type LPARAM = isize;
pub type LRESULT = isize;
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
pub const MONITOR_DEFAULTTONEAREST: i32 = 2;
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
pub const PM_REMOVE: i32 = 1;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}
pub type PROCESS_INFORMATION_CLASS = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PROCESS_POWER_THROTTLING_STATE {
    pub Version: u32,
    pub ControlMask: u32,
    pub StateMask: u32,
}
pub type PTIMERAPCROUTINE = Option<
    unsafe extern "system" fn(
        lpargtocompletionroutine: *const core::ffi::c_void,
        dwtimerlowvalue: u32,
        dwtimerhighvalue: u32,
    ),
>;
pub type PWINDOWPOS = *mut WINDOWPOS;
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
pub const SC_CLOSE: i32 = 61536;
pub const SC_MAXIMIZE: i32 = 61488;
pub const SC_MINIMIZE: i32 = 61472;
pub const SC_RESTORE: i32 = 61728;
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
pub const SIZE_MAXIMIZED: i32 = 2;
pub const SIZE_MINIMIZED: i32 = 1;
pub const SM_CXPADDEDBORDER: i32 = 92;
pub const SM_CYFRAME: i32 = 33;
pub const SWP_FRAMECHANGED: i32 = 32;
pub const SWP_NOACTIVATE: i32 = 16;
pub const SWP_NOSIZE: i32 = 1;
pub const SWP_NOZORDER: i32 = 4;
pub const SW_SHOW: i32 = 5;
pub const SW_SHOWNOACTIVATE: i32 = 4;
pub const SW_SHOWNORMAL: i32 = 1;
pub type THREAD_INFORMATION_CLASS = i32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct THREAD_POWER_THROTTLING_STATE {
    pub Version: u32,
    pub ControlMask: u32,
    pub StateMask: u32,
}
pub const TIMER_ALL_ACCESS: i32 = 2031619;
pub const TPM_RETURNCMD: i32 = 256;
pub const TPM_RIGHTBUTTON: i32 = 2;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UISettings(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    UISettings,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for UISettings {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IUISettings>();
}
unsafe impl windows_core::Interface for UISettings {
    type Vtable = <IUISettings as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IUISettings as windows_core::Interface>::IID;
}
impl core::ops::Deref for UISettings {
    type Target = IUISettings;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for UISettings {
    const NAME: &'static str = "Windows.UI.ViewManagement.UISettings";
}
unsafe impl Send for UISettings {}
unsafe impl Sync for UISettings {}
pub const WAIT_FAILED: u32 = 4294967295;
pub const WAIT_OBJECT_0: i32 = 0;
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
pub const WM_APP: i32 = 32768;
pub const WM_DESTROY: i32 = 2;
pub const WM_DISPLAYCHANGE: i32 = 126;
pub const WM_DPICHANGED: i32 = 736;
pub const WM_ERASEBKGND: i32 = 20;
pub const WM_GETMINMAXINFO: i32 = 36;
pub const WM_NCCALCSIZE: i32 = 131;
pub const WM_NCDESTROY: i32 = 130;
pub const WM_NCHITTEST: i32 = 132;
pub const WM_NCLBUTTONDOWN: i32 = 161;
pub const WM_NCLBUTTONUP: i32 = 162;
pub const WM_NCMOUSELEAVE: i32 = 674;
pub const WM_NCMOUSEMOVE: i32 = 160;
pub const WM_QUIT: i32 = 18;
pub const WM_SETCURSOR: i32 = 32;
pub const WM_SETTINGCHANGE: i32 = 26;
pub const WM_SIZE: i32 = 5;
pub const WM_SYSCOMMAND: i32 = 274;
pub const WM_WINDOWPOSCHANGED: i32 = 71;
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
pub const WS_EX_NOREDIRECTIONBITMAP: i32 = 2097152;
pub const WS_OVERLAPPEDWINDOW: i32 = 13565952;
