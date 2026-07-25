#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnimationController(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    AnimationController,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(AnimationController, CompositionObject);
impl windows_core::RuntimeType for AnimationController {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IAnimationController>();
}
unsafe impl windows_core::Interface for AnimationController {
    type Vtable = <IAnimationController as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IAnimationController as windows_core::Interface>::IID;
}
impl core::ops::Deref for AnimationController {
    type Target = IAnimationController;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for AnimationController {
    const NAME: &'static str = "Microsoft.UI.Composition.AnimationController";
}
unsafe impl Send for AnimationController {}
unsafe impl Sync for AnimationController {}
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
        b"enum(Microsoft.UI.Composition.AnimationIterationBehavior;i4)",
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionAnimation";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionBatchCompletedEventArgs";
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
        b"enum(Microsoft.UI.Composition.CompositionBatchTypes;u4)",
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
        b"enum(Microsoft.UI.Composition.CompositionBorderMode;i4)",
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionBrush";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionClip";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionColorBrush";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionColorGradientStop";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionColorGradientStopCollection";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionContainerShape";
}
unsafe impl Send for CompositionContainerShape {}
unsafe impl Sync for CompositionContainerShape {}
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
        b"enum(Microsoft.UI.Composition.CompositionDropShadowSourcePolicy;i4)",
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionEasingFunction";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionEllipseGeometry";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionGeometricClip";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionGeometry";
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
        b"enum(Microsoft.UI.Composition.CompositionGetValueStatus;i4)",
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionGradientBrush";
}
unsafe impl Send for CompositionGradientBrush {}
unsafe impl Sync for CompositionGradientBrush {}
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionLinearGradientBrush";
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
        b"enum(Microsoft.UI.Composition.CompositionMappingMode;i4)",
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionMaskBrush";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionNineGridBrush";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionObject";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionPath";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionPathGeometry";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionPropertySet";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionRoundedRectangleGeometry";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionScopedBatch";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionShadow";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionShape";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionShapeCollection";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionSpriteShape";
}
unsafe impl Send for CompositionSpriteShape {}
unsafe impl Sync for CompositionSpriteShape {}
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
        b"enum(Microsoft.UI.Composition.CompositionStrokeCap;i4)",
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
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionStrokeDashArray";
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
        b"enum(Microsoft.UI.Composition.CompositionStrokeLineJoin;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Compositor(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    Compositor,
    windows_core::IUnknown,
    windows_core::IInspectable
);
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
    const NAME: &'static str = "Microsoft.UI.Composition.Compositor";
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
    const NAME: &'static str = "Microsoft.UI.Composition.ContainerVisual";
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
    const NAME: &'static str = "Microsoft.UI.Composition.CubicBezierEasingFunction";
}
unsafe impl Send for CubicBezierEasingFunction {}
unsafe impl Sync for CubicBezierEasingFunction {}
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
    const NAME: &'static str = "Microsoft.UI.Composition.DropShadow";
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
    const NAME: &'static str = "Microsoft.UI.Composition.ExpressionAnimation";
}
unsafe impl Send for ExpressionAnimation {}
unsafe impl Sync for ExpressionAnimation {}
windows_core::imp::define_interface!(
    IAnimationController,
    IAnimationController_Vtbl,
    0xbd0f88a8_a415_5322_8c3d_1a6d9192754f
);
impl windows_core::RuntimeType for IAnimationController {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IAnimationController_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionAnimation,
    ICompositionAnimation_Vtbl,
    0xa829ccc8_6fde_5b90_ad37_efd307e1b631
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
    0x0926eb58_8965_5c74_bdac_852ebb5e8542
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
    0xa77c0e5a_f059_4e85_bcef_c068694cec78
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
impl windows_core::RuntimeName for ICompositionAnimationBase {
    const NAME: &'static str = "Microsoft.UI.Composition.ICompositionAnimationBase";
}
pub trait ICompositionAnimationBase_Impl: windows_core::IUnknownImpl {}
impl ICompositionAnimationBase_Vtbl {
    pub const fn new<Identity: ICompositionAnimationBase_Impl, const OFFSET: isize>() -> Self {
        Self {
            base__: windows_core::IInspectable_Vtbl::new::<
                Identity,
                ICompositionAnimationBase,
                OFFSET,
            >(),
        }
    }
    pub fn matches(iid: &windows_core::GUID) -> bool {
        iid == &<ICompositionAnimationBase as windows_core::Interface>::IID
    }
}
#[repr(C)]
pub struct ICompositionAnimationBase_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionBatchCompletedEventArgs,
    ICompositionBatchCompletedEventArgs_Vtbl,
    0xac400334_4358_5fb0_bfc3_117fe581998f
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
    0x483924e7_99a5_5377_968b_dec6d40bbccd
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
    0xb66b55cb_b5a5_5bee_8972_ae78233cb34c
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
    0x3f8ffb69_3e71_55a7_8e79_f27a214c56ae
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
    0x868030d8_c5c0_5dd8_a765_e32cd3497aa9
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
    0x4794cb29_ce9d_5837_9fc9_847df3e197de
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
    0x064aabd5_2dab_52d3_824b_c72456540f29
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
    ICompositionEasingFunction,
    ICompositionEasingFunction_Vtbl,
    0x8e1ecd0d_57d8_5bc9_9bcd_e43d0dd733c4
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
    0xf2a21042_7a57_58c1_8b47_8bc8b21d3aa0
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
    0xe3c0a8c5_5297_5d69_8b91_f5330bb1cb27
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
    0x4e40bdb2_450b_5a81_9e9b_149417980cc4
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
    0x9165d1fb_c738_5f44_addc_309ee071d588
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
    0x03e9edf3_ee3d_58fc_8c0b_95e4b6060a94
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
    ICompositionLinearGradientBrush,
    ICompositionLinearGradientBrush_Vtbl,
    0xf726fb52_270c_58b1_a902_89576772dedc
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
    0xc9502786_65aa_5be5_a679_c3b5dbfb0dc6
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
    0xc77a3d21_c7ee_517a_98f4_ad9a7202bc86
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
    0x0e583d49_fb5e_5481_a426_d3c41e059a5a
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
    0xbcbbfebf_799c_51ce_9c82_b6e49e7e62e1
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
    ICompositionObject4,
    ICompositionObject4_Vtbl,
    0x029ad57c_5497_51f3_8b40_e4e235992c6f
);
impl windows_core::RuntimeType for ICompositionObject4 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionObject4 {
    pub(crate) fn TryGetAnimationController(
        &self,
        propertyname: &str,
    ) -> windows_core::Result<AnimationController> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).TryGetAnimationController)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(propertyname)),
                &mut result__,
            )
            .and_then(|| windows_core::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositionObject4_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub TryGetAnimationController: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionPath,
    ICompositionPath_Vtbl,
    0x17c518c0_d7a2_54be_ac7f_c408562a6a20
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
    0x87143312_d280_51d2_b75b_5d76ea86c285
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
    0x0f6e6b82_060b_571e_b849_ab8e0d723962
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
    0x97f7a17b_97be_5545_9f1c_0b9d44577f57
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
    0x02eafc87_8d1f_5445_a416_d81baee8a750
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
    0xd31ca572_99ce_5969_b042_6c2d330a3859
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
    0x176b8bb5_4dae_59b2_b9a0_2499b243267c
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
    0xed75d4d8_437f_5640_9720_faae35ce5895
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
    0x982138f2_5781_509a_ba5d_112bcb0b98ef
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
    ICompositor,
    ICompositor_Vtbl,
    0x95213c13_c4cb_57de_b267_d21ab901ae38
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
    CreateSurfaceBrushWithSurface: usize,
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
    0xa9ffedad_3982_576d_a38a_c888ff605819
);
impl windows_core::RuntimeType for ICompositor2 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositor2 {
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
    CreateAnimationGroup: usize,
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
    0xee444bca_735c_5cf2_b79d_a6dd46c16160
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
    0xbb52d644_a030_5c19_b883_577ded739ae7
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
    0x5fd20200_c188_5737_b567_5e61741af19c
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
    0x5358b81b_4799_5e7c_906c_e3df3b2e95eb
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
    IContainerVisual,
    IContainerVisual_Vtbl,
    0xc70dbce1_2c2f_5d8e_91a4_aae1121e6186
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
    0x35e7fcde_f9ce_590a_8b88_64a82a6b4b48
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
    IDropShadow,
    IDropShadow_Vtbl,
    0x977095d2_265f_5f58_9789_cb7f85e98c9e
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
    0xcbc7c266_2d33_5711_b3b4_5699410bef56
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
    0x7fff5826_1992_56c0_9060_5ade561a4f2d
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
    IImplicitAnimationCollection,
    IImplicitAnimationCollection_Vtbl,
    0xc5c0689e_f5ae_5bed_829b_c522cda39717
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
    0xf9d99475_7b59_5b28_a1d2_b832da6988c9
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
    IKeyFrameAnimation,
    IKeyFrameAnimation_Vtbl,
    0x5a8f57f0_f059_5b47_b308_c4c80fc71248
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
    0x79bfeef6_70c7_50a6_bb3a_0e9636148695
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
    0x2ff54ebb_a035_50be_a73c_20014975ee31
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
    0xbc261502_2259_5c01_8616_556110a09657
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
    0x5a5f8abe_d129_5b25_8aff_8180fd9bfb22
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
    0x903224b0_2dbe_56eb_b9ea_33d2e3113181
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
    IShapeVisual,
    IShapeVisual_Vtbl,
    0xa911c80b_a5a5_5aca_b8ff_c43f08f06143
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
    0x6f720afd_d3d0_5e78_9c2e_83092f6ea45e
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
    0x5b4580a4_2594_521e_b288_eaf2c2a16a66
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
    0x02d9e1c6_7434_5464_a1f6_d644ce1fc9a6
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
    0x7e964632_45e4_5761_806d_5b4022c14f26
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
    0x3cb43662_9ecf_5128_850d_9eb8437774f0
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
    0xc7b1d4cf_c562_56b3_b4e6_b21326165f71
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
    0xe9c5e3fd_43b7_526e_9da0_4c3ea96db27d
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
    0xe00e60ce_d1be_5c24_885d_c1d0a749d109
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
    0xd7da980e_2dde_5dd1_a40c_d6868dd2449e
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
    0x13feeef2_d2c0_5b72_ad67_983a19b9b783
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
    0xc0eeab6c_c897_5ac6_a1c9_63abd5055b9b
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
    0x492a7995_0c5c_5993_a283_52e4da3050ee
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
    0x27cb223f_c431_57ac_b61e_d6b7515d579f
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
    0xd002896d_67d8_5f69_ab70_581fa3bf370f
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
    const NAME: &'static str = "Microsoft.UI.Composition.ImplicitAnimationCollection";
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
    const NAME: &'static str = "Microsoft.UI.Composition.InsetClip";
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
    const NAME: &'static str = "Microsoft.UI.Composition.KeyFrameAnimation";
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
    const NAME: &'static str = "Microsoft.UI.Composition.LinearEasingFunction";
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
    const NAME: &'static str = "Microsoft.UI.Composition.NaturalMotionAnimation";
}
unsafe impl Send for NaturalMotionAnimation {}
unsafe impl Sync for NaturalMotionAnimation {}
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
    const NAME: &'static str = "Microsoft.UI.Composition.RectangleClip";
}
unsafe impl Send for RectangleClip {}
unsafe impl Sync for RectangleClip {}
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
    const NAME: &'static str = "Microsoft.UI.Composition.ScalarKeyFrameAnimation";
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
    const NAME: &'static str = "Microsoft.UI.Composition.ScalarNaturalMotionAnimation";
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
    const NAME: &'static str = "Microsoft.UI.Composition.ShapeVisual";
}
unsafe impl Send for ShapeVisual {}
unsafe impl Sync for ShapeVisual {}
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
    const NAME: &'static str = "Microsoft.UI.Composition.SpringScalarNaturalMotionAnimation";
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
    const NAME: &'static str = "Microsoft.UI.Composition.SpringVector2NaturalMotionAnimation";
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
    const NAME: &'static str = "Microsoft.UI.Composition.SpringVector3NaturalMotionAnimation";
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
    const NAME: &'static str = "Microsoft.UI.Composition.SpriteVisual";
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
    const NAME: &'static str = "Microsoft.UI.Composition.StepEasingFunction";
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
    const NAME: &'static str = "Microsoft.UI.Composition.Vector2KeyFrameAnimation";
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
    const NAME: &'static str = "Microsoft.UI.Composition.Vector2NaturalMotionAnimation";
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
    const NAME: &'static str = "Microsoft.UI.Composition.Vector3KeyFrameAnimation";
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
    const NAME: &'static str = "Microsoft.UI.Composition.Vector3NaturalMotionAnimation";
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
    const NAME: &'static str = "Microsoft.UI.Composition.Visual";
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
    const NAME: &'static str = "Microsoft.UI.Composition.VisualCollection";
}
unsafe impl Send for VisualCollection {}
unsafe impl Sync for VisualCollection {}
