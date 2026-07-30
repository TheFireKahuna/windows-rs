//! Interaction trackers: motion the compositor owns (feature `system`).
//!
//! A tracker is a value the composition engine drives from a manipulation and from
//! inertia, in **another process**. Every call into it and every callback out of it is
//! asynchronous, and that single fact shapes this whole module:
//!
//! - A request may be **dropped**, silently and by design, depending on the state the
//!   tracker is in when it arrives. So every `try_update_*` returns the
//!   [`RequestId`] the tracker assigned, and a caller that cares reconciles it against
//!   [`TrackerEvent::RequestIgnored`].
//! - The tracker's position **cannot be read directly** — this crate deliberately does
//!   not carry that getter. The value delivered by [`TrackerEvent::ValuesChanged`] is
//!   the only trustworthy one.
//! - An **owner is not free**: it is supplied at construction, there is no per-callback
//!   subscription, and the crossing itself dominates its cost. A tracker whose motion
//!   nothing needs to observe should be created with
//!   [`Compositor::create_interaction_tracker`] and no owner at all.

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
    /// Hand off to an enclosing tracker if there is one. Nested scrollers then behave
    /// correctly with no hand-written plumbing.
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
    /// The tracker's value moved. **This is the only trustworthy read of it.**
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
    /// This arrives at the *instant* inertia begins, when where the motion will land is
    /// already known — which is what makes destination prefetch possible while DWM
    /// animates.
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
        /// which is worth a shorter decay.
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
    /// A request was **dropped**. Not an error: a position update arriving while the
    /// user is actively manipulating is documented to be ignored.
    ///
    /// Drop the pending request and reconcile against the next `ValuesChanged`. Never
    /// re-apply it blindly, or a user whose manipulation ends gets a double jump.
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
        // A re-entrant raise would otherwise unwind across the COM boundary; yielding is
        // the conservative answer and drops an event rather than the process.
        if let Ok(mut handler) = self.0.try_borrow_mut() {
            handler(event);
        }
    }
}

/// Reads `IsFromBinding` off whichever revision of an args type carries it.
///
/// Each args type grew the flag in its **own** later interface, so the interface has to be
/// named per call site — a single shared cast would answer only for the one type it named
/// and silently report `false` for the other three. Absence reads as "not from a binding",
/// which is right: a system that cannot report it is a system on which nothing was bound.
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
/// Bind a sink to it with an [`ExpressionAnimation`] referencing it as an
/// [`Animatable`], and the compositor evaluates that binding every vblank with the app
/// thread asleep — and keeps evaluating it when the app thread is busy, which is the
/// property a front-side stepped animation cannot have.
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
    /// The position may travel *outside* it while interacting or in inertia — that
    /// overpan is the bounce, and it is wanted, so a consumer must not clamp its own
    /// reads.
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
    /// A wheel notch deserves a shorter tail than a fling, which is what
    /// [`TrackerEvent::InertiaStateEntered`]'s `from_impulse` distinguishes.
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
    /// Ignored outright while the user is interacting. `scale_animation` says whether a
    /// running custom scale animation survives — stated rather than defaulted, because
    /// the default silently stops it.
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
    /// This is the mouse-drag path: mouse contacts cannot be redirected into a
    /// manipulation at all, so the front thread writes a request per consumed sample
    /// while the button is down — and does nothing after release, which is where most of
    /// a fling's frames are.
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
    /// This is how a position is *moved* rather than jumped: scrolling a focused row into
    /// view, returning to the top, settling on a chosen tab. Without it a programmatic
    /// scroll is a discontinuity, and [`TrackerEvent::CustomAnimationStateEntered`] is a
    /// state the owner reports that nothing can reach.
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
/// **It must not move during the manipulation**, which is why it is the scroll
/// container's viewport and never the content that scrolls inside it. No visual needs to
/// exist purely for input.
#[derive(Clone)]
pub struct VisualInteractionSource(pub(crate) bindings::VisualInteractionSource);

impl VisualInteractionSource {
    /// Creates a source on `visual`.
    ///
    /// `visual` must have a non-zero size or it will not hit-test correctly — a
    /// zero-size viewport is a bug rather than a surface that merely never responds, so
    /// it is asserted in debug builds.
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
    /// Wanted whenever both axes are live — a vertical list should not drift sideways —
    /// and meaningless when only one is.
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

    /// The visual this source collects manipulations on.
    pub fn source_visual(&self) -> Result<Visual> {
        let source: bindings::IVisualInteractionSource = self.0.cast()?;
        Ok(Visual(source.Source()?))
    }

    /// Hands an in-flight contact over to this source, so the compositor drives the
    /// manipulation from here on.
    ///
    /// **Touch and pen only** — a mouse contact is rejected outright, which is why mouse
    /// drags are driven by
    /// [`try_update_position_by`](InteractionTracker::try_update_position_by) instead. And
    /// **success is signalled by the window losing the pointer**, not by the returned
    /// `Ok`: an injected contact returns success while its updates keep arriving at the
    /// window, which means the contact was never handed over. Treat the pointer as
    /// redirected only once the window stops being told about it.
    ///
    /// Takes the pointer id a `WM_POINTER*` message carries, and reads the contact's state
    /// here. An `Err` is a pointer id the system no longer knows — the ordinary race
    /// between a message being handled and the contact ending, not a failure to redirect.
    pub fn try_redirect_for_manipulation(&self, pointer_id: u32) -> Result<()> {
        let mut info = bindings::POINTER_INFO::default();
        // SAFETY: `info` is a live, correctly-sized out-parameter for the duration of the
        // call, and `pointer_id` is validated by the system rather than by us — an id that
        // is stale or was never real fails the call instead of reading anything.
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
/// Built complete — there is no way to have one with a condition and no value — because a
/// half-configured modifier is applied silently rather than rejected.
#[derive(Clone)]
pub struct InertiaModifier(pub(crate) bindings::InteractionTrackerInertiaModifier);

impl Compositor {
    /// Creates a tracker with **no owner**, and therefore no callbacks.
    ///
    /// This is the cheap form and the default: the owner is what costs, and it costs
    /// whether or not the callbacks are read. Use it for any surface that is neither
    /// virtualized nor driven by
    /// [`try_update_position_by`](InteractionTracker::try_update_position_by) — nothing
    /// needs to observe motion the compositor is already carrying.
    pub fn create_interaction_tracker(&self) -> Result<InteractionTracker> {
        Ok(InteractionTracker(bindings::InteractionTracker::Create(
            &self.0,
        )?))
    }

    /// Creates a tracker that reports to `handler`.
    ///
    /// The owner is supplied here because the API takes it at construction and there is
    /// **no per-callback subscription**: a tracker that needs `RequestIgnored` or
    /// `InertiaStateEntered` pays for `ValuesChanged` too. Prefer
    /// [`create_interaction_tracker`](Self::create_interaction_tracker) unless something
    /// genuinely reconciles against the events.
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
    /// Both halves are expressions, so a snap point is authored the way every other
    /// compositor-evaluated value is. The condition is evaluated **once**, when inertia
    /// begins.
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
    ///
    /// This is the escape hatch when natural motion settles wrong — the same curve, stated
    /// rather than tuned.
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
