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
windows_core::link!("user32.dll" "system" fn SystemParametersInfoW(uiaction : u32, uiparam : u32, pvparam : *mut core::ffi::c_void, fwinini : u32) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn TrackMouseEvent(lpeventtrack : *mut TRACKMOUSEEVENT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn TranslateMessage(lpmsg : *const MSG) -> windows_core::BOOL);
windows_core::link!("uiautomationcore.dll" "system" fn UiaDisconnectProvider(pprovider : *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaHostProviderFromHwnd(hwnd : HWND, ppprovider : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaRaiseAutomationEvent(pprovider : *mut core::ffi::c_void, id : EVENTID) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaRaiseAutomationPropertyChangedEvent(pprovider : *mut core::ffi::c_void, id : PROPERTYID, oldvalue : VARIANT, newvalue : VARIANT) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaRaiseStructureChangedEvent(pprovider : *mut core::ffi::c_void, structurechangetype : StructureChangeType, pruntimeid : *mut i32, cruntimeidlen : i32) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaReturnRawElementProvider(hwnd : HWND, wparam : WPARAM, lparam : LPARAM, el : *mut core::ffi::c_void) -> LRESULT);
windows_core::link!("user32.dll" "system" fn UnregisterClassW(lpclassname : windows_core::PCWSTR, hinstance : HINSTANCE) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn UpdateWindow(hwnd : HWND) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ValidateRect(hwnd : HWND, lprect : *const RECT) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn WaitForMultipleObjects(ncount : u32, lphandles : *const HANDLE, bwaitall : windows_core::BOOL, dwmilliseconds : u32) -> u32);
pub type ATOM = u16;
pub const CF_UNICODETEXT: u32 = 13;
pub type CLIPFORMAT = u16;
pub type COLORREF = u32;
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
pub const HTCAPTION: u32 = 2;
pub const HTCLIENT: u32 = 1;
pub const HTCLOSE: u32 = 20;
pub const HTMAXBUTTON: u32 = 9;
pub const HTMINBUTTON: u32 = 8;
pub const HTTOP: u32 = 12;
pub type HWND = *mut core::ffi::c_void;
pub const HeadingLevel_None: i32 = 80050;
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
}
#[repr(C)]
pub struct IDispatcherQueueController_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub DispatcherQueue: unsafe extern "system" fn(
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
    GetTop: usize,
    GetBase: usize,
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
}
#[repr(C)]
pub struct ITfRangeACP_Vtbl {
    pub base__: ITfRange_Vtbl,
    pub GetExtent: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut i32,
        *mut i32,
    ) -> windows_core::HRESULT,
    SetExtent: usize,
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
pub type LPARAM = isize;
pub type LRESULT = isize;
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
pub const SPI_GETCARETWIDTH: u32 = 8198;
pub const SPI_GETCLIENTAREAANIMATION: u32 = 4162;
pub const SWP_NOACTIVATE: u32 = 16;
pub const SWP_NOSIZE: u32 = 1;
pub const SWP_NOZORDER: u32 = 4;
pub const SW_SHOW: u32 = 5;
pub type ScrollAmount = i32;
pub const ScrollAmount_LargeDecrement: ScrollAmount = 0;
pub const ScrollAmount_LargeIncrement: ScrollAmount = 3;
pub const ScrollAmount_NoAmount: ScrollAmount = 2;
pub const ScrollAmount_SmallDecrement: ScrollAmount = 1;
pub const ScrollAmount_SmallIncrement: ScrollAmount = 4;
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
pub type StructureChangeType = i32;
pub const StructureChangeType_ChildAdded: StructureChangeType = 0;
pub const StructureChangeType_ChildRemoved: StructureChangeType = 1;
pub const StructureChangeType_ChildrenBulkAdded: StructureChangeType = 3;
pub const StructureChangeType_ChildrenBulkRemoved: StructureChangeType = 4;
pub const StructureChangeType_ChildrenInvalidated: StructureChangeType = 2;
pub const StructureChangeType_ChildrenReordered: StructureChangeType = 5;
pub type SupportedTextSelection = i32;
pub const SupportedTextSelection_Multiple: SupportedTextSelection = 2;
pub const SupportedTextSelection_None: SupportedTextSelection = 0;
pub const SupportedTextSelection_Single: SupportedTextSelection = 1;
pub type TEXTATTRIBUTEID = i32;
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
pub const UIA_AcceleratorKeyPropertyId: i32 = 30006;
pub const UIA_AutomationFocusChangedEventId: i32 = 20005;
pub const UIA_AutomationIdPropertyId: i32 = 30011;
pub const UIA_ButtonControlTypeId: i32 = 50000;
pub const UIA_CheckBoxControlTypeId: i32 = 50002;
pub const UIA_ComboBoxControlTypeId: i32 = 50003;
pub const UIA_ControlTypePropertyId: i32 = 30003;
pub const UIA_EditControlTypeId: i32 = 50004;
pub const UIA_ExpandCollapseExpandCollapseStatePropertyId: i32 = 30070;
pub const UIA_ExpandCollapsePatternId: i32 = 10005;
pub const UIA_GroupControlTypeId: i32 = 50026;
pub const UIA_HasKeyboardFocusPropertyId: i32 = 30008;
pub const UIA_HeadingLevelPropertyId: i32 = 30173;
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
pub const UIA_ListControlTypeId: i32 = 50008;
pub const UIA_ListItemControlTypeId: i32 = 50007;
pub const UIA_LiveRegionChangedEventId: i32 = 20024;
pub const UIA_LiveSettingPropertyId: i32 = 30135;
pub const UIA_LocalizedControlTypePropertyId: i32 = 30004;
pub const UIA_MenuClosedEventId: i32 = 20007;
pub const UIA_MenuControlTypeId: i32 = 50009;
pub const UIA_MenuItemControlTypeId: i32 = 50011;
pub const UIA_MenuOpenedEventId: i32 = 20003;
pub const UIA_NamePropertyId: i32 = 30005;
pub const UIA_PaneControlTypeId: i32 = 50033;
pub const UIA_PositionInSetPropertyId: i32 = 30152;
pub const UIA_ProgressBarControlTypeId: i32 = 50012;
pub const UIA_RadioButtonControlTypeId: i32 = 50013;
pub const UIA_RangeValuePatternId: i32 = 10003;
pub const UIA_RangeValueValuePropertyId: i32 = 30047;
pub const UIA_ScrollItemPatternId: i32 = 10017;
pub const UIA_ScrollPatternId: i32 = 10004;
pub const UIA_SelectionItemPatternId: i32 = 10010;
pub const UIA_SelectionItem_ElementSelectedEventId: i32 = 20012;
pub const UIA_SelectionPatternId: i32 = 10001;
pub const UIA_SeparatorControlTypeId: i32 = 50038;
pub const UIA_SizeOfSetPropertyId: i32 = 30153;
pub const UIA_SliderControlTypeId: i32 = 50015;
pub const UIA_StatusBarControlTypeId: i32 = 50017;
pub const UIA_TabControlTypeId: i32 = 50018;
pub const UIA_TabItemControlTypeId: i32 = 50019;
pub const UIA_TextControlTypeId: i32 = 50020;
pub const UIA_TextPatternId: i32 = 10014;
pub const UIA_TitleBarControlTypeId: i32 = 50037;
pub const UIA_TogglePatternId: i32 = 10015;
pub const UIA_ToggleToggleStatePropertyId: i32 = 30086;
pub const UIA_ValuePatternId: i32 = 10002;
pub const UIA_ValueValuePropertyId: i32 = 30045;
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
pub const WAIT_FAILED: u32 = 4294967295;
pub const WHEEL_DELTA: u32 = 120;
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
pub const WM_APP: u32 = 32768;
pub const WM_CAPTURECHANGED: u32 = 533;
pub const WM_CHAR: u32 = 258;
pub const WM_DESTROY: u32 = 2;
pub const WM_DISPLAYCHANGE: u32 = 126;
pub const WM_DPICHANGED: u32 = 736;
pub const WM_ERASEBKGND: u32 = 20;
pub const WM_GETOBJECT: u32 = 61;
pub const WM_KEYDOWN: u32 = 256;
pub const WM_KEYUP: u32 = 257;
pub const WM_KILLFOCUS: u32 = 8;
pub const WM_LBUTTONDOWN: u32 = 513;
pub const WM_LBUTTONUP: u32 = 514;
pub const WM_MOUSEHWHEEL: u32 = 526;
pub const WM_MOUSELEAVE: u32 = 675;
pub const WM_MOUSEMOVE: u32 = 512;
pub const WM_MOUSEWHEEL: u32 = 522;
pub const WM_NCCALCSIZE: u32 = 131;
pub const WM_NCHITTEST: u32 = 132;
pub const WM_NCLBUTTONDOWN: u32 = 161;
pub const WM_NCLBUTTONUP: u32 = 162;
pub const WM_NCMOUSELEAVE: u32 = 674;
pub const WM_NCMOUSEMOVE: u32 = 160;
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
pub const WS_EX_NOREDIRECTIONBITMAP: u32 = 2097152;
pub const WS_OVERLAPPEDWINDOW: u32 = 13565952;
