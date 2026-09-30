//! Effect brushes: a compositor-evaluated Direct2D effect graph over named brush
//! sources (feature `system`).
//!
//! The graph is described as plain data ([`EffectGraph`]) and materialized into the
//! COM objects the compositor walks through `IGraphicsEffectD2D1Interop` when the
//! factory is created. The walk re-reads the nodes' properties every frame, which is
//! what makes a named effect property (a Gaussian's `BlurAmount`) animate like any
//! other channel with the app rendering nothing.
//!
//! The point of the graph over a [`DropShadow`](super::DropShadow) followed by a
//! [`CompositionMaskBrush`](super::CompositionMaskBrush) is precision: a mask brush can
//! only take an already-rendered result, which a visual-surface capture delivers 8 bits
//! wide. A composite node multiplies the alpha into the colour *inside* the compositor's
//! float pipeline, so nothing between the Gaussian and the screen crosses an 8-bit
//! boundary.

// The hand-written interface carries the ABI's own names, and the transmutes are the
// ones every generated interface uses; the same allows the bindings module carries
// cover them.
#![allow(non_camel_case_types, non_snake_case, clippy::missing_transmute_annotations)]

use super::*;
use crate::bindings::{IGraphicsEffect, IGraphicsEffectSource};
use windows_core::implement_decl;

// C-PROVENANCE: the interface ABI, IID and enum values below come from the Windows SDK
// `windows.graphics.effects.interop.h` and `d2d1effects.h`/`d2d1_1.h`. The interface is
// absent from the Win32 corpus and from Windows.Graphics.Effects' generated projection,
// which is why it is declared here, in the vtable order the header carries.

/// The D2D property index of a Gaussian's standard deviation.
const BLUR_STANDARD_DEVIATION: u32 = 0;
/// The D2D property index of a composite's blend mode.
const COMPOSITE_MODE: u32 = 0;
/// The Gaussian's remaining D2D property indices.
const GAUSSIAN_OPTIMIZATION: u32 = 1;
const GAUSSIAN_BORDER_MODE: u32 = 2;
/// `D2D1_GAUSSIANBLUR_OPTIMIZATION_BALANCED`, the D2D default.
const GAUSSIAN_OPTIMIZATION_BALANCED: u32 = 1;
/// `D2D1_BORDER_MODE_SOFT`, the D2D default.
const D2D1_BORDER_MODE_SOFT: u32 = 0;
/// `D2D1_COMPOSITE_MODE_SOURCE_IN`.
const D2D1_COMPOSITE_MODE_SOURCE_IN: u32 = 2;

/// `CLSID_D2D1GaussianBlur`.
const CLSID_D2D1_GAUSSIAN_BLUR: windows_core::GUID =
    windows_core::GUID::from_u128(0x1feb6d69_2fe6_4ac9_8c58_1d7f93e7a6a5);
/// `CLSID_D2D1Composite`.
const CLSID_D2D1_COMPOSITE: windows_core::GUID =
    windows_core::GUID::from_u128(0x48fc9f51_f6ac_48f1_8b58_3b28ac46f76d);

const E_INVALIDARG: windows_core::HRESULT = windows_core::HRESULT(0x8007_0057_u32 as _);

/// `GRAPHICS_EFFECT_PROPERTY_MAPPING`: how an effect's named property maps onto its
/// indexed one.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyMapping {
    Unknown = 0,
    Direct = 1,
    VectorX = 2,
    VectorY = 3,
    VectorZ = 4,
    VectorW = 5,
    RectToVector4 = 6,
    RadiansToDegrees = 7,
    ColorMatrixAlphaMode = 8,
    ColorToVector3 = 9,
    ColorToVector4 = 10,
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
        *const u16,
        *mut u32,
        *mut PropertyMapping,
    ) -> windows_core::HRESULT,
    pub GetPropertyCount: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut u32,
    ) -> windows_core::HRESULT,
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
    pub GetSourceCount: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut u32,
    ) -> windows_core::HRESULT,
}
pub trait IGraphicsEffectD2D1Interop_Impl: windows_core::IUnknownImpl {
    /// Returns the D2D effect the node describes.
    fn effect_id(&self) -> windows_core::GUID;
    /// Returns the D2D property index and value mapping a named property carries, or an
    /// error when the node animates nothing by that name.
    fn named_property_mapping(
        &self,
        name: &str,
    ) -> Result<(u32, PropertyMapping)>;
    fn property_count(&self) -> u32;
    /// Returns the node's `index`th property value, as an `IPropertyValue`.
    fn property(&self, index: u32) -> Result<windows_core::IInspectable>;
    /// Returns the node's `index`th input: a nested node or a named brush parameter.
    fn source(&self, index: u32) -> Result<IGraphicsEffectSource>;
    fn source_count(&self) -> u32;
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
                if id.is_null() {
                    return windows_core::imp::E_POINTER;
                }
                *id = IGraphicsEffectD2D1Interop_Impl::effect_id(this);
                windows_core::HRESULT(0)
            }
        }
        unsafe extern "system" fn GetNamedPropertyMapping<
            Identity: IGraphicsEffectD2D1Interop_Impl,
            const OFFSET: isize,
        >(
            this: *mut core::ffi::c_void,
            name: *const u16,
            index: *mut u32,
            mapping: *mut PropertyMapping,
        ) -> windows_core::HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                let Some(name) = from_wide(name) else {
                    return windows_core::HRESULT(0x8007_0057_u32 as _);
                };
                match IGraphicsEffectD2D1Interop_Impl::named_property_mapping(this, &name) {
                    Ok((property, mapped)) => {
                        *index = property;
                        *mapping = mapped;
                        windows_core::HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
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
                *count = IGraphicsEffectD2D1Interop_Impl::property_count(this);
                windows_core::HRESULT(0)
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
                match IGraphicsEffectD2D1Interop_Impl::property(this, index)
                    .and_then(|value| value.cast::<bindings::IPropertyValue>())
                {
                    Ok(property) => {
                        value.write(core::mem::transmute(property));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => {
                        value.write(core::ptr::null_mut());
                        err.into()
                    }
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
                match IGraphicsEffectD2D1Interop_Impl::source(this, index) {
                    Ok(input) => {
                        source.write(core::mem::transmute(input));
                        windows_core::HRESULT(0)
                    }
                    Err(err) => {
                        source.write(core::ptr::null_mut());
                        err.into()
                    }
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
                *count = IGraphicsEffectD2D1Interop_Impl::source_count(this);
                windows_core::HRESULT(0)
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
        iid == &<IGraphicsEffectD2D1Interop as Interface>::IID
    }
}

/// Reads a null-terminated UTF-16 string, if the pointer is live and the text well-formed.
fn from_wide(name: *const u16) -> Option<String> {
    if name.is_null() {
        return None;
    }
    let mut len = 0usize;
    // SAFETY: the caller's contract is a null-terminated wide string; the scan stops at
    // the terminator and reads nothing past it.
    unsafe {
        while *name.add(len) != 0 {
            len += 1;
        }
        Some(String::from_utf16_lossy(core::slice::from_raw_parts(name, len)))
    }
}

/// A node of the graph the walk reads: one D2D effect with its named sources.
struct EffectNode {
    name: windows_core::HSTRING,
    kind: Kind,
    sources: Vec<IGraphicsEffectSource>,
}

enum Kind {
    /// A Gaussian blur. `sigma` is the initial standard deviation, in DIPs.
    GaussianBlur { sigma: f32 },
    /// A D2D composite of two inputs.
    Composite { mode: u32 },
}

impl Kind {
    fn effect_id(&self) -> windows_core::GUID {
        match self {
            Self::GaussianBlur { .. } => CLSID_D2D1_GAUSSIAN_BLUR,
            Self::Composite { .. } => CLSID_D2D1_COMPOSITE,
        }
    }

    fn property(&self, index: u32) -> Result<windows_core::IInspectable> {
        match (self, index) {
            (Self::GaussianBlur { sigma }, BLUR_STANDARD_DEVIATION) => {
                bindings::PropertyValue::CreateSingle(*sigma)
            }
            // The two properties the walk reads beyond the deviation, at the D2D
            // defaults: balanced optimization and a soft border.
            (Self::GaussianBlur { .. }, GAUSSIAN_OPTIMIZATION) => {
                bindings::PropertyValue::CreateUInt32(GAUSSIAN_OPTIMIZATION_BALANCED)
            }
            (Self::GaussianBlur { .. }, GAUSSIAN_BORDER_MODE) => {
                bindings::PropertyValue::CreateUInt32(D2D1_BORDER_MODE_SOFT)
            }
            (Self::Composite { mode }, COMPOSITE_MODE) => {
                bindings::PropertyValue::CreateUInt32(*mode)
            }
            _ => Err(windows_core::Error::from_hresult(E_INVALIDARG)),
        }
    }

    fn property_mapping(&self, name: &str) -> Result<(u32, PropertyMapping)> {
        match self {
            Self::GaussianBlur { .. } if name == "BlurAmount" => {
                Ok((BLUR_STANDARD_DEVIATION, PropertyMapping::Direct))
            }
            _ => Err(windows_core::Error::from_hresult(E_INVALIDARG)),
        }
    }
}

implement_decl! {
    impl EffectNode as EffectNode_Impl: [IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectD2D1Interop]
}

impl bindings::IGraphicsEffect_Impl for EffectNode_Impl {
    fn Name(&self) -> Result<windows_core::HSTRING> {
        Ok(self.name.clone())
    }

    fn SetName(&self, _value: &windows_core::HSTRING) -> Result<()> {
        Ok(())
    }
}

impl bindings::IGraphicsEffectSource_Impl for EffectNode_Impl {}

impl IGraphicsEffectD2D1Interop_Impl for EffectNode_Impl {
    fn effect_id(&self) -> windows_core::GUID {
        self.kind.effect_id()
    }

    fn named_property_mapping(
        &self,
        name: &str,
    ) -> Result<(u32, PropertyMapping)> {
        self.kind.property_mapping(name)
    }

    fn property_count(&self) -> u32 {
        match self.kind {
            // The full D2D property set of the effect the node names: the walk applies
            // every index it reads, and an effect whose reported count falls short of
            // the D2D effect's own is rejected.
            Kind::GaussianBlur { .. } => 3,
            Kind::Composite { .. } => 1,
        }
    }

    fn property(&self, index: u32) -> Result<windows_core::IInspectable> {
        self.kind.property(index)
    }

    fn source(&self, index: u32) -> Result<IGraphicsEffectSource> {
        self.sources
            .get(index as usize)
            .cloned()
            .ok_or_else(|| windows_core::Error::from_hresult(E_INVALIDARG))
    }

    fn source_count(&self) -> u32 {
        self.sources.len() as u32
    }
}

/// A compositor-evaluated effect graph, described as plain data.
///
/// Built once per [`Compositor::create_effect_factory`] call and never mutated after;
/// the compositor re-reads the materialized nodes every frame, which is what makes an
/// animatable property move without the app rendering anything.
pub enum EffectGraph {
    /// A brush bound by name when the factory's brush is built, with
    /// [`CompositionEffectBrush::set_source_parameter`].
    Parameter(&'static str),
    /// A Gaussian blur of one input. `name` prefixes its animatable properties, so the
    /// standard deviation animates as `"<name>.BlurAmount"`.
    GaussianBlur {
        name: &'static str,
        sigma: f32,
        input: Box<Self>,
    },
    /// A D2D composite of two inputs: `source` composited onto `destination` by `mode`.
    /// Named by role rather than by D2D input index, which puts the destination first.
    Composite {
        mode: CompositeMode,
        source: Box<Self>,
        destination: Box<Self>,
    },
}

/// The composite's blend mode. Only the modes this crate's constructions need are
/// named; the ABI value is the D2D one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompositeMode {
    /// The source shown where the destination has alpha: the source's colour multiplied
    /// by the destination's coverage.
    SourceIn,
}

impl From<CompositeMode> for u32 {
    fn from(mode: CompositeMode) -> Self {
        match mode {
            CompositeMode::SourceIn => D2D1_COMPOSITE_MODE_SOURCE_IN,
        }
    }
}

/// Materializes a description into the root of the object graph the walk reads.
///
/// Nodes own their sources, held as the `IGraphicsEffectSource` interface both a nested
/// node and a named parameter present to the walk, so a child lives as long as its
/// parent. The root is always a node, and the caller casts it to `IGraphicsEffect`.
fn materialize(graph: &EffectGraph) -> Result<IGraphicsEffectSource> {
    match graph {
        EffectGraph::Parameter(name) => {
            let parameter = bindings::CompositionEffectSourceParameter::Create(name)?;
            Ok(parameter.cast()?)
        }
        EffectGraph::GaussianBlur { name, sigma, input } => {
            let node = EffectNode {
                name: windows_core::HSTRING::from(*name),
                kind: Kind::GaussianBlur { sigma: *sigma },
                sources: vec![materialize(input)?],
            };
            Ok(node.into())
        }
        EffectGraph::Composite {
            mode,
            source,
            destination,
        } => {
            // D2D's composite reads input 0 as the destination and input 1 as the source,
            // bottom to top, which is the reverse of how the description names them.
            // Swapped, `SourceIn` shows the destination where the source has alpha: a glow
            // then paints the blurred silhouette's own colour at the tint's alpha, which is
            // right only where the paint happens to be the tint.
            let node = EffectNode {
                name: windows_core::HSTRING::new(),
                kind: Kind::Composite {
                    mode: (*mode).into(),
                },
                sources: vec![materialize(destination)?, materialize(source)?],
            };
            Ok(node.into())
        }
    }
}

/// A factory for brushes that evaluate a [`EffectGraph`].
#[derive(Clone)]
pub struct CompositionEffectFactory(pub(crate) bindings::CompositionEffectFactory);

impl CompositionEffectFactory {
    /// Creates the graph's brush, with every [`EffectGraph::Parameter`] unbound. Binding
    /// is a [`set_source_parameter`](CompositionEffectBrush::set_source_parameter) call
    /// on the brush, so re-pointing a source re-keys no factory and mints no brush.
    pub fn create_brush(&self) -> CompositionEffectBrush {
        CompositionEffectBrush(self.0.CreateBrush().unwrap())
    }

    /// Reports whether the compositor accepted the graph. The load completes
    /// asynchronously, so the status reads `Pending` until a commit has been
    /// processed; a factory whose graph fails to load produces brushes that paint
    /// nothing.
    pub fn load_status(&self) -> bindings::CompositionEffectFactoryLoadStatus {
        let factory: bindings::ICompositionEffectFactory = self.0.cast().unwrap();
        factory.LoadStatus().unwrap()
    }
}

/// A visual's brush that paints the output of a compositor-evaluated effect graph.
#[derive(Clone)]
pub struct CompositionEffectBrush(pub(crate) bindings::CompositionEffectBrush);

impl CompositionEffectBrush {
    /// Binds the named [`EffectGraph::Parameter`] to a brush.
    pub fn set_source_parameter(&self, name: &str, brush: &impl Brush) {
        self.0.SetSourceParameter(name, &brush.as_brush().0).unwrap();
    }
}

impl Sealed for CompositionEffectBrush {}

impl Brush for CompositionEffectBrush {
    fn as_brush(&self) -> CompositionBrush {
        CompositionBrush(self.0.cast().unwrap())
    }
}

impl Compositor {
    /// Creates a factory for `graph`, declaring the named effect properties animatable.
    ///
    /// A property absent from `animatable` refuses [`Animatable::start_animation`] on
    /// the brushes this factory creates, which is the platform's only gate: name there
    /// everything the brush will animate, as `"<effect name>.<property>"`.
    ///
    /// # Errors
    ///
    /// Returns an error when the graph cannot be materialized.
    pub fn create_effect_factory(
        &self,
        graph: &EffectGraph,
        animatable: &[&str],
    ) -> Result<CompositionEffectFactory> {
        let effect = materialize(graph)?.cast::<IGraphicsEffect>()?;
        let animatable: Vec<windows_core::HSTRING> =
            animatable.iter().map(|name| (*name).into()).collect();
        let names: windows_collections::IIterable<windows_core::HSTRING> = animatable.into();
        let factory = self
            .0
            .CreateEffectFactoryWithProperties(&effect, &names)?;
        Ok(CompositionEffectFactory(factory))
    }
}


#[cfg(all(test, feature = "system"))]
mod tests {
    use super::*;

    /// Pumps and commits until `factory`'s load leaves `Pending`.
    ///
    /// The load completes on a later commit than the one that first carries the brush, so
    /// a status read as soon as that commit completes can still be `Pending`.
    fn settle(
        compositor: &Compositor,
        factory: &CompositionEffectFactory,
        deadline: std::time::Instant,
    ) -> Result<()> {
        while factory.load_status() == bindings::CompositionEffectFactoryLoadStatus::Pending {
            assert!(std::time::Instant::now() < deadline, "the effect graph's load never settled");
            drop(compositor.request_commit()?);
            windows_window::pump();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        Ok(())
    }

    /// The glow graph loads, its sources bind, its sigma animates, and a commit with a
    /// live effect brush completes. Rendering the brush is the scene's verification; this
    /// is the API-level gate the construction depends on.
    #[test]
    fn glow_graph_loads_and_animates() -> Result<()> {
        let _queue = DispatcherQueueController::create_on_current_thread()?;
        let window = windows_window::Window::new("effect brush probe")
            .size(120, 80)
            .hidden()
            .create()?;
        let compositor = Compositor::new()?;
        let target = compositor.create_desktop_window_target(&window, false)?;
        let root = compositor.create_container_visual();
        target.set_root(&root);

        let graph = EffectGraph::Composite {
            mode: CompositeMode::SourceIn,
            source: Box::new(EffectGraph::Parameter("tint")),
            destination: Box::new(EffectGraph::GaussianBlur {
                name: "blur",
                sigma: 4.0,
                input: Box::new(EffectGraph::Parameter("silhouette")),
            }),
        };
        let factory = compositor.create_effect_factory(&graph, &["blur.BlurAmount"])?;
        assert!(
            factory.load_status() != bindings::CompositionEffectFactoryLoadStatus::Other,
            "the effect graph failed to load"
        );
        let brush = factory.create_brush();
        let white = compositor.create_color_brush(Color::rgb(255, 255, 255));
        brush.set_source_parameter("tint", &white);
        brush.set_source_parameter("silhouette", &white);

        let sprite = compositor.create_sprite_visual();
        sprite.set_size(100.0, 60.0);
        sprite.set_brush(&brush);
        root.children().insert_at_top(&sprite);

        let props = compositor.create_property_set();
        props.insert_scalar("Amount", 4.0);
        let expression = compositor.create_expression_animation("pulse.Amount");
        expression.set_reference_parameter("pulse", &props);
        brush.start_animation("blur.BlurAmount", &expression);

        let commit = compositor.request_commit()?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            drop(compositor.request_commit()?);
            windows_window::pump();
            if commit.Status()? == windows_future::AsyncStatus::Completed {
                commit.GetResults()?;
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "commit did not complete with a live effect brush"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        settle(&compositor, &factory, deadline)?;
        assert_eq!(
            factory.load_status(),
            bindings::CompositionEffectFactoryLoadStatus::Success,
            "the effect graph was not accepted"
        );
        Ok(())
    }
}

#[cfg(all(test, feature = "system"))]
mod spring_tests {
    use super::*;

    /// A spring on the effect property: the channel path a scene's spring would take.
    #[test]
    fn sigma_takes_a_spring() -> Result<()> {
        let _queue = DispatcherQueueController::create_on_current_thread()?;
        let window = windows_window::Window::new("effect spring probe")
            .size(120, 80).hidden().create()?;
        let compositor = Compositor::new()?;
        compositor.create_desktop_window_target(&window, false)?;
        let graph = EffectGraph::GaussianBlur {
            name: "blur",
            sigma: 4.0,
            input: Box::new(EffectGraph::Parameter("s")),
        };
        let factory = compositor.create_effect_factory(&graph, &["blur.BlurAmount"])?;
        let brush = factory.create_brush();
        let spring = compositor.create_spring_scalar_animation();
        spring.set_final_value(8.0);
        let started = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            brush.start_animation("blur.BlurAmount", &spring);
        }));
        println!("spring on blur.BlurAmount: {}", if started.is_ok() { "OK" } else { "REFUSED" });
        Ok(())
    }
}
