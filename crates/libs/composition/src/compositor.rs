use super::*;

/// Factory for visuals, brushes, and window targets.
#[derive(Clone)]
pub struct Compositor(pub(crate) bindings::Compositor);

#[cfg(all(test, feature = "system"))]
mod tests {
    use super::*;

    #[test]
    fn commit_completes_while_spring_targets_keep_changing() -> Result<()> {
        let _queue = DispatcherQueueController::create_on_current_thread()?;
        let window = windows_window::Window::new("composition commit test")
            .size(200, 100).hidden().create()?;
        let compositor = Compositor::new()?;
        let target = compositor.create_desktop_window_target(&window, false)?;
        let root = compositor.create_container_visual();
        target.set_root(&root);
        let spring = compositor.create_spring_vector2_animation();
        spring.set_period(std::time::Duration::from_millis(90));
        spring.set_damping_ratio(0.9);
        root.set_size(100.0, 100.0);
        let commit = compositor.request_commit()?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut updates = 0;
        loop {
            spring.set_final_value(Vector2::new(100.0 + (updates % 50) as f32, 100.0));
            root.start_animation("Size", &spring);
            drop(compositor.request_commit()?);
            windows_window::pump();
            updates += 1;
            if updates > 1 && commit.Status()? == windows_future::AsyncStatus::Completed {
                commit.GetResults()?;
                break;
            }
            assert!(std::time::Instant::now() < deadline, "commit did not complete during updates");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        Ok(())
    }
}

impl Compositor {
    /// Requests publication of pending composition changes and returns its completion action.
    pub fn request_commit(&self) -> Result<windows_future::IAsyncAction> {
        let compositor: bindings::ICompositor5 = self.0.cast()?;
        compositor.RequestCommitAsync()
    }

    /// Creates a compositor. A dispatcher queue must exist on the current thread.
    ///
    /// ```no_run
    /// use windows_composition::{Compositor, DispatcherQueueController};
    ///
    /// let _queue = DispatcherQueueController::create_on_current_thread()?;
    /// let compositor = Compositor::new()?;
    /// # windows_core::Result::Ok(())
    /// ```
    #[cfg(feature = "system")]
    pub fn new() -> Result<Self> {
        Ok(Self(bindings::Compositor::new()?))
    }

    /// Wraps a lifted compositor obtained from a WinUI host element.
    #[cfg(feature = "reactor")]
    pub fn from_host(compositor: windows_core::IInspectable) -> Result<Self> {
        Ok(Self(compositor.cast()?))
    }

    /// Creates a window target. Keep it alive while the visual tree is shown.
    ///
    /// ```no_run
    /// use windows_composition::{Compositor, DispatcherQueueController};
    /// use windows_window::Window;
    ///
    /// let window = Window::new("Composition").create()?;
    /// let _queue = DispatcherQueueController::create_on_current_thread()?;
    /// let compositor = Compositor::new()?;
    /// let target = compositor.create_desktop_window_target(&window, false)?;
    /// # windows_core::Result::Ok(())
    /// ```
    #[cfg(feature = "system")]
    pub fn create_desktop_window_target(
        &self,
        window: &windows_window::Window,
        is_topmost: bool,
    ) -> Result<DesktopWindowTarget> {
        self.create_desktop_window_target_for(window.handle(), is_topmost)
    }

    /// Creates a window target for a window another thread owns.
    ///
    /// The compositor and the window need not share a thread: the target is agile, and the
    /// system resolves the handle per call. What the token's contract asks is that this
    /// thread finishes with the target before the window closes, which the caller's own
    /// join order supplies.
    #[cfg(feature = "system")]
    pub fn create_desktop_window_target_for(
        &self,
        window: windows_window::Hwnd,
        is_topmost: bool,
    ) -> Result<DesktopWindowTarget> {
        windows_census::count!("comp.target");
        // SAFETY: the token names a window whose owner outlives this call by the token's
        // contract, and the call reads nothing through the handle.
        unsafe { self.create_desktop_window_target_for_hwnd(window.raw(), is_topmost) }
    }

    /// Creates a composition target for a raw window handle.
    ///
    /// # Safety
    ///
    /// `hwnd` must be a valid, live window handle owned by the calling thread.
    #[cfg(feature = "system")]
    pub unsafe fn create_desktop_window_target_for_hwnd(
        &self,
        hwnd: *mut core::ffi::c_void,
        is_topmost: bool,
    ) -> Result<DesktopWindowTarget> {
        let interop: bindings::ICompositorDesktopInterop = self.0.cast()?;
        let target = unsafe { interop.CreateDesktopWindowTarget(hwnd, is_topmost)? };
        Ok(DesktopWindowTarget::new(target))
    }

    /// Creates an empty container visual that hosts a child visual tree.
    pub fn create_container_visual(&self) -> ContainerVisual {
        windows_census::count!("comp.visual.container");
        ContainerVisual::new(self.0.CreateContainerVisual().unwrap())
    }

    /// Creates a sprite visual that paints itself with a brush.
    pub fn create_sprite_visual(&self) -> SpriteVisual {
        windows_census::count!("comp.visual.sprite");
        SpriteVisual::new(self.0.CreateSpriteVisual().unwrap())
    }

    /// Creates a solid-color brush.
    pub fn create_color_brush(&self, color: Color) -> CompositionColorBrush {
        windows_census::count!("comp.brush.color");
        CompositionColorBrush(self.0.CreateColorBrushWithColor(color.0).unwrap())
    }

    /// Creates a nine-grid brush.
    pub fn create_nine_grid_brush(&self) -> CompositionNineGridBrush {
        windows_census::count!("comp.brush.nine_grid");
        let compositor: bindings::ICompositor2 = self.0.cast().unwrap();
        CompositionNineGridBrush(compositor.CreateNineGridBrush().unwrap())
    }

    /// Creates a shape visual that renders composition shapes.
    pub fn create_shape_visual(&self) -> ShapeVisual {
        windows_census::count!("comp.visual.shape");
        let compositor: bindings::ICompositor5 = self.0.cast().unwrap();
        ShapeVisual::new(compositor.CreateShapeVisual().unwrap())
    }

    /// Creates an empty container shape that groups child shapes.
    pub fn create_container_shape(&self) -> CompositionContainerShape {
        windows_census::count!("comp.shape.container");
        let compositor: bindings::ICompositor5 = self.0.cast().unwrap();
        CompositionContainerShape(compositor.CreateContainerShape().unwrap())
    }

    /// Creates an ellipse geometry.
    pub fn create_ellipse_geometry(&self) -> CompositionEllipseGeometry {
        windows_census::count!("comp.geometry.ellipse");
        let compositor: bindings::ICompositor5 = self.0.cast().unwrap();
        CompositionEllipseGeometry(compositor.CreateEllipseGeometry().unwrap())
    }

    /// Creates a sprite shape that fills the given geometry with a brush.
    pub fn create_sprite_shape(&self, geometry: &impl Geometry) -> CompositionSpriteShape {
        windows_census::count!("comp.shape.sprite");
        let compositor: bindings::ICompositor5 = self.0.cast().unwrap();
        CompositionSpriteShape(
            compositor
                .CreateSpriteShapeWithGeometry(&geometry.as_geometry().0)
                .unwrap(),
        )
    }

    /// Creates a scoped batch that tracks the completion of the given kind of
    /// work.
    pub fn create_scoped_batch(&self, kind: BatchKind) -> CompositionScopedBatch {
        windows_census::count!("comp.batch");
        CompositionScopedBatch(self.0.CreateScopedBatch(kind.into()).unwrap())
    }

    /// Creates a `Vector3` key-frame animation.
    pub fn create_vector3_key_frame_animation(&self) -> Vector3KeyFrameAnimation {
        windows_census::count!("comp.animation.key_frame");
        Vector3KeyFrameAnimation(self.0.CreateVector3KeyFrameAnimation().unwrap())
    }

    /// Creates an empty group whose animations start together.
    pub fn create_animation_group(&self) -> CompositionAnimationGroup {
        windows_census::count!("comp.animation.group");
        let compositor: bindings::ICompositor2 = self.0.cast().unwrap();
        CompositionAnimationGroup(compositor.CreateAnimationGroup().unwrap())
    }

    /// Creates a scalar (`f32`) key-frame animation.
    pub fn create_scalar_key_frame_animation(&self) -> ScalarKeyFrameAnimation {
        windows_census::count!("comp.animation.key_frame");
        ScalarKeyFrameAnimation(self.0.CreateScalarKeyFrameAnimation().unwrap())
    }

    /// Creates a linear easing function.
    pub fn create_linear_easing_function(&self) -> CompositionEasingFunction {
        windows_census::count!("comp.easing");
        CompositionEasingFunction(self.0.CreateLinearEasingFunction().unwrap().cast().unwrap())
    }

    /// Creates a cubic-bezier easing function through the two control points
    /// (each in `0.0..=1.0`), matching the CSS `cubic-bezier()` convention.
    pub fn create_cubic_bezier_easing_function(
        &self,
        control1: Vector2,
        control2: Vector2,
    ) -> CompositionEasingFunction {
        windows_census::count!("comp.easing");
        CompositionEasingFunction(
            self.0
                .CreateCubicBezierEasingFunction(control1, control2)
                .unwrap()
                .cast()
                .unwrap(),
        )
    }

    /// Creates an empty implicit-animation collection to attach to a visual
    /// with [`Visual::set_implicit_animations`](crate::Visual::set_implicit_animations).
    pub fn create_implicit_animation_collection(&self) -> ImplicitAnimationCollection {
        windows_census::count!("comp.animation.implicit");
        let compositor: bindings::ICompositor2 = self.0.cast().unwrap();
        ImplicitAnimationCollection(compositor.CreateImplicitAnimationCollection().unwrap())
    }

    /// Creates a composition graphics device backed by a Direct2D or DXGI device.
    #[cfg(feature = "system")]
    pub fn create_graphics_device(
        &self,
        rendering_device: &impl Interface,
    ) -> Result<CompositionGraphicsDevice> {
        windows_census::count!("comp.graphics_device");
        let interop: bindings::ICompositorInterop = self.0.cast()?;
        let device: windows_core::IUnknown = rendering_device.cast()?;
        let graphics = unsafe { interop.CreateGraphicsDevice(&device)? };
        Ok(CompositionGraphicsDevice(graphics.cast()?))
    }

    /// Creates a brush that paints a visual with any composition [`Surface`] — drawn
    /// pixels, a captured subtree, or a buffer the app presents itself.
    #[cfg(feature = "system")]
    pub fn create_surface_brush(&self, surface: &impl Surface) -> CompositionSurfaceBrush {
        windows_census::count!("comp.brush.surface");
        CompositionSurfaceBrush(
            self.0
                .CreateSurfaceBrushWithSurface(&surface.as_surface().0)
                .unwrap(),
        )
    }
}
