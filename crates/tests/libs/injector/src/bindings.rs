windows_core::link!("user32.dll" "system" fn ClientToScreen(hwnd : HWND, lppoint : *mut POINT) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn CloseHandle(hobject : HANDLE) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn CreateWaitableTimerExW(lptimerattributes : *const SECURITY_ATTRIBUTES, lptimername : windows_core::PCWSTR, dwflags : u32, dwdesiredaccess : u32) -> HANDLE);
windows_core::link!("dcomp.dll" "system" fn DCompositionWaitForCompositorClock(count : u32, handles : *const HANDLE, timeoutinms : u32) -> u32);
windows_core::link!("user32.dll" "system" fn DestroySyntheticPointerDevice(device : HSYNTHETICPOINTERDEVICE));
windows_core::link!("kernel32.dll" "system" fn GetCurrentPackageFullName(packagefullnamelength : *mut u32, packagefullname : windows_core::PWSTR) -> i32);
windows_core::link!("user32.dll" "system" fn GetCursorPos(lppoint : *mut POINT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetDpiForWindow(hwnd : HWND) -> u32);
windows_core::link!("kernel32.dll" "system" fn GetModuleHandleW(lpmodulename : windows_core::PCWSTR) -> HMODULE);
windows_core::link!("kernel32.dll" "system" fn GetProcAddress(hmodule : HMODULE, lpprocname : windows_core::PCSTR) -> FARPROC);
windows_core::link!("user32.dll" "system" fn GetSystemMetrics(nindex : i32) -> i32);
windows_core::link!("user32.dll" "system" fn InitializeTouchInjection(maxcount : u32, dwmode : u32) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn InjectSyntheticPointerInput(device : HSYNTHETICPOINTERDEVICE, pointerinfo : *const POINTER_TYPE_INFO, count : u32) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn InjectTouchInput(count : u32, contacts : *const POINTER_TOUCH_INFO) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn SetWaitableTimer(htimer : HANDLE, lpduetime : *const i64, lperiod : i32, pfncompletionroutine : PTIMERAPCROUTINE, lpargtocompletionroutine : *const core::ffi::c_void, fresume : windows_core::BOOL) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn WaitForSingleObject(hhandle : HANDLE, dwmilliseconds : u32) -> u32);
pub const APPMODEL_ERROR_NO_PACKAGE: i32 = 15700;
pub const CREATE_WAITABLE_TIMER_HIGH_RESOLUTION: i32 = 2;
pub const ERROR_NOT_READY: i32 = 21;
#[cfg(target_arch = "x86")]
pub type FARPROC = Option<unsafe extern "system" fn() -> i32>;
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "x86_64"
))]
pub type FARPROC = Option<unsafe extern "system" fn() -> isize>;
pub type HANDLE = *mut core::ffi::c_void;
pub type HINSTANCE = *mut core::ffi::c_void;
pub type HMODULE = HINSTANCE;
pub type HMONITOR = *mut core::ffi::c_void;
pub type HSYNTHETICPOINTERDEVICE = *mut core::ffi::c_void;
pub type HWND = *mut core::ffi::c_void;
windows_core::imp::define_interface!(
    IInjectedInputKeyboardInfo,
    IInjectedInputKeyboardInfo_Vtbl,
    0x4b46d140_2b6a_5ffa_7eae_bd077b052acd
);
impl windows_core::RuntimeType for IInjectedInputKeyboardInfo {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInjectedInputKeyboardInfo {
    pub(crate) fn SetKeyOptions(&self, value: InjectedInputKeyOptions) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetKeyOptions)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetVirtualKey(&self, value: u16) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetVirtualKey)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IInjectedInputKeyboardInfo_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    KeyOptions: usize,
    pub SetKeyOptions: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InjectedInputKeyOptions,
    ) -> windows_core::HRESULT,
    ScanCode: usize,
    SetScanCode: usize,
    VirtualKey: usize,
    pub SetVirtualKey:
        unsafe extern "system" fn(*mut core::ffi::c_void, u16) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInjectedInputMouseInfo,
    IInjectedInputMouseInfo_Vtbl,
    0x96f56e6b_e47a_5cf4_418d_8a5fb9670c7d
);
impl windows_core::RuntimeType for IInjectedInputMouseInfo {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInjectedInputMouseInfo {
    pub(crate) fn MouseOptions(&self) -> windows_core::Result<InjectedInputMouseOptions> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).MouseOptions)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn SetMouseOptions(
        &self,
        value: InjectedInputMouseOptions,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetMouseOptions)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetMouseData(&self, value: u32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetMouseData)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetDeltaY(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetDeltaY)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetDeltaX(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetDeltaX)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetTimeOffsetInMilliseconds(&self, value: u32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTimeOffsetInMilliseconds)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IInjectedInputMouseInfo_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub MouseOptions: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut InjectedInputMouseOptions,
    ) -> windows_core::HRESULT,
    pub SetMouseOptions: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InjectedInputMouseOptions,
    ) -> windows_core::HRESULT,
    MouseData: usize,
    pub SetMouseData:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    DeltaY: usize,
    pub SetDeltaY: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    DeltaX: usize,
    pub SetDeltaX: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    TimeOffsetInMilliseconds: usize,
    pub SetTimeOffsetInMilliseconds:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInjectedInputPenInfo,
    IInjectedInputPenInfo_Vtbl,
    0x6b40ad03_ca1e_5527_7e02_2828540bb1d4
);
impl windows_core::RuntimeType for IInjectedInputPenInfo {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInjectedInputPenInfo {
    pub(crate) fn SetPointerInfo(
        &self,
        value: InjectedInputPointerInfo,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPointerInfo)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetPenButtons(&self, value: InjectedInputPenButtons) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPenButtons)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetPenParameters(
        &self,
        value: InjectedInputPenParameters,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPenParameters)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetPressure(&self, value: f64) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPressure)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetRotation(&self, value: f64) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetRotation)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetTiltX(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTiltX)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetTiltY(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTiltY)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IInjectedInputPenInfo_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    PointerInfo: usize,
    pub SetPointerInfo: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InjectedInputPointerInfo,
    ) -> windows_core::HRESULT,
    PenButtons: usize,
    pub SetPenButtons: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InjectedInputPenButtons,
    ) -> windows_core::HRESULT,
    PenParameters: usize,
    pub SetPenParameters: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InjectedInputPenParameters,
    ) -> windows_core::HRESULT,
    Pressure: usize,
    pub SetPressure:
        unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    Rotation: usize,
    pub SetRotation:
        unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    TiltX: usize,
    pub SetTiltX: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    TiltY: usize,
    pub SetTiltY: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInjectedInputTouchInfo,
    IInjectedInputTouchInfo_Vtbl,
    0x224fd1df_43e8_5ef5_510a_69ca8c9b4c28
);
impl windows_core::RuntimeType for IInjectedInputTouchInfo {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInjectedInputTouchInfo {
    pub(crate) fn SetContact(&self, value: InjectedInputRectangle) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetContact)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetOrientation(&self, value: i32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetOrientation)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetPointerInfo(
        &self,
        value: InjectedInputPointerInfo,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPointerInfo)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetPressure(&self, value: f64) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPressure)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetTouchParameters(
        &self,
        value: InjectedInputTouchParameters,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTouchParameters)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IInjectedInputTouchInfo_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Contact: usize,
    pub SetContact: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InjectedInputRectangle,
    ) -> windows_core::HRESULT,
    Orientation: usize,
    pub SetOrientation:
        unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> windows_core::HRESULT,
    PointerInfo: usize,
    pub SetPointerInfo: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InjectedInputPointerInfo,
    ) -> windows_core::HRESULT,
    Pressure: usize,
    pub SetPressure:
        unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    TouchParameters: usize,
    pub SetTouchParameters: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InjectedInputTouchParameters,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInputInjector,
    IInputInjector_Vtbl,
    0x8ec26f84_0b02_4bd2_ad7a_3d4658be3e18
);
impl windows_core::RuntimeType for IInputInjector {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInputInjector {
    pub(crate) fn InjectKeyboardInput<P0>(&self, input: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_collections::IIterable<InjectedInputKeyboardInfo>>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InjectKeyboardInput)(
                windows_core::Interface::as_raw(self),
                input.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn InjectMouseInput<P0>(&self, input: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_collections::IIterable<InjectedInputMouseInfo>>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InjectMouseInput)(
                windows_core::Interface::as_raw(self),
                input.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn InitializeTouchInjection(
        &self,
        visualmode: InjectedInputVisualizationMode,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).InitializeTouchInjection)(
                windows_core::Interface::as_raw(self),
                visualmode,
            )
            .ok()
        }
    }
    pub(crate) fn InjectTouchInput<P0>(&self, input: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_collections::IIterable<InjectedInputTouchInfo>>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InjectTouchInput)(
                windows_core::Interface::as_raw(self),
                input.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn UninitializeTouchInjection(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).UninitializeTouchInjection)(
                windows_core::Interface::as_raw(self),
            )
            .ok()
        }
    }
    pub(crate) fn InitializePenInjection(
        &self,
        visualmode: InjectedInputVisualizationMode,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).InitializePenInjection)(
                windows_core::Interface::as_raw(self),
                visualmode,
            )
            .ok()
        }
    }
    pub(crate) fn InjectPenInput<P0>(&self, input: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<InjectedInputPenInfo>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InjectPenInput)(
                windows_core::Interface::as_raw(self),
                input.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn UninitializePenInjection(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).UninitializePenInjection)(
                windows_core::Interface::as_raw(self),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IInputInjector_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub InjectKeyboardInput: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub InjectMouseInput: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub InitializeTouchInjection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InjectedInputVisualizationMode,
    ) -> windows_core::HRESULT,
    pub InjectTouchInput: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub UninitializeTouchInjection:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub InitializePenInjection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InjectedInputVisualizationMode,
    ) -> windows_core::HRESULT,
    pub InjectPenInput: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub UninitializePenInjection:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInputInjectorStatics,
    IInputInjectorStatics_Vtbl,
    0xdeae6943_7402_4141_a5c6_0c01aa57b16a
);
impl windows_core::RuntimeType for IInputInjectorStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IInputInjectorStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub TryCreate: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
pub const INFINITE: u32 = 4294967295;
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InjectedInputKeyOptions(pub u32);
impl InjectedInputKeyOptions {
    pub const None: Self = Self(0);
    pub const ExtendedKey: Self = Self(1);
    pub const KeyUp: Self = Self(2);
    pub const ScanCode: Self = Self(8);
    pub const Unicode: Self = Self(4);
}
impl windows_core::TypeKind for InjectedInputKeyOptions {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InjectedInputKeyOptions {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Input.Preview.Injection.InjectedInputKeyOptions;u4)",
    );
}
impl InjectedInputKeyOptions {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for InjectedInputKeyOptions {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for InjectedInputKeyOptions {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for InjectedInputKeyOptions {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0);
    }
}
impl core::ops::BitAndAssign for InjectedInputKeyOptions {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0);
    }
}
impl core::ops::Not for InjectedInputKeyOptions {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InjectedInputKeyboardInfo(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InjectedInputKeyboardInfo,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl InjectedInputKeyboardInfo {
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
            InjectedInputKeyboardInfo,
            windows_core::imp::IGenericFactory,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for InjectedInputKeyboardInfo {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInjectedInputKeyboardInfo>();
}
unsafe impl windows_core::Interface for InjectedInputKeyboardInfo {
    type Vtable = <IInjectedInputKeyboardInfo as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IInjectedInputKeyboardInfo as windows_core::Interface>::IID;
}
impl core::ops::Deref for InjectedInputKeyboardInfo {
    type Target = IInjectedInputKeyboardInfo;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InjectedInputKeyboardInfo {
    const NAME: &'static str = "Windows.UI.Input.Preview.Injection.InjectedInputKeyboardInfo";
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InjectedInputMouseInfo(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InjectedInputMouseInfo,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl InjectedInputMouseInfo {
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
            InjectedInputMouseInfo,
            windows_core::imp::IGenericFactory,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for InjectedInputMouseInfo {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInjectedInputMouseInfo>();
}
unsafe impl windows_core::Interface for InjectedInputMouseInfo {
    type Vtable = <IInjectedInputMouseInfo as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IInjectedInputMouseInfo as windows_core::Interface>::IID;
}
impl core::ops::Deref for InjectedInputMouseInfo {
    type Target = IInjectedInputMouseInfo;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InjectedInputMouseInfo {
    const NAME: &'static str = "Windows.UI.Input.Preview.Injection.InjectedInputMouseInfo";
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InjectedInputMouseOptions(pub u32);
impl InjectedInputMouseOptions {
    pub const None: Self = Self(0);
    pub const Move: Self = Self(1);
    pub const LeftDown: Self = Self(2);
    pub const LeftUp: Self = Self(4);
    pub const RightDown: Self = Self(8);
    pub const RightUp: Self = Self(16);
    pub const MiddleDown: Self = Self(32);
    pub const MiddleUp: Self = Self(64);
    pub const XDown: Self = Self(128);
    pub const XUp: Self = Self(256);
    pub const Wheel: Self = Self(2048);
    pub const HWheel: Self = Self(4096);
    pub const MoveNoCoalesce: Self = Self(8192);
    pub const VirtualDesk: Self = Self(16384);
    pub const Absolute: Self = Self(32768);
}
impl windows_core::TypeKind for InjectedInputMouseOptions {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InjectedInputMouseOptions {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Input.Preview.Injection.InjectedInputMouseOptions;u4)",
    );
}
impl InjectedInputMouseOptions {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for InjectedInputMouseOptions {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for InjectedInputMouseOptions {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for InjectedInputMouseOptions {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0);
    }
}
impl core::ops::BitAndAssign for InjectedInputMouseOptions {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0);
    }
}
impl core::ops::Not for InjectedInputMouseOptions {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InjectedInputPenButtons(pub u32);
impl InjectedInputPenButtons {
    pub const None: Self = Self(0);
    pub const Barrel: Self = Self(1);
    pub const Inverted: Self = Self(2);
    pub const Eraser: Self = Self(4);
}
impl windows_core::TypeKind for InjectedInputPenButtons {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InjectedInputPenButtons {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Input.Preview.Injection.InjectedInputPenButtons;u4)",
    );
}
impl InjectedInputPenButtons {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for InjectedInputPenButtons {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for InjectedInputPenButtons {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for InjectedInputPenButtons {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0);
    }
}
impl core::ops::BitAndAssign for InjectedInputPenButtons {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0);
    }
}
impl core::ops::Not for InjectedInputPenButtons {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InjectedInputPenInfo(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InjectedInputPenInfo,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl InjectedInputPenInfo {
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
            InjectedInputPenInfo,
            windows_core::imp::IGenericFactory,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for InjectedInputPenInfo {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInjectedInputPenInfo>();
}
unsafe impl windows_core::Interface for InjectedInputPenInfo {
    type Vtable = <IInjectedInputPenInfo as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IInjectedInputPenInfo as windows_core::Interface>::IID;
}
impl core::ops::Deref for InjectedInputPenInfo {
    type Target = IInjectedInputPenInfo;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InjectedInputPenInfo {
    const NAME: &'static str = "Windows.UI.Input.Preview.Injection.InjectedInputPenInfo";
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InjectedInputPenParameters(pub u32);
impl InjectedInputPenParameters {
    pub const None: Self = Self(0);
    pub const Pressure: Self = Self(1);
    pub const Rotation: Self = Self(2);
    pub const TiltX: Self = Self(4);
    pub const TiltY: Self = Self(8);
}
impl windows_core::TypeKind for InjectedInputPenParameters {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InjectedInputPenParameters {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Input.Preview.Injection.InjectedInputPenParameters;u4)",
    );
}
impl InjectedInputPenParameters {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for InjectedInputPenParameters {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for InjectedInputPenParameters {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for InjectedInputPenParameters {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0);
    }
}
impl core::ops::BitAndAssign for InjectedInputPenParameters {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0);
    }
}
impl core::ops::Not for InjectedInputPenParameters {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InjectedInputPoint {
    pub position_x: i32,
    pub position_y: i32,
}
impl windows_core::TypeKind for InjectedInputPoint {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InjectedInputPoint {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"struct(Windows.UI.Input.Preview.Injection.InjectedInputPoint;i4;i4)",
    );
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InjectedInputPointerInfo {
    pub pointer_id: u32,
    pub pointer_options: InjectedInputPointerOptions,
    pub pixel_location: InjectedInputPoint,
    pub time_offset_in_milliseconds: u32,
    pub performance_count: u64,
}
impl windows_core::TypeKind for InjectedInputPointerInfo {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InjectedInputPointerInfo {
    const SIGNATURE : windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice (b"struct(Windows.UI.Input.Preview.Injection.InjectedInputPointerInfo;u4;enum(Windows.UI.Input.Preview.Injection.InjectedInputPointerOptions;u4);struct(Windows.UI.Input.Preview.Injection.InjectedInputPoint;i4;i4);u4;u8)") ;
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InjectedInputPointerOptions(pub u32);
impl InjectedInputPointerOptions {
    pub const None: Self = Self(0);
    pub const New: Self = Self(1);
    pub const InRange: Self = Self(2);
    pub const InContact: Self = Self(4);
    pub const FirstButton: Self = Self(16);
    pub const SecondButton: Self = Self(32);
    pub const Primary: Self = Self(8192);
    pub const Confidence: Self = Self(16384);
    pub const Canceled: Self = Self(32768);
    pub const PointerDown: Self = Self(65536);
    pub const Update: Self = Self(131072);
    pub const PointerUp: Self = Self(262144);
    pub const CaptureChanged: Self = Self(2097152);
}
impl windows_core::TypeKind for InjectedInputPointerOptions {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InjectedInputPointerOptions {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Input.Preview.Injection.InjectedInputPointerOptions;u4)",
    );
}
impl InjectedInputPointerOptions {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for InjectedInputPointerOptions {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for InjectedInputPointerOptions {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for InjectedInputPointerOptions {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0);
    }
}
impl core::ops::BitAndAssign for InjectedInputPointerOptions {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0);
    }
}
impl core::ops::Not for InjectedInputPointerOptions {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InjectedInputRectangle {
    pub left: i32,
    pub top: i32,
    pub bottom: i32,
    pub right: i32,
}
impl windows_core::TypeKind for InjectedInputRectangle {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InjectedInputRectangle {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"struct(Windows.UI.Input.Preview.Injection.InjectedInputRectangle;i4;i4;i4;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InjectedInputTouchInfo(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InjectedInputTouchInfo,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl InjectedInputTouchInfo {
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
            InjectedInputTouchInfo,
            windows_core::imp::IGenericFactory,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for InjectedInputTouchInfo {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInjectedInputTouchInfo>();
}
unsafe impl windows_core::Interface for InjectedInputTouchInfo {
    type Vtable = <IInjectedInputTouchInfo as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IInjectedInputTouchInfo as windows_core::Interface>::IID;
}
impl core::ops::Deref for InjectedInputTouchInfo {
    type Target = IInjectedInputTouchInfo;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InjectedInputTouchInfo {
    const NAME: &'static str = "Windows.UI.Input.Preview.Injection.InjectedInputTouchInfo";
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InjectedInputTouchParameters(pub u32);
impl InjectedInputTouchParameters {
    pub const None: Self = Self(0);
    pub const Contact: Self = Self(1);
    pub const Orientation: Self = Self(2);
    pub const Pressure: Self = Self(4);
}
impl windows_core::TypeKind for InjectedInputTouchParameters {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InjectedInputTouchParameters {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Input.Preview.Injection.InjectedInputTouchParameters;u4)",
    );
}
impl InjectedInputTouchParameters {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for InjectedInputTouchParameters {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for InjectedInputTouchParameters {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for InjectedInputTouchParameters {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0);
    }
}
impl core::ops::BitAndAssign for InjectedInputTouchParameters {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0);
    }
}
impl core::ops::Not for InjectedInputTouchParameters {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InjectedInputVisualizationMode(pub i32);
impl InjectedInputVisualizationMode {
    pub const None: Self = Self(0);
    pub const Default: Self = Self(1);
    pub const Indirect: Self = Self(2);
}
impl windows_core::TypeKind for InjectedInputVisualizationMode {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InjectedInputVisualizationMode {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Input.Preview.Injection.InjectedInputVisualizationMode;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputInjector(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InputInjector,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl InputInjector {
    pub(crate) fn TryCreate() -> windows_core::Result<Self> {
        Self::IInputInjectorStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).TryCreate)(
                windows_core::Interface::as_raw(this),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    fn IInputInjectorStatics<R, F: FnOnce(&IInputInjectorStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<InputInjector, IInputInjectorStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for InputInjector {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInputInjector>();
}
unsafe impl windows_core::Interface for InputInjector {
    type Vtable = <IInputInjector as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IInputInjector as windows_core::Interface>::IID;
}
impl core::ops::Deref for InputInjector {
    type Target = IInputInjector;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InputInjector {
    const NAME: &'static str = "Windows.UI.Input.Preview.Injection.InputInjector";
}
pub type PEN_FLAGS = u32;
pub type PEN_MASK = u32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}
pub type POINTER_BUTTON_CHANGE_TYPE = i32;
pub const POINTER_FEEDBACK_DEFAULT: POINTER_FEEDBACK_MODE = 1;
pub const POINTER_FEEDBACK_INDIRECT: POINTER_FEEDBACK_MODE = 2;
pub type POINTER_FEEDBACK_MODE = i32;
pub const POINTER_FEEDBACK_NONE: POINTER_FEEDBACK_MODE = 3;
pub type POINTER_FLAGS = u32;
pub const POINTER_FLAG_CANCELED: i32 = 32768;
pub const POINTER_FLAG_DOWN: i32 = 65536;
pub const POINTER_FLAG_INCONTACT: i32 = 4;
pub const POINTER_FLAG_INRANGE: i32 = 2;
pub const POINTER_FLAG_NONE: i32 = 0;
pub const POINTER_FLAG_UP: i32 = 262144;
pub const POINTER_FLAG_UPDATE: i32 = 131072;
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
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct POINTER_PEN_INFO {
    pub pointerInfo: POINTER_INFO,
    pub penFlags: PEN_FLAGS,
    pub penMask: PEN_MASK,
    pub pressure: u32,
    pub rotation: u32,
    pub tiltX: i32,
    pub tiltY: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct POINTER_TOUCH_INFO {
    pub pointerInfo: POINTER_INFO,
    pub touchFlags: TOUCH_FLAGS,
    pub touchMask: TOUCH_MASK,
    pub rcContact: RECT,
    pub rcContactRaw: RECT,
    pub orientation: u32,
    pub pressure: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct POINTER_TYPE_INFO {
    pub r#type: POINTER_INPUT_TYPE,
    pub Anonymous: POINTER_TYPE_INFO_0,
}
impl Default for POINTER_TYPE_INFO {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union POINTER_TYPE_INFO_0 {
    pub pointerInfo: POINTER_INFO,
    pub touchInfo: POINTER_TOUCH_INFO,
    pub penInfo: POINTER_PEN_INFO,
}
impl Default for POINTER_TYPE_INFO_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
pub type PTIMERAPCROUTINE = Option<
    unsafe extern "system" fn(
        lpargtocompletionroutine: *const core::ffi::c_void,
        dwtimerlowvalue: u32,
        dwtimerhighvalue: u32,
    ),
>;
pub const PT_TOUCH: tagPOINTER_INPUT_TYPE = 2;
pub const PT_TOUCHPAD: tagPOINTER_INPUT_TYPE = 5;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
pub const SDCO_NONE: SYNTHETIC_DEVICE_CREATION_OPTIONS = 0;
pub const SDCO_PHYSICAL_SIZE: SYNTHETIC_DEVICE_CREATION_OPTIONS = 1;
pub const SDCO_TOUCHPAD_GESTURE_ONLY: SYNTHETIC_DEVICE_CREATION_OPTIONS = 2;
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
pub const SM_CXVIRTUALSCREEN: i32 = 78;
pub const SM_CYVIRTUALSCREEN: i32 = 79;
pub const SM_XVIRTUALSCREEN: i32 = 76;
pub const SM_YVIRTUALSCREEN: i32 = 77;
pub type SYNTHETIC_DEVICE_CREATION_OPTIONS = u32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SYNTHETIC_DEVICE_CREATION_PARAMS {
    pub pointerType: POINTER_INPUT_TYPE,
    pub maxCount: u32,
    pub feedbackMode: POINTER_FEEDBACK_MODE,
    pub hMonitor: HMONITOR,
    pub deviceWidth: u32,
    pub deviceHeight: u32,
    pub options: SYNTHETIC_DEVICE_CREATION_OPTIONS,
}
pub const TA_3FINGER_PRESS: TOUCHPAD_ACTION = 1;
pub const TA_3FINGER_RELEASE: TOUCHPAD_ACTION = 2;
pub const TA_3FINGER_TAP: TOUCHPAD_ACTION = 0;
pub const TA_4FINGER_PRESS: TOUCHPAD_ACTION = 4;
pub const TA_4FINGER_RELEASE: TOUCHPAD_ACTION = 5;
pub const TA_4FINGER_TAP: TOUCHPAD_ACTION = 3;
pub const TA_5FINGER_PRESS: TOUCHPAD_ACTION = 7;
pub const TA_5FINGER_RELEASE: TOUCHPAD_ACTION = 8;
pub const TA_5FINGER_TAP: TOUCHPAD_ACTION = 6;
pub const TA_INERTIA_END: TOUCHPAD_ACTION = 10;
pub const TA_INERTIA_STOP: TOUCHPAD_ACTION = 9;
pub const TIMER_ALL_ACCESS: i32 = 2031619;
pub type TOUCHPAD_ACTION = i32;
pub const TOUCH_FEEDBACK_NONE: i32 = 3;
pub type TOUCH_FLAGS = u32;
pub const TOUCH_FLAG_NONE: i32 = 0;
pub type TOUCH_MASK = u32;
pub const TOUCH_MASK_CONTACTAREA: i32 = 1;
pub const TOUCH_MASK_ORIENTATION: i32 = 2;
pub const TOUCH_MASK_PRESSURE: i32 = 4;
pub const USER_DEFAULT_SCREEN_DPI: i32 = 96;
pub const VK_DOWN: i32 = 40;
pub const VK_ESCAPE: i32 = 27;
pub const VK_LEFT: i32 = 37;
pub const VK_RETURN: i32 = 13;
pub const VK_RIGHT: i32 = 39;
pub const VK_SPACE: i32 = 32;
pub const VK_TAB: i32 = 9;
pub const VK_UP: i32 = 38;
pub const WAIT_OBJECT_0: i32 = 0;
pub const WHEEL_DELTA: i32 = 120;
pub type tagPOINTER_INPUT_TYPE = i32;
