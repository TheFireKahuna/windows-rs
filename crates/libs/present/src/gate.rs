//! What a renderer's cached device resources are built from, and the two primitives that
//! keep them current: a version gate and a keyed offscreen layer.
//!
//! A [`Frame`](crate::Frame)'s resources go stale on more than one axis — the box it was
//! laid out for, the scale it was rasterized at, the transform its colours were resolved
//! through, the device that holds them, and whatever the consumer versions itself. A
//! [`Gate`] carries the set one resource depends on, so the test and the commit
//! [`Frame::should_draw`](crate::Frame::should_draw) requires in one call are that call.

use super::*;

/// What a cached device resource is built from.
///
/// Declared once per resource. Two resources of one renderer may name different sets: a
/// glyph atlas rasterized in DIPs depends on the scale and not on the box, while a ramp
/// aimed across the box depends on both.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct Sources(u8);

impl Sources {
    /// Nothing the frame context carries. Such a resource moves only on a version its
    /// owner supplies, or on the device.
    pub const NONE: Self = Self(0);
    /// The region's DIP box, which is also what its buffers are allocated for.
    pub const EXTENT: Self = Self(1 << 0);
    /// The DIP-to-pixel factor alone, which is what a rasterization is keyed on.
    pub const SCALE: Self = Self(1 << 1);
    /// The transform every colour drawn this frame passes through.
    pub const OUTPUT: Self = Self(1 << 2);
    /// The device the resource was built on. A gate naming it answers `true` after
    /// [`Gate::device_reset`]; one that does not is left alone, because a gate over
    /// content holds nothing a device rebuild invalidates.
    pub const DEVICE: Self = Self(1 << 3);

    /// Returns the union of two sets.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns whether every source in `other` is named here.
    #[must_use]
    pub const fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl core::ops::BitOr for Sources {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        self.with(other)
    }
}

/// What a gate last saw.
#[derive(Copy, Clone)]
struct Seen<const N: usize> {
    extent: Extent,
    out: OutputTransform,
    versions: [u64; N],
}

/// Tests a resource's declared sources against what they were when it was built, and
/// commits them in the same call.
///
/// `N` is how many version stamps the owner supplies — a theme counter, a published
/// sequence, a font size it derives itself. Zero of them is a gate over the frame context
/// alone.
///
/// Real-time: [`changed`](Self::changed) compares stack values and allocates nothing, so it
/// belongs on the per-frame path that [`Frame::should_draw`](crate::Frame::should_draw)
/// runs on.
pub struct Gate<const N: usize = 1> {
    sources: Sources,
    seen: Option<Seen<N>>,
}

impl<const N: usize> Gate<N> {
    /// Creates a gate over `sources`, in the state that answers `true` on its first test.
    #[must_use]
    pub const fn new(sources: Sources) -> Self {
        Self {
            sources,
            seen: None,
        }
    }

    /// Reports whether any declared source or supplied version moved, and commits them all.
    ///
    /// Answers `true` on the first call and after [`device_reset`](Self::device_reset), so
    /// the resource is built before anything reads it. Versions are always compared: a
    /// stamp the owner did not want tested is one it does not pass.
    pub fn changed(&mut self, ctx: GateCtx<'_>, versions: [u64; N]) -> bool {
        let now = Seen {
            extent: ctx.extent,
            out: ctx.out,
            versions,
        };
        let moved = match self.seen {
            None => true,
            Some(was) => {
                (self.sources.has(Sources::EXTENT) && was.extent != now.extent)
                    || (self.sources.has(Sources::SCALE)
                        && was.extent.scale() != now.extent.scale())
                    || (self.sources.has(Sources::OUTPUT) && was.out != now.out)
                    || was.versions != now.versions
            }
        };
        self.seen = Some(now);
        moved
    }

    /// Forgets what was seen, so the next [`changed`](Self::changed) answers `true`.
    ///
    /// A build that failed calls this: the test that preceded it committed the sources, and
    /// without this the failure would be committed with them.
    pub fn invalidate(&mut self) {
        self.seen = None;
    }

    /// Forgets what was seen when the gate names [`Sources::DEVICE`], so the next
    /// [`changed`](Self::changed) rebuilds.
    pub fn device_reset(&mut self) {
        if self.sources.has(Sources::DEVICE) {
            self.invalidate();
        }
    }
}

/// What one [`Layer::ensure`] did.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Cached {
    /// The layer already held this key's contents and nothing was drawn.
    Held,
    /// The layer was redrawn, so the frame it is blitted into differs from the last one.
    Drawn,
    /// The layer holds nothing. The key is uncommitted, so the next gate retries.
    Failed,
}

/// Holds an offscreen allocation and the key and logical extent its contents were drawn for.
///
/// Rasterizes through the supplied pass and keeps its key provisional until
/// [`Layer::finish_batch`] confirms a successful flush.
pub struct Layer<K: Copy + PartialEq> {
    target: Option<Target>,
    key: Option<K>,
    pending: Option<K>,
    opacity: Opacity,
    capacity: bool,
    extent: Option<Extent>,
}

impl<K: Copy + PartialEq> Layer<K> {
    /// Creates an empty layer whose offscreen is allocated at `opacity`.
    #[must_use]
    pub const fn new(opacity: Opacity) -> Self {
        Self {
            target: None,
            key: None,
            pending: None,
            opacity,
            capacity: false,
            extent: None,
        }
    }

    /// Creates a layer whose allocation grows in 128-pixel blocks and survives shrinking.
    /// Callers must blit the logical source rectangle explicitly and clear unused pixels.
    #[must_use]
    pub fn with_capacity(opacity: Opacity) -> Self {
        Self { capacity: true, ..Self::new(opacity) }
    }

    /// Redraws the layer when `key` or the region's logical extent moved, and reports whether it
    /// now holds `key`'s contents.
    ///
    /// `paint` is handed an open pass and the layer's target; it draws by retargeting that
    /// pass, so a nested intermediate costs no second context. A `paint` that fails commits
    /// no key, leaving the next gate to retry.
    ///
    /// The owner must discard this cache if the batch fails to flush.
    pub fn ensure(
        &mut self,
        ctx: GateCtx<'_>,
        key: K,
        pass: &mut Pass<'_>,
        paint: impl FnOnce(&mut Pass<'_>, &Target) -> Result<()>,
    ) -> Cached {
        let px = ctx.extent.px();
        let sized = self
            .target
            .as_ref()
            .is_some_and(|t| {
                let size = t.size_px();
                (if self.capacity { size.0 >= px.0 && size.1 >= px.1 } else { size == px })
                    && t.dpi() == ctx.extent.dpi
            });
        if !sized {
            let allocation = if self.capacity {
                let prior = self.target.as_ref().filter(|t| t.dpi() == ctx.extent.dpi)
                    .map_or((0, 0), Target::size_px);
                (px.0.max(prior.0).div_ceil(128) * 128, px.1.max(prior.1).div_ceil(128) * 128)
            } else { px };
            let Ok(target) = ctx.device.offscreen(allocation, ctx.extent.dpi, self.opacity) else {
                self.target = None;
                self.key = None;
                self.pending = None;
                return Cached::Failed;
            };
            self.target = Some(target);
            self.key = None;
            self.pending = None;
        }
        if self.pending.or(self.key) == Some(key) && self.extent == Some(ctx.extent) {
            return Cached::Held;
        }
        let Some(target) = self.target.as_ref() else {
            return Cached::Failed;
        };
        // Cleared before the paint and committed after it: a half-drawn layer holds no key,
        // so nothing blits it and the next gate retries.
        self.key = None;
        self.pending = None;
        self.extent = Some(ctx.extent);
        if paint(pass, target).is_err() {
            return Cached::Failed;
        }
        self.pending = Some(key);
        Cached::Drawn
    }

    /// Commits the prepared key after a successful bracket or discards unfinished pixels.
    pub fn finish_batch(&mut self, success: bool) {
        if success {
            if let Some(key) = self.pending.take() { self.key = Some(key); }
        } else { self.device_reset(); }
    }

    /// Returns prepared pixels for drawing within the batch, or previously committed pixels.
    /// Submission must wait for successful bracket completion.
    #[must_use]
    pub fn target(&self) -> Option<&Target> {
        self.pending.or(self.key).is_some().then_some(self.target.as_ref()).flatten()
    }

    /// Drops the offscreen, which belongs to a device that no longer exists.
    pub fn device_reset(&mut self) {
        self.target = None;
        self.key = None;
        self.pending = None;
        self.extent = None;
    }
}

impl<K: Copy + PartialEq> Default for Layer<K> {
    fn default() -> Self {
        Self::new(Opacity::Translucent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(gpu: &'a Gpu, input: &'a RegionInput, w: f32, dpi: f32) -> GateCtx<'a> {
        GateCtx {
            extent: Extent::new(w, 40.0, dpi),
            tick: 0,
            at: std::time::Instant::now(),
            interval: std::time::Duration::from_millis(16),
            device: gpu,
            out: OutputTransform::for_display(windows_color::DisplayCapability::Sdr, 1000.0),
            input,
        }
    }

    #[test]
    fn capacity_layer_repaints_logical_resizes_and_reuses_pixel_capacity() {
        let gpu = Gpu::for_presentation().unwrap();
        let input = RegionInput::default();
        let mut layer = Layer::<u32>::with_capacity(Opacity::Translucent);
        for (width, dpi, expected, result) in [
            (100.0, 96.0, (128, 128), Cached::Drawn),
            (80.0, 96.0, (128, 128), Cached::Drawn),
            (80.0, 96.0, (128, 128), Cached::Held),
            (120.0, 96.0, (128, 128), Cached::Drawn),
            (180.0, 96.0, (256, 128), Cached::Drawn),
            (100.0, 96.0, (256, 128), Cached::Drawn),
            (100.0, 144.0, (256, 128), Cached::Drawn),
        ] {
            let ctx = ctx(&gpu, &input, width, dpi);
            let mut pass = gpu.pass().unwrap();
            assert_eq!(layer.ensure(ctx, 1, &mut pass, |pass, target| {
                let draw = pass.draw(target);
                draw.clear(windows_color::Scrgb::TRANSPARENT);
                let ink = gpu.solid(windows_color::Scrgb { r: 1.0, g: 0.0, b: 0.0, a: 1.0 })?;
                draw.fill(Rect::sized(0.0, 0.0, width, 40.0), &ink);
                Ok(())
            }), result);
            pass.end().unwrap();
            layer.finish_batch(true);
            let target = layer.target().unwrap();
            assert_eq!(target.size_px(), expected);
            assert_eq!(target.dpi(), dpi);
            let pixels = gpu.read(target).unwrap();
            assert!(pixels.pixel(ctx.extent.px().0 - 2, 2)[3] > 0.99);
            assert_eq!(pixels.pixel(ctx.extent.px().0 + 2, 2)[3], 0.0);
        }
        layer.finish_batch(false);
        assert!(layer.target().is_none());
    }

    #[test]
    fn each_declared_source_moves_its_gate_and_no_other() {
        let gpu = Gpu::for_presentation().expect("native Direct2D device");
        let input = RegionInput::default();
        let base = ctx(&gpu, &input, 100.0, 96.0);

        // One gate per axis, so a change is attributed rather than merely observed.
        let mut extent = Gate::<0>::new(Sources::EXTENT);
        let mut scale = Gate::<0>::new(Sources::SCALE);
        let mut output = Gate::<0>::new(Sources::OUTPUT);
        let mut none = Gate::<0>::new(Sources::NONE);
        let mut version = Gate::<1>::new(Sources::NONE);

        // Every gate builds once.
        assert!(extent.changed(base, []));
        assert!(scale.changed(base, []));
        assert!(output.changed(base, []));
        assert!(none.changed(base, []));
        assert!(version.changed(base, [7]));
        // And nothing moved, so nothing rebuilds.
        assert!(!extent.changed(base, []));
        assert!(!scale.changed(base, []));
        assert!(!output.changed(base, []));
        assert!(!none.changed(base, []));
        assert!(!version.changed(base, [7]));

        // A width change is the box alone: the scale is unmoved.
        let wider = ctx(&gpu, &input, 120.0, 96.0);
        assert!(extent.changed(wider, []));
        assert!(!scale.changed(wider, []));
        assert!(!output.changed(wider, []));
        assert!(!none.changed(wider, []));

        // A DPI change is both.
        let dense = ctx(&gpu, &input, 120.0, 144.0);
        assert!(extent.changed(dense, []));
        assert!(scale.changed(dense, []));
        assert!(!output.changed(dense, []));

        // The transform alone.
        let mut hdr = dense;
        hdr.out = OutputTransform::for_display(
            windows_color::DisplayCapability::HighDynamicRange {
                gamut: windows_color::Gamut::REC709,
                white_nits: 200.0,
                peak_nits: 1000.0,
            },
            1000.0,
        );
        assert!(output.changed(hdr, []));
        assert!(!extent.changed(hdr, []));
        assert!(!scale.changed(hdr, []));

        // A version is compared whatever the sources say.
        assert!(version.changed(hdr, [8]));
        assert!(!version.changed(hdr, [8]));

        // Only a gate naming the device forgets on a device rebuild.
        let mut device = Gate::<0>::new(Sources::DEVICE);
        assert!(device.changed(hdr, []));
        assert!(!device.changed(hdr, []));
        device.device_reset();
        none.device_reset();
        assert!(device.changed(hdr, []));
        assert!(!none.changed(hdr, []));
    }

    #[test]
    fn a_layer_repaints_on_its_key_and_on_its_allocation_and_never_otherwise() {
        let gpu = Gpu::for_presentation().expect("native Direct2D device");
        let input = RegionInput::default();
        let mut layer = Layer::<u32>::new(Opacity::Translucent);
        let mut painted = 0u32;
        let paint = |layer: &mut Layer<u32>, ctx, key, painted: &mut u32| {
            let mut pass = gpu.pass().unwrap();
            let result = layer.ensure(ctx, key, &mut pass, |pass, target| {
                *painted += 1;
                pass.draw(target).clear(windows_color::Scrgb::TRANSPARENT);
                Ok(())
            });
            pass.end().unwrap();
            layer.finish_batch(true);
            result
        };

        let base = ctx(&gpu, &input, 100.0, 96.0);
        assert_eq!(paint(&mut layer, base, 1, &mut painted), Cached::Drawn);
        assert_eq!(painted, 1);
        assert!(layer.target().is_some());
        // The key stands, so the second pass is the key compare and nothing else.
        assert_eq!(paint(&mut layer, base, 1, &mut painted), Cached::Held);
        assert_eq!(painted, 1);
        assert_eq!(paint(&mut layer, base, 2, &mut painted), Cached::Drawn);
        assert_eq!(painted, 2);

        // A resize reallocates and therefore repaints, at the same key.
        let wider = ctx(&gpu, &input, 160.0, 96.0);
        assert_eq!(paint(&mut layer, wider, 2, &mut painted), Cached::Drawn);
        assert_eq!(painted, 3);
        assert_eq!(layer.target().map(Target::size_px), Some((160, 40)));
        assert_eq!(paint(&mut layer, wider, 2, &mut painted), Cached::Held);
        assert_eq!(painted, 3);

        // A device rebuild drops the offscreen, so nothing blits a dead target.
        layer.device_reset();
        assert!(layer.target().is_none());
        assert_eq!(paint(&mut layer, wider, 2, &mut painted), Cached::Drawn);
        assert_eq!(painted, 4);

        // A failed paint commits no key, so the next gate retries.
        let mut pass = gpu.pass().unwrap();
        let failed = layer.ensure(wider, 9, &mut pass, |_, _| {
            painted += 1;
            Err(windows_core::Error::from_hresult(windows_core::HRESULT(-1)))
        });
        pass.end().unwrap();
        assert_eq!(failed, Cached::Failed);
        assert!(layer.target().is_none());
        assert_eq!(paint(&mut layer, wider, 9, &mut painted), Cached::Drawn);
        assert_eq!(painted, 6);
    }
}
