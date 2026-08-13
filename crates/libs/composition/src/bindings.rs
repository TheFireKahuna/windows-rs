windows_core::link!("coremessaging.dll" "system" fn CreateDispatcherQueueController(options : DispatcherQueueOptions, dispatcherqueuecontroller : *mut *mut core::ffi::c_void) -> windows_core::HRESULT);
windows_core::link!("user32.dll" "system" fn GetPointerInfo(pointerid : u32, pointerinfo : *mut POINTER_INFO) -> windows_core::BOOL);
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
pub struct CompositionAnimationGroup(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionAnimationGroup,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CompositionAnimationGroup,
    ICompositionAnimationBase,
    CompositionObject
);
impl windows_core::RuntimeType for CompositionAnimationGroup {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionAnimationGroup>();
}
unsafe impl windows_core::Interface for CompositionAnimationGroup {
    type Vtable = <ICompositionAnimationGroup as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionAnimationGroup as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionAnimationGroup {
    type Target = ICompositionAnimationGroup;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionAnimationGroup {
    const NAME: &'static str = "Windows.UI.Composition.CompositionAnimationGroup";
}
unsafe impl Send for CompositionAnimationGroup {}
unsafe impl Sync for CompositionAnimationGroup {}
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompositionBorderMode(pub i32);
impl CompositionBorderMode {
    pub const Inherit: Self = Self(0);
    pub const Soft: Self = Self(1);
    pub const Hard: Self = Self(2);
}
impl windows_core::TypeKind for CompositionBorderMode {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for CompositionBorderMode {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.CompositionBorderMode;i4)",
    );
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
pub struct CompositionColorGradientStopCollection(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionColorGradientStopCollection,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for CompositionColorGradientStopCollection {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionColorGradientStopCollection>(
        );
}
unsafe impl windows_core::Interface for CompositionColorGradientStopCollection {
    type Vtable = <ICompositionColorGradientStopCollection as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <ICompositionColorGradientStopCollection as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionColorGradientStopCollection {
    type Target = ICompositionColorGradientStopCollection;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionColorGradientStopCollection {
    const NAME: &'static str = "Windows.UI.Composition.CompositionColorGradientStopCollection";
}
unsafe impl Send for CompositionColorGradientStopCollection {}
unsafe impl Sync for CompositionColorGradientStopCollection {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionContainerShape(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionContainerShape,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CompositionContainerShape,
    CompositionShape,
    CompositionObject
);
impl windows_core::RuntimeType for CompositionContainerShape {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionContainerShape>();
}
unsafe impl windows_core::Interface for CompositionContainerShape {
    type Vtable = <ICompositionContainerShape as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionContainerShape as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionContainerShape {
    type Target = ICompositionContainerShape;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionContainerShape {
    const NAME: &'static str = "Windows.UI.Composition.CompositionContainerShape";
}
unsafe impl Send for CompositionContainerShape {}
unsafe impl Sync for CompositionContainerShape {}
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompositionDropShadowSourcePolicy(pub i32);
impl CompositionDropShadowSourcePolicy {
    pub const Default: Self = Self(0);
    pub const InheritFromVisualContent: Self = Self(1);
}
impl windows_core::TypeKind for CompositionDropShadowSourcePolicy {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for CompositionDropShadowSourcePolicy {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.CompositionDropShadowSourcePolicy;i4)",
    );
}
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompositionGetValueStatus(pub i32);
impl CompositionGetValueStatus {
    pub const Succeeded: Self = Self(0);
    pub const TypeMismatch: Self = Self(1);
    pub const NotFound: Self = Self(2);
}
impl windows_core::TypeKind for CompositionGetValueStatus {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for CompositionGetValueStatus {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.CompositionGetValueStatus;i4)",
    );
}
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
pub struct CompositionInteractionSourceCollection(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionInteractionSourceCollection,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionInteractionSourceCollection, CompositionObject);
impl windows_core::RuntimeType for CompositionInteractionSourceCollection {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionInteractionSourceCollection>(
        );
}
unsafe impl windows_core::Interface for CompositionInteractionSourceCollection {
    type Vtable = <ICompositionInteractionSourceCollection as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <ICompositionInteractionSourceCollection as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionInteractionSourceCollection {
    type Target = ICompositionInteractionSourceCollection;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionInteractionSourceCollection {
    const NAME: &'static str =
        "Windows.UI.Composition.Interactions.CompositionInteractionSourceCollection";
}
unsafe impl Send for CompositionInteractionSourceCollection {}
unsafe impl Sync for CompositionInteractionSourceCollection {}
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompositionMappingMode(pub i32);
impl CompositionMappingMode {
    pub const Absolute: Self = Self(0);
    pub const Relative: Self = Self(1);
}
impl windows_core::TypeKind for CompositionMappingMode {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for CompositionMappingMode {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.CompositionMappingMode;i4)",
    );
}
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
pub struct CompositionShadow(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionShadow,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionShadow, CompositionObject);
impl windows_core::RuntimeType for CompositionShadow {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionShadow>();
}
unsafe impl windows_core::Interface for CompositionShadow {
    type Vtable = <ICompositionShadow as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionShadow as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionShadow {
    type Target = ICompositionShadow;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionShadow {
    const NAME: &'static str = "Windows.UI.Composition.CompositionShadow";
}
unsafe impl Send for CompositionShadow {}
unsafe impl Sync for CompositionShadow {}
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
pub struct CompositionStrokeDashArray(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionStrokeDashArray,
    windows_core::IUnknown,
    windows_core::IInspectable,
    windows_collections::IVector<f32>
);
windows_core::imp::required_hierarchy!(CompositionStrokeDashArray, CompositionObject);
impl windows_core::RuntimeType for CompositionStrokeDashArray {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, windows_collections::IVector<f32>>();
}
unsafe impl windows_core::Interface for CompositionStrokeDashArray {
    type Vtable = <windows_collections::IVector<f32> as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <windows_collections::IVector<f32> as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionStrokeDashArray {
    type Target = windows_collections::IVector<f32>;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionStrokeDashArray {
    const NAME: &'static str = "Windows.UI.Composition.CompositionStrokeDashArray";
}
unsafe impl Send for CompositionStrokeDashArray {}
unsafe impl Sync for CompositionStrokeDashArray {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompositionStrokeLineJoin(pub i32);
impl CompositionStrokeLineJoin {
    pub const Miter: Self = Self(0);
    pub const Bevel: Self = Self(1);
    pub const Round: Self = Self(2);
    pub const MiterOrBevel: Self = Self(3);
}
impl windows_core::TypeKind for CompositionStrokeLineJoin {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for CompositionStrokeLineJoin {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.CompositionStrokeLineJoin;i4)",
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
pub struct CompositionVirtualDrawingSurface(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionVirtualDrawingSurface,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CompositionVirtualDrawingSurface,
    ICompositionSurface,
    CompositionDrawingSurface,
    CompositionObject
);
impl windows_core::RuntimeType for CompositionVirtualDrawingSurface {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionVirtualDrawingSurface>();
}
unsafe impl windows_core::Interface for CompositionVirtualDrawingSurface {
    type Vtable = <ICompositionVirtualDrawingSurface as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <ICompositionVirtualDrawingSurface as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionVirtualDrawingSurface {
    type Target = ICompositionVirtualDrawingSurface;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionVirtualDrawingSurface {
    const NAME: &'static str = "Windows.UI.Composition.CompositionVirtualDrawingSurface";
}
unsafe impl Send for CompositionVirtualDrawingSurface {}
unsafe impl Sync for CompositionVirtualDrawingSurface {}
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
pub type DISPATCHERQUEUE_THREAD_APARTMENTTYPE = i32;
pub type DISPATCHERQUEUE_THREAD_TYPE = i32;
pub const DQTAT_COM_ASTA: DISPATCHERQUEUE_THREAD_APARTMENTTYPE = 1;
pub const DQTYPE_THREAD_CURRENT: DISPATCHERQUEUE_THREAD_TYPE = 2;
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
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DispatcherQueueOptions {
    pub dwSize: u32,
    pub threadType: DISPATCHERQUEUE_THREAD_TYPE,
    pub apartmentType: DISPATCHERQUEUE_THREAD_APARTMENTTYPE,
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DropShadow(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    DropShadow,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(DropShadow, CompositionShadow, CompositionObject);
impl windows_core::RuntimeType for DropShadow {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IDropShadow>();
}
unsafe impl windows_core::Interface for DropShadow {
    type Vtable = <IDropShadow as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IDropShadow as windows_core::Interface>::IID;
}
impl core::ops::Deref for DropShadow {
    type Target = IDropShadow;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for DropShadow {
    const NAME: &'static str = "Windows.UI.Composition.DropShadow";
}
unsafe impl Send for DropShadow {}
unsafe impl Sync for DropShadow {}
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
pub type HANDLE = *mut core::ffi::c_void;
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
    SetColorParameter: usize,
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
    ICompositionAnimationGroup,
    ICompositionAnimationGroup_Vtbl,
    0x5e7cc90c_cd14_4e07_8a55_c72527aabdac
);
impl windows_core::RuntimeType for ICompositionAnimationGroup {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionAnimationGroup {
    pub(crate) fn Add<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<CompositionAnimation>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).Add)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionAnimationGroup_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Count: usize,
    pub Add: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
    ICompositionColorBrush,
    ICompositionColorBrush_Vtbl,
    0x2b264c5e_bf35_4831_8642_cf70c20fff2f
);
impl windows_core::RuntimeType for ICompositionColorBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionColorBrush {
    pub(crate) fn Color(&self) -> windows_core::Result<Color> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Color)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
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
    pub Color:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Color) -> windows_core::HRESULT,
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
    ICompositionColorGradientStopCollection,
    ICompositionColorGradientStopCollection_Vtbl,
    0x9f1d20ec_7b04_4b1d_90bc_9fa32c0cfd26
);
impl windows_core::RuntimeType for ICompositionColorGradientStopCollection {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionColorGradientStopCollection_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionContainerShape,
    ICompositionContainerShape_Vtbl,
    0x4f5e859b_2e5b_44a8_982c_aa0f69c16059
);
impl windows_core::RuntimeType for ICompositionContainerShape {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionContainerShape {
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
pub struct ICompositionContainerShape_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Shapes: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
#[repr(C)]
pub struct ICompositionDrawingSurface_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
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
    SetCenter: usize,
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
impl ICompositionGeometricClip {
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
}
#[repr(C)]
pub struct ICompositionGeometricClip_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Geometry: usize,
    pub SetGeometry: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
impl ICompositionGradientBrush {
    pub(crate) fn ColorStops(
        &self,
    ) -> windows_core::Result<CompositionColorGradientStopCollection> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ColorStops)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositionGradientBrush_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    AnchorPoint: usize,
    SetAnchorPoint: usize,
    CenterPoint: usize,
    SetCenterPoint: usize,
    pub ColorStops: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionGradientBrush2,
    ICompositionGradientBrush2_Vtbl,
    0x899dd5a1_b4c7_4b33_a1b6_264addc26d10
);
impl windows_core::RuntimeType for ICompositionGradientBrush2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionGradientBrush2 {
    pub(crate) fn SetMappingMode(&self, value: CompositionMappingMode) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetMappingMode)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionGradientBrush2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    MappingMode: usize,
    pub SetMappingMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        CompositionMappingMode,
    ) -> windows_core::HRESULT,
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
    pub(crate) fn CreateVirtualDrawingSurface(
        &self,
        sizepixels: SizeInt32,
        pixelformat: DirectXPixelFormat,
        alphamode: DirectXAlphaMode,
    ) -> windows_core::Result<CompositionVirtualDrawingSurface> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateVirtualDrawingSurface)(
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
    pub CreateVirtualDrawingSurface: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        SizeInt32,
        DirectXPixelFormat,
        DirectXAlphaMode,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionInteractionSource,
    ICompositionInteractionSource_Vtbl,
    0x043b2431_06e3_495a_ba54_409f0017fac0
);
impl windows_core::RuntimeType for ICompositionInteractionSource {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
windows_core::imp::interface_hierarchy!(
    ICompositionInteractionSource,
    windows_core::IUnknown,
    windows_core::IInspectable
);
#[repr(C)]
pub struct ICompositionInteractionSource_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionInteractionSourceCollection,
    ICompositionInteractionSourceCollection_Vtbl,
    0x1b468e4b_a5bf_47d8_a547_3894155a158c
);
impl windows_core::RuntimeType for ICompositionInteractionSourceCollection {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionInteractionSourceCollection {
    pub(crate) fn Add<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<ICompositionInteractionSource>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).Add)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn Remove<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<ICompositionInteractionSource>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).Remove)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
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
pub struct ICompositionInteractionSourceCollection_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Count: usize,
    pub Add: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub Remove: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub RemoveAll: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
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
impl ICompositionLinearGradientBrush {
    pub(crate) fn SetEndPoint(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetEndPoint)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetStartPoint(
        &self,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetStartPoint)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionLinearGradientBrush_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    EndPoint: usize,
    pub SetEndPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    StartPoint: usize,
    pub SetStartPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
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
    pub(crate) fn SetIsCenterHollow(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsCenterHollow)(
                windows_core::Interface::as_raw(self),
                value,
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
}
#[repr(C)]
pub struct ICompositionNineGridBrush_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    BottomInset: usize,
    SetBottomInset: usize,
    BottomInsetScale: usize,
    SetBottomInsetScale: usize,
    IsCenterHollow: usize,
    pub SetIsCenterHollow:
        unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
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
    SetInsets: usize,
    pub SetInsetsWithValues: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        f32,
        f32,
        f32,
    ) -> windows_core::HRESULT,
    pub SetInsetScales:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
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
    pub(crate) fn Properties(&self) -> windows_core::Result<CompositionPropertySet> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Properties)(
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
    pub Properties: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
impl ICompositionPathGeometry {
    pub(crate) fn SetPath<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<CompositionPath>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetPath)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionPathGeometry_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Path: usize,
    pub SetPath: unsafe extern "system" fn(
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
    pub(crate) fn InsertVector2(
        &self,
        propertyname: &str,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).InsertVector2)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(propertyname)),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn InsertVector3(
        &self,
        propertyname: &str,
        value: windows_numerics::Vector3,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).InsertVector3)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(propertyname)),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn TryGetScalar(
        &self,
        propertyname: &str,
        value: &mut f32,
    ) -> windows_core::Result<CompositionGetValueStatus> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TryGetScalar)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(propertyname)),
                value,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn TryGetVector2(
        &self,
        propertyname: &str,
        value: &mut windows_numerics::Vector2,
    ) -> windows_core::Result<CompositionGetValueStatus> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TryGetVector2)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(propertyname)),
                value,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn TryGetVector3(
        &self,
        propertyname: &str,
        value: &mut windows_numerics::Vector3,
    ) -> windows_core::Result<CompositionGetValueStatus> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TryGetVector3)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(propertyname)),
                value,
                &mut result__,
            )
            .map(|| result__)
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
    pub InsertVector2: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    pub InsertVector3: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    InsertVector4: usize,
    TryGetColor: usize,
    TryGetMatrix3x2: usize,
    TryGetMatrix4x4: usize,
    TryGetQuaternion: usize,
    pub TryGetScalar: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut f32,
        *mut CompositionGetValueStatus,
    ) -> windows_core::HRESULT,
    pub TryGetVector2: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut windows_numerics::Vector2,
        *mut CompositionGetValueStatus,
    ) -> windows_core::HRESULT,
    pub TryGetVector3: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut windows_numerics::Vector3,
        *mut CompositionGetValueStatus,
    ) -> windows_core::HRESULT,
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
    ICompositionShadow,
    ICompositionShadow_Vtbl,
    0x329e52e2_4335_49cc_b14a_37782d10f0c4
);
impl windows_core::RuntimeType for ICompositionShadow {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionShadow_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
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
impl ICompositionShape {
    pub(crate) fn SetOffset(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetOffset)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetScale(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetScale)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionShape_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    CenterPoint: usize,
    SetCenterPoint: usize,
    Offset: usize,
    pub SetOffset: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    RotationAngle: usize,
    SetRotationAngle: usize,
    RotationAngleInDegrees: usize,
    SetRotationAngleInDegrees: usize,
    Scale: usize,
    pub SetScale: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
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
    pub(crate) fn SetIsStrokeNonScaling(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsStrokeNonScaling)(
                windows_core::Interface::as_raw(self),
                value,
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
    pub(crate) fn StrokeDashArray(&self) -> windows_core::Result<CompositionStrokeDashArray> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).StrokeDashArray)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn SetStrokeDashCap(&self, value: CompositionStrokeCap) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetStrokeDashCap)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetStrokeDashOffset(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetStrokeDashOffset)(
                windows_core::Interface::as_raw(self),
                value,
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
    pub(crate) fn SetStrokeLineJoin(
        &self,
        value: CompositionStrokeLineJoin,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetStrokeLineJoin)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetStrokeMiterLimit(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetStrokeMiterLimit)(
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
    pub SetIsStrokeNonScaling:
        unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    StrokeBrush: usize,
    pub SetStrokeBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub StrokeDashArray: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    StrokeDashCap: usize,
    pub SetStrokeDashCap: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        CompositionStrokeCap,
    ) -> windows_core::HRESULT,
    StrokeDashOffset: usize,
    pub SetStrokeDashOffset:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    StrokeEndCap: usize,
    pub SetStrokeEndCap: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        CompositionStrokeCap,
    ) -> windows_core::HRESULT,
    StrokeLineJoin: usize,
    pub SetStrokeLineJoin: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        CompositionStrokeLineJoin,
    ) -> windows_core::HRESULT,
    StrokeMiterLimit: usize,
    pub SetStrokeMiterLimit:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
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
    pub(crate) fn SetHorizontalAlignmentRatio(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetHorizontalAlignmentRatio)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
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
    pub(crate) fn SetVerticalAlignmentRatio(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetVerticalAlignmentRatio)(
                windows_core::Interface::as_raw(self),
                value,
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
    pub SetHorizontalAlignmentRatio:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
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
    VerticalAlignmentRatio: usize,
    pub SetVerticalAlignmentRatio:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionSurfaceBrush2,
    ICompositionSurfaceBrush2_Vtbl,
    0xd27174d5_64f5_4692_9dc7_71b61d7e5880
);
impl windows_core::RuntimeType for ICompositionSurfaceBrush2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionSurfaceBrush2 {
    pub(crate) fn SetOffset(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetOffset)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetScale(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetScale)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionSurfaceBrush2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    AnchorPoint: usize,
    SetAnchorPoint: usize,
    CenterPoint: usize,
    SetCenterPoint: usize,
    Offset: usize,
    pub SetOffset: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    RotationAngle: usize,
    SetRotationAngle: usize,
    RotationAngleInDegrees: usize,
    SetRotationAngleInDegrees: usize,
    Scale: usize,
    pub SetScale: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
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
    pub(crate) fn Root(&self) -> windows_core::Result<Visual> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Root)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
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
    pub Root: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetRoot: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionVirtualDrawingSurface,
    ICompositionVirtualDrawingSurface_Vtbl,
    0xa9c384db_8740_4f94_8b9d_b68521e7863d
);
impl windows_core::RuntimeType for ICompositionVirtualDrawingSurface {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionVirtualDrawingSurface_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
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
    CreateColorBrush: usize,
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
    CreateExpressionAnimation: usize,
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
    CreateInsetClipWithInsets: usize,
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
    pub CreateScopedBatch: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        CompositionBatchTypes,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateSpriteVisual: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateSurfaceBrush: usize,
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
    pub(crate) fn CreateAnimationGroup(&self) -> windows_core::Result<CompositionAnimationGroup> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateAnimationGroup)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn CreateDropShadow(&self) -> windows_core::Result<DropShadow> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateDropShadow)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
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
    pub(crate) fn CreateStepEasingFunctionWithStepCount(
        &self,
        stepcount: i32,
    ) -> windows_core::Result<StepEasingFunction> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateStepEasingFunctionWithStepCount)(
                windows_core::Interface::as_raw(self),
                stepcount,
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
    pub CreateAnimationGroup: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateBackdropBrush: usize,
    CreateDistantLight: usize,
    pub CreateDropShadow: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
    CreateStepEasingFunction: usize,
    pub CreateStepEasingFunctionWithStepCount: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        i32,
        *mut *mut core::ffi::c_void,
    )
        -> windows_core::HRESULT,
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
    pub(crate) fn CreateContainerShape(&self) -> windows_core::Result<CompositionContainerShape> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateContainerShape)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
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
    pub CreateContainerShape: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
    CreateSpriteShape: usize,
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
    CreateGeometricClip: usize,
    pub CreateGeometricClipWithGeometry: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositor7,
    ICompositor7_Vtbl,
    0xd3483fad_9a12_53ba_bfc8_88b7ff7977c6
);
impl windows_core::RuntimeType for ICompositor7 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositor7 {
    pub(crate) fn CreateRectangleClip(&self) -> windows_core::Result<RectangleClip> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateRectangleClip)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositor7_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    DispatcherQueue: usize,
    CreateAnimationPropertyInfo: usize,
    pub CreateRectangleClip: unsafe extern "system" fn(
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
    pub(crate) unsafe fn CreateCompositionSurfaceForHandle(
        &self,
        swapchain: HANDLE,
    ) -> windows_core::Result<ICompositionSurface> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateCompositionSurfaceForHandle)(
                windows_core::Interface::as_raw(self),
                swapchain,
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
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
    pub CreateCompositionSurfaceForHandle: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        HANDLE,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
    IDropShadow,
    IDropShadow_Vtbl,
    0xcb977c07_a154_4851_85e7_a8924c84fad8
);
impl windows_core::RuntimeType for IDropShadow {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDropShadow {
    pub(crate) fn SetBlurRadius(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetBlurRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetColor(&self, value: Color) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetColor)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
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
}
#[repr(C)]
pub struct IDropShadow_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    BlurRadius: usize,
    pub SetBlurRadius:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    Color: usize,
    pub SetColor: unsafe extern "system" fn(*mut core::ffi::c_void, Color) -> windows_core::HRESULT,
    Mask: usize,
    pub SetMask: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    Offset: usize,
    pub SetOffset: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    Opacity: usize,
    pub SetOpacity: unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IDropShadow2,
    IDropShadow2_Vtbl,
    0x6c4218bc_15b9_4c2d_8d4a_0767df11977a
);
impl windows_core::RuntimeType for IDropShadow2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IDropShadow2 {
    pub(crate) fn SetSourcePolicy(
        &self,
        value: CompositionDropShadowSourcePolicy,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetSourcePolicy)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IDropShadow2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    SourcePolicy: usize,
    pub SetSourcePolicy: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        CompositionDropShadowSourcePolicy,
    ) -> windows_core::HRESULT,
}
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
    IInteractionSourceConfiguration,
    IInteractionSourceConfiguration_Vtbl,
    0xa78347e5_a9d1_4d02_985e_b930cd0b9da4
);
impl windows_core::RuntimeType for IInteractionSourceConfiguration {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionSourceConfiguration {
    pub(crate) fn SetPositionXSourceMode(
        &self,
        value: InteractionSourceRedirectionMode,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPositionXSourceMode)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetPositionYSourceMode(
        &self,
        value: InteractionSourceRedirectionMode,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPositionYSourceMode)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetScaleSourceMode(
        &self,
        value: InteractionSourceRedirectionMode,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetScaleSourceMode)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IInteractionSourceConfiguration_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    PositionXSourceMode: usize,
    pub SetPositionXSourceMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InteractionSourceRedirectionMode,
    ) -> windows_core::HRESULT,
    PositionYSourceMode: usize,
    pub SetPositionYSourceMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InteractionSourceRedirectionMode,
    ) -> windows_core::HRESULT,
    ScaleSourceMode: usize,
    pub SetScaleSourceMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InteractionSourceRedirectionMode,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTracker,
    IInteractionTracker_Vtbl,
    0x2a8e8cb1_1000_4416_8363_cc27fb877308
);
impl windows_core::RuntimeType for IInteractionTracker {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTracker {
    pub(crate) fn InteractionSources(
        &self,
    ) -> windows_core::Result<CompositionInteractionSourceCollection> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).InteractionSources)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
    pub(crate) fn MaxPosition(&self) -> windows_core::Result<windows_numerics::Vector3> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).MaxPosition)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn SetMaxPosition(
        &self,
        value: windows_numerics::Vector3,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetMaxPosition)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetMaxScale(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetMaxScale)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn MinPosition(&self) -> windows_core::Result<windows_numerics::Vector3> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).MinPosition)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn SetMinPosition(
        &self,
        value: windows_numerics::Vector3,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetMinPosition)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetMinScale(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetMinScale)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetPositionInertiaDecayRate(
        &self,
        value: Option<windows_numerics::Vector3>,
    ) -> windows_core::Result<()> {
        let value__ =
            value.map(<windows_reference::IReference<windows_numerics::Vector3> as From<_>>::from);
        unsafe {
            (windows_core::Interface::vtable(self).SetPositionInertiaDecayRate)(
                windows_core::Interface::as_raw(self),
                windows_core::Param::param(value__.as_ref()).abi(),
            )
            .ok()
        }
    }
    pub(crate) fn SetScaleInertiaDecayRate(&self, value: Option<f32>) -> windows_core::Result<()> {
        let value__ = value.map(<windows_reference::IReference<f32> as From<_>>::from);
        unsafe {
            (windows_core::Interface::vtable(self).SetScaleInertiaDecayRate)(
                windows_core::Interface::as_raw(self),
                windows_core::Param::param(value__.as_ref()).abi(),
            )
            .ok()
        }
    }
    pub(crate) fn ConfigurePositionXInertiaModifiers<P0>(
        &self,
        modifiers: P0,
    ) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_collections::IIterable<InteractionTrackerInertiaModifier>>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).ConfigurePositionXInertiaModifiers)(
                windows_core::Interface::as_raw(self),
                modifiers.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn ConfigurePositionYInertiaModifiers<P0>(
        &self,
        modifiers: P0,
    ) -> windows_core::Result<()>
    where
        P0: windows_core::Param<windows_collections::IIterable<InteractionTrackerInertiaModifier>>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).ConfigurePositionYInertiaModifiers)(
                windows_core::Interface::as_raw(self),
                modifiers.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn TryUpdatePositionWithAnimation<P0>(
        &self,
        animation: P0,
    ) -> windows_core::Result<i32>
    where
        P0: windows_core::Param<CompositionAnimation>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TryUpdatePositionWithAnimation)(
                windows_core::Interface::as_raw(self),
                animation.param().abi(),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn TryUpdatePositionWithAdditionalVelocity(
        &self,
        velocityinpixelspersecond: windows_numerics::Vector3,
    ) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TryUpdatePositionWithAdditionalVelocity)(
                windows_core::Interface::as_raw(self),
                velocityinpixelspersecond,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn TryUpdateScale(
        &self,
        value: f32,
        centerpoint: windows_numerics::Vector3,
    ) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TryUpdateScale)(
                windows_core::Interface::as_raw(self),
                value,
                centerpoint,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn TryUpdateScaleWithAdditionalVelocity(
        &self,
        velocityinpercentpersecond: f32,
        centerpoint: windows_numerics::Vector3,
    ) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TryUpdateScaleWithAdditionalVelocity)(
                windows_core::Interface::as_raw(self),
                velocityinpercentpersecond,
                centerpoint,
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInteractionTracker_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub InteractionSources: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    IsPositionRoundingSuggested: usize,
    pub MaxPosition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    pub SetMaxPosition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    MaxScale: usize,
    pub SetMaxScale:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    pub MinPosition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    pub SetMinPosition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    MinScale: usize,
    pub SetMinScale:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    NaturalRestingPosition: usize,
    NaturalRestingScale: usize,
    Owner: usize,
    Position: usize,
    PositionInertiaDecayRate: usize,
    pub SetPositionInertiaDecayRate: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    PositionVelocityInPixelsPerSecond: usize,
    Scale: usize,
    ScaleInertiaDecayRate: usize,
    pub SetScaleInertiaDecayRate: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    ScaleVelocityInPercentPerSecond: usize,
    AdjustPositionXIfGreaterThanThreshold: usize,
    AdjustPositionYIfGreaterThanThreshold: usize,
    pub ConfigurePositionXInertiaModifiers: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ConfigurePositionYInertiaModifiers: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    ConfigureScaleInertiaModifiers: usize,
    TryUpdatePosition: usize,
    TryUpdatePositionBy: usize,
    pub TryUpdatePositionWithAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i32,
    ) -> windows_core::HRESULT,
    pub TryUpdatePositionWithAdditionalVelocity: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector3,
        *mut i32,
    )
        -> windows_core::HRESULT,
    pub TryUpdateScale: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        windows_numerics::Vector3,
        *mut i32,
    ) -> windows_core::HRESULT,
    TryUpdateScaleWithAnimation: usize,
    pub TryUpdateScaleWithAdditionalVelocity: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        windows_numerics::Vector3,
        *mut i32,
    )
        -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTracker4,
    IInteractionTracker4_Vtbl,
    0xebd222bc_04af_4ac7_847d_06ea36e80a16
);
impl windows_core::RuntimeType for IInteractionTracker4 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTracker4 {
    pub(crate) fn TryUpdatePositionByWithOption(
        &self,
        amount: windows_numerics::Vector3,
        option: InteractionTrackerClampingOption,
    ) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TryUpdatePositionByWithOption)(
                windows_core::Interface::as_raw(self),
                amount,
                option,
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn IsInertiaFromImpulse(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsInertiaFromImpulse)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInteractionTracker4_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    TryUpdatePositionWithOption: usize,
    pub TryUpdatePositionByWithOption: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector3,
        InteractionTrackerClampingOption,
        *mut i32,
    ) -> windows_core::HRESULT,
    pub IsInertiaFromImpulse:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTracker5,
    IInteractionTracker5_Vtbl,
    0xd3ef5da2_a254_40e4_88d5_44e4e16b5809
);
impl windows_core::RuntimeType for IInteractionTracker5 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTracker5 {
    pub(crate) fn TryUpdatePositionWithOption(
        &self,
        value: windows_numerics::Vector3,
        option: InteractionTrackerClampingOption,
        posupdateoption: InteractionTrackerPositionUpdateOption,
    ) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TryUpdatePositionWithOption)(
                windows_core::Interface::as_raw(self),
                value,
                option,
                posupdateoption,
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInteractionTracker5_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub TryUpdatePositionWithOption: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector3,
        InteractionTrackerClampingOption,
        InteractionTrackerPositionUpdateOption,
        *mut i32,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerCustomAnimationStateEnteredArgs,
    IInteractionTrackerCustomAnimationStateEnteredArgs_Vtbl,
    0x8d1c8cf1_d7b0_434c_a5d2_2d7611864834
);
impl windows_core::RuntimeType for IInteractionTrackerCustomAnimationStateEnteredArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTrackerCustomAnimationStateEnteredArgs {
    pub(crate) fn RequestId(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).RequestId)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInteractionTrackerCustomAnimationStateEnteredArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub RequestId:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerCustomAnimationStateEnteredArgs2,
    IInteractionTrackerCustomAnimationStateEnteredArgs2_Vtbl,
    0x47d579b7_0985_5e99_b024_2f32c380c1a4
);
impl windows_core::RuntimeType for IInteractionTrackerCustomAnimationStateEnteredArgs2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTrackerCustomAnimationStateEnteredArgs2 {
    pub(crate) fn IsFromBinding(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsFromBinding)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInteractionTrackerCustomAnimationStateEnteredArgs2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub IsFromBinding:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerIdleStateEnteredArgs,
    IInteractionTrackerIdleStateEnteredArgs_Vtbl,
    0x50012faa_1510_4142_a1a5_019b09f8857b
);
impl windows_core::RuntimeType for IInteractionTrackerIdleStateEnteredArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTrackerIdleStateEnteredArgs {
    pub(crate) fn RequestId(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).RequestId)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInteractionTrackerIdleStateEnteredArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub RequestId:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerIdleStateEnteredArgs2,
    IInteractionTrackerIdleStateEnteredArgs2_Vtbl,
    0xf2e771ed_b803_5137_9435_1c96e48721e9
);
impl windows_core::RuntimeType for IInteractionTrackerIdleStateEnteredArgs2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTrackerIdleStateEnteredArgs2 {
    pub(crate) fn IsFromBinding(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsFromBinding)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInteractionTrackerIdleStateEnteredArgs2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub IsFromBinding:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerInertiaModifier,
    IInteractionTrackerInertiaModifier_Vtbl,
    0xa0e2c920_26b4_4da2_8b61_5e683979bbe2
);
impl windows_core::RuntimeType for IInteractionTrackerInertiaModifier {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IInteractionTrackerInertiaModifier_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IInteractionTrackerInertiaMotion,
    IInteractionTrackerInertiaMotion_Vtbl,
    0x04922fdc_f154_4cb8_bf33_cc1ba611e6db
);
impl windows_core::RuntimeType for IInteractionTrackerInertiaMotion {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTrackerInertiaMotion {
    pub(crate) fn SetCondition<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<ExpressionAnimation>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetCondition)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn SetMotion<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<ExpressionAnimation>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetMotion)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IInteractionTrackerInertiaMotion_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Condition: usize,
    pub SetCondition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    Motion: usize,
    pub SetMotion: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerInertiaMotionStatics,
    IInteractionTrackerInertiaMotionStatics_Vtbl,
    0x8cc83dd6_ba7b_431a_844b_6eac9130f99a
);
impl windows_core::RuntimeType for IInteractionTrackerInertiaMotionStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IInteractionTrackerInertiaMotionStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Create: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerInertiaRestingValue,
    IInteractionTrackerInertiaRestingValue_Vtbl,
    0x86f7ec09_5096_4170_9cc8_df2fe101bb93
);
impl windows_core::RuntimeType for IInteractionTrackerInertiaRestingValue {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTrackerInertiaRestingValue {
    pub(crate) fn SetCondition<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<ExpressionAnimation>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetCondition)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn SetRestingValue<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<ExpressionAnimation>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetRestingValue)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IInteractionTrackerInertiaRestingValue_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Condition: usize,
    pub SetCondition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    RestingValue: usize,
    pub SetRestingValue: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerInertiaRestingValueStatics,
    IInteractionTrackerInertiaRestingValueStatics_Vtbl,
    0x18ed4699_0745_4096_bcab_3a4e99569bcf
);
impl windows_core::RuntimeType for IInteractionTrackerInertiaRestingValueStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IInteractionTrackerInertiaRestingValueStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Create: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerInertiaStateEnteredArgs,
    IInteractionTrackerInertiaStateEnteredArgs_Vtbl,
    0x87108cf2_e7ff_4f7d_9ffd_d72f1e409b63
);
impl windows_core::RuntimeType for IInteractionTrackerInertiaStateEnteredArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTrackerInertiaStateEnteredArgs {
    pub(crate) fn ModifiedRestingPosition(
        &self,
    ) -> windows_core::Result<windows_numerics::Vector3> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ModifiedRestingPosition)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
            .and_then(|r__: windows_reference::IReference<windows_numerics::Vector3>| r__.Value())
        }
    }
    pub(crate) fn ModifiedRestingScale(&self) -> windows_core::Result<f32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).ModifiedRestingScale)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
            .and_then(|r__: windows_reference::IReference<f32>| r__.Value())
        }
    }
    pub(crate) fn NaturalRestingPosition(&self) -> windows_core::Result<windows_numerics::Vector3> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).NaturalRestingPosition)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn NaturalRestingScale(&self) -> windows_core::Result<f32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).NaturalRestingScale)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn PositionVelocityInPixelsPerSecond(
        &self,
    ) -> windows_core::Result<windows_numerics::Vector3> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PositionVelocityInPixelsPerSecond)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn RequestId(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).RequestId)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInteractionTrackerInertiaStateEnteredArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub ModifiedRestingPosition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ModifiedRestingScale: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub NaturalRestingPosition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    pub NaturalRestingScale:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut f32) -> windows_core::HRESULT,
    pub PositionVelocityInPixelsPerSecond: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    pub RequestId:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerInertiaStateEnteredArgs2,
    IInteractionTrackerInertiaStateEnteredArgs2_Vtbl,
    0xb1eb32f6_c26c_41f6_a189_fabc22b323cc
);
impl windows_core::RuntimeType for IInteractionTrackerInertiaStateEnteredArgs2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTrackerInertiaStateEnteredArgs2 {
    pub(crate) fn IsInertiaFromImpulse(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsInertiaFromImpulse)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInteractionTrackerInertiaStateEnteredArgs2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub IsInertiaFromImpulse:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerInertiaStateEnteredArgs3,
    IInteractionTrackerInertiaStateEnteredArgs3_Vtbl,
    0x48ac1c2f_47bd_59af_a58c_79bd2eb9ef71
);
impl windows_core::RuntimeType for IInteractionTrackerInertiaStateEnteredArgs3 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTrackerInertiaStateEnteredArgs3 {
    pub(crate) fn IsFromBinding(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsFromBinding)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInteractionTrackerInertiaStateEnteredArgs3_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub IsFromBinding:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerInteractingStateEnteredArgs,
    IInteractionTrackerInteractingStateEnteredArgs_Vtbl,
    0xa7263939_a17b_4011_99fd_b5c24f143748
);
impl windows_core::RuntimeType for IInteractionTrackerInteractingStateEnteredArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTrackerInteractingStateEnteredArgs {
    pub(crate) fn RequestId(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).RequestId)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInteractionTrackerInteractingStateEnteredArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub RequestId:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerInteractingStateEnteredArgs2,
    IInteractionTrackerInteractingStateEnteredArgs2_Vtbl,
    0x509652d6_d488_59cd_819f_f52310295b11
);
impl windows_core::RuntimeType for IInteractionTrackerInteractingStateEnteredArgs2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTrackerInteractingStateEnteredArgs2 {
    pub(crate) fn IsFromBinding(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsFromBinding)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInteractionTrackerInteractingStateEnteredArgs2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub IsFromBinding:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerOwner,
    IInteractionTrackerOwner_Vtbl,
    0xdb2e8af3_4deb_4e53_b29c_b06c9f96d651
);
impl windows_core::RuntimeType for IInteractionTrackerOwner {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
    const NAME: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"Windows.UI.Composition.Interactions.IInteractionTrackerOwner",
    );
}
windows_core::imp::interface_hierarchy!(
    IInteractionTrackerOwner,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl IInteractionTrackerOwner {
    pub(crate) fn CustomAnimationStateEntered<P0, P1>(
        &self,
        sender: P0,
        args: P1,
    ) -> windows_core::Result<()>
    where
        P0: windows_core::Param<InteractionTracker>,
        P1: windows_core::Param<InteractionTrackerCustomAnimationStateEnteredArgs>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).CustomAnimationStateEntered)(
                windows_core::Interface::as_raw(self),
                sender.param().abi(),
                args.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn IdleStateEntered<P0, P1>(&self, sender: P0, args: P1) -> windows_core::Result<()>
    where
        P0: windows_core::Param<InteractionTracker>,
        P1: windows_core::Param<InteractionTrackerIdleStateEnteredArgs>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).IdleStateEntered)(
                windows_core::Interface::as_raw(self),
                sender.param().abi(),
                args.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn InertiaStateEntered<P0, P1>(
        &self,
        sender: P0,
        args: P1,
    ) -> windows_core::Result<()>
    where
        P0: windows_core::Param<InteractionTracker>,
        P1: windows_core::Param<InteractionTrackerInertiaStateEnteredArgs>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InertiaStateEntered)(
                windows_core::Interface::as_raw(self),
                sender.param().abi(),
                args.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn InteractingStateEntered<P0, P1>(
        &self,
        sender: P0,
        args: P1,
    ) -> windows_core::Result<()>
    where
        P0: windows_core::Param<InteractionTracker>,
        P1: windows_core::Param<InteractionTrackerInteractingStateEnteredArgs>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InteractingStateEntered)(
                windows_core::Interface::as_raw(self),
                sender.param().abi(),
                args.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn RequestIgnored<P0, P1>(&self, sender: P0, args: P1) -> windows_core::Result<()>
    where
        P0: windows_core::Param<InteractionTracker>,
        P1: windows_core::Param<InteractionTrackerRequestIgnoredArgs>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).RequestIgnored)(
                windows_core::Interface::as_raw(self),
                sender.param().abi(),
                args.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn ValuesChanged<P0, P1>(&self, sender: P0, args: P1) -> windows_core::Result<()>
    where
        P0: windows_core::Param<InteractionTracker>,
        P1: windows_core::Param<InteractionTrackerValuesChangedArgs>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).ValuesChanged)(
                windows_core::Interface::as_raw(self),
                sender.param().abi(),
                args.param().abi(),
            )
            .ok()
        }
    }
}
impl windows_core::RuntimeName for IInteractionTrackerOwner {
    const NAME: &'static str = "Windows.UI.Composition.Interactions.IInteractionTrackerOwner";
}
pub trait IInteractionTrackerOwner_Impl: windows_core::IUnknownImpl {
    fn CustomAnimationStateEntered(
        &self,
        sender: windows_core::Ref<InteractionTracker>,
        args: windows_core::Ref<InteractionTrackerCustomAnimationStateEnteredArgs>,
    ) -> windows_core::Result<()>;
    fn IdleStateEntered(
        &self,
        sender: windows_core::Ref<InteractionTracker>,
        args: windows_core::Ref<InteractionTrackerIdleStateEnteredArgs>,
    ) -> windows_core::Result<()>;
    fn InertiaStateEntered(
        &self,
        sender: windows_core::Ref<InteractionTracker>,
        args: windows_core::Ref<InteractionTrackerInertiaStateEnteredArgs>,
    ) -> windows_core::Result<()>;
    fn InteractingStateEntered(
        &self,
        sender: windows_core::Ref<InteractionTracker>,
        args: windows_core::Ref<InteractionTrackerInteractingStateEnteredArgs>,
    ) -> windows_core::Result<()>;
    fn RequestIgnored(
        &self,
        sender: windows_core::Ref<InteractionTracker>,
        args: windows_core::Ref<InteractionTrackerRequestIgnoredArgs>,
    ) -> windows_core::Result<()>;
    fn ValuesChanged(
        &self,
        sender: windows_core::Ref<InteractionTracker>,
        args: windows_core::Ref<InteractionTrackerValuesChangedArgs>,
    ) -> windows_core::Result<()>;
}
impl IInteractionTrackerOwner_Vtbl {
    pub const fn new<Identity: IInteractionTrackerOwner_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn CustomAnimationStateEntered<
            Identity: IInteractionTrackerOwner_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            sender: *mut core::ffi::c_void,
            args: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IInteractionTrackerOwner_Impl::CustomAnimationStateEntered(
                    this,
                    core::mem::transmute_copy(&sender),
                    core::mem::transmute_copy(&args),
                )
                .into()
            }
        }
        unsafe extern "system" fn IdleStateEntered<
            Identity: IInteractionTrackerOwner_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            sender: *mut core::ffi::c_void,
            args: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IInteractionTrackerOwner_Impl::IdleStateEntered(
                    this,
                    core::mem::transmute_copy(&sender),
                    core::mem::transmute_copy(&args),
                )
                .into()
            }
        }
        unsafe extern "system" fn InertiaStateEntered<
            Identity: IInteractionTrackerOwner_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            sender: *mut core::ffi::c_void,
            args: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IInteractionTrackerOwner_Impl::InertiaStateEntered(
                    this,
                    core::mem::transmute_copy(&sender),
                    core::mem::transmute_copy(&args),
                )
                .into()
            }
        }
        unsafe extern "system" fn InteractingStateEntered<
            Identity: IInteractionTrackerOwner_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            sender: *mut core::ffi::c_void,
            args: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IInteractionTrackerOwner_Impl::InteractingStateEntered(
                    this,
                    core::mem::transmute_copy(&sender),
                    core::mem::transmute_copy(&args),
                )
                .into()
            }
        }
        unsafe extern "system" fn RequestIgnored<
            Identity: IInteractionTrackerOwner_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            sender: *mut core::ffi::c_void,
            args: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IInteractionTrackerOwner_Impl::RequestIgnored(
                    this,
                    core::mem::transmute_copy(&sender),
                    core::mem::transmute_copy(&args),
                )
                .into()
            }
        }
        unsafe extern "system" fn ValuesChanged<
            Identity: IInteractionTrackerOwner_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            sender: *mut core::ffi::c_void,
            args: *mut core::ffi::c_void,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                IInteractionTrackerOwner_Impl::ValuesChanged(
                    this,
                    core::mem::transmute_copy(&sender),
                    core::mem::transmute_copy(&args),
                )
                .into()
            }
        }
        Self {
            base__: windows_core::IInspectable_Vtbl::new::<
                Identity,
                IInteractionTrackerOwner,
                OFFSET,
            >(),
            CustomAnimationStateEntered: CustomAnimationStateEntered::<Identity, OFFSET>,
            IdleStateEntered: IdleStateEntered::<Identity, OFFSET>,
            InertiaStateEntered: InertiaStateEntered::<Identity, OFFSET>,
            InteractingStateEntered: InteractingStateEntered::<Identity, OFFSET>,
            RequestIgnored: RequestIgnored::<Identity, OFFSET>,
            ValuesChanged: ValuesChanged::<Identity, OFFSET>,
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<IInteractionTrackerOwner as windows_core::Interface>::IID
    }
}
#[repr(C)]
pub struct IInteractionTrackerOwner_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CustomAnimationStateEntered: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub IdleStateEntered: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub InertiaStateEntered: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub InteractingStateEntered: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub RequestIgnored: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ValuesChanged: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerRequestIgnoredArgs,
    IInteractionTrackerRequestIgnoredArgs_Vtbl,
    0x80dd82f1_ce25_488f_91dd_cb6455ccff2e
);
impl windows_core::RuntimeType for IInteractionTrackerRequestIgnoredArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTrackerRequestIgnoredArgs {
    pub(crate) fn RequestId(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).RequestId)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInteractionTrackerRequestIgnoredArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub RequestId:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerStatics,
    IInteractionTrackerStatics_Vtbl,
    0xbba5d7b7_6590_4498_8d6c_eb62b514c92a
);
impl windows_core::RuntimeType for IInteractionTrackerStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IInteractionTrackerStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Create: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub CreateWithOwner: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerStatics2,
    IInteractionTrackerStatics2_Vtbl,
    0x35e53720_46b7_5cb0_b505_f3d6884a6163
);
impl windows_core::RuntimeType for IInteractionTrackerStatics2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IInteractionTrackerStatics2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub SetBindingMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        InteractionBindingAxisModes,
    ) -> windows_core::HRESULT,
    pub GetBindingMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut InteractionBindingAxisModes,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IInteractionTrackerValuesChangedArgs,
    IInteractionTrackerValuesChangedArgs_Vtbl,
    0xcf1578ef_d3df_4501_b9e6_f02fb22f73d0
);
impl windows_core::RuntimeType for IInteractionTrackerValuesChangedArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IInteractionTrackerValuesChangedArgs {
    pub(crate) fn Position(&self) -> windows_core::Result<windows_numerics::Vector3> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Position)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn RequestId(&self) -> windows_core::Result<i32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).RequestId)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub(crate) fn Scale(&self) -> windows_core::Result<f32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Scale)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
}
#[repr(C)]
pub struct IInteractionTrackerValuesChangedArgs_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Position: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    pub RequestId:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32) -> windows_core::HRESULT,
    pub Scale: unsafe extern "system" fn(*mut core::ffi::c_void, *mut f32) -> windows_core::HRESULT,
}
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
    pub(crate) fn SetDelayTime(&self, value: windows_time::TimeSpan) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetDelayTime)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetDuration(&self, value: windows_time::TimeSpan) -> windows_core::Result<()> {
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
    pub SetDelayTime: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_time::TimeSpan,
    ) -> windows_core::HRESULT,
    Duration: usize,
    pub SetDuration: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_time::TimeSpan,
    ) -> windows_core::HRESULT,
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
    IRectangleClip,
    IRectangleClip_Vtbl,
    0xb3e7549e_00b4_5b53_8be8_353f6c433101
);
impl windows_core::RuntimeType for IRectangleClip {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IRectangleClip {
    pub(crate) fn SetBottom(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetBottom)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetBottomLeftRadius(
        &self,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetBottomLeftRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetBottomRightRadius(
        &self,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetBottomRightRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetLeft(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetLeft)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetRight(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetRight)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetTop(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTop)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetTopLeftRadius(
        &self,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTopLeftRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetTopRightRadius(
        &self,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTopRightRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IRectangleClip_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Bottom: usize,
    pub SetBottom: unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    BottomLeftRadius: usize,
    pub SetBottomLeftRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    BottomRightRadius: usize,
    pub SetBottomRightRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    Left: usize,
    pub SetLeft: unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    Right: usize,
    pub SetRight: unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    Top: usize,
    pub SetTop: unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    TopLeftRadius: usize,
    pub SetTopLeftRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    TopRightRadius: usize,
    pub SetTopRightRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
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
    pub(crate) fn SetInitialVelocity(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetInitialVelocity)(
                windows_core::Interface::as_raw(self),
                value,
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
    InitialValue: usize,
    SetInitialValue: usize,
    InitialVelocity: usize,
    pub SetInitialVelocity:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
}
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
    pub(crate) fn SetPeriod(&self, value: windows_time::TimeSpan) -> windows_core::Result<()> {
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
    pub SetPeriod: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_time::TimeSpan,
    ) -> windows_core::HRESULT,
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
    pub(crate) fn SetPeriod(&self, value: windows_time::TimeSpan) -> windows_core::Result<()> {
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
    pub SetPeriod: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_time::TimeSpan,
    ) -> windows_core::HRESULT,
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
    pub(crate) fn SetPeriod(&self, value: windows_time::TimeSpan) -> windows_core::Result<()> {
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
    pub SetPeriod: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_time::TimeSpan,
    ) -> windows_core::HRESULT,
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
    ISpriteVisual2,
    ISpriteVisual2_Vtbl,
    0x588c9664_997a_4850_91fe_53cb58f81ce9
);
impl windows_core::RuntimeType for ISpriteVisual2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ISpriteVisual2 {
    pub(crate) fn SetShadow<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<CompositionShadow>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetShadow)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ISpriteVisual2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Shadow: usize,
    pub SetShadow: unsafe extern "system" fn(
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
    pub(crate) fn SetInitialVelocity(
        &self,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetInitialVelocity)(
                windows_core::Interface::as_raw(self),
                value,
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
    InitialValue: usize,
    SetInitialValue: usize,
    InitialVelocity: usize,
    pub SetInitialVelocity: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
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
    pub(crate) fn SetInitialVelocity(
        &self,
        value: windows_numerics::Vector3,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetInitialVelocity)(
                windows_core::Interface::as_raw(self),
                value,
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
    InitialValue: usize,
    SetInitialValue: usize,
    InitialVelocity: usize,
    pub SetInitialVelocity: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector3,
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
    pub(crate) fn SetBorderMode(&self, value: CompositionBorderMode) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetBorderMode)(
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
    pub(crate) fn IsVisible(&self) -> windows_core::Result<bool> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).IsVisible)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
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
    pub(crate) fn Opacity(&self) -> windows_core::Result<f32> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Opacity)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
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
    pub(crate) fn Scale(&self) -> windows_core::Result<windows_numerics::Vector3> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Scale)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
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
    pub SetBorderMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        CompositionBorderMode,
    ) -> windows_core::HRESULT,
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
    pub IsVisible:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut bool) -> windows_core::HRESULT,
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
    pub Opacity:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut f32) -> windows_core::HRESULT,
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
    pub Scale: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
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
    IVisual2,
    IVisual2_Vtbl,
    0x3052b611_56c3_4c3e_8bf3_f6e1ad473f06
);
impl windows_core::RuntimeType for IVisual2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IVisual2 {
    pub(crate) fn SetParentForTransform<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<Visual>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetParentForTransform)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub(crate) fn SetRelativeOffsetAdjustment(
        &self,
        value: windows_numerics::Vector3,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetRelativeOffsetAdjustment)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetRelativeSizeAdjustment(
        &self,
        value: windows_numerics::Vector2,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetRelativeSizeAdjustment)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IVisual2_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    ParentForTransform: usize,
    pub SetParentForTransform: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    RelativeOffsetAdjustment: usize,
    pub SetRelativeOffsetAdjustment: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector3,
    ) -> windows_core::HRESULT,
    RelativeSizeAdjustment: usize,
    pub SetRelativeSizeAdjustment: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IVisual4,
    IVisual4_Vtbl,
    0x9476bf11_e24b_5bf9_9ebe_6274109b2711
);
impl windows_core::RuntimeType for IVisual4 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IVisual4 {
    pub(crate) fn SetIsPixelSnappingEnabled(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsPixelSnappingEnabled)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IVisual4_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    IsPixelSnappingEnabled: usize,
    pub SetIsPixelSnappingEnabled:
        unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
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
    InsertBelow: usize,
    pub Remove: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub RemoveAll: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IVisualInteractionSource,
    IVisualInteractionSource_Vtbl,
    0xca0e8a86_d8d6_4111_b088_70347bd2b0ed
);
impl windows_core::RuntimeType for IVisualInteractionSource {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IVisualInteractionSource {
    pub(crate) fn SetIsPositionXRailsEnabled(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsPositionXRailsEnabled)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetIsPositionYRailsEnabled(&self, value: bool) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetIsPositionYRailsEnabled)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetManipulationRedirectionMode(
        &self,
        value: VisualInteractionSourceRedirectionMode,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetManipulationRedirectionMode)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetPositionXChainingMode(
        &self,
        value: InteractionChainingMode,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPositionXChainingMode)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetPositionXSourceMode(
        &self,
        value: InteractionSourceMode,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPositionXSourceMode)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetPositionYChainingMode(
        &self,
        value: InteractionChainingMode,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPositionYChainingMode)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetPositionYSourceMode(
        &self,
        value: InteractionSourceMode,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetPositionYSourceMode)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetScaleChainingMode(
        &self,
        value: InteractionChainingMode,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetScaleChainingMode)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn SetScaleSourceMode(
        &self,
        value: InteractionSourceMode,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetScaleSourceMode)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub(crate) fn Source(&self) -> windows_core::Result<Visual> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Source)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IVisualInteractionSource_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    IsPositionXRailsEnabled: usize,
    pub SetIsPositionXRailsEnabled:
        unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    IsPositionYRailsEnabled: usize,
    pub SetIsPositionYRailsEnabled:
        unsafe extern "system" fn(*mut core::ffi::c_void, bool) -> windows_core::HRESULT,
    ManipulationRedirectionMode: usize,
    pub SetManipulationRedirectionMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        VisualInteractionSourceRedirectionMode,
    ) -> windows_core::HRESULT,
    PositionXChainingMode: usize,
    pub SetPositionXChainingMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InteractionChainingMode,
    ) -> windows_core::HRESULT,
    PositionXSourceMode: usize,
    pub SetPositionXSourceMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InteractionSourceMode,
    ) -> windows_core::HRESULT,
    PositionYChainingMode: usize,
    pub SetPositionYChainingMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InteractionChainingMode,
    ) -> windows_core::HRESULT,
    PositionYSourceMode: usize,
    pub SetPositionYSourceMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InteractionSourceMode,
    ) -> windows_core::HRESULT,
    ScaleChainingMode: usize,
    pub SetScaleChainingMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InteractionChainingMode,
    ) -> windows_core::HRESULT,
    ScaleSourceMode: usize,
    pub SetScaleSourceMode: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        InteractionSourceMode,
    ) -> windows_core::HRESULT,
    pub Source: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IVisualInteractionSource3,
    IVisualInteractionSource3_Vtbl,
    0xd941ef2a_0d5c_4057_92d7_c9711533204f
);
impl windows_core::RuntimeType for IVisualInteractionSource3 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IVisualInteractionSource3 {
    pub(crate) fn PointerWheelConfig(
        &self,
    ) -> windows_core::Result<InteractionSourceConfiguration> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).PointerWheelConfig)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IVisualInteractionSource3_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub PointerWheelConfig: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IVisualInteractionSourceInterop,
    IVisualInteractionSourceInterop_Vtbl,
    0x11f62cd1_2f9d_42d3_b05f_d6790d9e9f8e
);
windows_core::imp::interface_hierarchy!(IVisualInteractionSourceInterop, windows_core::IUnknown);
impl IVisualInteractionSourceInterop {
    pub(crate) unsafe fn TryRedirectForManipulation(
        &self,
        pointerinfo: *const POINTER_INFO,
    ) -> windows_core::HRESULT {
        unsafe {
            (windows_core::Interface::vtable(self).TryRedirectForManipulation)(
                windows_core::Interface::as_raw(self),
                pointerinfo,
            )
        }
    }
}
#[repr(C)]
pub struct IVisualInteractionSourceInterop_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub TryRedirectForManipulation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const POINTER_INFO,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IVisualInteractionSourceStatics,
    IVisualInteractionSourceStatics_Vtbl,
    0x369965e1_8645_4f75_ba00_6479cd10c8e6
);
impl windows_core::RuntimeType for IVisualInteractionSourceStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IVisualInteractionSourceStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Create: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InteractionBindingAxisModes(pub u32);
impl InteractionBindingAxisModes {
    pub const None: Self = Self(0);
    pub const PositionX: Self = Self(1);
    pub const PositionY: Self = Self(2);
    pub const Scale: Self = Self(4);
}
impl windows_core::TypeKind for InteractionBindingAxisModes {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InteractionBindingAxisModes {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.Interactions.InteractionBindingAxisModes;u4)",
    );
}
impl InteractionBindingAxisModes {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for InteractionBindingAxisModes {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for InteractionBindingAxisModes {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for InteractionBindingAxisModes {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0);
    }
}
impl core::ops::BitAndAssign for InteractionBindingAxisModes {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0);
    }
}
impl core::ops::Not for InteractionBindingAxisModes {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InteractionChainingMode(pub i32);
impl InteractionChainingMode {
    pub const Auto: Self = Self(0);
    pub const Always: Self = Self(1);
    pub const Never: Self = Self(2);
}
impl windows_core::TypeKind for InteractionChainingMode {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InteractionChainingMode {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.Interactions.InteractionChainingMode;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionSourceConfiguration(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InteractionSourceConfiguration,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(InteractionSourceConfiguration, CompositionObject);
impl windows_core::RuntimeType for InteractionSourceConfiguration {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInteractionSourceConfiguration>();
}
unsafe impl windows_core::Interface for InteractionSourceConfiguration {
    type Vtable = <IInteractionSourceConfiguration as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IInteractionSourceConfiguration as windows_core::Interface>::IID;
}
impl core::ops::Deref for InteractionSourceConfiguration {
    type Target = IInteractionSourceConfiguration;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InteractionSourceConfiguration {
    const NAME: &'static str = "Windows.UI.Composition.Interactions.InteractionSourceConfiguration";
}
unsafe impl Send for InteractionSourceConfiguration {}
unsafe impl Sync for InteractionSourceConfiguration {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InteractionSourceMode(pub i32);
impl InteractionSourceMode {
    pub const Disabled: Self = Self(0);
    pub const EnabledWithInertia: Self = Self(1);
    pub const EnabledWithoutInertia: Self = Self(2);
}
impl windows_core::TypeKind for InteractionSourceMode {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InteractionSourceMode {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.Interactions.InteractionSourceMode;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InteractionSourceRedirectionMode(pub i32);
impl InteractionSourceRedirectionMode {
    pub const Disabled: Self = Self(0);
    pub const Enabled: Self = Self(1);
}
impl windows_core::TypeKind for InteractionSourceRedirectionMode {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InteractionSourceRedirectionMode {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.Interactions.InteractionSourceRedirectionMode;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionTracker(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InteractionTracker,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(InteractionTracker, CompositionObject);
impl InteractionTracker {
    pub(crate) fn Create<P0>(compositor: P0) -> windows_core::Result<Self>
    where
        P0: windows_core::Param<Compositor>,
    {
        Self::IInteractionTrackerStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).Create)(
                windows_core::Interface::as_raw(this),
                compositor.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    pub(crate) fn CreateWithOwner<P0, P1>(compositor: P0, owner: P1) -> windows_core::Result<Self>
    where
        P0: windows_core::Param<Compositor>,
        P1: windows_core::Param<IInteractionTrackerOwner>,
    {
        Self::IInteractionTrackerStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).CreateWithOwner)(
                windows_core::Interface::as_raw(this),
                compositor.param().abi(),
                owner.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    pub(crate) fn SetBindingMode<P0, P1>(
        boundtracker1: P0,
        boundtracker2: P1,
        axismode: InteractionBindingAxisModes,
    ) -> windows_core::Result<()>
    where
        P0: windows_core::Param<Self>,
        P1: windows_core::Param<Self>,
    {
        Self::IInteractionTrackerStatics2(|this| unsafe {
            (windows_core::Interface::vtable(this).SetBindingMode)(
                windows_core::Interface::as_raw(this),
                boundtracker1.param().abi(),
                boundtracker2.param().abi(),
                axismode,
            )
            .ok()
        })
    }
    pub(crate) fn GetBindingMode<P0, P1>(
        boundtracker1: P0,
        boundtracker2: P1,
    ) -> windows_core::Result<InteractionBindingAxisModes>
    where
        P0: windows_core::Param<Self>,
        P1: windows_core::Param<Self>,
    {
        Self::IInteractionTrackerStatics2(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).GetBindingMode)(
                windows_core::Interface::as_raw(this),
                boundtracker1.param().abi(),
                boundtracker2.param().abi(),
                &mut result__,
            )
            .map(|| result__)
        })
    }
    fn IInteractionTrackerStatics<
        R,
        F: FnOnce(&IInteractionTrackerStatics) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<
            InteractionTracker,
            IInteractionTrackerStatics,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
    fn IInteractionTrackerStatics2<
        R,
        F: FnOnce(&IInteractionTrackerStatics2) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<
            InteractionTracker,
            IInteractionTrackerStatics2,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for InteractionTracker {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInteractionTracker>();
}
unsafe impl windows_core::Interface for InteractionTracker {
    type Vtable = <IInteractionTracker as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IInteractionTracker as windows_core::Interface>::IID;
}
impl core::ops::Deref for InteractionTracker {
    type Target = IInteractionTracker;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InteractionTracker {
    const NAME: &'static str = "Windows.UI.Composition.Interactions.InteractionTracker";
}
unsafe impl Send for InteractionTracker {}
unsafe impl Sync for InteractionTracker {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InteractionTrackerClampingOption(pub i32);
impl InteractionTrackerClampingOption {
    pub const Auto: Self = Self(0);
    pub const Disabled: Self = Self(1);
}
impl windows_core::TypeKind for InteractionTrackerClampingOption {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InteractionTrackerClampingOption {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.Interactions.InteractionTrackerClampingOption;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionTrackerCustomAnimationStateEnteredArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InteractionTrackerCustomAnimationStateEnteredArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for InteractionTrackerCustomAnimationStateEnteredArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<
        Self,
        IInteractionTrackerCustomAnimationStateEnteredArgs,
    >();
}
unsafe impl windows_core::Interface for InteractionTrackerCustomAnimationStateEnteredArgs {
    type Vtable =
        <IInteractionTrackerCustomAnimationStateEnteredArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IInteractionTrackerCustomAnimationStateEnteredArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for InteractionTrackerCustomAnimationStateEnteredArgs {
    type Target = IInteractionTrackerCustomAnimationStateEnteredArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InteractionTrackerCustomAnimationStateEnteredArgs {
    const NAME: &'static str =
        "Windows.UI.Composition.Interactions.InteractionTrackerCustomAnimationStateEnteredArgs";
}
unsafe impl Send for InteractionTrackerCustomAnimationStateEnteredArgs {}
unsafe impl Sync for InteractionTrackerCustomAnimationStateEnteredArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionTrackerIdleStateEnteredArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InteractionTrackerIdleStateEnteredArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for InteractionTrackerIdleStateEnteredArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInteractionTrackerIdleStateEnteredArgs>(
        );
}
unsafe impl windows_core::Interface for InteractionTrackerIdleStateEnteredArgs {
    type Vtable = <IInteractionTrackerIdleStateEnteredArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IInteractionTrackerIdleStateEnteredArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for InteractionTrackerIdleStateEnteredArgs {
    type Target = IInteractionTrackerIdleStateEnteredArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InteractionTrackerIdleStateEnteredArgs {
    const NAME: &'static str =
        "Windows.UI.Composition.Interactions.InteractionTrackerIdleStateEnteredArgs";
}
unsafe impl Send for InteractionTrackerIdleStateEnteredArgs {}
unsafe impl Sync for InteractionTrackerIdleStateEnteredArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionTrackerInertiaModifier(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InteractionTrackerInertiaModifier,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(InteractionTrackerInertiaModifier, CompositionObject);
impl windows_core::RuntimeType for InteractionTrackerInertiaModifier {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInteractionTrackerInertiaModifier>();
}
unsafe impl windows_core::Interface for InteractionTrackerInertiaModifier {
    type Vtable = <IInteractionTrackerInertiaModifier as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IInteractionTrackerInertiaModifier as windows_core::Interface>::IID;
}
impl core::ops::Deref for InteractionTrackerInertiaModifier {
    type Target = IInteractionTrackerInertiaModifier;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InteractionTrackerInertiaModifier {
    const NAME: &'static str =
        "Windows.UI.Composition.Interactions.InteractionTrackerInertiaModifier";
}
unsafe impl Send for InteractionTrackerInertiaModifier {}
unsafe impl Sync for InteractionTrackerInertiaModifier {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionTrackerInertiaMotion(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InteractionTrackerInertiaMotion,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    InteractionTrackerInertiaMotion,
    InteractionTrackerInertiaModifier,
    CompositionObject
);
impl InteractionTrackerInertiaMotion {
    pub(crate) fn Create<P0>(compositor: P0) -> windows_core::Result<Self>
    where
        P0: windows_core::Param<Compositor>,
    {
        Self::IInteractionTrackerInertiaMotionStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).Create)(
                windows_core::Interface::as_raw(this),
                compositor.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    fn IInteractionTrackerInertiaMotionStatics<
        R,
        F: FnOnce(&IInteractionTrackerInertiaMotionStatics) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<
            InteractionTrackerInertiaMotion,
            IInteractionTrackerInertiaMotionStatics,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for InteractionTrackerInertiaMotion {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInteractionTrackerInertiaMotion>();
}
unsafe impl windows_core::Interface for InteractionTrackerInertiaMotion {
    type Vtable = <IInteractionTrackerInertiaMotion as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IInteractionTrackerInertiaMotion as windows_core::Interface>::IID;
}
impl core::ops::Deref for InteractionTrackerInertiaMotion {
    type Target = IInteractionTrackerInertiaMotion;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InteractionTrackerInertiaMotion {
    const NAME: &'static str =
        "Windows.UI.Composition.Interactions.InteractionTrackerInertiaMotion";
}
unsafe impl Send for InteractionTrackerInertiaMotion {}
unsafe impl Sync for InteractionTrackerInertiaMotion {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionTrackerInertiaRestingValue(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InteractionTrackerInertiaRestingValue,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    InteractionTrackerInertiaRestingValue,
    InteractionTrackerInertiaModifier,
    CompositionObject
);
impl InteractionTrackerInertiaRestingValue {
    pub(crate) fn Create<P0>(compositor: P0) -> windows_core::Result<Self>
    where
        P0: windows_core::Param<Compositor>,
    {
        Self::IInteractionTrackerInertiaRestingValueStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).Create)(
                windows_core::Interface::as_raw(this),
                compositor.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    fn IInteractionTrackerInertiaRestingValueStatics<
        R,
        F: FnOnce(&IInteractionTrackerInertiaRestingValueStatics) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<
            InteractionTrackerInertiaRestingValue,
            IInteractionTrackerInertiaRestingValueStatics,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for InteractionTrackerInertiaRestingValue {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInteractionTrackerInertiaRestingValue>();
}
unsafe impl windows_core::Interface for InteractionTrackerInertiaRestingValue {
    type Vtable = <IInteractionTrackerInertiaRestingValue as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IInteractionTrackerInertiaRestingValue as windows_core::Interface>::IID;
}
impl core::ops::Deref for InteractionTrackerInertiaRestingValue {
    type Target = IInteractionTrackerInertiaRestingValue;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InteractionTrackerInertiaRestingValue {
    const NAME: &'static str =
        "Windows.UI.Composition.Interactions.InteractionTrackerInertiaRestingValue";
}
unsafe impl Send for InteractionTrackerInertiaRestingValue {}
unsafe impl Sync for InteractionTrackerInertiaRestingValue {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionTrackerInertiaStateEnteredArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InteractionTrackerInertiaStateEnteredArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for InteractionTrackerInertiaStateEnteredArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<
        Self,
        IInteractionTrackerInertiaStateEnteredArgs,
    >();
}
unsafe impl windows_core::Interface for InteractionTrackerInertiaStateEnteredArgs {
    type Vtable = <IInteractionTrackerInertiaStateEnteredArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IInteractionTrackerInertiaStateEnteredArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for InteractionTrackerInertiaStateEnteredArgs {
    type Target = IInteractionTrackerInertiaStateEnteredArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InteractionTrackerInertiaStateEnteredArgs {
    const NAME: &'static str =
        "Windows.UI.Composition.Interactions.InteractionTrackerInertiaStateEnteredArgs";
}
unsafe impl Send for InteractionTrackerInertiaStateEnteredArgs {}
unsafe impl Sync for InteractionTrackerInertiaStateEnteredArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionTrackerInteractingStateEnteredArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InteractionTrackerInteractingStateEnteredArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for InteractionTrackerInteractingStateEnteredArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<
        Self,
        IInteractionTrackerInteractingStateEnteredArgs,
    >();
}
unsafe impl windows_core::Interface for InteractionTrackerInteractingStateEnteredArgs {
    type Vtable =
        <IInteractionTrackerInteractingStateEnteredArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IInteractionTrackerInteractingStateEnteredArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for InteractionTrackerInteractingStateEnteredArgs {
    type Target = IInteractionTrackerInteractingStateEnteredArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InteractionTrackerInteractingStateEnteredArgs {
    const NAME: &'static str =
        "Windows.UI.Composition.Interactions.InteractionTrackerInteractingStateEnteredArgs";
}
unsafe impl Send for InteractionTrackerInteractingStateEnteredArgs {}
unsafe impl Sync for InteractionTrackerInteractingStateEnteredArgs {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InteractionTrackerPositionUpdateOption(pub i32);
impl InteractionTrackerPositionUpdateOption {
    pub const Default: Self = Self(0);
    pub const AllowActiveCustomScaleAnimation: Self = Self(1);
}
impl windows_core::TypeKind for InteractionTrackerPositionUpdateOption {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for InteractionTrackerPositionUpdateOption {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.Interactions.InteractionTrackerPositionUpdateOption;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionTrackerRequestIgnoredArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InteractionTrackerRequestIgnoredArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for InteractionTrackerRequestIgnoredArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInteractionTrackerRequestIgnoredArgs>();
}
unsafe impl windows_core::Interface for InteractionTrackerRequestIgnoredArgs {
    type Vtable = <IInteractionTrackerRequestIgnoredArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IInteractionTrackerRequestIgnoredArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for InteractionTrackerRequestIgnoredArgs {
    type Target = IInteractionTrackerRequestIgnoredArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InteractionTrackerRequestIgnoredArgs {
    const NAME: &'static str =
        "Windows.UI.Composition.Interactions.InteractionTrackerRequestIgnoredArgs";
}
unsafe impl Send for InteractionTrackerRequestIgnoredArgs {}
unsafe impl Sync for InteractionTrackerRequestIgnoredArgs {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionTrackerValuesChangedArgs(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    InteractionTrackerValuesChangedArgs,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for InteractionTrackerValuesChangedArgs {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IInteractionTrackerValuesChangedArgs>();
}
unsafe impl windows_core::Interface for InteractionTrackerValuesChangedArgs {
    type Vtable = <IInteractionTrackerValuesChangedArgs as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <IInteractionTrackerValuesChangedArgs as windows_core::Interface>::IID;
}
impl core::ops::Deref for InteractionTrackerValuesChangedArgs {
    type Target = IInteractionTrackerValuesChangedArgs;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for InteractionTrackerValuesChangedArgs {
    const NAME: &'static str =
        "Windows.UI.Composition.Interactions.InteractionTrackerValuesChangedArgs";
}
unsafe impl Send for InteractionTrackerValuesChangedArgs {}
unsafe impl Sync for InteractionTrackerValuesChangedArgs {}
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
pub struct RectangleClip(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    RectangleClip,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(RectangleClip, CompositionClip, CompositionObject);
impl windows_core::RuntimeType for RectangleClip {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IRectangleClip>();
}
unsafe impl windows_core::Interface for RectangleClip {
    type Vtable = <IRectangleClip as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IRectangleClip as windows_core::Interface>::IID;
}
impl core::ops::Deref for RectangleClip {
    type Target = IRectangleClip;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for RectangleClip {
    const NAME: &'static str = "Windows.UI.Composition.RectangleClip";
}
unsafe impl Send for RectangleClip {}
unsafe impl Sync for RectangleClip {}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SIZE {
    pub cx: i32,
    pub cy: i32,
}
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
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualInteractionSource(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    VisualInteractionSource,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    VisualInteractionSource,
    ICompositionInteractionSource,
    CompositionObject
);
impl VisualInteractionSource {
    pub(crate) fn Create<P0>(source: P0) -> windows_core::Result<Self>
    where
        P0: windows_core::Param<Visual>,
    {
        Self::IVisualInteractionSourceStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(this).Create)(
                windows_core::Interface::as_raw(this),
                source.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        })
    }
    fn IVisualInteractionSourceStatics<
        R,
        F: FnOnce(&IVisualInteractionSourceStatics) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<
            VisualInteractionSource,
            IVisualInteractionSourceStatics,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for VisualInteractionSource {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IVisualInteractionSource>();
}
unsafe impl windows_core::Interface for VisualInteractionSource {
    type Vtable = <IVisualInteractionSource as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IVisualInteractionSource as windows_core::Interface>::IID;
}
impl core::ops::Deref for VisualInteractionSource {
    type Target = IVisualInteractionSource;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for VisualInteractionSource {
    const NAME: &'static str = "Windows.UI.Composition.Interactions.VisualInteractionSource";
}
unsafe impl Send for VisualInteractionSource {}
unsafe impl Sync for VisualInteractionSource {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VisualInteractionSourceRedirectionMode(pub i32);
impl VisualInteractionSourceRedirectionMode {
    pub const Off: Self = Self(0);
    pub const CapableTouchpadOnly: Self = Self(1);
    pub const PointerWheelOnly: Self = Self(2);
    pub const CapableTouchpadAndPointerWheel: Self = Self(3);
}
impl windows_core::TypeKind for VisualInteractionSourceRedirectionMode {
    type TypeKind = windows_core::CopyType;
}
impl windows_core::RuntimeType for VisualInteractionSourceRedirectionMode {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Windows.UI.Composition.Interactions.VisualInteractionSourceRedirectionMode;i4)",
    );
}
