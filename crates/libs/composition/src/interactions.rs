//! Interaction trackers: values the composition engine drives from a manipulation and from
//! inertia (feature `system`).
//!
//! The engine runs in another process, so every call into a tracker and every callback out
//! of it is asynchronous. Three consequences run through this module:
//!
//! - A request that arrives in a state the tracker will not accept it in is dropped rather
//!   than failed. Every `try_update_*` returns the [`RequestId`] the tracker assigned, and
//!   a caller that cares reconciles it against [`TrackerEvent::RequestIgnored`].
//! - The tracker's position cannot be read directly, and this crate carries no such
//!   getter. The value delivered by [`TrackerEvent::ValuesChanged`] is the only
//!   trustworthy one.
//! - An owner is supplied at construction, there is no per-callback subscription, and the
//!   process crossing dominates its cost. A tracker whose motion nothing observes is
//!   created with [`Compositor::create_interaction_tracker`] and no owner.

use super::*;

/// The id a tracker assigns to one position or scale request.
///
/// A request that arrives in the wrong state is dropped by the system rather than
/// failing, and the id is how a caller learns which one: hold it until either a
/// [`TrackerEvent::ValuesChanged`] supersedes it or a
/// [`TrackerEvent::RequestIgnored`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RequestId(pub i32);

/// Which manipulations an axis of a [`VisualInteractionSource`] drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMode {
    /// The axis does not participate.
    Disabled,
    /// The axis is driven, and a release carries momentum into inertia.
    EnabledWithInertia,
    /// The axis is driven, and motion stops dead on release.
    EnabledWithoutInertia,
}

impl From<SourceMode> for bindings::InteractionSourceMode {
    fn from(mode: SourceMode) -> Self {
        match mode {
            SourceMode::Disabled => Self::Disabled,
            SourceMode::EnabledWithInertia => Self::EnabledWithInertia,
            SourceMode::EnabledWithoutInertia => Self::EnabledWithoutInertia,
        }
    }
}

/// Which axes wheel input drives, configured independently of touch and pen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelMode {
    /// Wheel input does not drive this axis.
    Disabled,
    /// Wheel input drives this axis, compositor-side, with no front-thread handling at
    /// all.
    Enabled,
}

impl From<WheelMode> for bindings::InteractionSourceRedirectionMode {
    fn from(mode: WheelMode) -> Self {
        match mode {
            WheelMode::Disabled => Self::Disabled,
            WheelMode::Enabled => Self::Enabled,
        }
    }
}

/// Which input the system hands to a source without the window's help.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedirectionMode {
    /// Nothing is redirected; every contact must be handed over explicitly.
    Off,
    /// Precision-touchpad pan and zoom arrive at the source directly.
    TouchpadOnly,
    /// Wheel input arrives at the source directly.
    WheelOnly,
    /// Both precision touchpad and wheel arrive directly. The usual choice.
    TouchpadAndWheel,
}

impl From<RedirectionMode> for bindings::VisualInteractionSourceRedirectionMode {
    fn from(mode: RedirectionMode) -> Self {
        match mode {
            RedirectionMode::Off => Self::Off,
            RedirectionMode::TouchpadOnly => Self::CapableTouchpadOnly,
            RedirectionMode::WheelOnly => Self::PointerWheelOnly,
            RedirectionMode::TouchpadAndWheel => Self::CapableTouchpadAndPointerWheel,
        }
    }
}

/// What happens when a manipulation reaches this tracker's bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainingMode {
    /// Hand off to an enclosing tracker if there is one, so nested scrollers chain
    /// without app-side plumbing.
    Auto,
    /// Always hand off, even with nothing to hand off to.
    Always,
    /// Never hand off — for a self-contained pan surface inside a scrolling page.
    Never,
}

impl From<ChainingMode> for bindings::InteractionChainingMode {
    fn from(mode: ChainingMode) -> Self {
        match mode {
            ChainingMode::Auto => Self::Auto,
            ChainingMode::Always => Self::Always,
            ChainingMode::Never => Self::Never,
        }
    }
}

/// Whether a position correction is held inside the tracker's bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clamping {
    /// Let the tracker clamp as it would for a manipulation.
    Auto,
    /// Apply the value as given, even outside `MinPosition`/`MaxPosition`.
    Disabled,
}

impl From<Clamping> for bindings::InteractionTrackerClampingOption {
    fn from(option: Clamping) -> Self {
        match option {
            Clamping::Auto => Self::Auto,
            Clamping::Disabled => Self::Disabled,
        }
    }
}

/// Whether a position update stops a running custom scale animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleAnimationPolicy {
    /// A position update ends any custom scale animation in flight.
    Stop,
    /// A running custom scale animation survives the position update.
    Keep,
}

impl From<ScaleAnimationPolicy> for bindings::InteractionTrackerPositionUpdateOption {
    fn from(option: ScaleAnimationPolicy) -> Self {
        match option {
            ScaleAnimationPolicy::Stop => Self::Default,
            ScaleAnimationPolicy::Keep => Self::AllowActiveCustomScaleAnimation,
        }
    }
}

/// Which axes two bound trackers share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BindingAxes {
    /// Share the X position.
    pub x: bool,
    /// Share the Y position.
    pub y: bool,
    /// Share the scale.
    pub scale: bool,
}

impl From<BindingAxes> for bindings::InteractionBindingAxisModes {
    fn from(axes: BindingAxes) -> Self {
        let mut modes = Self::None;
        if axes.x {
            modes |= Self::PositionX;
        }
        if axes.y {
            modes |= Self::PositionY;
        }
        if axes.scale {
            modes |= Self::Scale;
        }
        modes
    }
}

/// What a tracker reports to its owner.
///
/// Delivered on the thread that owns the compositor. Every variant carries the
/// [`RequestId`] that caused the transition, which is `RequestId(0)` when the cause was
/// the user rather than a request.
#[derive(Clone, Copy, Debug)]
pub enum TrackerEvent {
    /// The tracker's value moved. **These fields are the only trustworthy read of it.**
    ValuesChanged {
        /// The tracker's position. Increases for up/left motion, so the canonical
        /// content binding is its negation.
        position: Vector3,
        /// The tracker's scale.
        scale: f32,
        /// The request this value came from, if any.
        request: RequestId,
    },
    /// A contact took hold and the user is now driving the tracker.
    InteractingStateEntered {
        /// The request this transition came from, if any.
        request: RequestId,
        /// Whether this transition came from a tracker bound to this one rather than from
        /// this tracker's own input. Always `false` unless
        /// [`InteractionTracker::bind`] was used.
        from_binding: bool,
    },
    /// The contact was released and the compositor is now animating the fling.
    ///
    /// Arrives the instant inertia begins, when the resting position is already known, so
    /// a consumer can prefetch the destination while the compositor animates.
    InertiaStateEntered {
        /// Where the motion would rest with no modifiers applied.
        natural_resting_position: Vector3,
        /// Where it will actually rest once snap points are applied.
        modified_resting_position: Vector3,
        /// The scale it would rest at with no modifiers applied.
        natural_resting_scale: f32,
        /// The scale it will actually rest at.
        modified_resting_scale: f32,
        /// The velocity inertia started with, in pixels per second.
        position_velocity: Vector3,
        /// Whether the motion came from an impulse (a wheel notch) rather than a fling,
        /// which usually takes a shorter decay rate.
        from_impulse: bool,
        /// Whether this transition came from a tracker bound to this one rather than from
        /// this tracker's own input.
        from_binding: bool,
        /// The request this transition came from, if any.
        request: RequestId,
    },
    /// The tracker came to rest.
    IdleStateEntered {
        /// The request this transition came from, if any.
        request: RequestId,
        /// Whether this transition came from a tracker bound to this one rather than from
        /// this tracker's own input. Always `false` unless
        /// [`InteractionTracker::bind`] was used.
        from_binding: bool,
    },
    /// An animation the app supplied is driving the tracker.
    CustomAnimationStateEntered {
        /// The request this transition came from, if any.
        request: RequestId,
        /// Whether this transition came from a tracker bound to this one rather than from
        /// this tracker's own input. Always `false` unless
        /// [`InteractionTracker::bind`] was used.
        from_binding: bool,
    },
    /// A request was dropped. Not an error: a position update that arrives while the user
    /// is manipulating the tracker is ignored.
    ///
    /// Discard the pending request and reconcile against the next `ValuesChanged`.
    /// Re-applying it blindly double-jumps the position once the manipulation ends.
    RequestIgnored {
        /// The request that did not take.
        request: RequestId,
    },
}

/// The `IInteractionTrackerOwner` implementation, so no generated COM type has to cross
/// a crate boundary for an app to receive tracker events.
///
/// The callback is `FnMut` because a consumer reconciles tracker state it owns; the
/// `RefCell` adapts that to the `Fn` shape a COM vtable requires. Callbacks arrive on the
/// compositor's own thread, which is the thread that created it, so nothing here needs to
/// be `Send`.
#[windows_core::implement(bindings::IInteractionTrackerOwner)]
struct Owner(core::cell::RefCell<Box<dyn FnMut(TrackerEvent)>>);

impl Owner {
    fn deliver(&self, event: TrackerEvent) {
        // A re-entrant raise would panic on the double borrow and unwind across the COM
        // boundary; failing the borrow drops the event instead.
        if let Ok(mut handler) = self.0.try_borrow_mut() {
            handler(event);
        }
    }
}

/// Reads `IsFromBinding` off whichever revision of an args type carries it.
///
/// Each args type carries the flag on its own later interface, so the interface is named
/// per call site; one shared cast would report `false` for every args type but the one it
/// named. A missing interface reads as `false`, which is correct — a system that does not
/// carry the flag is one on which nothing can be bound.
macro_rules! from_binding {
    ($args:expr, $iface:ident) => {
        $args
            .cast::<bindings::$iface>()
            .and_then(|args| args.IsFromBinding())
            .unwrap_or(false)
    };
}

impl bindings::IInteractionTrackerOwner_Impl for Owner_Impl {
    fn ValuesChanged(
        &self,
        _sender: windows_core::Ref<bindings::InteractionTracker>,
        args: windows_core::Ref<bindings::InteractionTrackerValuesChangedArgs>,
    ) -> Result<()> {
        if let Some(args) = args.as_ref() {
            self.deliver(TrackerEvent::ValuesChanged {
                position: args.Position()?,
                scale: args.Scale()?,
                request: RequestId(args.RequestId()?),
            });
        }
        Ok(())
    }

    fn InteractingStateEntered(
        &self,
        _sender: windows_core::Ref<bindings::InteractionTracker>,
        args: windows_core::Ref<bindings::InteractionTrackerInteractingStateEnteredArgs>,
    ) -> Result<()> {
        if let Some(args) = args.as_ref() {
            self.deliver(TrackerEvent::InteractingStateEntered {
                request: RequestId(args.RequestId()?),
                from_binding: from_binding!(args, IInteractionTrackerInteractingStateEnteredArgs2),
            });
        }
        Ok(())
    }

    fn InertiaStateEntered(
        &self,
        _sender: windows_core::Ref<bindings::InteractionTracker>,
        args: windows_core::Ref<bindings::InteractionTrackerInertiaStateEnteredArgs>,
    ) -> Result<()> {
        if let Some(args) = args.as_ref() {
            self.deliver(TrackerEvent::InertiaStateEntered {
                natural_resting_position: args.NaturalRestingPosition()?,
                modified_resting_position: args.ModifiedRestingPosition()?,
                natural_resting_scale: args.NaturalRestingScale()?,
                modified_resting_scale: args.ModifiedRestingScale()?,
                position_velocity: args.PositionVelocityInPixelsPerSecond()?,
                from_impulse: args
                    .cast::<bindings::IInteractionTrackerInertiaStateEnteredArgs2>()
                    .and_then(|args| args.IsInertiaFromImpulse())
                    .unwrap_or(false),
                from_binding: from_binding!(args, IInteractionTrackerInertiaStateEnteredArgs3),
                request: RequestId(args.RequestId()?),
            });
        }
        Ok(())
    }

    fn IdleStateEntered(
        &self,
        _sender: windows_core::Ref<bindings::InteractionTracker>,
        args: windows_core::Ref<bindings::InteractionTrackerIdleStateEnteredArgs>,
    ) -> Result<()> {
        if let Some(args) = args.as_ref() {
            self.deliver(TrackerEvent::IdleStateEntered {
                request: RequestId(args.RequestId()?),
                from_binding: from_binding!(args, IInteractionTrackerIdleStateEnteredArgs2),
            });
        }
        Ok(())
    }

    fn CustomAnimationStateEntered(
        &self,
        _sender: windows_core::Ref<bindings::InteractionTracker>,
        args: windows_core::Ref<bindings::InteractionTrackerCustomAnimationStateEnteredArgs>,
    ) -> Result<()> {
        if let Some(args) = args.as_ref() {
            self.deliver(TrackerEvent::CustomAnimationStateEntered {
                request: RequestId(args.RequestId()?),
                from_binding: from_binding!(
                    args,
                    IInteractionTrackerCustomAnimationStateEnteredArgs2
                ),
            });
        }
        Ok(())
    }

    fn RequestIgnored(
        &self,
        _sender: windows_core::Ref<bindings::InteractionTracker>,
        args: windows_core::Ref<bindings::InteractionTrackerRequestIgnoredArgs>,
    ) -> Result<()> {
        if let Some(args) = args.as_ref() {
            self.deliver(TrackerEvent::RequestIgnored {
                request: RequestId(args.RequestId()?),
            });
        }
        Ok(())
    }
}

/// A compositor-side value driven by a manipulation and by inertia.
///
/// Bind a sink to it with an [`ExpressionAnimation`] referencing it as an [`Animatable`].
/// The compositor evaluates that binding every vblank whether the app thread is asleep or
/// busy.
#[derive(Clone)]
pub struct InteractionTracker(pub(crate) bindings::InteractionTracker);

impl InteractionTracker {
    /// Adds a source whose manipulations drive this tracker.
    ///
    /// One source per surface is the normal case; several are legal when one tracker has
    /// several hit-test regions.
    pub fn add_source(&self, source: &VisualInteractionSource) -> Result<()> {
        let sources = self.interaction_sources()?;
        let source: bindings::ICompositionInteractionSource = source.0.cast()?;
        sources.Add(&source)
    }

    /// Removes a source.
    pub fn remove_source(&self, source: &VisualInteractionSource) -> Result<()> {
        let sources = self.interaction_sources()?;
        let source: bindings::ICompositionInteractionSource = source.0.cast()?;
        sources.Remove(&source)
    }

    /// Removes every source, so nothing drives this tracker.
    pub fn clear_sources(&self) -> Result<()> {
        self.interaction_sources()?.RemoveAll()
    }

    fn interaction_sources(&self) -> Result<bindings::CompositionInteractionSourceCollection> {
        let tracker: bindings::IInteractionTracker = self.0.cast()?;
        tracker.InteractionSources()
    }

    /// Sets the range the tracker rests inside.
    ///
    /// The position may travel outside the range while interacting or in inertia; that
    /// overpan is the bounce, so a consumer does not clamp its own reads.
    pub fn set_position_bounds(&self, min: Vector3, max: Vector3) {
        let tracker: bindings::IInteractionTracker = self.0.cast().unwrap();
        tracker.SetMinPosition(min).unwrap();
        tracker.SetMaxPosition(max).unwrap();
    }

    /// Returns the upper bound of the tracker's range, as last set.
    ///
    /// Unlike the position, this is a value the app itself wrote, so reading it back does
    /// not race the compositor.
    pub fn max_position(&self) -> Vector3 {
        let tracker: bindings::IInteractionTracker = self.0.cast().unwrap();
        tracker.MaxPosition().unwrap()
    }

    /// Returns the lower bound of the tracker's range, as last set.
    pub fn min_position(&self) -> Vector3 {
        let tracker: bindings::IInteractionTracker = self.0.cast().unwrap();
        tracker.MinPosition().unwrap()
    }

    /// Sets the scale range the tracker rests inside.
    pub fn set_scale_bounds(&self, min: f32, max: f32) {
        let tracker: bindings::IInteractionTracker = self.0.cast().unwrap();
        tracker.SetMinScale(min).unwrap();
        tracker.SetMaxScale(max).unwrap();
    }

    /// Sets how fast inertia decays per axis, in `0.0..=1.0`, or restores the system
    /// default with `None`.
    ///
    /// [`TrackerEvent::InertiaStateEntered`]'s `from_impulse` distinguishes a wheel notch
    /// from a fling, which usually take different rates.
    pub fn set_position_inertia_decay_rate(&self, rate: Option<Vector3>) {
        let tracker: bindings::IInteractionTracker = self.0.cast().unwrap();
        tracker.SetPositionInertiaDecayRate(rate).unwrap();
    }

    /// Sets how fast scale inertia decays, or restores the system default with `None`.
    pub fn set_scale_inertia_decay_rate(&self, rate: Option<f32>) {
        let tracker: bindings::IInteractionTracker = self.0.cast().unwrap();
        tracker.SetScaleInertiaDecayRate(rate).unwrap();
    }

    /// Moves the tracker to `position`.
    ///
    /// The request is ignored while the user is interacting. `scale_animation` states
    /// whether a running custom scale animation survives; the platform's own default
    /// stops it.
    pub fn try_update_position(
        &self,
        position: Vector3,
        clamping: Clamping,
        scale_animation: ScaleAnimationPolicy,
    ) -> Result<RequestId> {
        let tracker: bindings::IInteractionTracker5 = self.0.cast()?;
        Ok(RequestId(tracker.TryUpdatePositionWithOption(
            position,
            clamping.into(),
            scale_animation.into(),
        )?))
    }

    /// Moves the tracker by `delta`. Ignored while the user is interacting.
    ///
    /// The mouse-drag path: a mouse contact cannot be redirected into a manipulation, so
    /// the app writes one request per consumed sample while the button is down, and
    /// nothing after release.
    pub fn try_update_position_by(&self, delta: Vector3, clamping: Clamping) -> Result<RequestId> {
        let tracker: bindings::IInteractionTracker4 = self.0.cast()?;
        Ok(RequestId(
            tracker.TryUpdatePositionByWithOption(delta, clamping.into())?,
        ))
    }

    /// Hands a fling to the compositor: enters inertia with the given velocity, in pixels
    /// per second, and re-evaluates the inertia modifiers against it.
    ///
    /// After this the front thread does nothing at all until the motion rests.
    pub fn try_update_position_with_additional_velocity(
        &self,
        velocity: Vector3,
    ) -> Result<RequestId> {
        let tracker: bindings::IInteractionTracker = self.0.cast()?;
        Ok(RequestId(
            tracker.TryUpdatePositionWithAdditionalVelocity(velocity)?,
        ))
    }

    /// Hands the tracker's position to an animation of the app's own, entering the
    /// custom-animation state until it finishes.
    ///
    /// Moves the position rather than jumping it: scrolling a focused row into view,
    /// returning to the top, settling on a chosen tab. It is the only way to enter the
    /// state [`TrackerEvent::CustomAnimationStateEntered`] reports.
    ///
    /// The animation runs on the compositor like any other, so the front thread does
    /// nothing while it plays.
    pub fn try_update_position_with_animation(
        &self,
        animation: &impl Animation,
    ) -> Result<RequestId> {
        let tracker: bindings::IInteractionTracker = self.0.cast()?;
        Ok(RequestId(tracker.TryUpdatePositionWithAnimation(
            &animation.as_animation().0,
        )?))
    }

    /// Sets the tracker's scale about `center`, in the source visual's coordinate space.
    pub fn try_update_scale(&self, scale: f32, center: Vector3) -> Result<RequestId> {
        let tracker: bindings::IInteractionTracker = self.0.cast()?;
        Ok(RequestId(tracker.TryUpdateScale(scale, center)?))
    }

    /// Enters scale inertia with the given velocity, in percent per second.
    pub fn try_update_scale_with_additional_velocity(
        &self,
        velocity: f32,
        center: Vector3,
    ) -> Result<RequestId> {
        let tracker: bindings::IInteractionTracker = self.0.cast()?;
        Ok(RequestId(
            tracker.TryUpdateScaleWithAdditionalVelocity(velocity, center)?,
        ))
    }

    /// Whether the inertia currently running came from an impulse rather than a fling.
    pub fn is_inertia_from_impulse(&self) -> bool {
        let tracker: bindings::IInteractionTracker4 = self.0.cast().unwrap();
        tracker.IsInertiaFromImpulse().unwrap_or(false)
    }

    /// Replaces the modifiers applied to X inertia — snap points, or an explicit motion
    /// equation.
    ///
    /// Each modifier's **condition is evaluated once**, at the moment inertia begins, so a
    /// snap point cannot depend on state that changes mid-fling.
    pub fn set_position_x_inertia_modifiers(&self, modifiers: &[InertiaModifier]) -> Result<()> {
        let tracker: bindings::IInteractionTracker = self.0.cast()?;
        tracker.ConfigurePositionXInertiaModifiers(&Self::modifier_list(modifiers))
    }

    /// Replaces the modifiers applied to Y inertia. See
    /// [`set_position_x_inertia_modifiers`](Self::set_position_x_inertia_modifiers).
    pub fn set_position_y_inertia_modifiers(&self, modifiers: &[InertiaModifier]) -> Result<()> {
        let tracker: bindings::IInteractionTracker = self.0.cast()?;
        tracker.ConfigurePositionYInertiaModifiers(&Self::modifier_list(modifiers))
    }

    fn modifier_list(
        modifiers: &[InertiaModifier],
    ) -> windows_collections::IIterable<bindings::InteractionTrackerInertiaModifier> {
        modifiers
            .iter()
            // A WinRT class's default value is nullable, so the collection this builds
            // from is a `Vec<Option<_>>`; none of these entries is ever `None`.
            .map(|modifier| Some(modifier.0.clone()))
            .collect::<Vec<_>>()
            .into()
    }

    /// Binds two trackers so the named axes stay locked compositor-side, rather than by
    /// one surface copying the other's value per tick.
    pub fn bind(first: &Self, second: &Self, axes: BindingAxes) -> Result<()> {
        bindings::InteractionTracker::SetBindingMode(&first.0, &second.0, axes.into())
    }

    /// Returns which axes two trackers currently share.
    pub fn binding(first: &Self, second: &Self) -> Result<BindingAxes> {
        let modes = bindings::InteractionTracker::GetBindingMode(&first.0, &second.0)?;
        let has = |flag: bindings::InteractionBindingAxisModes| modes.contains(flag);
        Ok(BindingAxes {
            x: has(bindings::InteractionBindingAxisModes::PositionX),
            y: has(bindings::InteractionBindingAxisModes::PositionY),
            scale: has(bindings::InteractionBindingAxisModes::Scale),
        })
    }
}

/// The visual a manipulation is collected on: both the hit-test target and the gesture's
/// coordinate space.
///
/// The source visual must not move during the manipulation, so it is the scroll
/// container's viewport rather than the content scrolling inside it. Any visual already in
/// the tree can serve; none has to exist for input alone.
#[derive(Clone)]
pub struct VisualInteractionSource(pub(crate) bindings::VisualInteractionSource);

impl VisualInteractionSource {
    /// Creates a source on `visual`.
    ///
    /// `visual` must have a non-zero size, or the source never hit-tests. Debug builds
    /// assert it.
    pub fn for_visual(visual: &Visual) -> Result<Self> {
        let size = visual.size();
        debug_assert!(
            size.x > 0.0 && size.y > 0.0,
            "an interaction source needs a visual with a non-zero size"
        );
        Ok(Self(bindings::VisualInteractionSource::Create(&visual.0)?))
    }

    /// Sets which axes a manipulation drives.
    pub fn set_axis_modes(&self, x: SourceMode, y: SourceMode, scale: SourceMode) {
        let source: bindings::IVisualInteractionSource = self.0.cast().unwrap();
        source.SetPositionXSourceMode(x.into()).unwrap();
        source.SetPositionYSourceMode(y.into()).unwrap();
        source.SetScaleSourceMode(scale.into()).unwrap();
    }

    /// Enables rails, so a pan started primarily on one axis locks to it.
    ///
    /// Rails matter only where both axes are live, keeping a vertical list from drifting
    /// sideways; with one axis enabled they do nothing.
    pub fn set_rails(&self, x: bool, y: bool) {
        let source: bindings::IVisualInteractionSource = self.0.cast().unwrap();
        source.SetIsPositionXRailsEnabled(x).unwrap();
        source.SetIsPositionYRailsEnabled(y).unwrap();
    }

    /// Sets which input the system delivers to this source without the window's help.
    pub fn set_redirection_mode(&self, mode: RedirectionMode) {
        let source: bindings::IVisualInteractionSource = self.0.cast().unwrap();
        source.SetManipulationRedirectionMode(mode.into()).unwrap();
    }

    /// Sets how each axis hands off at its bounds.
    pub fn set_chaining(&self, x: ChainingMode, y: ChainingMode, scale: ChainingMode) {
        let source: bindings::IVisualInteractionSource = self.0.cast().unwrap();
        source.SetPositionXChainingMode(x.into()).unwrap();
        source.SetPositionYChainingMode(y.into()).unwrap();
        source.SetScaleChainingMode(scale.into()).unwrap();
    }

    /// Sets which axes **wheel** input drives, independently of touch and pen.
    ///
    /// With an axis enabled here, a wheel message over this source needs no front-thread
    /// handling whatsoever: the system routes it to the tracker. The window's own wheel
    /// handling is then only for wheel over something that is not a manipulable surface.
    pub fn set_wheel_modes(&self, x: WheelMode, y: WheelMode, scale: WheelMode) -> Result<()> {
        let source: bindings::IVisualInteractionSource3 = self.0.cast()?;
        let config = source.PointerWheelConfig()?;
        config.SetPositionXSourceMode(x.into())?;
        config.SetPositionYSourceMode(y.into())?;
        config.SetScaleSourceMode(scale.into())?;
        Ok(())
    }

    /// Returns the visual this source collects manipulations on.
    pub fn source_visual(&self) -> Result<Visual> {
        let source: bindings::IVisualInteractionSource = self.0.cast()?;
        Ok(Visual(source.Source()?))
    }

    /// Hands an in-flight contact over to this source, so the compositor drives the
    /// manipulation from here on.
    ///
    /// **Touch and pen only**: a mouse contact is rejected, so mouse drags go through
    /// [`try_update_position_by`](InteractionTracker::try_update_position_by) instead.
    ///
    /// **The window losing the pointer signals success, not the returned `Ok`.** An
    /// injected contact returns success while its updates keep arriving at the window,
    /// meaning the contact was never handed over. Treat the pointer as redirected only
    /// once the window stops being told about it.
    ///
    /// `pointer_id` is the id a `WM_POINTER*` message carries; the contact's state is read
    /// here. An `Err` is a pointer id the system no longer knows — the ordinary race
    /// between a message being handled and the contact ending, not a failure to redirect.
    pub fn try_redirect_for_manipulation(&self, pointer_id: u32) -> Result<()> {
        let mut info = bindings::POINTER_INFO::default();
        // SAFETY: `info` is a stack local of the layout the system writes, live for the
        // whole call. `pointer_id` is validated by the system, so a stale or invented id
        // fails the call rather than reading anything.
        unsafe { bindings::GetPointerInfo(pointer_id, &mut info).ok()? };

        let interop: bindings::IVisualInteractionSourceInterop = self.0.cast()?;
        // SAFETY: `info` was just filled by the system and outlives the call, which reads it
        // and retains nothing.
        unsafe { interop.TryRedirectForManipulation(&info).ok() }
    }
}

/// A rule applied to a tracker's inertia: a condition, plus either a resting value or an
/// explicit motion equation.
///
/// Every constructor sets both halves, since a modifier missing one is applied silently
/// rather than rejected.
#[derive(Clone)]
pub struct InertiaModifier(pub(crate) bindings::InteractionTrackerInertiaModifier);

impl Compositor {
    /// Creates a tracker with **no owner**, and therefore no callbacks.
    ///
    /// An owner costs a cross-process callback per event whether or not the events are
    /// read, so a surface that is neither virtualized nor driven by
    /// [`try_update_position_by`](InteractionTracker::try_update_position_by) takes this
    /// form: nothing has to observe motion the compositor already carries.
    pub fn create_interaction_tracker(&self) -> Result<InteractionTracker> {
        Ok(InteractionTracker(bindings::InteractionTracker::Create(
            &self.0,
        )?))
    }

    /// Creates a tracker that reports to `handler`.
    ///
    /// The owner is taken at construction and there is no per-callback subscription, so a
    /// tracker that needs `RequestIgnored` or `InertiaStateEntered` receives
    /// `ValuesChanged` as well. Use
    /// [`create_interaction_tracker`](Self::create_interaction_tracker) where nothing
    /// reconciles against the events.
    pub fn create_interaction_tracker_with_owner(
        &self,
        handler: impl FnMut(TrackerEvent) + 'static,
    ) -> Result<InteractionTracker> {
        let owner = Owner(core::cell::RefCell::new(Box::new(handler)));
        let owner: bindings::IInteractionTrackerOwner =
            windows_core::ComObject::new(owner).into_interface();
        Ok(InteractionTracker(
            bindings::InteractionTracker::CreateWithOwner(&self.0, &owner)?,
        ))
    }

    /// Creates a snap point: where inertia rests when `condition` holds at inertia entry.
    ///
    /// Both halves are expressions the compositor evaluates. The condition is evaluated
    /// **once**, when inertia begins.
    pub fn create_inertia_resting_value(
        &self,
        condition: &ExpressionAnimation,
        resting_value: &ExpressionAnimation,
    ) -> Result<InertiaModifier> {
        let modifier = bindings::InteractionTrackerInertiaRestingValue::Create(&self.0)?;
        modifier.SetCondition(&condition.0)?;
        modifier.SetRestingValue(&resting_value.0)?;
        Ok(InertiaModifier(modifier.cast()?))
    }

    /// Creates an explicit inertia motion: a second-derivative equation the compositor
    /// evaluates **every frame** while `condition` held at inertia entry.
    pub fn create_inertia_motion(
        &self,
        condition: &ExpressionAnimation,
        motion: &ExpressionAnimation,
    ) -> Result<InertiaModifier> {
        let modifier = bindings::InteractionTrackerInertiaMotion::Create(&self.0)?;
        modifier.SetCondition(&condition.0)?;
        modifier.SetMotion(&motion.0)?;
        Ok(InertiaModifier(modifier.cast()?))
    }
}
