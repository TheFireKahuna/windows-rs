windows_core::link!("user32.dll" "system" fn GetAsyncKeyState(vkey : i32) -> i16);
windows_core::link!("user32.dll" "system" fn GetCapture() -> HWND);
windows_core::link!("user32.dll" "system" fn GetClientRect(hwnd : HWND, lprect : *mut RECT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetKeyState(nvirtkey : i32) -> i16);
windows_core::link!("kernel32.dll" "system" fn GetModuleHandleW(lpmodulename : windows_core::PCWSTR) -> HMODULE);
windows_core::link!("user32.dll" "system" fn GetPointerFrameInfo(pointerid : u32, pointercount : *mut u32, pointerinfo : *mut POINTER_INFO) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetPointerFrameInfoHistory(pointerid : u32, entriescount : *mut u32, pointercount : *mut u32, pointerinfo : *mut POINTER_INFO) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetPointerInfo(pointerid : u32, pointerinfo : *mut POINTER_INFO) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetPointerInfoHistory(pointerid : u32, entriescount : *mut u32, pointerinfo : *mut POINTER_INFO) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetPointerInputTransform(pointerid : u32, historycount : u32, inputtransform : *mut INPUT_TRANSFORM) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetPointerPenInfo(pointerid : u32, peninfo : *mut POINTER_PEN_INFO) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetPointerPenInfoHistory(pointerid : u32, entriescount : *mut u32, peninfo : *mut POINTER_PEN_INFO) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetPointerTouchInfo(pointerid : u32, touchinfo : *mut POINTER_TOUCH_INFO) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetPointerTouchInfoHistory(pointerid : u32, entriescount : *mut u32, touchinfo : *mut POINTER_TOUCH_INFO) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn GetPointerType(pointerid : u32, pointertype : *mut POINTER_INPUT_TYPE) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn GetProcAddress(hmodule : HMODULE, lpprocname : windows_core::PCSTR) -> FARPROC);
windows_core::link!("user32.dll" "system" fn PostMessageW(hwnd : HWND, msg : u32, wparam : WPARAM, lparam : LPARAM) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ReleaseCapture() -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn ScreenToClient(hwnd : HWND, lppoint : *mut POINT) -> windows_core::BOOL);
windows_core::link!("user32.dll" "system" fn SetCapture(hwnd : HWND) -> HWND);
windows_core::link!("user32.dll" "system" fn SkipPointerFrameMessages(pointerid : u32) -> windows_core::BOOL);
windows_core::link!("uiautomationcore.dll" "system" fn UiaClientsAreListening() -> windows_core::BOOL);
windows_core::link!("uiautomationcore.dll" "system" fn UiaDisconnectProvider(pprovider : *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaHostProviderFromHwnd(hwnd : HWND, ppprovider : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaRaiseAutomationEvent(pprovider : *mut core::ffi::c_void, id : EVENTID) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaRaiseAutomationPropertyChangedEvent(pprovider : *mut core::ffi::c_void, id : PROPERTYID, oldvalue : VARIANT, newvalue : VARIANT) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaRaiseStructureChangedEvent(pprovider : *mut core::ffi::c_void, structurechangetype : StructureChangeType, pruntimeid : *mut i32, cruntimeidlen : i32) -> windows_core::HRESULT);
windows_core::link!("uiautomationcore.dll" "system" fn UiaReturnRawElementProvider(hwnd : HWND, wparam : WPARAM, lparam : LPARAM, el : *mut core::ffi::c_void) -> LRESULT);
pub type CLIPFORMAT = u16;
pub type COLORREF = u32;
pub type CONTROLTYPEID = i32;
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
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreFrameworkInputView(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CoreFrameworkInputView,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for CoreFrameworkInputView {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICoreFrameworkInputView>();
}
unsafe impl windows_core::Interface for CoreFrameworkInputView {
    type Vtable = <ICoreFrameworkInputView as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICoreFrameworkInputView as windows_core::Interface>::IID;
}
impl core::ops::Deref for CoreFrameworkInputView {
    type Target = ICoreFrameworkInputView;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CoreFrameworkInputView {
    const NAME: &'static str = "Windows.UI.ViewManagement.Core.CoreFrameworkInputView";
}
unsafe impl Send for CoreFrameworkInputView {}
unsafe impl Sync for CoreFrameworkInputView {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreInputView(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CoreInputView,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for CoreInputView {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICoreInputView>();
}
unsafe impl windows_core::Interface for CoreInputView {
    type Vtable = <ICoreInputView as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICoreInputView as windows_core::Interface>::IID;
}
impl core::ops::Deref for CoreInputView {
    type Target = ICoreInputView;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CoreInputView {
    const NAME: &'static str = "Windows.UI.ViewManagement.Core.CoreInputView";
}
unsafe impl Send for CoreInputView {}
unsafe impl Sync for CoreInputView {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CoreInputViewKind(pub i32);
impl CoreInputViewKind {
    pub const Default: Self = Self(0);
    pub const Keyboard: Self = Self(1);
    pub const Handwriting: Self = Self(2);
    pub const Emoji: Self = Self(3);
    pub const Symbols: Self = Self(4);
    pub const Clipboard: Self = Self(5);
    pub const Dictation: Self = Self(6);
    pub const Gamepad: Self = Self(7);
}
impl windows_core::TypeKind for CoreInputViewKind {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for CoreInputViewKind {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.ViewManagement.Core.CoreInputViewKind;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreInputViewOcclusion(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CoreInputViewOcclusion,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for CoreInputViewOcclusion {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICoreInputViewOcclusion>();
}
unsafe impl windows_core::Interface for CoreInputViewOcclusion {
    type Vtable = <ICoreInputViewOcclusion as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICoreInputViewOcclusion as windows_core::Interface>::IID;
}
impl core::ops::Deref for CoreInputViewOcclusion {
    type Target = ICoreInputViewOcclusion;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CoreInputViewOcclusion {
    const NAME: &'static str = "Windows.UI.ViewManagement.Core.CoreInputViewOcclusion";
}
unsafe impl Send for CoreInputViewOcclusion {}
unsafe impl Sync for CoreInputViewOcclusion {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CoreInputViewOcclusionKind(pub i32);
impl CoreInputViewOcclusionKind {
    pub const Docked: Self = Self(0);
    pub const Floating: Self = Self(1);
    pub const Overlay: Self = Self(2);
}
impl windows_core::TypeKind for CoreInputViewOcclusionKind {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for CoreInputViewOcclusionKind {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.ViewManagement.Core.CoreInputViewOcclusionKind;i4)",
    );
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
pub struct DraggingEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    DraggingEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for DraggingEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDraggingEventArgs>();
}
unsafe impl windows_core::Interface for DraggingEventArgs {
    type Vtable = <IDraggingEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDraggingEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for DraggingEventArgs {
    type Target = IDraggingEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DraggingEventArgs {
    const NAME: &'static str = "Windows.UI.Input.DraggingEventArgs";
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DraggingState(pub i32);
impl DraggingState {
    pub const Started: Self = Self(0);
    pub const Continuing: Self = Self(1);
    pub const Completed: Self = Self(2);
}
impl windows_core::TypeKind for DraggingState {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for DraggingState {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Windows.UI.Input.DraggingState;i4)");
}
pub type EVENTID = i32;
pub type ExpandCollapseState = i32;
pub const ExpandCollapseState_Collapsed: ExpandCollapseState = 0;
pub const ExpandCollapseState_Expanded: ExpandCollapseState = 1;
pub const ExpandCollapseState_LeafNode: ExpandCollapseState = 3;
pub const ExpandCollapseState_PartiallyExpanded: ExpandCollapseState = 2;
#[cfg(target_arch = "x86")]
pub type FARPROC = Option<unsafe extern "system" fn() -> i32>;
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "x86_64"
))]
pub type FARPROC = Option<unsafe extern "system" fn() -> isize>;
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
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GestureRecognizer(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    GestureRecognizer,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl GestureRecognizer {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<
        R,
        F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<
            GestureRecognizer,
            windows_core::imp::IGenericFactory,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for GestureRecognizer {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IGestureRecognizer>();
}
unsafe impl windows_core::Interface for GestureRecognizer {
    type Vtable = <IGestureRecognizer as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IGestureRecognizer as windows_core::Interface>::IID;
}
impl core::ops::Deref for GestureRecognizer {
    type Target = IGestureRecognizer;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for GestureRecognizer {
    const NAME: &'static str = "Windows.UI.Input.GestureRecognizer";
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GestureSettings(pub u32);
impl GestureSettings {
    pub const None: Self = Self(0);
    pub const Tap: Self = Self(1);
    pub const DoubleTap: Self = Self(2);
    pub const Hold: Self = Self(4);
    pub const HoldWithMouse: Self = Self(8);
    pub const RightTap: Self = Self(16);
    pub const Drag: Self = Self(32);
    pub const ManipulationTranslateX: Self = Self(64);
    pub const ManipulationTranslateY: Self = Self(128);
    pub const ManipulationTranslateRailsX: Self = Self(256);
    pub const ManipulationTranslateRailsY: Self = Self(512);
    pub const ManipulationRotate: Self = Self(1024);
    pub const ManipulationScale: Self = Self(2048);
    pub const ManipulationTranslateInertia: Self = Self(4096);
    pub const ManipulationRotateInertia: Self = Self(8192);
    pub const ManipulationScaleInertia: Self = Self(16384);
    pub const CrossSlide: Self = Self(32768);
    pub const ManipulationMultipleFingerPanning: Self = Self(65536);
}
impl windows_core::TypeKind for GestureSettings {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for GestureSettings {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Windows.UI.Input.GestureSettings;u4)");
}
impl GestureSettings {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for GestureSettings {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for GestureSettings {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for GestureSettings {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0);
    }
}
impl core::ops::BitAndAssign for GestureSettings {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0);
    }
}
impl core::ops::Not for GestureSettings {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
pub type HANDLE = *mut core::ffi::c_void;
pub type HINSTANCE = *mut core::ffi::c_void;
pub type HMODULE = HINSTANCE;
pub type HWND = *mut core::ffi::c_void;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HoldingEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    HoldingEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for HoldingEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IHoldingEventArgs>();
}
unsafe impl windows_core::Interface for HoldingEventArgs {
    type Vtable = <IHoldingEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IHoldingEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for HoldingEventArgs {
    type Target = IHoldingEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for HoldingEventArgs {
    const NAME: &'static str = "Windows.UI.Input.HoldingEventArgs";
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HoldingState(pub i32);
impl HoldingState {
    pub const Started: Self = Self(0);
    pub const Completed: Self = Self(1);
    pub const Canceled: Self = Self(2);
}
impl windows_core::TypeKind for HoldingState {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for HoldingState {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Windows.UI.Input.HoldingState;i4)");
}
windows_core::imp::define_interface!(
    ICoreFrameworkInputView,
    ICoreFrameworkInputView_Vtbl,
    0xd77c94ae_46b8_5d4a_9489_8ddec3d639a6
);
impl windows_core::RuntimeType for ICoreFrameworkInputView {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICoreFrameworkInputView_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICoreInputView,
    ICoreInputView_Vtbl,
    0xc770cd7a_7001_4c32_bf94_25c1f554cbf1
);
impl windows_core::RuntimeType for ICoreInputView {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICoreInputView_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICoreInputViewOcclusion,
    ICoreInputViewOcclusion_Vtbl,
    0xcc36ce06_3865_4177_b5f5_8b65e0b9ce84
);
impl windows_core::RuntimeType for ICoreInputViewOcclusion {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICoreInputViewOcclusion_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
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
    IDraggingEventArgs,
    IDraggingEventArgs_Vtbl,
    0x1c905384_083c_4bd3_b559_179cddeb33ec
);
impl windows_core::RuntimeType for IDraggingEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDraggingEventArgs {
    pub fn PointerDeviceType(&self) -> windows_core::Result<PointerDeviceType> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PointerDeviceType)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Position(&self) -> windows_core::Result<Point> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Position)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn DraggingState(&self) -> windows_core::Result<DraggingState> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).DraggingState)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IDraggingEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub PointerDeviceType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut PointerDeviceType,
    ) -> windows_core::HRESULT,
    pub Position:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Point) -> windows_core::HRESULT,
    pub DraggingState: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut DraggingState,
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
    IEnumTfContexts,
    IEnumTfContexts_Vtbl,
    0x8f1a7ea6_1654_4502_a86e_b2902344d507
);
windows_core::imp::interface_hierarchy!(IEnumTfContexts, windows_core::IUnknown);
#[repr(C)]
pub struct IEnumTfContexts_Vtbl {
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
    IEnumTfDocumentMgrs,
    IEnumTfDocumentMgrs_Vtbl,
    0xaa80e808_2021_11d2_93e0_0060b067b86e
);
windows_core::imp::interface_hierarchy!(IEnumTfDocumentMgrs, windows_core::IUnknown);
#[repr(C)]
pub struct IEnumTfDocumentMgrs_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    Clone: usize,
    Next: usize,
    Reset: usize,
    Skip: usize,
}
windows_core::imp::define_interface!(
    IEnumTfFunctionProviders,
    IEnumTfFunctionProviders_Vtbl,
    0xe4b24db0_0990_11d3_8df0_00105a2799b5
);
windows_core::imp::interface_hierarchy!(IEnumTfFunctionProviders, windows_core::IUnknown);
#[repr(C)]
pub struct IEnumTfFunctionProviders_Vtbl {
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
    IGestureRecognizer,
    IGestureRecognizer_Vtbl,
    0xb47a37bf_3d6b_4f88_83e8_6dcb4012ffb0
);
impl windows_core::RuntimeType for IGestureRecognizer {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IGestureRecognizer {
    pub fn GestureSettings(&self) -> windows_core::Result<GestureSettings> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GestureSettings)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetGestureSettings(&self, value: GestureSettings) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetGestureSettings)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn IsInertial(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsInertial)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn IsActive(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsActive)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetShowGestureFeedback(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetShowGestureFeedback)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetPivotCenter(&self, value: Point) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPivotCenter)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetPivotRadius(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPivotRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetInertiaTranslationDeceleration(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetInertiaTranslationDeceleration)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetInertiaRotationDeceleration(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetInertiaRotationDeceleration)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetInertiaExpansionDeceleration(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetInertiaExpansionDeceleration)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetManipulationExact(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetManipulationExact)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetAutoProcessInertia(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetAutoProcessInertia)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn ProcessDownEvent<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<PointerPoint>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).ProcessDownEvent)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn ProcessMoveEvents<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_collections::IVector<PointerPoint>>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).ProcessMoveEvents)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn ProcessUpEvent<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<PointerPoint>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).ProcessUpEvent)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn ProcessMouseWheelEvent<P0>(
        &self,
        value: P0,
        isshiftkeydown: bool,
        iscontrolkeydown: bool,
    ) -> windows_core::Result<()>
    where
        P0: windows_core::Param<PointerPoint>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).ProcessMouseWheelEvent)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
                isshiftkeydown,
                iscontrolkeydown,
            )
            .ok()
        }
    }
    pub fn ProcessInertia(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).ProcessInertia)(windows_core::Interface::as_raw(
                self,
            ))
            .ok()
        }
    }
    pub fn CompleteGesture(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).CompleteGesture)(
                windows_core::Interface::as_raw(self),
            )
            .ok()
        }
    }
    pub fn Tapped<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<GestureRecognizer>, windows_core::Ref<TappedEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<GestureRecognizer, TappedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<GestureRecognizer, TappedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<GestureRecognizer, TappedEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Tapped)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveTapped,
            ))
        }
    }
    pub fn RightTapped<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<GestureRecognizer>, windows_core::Ref<RightTappedEventArgs>)
            + 'static,
    {
        let handler: TypedEventHandler<GestureRecognizer, RightTappedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<GestureRecognizer, RightTappedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<GestureRecognizer, RightTappedEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).RightTapped)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveRightTapped,
            ))
        }
    }
    pub fn Holding<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<GestureRecognizer>, windows_core::Ref<HoldingEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<GestureRecognizer, HoldingEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<GestureRecognizer, HoldingEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<GestureRecognizer, HoldingEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Holding)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveHolding,
            ))
        }
    }
    pub fn Dragging<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<GestureRecognizer>, windows_core::Ref<DraggingEventArgs>) + 'static,
    {
        let handler: TypedEventHandler<GestureRecognizer, DraggingEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<GestureRecognizer, DraggingEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<GestureRecognizer, DraggingEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Dragging)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveDragging,
            ))
        }
    }
    pub fn ManipulationStarted<F>(
        &self,
        handler: F,
    ) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<GestureRecognizer>,
                windows_core::Ref<ManipulationStartedEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<GestureRecognizer, ManipulationStartedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<GestureRecognizer, ManipulationStartedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<GestureRecognizer, ManipulationStartedEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ManipulationStarted)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveManipulationStarted,
            ))
        }
    }
    pub fn ManipulationUpdated<F>(
        &self,
        handler: F,
    ) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<GestureRecognizer>,
                windows_core::Ref<ManipulationUpdatedEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<GestureRecognizer, ManipulationUpdatedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<GestureRecognizer, ManipulationUpdatedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<GestureRecognizer, ManipulationUpdatedEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ManipulationUpdated)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveManipulationUpdated,
            ))
        }
    }
    pub fn ManipulationInertiaStarting<F>(
        &self,
        handler: F,
    ) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<GestureRecognizer>,
                windows_core::Ref<ManipulationInertiaStartingEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<GestureRecognizer, ManipulationInertiaStartingEventArgs> = {
            let com =
                windows_core::imp::DelegateBox::<
                    TypedEventHandler<GestureRecognizer, ManipulationInertiaStartingEventArgs>,
                    F,
                >::new(
                    &TypedEventHandlerBox::<
                        GestureRecognizer,
                        ManipulationInertiaStartingEventArgs,
                        F,
                    >::VTABLE,
                    handler,
                );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ManipulationInertiaStarting)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveManipulationInertiaStarting,
            ))
        }
    }
    pub fn ManipulationCompleted<F>(
        &self,
        handler: F,
    ) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<GestureRecognizer>,
                windows_core::Ref<ManipulationCompletedEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<GestureRecognizer, ManipulationCompletedEventArgs> = {
            let com = windows_core::imp::DelegateBox::< TypedEventHandler < GestureRecognizer , ManipulationCompletedEventArgs > , F >::new (& TypedEventHandlerBox::< GestureRecognizer , ManipulationCompletedEventArgs , F >::VTABLE , handler) ;
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ManipulationCompleted)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveManipulationCompleted,
            ))
        }
    }
}
#[repr(C)]
pub struct IGestureRecognizer_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub GestureSettings: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut GestureSettings,
    ) -> windows_core::HRESULT,
    pub SetGestureSettings:
        unsafe extern "system" fn(*mut core::ffi::c_void, GestureSettings) -> windows_core::HRESULT,
    pub IsInertial:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub IsActive:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    ShowGestureFeedback: usize,
    pub SetShowGestureFeedback:
        unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    PivotCenter: usize,
    pub SetPivotCenter:
        unsafe extern "system" fn(*mut core::ffi::c_void, Point) -> windows_core::HRESULT,
    PivotRadius: usize,
    pub SetPivotRadius:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    InertiaTranslationDeceleration: usize,
    pub SetInertiaTranslationDeceleration:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    InertiaRotationDeceleration: usize,
    pub SetInertiaRotationDeceleration:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    InertiaExpansionDeceleration: usize,
    pub SetInertiaExpansionDeceleration:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    InertiaTranslationDisplacement: usize,
    SetInertiaTranslationDisplacement: usize,
    InertiaRotationAngle: usize,
    SetInertiaRotationAngle: usize,
    InertiaExpansion: usize,
    SetInertiaExpansion: usize,
    ManipulationExact: usize,
    pub SetManipulationExact:
        unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    CrossSlideThresholds: usize,
    SetCrossSlideThresholds: usize,
    CrossSlideHorizontally: usize,
    SetCrossSlideHorizontally: usize,
    CrossSlideExact: usize,
    SetCrossSlideExact: usize,
    AutoProcessInertia: usize,
    pub SetAutoProcessInertia:
        unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    MouseWheelParameters: usize,
    CanBeDoubleTap: usize,
    pub ProcessDownEvent: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ProcessMoveEvents: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ProcessUpEvent: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ProcessMouseWheelEvent: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        bool,
        bool,
    ) -> windows_core::HRESULT,
    pub ProcessInertia: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub CompleteGesture: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Tapped: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveTapped:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub RightTapped: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveRightTapped:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub Holding: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveHolding:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub Dragging: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveDragging:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub ManipulationStarted: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveManipulationStarted:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub ManipulationUpdated: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveManipulationUpdated:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub ManipulationInertiaStarting: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveManipulationInertiaStarting:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub ManipulationCompleted: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveManipulationCompleted:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IGestureRecognizer2,
    IGestureRecognizer2_Vtbl,
    0xd646097f_6ef7_5746_8ba8_8ff2206e6f3b
);
impl windows_core::RuntimeType for IGestureRecognizer2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IGestureRecognizer2 {
    pub fn SetTapMinContactCount(&self, value: u32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTapMinContactCount)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetTapMaxContactCount(&self, value: u32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTapMaxContactCount)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetHoldMinContactCount(&self, value: u32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetHoldMinContactCount)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetHoldMaxContactCount(&self, value: u32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetHoldMaxContactCount)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetHoldRadius(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetHoldRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetHoldStartDelay(&self, value: windows_time::TimeSpan) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetHoldStartDelay)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetTranslationMinContactCount(&self, value: u32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTranslationMinContactCount)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetTranslationMaxContactCount(&self, value: u32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTranslationMaxContactCount)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IGestureRecognizer2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    TapMinContactCount: usize,
    pub SetTapMinContactCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    TapMaxContactCount: usize,
    pub SetTapMaxContactCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    HoldMinContactCount: usize,
    pub SetHoldMinContactCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    HoldMaxContactCount: usize,
    pub SetHoldMaxContactCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    HoldRadius: usize,
    pub SetHoldRadius:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    HoldStartDelay: usize,
    pub SetHoldStartDelay: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_time::TimeSpan,
    ) -> windows_core::HRESULT,
    TranslationMinContactCount: usize,
    pub SetTranslationMinContactCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    TranslationMaxContactCount: usize,
    pub SetTranslationMaxContactCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IHoldingEventArgs,
    IHoldingEventArgs_Vtbl,
    0x2bf755c5_e799_41b4_bb40_242f40959b71
);
impl windows_core::RuntimeType for IHoldingEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IHoldingEventArgs {
    pub fn PointerDeviceType(&self) -> windows_core::Result<PointerDeviceType> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PointerDeviceType)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Position(&self) -> windows_core::Result<Point> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Position)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn HoldingState(&self) -> windows_core::Result<HoldingState> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).HoldingState)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IHoldingEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub PointerDeviceType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut PointerDeviceType,
    ) -> windows_core::HRESULT,
    pub Position:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Point) -> windows_core::HRESULT,
    pub HoldingState: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut HoldingState,
    ) -> windows_core::HRESULT,
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
    IManipulationCompletedEventArgs,
    IManipulationCompletedEventArgs_Vtbl,
    0xb34ab22b_d19b_46ff_9f38_dec7754bb9e7
);
impl windows_core::RuntimeType for IManipulationCompletedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IManipulationCompletedEventArgs {
    pub fn PointerDeviceType(&self) -> windows_core::Result<PointerDeviceType> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PointerDeviceType)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Position(&self) -> windows_core::Result<Point> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Position)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Cumulative(&self) -> windows_core::Result<ManipulationDelta> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Cumulative)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Velocities(&self) -> windows_core::Result<ManipulationVelocities> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Velocities)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IManipulationCompletedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub PointerDeviceType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut PointerDeviceType,
    ) -> windows_core::HRESULT,
    pub Position:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Point) -> windows_core::HRESULT,
    pub Cumulative: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut ManipulationDelta,
    ) -> windows_core::HRESULT,
    pub Velocities: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut ManipulationVelocities,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IManipulationInertiaStartingEventArgs,
    IManipulationInertiaStartingEventArgs_Vtbl,
    0xdd37a898_26bf_467a_9ce5_ccf3fb11371e
);
impl windows_core::RuntimeType for IManipulationInertiaStartingEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IManipulationInertiaStartingEventArgs {
    pub fn PointerDeviceType(&self) -> windows_core::Result<PointerDeviceType> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PointerDeviceType)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Position(&self) -> windows_core::Result<Point> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Position)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Delta(&self) -> windows_core::Result<ManipulationDelta> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Delta)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Cumulative(&self) -> windows_core::Result<ManipulationDelta> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Cumulative)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Velocities(&self) -> windows_core::Result<ManipulationVelocities> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Velocities)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IManipulationInertiaStartingEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub PointerDeviceType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut PointerDeviceType,
    ) -> windows_core::HRESULT,
    pub Position:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Point) -> windows_core::HRESULT,
    pub Delta: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut ManipulationDelta,
    ) -> windows_core::HRESULT,
    pub Cumulative: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut ManipulationDelta,
    ) -> windows_core::HRESULT,
    pub Velocities: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut ManipulationVelocities,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IManipulationStartedEventArgs,
    IManipulationStartedEventArgs_Vtbl,
    0xddec873e_cfce_4932_8c1d_3c3d011a34c0
);
impl windows_core::RuntimeType for IManipulationStartedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IManipulationStartedEventArgs {
    pub fn PointerDeviceType(&self) -> windows_core::Result<PointerDeviceType> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PointerDeviceType)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Position(&self) -> windows_core::Result<Point> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Position)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Cumulative(&self) -> windows_core::Result<ManipulationDelta> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Cumulative)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IManipulationStartedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub PointerDeviceType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut PointerDeviceType,
    ) -> windows_core::HRESULT,
    pub Position:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Point) -> windows_core::HRESULT,
    pub Cumulative: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut ManipulationDelta,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IManipulationUpdatedEventArgs,
    IManipulationUpdatedEventArgs_Vtbl,
    0xcb354ce5_abb8_4f9f_b3ce_8181aa61ad82
);
impl windows_core::RuntimeType for IManipulationUpdatedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IManipulationUpdatedEventArgs {
    pub fn PointerDeviceType(&self) -> windows_core::Result<PointerDeviceType> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PointerDeviceType)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Position(&self) -> windows_core::Result<Point> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Position)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Delta(&self) -> windows_core::Result<ManipulationDelta> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Delta)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Cumulative(&self) -> windows_core::Result<ManipulationDelta> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Cumulative)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Velocities(&self) -> windows_core::Result<ManipulationVelocities> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Velocities)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IManipulationUpdatedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub PointerDeviceType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut PointerDeviceType,
    ) -> windows_core::HRESULT,
    pub Position:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Point) -> windows_core::HRESULT,
    pub Delta: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut ManipulationDelta,
    ) -> windows_core::HRESULT,
    pub Cumulative: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut ManipulationDelta,
    ) -> windows_core::HRESULT,
    pub Velocities: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut ManipulationVelocities,
    ) -> windows_core::HRESULT,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct INPUT_TRANSFORM {
    pub Anonymous: INPUT_TRANSFORM_0,
}
impl Default for INPUT_TRANSFORM {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union INPUT_TRANSFORM_0 {
    pub Anonymous: INPUT_TRANSFORM_0_0,
    pub m: [[f32; 4]; 4],
}
impl Default for INPUT_TRANSFORM_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct INPUT_TRANSFORM_0_0 {
    pub _11: f32,
    pub _12: f32,
    pub _13: f32,
    pub _14: f32,
    pub _21: f32,
    pub _22: f32,
    pub _23: f32,
    pub _24: f32,
    pub _31: f32,
    pub _32: f32,
    pub _33: f32,
    pub _34: f32,
    pub _41: f32,
    pub _42: f32,
    pub _43: f32,
    pub _44: f32,
}
windows_core::imp::define_interface!(
    IPhysicalGestureRecognizer,
    IPhysicalGestureRecognizer_Vtbl,
    0x79a29f4d_32a6_5aa5_a999_42b0b420c66d
);
impl windows_core::RuntimeType for IPhysicalGestureRecognizer {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IPhysicalGestureRecognizer {
    pub fn IsActive(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsActive)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn GestureSettings(&self) -> windows_core::Result<GestureSettings> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GestureSettings)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetGestureSettings(&self, value: GestureSettings) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetGestureSettings)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetHoldRadius(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetHoldRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetHoldStartDelay(&self, value: windows_time::TimeSpan) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetHoldStartDelay)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetTranslationMinContactCount(&self, value: u32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTranslationMinContactCount)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetTranslationMaxContactCount(&self, value: u32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTranslationMaxContactCount)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn ProcessDownEvent<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<PointerPoint>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).ProcessDownEvent)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn ProcessMoveEvents<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_collections::IVector<PointerPoint>>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).ProcessMoveEvents)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn ProcessUpEvent<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<PointerPoint>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).ProcessUpEvent)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn CompleteGesture(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).CompleteGesture)(
                windows_core::Interface::as_raw(self),
            )
            .ok()
        }
    }
    pub fn ManipulationStarted<F>(
        &self,
        handler: F,
    ) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<PhysicalGestureRecognizer>,
                windows_core::Ref<ManipulationStartedEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<PhysicalGestureRecognizer, ManipulationStartedEventArgs> = {
            let com =
                windows_core::imp::DelegateBox::<
                    TypedEventHandler<PhysicalGestureRecognizer, ManipulationStartedEventArgs>,
                    F,
                >::new(
                    &TypedEventHandlerBox::<
                        PhysicalGestureRecognizer,
                        ManipulationStartedEventArgs,
                        F,
                    >::VTABLE,
                    handler,
                );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ManipulationStarted)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveManipulationStarted,
            ))
        }
    }
    pub fn ManipulationUpdated<F>(
        &self,
        handler: F,
    ) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<PhysicalGestureRecognizer>,
                windows_core::Ref<ManipulationUpdatedEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<PhysicalGestureRecognizer, ManipulationUpdatedEventArgs> = {
            let com =
                windows_core::imp::DelegateBox::<
                    TypedEventHandler<PhysicalGestureRecognizer, ManipulationUpdatedEventArgs>,
                    F,
                >::new(
                    &TypedEventHandlerBox::<
                        PhysicalGestureRecognizer,
                        ManipulationUpdatedEventArgs,
                        F,
                    >::VTABLE,
                    handler,
                );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ManipulationUpdated)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveManipulationUpdated,
            ))
        }
    }
    pub fn ManipulationCompleted<F>(
        &self,
        handler: F,
    ) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<PhysicalGestureRecognizer>,
                windows_core::Ref<ManipulationCompletedEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<PhysicalGestureRecognizer, ManipulationCompletedEventArgs> = {
            let com =
                windows_core::imp::DelegateBox::<
                    TypedEventHandler<PhysicalGestureRecognizer, ManipulationCompletedEventArgs>,
                    F,
                >::new(
                    &TypedEventHandlerBox::<
                        PhysicalGestureRecognizer,
                        ManipulationCompletedEventArgs,
                        F,
                    >::VTABLE,
                    handler,
                );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ManipulationCompleted)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveManipulationCompleted,
            ))
        }
    }
    pub fn Tapped<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<PhysicalGestureRecognizer>, windows_core::Ref<TappedEventArgs>)
            + 'static,
    {
        let handler: TypedEventHandler<PhysicalGestureRecognizer, TappedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<PhysicalGestureRecognizer, TappedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<PhysicalGestureRecognizer, TappedEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Tapped)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveTapped,
            ))
        }
    }
    pub fn Holding<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<PhysicalGestureRecognizer>, windows_core::Ref<HoldingEventArgs>)
            + 'static,
    {
        let handler: TypedEventHandler<PhysicalGestureRecognizer, HoldingEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<PhysicalGestureRecognizer, HoldingEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<PhysicalGestureRecognizer, HoldingEventArgs, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Holding)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveHolding,
            ))
        }
    }
}
#[repr(C)]
pub struct IPhysicalGestureRecognizer_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub IsActive:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub GestureSettings: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut GestureSettings,
    ) -> windows_core::HRESULT,
    pub SetGestureSettings:
        unsafe extern "system" fn(*mut core::ffi::c_void, GestureSettings) -> windows_core::HRESULT,
    TapMinContactCount: usize,
    SetTapMinContactCount: usize,
    TapMaxContactCount: usize,
    SetTapMaxContactCount: usize,
    HoldMinContactCount: usize,
    SetHoldMinContactCount: usize,
    HoldMaxContactCount: usize,
    SetHoldMaxContactCount: usize,
    HoldRadius: usize,
    pub SetHoldRadius:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    HoldStartDelay: usize,
    pub SetHoldStartDelay: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_time::TimeSpan,
    ) -> windows_core::HRESULT,
    TranslationMinContactCount: usize,
    pub SetTranslationMinContactCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    TranslationMaxContactCount: usize,
    pub SetTranslationMaxContactCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    pub ProcessDownEvent: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ProcessMoveEvents: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ProcessUpEvent: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CompleteGesture: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub ManipulationStarted: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveManipulationStarted:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub ManipulationUpdated: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveManipulationUpdated:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub ManipulationCompleted: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveManipulationCompleted:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub Tapped: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveTapped:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub Holding: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveHolding:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IPointerDevice,
    IPointerDevice_Vtbl,
    0x93c9bafc_ebcb_467e_82c6_276feae36b5a
);
impl windows_core::RuntimeType for IPointerDevice {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IPointerDevice {
    pub fn PointerDeviceType(&self) -> windows_core::Result<PointerDeviceType> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PointerDeviceType)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn IsIntegrated(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsIntegrated)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn MaxContacts(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).MaxContacts)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IPointerDevice_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub PointerDeviceType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut PointerDeviceType,
    ) -> windows_core::HRESULT,
    pub IsIntegrated:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub MaxContacts:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IPointerDeviceStatics,
    IPointerDeviceStatics_Vtbl,
    0xd8b89aa1_d1c6_416e_bd8d_5790914dc563
);
impl windows_core::RuntimeType for IPointerDeviceStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IPointerDeviceStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub GetPointerDevice: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetPointerDevices: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IPointerPoint,
    IPointerPoint_Vtbl,
    0xe995317d_7296_42d9_8233_c5be73b74a4a
);
impl windows_core::RuntimeType for IPointerPoint {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IPointerPoint {
    pub fn Position(&self) -> windows_core::Result<Point> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Position)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn RawPosition(&self) -> windows_core::Result<Point> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).RawPosition)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn PointerId(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PointerId)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Timestamp(&self) -> windows_core::Result<u64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Timestamp)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn IsInContact(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsInContact)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Properties(&self) -> windows_core::Result<PointerPointProperties> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Properties)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IPointerPoint_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    PointerDevice: usize,
    pub Position:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Point) -> windows_core::HRESULT,
    pub RawPosition:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Point) -> windows_core::HRESULT,
    pub PointerId:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    FrameId: usize,
    pub Timestamp:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u64) -> windows_core::HRESULT,
    pub IsInContact:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub Properties: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IPointerPointProperties,
    IPointerPointProperties_Vtbl,
    0xc79d8a4b_c163_4ee7_803f_67ce79f9972d
);
impl windows_core::RuntimeType for IPointerPointProperties {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IPointerPointProperties {
    pub fn Pressure(&self) -> windows_core::Result<f32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Pressure)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn IsInverted(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsInverted)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn IsEraser(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsEraser)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Orientation(&self) -> windows_core::Result<f32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Orientation)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn XTilt(&self) -> windows_core::Result<f32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).XTilt)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn YTilt(&self) -> windows_core::Result<f32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).YTilt)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Twist(&self) -> windows_core::Result<f32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Twist)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn ContactRect(&self) -> windows_core::Result<Rect> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ContactRect)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn ContactRectRaw(&self) -> windows_core::Result<Rect> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ContactRectRaw)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn TouchConfidence(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TouchConfidence)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn MouseWheelDelta(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).MouseWheelDelta)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn IsHorizontalMouseWheel(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsHorizontalMouseWheel)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn IsPrimary(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsPrimary)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn IsInRange(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsInRange)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn IsCanceled(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsCanceled)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn IsBarrelButtonPressed(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsBarrelButtonPressed)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn PointerUpdateKind(&self) -> windows_core::Result<PointerUpdateKind> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PointerUpdateKind)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IPointerPointProperties_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Pressure:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut f32) -> windows_core::HRESULT,
    pub IsInverted:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub IsEraser:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub Orientation:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut f32) -> windows_core::HRESULT,
    pub XTilt: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f32) -> windows_core::HRESULT,
    pub YTilt: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f32) -> windows_core::HRESULT,
    pub Twist: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f32) -> windows_core::HRESULT,
    pub ContactRect:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Rect) -> windows_core::HRESULT,
    pub ContactRectRaw:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Rect) -> windows_core::HRESULT,
    pub TouchConfidence:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    IsLeftButtonPressed: usize,
    IsRightButtonPressed: usize,
    IsMiddleButtonPressed: usize,
    pub MouseWheelDelta:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub IsHorizontalMouseWheel:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub IsPrimary:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub IsInRange:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub IsCanceled:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    pub IsBarrelButtonPressed:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
    IsXButton1Pressed: usize,
    IsXButton2Pressed: usize,
    pub PointerUpdateKind: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut PointerUpdateKind,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IPointerPointStatics,
    IPointerPointStatics_Vtbl,
    0xa506638d_2a1a_413e_bc75_9f38381cc069
);
impl windows_core::RuntimeType for IPointerPointStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IPointerPointStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub GetCurrentPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetIntermediatePoints: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetCurrentPointTransformed: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetIntermediatePointsTransformed: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IPointerPointTransform,
    IPointerPointTransform_Vtbl,
    0x4d5fe14f_b87c_4028_bc9c_59e9947fb056
);
impl windows_core::RuntimeType for IPointerPointTransform {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
    const NAME: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"Windows.UI.Input.IPointerPointTransform");
}
windows_core::imp::interface_hierarchy!(
    IPointerPointTransform,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl IPointerPointTransform {
    pub fn Inverse(&self) -> windows_core::Result<Self> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Inverse)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub fn TryTransform(&self, inpoint: Point, outpoint: &mut Point) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TryTransform)(
                windows_core::Interface::as_raw(self),
                inpoint,
                outpoint,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn TransformBounds(&self, rect: Rect) -> windows_core::Result<Rect> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TransformBounds)(
                windows_core::Interface::as_raw(self),
                rect,
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
impl windows_core::RuntimeName for IPointerPointTransform {
    const NAME: &'static str = "Windows.UI.Input.IPointerPointTransform";
}
pub trait IPointerPointTransform_Impl: windows_core::IUnknownImpl {
    fn Inverse(&self) -> windows_core::Result<IPointerPointTransform>;
    fn TryTransform(&self, inPoint: &Point, outPoint: &mut Point) -> windows_core::Result<bool>;
    fn TransformBounds(&self, rect: &Rect) -> windows_core::Result<Rect>;
}
impl IPointerPointTransform_Vtbl {
    pub const fn new<Identity: IPointerPointTransform_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn Inverse<
            Identity: IPointerPointTransform_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            result__: *mut *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IPointerPointTransform_Impl::Inverse(this) {
                    Ok(ok__) => {
                        result__.write(core::mem::transmute_copy(&ok__));
                        core::mem::forget(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn TryTransform<
            Identity: IPointerPointTransform_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            inpoint: Point,
            outpoint: *mut Point,
            result__: *mut bool,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IPointerPointTransform_Impl::TryTransform(
                    this,
                    core::mem::transmute(&inpoint),
                    core::mem::transmute_copy(&outpoint),
                ) {
                    Ok(ok__) => {
                        result__.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn TransformBounds<
            Identity: IPointerPointTransform_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            rect: Rect,
            result__: *mut Rect,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                match IPointerPointTransform_Impl::TransformBounds(
                    this,
                    core::mem::transmute(&rect),
                ) {
                    Ok(ok__) => {
                        result__.write(ok__);
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IInspectable_Vtbl::new::<Identity, IPointerPointTransform, OFFSET>(
            ),
            Inverse: Inverse::<Identity, OFFSET>,
            TryTransform: TryTransform::<Identity, OFFSET>,
            TransformBounds: TransformBounds::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IPointerPointTransform as windows_core::Interface>::IID
    }
}
#[repr(C)]
pub struct IPointerPointTransform_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Inverse: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub TryTransform: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        Point,
        *mut Point,
        *mut bool,
    ) -> windows_core::HRESULT,
    pub TransformBounds:
        unsafe extern "system" fn(*mut core::ffi::c_void, Rect, *mut Rect) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadialController,
    IRadialController_Vtbl,
    0x3055d1c8_df51_43d4_b23b_0e1037467a09
);
impl windows_core::RuntimeType for IRadialController {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRadialController {
    pub fn Menu(&self) -> windows_core::Result<RadialControllerMenu> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Menu)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub fn SetRotationResolutionInDegrees(&self, value: f64) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetRotationResolutionInDegrees)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetUseAutomaticHapticFeedback(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetUseAutomaticHapticFeedback)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn ScreenContactStarted<F>(
        &self,
        handler: F,
    ) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<RadialController>,
                windows_core::Ref<RadialControllerScreenContactStartedEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<
            RadialController,
            RadialControllerScreenContactStartedEventArgs,
        > = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<RadialController, RadialControllerScreenContactStartedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<
                    RadialController,
                    RadialControllerScreenContactStartedEventArgs,
                    F,
                >::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ScreenContactStarted)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveScreenContactStarted,
            ))
        }
    }
    pub fn ScreenContactEnded<F>(
        &self,
        handler: F,
    ) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<RadialController>, windows_core::Ref<windows_core::IInspectable>)
            + 'static,
    {
        let handler: TypedEventHandler<RadialController, windows_core::IInspectable> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<RadialController, windows_core::IInspectable>,
                F,
            >::new(
                &TypedEventHandlerBox::<RadialController, windows_core::IInspectable, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ScreenContactEnded)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveScreenContactEnded,
            ))
        }
    }
    pub fn ScreenContactContinued<F>(
        &self,
        handler: F,
    ) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<RadialController>,
                windows_core::Ref<RadialControllerScreenContactContinuedEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<
            RadialController,
            RadialControllerScreenContactContinuedEventArgs,
        > = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<
                    RadialController,
                    RadialControllerScreenContactContinuedEventArgs,
                >,
                F,
            >::new(
                &TypedEventHandlerBox::<
                    RadialController,
                    RadialControllerScreenContactContinuedEventArgs,
                    F,
                >::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ScreenContactContinued)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveScreenContactContinued,
            ))
        }
    }
    pub fn ControlLost<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(windows_core::Ref<RadialController>, windows_core::Ref<windows_core::IInspectable>)
            + 'static,
    {
        let handler: TypedEventHandler<RadialController, windows_core::IInspectable> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<RadialController, windows_core::IInspectable>,
                F,
            >::new(
                &TypedEventHandlerBox::<RadialController, windows_core::IInspectable, F>::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ControlLost)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveControlLost,
            ))
        }
    }
    pub fn RotationChanged<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<RadialController>,
                windows_core::Ref<RadialControllerRotationChangedEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<RadialController, RadialControllerRotationChangedEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<RadialController, RadialControllerRotationChangedEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<
                    RadialController,
                    RadialControllerRotationChangedEventArgs,
                    F,
                >::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).RotationChanged)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveRotationChanged,
            ))
        }
    }
    pub fn ButtonClicked<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<RadialController>,
                windows_core::Ref<RadialControllerButtonClickedEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<RadialController, RadialControllerButtonClickedEventArgs> = {
            let com =
                windows_core::imp::DelegateBox::<
                    TypedEventHandler<RadialController, RadialControllerButtonClickedEventArgs>,
                    F,
                >::new(
                    &TypedEventHandlerBox::<
                        RadialController,
                        RadialControllerButtonClickedEventArgs,
                        F,
                    >::VTABLE,
                    handler,
                );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ButtonClicked)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveButtonClicked,
            ))
        }
    }
    pub fn ControlAcquired<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<RadialController>,
                windows_core::Ref<RadialControllerControlAcquiredEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<RadialController, RadialControllerControlAcquiredEventArgs> = {
            let com = windows_core::imp::DelegateBox::<
                TypedEventHandler<RadialController, RadialControllerControlAcquiredEventArgs>,
                F,
            >::new(
                &TypedEventHandlerBox::<
                    RadialController,
                    RadialControllerControlAcquiredEventArgs,
                    F,
                >::VTABLE,
                handler,
            );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ControlAcquired)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveControlAcquired,
            ))
        }
    }
}
#[repr(C)]
pub struct IRadialController_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Menu: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    RotationResolutionInDegrees: usize,
    pub SetRotationResolutionInDegrees:
        unsafe extern "system" fn(*mut core::ffi::c_void, f64) -> windows_core::HRESULT,
    UseAutomaticHapticFeedback: usize,
    pub SetUseAutomaticHapticFeedback:
        unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    pub ScreenContactStarted: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveScreenContactStarted:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub ScreenContactEnded: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveScreenContactEnded:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub ScreenContactContinued: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveScreenContactContinued:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub ControlLost: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveControlLost:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub RotationChanged: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveRotationChanged:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub ButtonClicked: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveButtonClicked:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub ControlAcquired: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveControlAcquired:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadialController2,
    IRadialController2_Vtbl,
    0x3d577eff_4cee_11e6_b535_001bdc06ab3b
);
impl windows_core::RuntimeType for IRadialController2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRadialController2 {
    pub fn ButtonPressed<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<RadialController>,
                windows_core::Ref<RadialControllerButtonPressedEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<RadialController, RadialControllerButtonPressedEventArgs> = {
            let com =
                windows_core::imp::DelegateBox::<
                    TypedEventHandler<RadialController, RadialControllerButtonPressedEventArgs>,
                    F,
                >::new(
                    &TypedEventHandlerBox::<
                        RadialController,
                        RadialControllerButtonPressedEventArgs,
                        F,
                    >::VTABLE,
                    handler,
                );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ButtonPressed)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveButtonPressed,
            ))
        }
    }
    pub fn ButtonReleased<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<RadialController>,
                windows_core::Ref<RadialControllerButtonReleasedEventArgs>,
            ) + 'static,
    {
        let handler: TypedEventHandler<RadialController, RadialControllerButtonReleasedEventArgs> = {
            let com =
                windows_core::imp::DelegateBox::<
                    TypedEventHandler<RadialController, RadialControllerButtonReleasedEventArgs>,
                    F,
                >::new(
                    &TypedEventHandlerBox::<
                        RadialController,
                        RadialControllerButtonReleasedEventArgs,
                        F,
                    >::VTABLE,
                    handler,
                );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).ButtonReleased)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveButtonReleased,
            ))
        }
    }
}
#[repr(C)]
pub struct IRadialController2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub ButtonPressed: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveButtonPressed:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    ButtonHolding: usize,
    RemoveButtonHolding: usize,
    pub ButtonReleased: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveButtonReleased:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadialControllerButtonClickedEventArgs,
    IRadialControllerButtonClickedEventArgs_Vtbl,
    0x206aa438_e651_11e5_bf62_2c27d7404e85
);
impl windows_core::RuntimeType for IRadialControllerButtonClickedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRadialControllerButtonClickedEventArgs {
    pub fn Contact(&self) -> windows_core::Result<RadialControllerScreenContact> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Contact)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IRadialControllerButtonClickedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Contact: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadialControllerButtonPressedEventArgs,
    IRadialControllerButtonPressedEventArgs_Vtbl,
    0x3d577eed_4cee_11e6_b535_001bdc06ab3b
);
impl windows_core::RuntimeType for IRadialControllerButtonPressedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IRadialControllerButtonPressedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IRadialControllerButtonReleasedEventArgs,
    IRadialControllerButtonReleasedEventArgs_Vtbl,
    0x3d577eef_3cee_11e6_b535_001bdc06ab3b
);
impl windows_core::RuntimeType for IRadialControllerButtonReleasedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IRadialControllerButtonReleasedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IRadialControllerConfiguration,
    IRadialControllerConfiguration_Vtbl,
    0xa6b79ecb_6a52_4430_910c_56370a9d6b42
);
impl windows_core::RuntimeType for IRadialControllerConfiguration {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRadialControllerConfiguration {
    pub fn SetDefaultMenuItems<P0>(&self, buttons: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_collections::IIterable<RadialControllerSystemMenuItemKind>>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetDefaultMenuItems)(
                windows_core::Interface::as_raw(self),
                buttons.param().abi(),
            )
            .ok()
        }
    }
    pub fn ResetToDefaultMenuItems(&self) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).ResetToDefaultMenuItems)(
                windows_core::Interface::as_raw(self),
            )
            .ok()
        }
    }
    pub fn TrySelectDefaultMenuItem(
        &self,
        r#type: RadialControllerSystemMenuItemKind,
    ) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TrySelectDefaultMenuItem)(
                windows_core::Interface::as_raw(self),
                r#type,
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IRadialControllerConfiguration_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub SetDefaultMenuItems: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ResetToDefaultMenuItems:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub TrySelectDefaultMenuItem: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        RadialControllerSystemMenuItemKind,
        *mut bool,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadialControllerConfigurationInterop,
    IRadialControllerConfigurationInterop_Vtbl,
    0x787cdaac_3186_476d_87e4_b9374a7b9970
);
windows_core::imp::interface_hierarchy!(
    IRadialControllerConfigurationInterop,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl IRadialControllerConfigurationInterop {
    pub unsafe fn GetForWindow<T>(&self, hwnd: HWND) -> windows_core::Result<T>
    where
        T: windows_core::Interface,
    {
        let mut result__ = core::ptr::null_mut();
        unsafe {
            (windows_core::Interface::vtable(self).GetForWindow)(
                windows_core::Interface::as_raw(self),
                hwnd,
                &T::IID,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IRadialControllerConfigurationInterop_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub GetForWindow: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        HWND,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadialControllerControlAcquiredEventArgs,
    IRadialControllerControlAcquiredEventArgs_Vtbl,
    0x206aa439_e651_11e5_bf62_2c27d7404e85
);
impl windows_core::RuntimeType for IRadialControllerControlAcquiredEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IRadialControllerControlAcquiredEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IRadialControllerInterop,
    IRadialControllerInterop_Vtbl,
    0x1b0535c9_57ad_45c1_9d79_ad5c34360513
);
windows_core::imp::interface_hierarchy!(
    IRadialControllerInterop,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl IRadialControllerInterop {
    pub unsafe fn CreateForWindow<T>(&self, hwnd: HWND) -> windows_core::Result<T>
    where
        T: windows_core::Interface,
    {
        let mut result__ = core::ptr::null_mut();
        unsafe {
            (windows_core::Interface::vtable(self).CreateForWindow)(
                windows_core::Interface::as_raw(self),
                hwnd,
                &T::IID,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IRadialControllerInterop_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateForWindow: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        HWND,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadialControllerMenu,
    IRadialControllerMenu_Vtbl,
    0x8506b35d_f640_4412_aba0_bad077e5ea8a
);
impl windows_core::RuntimeType for IRadialControllerMenu {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRadialControllerMenu {
    pub fn Items(
        &self,
    ) -> windows_core::Result<windows_collections::IVector<RadialControllerMenuItem>> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Items)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub fn SetIsEnabled(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsEnabled)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn GetSelectedMenuItem(&self) -> windows_core::Result<RadialControllerMenuItem> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetSelectedMenuItem)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub fn SelectMenuItem<P0>(&self, menuitem: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<RadialControllerMenuItem>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SelectMenuItem)(
                windows_core::Interface::as_raw(self),
                menuitem.param().abi(),
            )
            .ok()
        }
    }
    pub fn TrySelectPreviouslySelectedMenuItem(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TrySelectPreviouslySelectedMenuItem)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IRadialControllerMenu_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Items: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    IsEnabled: usize,
    pub SetIsEnabled:
        unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    pub GetSelectedMenuItem: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SelectMenuItem: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub TrySelectPreviouslySelectedMenuItem:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadialControllerMenuItem,
    IRadialControllerMenuItem_Vtbl,
    0xc80fc98d_ad0b_4c9c_8f2f_136a2373a6ba
);
impl windows_core::RuntimeType for IRadialControllerMenuItem {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRadialControllerMenuItem {
    pub fn DisplayText(&self) -> windows_core::Result<String> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).DisplayText)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| {
                let hstring: windows_core::HSTRING = core::mem::transmute(result__);
                hstring.to_string_lossy()
            })
        }
    }
    pub fn Invoked<F>(&self, handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<RadialControllerMenuItem>,
                windows_core::Ref<windows_core::IInspectable>,
            ) + 'static,
    {
        let handler: TypedEventHandler<RadialControllerMenuItem, windows_core::IInspectable> = {
            let com = windows_core::imp::DelegateBox::< TypedEventHandler < RadialControllerMenuItem , windows_core::IInspectable > , F >::new (& TypedEventHandlerBox::< RadialControllerMenuItem , windows_core::IInspectable , F >::VTABLE , handler) ;
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(self).Invoked)(
                windows_core::Interface::as_raw(self),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                self.clone(),
                token__,
                windows_core::Interface::vtable(self).RemoveInvoked,
            ))
        }
    }
}
#[repr(C)]
pub struct IRadialControllerMenuItem_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub DisplayText: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    Tag: usize,
    SetTag: usize,
    pub Invoked: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveInvoked:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadialControllerMenuItemStatics2,
    IRadialControllerMenuItemStatics2_Vtbl,
    0x0cbb70be_7e3e_48bd_be04_2c7fcaa9c1ff
);
impl windows_core::RuntimeType for IRadialControllerMenuItemStatics2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IRadialControllerMenuItemStatics2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateFromFontGlyph: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadialControllerRotationChangedEventArgs,
    IRadialControllerRotationChangedEventArgs_Vtbl,
    0x206aa435_e651_11e5_bf62_2c27d7404e85
);
impl windows_core::RuntimeType for IRadialControllerRotationChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRadialControllerRotationChangedEventArgs {
    pub fn RotationDeltaInDegrees(&self) -> windows_core::Result<f64> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).RotationDeltaInDegrees)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Contact(&self) -> windows_core::Result<RadialControllerScreenContact> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Contact)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IRadialControllerRotationChangedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub RotationDeltaInDegrees:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut f64) -> windows_core::HRESULT,
    pub Contact: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadialControllerScreenContact,
    IRadialControllerScreenContact_Vtbl,
    0x206aa434_e651_11e5_bf62_2c27d7404e85
);
impl windows_core::RuntimeType for IRadialControllerScreenContact {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRadialControllerScreenContact {
    pub fn Bounds(&self) -> windows_core::Result<Rect> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Bounds)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Position(&self) -> windows_core::Result<Point> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Position)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IRadialControllerScreenContact_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Bounds:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Rect) -> windows_core::HRESULT,
    pub Position:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Point) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadialControllerScreenContactContinuedEventArgs,
    IRadialControllerScreenContactContinuedEventArgs_Vtbl,
    0x206aa437_e651_11e5_bf62_2c27d7404e85
);
impl windows_core::RuntimeType for IRadialControllerScreenContactContinuedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRadialControllerScreenContactContinuedEventArgs {
    pub fn Contact(&self) -> windows_core::Result<RadialControllerScreenContact> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Contact)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IRadialControllerScreenContactContinuedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Contact: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IRadialControllerScreenContactStartedEventArgs,
    IRadialControllerScreenContactStartedEventArgs_Vtbl,
    0x206aa436_e651_11e5_bf62_2c27d7404e85
);
impl windows_core::RuntimeType for IRadialControllerScreenContactStartedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRadialControllerScreenContactStartedEventArgs {
    pub fn Contact(&self) -> windows_core::Result<RadialControllerScreenContact> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Contact)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IRadialControllerScreenContactStartedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Contact: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
    IRightTappedEventArgs,
    IRightTappedEventArgs_Vtbl,
    0x4cbf40bd_af7a_4a36_9476_b1dce141709a
);
impl windows_core::RuntimeType for IRightTappedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRightTappedEventArgs {
    pub fn PointerDeviceType(&self) -> windows_core::Result<PointerDeviceType> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PointerDeviceType)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Position(&self) -> windows_core::Result<Point> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Position)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IRightTappedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub PointerDeviceType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut PointerDeviceType,
    ) -> windows_core::HRESULT,
    pub Position:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Point) -> windows_core::HRESULT,
}
pub const IS_ADDRESS_CITY: InputScope = 17;
pub const IS_ADDRESS_COUNTRYNAME: InputScope = 18;
pub const IS_ADDRESS_COUNTRYSHORTNAME: InputScope = 19;
pub const IS_ADDRESS_FULLPOSTALADDRESS: InputScope = 13;
pub const IS_ADDRESS_POSTALCODE: InputScope = 14;
pub const IS_ADDRESS_STATEORPROVINCE: InputScope = 16;
pub const IS_ADDRESS_STREET: InputScope = 15;
pub const IS_ALPHANUMERIC_FULLWIDTH: InputScope = 41;
pub const IS_ALPHANUMERIC_HALFWIDTH: InputScope = 40;
pub const IS_ALPHANUMERIC_PIN: InputScope = 65;
pub const IS_ALPHANUMERIC_PIN_SET: InputScope = 66;
pub const IS_BOPOMOFO: InputScope = 43;
pub const IS_CHAT: InputScope = 58;
pub const IS_CHAT_WITHOUT_EMOJI: InputScope = 68;
pub const IS_CHINESE_FULLWIDTH: InputScope = 54;
pub const IS_CHINESE_HALFWIDTH: InputScope = 53;
pub const IS_CURRENCY_AMOUNT: InputScope = 21;
pub const IS_CURRENCY_AMOUNTANDSYMBOL: InputScope = 20;
pub const IS_CURRENCY_CHINESE: InputScope = 42;
pub const IS_DATE_DAY: InputScope = 24;
pub const IS_DATE_DAYNAME: InputScope = 27;
pub const IS_DATE_FULLDATE: InputScope = 22;
pub const IS_DATE_MONTH: InputScope = 23;
pub const IS_DATE_MONTHNAME: InputScope = 26;
pub const IS_DATE_YEAR: InputScope = 25;
pub const IS_DEFAULT: InputScope = 0;
pub const IS_DIGITS: InputScope = 28;
pub const IS_EMAILNAME_OR_ADDRESS: InputScope = 60;
pub const IS_EMAIL_SMTPEMAILADDRESS: InputScope = 5;
pub const IS_EMAIL_USERNAME: InputScope = 4;
pub const IS_ENUMSTRING: InputScope = -5;
pub const IS_FILE_FILENAME: InputScope = 3;
pub const IS_FILE_FULLFILEPATH: InputScope = 2;
pub const IS_FORMULA: InputScope = 51;
pub const IS_FORMULA_NUMBER: InputScope = 67;
pub const IS_HANGUL_FULLWIDTH: InputScope = 49;
pub const IS_HANGUL_HALFWIDTH: InputScope = 48;
pub const IS_HANJA: InputScope = 47;
pub const IS_HIRAGANA: InputScope = 44;
pub const IS_KATAKANA_FULLWIDTH: InputScope = 46;
pub const IS_KATAKANA_HALFWIDTH: InputScope = 45;
pub const IS_LOGINNAME: InputScope = 6;
pub const IS_MAPS: InputScope = 62;
pub const IS_NAME_OR_PHONENUMBER: InputScope = 59;
pub const IS_NATIVE_SCRIPT: InputScope = 55;
pub const IS_NUMBER: InputScope = 29;
pub const IS_NUMBER_FULLWIDTH: InputScope = 39;
pub const IS_NUMERIC_PASSWORD: InputScope = 63;
pub const IS_NUMERIC_PIN: InputScope = 64;
pub const IS_ONECHAR: InputScope = 30;
pub const IS_PASSWORD: InputScope = 31;
pub const IS_PERSONALNAME_FULLNAME: InputScope = 7;
pub const IS_PERSONALNAME_GIVENNAME: InputScope = 9;
pub const IS_PERSONALNAME_MIDDLENAME: InputScope = 10;
pub const IS_PERSONALNAME_PREFIX: InputScope = 8;
pub const IS_PERSONALNAME_SUFFIX: InputScope = 12;
pub const IS_PERSONALNAME_SURNAME: InputScope = 11;
pub const IS_PHRASELIST: InputScope = -1;
pub const IS_PRIVATE: InputScope = 61;
pub const IS_REGULAREXPRESSION: InputScope = -2;
pub const IS_SEARCH: InputScope = 50;
pub const IS_SEARCH_INCREMENTAL: InputScope = 52;
pub const IS_SRGS: InputScope = -3;
pub const IS_TELEPHONE_AREACODE: InputScope = 34;
pub const IS_TELEPHONE_COUNTRYCODE: InputScope = 33;
pub const IS_TELEPHONE_FULLTELEPHONENUMBER: InputScope = 32;
pub const IS_TELEPHONE_LOCALNUMBER: InputScope = 35;
pub const IS_TEXT: InputScope = 57;
pub const IS_TIME_FULLTIME: InputScope = 36;
pub const IS_TIME_HOUR: InputScope = 37;
pub const IS_TIME_MINORSEC: InputScope = 38;
pub const IS_URL: InputScope = 1;
pub const IS_XML: InputScope = -4;
pub const IS_YOMI: InputScope = 56;
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
    ITappedEventArgs,
    ITappedEventArgs_Vtbl,
    0xcfa126e4_253a_4c3c_953b_395c37aed309
);
impl windows_core::RuntimeType for ITappedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ITappedEventArgs {
    pub fn PointerDeviceType(&self) -> windows_core::Result<PointerDeviceType> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PointerDeviceType)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Position(&self) -> windows_core::Result<Point> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Position)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn TapCount(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TapCount)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct ITappedEventArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub PointerDeviceType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut PointerDeviceType,
    ) -> windows_core::HRESULT,
    pub Position:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Point) -> windows_core::HRESULT,
    pub TapCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
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
    pub unsafe fn OnTextChange(
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
    pub unsafe fn OnSelectionChange(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).OnSelectionChange)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn OnLayoutChange(
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
    pub unsafe fn OnStatusChange(&self, dwflags: u32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).OnStatusChange)(
                windows_core::Interface::as_raw(self),
                dwflags,
            )
        }
    }
    pub unsafe fn OnAttrsChange(
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
    pub unsafe fn OnLockGranted(&self, dwlockflags: u32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).OnLockGranted)(
                windows_core::Interface::as_raw(self),
                dwlockflags,
            )
        }
    }
    pub unsafe fn OnStartEditTransaction(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).OnStartEditTransaction)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn OnEndEditTransaction(&self) -> windows_core::HRESULT {
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
    ITfCompartmentMgr,
    ITfCompartmentMgr_Vtbl,
    0x7dcf57ac_18ad_438b_824d_979bffb74b7c
);
windows_core::imp::interface_hierarchy!(ITfCompartmentMgr, windows_core::IUnknown);
#[repr(C)]
pub struct ITfCompartmentMgr_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetCompartment: usize,
    ClearCompartment: usize,
    EnumCompartments: usize,
}
windows_core::imp::define_interface!(
    ITfCompositionView,
    ITfCompositionView_Vtbl,
    0xd7540241_f9a1_4364_befc_dbcd2c4395b7
);
windows_core::imp::interface_hierarchy!(ITfCompositionView, windows_core::IUnknown);
impl ITfCompositionView {
    pub unsafe fn GetOwnerClsid(&self) -> windows_core::Result<windows_core::GUID> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetOwnerClsid)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn GetRange(&self) -> windows_core::Result<ITfRange> {
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
    pub GetOwnerClsid: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::GUID,
    ) -> windows_core::HRESULT,
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
    pub unsafe fn RequestEditSession<P1>(
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
    pub unsafe fn InWriteSession(
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
    pub unsafe fn GetSelection(
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
    pub unsafe fn SetSelection(
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
    pub unsafe fn GetStart(&self, ec: TfEditCookie) -> windows_core::Result<ITfRange> {
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
    pub unsafe fn GetEnd(&self, ec: TfEditCookie) -> windows_core::Result<ITfRange> {
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
    pub unsafe fn GetActiveView(&self) -> windows_core::Result<ITfContextView> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetActiveView)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn EnumViews(&self) -> windows_core::Result<IEnumTfContextViews> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).EnumViews)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetStatus(&self) -> windows_core::Result<TF_STATUS> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetStatus)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn GetProperty(
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
    pub unsafe fn GetAppProperty(
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
    pub unsafe fn TrackProperties(
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
    pub unsafe fn EnumProperties(&self) -> windows_core::Result<IEnumTfProperties> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).EnumProperties)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetDocumentMgr(&self) -> windows_core::Result<ITfDocumentMgr> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetDocumentMgr)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn CreateRangeBackup<P1>(
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
    pub unsafe fn GetGUID(&self) -> windows_core::Result<windows_core::GUID> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetGUID)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn GetDescription(&self) -> windows_core::Result<windows_core::BSTR> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetDescription)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| core::mem::transmute(result__))
        }
    }
    pub unsafe fn GetAttributeInfo(&self, pda: *mut TF_DISPLAYATTRIBUTE) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetAttributeInfo)(
                windows_core::Interface::as_raw(self),
                pda as _,
            )
        }
    }
    pub unsafe fn SetAttributeInfo(
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
    pub unsafe fn Reset(&self) -> windows_core::HRESULT {
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
    pub unsafe fn OnUpdateInfo(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).OnUpdateInfo)(windows_core::Interface::as_raw(
                self,
            ))
        }
    }
    pub unsafe fn EnumDisplayAttributeInfo(
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
    pub unsafe fn GetDisplayAttributeInfo(
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
    pub unsafe fn EnumDisplayAttributeInfo(
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
    pub unsafe fn GetDisplayAttributeInfo(
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
    pub unsafe fn CreateContext<P2>(
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
    pub unsafe fn Push<P0>(&self, pic: P0) -> windows_core::HRESULT
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
    pub unsafe fn Pop(&self, dwflags: u32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Pop)(
                windows_core::Interface::as_raw(self),
                dwflags,
            )
        }
    }
    pub unsafe fn GetTop(&self) -> windows_core::Result<ITfContext> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetTop)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetBase(&self) -> windows_core::Result<ITfContext> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetBase)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn EnumContexts(&self) -> windows_core::Result<IEnumTfContexts> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).EnumContexts)(
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
    pub EnumContexts: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
    ITfFunctionProvider,
    ITfFunctionProvider_Vtbl,
    0x101d6610_0990_11d3_8df0_00105a2799b5
);
windows_core::imp::interface_hierarchy!(ITfFunctionProvider, windows_core::IUnknown);
#[repr(C)]
pub struct ITfFunctionProvider_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    GetType: usize,
    GetDescription: usize,
    GetFunction: usize,
}
windows_core::imp::define_interface!(
    ITfInputScope,
    ITfInputScope_Vtbl,
    0xfde1eaee_6924_4cdf_91e7_da38cff5559d
);
windows_core::imp::interface_hierarchy!(ITfInputScope, windows_core::IUnknown);
impl ITfInputScope {
    pub unsafe fn GetInputScopes(
        &self,
        pprginputscopes: *mut *mut InputScope,
        pccount: *mut u32,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetInputScopes)(
                windows_core::Interface::as_raw(self),
                pprginputscopes as _,
                pccount as _,
            )
        }
    }
    pub unsafe fn GetPhrase(
        &self,
        ppbstrphrases: *mut *mut windows_core::BSTR,
        pccount: *mut u32,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetPhrase)(
                windows_core::Interface::as_raw(self),
                ppbstrphrases as _,
                pccount as _,
            )
        }
    }
    pub unsafe fn GetRegularExpression(&self) -> windows_core::Result<windows_core::BSTR> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetRegularExpression)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| core::mem::transmute(result__))
        }
    }
    pub unsafe fn GetSRGS(&self) -> windows_core::Result<windows_core::BSTR> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetSRGS)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| core::mem::transmute(result__))
        }
    }
    pub unsafe fn GetXML(&self) -> windows_core::Result<windows_core::BSTR> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetXML)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| core::mem::transmute(result__))
        }
    }
}
#[repr(C)]
pub struct ITfInputScope_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetInputScopes: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut InputScope,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub GetPhrase: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut *mut core::ffi::c_void,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub GetRegularExpression: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetSRGS: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetXML: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
    pub unsafe fn AdviseKeyEventSink<P1>(
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
    pub unsafe fn UnadviseKeyEventSink(&self, tid: TfClientId) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).UnadviseKeyEventSink)(
                windows_core::Interface::as_raw(self),
                tid,
            )
        }
    }
    pub unsafe fn GetForeground(&self) -> windows_core::Result<windows_core::GUID> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetForeground)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn TestKeyDown(
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
    pub unsafe fn TestKeyUp(
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
    pub unsafe fn KeyDown(
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
    pub unsafe fn KeyUp(
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
    pub unsafe fn GetPreservedKey<P0>(
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
    pub unsafe fn IsPreservedKey(
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
    pub unsafe fn PreserveKey(
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
    pub unsafe fn UnpreserveKey(
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
    pub unsafe fn SetPreservedKeyDescription(
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
    pub unsafe fn GetPreservedKeyDescription(
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
    pub unsafe fn SimulatePreservedKey<P0>(
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
    pub unsafe fn GetExtent(&self, pacpanchor: *mut i32, pcch: *mut i32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).GetExtent)(
                windows_core::Interface::as_raw(self),
                pacpanchor as _,
                pcch as _,
            )
        }
    }
    pub unsafe fn SetExtent(&self, acpanchor: i32, cch: i32) -> windows_core::HRESULT {
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
    pub unsafe fn AdviseSink<P1>(
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
    pub unsafe fn UnadviseSink(&self, dwcookie: u32) -> windows_core::HRESULT {
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
    pub unsafe fn Activate(&self) -> windows_core::Result<TfClientId> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Activate)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn Deactivate(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Deactivate)(windows_core::Interface::as_raw(
                self,
            ))
        }
    }
    pub unsafe fn CreateDocumentMgr(&self) -> windows_core::Result<ITfDocumentMgr> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateDocumentMgr)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn EnumDocumentMgrs(&self) -> windows_core::Result<IEnumTfDocumentMgrs> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).EnumDocumentMgrs)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetFocus(&self) -> windows_core::Result<ITfDocumentMgr> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFocus)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn SetFocus<P0>(&self, pdimfocus: P0) -> windows_core::HRESULT
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
    pub unsafe fn AssociateFocus<P1>(
        &self,
        hwnd: HWND,
        pdimnew: P1,
    ) -> windows_core::Result<ITfDocumentMgr>
    where
        P1: windows_core::Param<ITfDocumentMgr>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).AssociateFocus)(
                windows_core::Interface::as_raw(self),
                hwnd,
                pdimnew.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn IsThreadFocus(&self) -> windows_core::Result<windows_core::BOOL> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsThreadFocus)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn GetFunctionProvider(
        &self,
        clsid: *const windows_core::GUID,
    ) -> windows_core::Result<ITfFunctionProvider> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFunctionProvider)(
                windows_core::Interface::as_raw(self),
                clsid,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn EnumFunctionProviders(&self) -> windows_core::Result<IEnumTfFunctionProviders> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).EnumFunctionProviders)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetGlobalCompartment(&self) -> windows_core::Result<ITfCompartmentMgr> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetGlobalCompartment)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
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
    pub EnumDocumentMgrs: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetFocus: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetFocus: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub AssociateFocus: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        HWND,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub IsThreadFocus: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub GetFunctionProvider: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub EnumFunctionProviders: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetGlobalCompartment: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ITfThreadMgr2,
    ITfThreadMgr2_Vtbl,
    0x0ab198ef_6477_4ee8_8812_6780edb82d5e
);
windows_core::imp::interface_hierarchy!(ITfThreadMgr2, windows_core::IUnknown);
impl ITfThreadMgr2 {
    pub unsafe fn Activate(&self) -> windows_core::Result<TfClientId> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Activate)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn Deactivate(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).Deactivate)(windows_core::Interface::as_raw(
                self,
            ))
        }
    }
    pub unsafe fn CreateDocumentMgr(&self) -> windows_core::Result<ITfDocumentMgr> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateDocumentMgr)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn EnumDocumentMgrs(&self) -> windows_core::Result<IEnumTfDocumentMgrs> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).EnumDocumentMgrs)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetFocus(&self) -> windows_core::Result<ITfDocumentMgr> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFocus)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn SetFocus<P0>(&self, pdimfocus: P0) -> windows_core::HRESULT
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
    pub unsafe fn IsThreadFocus(&self) -> windows_core::Result<windows_core::BOOL> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsThreadFocus)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn GetFunctionProvider(
        &self,
        clsid: *const windows_core::GUID,
    ) -> windows_core::Result<ITfFunctionProvider> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetFunctionProvider)(
                windows_core::Interface::as_raw(self),
                clsid,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn EnumFunctionProviders(&self) -> windows_core::Result<IEnumTfFunctionProviders> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).EnumFunctionProviders)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn GetGlobalCompartment(&self) -> windows_core::Result<ITfCompartmentMgr> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetGlobalCompartment)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub unsafe fn ActivateEx(&self, ptid: *mut TfClientId, dwflags: u32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).ActivateEx)(
                windows_core::Interface::as_raw(self),
                ptid as _,
                dwflags,
            )
        }
    }
    pub unsafe fn GetActiveFlags(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetActiveFlags)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub unsafe fn SuspendKeystrokeHandling(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).SuspendKeystrokeHandling)(
                windows_core::Interface::as_raw(self),
            )
        }
    }
    pub unsafe fn ResumeKeystrokeHandling(&self) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).ResumeKeystrokeHandling)(
                windows_core::Interface::as_raw(self),
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
    pub EnumDocumentMgrs: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetFocus: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetFocus: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub IsThreadFocus: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::BOOL,
    ) -> windows_core::HRESULT,
    pub GetFunctionProvider: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub EnumFunctionProviders: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetGlobalCompartment: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ActivateEx: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut TfClientId,
        u32,
    ) -> windows_core::HRESULT,
    pub GetActiveFlags:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub SuspendKeystrokeHandling:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub ResumeKeystrokeHandling:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ITfThreadMgrEx,
    ITfThreadMgrEx_Vtbl,
    0x3e90ade3_7594_4cb0_bb58_69628f5f458c
);
impl core::ops::Deref for ITfThreadMgrEx {
    type Target = ITfThreadMgr;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(ITfThreadMgrEx, windows_core::IUnknown, ITfThreadMgr);
impl ITfThreadMgrEx {
    pub unsafe fn ActivateEx(&self, ptid: *mut TfClientId, dwflags: u32) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).ActivateEx)(
                windows_core::Interface::as_raw(self),
                ptid as _,
                dwflags,
            )
        }
    }
    pub unsafe fn GetActiveFlags(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).GetActiveFlags)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct ITfThreadMgrEx_Vtbl {
    pub base__: ITfThreadMgr_Vtbl,
    pub ActivateEx: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut TfClientId,
        u32,
    ) -> windows_core::HRESULT,
    pub GetActiveFlags:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
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
    ITouchCapabilities,
    ITouchCapabilities_Vtbl,
    0x20dd55f9_13f1_46c8_9285_2c05fa3eda6f
);
impl windows_core::RuntimeType for ITouchCapabilities {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ITouchCapabilities {
    pub fn TouchPresent(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TouchPresent)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn Contacts(&self) -> windows_core::Result<u32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Contacts)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct ITouchCapabilities_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub TouchPresent:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub Contacts:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IUIViewSettings,
    IUIViewSettings_Vtbl,
    0xc63657f6_8850_470d_88f8_455e16ea2c26
);
impl windows_core::RuntimeType for IUIViewSettings {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IUIViewSettings {
    pub fn UserInteractionMode(&self) -> windows_core::Result<UserInteractionMode> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).UserInteractionMode)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IUIViewSettings_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub UserInteractionMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut UserInteractionMode,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IUIViewSettingsInterop,
    IUIViewSettingsInterop_Vtbl,
    0x3694dbf9_8f68_44be_8ff5_195c98ede8a6
);
windows_core::imp::interface_hierarchy!(
    IUIViewSettingsInterop,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl IUIViewSettingsInterop {
    pub unsafe fn GetForWindow<T>(&self, hwnd: HWND) -> windows_core::Result<T>
    where
        T: windows_core::Interface,
    {
        let mut result__ = core::ptr::null_mut();
        unsafe {
            (windows_core::Interface::vtable(self).GetForWindow)(
                windows_core::Interface::as_raw(self),
                hwnd,
                &T::IID,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IUIViewSettingsInterop_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub GetForWindow: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        HWND,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
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
pub type InputScope = i32;
pub type LPARAM = isize;
pub type LRESULT = isize;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManipulationCompletedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ManipulationCompletedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for ManipulationCompletedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IManipulationCompletedEventArgs>();
}
unsafe impl windows_core::Interface for ManipulationCompletedEventArgs {
    type Vtable = <IManipulationCompletedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IManipulationCompletedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for ManipulationCompletedEventArgs {
    type Target = IManipulationCompletedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ManipulationCompletedEventArgs {
    const NAME: &'static str = "Windows.UI.Input.ManipulationCompletedEventArgs";
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ManipulationDelta {
    pub translation: Point,
    pub scale: f32,
    pub rotation: f32,
    pub expansion: f32,
}
impl windows_core::TypeKind for ManipulationDelta {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for ManipulationDelta {
    const SIGNATURE : windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice (b"struct(Windows.UI.Input.ManipulationDelta;struct(Windows.Foundation.Point;f4;f4);f4;f4;f4)") ;
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManipulationInertiaStartingEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ManipulationInertiaStartingEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for ManipulationInertiaStartingEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IManipulationInertiaStartingEventArgs>();
}
unsafe impl windows_core::Interface for ManipulationInertiaStartingEventArgs {
    type Vtable = <IManipulationInertiaStartingEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IManipulationInertiaStartingEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for ManipulationInertiaStartingEventArgs {
    type Target = IManipulationInertiaStartingEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ManipulationInertiaStartingEventArgs {
    const NAME: &'static str = "Windows.UI.Input.ManipulationInertiaStartingEventArgs";
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManipulationStartedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ManipulationStartedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for ManipulationStartedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IManipulationStartedEventArgs>();
}
unsafe impl windows_core::Interface for ManipulationStartedEventArgs {
    type Vtable = <IManipulationStartedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IManipulationStartedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for ManipulationStartedEventArgs {
    type Target = IManipulationStartedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ManipulationStartedEventArgs {
    const NAME: &'static str = "Windows.UI.Input.ManipulationStartedEventArgs";
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManipulationUpdatedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ManipulationUpdatedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for ManipulationUpdatedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IManipulationUpdatedEventArgs>();
}
unsafe impl windows_core::Interface for ManipulationUpdatedEventArgs {
    type Vtable = <IManipulationUpdatedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IManipulationUpdatedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for ManipulationUpdatedEventArgs {
    type Target = IManipulationUpdatedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ManipulationUpdatedEventArgs {
    const NAME: &'static str = "Windows.UI.Input.ManipulationUpdatedEventArgs";
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ManipulationVelocities {
    pub linear: Point,
    pub angular: f32,
    pub expansion: f32,
}
impl windows_core::TypeKind for ManipulationVelocities {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for ManipulationVelocities {
    const SIGNATURE : windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice (b"struct(Windows.UI.Input.ManipulationVelocities;struct(Windows.Foundation.Point;f4;f4);f4;f4)") ;
}
pub type NavigateDirection = i32;
pub const NavigateDirection_FirstChild: NavigateDirection = 3;
pub const NavigateDirection_LastChild: NavigateDirection = 4;
pub const NavigateDirection_NextSibling: NavigateDirection = 1;
pub const NavigateDirection_Parent: NavigateDirection = 0;
pub const NavigateDirection_PreviousSibling: NavigateDirection = 2;
pub type PATTERNID = i32;
pub type PEN_FLAGS = u32;
pub const PEN_FLAG_BARREL: i32 = 1;
pub const PEN_FLAG_ERASER: i32 = 4;
pub const PEN_FLAG_INVERTED: i32 = 2;
pub type PEN_MASK = u32;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}
pub type POINTER_BUTTON_CHANGE_TYPE = i32;
pub const POINTER_CHANGE_FIFTHBUTTON_DOWN: POINTER_BUTTON_CHANGE_TYPE = 9;
pub const POINTER_CHANGE_FIFTHBUTTON_UP: POINTER_BUTTON_CHANGE_TYPE = 10;
pub const POINTER_CHANGE_FIRSTBUTTON_DOWN: POINTER_BUTTON_CHANGE_TYPE = 1;
pub const POINTER_CHANGE_FIRSTBUTTON_UP: POINTER_BUTTON_CHANGE_TYPE = 2;
pub const POINTER_CHANGE_FOURTHBUTTON_DOWN: POINTER_BUTTON_CHANGE_TYPE = 7;
pub const POINTER_CHANGE_FOURTHBUTTON_UP: POINTER_BUTTON_CHANGE_TYPE = 8;
pub const POINTER_CHANGE_NONE: POINTER_BUTTON_CHANGE_TYPE = 0;
pub const POINTER_CHANGE_SECONDBUTTON_DOWN: POINTER_BUTTON_CHANGE_TYPE = 3;
pub const POINTER_CHANGE_SECONDBUTTON_UP: POINTER_BUTTON_CHANGE_TYPE = 4;
pub const POINTER_CHANGE_THIRDBUTTON_DOWN: POINTER_BUTTON_CHANGE_TYPE = 5;
pub const POINTER_CHANGE_THIRDBUTTON_UP: POINTER_BUTTON_CHANGE_TYPE = 6;
pub type POINTER_FLAGS = u32;
pub const POINTER_FLAG_CANCELED: i32 = 32768;
pub const POINTER_FLAG_CONFIDENCE: i32 = 16384;
pub const POINTER_FLAG_FIFTHBUTTON: i32 = 256;
pub const POINTER_FLAG_FIRSTBUTTON: i32 = 16;
pub const POINTER_FLAG_FOURTHBUTTON: i32 = 128;
pub const POINTER_FLAG_INCONTACT: i32 = 4;
pub const POINTER_FLAG_INRANGE: i32 = 2;
pub const POINTER_FLAG_NEW: i32 = 1;
pub const POINTER_FLAG_PRIMARY: i32 = 8192;
pub const POINTER_FLAG_SECONDBUTTON: i32 = 32;
pub const POINTER_FLAG_THIRDBUTTON: i32 = 64;
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
pub type PROPERTYID = i32;
pub const PT_MOUSE: tagPOINTER_INPUT_TYPE = 4;
pub const PT_PEN: tagPOINTER_INPUT_TYPE = 3;
pub const PT_TOUCH: tagPOINTER_INPUT_TYPE = 2;
pub const PT_TOUCHPAD: tagPOINTER_INPUT_TYPE = 5;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalGestureRecognizer(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    PhysicalGestureRecognizer,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl PhysicalGestureRecognizer {
    pub fn new() -> windows_core::Result<Self> {
        Self::IActivationFactory(|f| f.ActivateInstance::<Self>())
    }
    fn IActivationFactory<
        R,
        F: FnOnce(&windows_core::imp::IGenericFactory) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<
            PhysicalGestureRecognizer,
            windows_core::imp::IGenericFactory,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for PhysicalGestureRecognizer {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IPhysicalGestureRecognizer>();
}
unsafe impl windows_core::Interface for PhysicalGestureRecognizer {
    type Vtable = <IPhysicalGestureRecognizer as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPhysicalGestureRecognizer as windows_core::Interface>::IID;
}
impl core::ops::Deref for PhysicalGestureRecognizer {
    type Target = IPhysicalGestureRecognizer;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for PhysicalGestureRecognizer {
    const NAME: &'static str = "Windows.UI.Input.PhysicalGestureRecognizer";
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
impl windows_core::TypeKind for Point {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for Point {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.Foundation.Point;f4;f4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointerDevice(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    PointerDevice,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl PointerDevice {
    pub fn GetPointerDevice(pointerid: u32) -> windows_core::Result<Self> {
        Self::IPointerDeviceStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetPointerDevice)(
                windows_core::Interface::as_raw(this),
                pointerid,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    pub fn GetPointerDevices() -> windows_core::Result<windows_collections::IVectorView<Self>> {
        Self::IPointerDeviceStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetPointerDevices)(
                windows_core::Interface::as_raw(this),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    fn IPointerDeviceStatics<R, F: FnOnce(&IPointerDeviceStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<PointerDevice, IPointerDeviceStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for PointerDevice {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IPointerDevice>();
}
unsafe impl windows_core::Interface for PointerDevice {
    type Vtable = <IPointerDevice as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPointerDevice as windows_core::Interface>::IID;
}
impl core::ops::Deref for PointerDevice {
    type Target = IPointerDevice;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for PointerDevice {
    const NAME: &'static str = "Windows.Devices.Input.PointerDevice";
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PointerDeviceType(pub i32);
impl PointerDeviceType {
    pub const Touch: Self = Self(0);
    pub const Pen: Self = Self(1);
    pub const Mouse: Self = Self(2);
    pub const Touchpad: Self = Self(3);
}
impl windows_core::TypeKind for PointerDeviceType {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for PointerDeviceType {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.Devices.Input.PointerDeviceType;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointerPoint(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    PointerPoint,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl PointerPoint {
    pub fn GetCurrentPoint(pointerid: u32) -> windows_core::Result<Self> {
        Self::IPointerPointStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetCurrentPoint)(
                windows_core::Interface::as_raw(this),
                pointerid,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    pub fn GetIntermediatePoints(
        pointerid: u32,
    ) -> windows_core::Result<windows_collections::IVector<Self>> {
        Self::IPointerPointStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetIntermediatePoints)(
                windows_core::Interface::as_raw(this),
                pointerid,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    pub fn GetCurrentPointTransformed<P1>(
        pointerid: u32,
        transform: P1,
    ) -> windows_core::Result<Self>
    where
        P1: windows_core::Param<IPointerPointTransform>,
    {
        Self::IPointerPointStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetCurrentPointTransformed)(
                windows_core::Interface::as_raw(this),
                pointerid,
                transform.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    pub fn GetIntermediatePointsTransformed<P1>(
        pointerid: u32,
        transform: P1,
    ) -> windows_core::Result<windows_collections::IVector<Self>>
    where
        P1: windows_core::Param<IPointerPointTransform>,
    {
        Self::IPointerPointStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetIntermediatePointsTransformed)(
                windows_core::Interface::as_raw(this),
                pointerid,
                transform.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    fn IPointerPointStatics<R, F: FnOnce(&IPointerPointStatics) -> windows_core::Result<R>>(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<PointerPoint, IPointerPointStatics> =
            windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for PointerPoint {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IPointerPoint>();
}
unsafe impl windows_core::Interface for PointerPoint {
    type Vtable = <IPointerPoint as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPointerPoint as windows_core::Interface>::IID;
}
impl core::ops::Deref for PointerPoint {
    type Target = IPointerPoint;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for PointerPoint {
    const NAME: &'static str = "Windows.UI.Input.PointerPoint";
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointerPointProperties(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    PointerPointProperties,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for PointerPointProperties {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IPointerPointProperties>();
}
unsafe impl windows_core::Interface for PointerPointProperties {
    type Vtable = <IPointerPointProperties as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IPointerPointProperties as windows_core::Interface>::IID;
}
impl core::ops::Deref for PointerPointProperties {
    type Target = IPointerPointProperties;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for PointerPointProperties {
    const NAME: &'static str = "Windows.UI.Input.PointerPointProperties";
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PointerUpdateKind(pub i32);
impl PointerUpdateKind {
    pub const Other: Self = Self(0);
    pub const LeftButtonPressed: Self = Self(1);
    pub const LeftButtonReleased: Self = Self(2);
    pub const RightButtonPressed: Self = Self(3);
    pub const RightButtonReleased: Self = Self(4);
    pub const MiddleButtonPressed: Self = Self(5);
    pub const MiddleButtonReleased: Self = Self(6);
    pub const XButton1Pressed: Self = Self(7);
    pub const XButton1Released: Self = Self(8);
    pub const XButton2Pressed: Self = Self(9);
    pub const XButton2Released: Self = Self(10);
}
impl windows_core::TypeKind for PointerUpdateKind {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for PointerUpdateKind {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"enum(Windows.UI.Input.PointerUpdateKind;i4)");
}
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadialController(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RadialController,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for RadialController {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRadialController>();
}
unsafe impl windows_core::Interface for RadialController {
    type Vtable = <IRadialController as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IRadialController as windows_core::Interface>::IID;
}
impl core::ops::Deref for RadialController {
    type Target = IRadialController;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RadialController {
    const NAME: &'static str = "Windows.UI.Input.RadialController";
}
unsafe impl Send for RadialController {}
unsafe impl Sync for RadialController {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadialControllerButtonClickedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RadialControllerButtonClickedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for RadialControllerButtonClickedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRadialControllerButtonClickedEventArgs>(
        );
}
unsafe impl windows_core::Interface for RadialControllerButtonClickedEventArgs {
    type Vtable = <IRadialControllerButtonClickedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IRadialControllerButtonClickedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for RadialControllerButtonClickedEventArgs {
    type Target = IRadialControllerButtonClickedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RadialControllerButtonClickedEventArgs {
    const NAME: &'static str = "Windows.UI.Input.RadialControllerButtonClickedEventArgs";
}
unsafe impl Send for RadialControllerButtonClickedEventArgs {}
unsafe impl Sync for RadialControllerButtonClickedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadialControllerButtonPressedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RadialControllerButtonPressedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for RadialControllerButtonPressedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRadialControllerButtonPressedEventArgs>(
        );
}
unsafe impl windows_core::Interface for RadialControllerButtonPressedEventArgs {
    type Vtable = <IRadialControllerButtonPressedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IRadialControllerButtonPressedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for RadialControllerButtonPressedEventArgs {
    type Target = IRadialControllerButtonPressedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RadialControllerButtonPressedEventArgs {
    const NAME: &'static str = "Windows.UI.Input.RadialControllerButtonPressedEventArgs";
}
unsafe impl Send for RadialControllerButtonPressedEventArgs {}
unsafe impl Sync for RadialControllerButtonPressedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadialControllerButtonReleasedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RadialControllerButtonReleasedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for RadialControllerButtonReleasedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<
        Self,
        IRadialControllerButtonReleasedEventArgs,
    >();
}
unsafe impl windows_core::Interface for RadialControllerButtonReleasedEventArgs {
    type Vtable = <IRadialControllerButtonReleasedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IRadialControllerButtonReleasedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for RadialControllerButtonReleasedEventArgs {
    type Target = IRadialControllerButtonReleasedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RadialControllerButtonReleasedEventArgs {
    const NAME: &'static str = "Windows.UI.Input.RadialControllerButtonReleasedEventArgs";
}
unsafe impl Send for RadialControllerButtonReleasedEventArgs {}
unsafe impl Sync for RadialControllerButtonReleasedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadialControllerConfiguration(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RadialControllerConfiguration,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for RadialControllerConfiguration {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRadialControllerConfiguration>();
}
unsafe impl windows_core::Interface for RadialControllerConfiguration {
    type Vtable = <IRadialControllerConfiguration as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IRadialControllerConfiguration as windows_core::Interface>::IID;
}
impl core::ops::Deref for RadialControllerConfiguration {
    type Target = IRadialControllerConfiguration;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RadialControllerConfiguration {
    const NAME: &'static str = "Windows.UI.Input.RadialControllerConfiguration";
}
unsafe impl Send for RadialControllerConfiguration {}
unsafe impl Sync for RadialControllerConfiguration {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadialControllerControlAcquiredEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RadialControllerControlAcquiredEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for RadialControllerControlAcquiredEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<
        Self,
        IRadialControllerControlAcquiredEventArgs,
    >();
}
unsafe impl windows_core::Interface for RadialControllerControlAcquiredEventArgs {
    type Vtable = <IRadialControllerControlAcquiredEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IRadialControllerControlAcquiredEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for RadialControllerControlAcquiredEventArgs {
    type Target = IRadialControllerControlAcquiredEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RadialControllerControlAcquiredEventArgs {
    const NAME: &'static str = "Windows.UI.Input.RadialControllerControlAcquiredEventArgs";
}
unsafe impl Send for RadialControllerControlAcquiredEventArgs {}
unsafe impl Sync for RadialControllerControlAcquiredEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadialControllerMenu(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RadialControllerMenu,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for RadialControllerMenu {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRadialControllerMenu>();
}
unsafe impl windows_core::Interface for RadialControllerMenu {
    type Vtable = <IRadialControllerMenu as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IRadialControllerMenu as windows_core::Interface>::IID;
}
impl core::ops::Deref for RadialControllerMenu {
    type Target = IRadialControllerMenu;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RadialControllerMenu {
    const NAME: &'static str = "Windows.UI.Input.RadialControllerMenu";
}
unsafe impl Send for RadialControllerMenu {}
unsafe impl Sync for RadialControllerMenu {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadialControllerMenuItem(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RadialControllerMenuItem,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl RadialControllerMenuItem {
    pub fn CreateFromFontGlyph(
        displaytext: &str,
        glyph: &str,
        fontfamily: &str,
    ) -> windows_core::Result<Self> {
        Self::IRadialControllerMenuItemStatics2(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateFromFontGlyph)(
                windows_core::Interface::as_raw(this),
                core::mem::transmute_copy(&windows_core::HSTRING::from(displaytext)),
                core::mem::transmute_copy(&windows_core::HSTRING::from(glyph)),
                core::mem::transmute_copy(&windows_core::HSTRING::from(fontfamily)),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    fn IRadialControllerMenuItemStatics2<
        R,
        F: FnOnce(&IRadialControllerMenuItemStatics2) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<
            RadialControllerMenuItem,
            IRadialControllerMenuItemStatics2,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for RadialControllerMenuItem {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRadialControllerMenuItem>();
}
unsafe impl windows_core::Interface for RadialControllerMenuItem {
    type Vtable = <IRadialControllerMenuItem as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IRadialControllerMenuItem as windows_core::Interface>::IID;
}
impl core::ops::Deref for RadialControllerMenuItem {
    type Target = IRadialControllerMenuItem;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RadialControllerMenuItem {
    const NAME: &'static str = "Windows.UI.Input.RadialControllerMenuItem";
}
unsafe impl Send for RadialControllerMenuItem {}
unsafe impl Sync for RadialControllerMenuItem {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadialControllerRotationChangedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RadialControllerRotationChangedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for RadialControllerRotationChangedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<
        Self,
        IRadialControllerRotationChangedEventArgs,
    >();
}
unsafe impl windows_core::Interface for RadialControllerRotationChangedEventArgs {
    type Vtable = <IRadialControllerRotationChangedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IRadialControllerRotationChangedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for RadialControllerRotationChangedEventArgs {
    type Target = IRadialControllerRotationChangedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RadialControllerRotationChangedEventArgs {
    const NAME: &'static str = "Windows.UI.Input.RadialControllerRotationChangedEventArgs";
}
unsafe impl Send for RadialControllerRotationChangedEventArgs {}
unsafe impl Sync for RadialControllerRotationChangedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadialControllerScreenContact(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RadialControllerScreenContact,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for RadialControllerScreenContact {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRadialControllerScreenContact>();
}
unsafe impl windows_core::Interface for RadialControllerScreenContact {
    type Vtable = <IRadialControllerScreenContact as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IRadialControllerScreenContact as windows_core::Interface>::IID;
}
impl core::ops::Deref for RadialControllerScreenContact {
    type Target = IRadialControllerScreenContact;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RadialControllerScreenContact {
    const NAME: &'static str = "Windows.UI.Input.RadialControllerScreenContact";
}
unsafe impl Send for RadialControllerScreenContact {}
unsafe impl Sync for RadialControllerScreenContact {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadialControllerScreenContactContinuedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RadialControllerScreenContactContinuedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for RadialControllerScreenContactContinuedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<
        Self,
        IRadialControllerScreenContactContinuedEventArgs,
    >();
}
unsafe impl windows_core::Interface for RadialControllerScreenContactContinuedEventArgs {
    type Vtable =
        <IRadialControllerScreenContactContinuedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IRadialControllerScreenContactContinuedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for RadialControllerScreenContactContinuedEventArgs {
    type Target = IRadialControllerScreenContactContinuedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RadialControllerScreenContactContinuedEventArgs {
    const NAME: &'static str = "Windows.UI.Input.RadialControllerScreenContactContinuedEventArgs";
}
unsafe impl Send for RadialControllerScreenContactContinuedEventArgs {}
unsafe impl Sync for RadialControllerScreenContactContinuedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadialControllerScreenContactStartedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RadialControllerScreenContactStartedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for RadialControllerScreenContactStartedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<
        Self,
        IRadialControllerScreenContactStartedEventArgs,
    >();
}
unsafe impl windows_core::Interface for RadialControllerScreenContactStartedEventArgs {
    type Vtable =
        <IRadialControllerScreenContactStartedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IRadialControllerScreenContactStartedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for RadialControllerScreenContactStartedEventArgs {
    type Target = IRadialControllerScreenContactStartedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RadialControllerScreenContactStartedEventArgs {
    const NAME: &'static str = "Windows.UI.Input.RadialControllerScreenContactStartedEventArgs";
}
unsafe impl Send for RadialControllerScreenContactStartedEventArgs {}
unsafe impl Sync for RadialControllerScreenContactStartedEventArgs {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RadialControllerSystemMenuItemKind(pub i32);
impl RadialControllerSystemMenuItemKind {
    pub const Scroll: Self = Self(0);
    pub const Zoom: Self = Self(1);
    pub const UndoRedo: Self = Self(2);
    pub const Volume: Self = Self(3);
    pub const NextPreviousTrack: Self = Self(4);
}
impl windows_core::TypeKind for RadialControllerSystemMenuItemKind {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for RadialControllerSystemMenuItemKind {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Input.RadialControllerSystemMenuItemKind;i4)",
    );
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl windows_core::TypeKind for Rect {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for Rect {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.Foundation.Rect;f4;f4;f4;f4)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RightTappedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RightTappedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for RightTappedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRightTappedEventArgs>();
}
unsafe impl windows_core::Interface for RightTappedEventArgs {
    type Vtable = <IRightTappedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IRightTappedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for RightTappedEventArgs {
    type Target = IRightTappedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RightTappedEventArgs {
    const NAME: &'static str = "Windows.UI.Input.RightTappedEventArgs";
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
pub type ScrollAmount = i32;
pub const ScrollAmount_LargeDecrement: ScrollAmount = 0;
pub const ScrollAmount_LargeIncrement: ScrollAmount = 3;
pub const ScrollAmount_NoAmount: ScrollAmount = 2;
pub const ScrollAmount_SmallDecrement: ScrollAmount = 1;
pub const ScrollAmount_SmallIncrement: ScrollAmount = 4;
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
pub type TOUCH_FLAGS = u32;
pub type TOUCH_MASK = u32;
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
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TappedEventArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    TappedEventArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for TappedEventArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ITappedEventArgs>();
}
unsafe impl windows_core::Interface for TappedEventArgs {
    type Vtable = <ITappedEventArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ITappedEventArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for TappedEventArgs {
    type Target = ITappedEventArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for TappedEventArgs {
    const NAME: &'static str = "Windows.UI.Input.TappedEventArgs";
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
pub type ToggleState = i32;
pub const ToggleState_Indeterminate: ToggleState = 2;
pub const ToggleState_Off: ToggleState = 0;
pub const ToggleState_On: ToggleState = 1;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TouchCapabilities(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    TouchCapabilities,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for TouchCapabilities {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ITouchCapabilities>();
}
unsafe impl windows_core::Interface for TouchCapabilities {
    type Vtable = <ITouchCapabilities as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ITouchCapabilities as windows_core::Interface>::IID;
}
impl core::ops::Deref for TouchCapabilities {
    type Target = ITouchCapabilities;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for TouchCapabilities {
    const NAME: &'static str = "Windows.Devices.Input.TouchCapabilities";
}
unsafe impl Send for TouchCapabilities {}
unsafe impl Sync for TouchCapabilities {}
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
pub const UIA_AcceleratorKeyPropertyId: i32 = 30006;
pub const UIA_AccessKeyPropertyId: i32 = 30007;
pub const UIA_ActiveTextPositionChangedEventId: i32 = 20036;
pub const UIA_AfterParagraphSpacingAttributeId: i32 = 40042;
pub const UIA_AnimationStyleAttributeId: i32 = 40000;
pub const UIA_AnnotationAnnotationTypeIdPropertyId: i32 = 30113;
pub const UIA_AnnotationAnnotationTypeNamePropertyId: i32 = 30114;
pub const UIA_AnnotationAuthorPropertyId: i32 = 30115;
pub const UIA_AnnotationDateTimePropertyId: i32 = 30116;
pub const UIA_AnnotationObjectsAttributeId: i32 = 40032;
pub const UIA_AnnotationObjectsPropertyId: i32 = 30156;
pub const UIA_AnnotationPatternId: i32 = 10023;
pub const UIA_AnnotationTargetPropertyId: i32 = 30117;
pub const UIA_AnnotationTypesAttributeId: i32 = 40031;
pub const UIA_AnnotationTypesPropertyId: i32 = 30155;
pub const UIA_AppBarControlTypeId: i32 = 50040;
pub const UIA_AriaPropertiesPropertyId: i32 = 30102;
pub const UIA_AriaRolePropertyId: i32 = 30101;
pub const UIA_AsyncContentLoadedEventId: i32 = 20006;
pub const UIA_AutomationFocusChangedEventId: i32 = 20005;
pub const UIA_AutomationIdPropertyId: i32 = 30011;
pub const UIA_AutomationPropertyChangedEventId: i32 = 20004;
pub const UIA_BackgroundColorAttributeId: i32 = 40001;
pub const UIA_BeforeParagraphSpacingAttributeId: i32 = 40041;
pub const UIA_BoundingRectanglePropertyId: i32 = 30001;
pub const UIA_BulletStyleAttributeId: i32 = 40002;
pub const UIA_ButtonControlTypeId: i32 = 50000;
pub const UIA_CalendarControlTypeId: i32 = 50001;
pub const UIA_CapStyleAttributeId: i32 = 40003;
pub const UIA_CaretBidiModeAttributeId: i32 = 40039;
pub const UIA_CaretPositionAttributeId: i32 = 40038;
pub const UIA_CenterPointPropertyId: i32 = 30165;
pub const UIA_ChangesEventId: i32 = 20034;
pub const UIA_CheckBoxControlTypeId: i32 = 50002;
pub const UIA_ClassNamePropertyId: i32 = 30012;
pub const UIA_ClickablePointPropertyId: i32 = 30014;
pub const UIA_ComboBoxControlTypeId: i32 = 50003;
pub const UIA_ControlTypePropertyId: i32 = 30003;
pub const UIA_ControllerForPropertyId: i32 = 30104;
pub const UIA_CultureAttributeId: i32 = 40004;
pub const UIA_CulturePropertyId: i32 = 30015;
pub const UIA_CustomControlTypeId: i32 = 50025;
pub const UIA_CustomLandmarkTypeId: i32 = 80000;
pub const UIA_CustomNavigationPatternId: i32 = 10033;
pub const UIA_DataGridControlTypeId: i32 = 50028;
pub const UIA_DataItemControlTypeId: i32 = 50029;
pub const UIA_DescribedByPropertyId: i32 = 30105;
pub const UIA_DockDockPositionPropertyId: i32 = 30069;
pub const UIA_DockPatternId: i32 = 10011;
pub const UIA_DocumentControlTypeId: i32 = 50030;
pub const UIA_DragDropEffectPropertyId: i32 = 30139;
pub const UIA_DragDropEffectsPropertyId: i32 = 30140;
pub const UIA_DragGrabbedItemsPropertyId: i32 = 30144;
pub const UIA_DragIsGrabbedPropertyId: i32 = 30138;
pub const UIA_DragPatternId: i32 = 10030;
pub const UIA_Drag_DragCancelEventId: i32 = 20027;
pub const UIA_Drag_DragCompleteEventId: i32 = 20028;
pub const UIA_Drag_DragStartEventId: i32 = 20026;
pub const UIA_DropTargetDropTargetEffectPropertyId: i32 = 30142;
pub const UIA_DropTargetDropTargetEffectsPropertyId: i32 = 30143;
pub const UIA_DropTargetPatternId: i32 = 10031;
pub const UIA_DropTarget_DragEnterEventId: i32 = 20029;
pub const UIA_DropTarget_DragLeaveEventId: i32 = 20030;
pub const UIA_DropTarget_DroppedEventId: i32 = 20031;
pub const UIA_E_ELEMENTNOTAVAILABLE: u32 = 2147746305;
pub const UIA_E_ELEMENTNOTENABLED: u32 = 2147746304;
pub const UIA_E_INVALIDOPERATION: u32 = 2148734217;
pub const UIA_E_NOCLICKABLEPOINT: u32 = 2147746306;
pub const UIA_E_NOTSUPPORTED: u32 = 2147746308;
pub const UIA_E_PROXYASSEMBLYNOTLOADED: u32 = 2147746307;
pub const UIA_E_TIMEOUT: u32 = 2148734213;
pub const UIA_EditControlTypeId: i32 = 50004;
pub const UIA_ExpandCollapseExpandCollapseStatePropertyId: i32 = 30070;
pub const UIA_ExpandCollapsePatternId: i32 = 10005;
pub const UIA_FillColorPropertyId: i32 = 30160;
pub const UIA_FillTypePropertyId: i32 = 30162;
pub const UIA_FlowsFromPropertyId: i32 = 30148;
pub const UIA_FlowsToPropertyId: i32 = 30106;
pub const UIA_FontNameAttributeId: i32 = 40005;
pub const UIA_FontSizeAttributeId: i32 = 40006;
pub const UIA_FontWeightAttributeId: i32 = 40007;
pub const UIA_ForegroundColorAttributeId: i32 = 40008;
pub const UIA_FormLandmarkTypeId: i32 = 80001;
pub const UIA_FrameworkIdPropertyId: i32 = 30024;
pub const UIA_FullDescriptionPropertyId: i32 = 30159;
pub const UIA_GridColumnCountPropertyId: i32 = 30063;
pub const UIA_GridItemColumnPropertyId: i32 = 30065;
pub const UIA_GridItemColumnSpanPropertyId: i32 = 30067;
pub const UIA_GridItemContainingGridPropertyId: i32 = 30068;
pub const UIA_GridItemPatternId: i32 = 10007;
pub const UIA_GridItemRowPropertyId: i32 = 30064;
pub const UIA_GridItemRowSpanPropertyId: i32 = 30066;
pub const UIA_GridPatternId: i32 = 10006;
pub const UIA_GridRowCountPropertyId: i32 = 30062;
pub const UIA_GroupControlTypeId: i32 = 50026;
pub const UIA_HasKeyboardFocusPropertyId: i32 = 30008;
pub const UIA_HeaderControlTypeId: i32 = 50034;
pub const UIA_HeaderItemControlTypeId: i32 = 50035;
pub const UIA_HeadingLevelPropertyId: i32 = 30173;
pub const UIA_HelpTextPropertyId: i32 = 30013;
pub const UIA_HorizontalTextAlignmentAttributeId: i32 = 40009;
pub const UIA_HostedFragmentRootsInvalidatedEventId: i32 = 20025;
pub const UIA_HyperlinkControlTypeId: i32 = 50005;
pub const UIA_IAFP_DEFAULT: u32 = 0;
pub const UIA_IAFP_UNWRAP_BRIDGE: u32 = 1;
pub const UIA_ImageControlTypeId: i32 = 50006;
pub const UIA_IndentationFirstLineAttributeId: i32 = 40010;
pub const UIA_IndentationLeadingAttributeId: i32 = 40011;
pub const UIA_IndentationTrailingAttributeId: i32 = 40012;
pub const UIA_InputDiscardedEventId: i32 = 20022;
pub const UIA_InputReachedOtherElementEventId: i32 = 20021;
pub const UIA_InputReachedTargetEventId: i32 = 20020;
pub const UIA_InvokePatternId: i32 = 10000;
pub const UIA_Invoke_InvokedEventId: i32 = 20009;
pub const UIA_IsActiveAttributeId: i32 = 40036;
pub const UIA_IsAnnotationPatternAvailablePropertyId: i32 = 30118;
pub const UIA_IsContentElementPropertyId: i32 = 30017;
pub const UIA_IsControlElementPropertyId: i32 = 30016;
pub const UIA_IsCustomNavigationPatternAvailablePropertyId: i32 = 30151;
pub const UIA_IsDataValidForFormPropertyId: i32 = 30103;
pub const UIA_IsDialogPropertyId: i32 = 30174;
pub const UIA_IsDockPatternAvailablePropertyId: i32 = 30027;
pub const UIA_IsDragPatternAvailablePropertyId: i32 = 30137;
pub const UIA_IsDropTargetPatternAvailablePropertyId: i32 = 30141;
pub const UIA_IsEnabledPropertyId: i32 = 30010;
pub const UIA_IsExpandCollapsePatternAvailablePropertyId: i32 = 30028;
pub const UIA_IsGridItemPatternAvailablePropertyId: i32 = 30029;
pub const UIA_IsGridPatternAvailablePropertyId: i32 = 30030;
pub const UIA_IsHiddenAttributeId: i32 = 40013;
pub const UIA_IsInvokePatternAvailablePropertyId: i32 = 30031;
pub const UIA_IsItalicAttributeId: i32 = 40014;
pub const UIA_IsItemContainerPatternAvailablePropertyId: i32 = 30108;
pub const UIA_IsKeyboardFocusablePropertyId: i32 = 30009;
pub const UIA_IsLegacyIAccessiblePatternAvailablePropertyId: i32 = 30090;
pub const UIA_IsMultipleViewPatternAvailablePropertyId: i32 = 30032;
pub const UIA_IsObjectModelPatternAvailablePropertyId: i32 = 30112;
pub const UIA_IsOffscreenPropertyId: i32 = 30022;
pub const UIA_IsPasswordPropertyId: i32 = 30019;
pub const UIA_IsPeripheralPropertyId: i32 = 30150;
pub const UIA_IsRangeValuePatternAvailablePropertyId: i32 = 30033;
pub const UIA_IsReadOnlyAttributeId: i32 = 40015;
pub const UIA_IsRequiredForFormPropertyId: i32 = 30025;
pub const UIA_IsScrollItemPatternAvailablePropertyId: i32 = 30035;
pub const UIA_IsScrollPatternAvailablePropertyId: i32 = 30034;
pub const UIA_IsSelectionItemPatternAvailablePropertyId: i32 = 30036;
pub const UIA_IsSelectionPattern2AvailablePropertyId: i32 = 30168;
pub const UIA_IsSelectionPatternAvailablePropertyId: i32 = 30037;
pub const UIA_IsSpreadsheetItemPatternAvailablePropertyId: i32 = 30132;
pub const UIA_IsSpreadsheetPatternAvailablePropertyId: i32 = 30128;
pub const UIA_IsStylesPatternAvailablePropertyId: i32 = 30127;
pub const UIA_IsSubscriptAttributeId: i32 = 40016;
pub const UIA_IsSuperscriptAttributeId: i32 = 40017;
pub const UIA_IsSynchronizedInputPatternAvailablePropertyId: i32 = 30110;
pub const UIA_IsTableItemPatternAvailablePropertyId: i32 = 30039;
pub const UIA_IsTablePatternAvailablePropertyId: i32 = 30038;
pub const UIA_IsTextChildPatternAvailablePropertyId: i32 = 30136;
pub const UIA_IsTextEditPatternAvailablePropertyId: i32 = 30149;
pub const UIA_IsTextPattern2AvailablePropertyId: i32 = 30119;
pub const UIA_IsTextPatternAvailablePropertyId: i32 = 30040;
pub const UIA_IsTogglePatternAvailablePropertyId: i32 = 30041;
pub const UIA_IsTransformPattern2AvailablePropertyId: i32 = 30134;
pub const UIA_IsTransformPatternAvailablePropertyId: i32 = 30042;
pub const UIA_IsValuePatternAvailablePropertyId: i32 = 30043;
pub const UIA_IsVirtualizedItemPatternAvailablePropertyId: i32 = 30109;
pub const UIA_IsWindowPatternAvailablePropertyId: i32 = 30044;
pub const UIA_ItemContainerPatternId: i32 = 10019;
pub const UIA_ItemStatusPropertyId: i32 = 30026;
pub const UIA_ItemTypePropertyId: i32 = 30021;
pub const UIA_LabeledByPropertyId: i32 = 30018;
pub const UIA_LandmarkTypePropertyId: i32 = 30157;
pub const UIA_LayoutInvalidatedEventId: i32 = 20008;
pub const UIA_LegacyIAccessibleChildIdPropertyId: i32 = 30091;
pub const UIA_LegacyIAccessibleDefaultActionPropertyId: i32 = 30100;
pub const UIA_LegacyIAccessibleDescriptionPropertyId: i32 = 30094;
pub const UIA_LegacyIAccessibleHelpPropertyId: i32 = 30097;
pub const UIA_LegacyIAccessibleKeyboardShortcutPropertyId: i32 = 30098;
pub const UIA_LegacyIAccessibleNamePropertyId: i32 = 30092;
pub const UIA_LegacyIAccessiblePatternId: i32 = 10018;
pub const UIA_LegacyIAccessibleRolePropertyId: i32 = 30095;
pub const UIA_LegacyIAccessibleSelectionPropertyId: i32 = 30099;
pub const UIA_LegacyIAccessibleStatePropertyId: i32 = 30096;
pub const UIA_LegacyIAccessibleValuePropertyId: i32 = 30093;
pub const UIA_LevelPropertyId: i32 = 30154;
pub const UIA_LineSpacingAttributeId: i32 = 40040;
pub const UIA_LinkAttributeId: i32 = 40035;
pub const UIA_ListControlTypeId: i32 = 50008;
pub const UIA_ListItemControlTypeId: i32 = 50007;
pub const UIA_LiveRegionChangedEventId: i32 = 20024;
pub const UIA_LiveSettingPropertyId: i32 = 30135;
pub const UIA_LocalizedControlTypePropertyId: i32 = 30004;
pub const UIA_LocalizedLandmarkTypePropertyId: i32 = 30158;
pub const UIA_MainLandmarkTypeId: i32 = 80002;
pub const UIA_MarginBottomAttributeId: i32 = 40018;
pub const UIA_MarginLeadingAttributeId: i32 = 40019;
pub const UIA_MarginTopAttributeId: i32 = 40020;
pub const UIA_MarginTrailingAttributeId: i32 = 40021;
pub const UIA_MenuBarControlTypeId: i32 = 50010;
pub const UIA_MenuClosedEventId: i32 = 20007;
pub const UIA_MenuControlTypeId: i32 = 50009;
pub const UIA_MenuItemControlTypeId: i32 = 50011;
pub const UIA_MenuModeEndEventId: i32 = 20019;
pub const UIA_MenuModeStartEventId: i32 = 20018;
pub const UIA_MenuOpenedEventId: i32 = 20003;
pub const UIA_MultipleViewCurrentViewPropertyId: i32 = 30071;
pub const UIA_MultipleViewPatternId: i32 = 10008;
pub const UIA_MultipleViewSupportedViewsPropertyId: i32 = 30072;
pub const UIA_NamePropertyId: i32 = 30005;
pub const UIA_NativeWindowHandlePropertyId: i32 = 30020;
pub const UIA_NavigationLandmarkTypeId: i32 = 80003;
pub const UIA_NotificationEventId: i32 = 20035;
pub const UIA_ObjectModelPatternId: i32 = 10022;
pub const UIA_OptimizeForVisualContentPropertyId: i32 = 30111;
pub const UIA_OrientationPropertyId: i32 = 30023;
pub const UIA_OutlineColorPropertyId: i32 = 30161;
pub const UIA_OutlineStylesAttributeId: i32 = 40022;
pub const UIA_OutlineThicknessPropertyId: i32 = 30164;
pub const UIA_OverlineColorAttributeId: i32 = 40023;
pub const UIA_OverlineStyleAttributeId: i32 = 40024;
pub const UIA_PFIA_DEFAULT: u32 = 0;
pub const UIA_PFIA_UNWRAP_BRIDGE: u32 = 1;
pub const UIA_PaneControlTypeId: i32 = 50033;
pub const UIA_PositionInSetPropertyId: i32 = 30152;
pub const UIA_ProcessIdPropertyId: i32 = 30002;
pub const UIA_ProgressBarControlTypeId: i32 = 50012;
pub const UIA_ProviderDescriptionPropertyId: i32 = 30107;
pub const UIA_RadioButtonControlTypeId: i32 = 50013;
pub const UIA_RangeValueIsReadOnlyPropertyId: i32 = 30048;
pub const UIA_RangeValueLargeChangePropertyId: i32 = 30051;
pub const UIA_RangeValueMaximumPropertyId: i32 = 30050;
pub const UIA_RangeValueMinimumPropertyId: i32 = 30049;
pub const UIA_RangeValuePatternId: i32 = 10003;
pub const UIA_RangeValueSmallChangePropertyId: i32 = 30052;
pub const UIA_RangeValueValuePropertyId: i32 = 30047;
pub const UIA_RotationPropertyId: i32 = 30166;
pub const UIA_RuntimeIdPropertyId: i32 = 30000;
pub const UIA_SayAsInterpretAsAttributeId: i32 = 40043;
pub const UIA_SayAsInterpretAsMetadataId: i32 = 100000;
pub const UIA_ScrollBarControlTypeId: i32 = 50014;
pub const UIA_ScrollHorizontalScrollPercentPropertyId: i32 = 30053;
pub const UIA_ScrollHorizontalViewSizePropertyId: i32 = 30054;
pub const UIA_ScrollHorizontallyScrollablePropertyId: i32 = 30057;
pub const UIA_ScrollItemPatternId: i32 = 10017;
pub const UIA_ScrollPatternId: i32 = 10004;
pub const UIA_ScrollVerticalScrollPercentPropertyId: i32 = 30055;
pub const UIA_ScrollVerticalViewSizePropertyId: i32 = 30056;
pub const UIA_ScrollVerticallyScrollablePropertyId: i32 = 30058;
pub const UIA_SearchLandmarkTypeId: i32 = 80004;
pub const UIA_Selection2CurrentSelectedItemPropertyId: i32 = 30171;
pub const UIA_Selection2FirstSelectedItemPropertyId: i32 = 30169;
pub const UIA_Selection2ItemCountPropertyId: i32 = 30172;
pub const UIA_Selection2LastSelectedItemPropertyId: i32 = 30170;
pub const UIA_SelectionActiveEndAttributeId: i32 = 40037;
pub const UIA_SelectionCanSelectMultiplePropertyId: i32 = 30060;
pub const UIA_SelectionIsSelectionRequiredPropertyId: i32 = 30061;
pub const UIA_SelectionItemIsSelectedPropertyId: i32 = 30079;
pub const UIA_SelectionItemPatternId: i32 = 10010;
pub const UIA_SelectionItemSelectionContainerPropertyId: i32 = 30080;
pub const UIA_SelectionItem_ElementAddedToSelectionEventId: i32 = 20010;
pub const UIA_SelectionItem_ElementRemovedFromSelectionEventId: i32 = 20011;
pub const UIA_SelectionItem_ElementSelectedEventId: i32 = 20012;
pub const UIA_SelectionPattern2Id: i32 = 10034;
pub const UIA_SelectionPatternId: i32 = 10001;
pub const UIA_SelectionSelectionPropertyId: i32 = 30059;
pub const UIA_Selection_InvalidatedEventId: i32 = 20013;
pub const UIA_SemanticZoomControlTypeId: i32 = 50039;
pub const UIA_SeparatorControlTypeId: i32 = 50038;
pub const UIA_SizeOfSetPropertyId: i32 = 30153;
pub const UIA_SizePropertyId: i32 = 30167;
pub const UIA_SliderControlTypeId: i32 = 50015;
pub const UIA_SpinnerControlTypeId: i32 = 50016;
pub const UIA_SplitButtonControlTypeId: i32 = 50031;
pub const UIA_SpreadsheetItemAnnotationObjectsPropertyId: i32 = 30130;
pub const UIA_SpreadsheetItemAnnotationTypesPropertyId: i32 = 30131;
pub const UIA_SpreadsheetItemFormulaPropertyId: i32 = 30129;
pub const UIA_SpreadsheetItemPatternId: i32 = 10027;
pub const UIA_SpreadsheetPatternId: i32 = 10026;
pub const UIA_StatusBarControlTypeId: i32 = 50017;
pub const UIA_StrikethroughColorAttributeId: i32 = 40025;
pub const UIA_StrikethroughStyleAttributeId: i32 = 40026;
pub const UIA_StructureChangedEventId: i32 = 20002;
pub const UIA_StyleIdAttributeId: i32 = 40034;
pub const UIA_StyleNameAttributeId: i32 = 40033;
pub const UIA_StylesExtendedPropertiesPropertyId: i32 = 30126;
pub const UIA_StylesFillColorPropertyId: i32 = 30122;
pub const UIA_StylesFillPatternColorPropertyId: i32 = 30125;
pub const UIA_StylesFillPatternStylePropertyId: i32 = 30123;
pub const UIA_StylesPatternId: i32 = 10025;
pub const UIA_StylesShapePropertyId: i32 = 30124;
pub const UIA_StylesStyleIdPropertyId: i32 = 30120;
pub const UIA_StylesStyleNamePropertyId: i32 = 30121;
pub const UIA_SummaryChangeId: i32 = 90000;
pub const UIA_SynchronizedInputPatternId: i32 = 10021;
pub const UIA_SystemAlertEventId: i32 = 20023;
pub const UIA_TabControlTypeId: i32 = 50018;
pub const UIA_TabItemControlTypeId: i32 = 50019;
pub const UIA_TableColumnHeadersPropertyId: i32 = 30082;
pub const UIA_TableControlTypeId: i32 = 50036;
pub const UIA_TableItemColumnHeaderItemsPropertyId: i32 = 30085;
pub const UIA_TableItemPatternId: i32 = 10013;
pub const UIA_TableItemRowHeaderItemsPropertyId: i32 = 30084;
pub const UIA_TablePatternId: i32 = 10012;
pub const UIA_TableRowHeadersPropertyId: i32 = 30081;
pub const UIA_TableRowOrColumnMajorPropertyId: i32 = 30083;
pub const UIA_TabsAttributeId: i32 = 40027;
pub const UIA_TextChildPatternId: i32 = 10029;
pub const UIA_TextControlTypeId: i32 = 50020;
pub const UIA_TextEditPatternId: i32 = 10032;
pub const UIA_TextEdit_ConversionTargetChangedEventId: i32 = 20033;
pub const UIA_TextEdit_TextChangedEventId: i32 = 20032;
pub const UIA_TextFlowDirectionsAttributeId: i32 = 40028;
pub const UIA_TextPattern2Id: i32 = 10024;
pub const UIA_TextPatternId: i32 = 10014;
pub const UIA_Text_TextChangedEventId: i32 = 20015;
pub const UIA_Text_TextSelectionChangedEventId: i32 = 20014;
pub const UIA_ThumbControlTypeId: i32 = 50027;
pub const UIA_TitleBarControlTypeId: i32 = 50037;
pub const UIA_TogglePatternId: i32 = 10015;
pub const UIA_ToggleToggleStatePropertyId: i32 = 30086;
pub const UIA_ToolBarControlTypeId: i32 = 50021;
pub const UIA_ToolTipClosedEventId: i32 = 20001;
pub const UIA_ToolTipControlTypeId: i32 = 50022;
pub const UIA_ToolTipOpenedEventId: i32 = 20000;
pub const UIA_Transform2CanZoomPropertyId: i32 = 30133;
pub const UIA_Transform2ZoomLevelPropertyId: i32 = 30145;
pub const UIA_Transform2ZoomMaximumPropertyId: i32 = 30147;
pub const UIA_Transform2ZoomMinimumPropertyId: i32 = 30146;
pub const UIA_TransformCanMovePropertyId: i32 = 30087;
pub const UIA_TransformCanResizePropertyId: i32 = 30088;
pub const UIA_TransformCanRotatePropertyId: i32 = 30089;
pub const UIA_TransformPattern2Id: i32 = 10028;
pub const UIA_TransformPatternId: i32 = 10016;
pub const UIA_TreeControlTypeId: i32 = 50023;
pub const UIA_TreeItemControlTypeId: i32 = 50024;
pub const UIA_UnderlineColorAttributeId: i32 = 40029;
pub const UIA_UnderlineStyleAttributeId: i32 = 40030;
pub const UIA_ValueIsReadOnlyPropertyId: i32 = 30046;
pub const UIA_ValuePatternId: i32 = 10002;
pub const UIA_ValueValuePropertyId: i32 = 30045;
pub const UIA_VirtualizedItemPatternId: i32 = 10020;
pub const UIA_VisualEffectsPropertyId: i32 = 30163;
pub const UIA_WindowCanMaximizePropertyId: i32 = 30073;
pub const UIA_WindowCanMinimizePropertyId: i32 = 30074;
pub const UIA_WindowControlTypeId: i32 = 50032;
pub const UIA_WindowIsModalPropertyId: i32 = 30077;
pub const UIA_WindowIsTopmostPropertyId: i32 = 30078;
pub const UIA_WindowPatternId: i32 = 10009;
pub const UIA_WindowWindowInteractionStatePropertyId: i32 = 30076;
pub const UIA_WindowWindowVisualStatePropertyId: i32 = 30075;
pub const UIA_Window_WindowClosedEventId: i32 = 20017;
pub const UIA_Window_WindowOpenedEventId: i32 = 20016;
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UIViewSettings(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    UIViewSettings,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for UIViewSettings {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IUIViewSettings>();
}
unsafe impl windows_core::Interface for UIViewSettings {
    type Vtable = <IUIViewSettings as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IUIViewSettings as windows_core::Interface>::IID;
}
impl core::ops::Deref for UIViewSettings {
    type Target = IUIViewSettings;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for UIViewSettings {
    const NAME: &'static str = "Windows.UI.ViewManagement.UIViewSettings";
}
unsafe impl Send for UIViewSettings {}
unsafe impl Sync for UIViewSettings {}
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
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UserInteractionMode(pub i32);
impl UserInteractionMode {
    pub const Mouse: Self = Self(0);
    pub const Touch: Self = Self(1);
}
impl windows_core::TypeKind for UserInteractionMode {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for UserInteractionMode {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.ViewManagement.UserInteractionMode;i4)",
    );
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
pub const VK_BACK: i32 = 8;
pub const VK_CONTROL: i32 = 17;
pub const VK_DELETE: i32 = 46;
pub const VK_DOWN: i32 = 40;
pub const VK_END: i32 = 35;
pub const VK_ESCAPE: i32 = 27;
pub const VK_HOME: i32 = 36;
pub const VK_LEFT: i32 = 37;
pub const VK_MENU: i32 = 18;
pub const VK_NEXT: i32 = 34;
pub const VK_PRIOR: i32 = 33;
pub const VK_RETURN: i32 = 13;
pub const VK_RIGHT: i32 = 39;
pub const VK_SHIFT: i32 = 16;
pub const VK_SPACE: i32 = 32;
pub const VK_TAB: i32 = 9;
pub const VK_UP: i32 = 38;
pub const WHEEL_DELTA: i32 = 120;
pub const WM_CAPTURECHANGED: i32 = 533;
pub const WM_CHAR: i32 = 258;
pub const WM_GETOBJECT: i32 = 61;
pub const WM_KEYDOWN: i32 = 256;
pub const WM_KEYUP: i32 = 257;
pub const WM_KILLFOCUS: i32 = 8;
pub const WM_POINTERCAPTURECHANGED: i32 = 588;
pub const WM_POINTERDOWN: i32 = 582;
pub const WM_POINTERENTER: i32 = 585;
pub const WM_POINTERHWHEEL: i32 = 591;
pub const WM_POINTERLEAVE: i32 = 586;
pub const WM_POINTERUP: i32 = 583;
pub const WM_POINTERUPDATE: i32 = 581;
pub const WM_POINTERWHEEL: i32 = 590;
pub const WM_SETFOCUS: i32 = 7;
pub const WM_SYSKEYDOWN: i32 = 260;
pub const WM_SYSKEYUP: i32 = 261;
pub type WPARAM = usize;
pub type tagPOINTER_INPUT_TYPE = i32;
