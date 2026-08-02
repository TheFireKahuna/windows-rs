//! A shaped run: what a string became, and everything downstream reads.
//!
//! ## It keeps its layout, and so it is not `Send`
//!
//! `IDWriteTextLayout` is retained because three things need it: re-flowing at a new width
//! is a property set instead of a rebuild, measuring is one call, and caret and selection
//! geometry across bidi, combining marks and surrogate pairs is DirectWrite's arithmetic
//! rather than ours. Nothing requires a run to cross a thread — what crosses is
//! [`GlyphSeg`] over [`SegBuffers`], plain `Copy` data with nowhere for a face to sit.
//!
//! ## Measure, pin, harvest, read
//!
//! A layout solve probes a node several times with different constraints before deciding
//! one, so glyph positions are not knowable until the last of those. The harvest is
//! therefore lazy: [`measure`](ShapedRun::measure) and [`pin`](ShapedRun::pin) move the
//! layout and mark it stale, [`TextEngine::harvest`] brings it back, and every reader after
//! that is plain data.
//!
//! **This type does not compare text.** Whoever owns the string owns the identity and
//! decides when to reshape; retaining a copy to compare against would hold the one thing
//! the layer above drops.

use super::*;
use core::cell::Cell;
use core::ops::Range;

/// The construction box a run is measured in before anything constrains it — its
/// max-content state. Large rather than infinite: DirectWrite treats the box as a number.
const UNBOUNDED: f32 = 100_000.0;

/// One laid-out line.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct LineMetrics {
    /// Code units on this line, trailing whitespace and newline included.
    pub length: u32,
    pub trailing_whitespace: u32,
    pub newline: u32,
    /// Line box height, in DIPs.
    pub height: f32,
    /// Baseline offset from the line box's top, in DIPs.
    pub baseline: f32,
}

/// The box one line's coverage occupies.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Ink {
    /// Tile extent in DIPs. Pixels belong to whoever rasterizes it, which is the half that
    /// holds the scale and re-rasterizes when it moves.
    pub size: Vector2,
    /// Baseline origin measured from the tile's top-left, in DIPs.
    pub baseline: Vector2,
}

pub struct ShapedRun {
    layout: IDWriteTextLayout,
    spec: FontSpec,
    flow: Flow,
    /// Code units, for the ranges DirectWrite's hit-testing speaks in.
    len: u32,
    /// The width the layout box currently stands at.
    width: f32,
    dirty: bool,
    /// How far the ink reaches past the layout box. Measured from the box's edges, so it
    /// moves whenever the box does — including for a run whose glyphs did not.
    pad: Cell<Option<DWRITE_OVERHANG_METRICS>>,
    pub(crate) harvest: Harvest,
    lines: Vec<LineMetrics>,
    /// DirectWrite's own line metrics, kept only so reading them allocates once per run
    /// rather than once per re-flow.
    raw_lines: Vec<DWRITE_LINE_METRICS>,
}

impl TextEngine {
    /// Lays `text` out under `spec`, unconstrained.
    ///
    /// The result is stale until harvested: it has been laid out, but the width it will be
    /// given is not known yet.
    pub fn shape(&self, text: &str, spec: &FontSpec, flow: Flow) -> Result<ShapedRun> {
        let (layout, len) = self.lay_out(text, spec, flow)?;
        Ok(ShapedRun {
            layout,
            spec: *spec,
            flow,
            len,
            width: UNBOUNDED,
            dirty: true,
            pad: Cell::new(None),
            harvest: Harvest::default(),
            lines: Vec::new(),
            raw_lines: Vec::new(),
        })
    }

    /// Re-lays `run` for new text or a new spec, keeping its buffers.
    pub fn reshape(
        &self,
        run: &mut ShapedRun,
        text: &str,
        spec: &FontSpec,
        flow: Flow,
    ) -> Result<()> {
        let (layout, len) = self.lay_out(text, spec, flow)?;
        run.layout = layout;
        run.spec = *spec;
        run.flow = flow;
        run.len = len;
        run.width = UNBOUNDED;
        run.dirty = true;
        run.pad.set(None);
        Ok(())
    }

    fn lay_out(&self, text: &str, spec: &FontSpec, flow: Flow) -> Result<(IDWriteTextLayout, u32)> {
        let format = self.format(spec, flow)?;
        let mut scratch = self.scratch.borrow_mut();
        scratch.clear();
        scratch.extend(text.encode_utf16());
        let len = u32::try_from(scratch.len()).unwrap_or(u32::MAX);

        // SAFETY: the layout copies the string, so the scratch is free after the call, and
        // the range below names code units the layout has.
        unsafe {
            let layout =
                self.factory
                    .CreateTextLayout(&scratch[..], &format, UNBOUNDED, UNBOUNDED)?;
            if let Some(typo) = self.typography(spec.features) {
                let all = DWRITE_TEXT_RANGE {
                    startPosition: 0,
                    length: len,
                };
                layout.SetTypography(&typo, all).ok()?;
            }
            Ok((layout, len))
        }
    }

    /// Brings `run`'s glyph data up to date with the width it now stands at.
    ///
    /// A no-op on a run nothing has moved, so calling it before every read is the intended
    /// use rather than a cost to avoid.
    pub fn harvest(&self, run: &mut ShapedRun) -> Result<()> {
        if !run.dirty {
            return Ok(());
        }
        let mut slot = self.collector.borrow_mut();
        let collector = slot.get_or_insert_with(|| Collector::new(self.ladder().clone()));
        Collector::walk(collector, &run.layout, &mut run.harvest);
        run.after_walk()?;
        Ok(())
    }
}

impl ShapedRun {
    /// What it was shaped under.
    #[must_use]
    pub fn spec(&self) -> &FontSpec {
        &self.spec
    }

    #[must_use]
    pub fn flow(&self) -> Flow {
        self.flow
    }

    /// Code units in the shaped text — the unit every range here is measured in.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the glyph data is behind the layout.
    #[must_use]
    pub fn is_stale(&self) -> bool {
        self.dirty
    }

    /// The size this run takes at `max_w`, or unconstrained if `None`.
    ///
    /// **This does not reshape.** It is the call a layout solve makes several times per
    /// pass with different constraints, so it is a property set and a metrics read; the
    /// glyphs are re-placed once, later, by [`TextEngine::harvest`].
    pub fn measure(&mut self, max_w: Option<f32>) -> Vector2 {
        self.set_width(max_w.unwrap_or(UNBOUNDED));
        let mut m = DWRITE_TEXT_METRICS::default();
        // SAFETY: the out-parameter is a stack local outliving the call.
        if unsafe { self.layout.GetMetrics(&mut m) }.is_err() {
            return Vector2::default();
        }
        Vector2 {
            x: m.width,
            y: m.height,
        }
    }

    /// The narrowest width this run can occupy — its longest unbreakable span.
    ///
    /// Independent of the layout box, so it neither moves the run nor stales it.
    #[must_use]
    pub fn min_width(&self) -> f32 {
        // SAFETY: a call on an interface this run owns, taking no arguments.
        unsafe { self.layout.DetermineMinWidth() }.unwrap_or_default()
    }

    /// Fixes the run at the width it was actually given, and reports whether that moved
    /// its glyphs.
    ///
    /// The single authoritative writer of that width. [`measure`](Self::measure) probes
    /// with whatever the solve is asking about, and whichever probe ran last would
    /// otherwise decide where the glyphs land; here the answer is known.
    pub fn pin(&mut self, width: f32) -> bool {
        self.set_width(width);
        self.dirty
    }

    /// Takes the stale flag, so an emit can ask once whether to re-publish.
    pub fn take_stale(&mut self) -> bool {
        core::mem::take(&mut self.dirty)
    }

    /// The laid-out lines. Harvested data — bring it up to date first.
    #[must_use]
    pub fn lines(&self) -> &[LineMetrics] {
        debug_assert!(
            !self.dirty,
            "lines() read a run that has not been harvested"
        );
        &self.lines
    }

    /// Rules DirectWrite resolved that are not glyphs. Empty for almost all text.
    #[must_use]
    pub fn decorations(&self) -> &[Decoration] {
        debug_assert!(
            !self.dirty,
            "decorations() read a run that has not been harvested"
        );
        &self.harvest.decorations
    }

    /// The tile `line` needs, and where its baseline sits in it.
    #[must_use]
    pub fn line_ink(&self, line: usize) -> Ink {
        debug_assert!(
            !self.dirty,
            "line_ink() read a run that has not been harvested"
        );
        let Some((_, metrics)) = self.line_at(line) else {
            return Ink::default();
        };
        let (left, right) = self.line_extent(line);
        let pad = self.pad();
        let (px, py) = (pad.left.max(0.0), pad.top.max(0.0));
        Ink {
            size: Vector2 {
                x: right - left + px + pad.right.max(0.0),
                y: metrics.height + py + pad.bottom.max(0.0),
            },
            baseline: Vector2 {
                x: px,
                y: py + metrics.baseline,
            },
        }
    }

    /// Appends `line`'s segments to `out`, and returns the span naming them.
    ///
    /// Origins are rebased onto the tile [`line_ink`](Self::line_ink) describes, so the
    /// front side draws each segment at `tile_top_left + seg.origin` and never re-derives a
    /// pen position — which is what keeps a bidi line, where visual order and advance order
    /// disagree, from needing a second rule.
    pub fn segments(&self, line: usize, out: &mut SegBuffers) -> Span {
        debug_assert!(
            !self.dirty,
            "segments() read a run that has not been harvested"
        );
        let start = out.segs.len() as u32;
        let Some((top, _)) = self.line_at(line) else {
            return Span::EMPTY;
        };
        let (left, _) = self.line_extent(line);
        let pad = self.pad();
        let origin = Vector2 {
            x: left - pad.left.max(0.0),
            y: top - pad.top.max(0.0),
        };

        for h in self.harvest.segs.iter().filter(|h| h.line as usize == line) {
            let spans = out.push(
                h.seg.glyphs.of(&self.harvest.glyphs),
                h.seg.advances.of(&self.harvest.advances),
                h.seg.offsets.of(&self.harvest.offsets),
            );
            out.segs.push(GlyphSeg {
                origin: Vector2 {
                    x: h.origin.x - origin.x,
                    y: h.origin.y - origin.y,
                },
                glyphs: spans.glyphs,
                advances: spans.advances,
                offsets: spans.offsets,
                ..h.seg
            });
        }
        Span::new(start, out.segs.len() as u32 - start)
    }

    pub(crate) fn layout(&self) -> &IDWriteTextLayout {
        &self.layout
    }

    /// Sets the layout box width, reporting whether it moved.
    ///
    /// The box always invalidates the overhang, and only sometimes the glyphs: a
    /// non-wrapping run is laid out leading and does not break, so its box decides what
    /// hangs outside it and nothing else. Conflating the two would re-harvest a label on
    /// every resize of the container it sits in.
    fn set_width(&mut self, width: f32) -> bool {
        if self.width == width {
            return false;
        }
        // SAFETY: a call on an interface this run owns, taking one scalar.
        if unsafe { self.layout.SetMaxWidth(width) }.is_err() {
            return false;
        }
        self.width = width;
        self.pad.set(None);
        self.dirty |= self.flow != Flow::Line;
        true
    }

    /// How far the ink reaches past the layout box, read on demand because it is a
    /// property of the box rather than of the glyphs.
    fn pad(&self) -> DWRITE_OVERHANG_METRICS {
        if let Some(pad) = self.pad.get() {
            return pad;
        }
        // SAFETY: a call on an interface this run owns, taking no arguments.
        let pad = unsafe { self.layout.GetOverhangMetrics() }.unwrap_or_default();
        self.pad.set(Some(pad));
        pad
    }

    /// Reads back what the walk could not: line boxes, ink overhang, and which line each
    /// segment landed on.
    fn after_walk(&mut self) -> Result<()> {
        self.read_lines()?;

        // A segment's baseline y is its line's top plus that line's baseline, and both
        // come from the same layout — so the nearest match is an exact one, and nearest
        // rather than equal only so a run can never end up on no line at all.
        for h in &mut self.harvest.segs {
            let (mut top, mut best, mut nearest) = (0.0f32, 0u16, f32::MAX);
            for (i, line) in self.lines.iter().enumerate() {
                let gap = (top + line.baseline - h.origin.y).abs();
                if gap < nearest {
                    (nearest, best) = (gap, i as u16);
                }
                top += line.height;
            }
            h.line = best;
        }
        self.dirty = false;
        Ok(())
    }

    fn read_lines(&mut self) -> Result<()> {
        let mut count = 0u32;
        // The first call reports the count it needs through the out-parameter and returns
        // the expected insufficient-buffer error, so the result is deliberately dropped.
        // SAFETY: both calls take a slice this run owns and a stack-local counter.
        unsafe {
            let _ = self.layout.GetLineMetrics(None, &mut count);
        }
        // Reused across harvests, so a paragraph re-flowing under a resize drag allocates
        // once and then never again.
        self.raw_lines.clear();
        self.raw_lines
            .resize(count as usize, DWRITE_LINE_METRICS::default());
        if count > 0 {
            // SAFETY: as above; the slice is sized from the count the probe reported.
            unsafe {
                self.layout
                    .GetLineMetrics(Some(&mut self.raw_lines), &mut count)
                    .ok()?;
            }
        }
        self.lines.clear();
        self.lines.extend(
            self.raw_lines
                .iter()
                .take(count as usize)
                .map(|m| LineMetrics {
                    length: m.length,
                    trailing_whitespace: m.trailingWhitespaceLength,
                    newline: m.newlineLength,
                    height: m.height,
                    baseline: m.baseline,
                }),
        );
        Ok(())
    }

    /// `line`'s top edge and metrics.
    fn line_at(&self, line: usize) -> Option<(f32, LineMetrics)> {
        let mut top = 0.0f32;
        for (i, m) in self.lines.iter().enumerate() {
            if i == line {
                return Some((top, *m));
            }
            top += m.height;
        }
        None
    }

    /// `line`'s horizontal ink span, in layout space.
    ///
    /// Taken from the segments rather than from the line's own width because a
    /// right-to-left segment advances leftward from its origin, so its extent is not
    /// `origin.x + width` and folding advances would place its tile past its glyphs.
    fn line_extent(&self, line: usize) -> (f32, f32) {
        let (mut left, mut right) = (f32::MAX, f32::MIN);
        for h in self.harvest.segs.iter().filter(|h| h.line as usize == line) {
            let w: f32 = h.seg.advances.of(&self.harvest.advances).iter().sum();
            let (a, b) = if h.seg.bidi % 2 == 1 {
                (h.origin.x - w, h.origin.x)
            } else {
                (h.origin.x, h.origin.x + w)
            };
            left = left.min(a);
            right = right.max(b);
        }
        if left > right {
            (0.0, 0.0)
        } else {
            (left, right)
        }
    }

    /// The code-unit range `line` covers.
    #[must_use]
    pub fn line_range(&self, line: usize) -> Range<u32> {
        let start: u32 = self.lines.iter().take(line).map(|m| m.length).sum();
        let len = self.lines.get(line).map_or(0, |m| m.length);
        start..start + len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "Segoe UI" is present on every supported floor, and a ladder of one is enough to
    /// exercise everything except fallback — which is the point of the second family
    /// arriving without being declared.
    fn engine() -> (TextEngine, FontSpec) {
        let engine = TextEngine::new(FontLadder::new(["Segoe UI"])).unwrap();
        (engine, FontSpec::new(FamilyId(0), 14.0))
    }

    fn shaped(engine: &TextEngine, text: &str, spec: &FontSpec, flow: Flow, w: f32) -> ShapedRun {
        let mut run = engine.shape(text, spec, flow).unwrap();
        run.pin(w);
        engine.harvest(&mut run).unwrap();
        run
    }

    fn segs_of(run: &ShapedRun, line: usize) -> (SegBuffers, Span) {
        let mut buffers = SegBuffers::default();
        let span = run.segments(line, &mut buffers);
        (buffers, span)
    }

    #[test]
    fn a_run_accounts_for_all_of_its_text() {
        let (engine, spec) = engine();
        let mut run = shaped(&engine, "Hello", &spec, Flow::Line, 400.0);
        let (buffers, span) = segs_of(&run, 0);

        assert_eq!(span.len(), 1, "one face, so one segment");
        let seg = buffers.segs[0];
        assert_eq!(seg.glyphs.len(), 5);
        let width: f32 = seg.advances.of(&buffers.advances).iter().sum();
        assert!((width - run.measure(Some(400.0)).x).abs() < 0.01);
    }

    /// The test the whole [`FaceId`] mechanism exists for. A segment naming only the
    /// requested [`FontSpec`] would draw the second run's glyph ids through the first
    /// run's face, which is not tofu — it is arbitrary glyphs at arbitrary positions.
    #[test]
    fn fallback_splits_a_line_and_every_face_it_chose_round_trips() {
        let ladder = FontLadder::new(["Segoe UI"]);
        let engine = TextEngine::new(ladder.clone()).unwrap();
        let spec = FontSpec::new(FamilyId(0), 14.0);
        let run = shaped(&engine, "Hello 世界", &spec, Flow::Line, 400.0);
        let (buffers, span) = segs_of(&run, 0);

        assert!(span.len() >= 2, "CJK does not come from the Latin face");
        let mut faces: Vec<_> = buffers.segs.iter().map(|s| s.face).collect();
        faces.dedup();
        assert!(faces.len() >= 2, "and the segments must say so");

        // The other half of the seam: a thread that did no shaping resolves every id the
        // shaping thread minted, through nothing but the shared ladder.
        let far_side = TextEngine::new(ladder).unwrap();
        for face in faces {
            assert!(far_side.face(face).is_ok(), "{face:?} did not round-trip");
        }
    }

    /// A trimming sign is an inline object, so a walk that quietly succeeded without
    /// re-entering would shorten the run at the trim point and never emit the `…`.
    #[test]
    fn a_trimmed_run_yields_its_ellipsis_as_glyphs() {
        let (engine, spec) = engine();
        let text = "a label far too long for the space it was given";
        let full = shaped(&engine, text, &spec, Flow::Line, 40.0);
        let cut = shaped(&engine, text, &spec, Flow::Ellipsis, 40.0);

        let (full_b, _) = segs_of(&full, 0);
        let (cut_b, cut_span) = segs_of(&cut, 0);
        assert!(!cut_span.is_empty(), "a trimmed run still draws");
        assert!(
            cut_b.glyphs.len() < full_b.glyphs.len(),
            "trimming dropped nothing"
        );
        let width: f32 = cut_b.advances.iter().sum();
        assert!(width <= 40.5, "trimmed to {width}, not to the box");
    }

    #[test]
    fn tabular_figures_give_every_digit_one_advance() {
        let (engine, spec) = engine();
        let spec = spec.features(FontFeatures::TABULAR);
        let run = shaped(&engine, "0123456789", &spec, Flow::Line, 400.0);
        let (buffers, _) = segs_of(&run, 0);

        let advances = buffers.segs[0].advances.of(&buffers.advances);
        assert_eq!(advances.len(), 10);
        assert!(
            advances.windows(2).all(|w| (w[0] - w[1]).abs() < 0.001),
            "a read-out shifts as its digits change: {advances:?}"
        );
    }

    #[test]
    fn the_wrap_pin_decides_where_the_glyphs_land() {
        let (engine, spec) = engine();
        let text = "a paragraph with enough words in it to need more than one line";
        let mut run = engine.shape(text, &spec, Flow::Wrap).unwrap();

        // What a solve does: probe max-content, probe min-content, then decide. Whichever
        // probe ran last would otherwise be what the glyphs were placed against.
        let wide = run.measure(None);
        let narrow = run.measure(Some(run.min_width()));
        assert!(wide.x > narrow.x && narrow.y > wide.y);

        assert!(run.pin(180.0), "a wrapping run's glyphs move with its box");
        engine.harvest(&mut run).unwrap();
        assert!(run.lines().len() > 1);

        // A run shaped and pinned directly to the same width is the same run.
        let fresh = shaped(&engine, text, &spec, Flow::Wrap, 180.0);
        assert_eq!(run.lines(), fresh.lines());
        assert_eq!(segs_of(&run, 0).0.advances, segs_of(&fresh, 0).0.advances);
    }

    #[test]
    fn a_non_wrapping_run_is_not_staled_by_its_box() {
        let (engine, spec) = engine();
        let mut run = shaped(&engine, "Bands", &spec, Flow::Line, 400.0);
        assert!(!run.pin(40.0), "it is laid out leading and does not break");
        assert!(!run.is_stale());
    }

    #[test]
    fn an_empty_run_shapes_nothing_and_still_measures() {
        let (engine, spec) = engine();
        let run = shaped(&engine, "", &spec, Flow::Line, 400.0);
        assert_eq!(segs_of(&run, 0).1.len(), 0);
        assert_eq!(run.line_ink(0).size.x, 0.0);
        assert!(run.line_ink(0).size.y > 0.0, "an empty line has a height");
    }

    #[test]
    fn a_lines_ink_holds_its_baseline_and_its_overhang() {
        let (engine, spec) = engine();
        let run = shaped(&engine, "Ág", &spec, Flow::Line, 400.0);
        let ink = run.line_ink(0);
        assert!(ink.baseline.y > 0.0 && ink.baseline.y < ink.size.y);
        // A descender and an accent both leave the line box, and the tile has to hold them.
        let (buffers, _) = segs_of(&run, 0);
        let advance: f32 = buffers.advances.iter().sum();
        assert!(ink.size.x >= advance);
    }

    #[test]
    fn min_width_is_the_longest_unbreakable_span() {
        let (engine, spec) = engine();
        let run = shaped(&engine, "a bbbbbbbb c", &spec, Flow::Wrap, 400.0);
        let word = shaped(&engine, "bbbbbbbb", &spec, Flow::Line, 400.0);
        let mut word = word;
        assert!((run.min_width() - word.measure(None).x).abs() < 0.5);
    }

    #[test]
    fn a_caret_and_a_point_agree() {
        let (engine, spec) = engine();
        let run = shaped(&engine, "Hello", &spec, Flow::Line, 400.0);
        let (at, _) = run.caret(3, false);
        assert!(at.x > 0.0);

        // A point just inside the cluster that starts at that caret resolves back to it.
        let hit = run.hit_test(Vector2 {
            x: at.x + 1.0,
            y: at.y + 1.0,
        });
        assert_eq!(hit.position, 3);
        assert!(hit.inside && !hit.is_rtl());
    }

    #[test]
    fn a_selection_covers_one_box_per_line() {
        let (engine, spec) = engine();
        let run = shaped(&engine, "one two three four five", &spec, Flow::Wrap, 60.0);
        let mut rects = Vec::new();
        run.cluster_rects(0..run.len(), &mut rects);
        assert!(rects.len() >= run.lines().len());
        assert!(rects.iter().all(|r| r.w > 0.0 && r.h > 0.0));
    }

    /// Every extent this crate hands out is in DIPs, so a tile's size is a fact about the
    /// text and not about the display it will land on. A scale-bearing extent would be
    /// stale the moment the window moved to another monitor, and nothing would say so.
    #[test]
    fn ink_is_scale_free() {
        let (engine, spec) = engine();
        let run = shaped(&engine, "Hello", &spec, Flow::Line, 400.0);
        let ink = run.line_ink(0);
        let (buffers, _) = segs_of(&run, 0);
        let advance: f32 = buffers.advances.iter().sum();
        // The tile is the ink, in the same units the advances are in. Whatever pixel grid
        // it is rasterized against is applied by whoever rasterizes it.
        assert!(ink.size.x >= advance && ink.size.x < advance + spec.size);
    }

    /// Segment origins are relative to the tile, so a caller draws at `tile + origin` and
    /// never folds advances — which is what a bidi line, where visual order and advance
    /// order disagree, would get wrong.
    #[test]
    fn segment_origins_are_rebased_onto_their_tile() {
        let (engine, spec) = engine();
        let run = shaped(&engine, "Hello 世界", &spec, Flow::Line, 400.0);
        let ink = run.line_ink(0);
        let (buffers, span) = segs_of(&run, 0);

        assert!(span.len() >= 2);
        for seg in &buffers.segs {
            assert!((seg.origin.y - ink.baseline.y).abs() < 0.01, "one baseline");
            assert!(seg.origin.x >= 0.0 && seg.origin.x <= ink.size.x);
        }
        // The second segment starts where the first one ends, and nobody had to add it up.
        let first: f32 = buffers.segs[0].advances.of(&buffers.advances).iter().sum();
        assert!((buffers.segs[1].origin.x - (buffers.segs[0].origin.x + first)).abs() < 0.5);
    }

    /// A second line's segments must not carry the first line's, and each line's tile is
    /// its own — this is what makes a wrapped paragraph N sprites rather than one.
    #[test]
    fn each_line_gets_its_own_segments_and_tile() {
        let (engine, spec) = engine();
        let run = shaped(
            &engine,
            "one two three four five six",
            &spec,
            Flow::Wrap,
            60.0,
        );
        assert!(run.lines().len() > 1);

        let mut buffers = SegBuffers::default();
        let mut total = 0;
        for line in 0..run.lines().len() {
            let span = run.segments(line, &mut buffers);
            assert!(!span.is_empty(), "line {line} placed nothing");
            total += span.len();
            let ink = run.line_ink(line);
            assert!(ink.size.x > 0.0 && ink.size.y > 0.0);
            // Each line's segments sit inside that line's own tile.
            for seg in span.of(&buffers.segs) {
                assert!(seg.origin.x >= 0.0 && seg.origin.x <= ink.size.x);
                assert!((seg.origin.y - ink.baseline.y).abs() < 0.01);
            }
        }
        assert_eq!(total, buffers.segs.len());
    }

    #[test]
    fn a_reshape_keeps_the_runs_buffers() {
        let (engine, spec) = engine();
        let mut run = shaped(&engine, "Hello", &spec, Flow::Line, 400.0);
        let capacity = run.harvest.glyphs.capacity();
        engine
            .reshape(&mut run, "Goodbye", &spec, Flow::Line)
            .unwrap();
        assert!(run.is_stale());
        engine.harvest(&mut run).unwrap();
        // Grown, never replaced: a reshape that swapped in a fresh `Harvest` would put an
        // allocation on every text change.
        assert!(run.harvest.glyphs.capacity() >= capacity);
        assert_eq!(segs_of(&run, 0).0.glyphs.len(), 7);
    }
}
