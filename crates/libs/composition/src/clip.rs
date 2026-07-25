use super::*;

// There is no `CompositionClip` base newtype here on purpose. `InsetClip` is the
// only clip the bindings expose, so a base type would have one implementor and
// no call site — `Visual::set_clip` takes an `&InsetClip` and casts to the base
// interface internally. Add one when a second clip kind earns it.

/// A clip that hides a fixed inset from each edge of the visual it is applied
/// to, leaving the rest visible.
///
/// Create one with [`Compositor::create_inset_clip`] and attach it with
/// [`Visual::set_clip`](crate::Visual::set_clip). The insets are in the clipped
/// visual's own coordinate space, in DIPs, measured inward from each edge, so an
/// inset of `0.0` on every edge shows the visual whole.
///
/// The insets are animatable properties in their own right (`"LeftInset"`,
/// `"TopInset"`, `"RightInset"`, `"BottomInset"`), which is how a reveal is
/// expressed: hold the visual still and animate the clip's inset across it,
/// rather than animating the visual's size. Because a clip is not a visual,
/// those animations start through this type's own
/// [`start_animation`](Self::start_animation) rather than the visual's.
#[derive(Clone)]
pub struct InsetClip(pub(crate) bindings::InsetClip);

impl InsetClip {
    /// Sets all four insets, in DIPs, measured inward from the corresponding
    /// edge of the clipped visual.
    pub fn set_insets(&self, left: f32, top: f32, right: f32, bottom: f32) {
        self.0.SetLeftInset(left).unwrap();
        self.0.SetTopInset(top).unwrap();
        self.0.SetRightInset(right).unwrap();
        self.0.SetBottomInset(bottom).unwrap();
    }

    /// Sets the inset from the left edge, in DIPs.
    pub fn set_left_inset(&self, inset: f32) {
        self.0.SetLeftInset(inset).unwrap();
    }

    /// Sets the inset from the top edge, in DIPs.
    pub fn set_top_inset(&self, inset: f32) {
        self.0.SetTopInset(inset).unwrap();
    }

    /// Sets the inset from the right edge, in DIPs.
    pub fn set_right_inset(&self, inset: f32) {
        self.0.SetRightInset(inset).unwrap();
    }

    /// Sets the inset from the bottom edge, in DIPs.
    pub fn set_bottom_inset(&self, inset: f32) {
        self.0.SetBottomInset(inset).unwrap();
    }

    /// Starts an animation on the named property (for example `"RightInset"`).
    pub fn start_animation(&self, property: &str, animation: &impl Animation) {
        let object: bindings::ICompositionObject = self.0.cast().unwrap();
        object
            .StartAnimation(property, &animation.as_animation().0)
            .unwrap();
    }

    /// Stops any animation on the named property, leaving the property at the
    /// value it had reached.
    ///
    /// As on [`Visual::stop_animation`](crate::Visual::stop_animation), a failure
    /// here is discarded rather than panicked on: stopping a property that
    /// nothing is animating is the ordinary case, not an exceptional one, so a
    /// caller taking an inset back under manual control can stop it
    /// unconditionally and then set it.
    pub fn stop_animation(&self, property: &str) {
        let object: bindings::ICompositionObject = self.0.cast().unwrap();
        let _ = object.StopAnimation(property);
    }
}

impl Sealed for InsetClip {}

impl Object for InsetClip {
    fn as_object(&self) -> CompositionObject {
        CompositionObject(self.0.cast().unwrap())
    }
}

impl Compositor {
    /// Creates an inset clip, initially clipping nothing (every inset `0.0`).
    pub fn create_inset_clip(&self) -> InsetClip {
        bump_count(Count::Clip);
        InsetClip(self.0.CreateInsetClip().unwrap())
    }
}
