//! Text drawn as one composition sprite per glyph.
//!
//! The glyph atlas ([`super::glyph_atlas`]) holds one mask per glyph; this
//! places them. Every glyph is a [`SpriteVisual`] whose brush is a
//! [`CompositionMaskBrush`] — the atlas mask cutting a **shared** FP16 colour
//! source. Once placed, nothing about the text reaches a `BeginDraw` again:
//!
//! - a **recolour** rebuilds one source surface and points every glyph's mask
//!   brush at it, so it costs N `SetSource` calls and zero rasters;
//! - **enable/disable** is the container's opacity;
//! - a **text change** re-places sprites against masks that are already cached
//!   whenever the letters have been seen before.
//!
//! One source per run, not per glyph, is the whole point of the sharing: the
//! source is a 4×4 solid, and minting one per glyph would put an allocation on
//! a path whose reason to exist is that it has none.
//!
//! ## Three runs, not one
//!
//! A [`TextPart`] is one shaped run — it knows nothing about what the run
//! means. A button-family node owns [`ButtonText`]: its label, its leading icon
//! glyph and its badge's count, each an independent run with its own layout,
//! colour and origin. The icon is a run rather than a painted character because
//! a button that painted even one glyph would need a surface, and the whole
//! point of the family's chrome being retained is that it never gets one.
//!
//! A part with nothing to place never mints its host, so an ordinary label-only
//! button still owns exactly one container.
//!
//! ## The host visual
//!
//! Every glyph sprite parents into ONE container the run owns, rather than
//! into the node directly. That container is what makes the two whole-run
//! operations single writes instead of per-glyph loops:
//!
//! - **disabled dim** is its `Opacity`, which is also the only correct place for
//!   it. The painted path folded the dim into the brush alpha *after* the output
//!   colour transform, and a mask brush's FP16 source is rasterized through that
//!   transform — so there is no pre-transform alpha that reproduces it. A visual
//!   opacity composites after everything, which is exactly what the old
//!   `put(brush, c, dim)` did.
//! - **clipping** is its `InsetClip`. A painted run got clipped for free by the
//!   fixed-size surface it drew into; sprites are not clipped by anything, so a
//!   label wider than its control would spill outside the button without this.
//!
//! ## Z-order
//!
//! Each host is inserted at the top of the node's children, so the text sits
//! above both the chrome parts *and* the hover/press ink. That is a deliberate
//! departure from the painted label, which sat under the ink and was lightened
//! by it: a wash belongs on the surface behind text, not over the text, and
//! WinUI's own button states recolour the background alone.
//!
//! The ordering is positional, not declared — it holds because the hosts are
//! created after `parts::ensure` has built the ink, and it would break if
//! anything re-stacked the node's children afterwards. The badge count is the
//! one run whose order against a sibling matters: its plate is a chrome part
//! *below* the surface band, so the count lands above it whichever order the
//! three runs happen to mint their hosts in.
//!
//! WITHIN a host the ordering is declared rather than positional, because one
//! case genuinely needs it: an editor's selection wash must sit under the very
//! glyphs it highlights. Fills therefore hang in their own container at the
//! bottom of the host, so a fill minted after a glyph still lands below it.
//! Relying on insertion order there would mean text disappearing behind its own
//! highlight — a bug that stays invisible until a selection happens to grow.
//! Decoration rules take the run's own ink and so need no such ordering against
//! it.

use windows_canvas::{Rect, TextDecoration, TextLayout};
use windows_composition::{
    CompositionMaskBrush, CompositionSurfaceBrush, ContainerVisual, SpriteVisual, Visual,
};

use super::bootstrap::Compositing;
use super::glyph_atlas::{pen_phase, GlyphAtlas};
use super::node::Node;
use super::parts::build_solid_surface;
use super::theme;

/// One glyph's sprite and the mask brush that colours it.
///
/// Only the `Visual` view is retained: the sprite itself is parented into the
/// host, which owns the reference that keeps it alive, and every operation here
/// (place, show) is a visual one.
struct GlyphSprite {
    vis: Visual,
    mask: CompositionMaskBrush,
    /// [`GlyphRaster::id`](super::glyph_atlas::GlyphRaster::id) of the atlas
    /// mask currently bound. Identity is the right test here: the atlas hands
    /// back the same raster for a cache hit, so an unchanged glyph compares
    /// equal and re-binds nothing — which is what makes a recolour or a
    /// re-place cost zero rasters.
    ///
    /// An id rather than the brush itself, and deliberately: comparing
    /// `CompositionSurfaceBrush`es is COM identity, so it costs a
    /// `QueryInterface` per side — and this compare runs once per glyph on
    /// every sync. See the atlas item for the full argument.
    bound: Option<u64>,
    offset: Option<(f32, f32)>,
    size: Option<(f32, f32)>,
    shown: bool,
}

impl GlyphSprite {
    fn new(comp: &Compositing, host: &ContainerVisual) -> Self {
        let sprite = comp.new_sprite();
        let mask = sprite.compositor().create_mask_brush();
        sprite.set_brush(&mask);
        host.children().insert_at_top(&sprite);
        Self {
            // The sprite's own `Visual` face, cloned out before the sprite is
            // dropped — the host's child collection is what keeps it alive.
            vis: Visual::clone(&sprite),
            mask,
            bound: None,
            offset: None,
            size: None,
            shown: false,
        }
    }

    fn place(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if self.offset != Some((x, y)) {
            self.vis.set_offset(x, y, 0.0);
            self.offset = Some((x, y));
        }
        if self.size != Some((w, h)) {
            self.vis.set_size(w, h);
            self.size = Some((w, h));
        }
    }

    fn show(&mut self, on: bool) {
        if self.shown != on {
            self.vis.set_visible(on);
            self.shown = on;
        }
    }
}

/// A solid rectangle inside a text host: a decoration rule, or a selection
/// fill.
///
/// Not a glyph and not a mask — a rule has no outline and no coverage, so it is
/// a sprite painted with a colour source directly rather than one cut by an
/// atlas entry.
struct RectSprite {
    /// The sprite is kept whole here (not just its `Visual` face) because a
    /// rule is REPAINTED as well as placed, and `set_brush` lives on the
    /// sprite. It derefs to `Visual`, so placement reads the same.
    sprite: SpriteVisual,
    /// The brush currently bound, on the same identity argument
    /// [`GlyphSprite::bound`] makes.
    ///
    /// This one holds the brush rather than an id: there are at most a handful
    /// of rules and fills per label, so the COM identity compare is not on a
    /// per-glyph path and the brush is the thing actually being compared.
    bound: Option<CompositionSurfaceBrush>,
    offset: Option<(f32, f32)>,
    size: Option<(f32, f32)>,
    shown: bool,
}

impl RectSprite {
    fn new(comp: &Compositing, parent: &ContainerVisual) -> Self {
        let sprite = comp.new_sprite();
        parent.children().insert_at_top(&sprite);
        Self {
            sprite,
            bound: None,
            offset: None,
            size: None,
            shown: false,
        }
    }

    fn paint(&mut self, brush: &CompositionSurfaceBrush) {
        if self.bound.as_ref() != Some(brush) {
            self.sprite.set_brush(brush);
            self.bound = Some(brush.clone());
        }
    }

    fn place(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if self.offset != Some((x, y)) {
            self.sprite.set_offset(x, y, 0.0);
            self.offset = Some((x, y));
        }
        if self.size != Some((w, h)) {
            self.sprite.set_size(w, h);
            self.size = Some((w, h));
        }
    }

    fn show(&mut self, on: bool) {
        if self.shown != on {
            self.sprite.set_visible(on);
            self.shown = on;
        }
    }
}

/// The retained glyph sprites of one label.
#[derive(Default)]
pub(crate) struct TextPart {
    /// The container every glyph parents into — the label's dim and its clip.
    /// See the module header for why both belong on a visual rather than on the
    /// glyphs or the colour.
    host: Option<ContainerVisual>,
    /// Last box pushed to the host: its offset, and the size its zero inset
    /// clip cuts to.
    ///
    /// It is a full rect rather than a size because an editor's text is clipped
    /// to its CONTENT COLUMN, not to its box — a scrolled field would otherwise
    /// spill its run over the spin buttons and out through the border. A
    /// painted run got that clip for free from `push_clip`; sprites are clipped
    /// by nothing, so the host has to be the column.
    host_box: Option<Rect>,
    /// Last dim pushed to the host's opacity.
    host_dim: Option<f32>,
    glyphs: Vec<GlyphSprite>,
    /// How many of `glyphs` the last sync actually used. The rest stay
    /// allocated and hidden — a label that shortens will very likely lengthen
    /// again, and a composition visual is cheaper to hide than to rebuild.
    live: usize,
    /// The one FP16 colour source every glyph's mask brush reads.
    source: Option<CompositionSurfaceBrush>,
    /// `(colour bits, scale bits)` the source was built for.
    source_for: Option<([u32; 4], u32)>,
    /// Underline / strikethrough rules. They take the text's own colour source,
    /// so they cost no surface of their own, and they sit among the glyphs
    /// rather than under them — with identical ink, the order is immaterial.
    rules: Vec<RectSprite>,
    /// Selection fills, and the container that keeps them BELOW every glyph.
    ///
    /// The container is what makes the ordering robust rather than incidental.
    /// Sprites here are minted lazily, so relying on insertion order against the
    /// glyphs would mean a fill minted after a glyph landing on top of it —
    /// which is exactly the bug (text disappearing behind its own highlight)
    /// that is invisible until a selection happens to grow.
    fill_host: Option<ContainerVisual>,
    fills: Vec<RectSprite>,
    /// The one colour source every fill reads, and the `(colour, scale)` it was
    /// built for. Separate from [`source`](Self::source) because a highlight is
    /// deliberately not the text's colour.
    fill_source: Option<CompositionSurfaceBrush>,
    fill_source_for: Option<([u32; 4], u32)>,
    /// The atlas epoch the bound masks came from; a bump invalidates them all.
    epoch: u32,
}

fn color_bits(c: crate::Color) -> [u32; 4] {
    [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]
}

impl TextPart {
    /// Mint the host container (once) and hang it at the top of `parent`'s
    /// children. Its inset clip is all zeros, which cuts to the host's own
    /// `Size` — pushed per sync by [`place_host`](Self::place_host).
    fn ensure_host(&mut self, comp: &Compositing, parent: &ContainerVisual) {
        if self.host.is_some() {
            return;
        }
        let host = comp.new_container();
        // All-zero insets, which cuts to the host's own `Size` — pushed per
        // sync by `place_host`.
        host.set_clip(Some(&host.compositor().create_inset_clip()));
        parent.children().insert_at_top(&host);
        self.host = Some(host);
    }

    /// Place and size the host to the box its clip cuts to, and carry the
    /// disabled dim on its opacity. All three self-gate.
    fn place_host(&mut self, box_: Rect, dim: f32) {
        let Some(host) = self.host.as_ref() else { return };
        if self.host_box != Some(box_) {
            host.set_offset(box_.left, box_.top, 0.0);
            host.set_size(box_.width(), box_.height());
            self.host_box = Some(box_);
        }
        if self.host_dim != Some(dim) {
            host.set_opacity(dim);
            self.host_dim = Some(dim);
        }
    }

    /// Node-local point → host-local, which is what every sprite offset is in.
    fn to_host(&self, p: (f32, f32)) -> (f32, f32) {
        match self.host_box {
            Some(b) => (p.0 - b.left, p.1 - b.top),
            None => p,
        }
    }

    /// Rebuild the shared colour source if the colour or scale moved, and point
    /// every live glyph at it.
    ///
    /// Returns the source, or `None` if it could not be built — in which case
    /// the caller must not show the glyphs, since a mask brush with no source
    /// paints nothing and a half-coloured label is worse than an absent one.
    fn ensure_source(
        &mut self,
        comp: &Compositing,
        color: crate::Color,
        scale: f32,
    ) -> Option<CompositionSurfaceBrush> {
        let want = (color_bits(color), scale.to_bits());
        if self.source_for != Some(want) {
            let src = build_solid_surface(comp, color, scale)?;
            // Re-point every sprite, including ones this sync will not touch:
            // a hidden sprite that is shown again later must not surface the
            // previous colour.
            for g in &mut self.glyphs {
                g.mask.set_source(&src);
            }
            self.source = Some(src);
            self.source_for = Some(want);
        }
        // Cloned, not borrowed: the placement loop below needs `self.glyphs`
        // mutably while it holds this. One `AddRef` per dirty sync — where the
        // painted path paid a `QueryInterface` here for the same reason.
        self.source.clone()
    }

    /// Place the label's glyphs.
    ///
    /// `origin` is the top-left of the text box in node-local DIPs; the run's
    /// own baseline origin is added to it. `scale` is DIP→px.
    ///
    /// Placement uses the SHAPED advances from the run, not the design advances
    /// the atlas reports — kerning and other GPOS positioning live in the
    /// former, and using the latter would space text correctly only for pairs
    /// the font does not kern.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sync(
        &mut self,
        comp: &Compositing,
        atlas: &mut GlyphAtlas,
        parent: &ContainerVisual,
        layout: &TextLayout,
        origin: (f32, f32),
        host_box: Rect,
        color: crate::Color,
        dim: f32,
        scale: f32,
    ) {
        self.ensure_host(comp, parent);
        self.place_host(host_box, dim);
        // Everything below is host-local: the host is no longer necessarily at
        // the node's origin (an editor's is its content column).
        let origin = self.to_host(origin);
        // One AddRef per dirty sync, so the placement loop can grow sprites into
        // the host while `self.glyphs` is mutably borrowed.
        let Some(host) = self.host.clone() else { return };
        let Ok(runs) = layout.glyph_runs() else {
            self.hide_from(0);
            return;
        };
        if atlas.epoch() != self.epoch {
            // The atlas was cleared (device loss, DPI, theme): every bound mask
            // is stale, so drop the identity cache and let the walk re-bind.
            for g in &mut self.glyphs {
                g.bound = None;
            }
            self.epoch = atlas.epoch();
        }
        let Some(source) = self.ensure_source(comp, color, scale) else {
            self.hide_from(0);
            return;
        };

        let mut slot = 0usize;
        for run in &runs {
            // The baseline row is snapped to a whole physical pixel, once per
            // run. There is no vertical subpixel phase (glyphs are rasterized at
            // horizontal phases only), so an unsnapped baseline would put every
            // sprite in the run between two pixel rows and the compositor would
            // resample all of them.
            let baseline_py = ((origin.1 + run.baseline_origin.y) * scale).round() as i32;
            let mut pen_x = origin.0 + run.baseline_origin.x;
            for (i, &glyph) in run.glyph_indices.iter().enumerate() {
                // A glyph offset DISPLACES this one glyph; it does not move the
                // pen. GPOS mark positioning is expressed entirely through it —
                // an acute accent is a glyph at the pen of the letter it sits on,
                // nudged up and sideways — so folding it into the pen would both
                // drop the vertical half and smear the horizontal half across
                // every glyph after it.
                let (off_x, off_y) = run
                    .glyph_offsets
                    .get(i)
                    .map_or((0.0, 0.0), |o| (o.advance_offset, o.ascender_offset));
                let (whole_px, phase) = pen_phase(pen_x + off_x, scale);
                // `ascender_offset` points toward the ascender, i.e. up the
                // screen, which is the negative y direction.
                let glyph_py = baseline_py - (off_y * scale).round() as i32;
                if let Some(raster) =
                    atlas.get(comp, &run.font_face, glyph, run.font_em_size, scale, phase)
                {
                    // Grow on demand; a label only pays for the glyphs it has.
                    if slot == self.glyphs.len() {
                        self.glyphs.push(GlyphSprite::new(comp, &host));
                    }
                    let g = &mut self.glyphs[slot];
                    // An integer compare, and nothing else, on the path every
                    // glyph of every label takes on every sync.
                    if g.bound != Some(raster.id) {
                        g.mask.set_mask(&raster.brush);
                        g.mask.set_source(&source);
                        g.bound = Some(raster.id);
                    }
                    // Integer minus integer, then one divide: the sprite lands
                    // exactly on the pixel grid, which is what keeps the mask a
                    // 1:1 blit instead of a bilinear resample.
                    let (w, h) = raster.geom.size_dip;
                    let (ox, oy) = raster.geom.origin_px;
                    g.place(
                        (whole_px - ox) as f32 / scale,
                        (glyph_py - oy) as f32 / scale,
                        w,
                        h,
                    );
                    g.show(true);
                    slot += 1;
                }
                // The pen moves by the SHAPED advance alone — kerning and other
                // GPOS positioning are already in it, and the per-glyph offset
                // above is not part of it.
                pen_x += run.glyph_advances.get(i).copied().unwrap_or(0.0);
            }
        }
        self.live = slot;
        self.hide_from(slot);
    }

    /// Place the layout's underline / strikethrough rules.
    ///
    /// Call after [`sync`](Self::sync), which is what mints the host and builds
    /// the colour source these share. `decorations` come from
    /// [`TextLayout::shape`] — [`glyph_runs`](TextLayout::glyph_runs) drops
    /// them, and does so silently.
    ///
    /// `origin` must be the same one `sync` was given: a decoration's baseline
    /// origin is reported in the layout's space exactly as a run's is.
    pub(crate) fn sync_rules(
        &mut self,
        comp: &Compositing,
        decorations: &[TextDecoration],
        origin: (f32, f32),
        scale: f32,
    ) {
        let (Some(host), Some(brush)) = (self.host.clone(), self.source.clone()) else {
            self.hide_rules_from(0);
            return;
        };

        let origin = self.to_host(origin);
        let mut slot = 0usize;
        for d in decorations {
            let (x, y, w, h) = d.rect(false);
            if !(w > 0.0 && h > 0.0) {
                continue;
            }
            if slot == self.rules.len() {
                self.rules.push(RectSprite::new(comp, &host));
            }
            let r = &mut self.rules[slot];
            r.paint(&brush);
            // Snapped to whole physical pixels for the same reason a glyph box
            // is: a hairline rule at a fractional offset is resampled across two
            // rows and renders as a grey smear at half the intended weight.
            let top = ((origin.1 + y) * scale).round() / scale;
            let bottom = (((origin.1 + y) + h) * scale).round().max((origin.1 + y) * scale + 1.0) / scale;
            r.place(origin.0 + x, top, w, bottom - top);
            r.show(true);
            slot += 1;
        }
        self.hide_rules_from(slot);
    }

    /// Place selection fills BELOW the glyphs.
    ///
    /// `rects` are layout-relative, as [`TextLayout::hit_test_range`] returns
    /// them; `origin` is the same one [`sync`](Self::sync) was given.
    pub(crate) fn sync_fills(
        &mut self,
        comp: &Compositing,
        rects: &[(f32, f32, f32, f32)],
        origin: (f32, f32),
        color: crate::Color,
        scale: f32,
    ) {
        if rects.is_empty() {
            self.hide_fills_from(0);
            return;
        }
        let Some(host) = self.host.clone() else {
            self.hide_fills_from(0);
            return;
        };
        // The fill container hangs at the BOTTOM of the host, so everything in
        // it is under every glyph however the two are minted relative to
        // each other.
        let fill_host = match self.fill_host.clone() {
            Some(c) => c,
            None => {
                let c = comp.new_container();
                host.children().insert_at_bottom(&c);
                self.fill_host = Some(c.clone());
                c
            }
        };

        let want = (color_bits(color), scale.to_bits());
        if self.fill_source_for != Some(want) {
            if let Some(s) = build_solid_surface(comp, color, scale) {
                self.fill_source = Some(s);
                self.fill_source_for = Some(want);
                // Re-point every fill, hidden ones included — a fill shown
                // again later must not surface the previous colour.
                self.fills.iter_mut().for_each(|f| f.bound = None);
            } else {
                self.hide_fills_from(0);
                return;
            }
        }
        let Some(brush) = self.fill_source.clone() else {
            self.hide_fills_from(0);
            return;
        };

        let origin = self.to_host(origin);
        let mut slot = 0usize;
        for &(x, y, w, h) in rects {
            if !(w > 0.0 && h > 0.0) {
                continue;
            }
            if slot == self.fills.len() {
                self.fills.push(RectSprite::new(comp, &fill_host));
            }
            let f = &mut self.fills[slot];
            f.paint(&brush);
            f.place(origin.0 + x, origin.1 + y, w, h);
            f.show(true);
            slot += 1;
        }
        self.hide_fills_from(slot);
    }

    fn hide_rules_from(&mut self, from: usize) {
        let start = from.min(self.rules.len());
        self.rules[start..].iter_mut().for_each(|r| r.show(false));
    }

    fn hide_fills_from(&mut self, from: usize) {
        let start = from.min(self.fills.len());
        self.fills[start..].iter_mut().for_each(|f| f.show(false));
    }

    fn hide_from(&mut self, from: usize) {
        let start = from.min(self.glyphs.len());
        for g in &mut self.glyphs[start..] {
            g.show(false);
        }
        self.live = self.live.min(from);
    }

    /// Hide the whole label — for a node that has stopped owning a retained one
    /// (its text was cleared, or it stopped being a button). The sprites stay
    /// allocated: the same node very often gets a label back.
    pub(crate) fn hide_all(&mut self) {
        self.hide_from(0);
        self.hide_rules_from(0);
        self.hide_fills_from(0);
    }
}

/// Every retained run a button-family node draws.
///
/// The label's shaped run lives in the node's generic `text_layout` slot, which
/// the layout pass already maintains for every text-bearing control. The two
/// ornament runs need their own: the icon is a different family at a different
/// size, and the count is the badge's smaller, heavier type.
#[derive(Default)]
pub(crate) struct ButtonText {
    label: TextPart,
    /// The leading icon glyph's shaped run, rebuilt by the layout pass.
    pub(crate) icon_layout: Option<TextLayout>,
    icon: TextPart,
    /// The badge count's shaped run, rebuilt by the layout pass. `None` for the
    /// dot form, which has no text at all.
    pub(crate) badge_layout: Option<TextLayout>,
    badge: TextPart,
}

/// The runs a control owns one of PER ITEM, which its single `text_layout` slot
/// cannot hold: a `SelectorBar`'s segment labels, and a `ToggleSwitch`'s two
/// state labels.
///
/// Both vectors are positional — index `i` is item `i` — so a run that failed to
/// build holds its slot as `None` rather than shifting every item after it onto
/// the wrong words.
///
/// `strong` exists because a selected segment sets at 600 while its neighbours
/// stay at 400, and a weight is baked into a layout at construction. Keeping
/// both weights shaped means **selection is never a rebuild**: the layout pass
/// only runs on `text_dirty`, which picking a segment does not set, so a sync
/// that reshaped on selection would either miss the flip entirely or put a
/// DirectWrite layout build on the click path. A control with no emphasis weight
/// (the toggle) leaves it empty.
#[derive(Default)]
pub(crate) struct ItemText {
    /// One shaped run per item at the rest weight, rebuilt by the layout pass.
    pub(crate) layouts: Vec<Option<TextLayout>>,
    /// The same items at the weight a selected one takes, or empty.
    pub(crate) strong: Vec<Option<TextLayout>>,
    /// One placed run per item. Grown on demand and never shrunk — an item count
    /// that drops usually comes back, and a hidden part costs nothing.
    parts: Vec<TextPart>,
}

impl ItemText {
    /// Every run the control could ever show, for the measure pass.
    ///
    /// The emphasis runs when there are any, else the rest ones — the widest
    /// item at the weight it will be widest at. Sizing a control to the runs it
    /// happens to be showing right now is what makes a row reflow when a
    /// selection moves or a switch flips.
    pub(crate) fn measurable(&self) -> impl Iterator<Item = &TextLayout> {
        let v = if self.strong.is_empty() {
            &self.layouts
        } else {
            &self.strong
        };
        v.iter().flatten()
    }

    /// Part `part` and the run for item `item`, borrowed together.
    ///
    /// One call rather than two accessors because the two live in different
    /// fields: the part is taken mutably and the run shared, which is a split
    /// borrow every caller would otherwise have to spell out by hand. The part
    /// is grown into existence if this is the first sync to reach that far.
    ///
    /// The two indices are separate because they are separate questions, and
    /// only one control makes them the same one. A segment bar places item `i`
    /// into part `i`; a toggle owns TWO shaped labels and shows ONE of them, so
    /// it places item `is_on` into part `0` — and if the part index followed the
    /// item index there, flipping the switch would light a second part and leave
    /// both words on screen.
    ///
    /// `strong` falls back to the rest weight when the control shaped no
    /// emphasis run — a missing weight must render the words plainly, never
    /// render nothing.
    fn slot(&mut self, part: usize, item: usize, strong: bool) -> (&mut TextPart, Option<&TextLayout>) {
        if self.parts.len() <= part {
            self.parts.resize_with(part + 1, TextPart::default);
        }
        let run = strong
            .then(|| self.strong.get(item).and_then(Option::as_ref))
            .flatten()
            .or_else(|| self.layouts.get(item).and_then(Option::as_ref));
        (&mut self.parts[part], run)
    }

    /// Hide every part from `i` on — the items that no longer exist.
    fn hide_from(&mut self, i: usize) {
        for p in self.parts.iter_mut().skip(i) {
            p.hide_all();
        }
    }
}

/// A run shaped from a string at a size, reshaped only when either has moved.
///
/// Every other converted kind has its runs built by the layout pass, because
/// every other kind's font size is a property of its *style* — known before the
/// solve, and invalidated by `text_dirty`. A knob's four sizes are properties of
/// its **radius**, which is not known until the solve has run. So a knob shapes
/// at placement time instead, and this is what keeps that from meaning "reshape
/// on every paint": the inputs are compared, and a knob that neither resized nor
/// changed its words rebuilds nothing.
///
/// Comparing is what makes it exact rather than approximately right. Keying off
/// `text_dirty` instead would reshape a knob's whole tick set on every frame of
/// a value drag — the readout changes, the ticks do not — and would silently
/// miss any prop that fed a run without setting the flag.
///
/// Weight and family are not compared because no run varies them: each of the
/// knob's four is one fixed style at a derived size. A site that needed them to
/// vary would have to store them too, and should say so here.
#[derive(Default)]
pub(crate) struct Shaped {
    text: String,
    em: f32,
    layout: Option<TextLayout>,
    part: TextPart,
}

impl Shaped {
    /// The part and the run for `text` at `em`, reshaping first if either moved.
    ///
    /// Returns both borrowed together for the reason [`ItemText::slot`] does:
    /// the part is taken mutably and the run shared, out of one struct.
    pub(crate) fn pin(
        &mut self,
        text: &str,
        em: f32,
        weight: u16,
        family: &str,
    ) -> (&mut TextPart, Option<&TextLayout>) {
        // An empty run is not shaped at all — a knob with no unit must place
        // nothing, not an empty layout whose measure is a zero-width box.
        if self.em != em || self.text != text {
            self.em = em;
            self.text.clear();
            self.text.push_str(text);
            self.layout = (!text.is_empty())
                .then(|| super::layout::build_text_layout(text, em, weight, family, false))
                .flatten();
        }
        (&mut self.part, self.layout.as_ref())
    }

    /// Retire the run — the site no longer has words for this slot.
    ///
    /// The part is kept rather than dropped, so a set that grows back re-pins it
    /// instead of minting a new one. Clearing the cached string is what makes
    /// that safe: a slot that came back with the words it used to have would
    /// otherwise compare equal and place a layout it no longer holds.
    pub(crate) fn hide(&mut self) {
        self.text.clear();
        self.layout = None;
        self.part.hide_all();
    }
}

/// The knob dial's runs: its centre readout, the unit under it, its sub-line,
/// and one per tick label.
///
/// Every one is a [`Shaped`] rather than a bare layout, because all four sizes
/// are derived from the radius and the radius can move under any of them. The
/// three centre runs are named rather than indexed for the reason
/// [`BarText`]'s are: they are three different things with three different inks,
/// not three of one thing.
#[derive(Default)]
pub(crate) struct KnobText {
    readout: Shaped,
    unit: Shaped,
    sub: Shaped,
    /// One per tick label. Grown on demand and never shrunk — a knob's tick set
    /// is fixed in practice, and a hidden part costs nothing.
    ticks: Vec<Shaped>,
}

/// The runs one row of a popup menu owns: its leading icon, its label, and its
/// trailing shortcut hint.
pub(crate) const POPUP_RUNS_PER_ROW: usize = 3;

/// The runs a popup places: a text flyout's paragraph, or a menu's three per
/// row.
///
/// One holder for both bodies because a popup is one surface showing one or the
/// other, and because the body can be **replaced underneath it** — a suggestion
/// list refiltering as the user types — which has to retire the runs the old
/// body owned whichever kind it was.
#[derive(Default)]
pub(crate) struct PopupText {
    /// A text flyout's paragraph.
    ///
    /// A bare [`TextPart`] rather than a [`Shaped`], and deliberately: the run
    /// placed here is the very layout the panel was SIZED by. Re-shaping it from
    /// a cached string would allow the panel to be sized to one paragraph and
    /// have another placed in it.
    pub(crate) para: TextPart,
    /// A menu's runs, [`POPUP_RUNS_PER_ROW`] per row and **interleaved**, for
    /// the reason [`RowText`]'s are: one grow reaches a whole row and one retire
    /// takes one away, so a row count that drops cannot hide a departed row's
    /// label while leaving its icon lit.
    ///
    /// Indexed by the row's own index, separators included. A separator owns
    /// three runs it never places — three empty slots, against keeping this
    /// index in step with the one `hit`, `hovered` and the app's selection all
    /// speak, which is the index that must not be allowed to drift.
    rows: Vec<Shaped>,
}

impl PopupText {
    /// Row `i`'s three runs: icon, label, shortcut. Grown into existence if this
    /// is the first sync to reach that far.
    pub(crate) fn row(&mut self, i: usize) -> &mut [Shaped] {
        let end = (i + 1) * POPUP_RUNS_PER_ROW;
        if self.rows.len() < end {
            self.rows.resize_with(end, Shaped::default);
        }
        &mut self.rows[i * POPUP_RUNS_PER_ROW..end]
    }

    /// Retire every run from row `i` on — the rows the body no longer has.
    pub(crate) fn hide_rows_from(&mut self, i: usize) {
        for s in self.rows.iter_mut().skip(i * POPUP_RUNS_PER_ROW) {
            s.hide();
        }
    }

    /// Drop every sprite, because the container they are parented into is being
    /// replaced.
    ///
    /// Hiding them would not do: the parts would keep visuals belonging to a
    /// container that is about to be removed from the tree, and the next sync
    /// would place into it rather than into the live one — a menu that renders
    /// its panel and none of its words.
    pub(crate) fn orphan(&mut self) {
        self.para = TextPart::default();
        self.rows.clear();
    }
}

/// The runs a **select trigger** — a `ComboBox` or a `DropDownButton` — owns:
/// every label it could show, and its trailing chevron.
///
/// Not an [`ItemText`], despite holding a list, because a select shows exactly
/// ONE of its labels at a time. `ItemText` places item `i` into part `i`, which
/// is right for a segment bar and wrong here: a select needs one part that
/// re-places onto a different run as the selection moves, and a part per item
/// would light a second label the first time one was picked and leave both on
/// screen.
///
/// It holds every candidate rather than the current one because a select is
/// SIZED to the widest label it could ever show — so that picking an item never
/// reflows the row around it — and because `SelectedIndex` marks the node dirty
/// but not `text_dirty`, so the layout pass never re-runs on a selection and a
/// single reshaped slot would keep pointing at the old word. The same argument
/// [`toggle_sync`] makes for two labels, over N.
///
/// `labels` is positional: a `ComboBox`'s items in order, then its placeholder
/// last, so `selected_index` indexes it directly and the "nothing selected" case
/// is [`placeholder`](Self::placeholder). A `DropDownButton` has one entry, its
/// own content.
#[derive(Default)]
pub(crate) struct SelectText {
    /// Every label the trigger could show, shaped by the layout pass.
    pub(crate) labels: Vec<Option<TextLayout>>,
    /// The trailing chevron, shaped once from the icon face. It is not one of
    /// the labels — different family, different size, different ink — and
    /// keeping it out of `labels` also keeps it out of [`measurable`], where a
    /// glyph wider than every word would size the control to the ornament.
    ///
    /// [`measurable`]: Self::measurable
    pub(crate) chevron: Option<TextLayout>,
    label: TextPart,
    chev: TextPart,
}

impl SelectText {
    /// Every label the trigger could show, for the measure pass.
    pub(crate) fn measurable(&self) -> impl Iterator<Item = &TextLayout> {
        self.labels.iter().flatten()
    }

    /// Which entry of `labels` the current state names.
    ///
    /// `selected` is the node's `selected_index`, which is `-1` for "nothing
    /// selected" and may also out-run a `labels` rebuilt from a shorter item
    /// list — the two passes are invalidated by different props, so a selection
    /// can outlive the items it indexed. Both fall through to the placeholder —
    /// the last entry — rather than to nothing, so a stale index shows the
    /// empty-state words instead of an empty control.
    ///
    /// A `DropDownButton` has exactly one entry and no empty state, which this
    /// answers correctly without a special case: the placeholder IS entry 0.
    fn label_index(&self, selected: i32) -> usize {
        let placeholder = self.labels.len().saturating_sub(1);
        usize::try_from(selected)
            .ok()
            .filter(|i| *i < placeholder)
            .unwrap_or(placeholder)
    }

    /// The label part and the run [`label_index`](Self::label_index) names,
    /// borrowed together.
    ///
    /// One call rather than two accessors for the reason [`ItemText::slot`] is
    /// one: the part is taken mutably and the run shared, which is a split
    /// borrow every caller would otherwise have to spell out by hand.
    fn label_slot(&mut self, selected: i32) -> (&mut TextPart, Option<&TextLayout>) {
        let i = self.label_index(selected);
        (&mut self.label, self.labels.get(i).and_then(Option::as_ref))
    }

    /// The chevron part and its run, borrowed together — the same split.
    fn chevron_slot(&mut self) -> (&mut TextPart, Option<&TextLayout>) {
        (&mut self.chev, self.chevron.as_ref())
    }
}

/// The retained runs of a list whose rows are a **leading glyph plus a label**,
/// addressed by row index.
///
/// [`ItemText`] holds one run per item; this holds two, and they are not
/// interchangeable — the glyph is a different family at a different size, and
/// the two take different colours in the same row (an active row's icon goes
/// accent while its label goes primary). A control with this shape therefore
/// cannot express itself as an `ItemText` without shaping the two into one run,
/// which would tie their colours together.
///
/// The three vectors are positional — index `i` is row `i` — so a run that
/// failed to build holds its slot as `None` rather than shifting every row after
/// it onto the wrong words. `leading` is sparse in the same sense: a row with no
/// icon is `None` there and still owns its label.
///
/// The parts are stored **interleaved**, `[glyph 0, label 0, glyph 1, …]`,
/// rather than as two vectors. One `resize_with` grows a whole row and one
/// [`hide_from`](Self::hide_from) retires one, so a row count that drops can
/// never hide a departed row's label while leaving its glyph on screen — a
/// split that two vectors would make it possible to get half right.
#[derive(Default)]
pub(crate) struct RowText {
    /// One shaped leading glyph per row, or `None` where the row carries none.
    pub(crate) leading: Vec<Option<TextLayout>>,
    /// One shaped label per row.
    pub(crate) labels: Vec<Option<TextLayout>>,
    /// Two placed runs per row. Grown on demand and never shrunk — a row count
    /// that drops usually comes back, and a hidden part costs nothing.
    parts: Vec<TextPart>,
}

/// One row's two runs, each paired with the part that places it.
pub(crate) struct RowSlot<'a> {
    pub leading: (&'a mut TextPart, Option<&'a TextLayout>),
    pub label: (&'a mut TextPart, Option<&'a TextLayout>),
}

impl RowText {
    /// Replace the shaped runs, keeping every part.
    ///
    /// The layout pass rebuilds the runs whenever `text_dirty` is set, and the
    /// parts must not go with them: they own compositor visuals parented into
    /// the node, so dropping them would orphan the sprites already on screen and
    /// mint a second set beside them on the next sync.
    pub(crate) fn adopt(
        &mut self,
        leading: Vec<Option<TextLayout>>,
        labels: Vec<Option<TextLayout>>,
    ) {
        self.leading = leading;
        self.labels = labels;
    }

    /// How many rows have shaped runs.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.labels.len().max(self.leading.len())
    }

    /// Row `i`'s two parts and two runs, borrowed together.
    ///
    /// One call rather than four accessors for the reason [`ItemText::slot`] is
    /// one: the parts are taken mutably and the runs shared, out of fields the
    /// borrow checker will only let a caller split by hand. The row's parts are
    /// grown into existence if this is the first sync to reach that far.
    pub(crate) fn row(&mut self, i: usize) -> RowSlot<'_> {
        let end = 2 * i + 2;
        if self.parts.len() < end {
            self.parts.resize_with(end, TextPart::default);
        }
        // Disjoint borrows: the two parts come from one slice split in half, and
        // the two runs from fields the slice does not touch.
        let (glyph, label) = self.parts[2 * i..end].split_at_mut(1);
        RowSlot {
            leading: (&mut glyph[0], self.leading.get(i).and_then(Option::as_ref)),
            label: (&mut label[0], self.labels.get(i).and_then(Option::as_ref)),
        }
    }

    /// Hide every part from row `i` on — the rows that no longer exist.
    pub(crate) fn hide_from(&mut self, i: usize) {
        for p in self.parts.iter_mut().skip(2 * i) {
            p.hide_all();
        }
    }
}

/// Where a run sits inside the box it is given.
///
/// Alignment is an origin rather than a text format because a shaped run carries
/// none of its own once it is placed by hand: `TextAlignment` and
/// `ParagraphAlignment` are instructions to the drawing call this path no longer
/// makes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Align {
    /// Top-left, unmeasured — for text whose own layout already decides where
    /// each line goes: wrapped prose, and anything leading-aligned. Wrapping
    /// needs nothing beyond this: DirectWrite gives one run per line at its own
    /// descending baseline and [`TextPart::sync`] honours each run's origin.
    Leading,
    /// Leading edge, centred on the vertical axis — a label beside something.
    LeadingCentered,
    /// Centred both ways.
    Centered,
    /// Centred horizontally, at the TOP of the box — a run stacked under
    /// another, whose box is a column it hangs from rather than one it sits in
    /// the middle of. The knob's unit and sub-line, which the painted dial
    /// expressed as `ParagraphAlignment::Top`.
    TopCentered,
    /// Trailing edge, centred on the vertical axis — the mirror of
    /// [`LeadingCentered`](Self::LeadingCentered), for a run that shares its box
    /// with one and is kept apart from it by alignment alone. A menu row's
    /// shortcut hint, which sits in the label's own column.
    TrailingCentered,
}

/// Everything a placement needs that the control does not choose.
///
/// The four fields were positional arguments on every `place_*` call, which put
/// ten parameters on each and left the choice of alignment encoded in the
/// function's *name*. Carrying them together makes a placement one line and its
/// alignment a value — which is what lets the per-kind sync functions live
/// beside their own geometry instead of here.
pub(crate) struct Pen<'a> {
    pub(crate) comp: &'a Compositing,
    pub(crate) atlas: &'a mut GlyphAtlas,
    /// The node's container; every run's host parents into it.
    ///
    /// Held **owned** rather than borrowed, which is what lets a run be placed
    /// straight out of the node's own field. Borrowing it froze the node, so
    /// every sync had to lift its text out with `take()` and put it back at the
    /// end — a dance that also made an early `return` in between silently orphan
    /// the sprites already on screen. `ContainerVisual` is refcounted, so the
    /// clone is an `AddRef` and the hazard is gone by construction.
    pub(crate) host: ContainerVisual,
    /// The node's disabled dim, carried on each host's opacity.
    pub(crate) dim: f32,
    pub(crate) scale: f32,
}

impl<'a> Pen<'a> {
    /// A pen over `node`, taking its enabled state as the dim every run carries.
    pub(crate) fn new(
        comp: &'a Compositing,
        atlas: &'a mut GlyphAtlas,
        node: &Node,
        scale: f32,
    ) -> Self {
        let dim = if node.paint.is_enabled { 1.0 } else { theme::disabled_opacity() };
        Self::over(comp, atlas, node.container.clone(), dim, scale)
    }

    /// A pen over an arbitrary host visual.
    ///
    /// The node-free half of [`new`](Self::new), for the one surface in the
    /// backend that is not a node: a popup is a visual promoted to the
    /// compositor root, so it never reaches the paint walk and has no `Node` to
    /// take a container and a dim from. Nothing else about placement changes —
    /// which is the point of `Pen` holding its host owned rather than borrowing
    /// a node.
    pub(crate) fn over(
        comp: &'a Compositing,
        atlas: &'a mut GlyphAtlas,
        host: ContainerVisual,
        dim: f32,
        scale: f32,
    ) -> Self {
        Self { comp, atlas, host, dim, scale }
    }
}

impl Pen<'_> {
    /// Place `run` in `b`, or hide `part` if there is nothing to place.
    ///
    /// Absent and unmeasurable are the same outcome deliberately: a run that
    /// cannot be measured cannot be aligned, and a half-placed label is worse
    /// than an absent one.
    pub(crate) fn place(
        &mut self,
        part: &mut TextPart,
        run: Option<&TextLayout>,
        b: Rect,
        a: Align,
        color: crate::Color,
    ) {
        self.place_in(part, run, b, b, a, color);
    }

    /// [`place`](Self::place), with the host's clip box given separately.
    ///
    /// The two differ only where a run is placed against one rectangle but must
    /// be *cut* to another — a title measured against its own block but clipped
    /// to the column it shares.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn place_in(
        &mut self,
        part: &mut TextPart,
        run: Option<&TextLayout>,
        b: Rect,
        host_box: Rect,
        a: Align,
        color: crate::Color,
    ) {
        let Some(layout) = run else {
            part.hide_all();
            return;
        };
        // Leading takes no measurement, so a run that cannot report its extent
        // still places — the alignment does not depend on knowing it.
        let origin = if a == Align::Leading { (b.left, b.top) } else {
            let Ok((tw, th)) = layout.measure() else {
                part.hide_all();
                return;
            };
            let x = match a {
                // Clamped at the leading edge so a run too wide for its box
                // loses its tail — which the host's clip hides — not its
                // head.
                Align::Centered | Align::TopCentered => {
                    b.left + ((b.width() - tw) / 2.0).max(0.0)
                }
                // Clamped the same way, and for the same reason: a hint
                // wider than its row must not walk out of the leading edge.
                Align::TrailingCentered => b.left + (b.width() - tw).max(0.0),
                _ => b.left,
            };
            let y = match a {
                Align::TopCentered => b.top,
                _ => b.top + ((b.height() - th) / 2.0).max(0.0),
            };
            (x, y)
        };
        part.sync(
            self.comp,
            self.atlas,
            &self.host,
            layout,
            origin,
            host_box,
            color,
            self.dim,
            self.scale,
        );
    }

    /// Place a run at `(x, y)` after re-pinning its max width to `max_w`.
    ///
    /// Re-pinning is what arms `CharacterEllipsis`: the layout is built at its
    /// natural width and only ellipsizes once narrowed. Skipping it renders a
    /// title at full length over the window buttons.
    pub(crate) fn place_trimmed(
        &mut self,
        part: &mut TextPart,
        run: Option<&TextLayout>,
        at: Option<(f32, f32)>,
        y: f32,
        line_h: f32,
        color: crate::Color,
    ) {
        let Some((layout, (x, max_w))) = run.zip(at) else {
            part.hide_all();
            return;
        };
        let _ = layout.set_max_width(max_w);
        let b = Rect::from_xywh(x, y, max_w, line_h);
        self.place_in(part, Some(layout), b, b, Align::Leading, color);
    }
}

/// Where a LIVE run sits inside its (oversized) box, from the block's requested
/// `HorizontalAlignment` — 0 Left, 1 Center, 2 Right, anything else leading.
///
/// A live readout's box is sized for the widest value it can ever hold, so a
/// shorter value leaves slack. Centred is what a column of numbers under a
/// heading wants; trailing is what a right-aligned readout wants, and is the one
/// that keeps a decimal point still while the digits before it change. Leading
/// would let both wander.
///
/// The centred variants also centre VERTICALLY, where leading pins the top. A
/// live box is sized to one line — it holds a readout, not prose — so the two
/// coincide and only the horizontal choice is doing any work here.
fn live_text_align(h_align: i32) -> Align {
    match h_align {
        1 => Align::Centered,
        2 => Align::TrailingCentered,
        _ => Align::Leading,
    }
}

/// Reconcile a `TextBlock`'s prose as retained glyph sprites.
///
/// The counterpart to [`button_sync`] for the one control that is nothing but
/// text. It places at the node's top-left rather than centring, because a
/// TextBlock's own layout — its alignment, and its wrapping — has already
/// decided where every line goes; centring the block here would fight it.
#[allow(
    clippy::single_match_else,
    reason = "the live and ordinary paths are symmetric, and each arm's comment \
              belongs to its own case"
)]
pub(crate) fn text_sync(comp: &Compositing, atlas: &mut GlyphAtlas, node: &mut Node, scale: f32) {
    if node.kind != crate::backend::ControlKind::TextBlock {
        return;
    }
    let b = Rect::from_xywh(0.0, 0.0, node.rect.w, node.rect.h);
    // Un-styled text takes the themable primary text token — never a literal —
    // so a host token table restyles default text too.
    let fg = node.paint.foreground.unwrap_or_else(theme::text);

    // Read the type before anything borrows a run out of the node: the family is
    // owned by `paint` and the run by `live_run`, and shaping needs both.
    let live = node.live_words.clone();
    let em = node.paint.font_size;
    let weight = node.paint.font_weight;
    let family = node
        .paint
        .font_family
        .clone()
        .unwrap_or_else(|| "Segoe UI".to_string());
    // Only a LIVE run needs this. A rendered block's box is measured from its own
    // words, so it has no slack to align within and leading is already right; a
    // live block's box is deliberately oversized — it has to hold the widest value
    // its producer can send — so where the run sits inside that slack is a real
    // choice, and it is the one the block asked for.
    let live_align = live_text_align(node.h_align);

    let mut pen = Pen::new(comp, atlas, node, scale);
    pen.dim = 1.0; // A TextBlock is not interactive; it never dims.

    // A block has two possible sources of words and exactly one may be on screen.
    // They own SEPARATE sprites, so the source that is not chosen has to be
    // retired rather than merely skipped: left standing, its glyphs stay visible
    // under the ones placed here and the two readings interleave into a number
    // that was never published. Every block takes this path — the first live
    // value on a block that mounted with text is precisely the crossing.
    match live {
        // Words a producer thread published: shaped HERE, at placement, because
        // the value never passed through a reconcile and so the layout pass
        // never ran on it. `Shaped::pin` compares the string and the em first,
        // so a republished-unchanged value reshapes nothing.
        Some(words) => {
            if let Some(part) = node.text_part.as_mut() {
                part.hide_all();
            }
            let run = node.live_run.get_or_insert_default();
            let (part, layout) = run.pin(&words, em, weight, &family);
            // A live value cannot resize its own box: the layout pass measured
            // this node from the words it was RENDERED with, and a value that
            // arrives without a render never re-runs it. The host's `InsetClip`
            // then cuts anything wider at the edge — silently, and for a numeric
            // readout that is worse than a visible failure, because `-14.2`
            // clipped to `-14` still reads as a real number. Loud in dev, so the
            // box gets sized for the widest value the producer can send (see
            // `newapo_viz::draw::measure::widest`) rather than for whatever it
            // happened to mount with.
            debug_assert!(
                layout
                    .and_then(|l| l.measure().ok())
                    .is_none_or(|(w, _)| w <= b.width() + 0.5),
                "live text {words:?} is wider than its box ({:.1} DIP) on {:?}; \
                 size the node for the widest value it can hold — a clipped \
                 readout shows a plausible wrong number",
                b.width(),
                node.kind,
            );
            pen.place(part, layout, b, live_align, fg);
        }
        // The ordinary path: the run the layout pass shaped, placed through the
        // node's own part.
        None => {
            if let Some(run) = node.live_run.as_mut() {
                run.hide();
            }
            let part = node.text_part.get_or_insert_default();
            pen.place(part, node.text_layout.as_ref(), b, Align::Leading, fg);
        }
    }
}

/// Reconcile a `HyperlinkButton`'s words as retained glyph sprites.
///
/// Leading horizontally, centred vertically — the alignment the painted link
/// used, expressed as an origin rather than as a text format, because a shaped
/// run carries no alignment of its own once it is placed by hand.
///
/// The hover recolour that used to be a repaint is now a `SetSource` on the
/// shared colour brush: the glyph masks are colourless, so switching a link
/// from accent to accent-light re-rasterizes nothing.
pub(crate) fn hyperlink_sync(
    comp: &Compositing,
    atlas: &mut GlyphAtlas,
    node: &mut Node,
    scale: f32,
) {
    if node.kind != crate::backend::ControlKind::HyperlinkButton {
        return;
    }
    let b = Rect::from_xywh(0.0, 0.0, node.rect.w, node.rect.h);
    let ink = if node.hovered { theme::accent_light() } else { theme::accent() };
    let mut pen = Pen::new(comp, atlas, node, scale);
    let part = node.text_part.get_or_insert_default();
    pen.place(part, node.text_layout.as_ref(), b, Align::LeadingCentered, ink);
}

/// Reconcile an `Expander`'s header label and chevron as retained glyph sprites.
///
/// The chevron reads out of [`ItemText`] for the reason a ToggleSwitch's label
/// does: `layouts` is `[collapsed, expanded]` and the state indexes it, so
/// expanding is a re-place of ONE part rather than a reshape. It has to be —
/// `IsExpanded` marks the node dirty but not `text_dirty`, so the layout pass
/// never re-runs on it and a single reshaped slot would keep pointing the old
/// way.
pub(crate) fn expander_sync(
    comp: &Compositing,
    atlas: &mut GlyphAtlas,
    node: &mut Node,
    scale: f32,
) {
    if node.kind != crate::backend::ControlKind::Expander {
        return;
    }
    let ink = node.paint.foreground.unwrap_or_else(theme::text);
    let label_box = super::controls::expander_label_box(node);
    let chev_box = super::controls::expander_chevron_box(node);
    let i = usize::from(node.ctrl().expanded);
    let mut pen = Pen::new(comp, atlas, node, scale);

    // The header label lives in the generic single-run slot — the Expander has
    // one run of its own, and the chevron is not one of its words.
    let label = node.text_part.get_or_insert_default();
    pen.place(label, node.text_layout.as_ref(), label_box, Align::LeadingCentered, ink);

    let (chev, run) = node.item_text.get_or_insert_default().slot(0, i, false);
    pen.place(chev, run, chev_box, Align::Centered, theme::text_secondary());
}

/// Reconcile a `CheckBox`'s trailing label as retained glyph sprites.
///
/// Leading horizontally, centred vertically — the alignment the painted label
/// had, expressed as an origin because a shaped run carries no alignment of its
/// own once it is placed by hand.
pub(crate) fn check_sync(comp: &Compositing, atlas: &mut GlyphAtlas, node: &mut Node, scale: f32) {
    if node.kind != crate::backend::ControlKind::CheckBox {
        return;
    }
    let ink = node.paint.foreground.unwrap_or_else(theme::text);
    let b = super::controls::check_label_box(node);
    let mut pen = Pen::new(comp, atlas, node, scale);
    let part = node.text_part.get_or_insert_default();
    pen.place(part, node.text_layout.as_ref(), b, Align::LeadingCentered, ink);
}

/// Reconcile a `Knob`'s dial text as retained glyph sprites.
///
/// The one converted kind that shapes its own runs. Every other kind's font
/// size is a property of its style, so the layout pass can build the runs before
/// the solve; a knob's four sizes are derived from its **radius**, which the
/// solve is what decides. Shaping here is not a shortcut around the text pass —
/// it is the only point at which the size is known.
///
/// [`Shaped`] is what keeps that honest: each run carries the string and em it
/// was built from, so a knob that neither resized nor changed its words reshapes
/// nothing, and a value drag reshapes the readout alone rather than the whole
/// tick set.
///
/// The dial itself — track, ticks, hub — stays painted, and the value arc and
/// needle stay retained vector chrome (`knob::sync_knob`). Only the words move.
pub(crate) fn knob_sync(comp: &Compositing, atlas: &mut GlyphAtlas, node: &mut Node, scale: f32) {
    if node.kind != crate::backend::ControlKind::Knob {
        return;
    }
    // Read the geometry before the mutable borrow below: it needs the node, and
    // it fits the radius to the tick labels, so it cannot be re-derived after.
    let (cx, cy, radius) = super::knob::dial_geom(node);
    let readout_em = super::knob::readout_em(radius);
    let (readout_box, unit_box, sub_box) = (
        super::knob::readout_box(cx, cy, radius),
        super::knob::unit_box(cx, cy, radius),
        super::knob::sub_box(cx, cy, radius),
    );

    let mut pen = Pen::new(comp, atlas, node, scale);
    let (ctrl, readout, t) = node.knob_runs();

    let (part, run) = t.readout.pin(readout, readout_em, KNOB_READOUT_WEIGHT, KNOB_FACE);
    pen.place(part, run, readout_box, Align::Centered, theme::w(0.9));

    let (part, run) = t.unit.pin(&ctrl.unit, super::knob::unit_em(readout_em), 400, KNOB_FACE);
    pen.place(part, run, unit_box, Align::TopCentered, theme::w(0.35));

    let (part, run) = t.sub.pin(&ctrl.sub_text, super::knob::sub_em(radius), 400, KNOB_FACE);
    pen.place(part, run, sub_box, Align::TopCentered, theme::w(0.25));

    // The outer numeric labels. Each sits at its own value's angle, which is
    // the same mapping the ticks, the arc and the drag all read.
    let tick_em = super::knob::tick_em(radius);
    if t.ticks.len() < ctrl.tick_labels.len() {
        t.ticks.resize_with(ctrl.tick_labels.len(), Shaped::default);
    }
    for (slot, (v, label)) in t.ticks.iter_mut().zip(ctrl.tick_labels.iter()) {
        let a = super::knob::value_to_angle(*v, ctrl.min, ctrl.max, ctrl.start_angle, ctrl.end_angle);
        let b = super::knob::tick_label_box(cx, cy, radius, a);
        let (part, run) = slot.pin(label, tick_em, 400, KNOB_FACE);
        pen.place(part, run, b, Align::Centered, theme::text_tertiary());
    }
    // Labels the knob no longer has.
    for slot in t.ticks.iter_mut().skip(ctrl.tick_labels.len()) {
        slot.hide();
    }
}

/// The face every one of the dial's runs is set in, and the readout's weight —
/// thin, because the readout is large and a large run at book weight reads as
/// shouting.
const KNOB_FACE: &str = "Segoe UI";
const KNOB_READOUT_WEIGHT: u16 = 200;

/// Reconcile a select trigger's label and chevron as retained glyph sprites.
///
/// The label is read out of [`SelectText`] rather than the node's own
/// `text_layout` for the reason a ToggleSwitch's is: the slot the measure pass
/// wants holds the WIDEST candidate, and the slot the draw wants holds the
/// CURRENT one. They are the same run only by coincidence.
///
/// The trigger's box fill and border are retained parts (`parts::select_plan`),
/// so these sprites land above parts rather than above a painted surface — the
/// trigger owns none.
///
/// Each run's host is its own column, which is what stops a selected item too
/// long for the trigger from running out under the chevron. The label loses its
/// tail to the clip instead.
pub(crate) fn select_sync(comp: &Compositing, atlas: &mut GlyphAtlas, node: &mut Node, scale: f32) {
    if !matches!(
        node.kind,
        crate::backend::ControlKind::ComboBox | crate::backend::ControlKind::DropDownButton
    ) {
        return;
    }
    let label_box = super::controls::select_label_box(node);
    let chev_box = super::controls::select_chevron_box(node);
    // A ComboBox showing its placeholder inks it as tertiary — the empty state
    // must not read as a chosen value. A DropDownButton has no empty state; its
    // content is always its own.
    let placeholder = node.kind == crate::backend::ControlKind::ComboBox
        && node.ctrl().selected_index < 0;
    let ink = if placeholder {
        theme::text_tertiary()
    } else {
        theme::text()
    };
    let sel = node.ctrl().selected_index;

    let mut pen = Pen::new(comp, atlas, node, scale);
    let t = node.select_text.get_or_insert_default();
    let (part, run) = t.label_slot(sel);
    pen.place(part, run, label_box, Align::LeadingCentered, ink);
    let (part, run) = t.chevron_slot();
    pen.place(part, run, chev_box, Align::Centered, theme::text_secondary());
}

/// The caption band's six runs: two titles, and one glyph per button.
///
/// One part per BUTTON, not per glyph slot — maximize and restore share button
/// 1's part, exactly as a ToggleSwitch's two labels share one. A part per slot
/// would light both the moment a window was maximized and leave the other glyph
/// on screen underneath it.
#[derive(Default)]
pub(crate) struct CaptionGlyphs {
    title: TextPart,
    subtitle: TextPart,
    /// Index 0 is the leading back button; 1..4 are the window cluster.
    buttons: [TextPart; 4],
}

/// Reconcile the caption band's text as retained glyph sprites.
///
/// The two titles come from one [`caption::title_placement`], so the coupling
/// between them — the subtitle starts after whatever width the title was clamped
/// to — is resolved once, as a value, before a glyph is placed.
///
/// ## Z-order against the band's slot children
///
/// A TitleBar is the one converted kind with real children (the app's `Content`
/// and trailing header elements), and a glyph host inserts at the TOP of the
/// node's children — so the caption's own text now sits ABOVE them, where the
/// painted version sat below on the node's surface. That is safe because they do
/// not overlap by construction: `caption::title_block` reserves the measured
/// title block as leading inset precisely so the app's content begins after it.
pub(crate) fn caption_sync(
    comp: &Compositing,
    atlas: &mut GlyphAtlas,
    node: &mut Node,
    scale: f32,
    content_left: Option<f32>,
) {
    if node.kind != crate::backend::ControlKind::TitleBar {
        return;
    }
    let band = Rect::from_xywh(0.0, 0.0, node.rect.w, node.rect.h);
    let back_w = super::caption::back_width(node.extras());
    let back_enabled = node.extras().back_button_enabled;
    let back_box = super::caption::back_rect(node.extras(), band);
    let maximized = super::caption::maximized();

    let mut pen = Pen::new(comp, atlas, node, scale);
    // The band's ink never dims as a whole; a disabled back arrow greys itself.
    pen.dim = 1.0;
    let Some(t) = node.caption_text.as_ref() else {
        return;
    };
    let g = node.caption_glyphs.get_or_insert_default();

    // The two titles, placed from one resolved coupling.
    if let Some(p) = super::caption::title_placement(t, back_w, band, content_left) {
        let h = t.line_h;
        pen.place_trimmed(&mut g.title, t.title.as_ref(), p.title, p.y, h, theme::text());
        let ink = theme::text_secondary();
        pen.place_trimmed(&mut g.subtitle, t.subtitle.as_ref(), p.subtitle, p.y, h, ink);
    } else {
        g.title.hide_all();
        g.subtitle.hide_all();
    }

    // The back chevron, then the window cluster. Disabled greys the arrow
    // rather than hiding it, exactly as WinUI does — hiding it would reflow the
    // whole band every time navigation depth hit zero.
    let back_ink = if back_enabled { theme::text() } else { theme::text_disabled() };
    let back = t.glyphs[super::caption::glyph_slot::BACK].as_ref();
    match back_box {
        Some(r) => pen.place(&mut g.buttons[0], back, r, Align::Centered, back_ink),
        None => g.buttons[0].hide_all(),
    }
    for i in 0..3 {
        let run = t.glyphs[super::caption::window_glyph_slot(i, maximized)].as_ref();
        let r = super::caption::button_rect(i, band);
        pen.place(&mut g.buttons[1 + i as usize], run, r, Align::Centered, theme::text());
    }
}

/// The three runs an `InfoBar` places: its severity glyph, its paragraph, and
/// its close glyph.
///
/// Named rather than indexed, like [`ButtonText`] and unlike [`ItemText`]:
/// these are three different things, not three of one thing, and each has its
/// own box and its own ink. The shaped layouts live in
/// [`info_bar::InfoBarText`](super::info_bar::InfoBarText) — the module that
/// builds and re-pins them — so this holds only their sprites.
#[derive(Default)]
pub(crate) struct BarText {
    icon: TextPart,
    para: TextPart,
    close: TextPart,
}

/// Reconcile an `InfoBar`'s text as retained glyph sprites.
///
/// The paragraph is the one run in the library that WRAPS, so it is also the
/// only one whose layout is stateful: `info_bar::measure` leaves it flowed at
/// whatever width Taffy last probed. It is re-pinned through
/// [`InfoBarText::pinned`](super::info_bar::InfoBarText::pinned) before a single
/// glyph is placed, which is the same thing the painted path did on every
/// repaint and the reason a sprite placement cannot simply read the run.
///
/// Its host box is the text column, so a paragraph that overflows a band too
/// short for it loses the overflow to the clip rather than spilling across the
/// close button.
pub(crate) fn info_bar_sync(
    comp: &Compositing,
    atlas: &mut GlyphAtlas,
    node: &mut Node,
    scale: f32,
) {
    if node.kind != crate::backend::ControlKind::InfoBar {
        return;
    }
    let (w, h) = (node.rect.w, node.rect.h);
    let closable = node.extras().bar_closable;
    let sev_color = super::info_bar::severity(node.extras()).color();
    let ink = node.paint.foreground.unwrap_or_else(theme::text);

    let mut pen = Pen::new(comp, atlas, node, scale);
    let Some(t) = node.bar_text.as_ref() else {
        return;
    };
    let g = node.bar_glyphs.get_or_insert_default();

    // The severity glyph, centred in its column.
    let cell = super::info_bar::icon_cell(h);
    pen.place(&mut g.icon, t.icon.as_ref(), cell, Align::Centered, sev_color);

    // The paragraph, re-pinned to the column it is about to be placed in.
    match t.pinned(w, closable) {
        Some((run, th)) => {
            let b = super::info_bar::text_box(w, h, th, closable);
            pen.place(&mut g.para, Some(run), b, Align::Leading, ink);
        }
        None => g.para.hide_all(),
    }

    // The close glyph, centred in the button box the hit test uses.
    match super::info_bar::close_rect(w, h, closable) {
        Some(r) => {
            let run = t.close.as_ref();
            pen.place(&mut g.close, run, r, Align::Centered, theme::text_secondary());
        }
        None => g.close.hide_all(),
    }
}

/// Reconcile an `InfoBadge`'s count as retained glyph sprites.
///
/// Centred in the node rather than in the plate, which is what the painted
/// version did and what the numeric form makes correct anyway: for a pill the
/// plate IS the node, and the dot form carries no count to place.
pub(crate) fn info_badge_sync(
    comp: &Compositing,
    atlas: &mut GlyphAtlas,
    node: &mut Node,
    scale: f32,
) {
    if node.kind != crate::backend::ControlKind::InfoBadge {
        return;
    }
    let b = Rect::from_xywh(0.0, 0.0, node.rect.w, node.rect.h);
    // The count sits ON the fill, so its default is the on-accent ink rather
    // than the body-text token — which is near-invisible against a light-theme
    // accent.
    //
    // An explicit `Foreground` wins, and has to: the ink that reads on a badge
    // is a property of the FILL, and the fill is app-supplied (`Background`). A
    // host colouring badges by meaning — a per-band accent, a danger count —
    // picks the fill and therefore owns the contrast decision; deriving the ink
    // from the theme's accent would be answering a question about a colour the
    // theme never saw.
    let ink = node.paint.foreground.unwrap_or_else(theme::text_on_accent);
    let mut pen = Pen::new(comp, atlas, node, scale);
    let part = node.text_part.get_or_insert_default();
    pen.place(part, node.text_layout.as_ref(), b, Align::Centered, ink);
}

/// Reconcile a `ToggleSwitch`'s state label as retained glyph sprites.
///
/// One run, placed after the track — but read out of [`ItemText`] rather than
/// the node's own `text_layout`, because the two labels are not
/// interchangeable. The switch is SIZED to the wider of "On" and "Off" so
/// flipping it never reflows the row around it, and it is DRAWN with whichever
/// one the state currently names. A single cached layout can answer one of those
/// questions or the other, and answering the sizing one is what it was there
/// for; placing sprites from it would have rendered the wrong word whenever the
/// two labels differed in width.
///
/// The host is the region right of the track, so a label wider than the room
/// left for it loses its tail to the clip rather than overrunning the control.
pub(crate) fn toggle_sync(comp: &Compositing, atlas: &mut GlyphAtlas, node: &mut Node, scale: f32) {
    if node.kind != crate::backend::ControlKind::ToggleSwitch {
        return;
    }
    let fg = node.paint.foreground.unwrap_or_else(theme::text);
    let x0 = super::parts::TRACK_W + super::controls::TOGGLE_LABEL_GAP;
    let b = Rect::from_xywh(x0, 0.0, (node.rect.w - x0).max(0.0), node.rect.h);
    // Index, not a stored string: `layouts` is `[off, on]`, so the state picks
    // the run and the same single part re-places on every flip.
    let i = usize::from(node.ctrl().is_on);
    let mut pen = Pen::new(comp, atlas, node, scale);
    let (part, run) = node.item_text.get_or_insert_default().slot(0, i, false);
    pen.place(part, run, b, Align::LeadingCentered, fg);
}

/// Reconcile a `SelectorBar`'s segment labels as retained glyph sprites.
///
/// N segments are N independent runs — each with its own host, its own colour
/// and its own centred origin inside its segment — so this is the first site to
/// need [`ItemText`]'s per-item parts rather than a single [`TextPart`].
///
/// The selected segment sets at 600 and the rest at 400, which is a different
/// SHAPED run and not a property that can be switched at placement time. Both
/// weights are therefore shaped up front and picked between here; see
/// [`ItemText::strong`] for why selection must not be allowed to trigger a
/// rebuild.
///
/// Each label's host is its own segment rect, which is what keeps a label too
/// wide for its share of the tray from bleeding into its neighbour.
pub(crate) fn segmented_sync(
    comp: &Compositing,
    atlas: &mut GlyphAtlas,
    node: &mut Node,
    scale: f32,
) {
    if node.kind != crate::backend::ControlKind::SelectorBar {
        return;
    }
    let n = node.ctrl().items.len();
    // The same geometry paint, hit-testing and UIA item rects all read, so a
    // label cannot land anywhere but on the segment the pointer will report.
    let m = super::controls::seg_metrics(node.paint.style_variant, node.paint.font_size);
    let edges = super::controls::segment_edges(node);
    let pill_h = (node.rect.h - 2.0 * m.tray).max(0.0);
    let sel = node.ctrl().selected_index;
    let hot = node.ctrl().hot_index;
    let hovered = node.paint.is_enabled && node.hovered;

    let mut pen = Pen::new(comp, atlas, node, scale);
    let t = node.item_text.get_or_insert_default();
    for i in 0..n {
        let Some((&a, &b)) = edges.get(i).zip(edges.get(i + 1)) else {
            break;
        };
        let active = i as i32 == sel;
        // A recolour is a `SetSource` on the run's shared colour brush, so
        // hovering a segment re-rasterizes no glyph — it was a full repaint of
        // the bar's surface when these labels were painted.
        let color = if active {
            theme::text()
        } else if hovered && i as i32 == hot {
            theme::text_secondary()
        } else {
            theme::text_tertiary()
        };
        let seg = Rect::from_xywh(a, m.tray, b - a, pill_h);
        let (part, run) = t.slot(i, i, active);
        pen.place(part, run, seg, Align::Centered, color);
    }
    // Items can go away: a bar rebuilt with fewer segments must not leave the
    // departed ones' words on screen.
    t.hide_from(n);
}

/// Reconcile a NavigationView pane's every run as retained sprites: its two
/// chrome glyphs, its header, and a leading glyph plus a label per row.
///
/// The pane owns no surface, so this and [`parts::nav_plan`](super::parts) are
/// its entire appearance. Everything geometric is read from
/// [`nav`](super::nav) — the same [`Metrics`](super::nav::Metrics) the hit test
/// and the accessibility tree resolve — so a label cannot land on a row the
/// pointer will report as a different one.
///
/// Ellipsization comes for free and is not a special case: the runs were shaped
/// with a trimming sign and no width, and narrowing one to the room the current
/// pane leaves makes DirectWrite re-walk it on this very sync. That is why a
/// label degrades to "Equalizer…" as the pane closes without anything here
/// deciding that it should.
pub(crate) fn nav_sync(comp: &Compositing, atlas: &mut GlyphAtlas, node: &mut Node, scale: f32) {
    if node.kind != crate::backend::ControlKind::NavigationView {
        return;
    }
    use super::nav::ChromeRun;
    let (w, h) = (node.rect.w, node.rect.h);
    let count = node.ctrl().items.len();
    let sel = node.ctrl().selected_index;
    let back_enabled = node.extras().back_enabled;
    let has_title = node.nav_text.as_ref().is_some_and(|t| t.title.is_some());
    // Resolved before the borrow below: `extras()` takes the whole node.
    let m = super::nav::metrics(node.extras(), w, has_title);

    let mut pen = Pen::new(comp, atlas, node, scale);
    let Some(t) = node.nav_text.as_mut() else {
        return;
    };

    // ── The pane's own three runs ────────────────────────────────────────────
    // A disabled back arrow is still SHOWN, greyed — hiding it on disable would
    // reflow the pane every time the navigation stack hit depth zero. The grey
    // is the run's colour and not the host's dim, which carries the whole
    // control's enablement and would grey the hamburger beside it too.
    let ink = if back_enabled { theme::text() } else { theme::text_disabled() };
    let (part, run) = t.chrome_slot(ChromeRun::Back);
    match super::nav::back_rect(&m) {
        Some(b) => pen.place(part, run, b, Align::Centered, ink),
        None => part.hide_all(),
    }
    let (part, run) = t.chrome_slot(ChromeRun::Toggle);
    match super::nav::toggle_rect(&m) {
        Some(b) => pen.place(part, run, b, Align::Centered, theme::text()),
        None => part.hide_all(),
    }
    // The header is clamped to its own natural width before the pane's, so a
    // short title stays short: `set_max_width` is a wrap/trim bound, and handing
    // it the whole column would place an ellipsis budget the title never needs.
    let title_w = t.title_w;
    let (part, run) = t.chrome_slot(ChromeRun::Title);
    match super::nav::title_box(&m) {
        Some(b) => {
            if let Some(l) = run {
                let _ = l.set_max_width(title_w.min(b.width()));
            }
            pen.place(part, run, b, Align::LeadingCentered, theme::text_secondary());
        }
        None => part.hide_all(),
    }

    // ── The rows ─────────────────────────────────────────────────────────────
    let n = super::nav::visible_items(&m, h, count);
    let labels = m.kind.expanded();
    for i in 0..n {
        let row = super::nav::item_rect(&m, i as i32);
        nav_row(&mut pen, &mut t.rows, i, row, &m, i as i32 == sel, labels);
    }
    // Every row past the visible ones — a pane too short for its whole menu must
    // not leave the surplus rows' words floating over the settings row, and a
    // menu rebuilt with fewer items must not leave the departed ones at all.
    // This sweeps the settings row's slot too; the placement below restores it.
    t.rows.hide_from(n);

    // The settings row is row `count`, not row `n`: its index follows the ITEM
    // COUNT so that a pane shrinking its visible window never re-points the
    // settings sprites at a menu item's runs.
    if let Some(row) = super::nav::settings_rect(&m, h) {
        let active = sel == super::nav::SETTINGS_INDEX;
        nav_row(&mut pen, &mut t.rows, count, row, &m, active, labels);
    }
}

/// Place one pane row: its leading glyph in the rail column, and — in an
/// expanded pane — its label in the column beside it.
///
/// The two runs take DIFFERENT colours in the same row on purpose, and it is why
/// a row is two runs rather than one shaped string: an active row's glyph goes
/// accent while its words go primary, which is WinUI's own selected-item
/// treatment and cannot be expressed by one run with one colour source.
fn nav_row(
    pen: &mut Pen,
    rows: &mut RowText,
    i: usize,
    row: Rect,
    m: &super::nav::Metrics,
    active: bool,
    labels: bool,
) {
    let slot = rows.row(i);
    let cell = super::nav::icon_cell(row);
    let glyph_ink = if active { theme::accent() } else { theme::text_tertiary() };
    let (part, run) = slot.leading;
    pen.place(part, run, cell, Align::Centered, glyph_ink);

    let b = super::nav::label_box(m, row);
    let (part, run) = slot.label;
    match run.filter(|_| labels && b.width() > 0.0) {
        Some(layout) => {
            // Narrow the run to the room this pane width leaves, which is what
            // makes labels ellipsize as the pane closes. `sync` re-walks the
            // layout below, so the reshape lands in this same pass.
            let _ = layout.set_max_width(b.width());
            let ink = if active { theme::text() } else { theme::text_secondary() };
            pen.place(part, Some(layout), b, Align::LeadingCentered, ink);
        }
        None => part.hide_all(),
    }
}

/// Reconcile an editor's text run, its selection highlight and its IME
/// composition rule as retained sprites.
///
/// The box fill, the border and the spin column's hairline are retained parts
/// (`parts::editor_plan`) and the chevrons are sprites placed here, so an editor
/// owns no surface at all. That is also why this function opens by building the
/// DirectWrite layout: `prepare` used to run inside the surface's `BeginDraw`
/// bracket, and with the surface gone this is the one place that still runs.
///
/// Three things here are not optional:
///
/// - The host is the CONTENT COLUMN, not the node box. The painted run was
///   confined by a `push_clip`; sprites are clipped by nothing, so a scrolled
///   field would otherwise spill its text across the spin buttons and out
///   through the border.
/// - The run is read with [`TextLayout::shape`] rather than `glyph_runs`,
///   because the layout carries an underline over the active composition span
///   and `glyph_runs` drops it. `draw_text_layout` used to render that rule for
///   free; losing it would mean a user could not see what they were composing,
///   with nothing else about the text looking wrong.
/// - Every origin comes from [`editor::TextBand`], the same one the caret
///   sprite, the IME candidate window and UIA are placed by — which is what
///   keeps the sprites from drifting away from all three.
pub(crate) fn editor_sync(comp: &Compositing, atlas: &mut GlyphAtlas, node: &mut Node, scale: f32) {
    if node.editor.is_none() {
        return;
    }
    // Build the DirectWrite layout and re-scroll the caret into view BEFORE
    // anything reads it — `TextBand::of` immediately below is the first reader,
    // and the caret sprite, the IME candidate window and UIA all measure from
    // the same band.
    //
    // This used to run inside `draw_surface`'s `BeginDraw` bracket, which was
    // the editor's one real tie to owning a surface: it needs no device context
    // (only `TextFormat`/`TextLayout`), but it was reached only when there was
    // a surface to draw into. Left there, taking the surface away would have
    // stopped the layout ever being rebuilt — chrome intact, text frozen. It
    // belongs to the run's shaper, which is here.
    {
        let (kind, w, fs, weight, align) = (
            node.kind,
            node.rect.w,
            node.paint.font_size,
            node.paint.font_weight,
            node.ctrl().content_align,
        );
        let (_, content_w) = super::editor::editor_content(kind, w);
        if let Some(ed) = &mut node.editor {
            ed.prepare(fs, weight, content_w, align);
        }
    }
    let Some(band) = super::editor::TextBand::of(node) else {
        return;
    };
    let dim = if node.paint.is_enabled {
        1.0
    } else {
        theme::disabled_opacity()
    };
    // The clip column, in node-local DIPs. Full height: the run is centred
    // within it, and a descender must not be cut.
    let column = Rect::from_xywh(band.content_x, 0.0, band.content_w, node.rect.h);
    let origin = (band.origin_x, band.origin_y);

    // Borrowed, not taken: `text_part`, `editor` and `container` are distinct
    // fields, so the placement below needs no take/put-back dance.
    let fg = node.paint.foreground.unwrap_or_else(theme::text);
    let host = &node.container;
    let ed = node.editor.as_ref().expect("checked above");
    let empty = ed.buf.is_empty();
    let part = node.text_part.get_or_insert_default();

    match ed.layout.as_ref().filter(|_| !empty) {
        Some(layout) => {
            let shaped = layout.shape().ok();
            part.sync(comp, atlas, host, layout, origin, column, fg, dim, scale);

            // Selection sits behind the run, so it is placed after the host
            // exists but lands under every glyph — see `TextPart::sync_fills`.
            let sel = if node.focused && ed.has_selection() {
                let (a, b) = ed.sel();
                layout
                    .hit_test_range(a as u32, (b - a) as u32, origin.0, origin.1)
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            part.sync_fills(
                comp,
                &sel,
                (0.0, 0.0),
                theme::with_alpha(theme::accent(), 0.32),
                scale,
            );

            // The composition rule takes the text's own ink, as it did when
            // DirectWrite drew it.
            match shaped {
                Some(s) => part.sync_rules(comp, &s.decorations, origin, scale),
                None => part.sync_rules(comp, &[], origin, scale),
            }
        }
        // Empty field: nothing of the editor's OWN text to place. The
        // placeholder that takes its place is a separate run, below.
        None => part.hide_all(),
    }

    // The placeholder, shown only while the field is empty.
    //
    // Kept SHAPED across the transition rather than pinned to the empty string
    // when hidden: typing the first character and deleting the last are the two
    // moments this flips, and reshaping on each would put a DirectWrite build on
    // the keystroke path for a run that has not changed.
    //
    // Placed by the pen, which is why it is laid out at its natural width and
    // not against the content column: the alignment the painted placeholder got
    // from a `TextFormat` is now an origin, so the column is where it is CLIPPED
    // rather than what it was shaped against.
    let align = placeholder_align(node.ctrl().content_align);
    let em = node.paint.font_size;
    {
        let mut pen = Pen::new(comp, atlas, node, scale);
        let (words, slot) = node.placeholder_run();
        let (part, run) = slot.pin(words, em, 400, EDITOR_FACE);
        let run = if empty { run } else { None };
        pen.place(part, run, column, align, theme::text_tertiary());
    }

    // A wide NumberBox's spin chevrons. Placed here rather than in a sync of
    // their own because they belong to the editor's surface and share its dim,
    // and because a NumberBox that narrows past the threshold has to lose them
    // in the same pass that stops drawing their divider.
    if node.kind == crate::backend::ControlKind::NumberBox {
        let boxes = super::editor::spin_boxes(node.rect.w, node.rect.h);
        let ink = super::controls::spin_ink(node.hovered);
        let mut pen = Pen::new(comp, atlas, node, scale);
        let t = node.item_text.get_or_insert_default();
        match boxes {
            Some((up, down)) => {
                for (i, b) in [up, down].into_iter().enumerate() {
                    let (part, run) = t.slot(i, i, false);
                    pen.place(part, run, b, Align::Centered, ink);
                }
            }
            // Too narrow for the column: both chevrons go, and the press that
            // would have stepped the value is already refused by the same
            // `spin_boxes` returning `None`.
            None => t.hide_from(0),
        }
    }
}

/// The face an editor's placeholder is set in — the same one the editor lays its
/// own run out in, so the two do not change shape as the field fills.
const EDITOR_FACE: &str = "Segoe UI";

/// A placeholder's alignment, from the editor's `content_align`.
///
/// The painted placeholder expressed this as a `TextAlignment` on a format and
/// let DirectWrite position the run inside the content box. Every variant is
/// vertically centred, which the box did too.
fn placeholder_align(content_align: i32) -> Align {
    match content_align {
        1 => Align::Centered,
        2 => Align::TrailingCentered,
        _ => Align::LeadingCentered,
    }
}

/// Reconcile a button-family node's label and ornaments as retained glyph
/// sprites.
///
/// Runs from the paint pass, on a dirty node, straight after `parts::sync` — so
/// the hosts land above the ink the parts sync just created (see the module
/// header on z-order), and so a state flip that does not dirty the node never
/// gets here at all.
///
/// The geometry is deliberately not re-derived here:
/// [`controls::button_boxes`](super::controls::button_boxes) is the one
/// definition of where a button's content sits, and the measure pass sizes the
/// control from the same answer.
pub(crate) fn button_sync(comp: &Compositing, atlas: &mut GlyphAtlas, node: &mut Node, scale: f32) {
    if !super::node::is_button_family(node.kind) {
        return;
    }
    let (w, h) = (node.rect.w, node.rect.h);
    let boxes = super::controls::button_boxes(node, Rect::from_xywh(0.0, 0.0, w, h));
    let pal = super::controls::button_palette(node);
    let fg = pal.fg;
    let badge_ink = super::controls::badge_paint(node, &pal).map(|(_, ink)| ink);
    // Every run is clipped to the whole control, not to its own box: a label too
    // wide for its column loses its tail at the button's edge, not at the
    // column's.
    let clip = Rect::from_xywh(0.0, 0.0, w, h);
    let retained = super::controls::label_is_retained(node);

    let mut pen = Pen::new(comp, atlas, node, scale);
    let label = node.text_layout.as_ref().filter(|_| retained);
    let t = node.button_text.get_or_insert_default();
    pen.place_in(&mut t.label, label, boxes.label, clip, Align::Centered, fg);

    // The icon takes the label's ink: it is chrome belonging to the same
    // control, and a glyph that recoloured independently of the words beside it
    // would read as a second, unrelated element.
    match boxes.icon {
        Some(b) => {
            let run = t.icon_layout.as_ref();
            pen.place_in(&mut t.icon, run, b, clip, Align::Centered, fg);
        }
        None => t.icon.hide_all(),
    }

    // The count sits ON the badge plate, so its ink comes from the same place
    // the plate's fill does — see `controls::badge_paint`.
    match boxes.badge.zip(badge_ink) {
        Some((b, ink)) => {
            let run = t.badge_layout.as_ref();
            pen.place_in(&mut t.badge, run, b, clip, Align::Centered, ink);
        }
        None => t.badge.hide_all(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A live readout must honour the alignment its block asked for, because its
    /// box is sized for the widest value it can hold and every shorter value has
    /// slack to sit in. Left-aligning a centred column of numbers, or a
    /// right-aligned readout, puts the value somewhere its own layout never would.
    #[test]
    fn a_live_run_takes_the_blocks_own_alignment() {
        assert_eq!(live_text_align(1), Align::Centered);
        assert_eq!(live_text_align(2), Align::TrailingCentered);
        assert_eq!(live_text_align(0), Align::Leading);
    }

    /// Unset alignment must lead, not centre — a block that asked for nothing
    /// reads the same live as it does rendered.
    #[test]
    fn an_unaligned_live_run_leads() {
        assert_eq!(live_text_align(super::super::node::ALIGN_UNSET), Align::Leading);
        assert_eq!(live_text_align(3), Align::Leading, "Stretch is not a text alignment");
    }

    /// `ItemText` holds only shaped runs and placed parts, and a part with
    /// nothing placed in it has minted no compositor object — so the indexing
    /// below is exercisable with no device at all.
    fn item_text(items: usize) -> ItemText {
        ItemText {
            layouts: (0..items).map(|_| None).collect(),
            strong: Vec::new(),
            parts: Vec::new(),
        }
    }

    /// The same argument for [`RowText`]: no device is needed to exercise the
    /// indexing, because an unplaced part has minted nothing.
    fn row_text(rows: usize) -> RowText {
        RowText {
            leading: (0..rows).map(|_| None).collect(),
            labels: (0..rows).map(|_| None).collect(),
            parts: Vec::new(),
        }
    }

    /// The width `s` reports after pinning `text` at `em`. `None` when the pin
    /// shaped nothing.
    fn pinned_w(s: &mut Shaped, text: &str, em: f32) -> Option<f32> {
        let (_, run) = s.pin(text, em, 400, "Segoe UI");
        run.and_then(|l| l.measure().ok()).map(|(w, _)| w)
    }

    /// The knob's whole reason for shaping at placement time: a resize changes
    /// the em, and a mask is cached per em, so the run must be rebuilt rather
    /// than reused at the size it used to be.
    #[test]
    fn a_run_reshapes_when_its_size_moves() {
        let mut s = Shaped::default();
        let small = pinned_w(&mut s, "100", 10.0).expect("shapes at a real size");
        let large = pinned_w(&mut s, "100", 30.0).expect("reshapes at the new size");
        assert!(large > small, "{large} should exceed {small} — same words, larger em");
        assert_eq!(s.em, 30.0, "the cache key follows the run it describes");
    }

    /// The other half of the key. A knob's readout changes every frame of a
    /// drag at a size that does not move at all.
    #[test]
    fn a_run_reshapes_when_its_words_change() {
        let mut s = Shaped::default();
        let short = pinned_w(&mut s, "5", 20.0).expect("shapes");
        let long = pinned_w(&mut s, "1000", 20.0).expect("reshapes");
        assert!(long > short, "{long} should exceed {short} — more digits, same em");
        assert_eq!(s.text, "1000");
    }

    /// A knob with no unit must place NOTHING, rather than an empty layout whose
    /// measure is a zero-width box that still mints a host.
    #[test]
    fn an_empty_run_shapes_nothing() {
        let mut s = Shaped::default();
        assert!(pinned_w(&mut s, "", 20.0).is_none());
    }

    /// [`Shaped::hide`] clears the cached string, not just the layout.
    ///
    /// Leaving it would make a slot that came back with the words it used to
    /// have compare EQUAL — so it would skip the rebuild and place a layout it
    /// had already dropped, i.e. render nothing, permanently, for exactly the
    /// tick labels that were removed and then restored.
    #[test]
    fn a_hidden_run_reshapes_when_its_words_come_back() {
        let mut s = Shaped::default();
        assert!(pinned_w(&mut s, "50", 20.0).is_some());
        s.hide();
        assert!(s.layout.is_none(), "hiding drops the run…");
        assert!(
            pinned_w(&mut s, "50", 20.0).is_some(),
            "…and the same words afterwards must reshape, not compare equal to a dropped run"
        );
    }

    /// The same argument again for [`SelectText`]: the index mapping is what
    /// decides which word a trigger shows, and it needs no device to exercise.
    fn select_text(items: usize) -> SelectText {
        SelectText {
            // Items, then the placeholder — the shape the layout pass builds.
            labels: (0..items + 1).map(|_| None).collect(),
            ..Default::default()
        }
    }

    /// A ComboBox indexes its items directly, and "nothing selected" lands on
    /// the placeholder rather than on item 0.
    ///
    /// Off-by-one here is not a crash but a trigger that reads back the wrong
    /// value — the failure a user would report as the control lying about its
    /// own state.
    #[test]
    fn a_select_shows_its_selected_item_and_falls_back_to_the_placeholder() {
        let t = select_text(3);
        assert_eq!(t.label_index(0), 0);
        assert_eq!(t.label_index(2), 2, "the last real item, not the placeholder");
        assert_eq!(t.label_index(-1), 3, "nothing selected shows the placeholder");
    }

    /// A selection outlives the items it indexed: `Items` re-runs the text pass
    /// and `SelectedIndex` does not, so the two can disagree for a frame. The
    /// stale index must show the empty state, never nothing at all.
    #[test]
    fn a_stale_selection_falls_back_rather_than_placing_no_run() {
        let t = select_text(2);
        assert_eq!(t.label_index(7), 2, "past the end is the placeholder");
        assert_eq!(
            select_text(0).label_index(4),
            0,
            "a trigger with no items has only its placeholder to show"
        );
    }

    /// A DropDownButton owns ONE run — its own content — and has no empty
    /// state. Its `selected_index` is never set, so the placeholder fallback is
    /// what has to land on that single run.
    #[test]
    fn a_dropdown_button_places_its_only_run_whatever_the_index_says() {
        let t = SelectText {
            labels: vec![None],
            ..Default::default()
        };
        assert_eq!(t.label_index(-1), 0);
        assert_eq!(t.label_index(3), 0);
    }

    /// A row is TWO parts, and reaching row `i` must grow both — the leading
    /// glyph and the label are placed from one call and neither may find its
    /// slot missing.
    #[test]
    fn a_row_grows_both_its_parts() {
        let mut t = row_text(3);
        t.row(0);
        assert_eq!(t.parts.len(), 2, "one row is a glyph part and a label part");
        t.row(2);
        assert_eq!(
            t.parts.len(),
            6,
            "reaching row 2 grows every row up to it, not just its own"
        );
    }

    /// The interleaving is the invariant: row `i` owns parts `2i` and `2i+1`.
    ///
    /// Two parallel vectors would let a row be half-retired — its label hidden
    /// while its glyph stayed lit — which is precisely what one `hide_from` over
    /// one interleaved vector makes unspellable.
    #[test]
    fn hiding_a_row_retires_both_of_its_parts() {
        let mut t = row_text(3);
        for i in 0..3 {
            t.row(i);
        }
        t.labels.truncate(1);
        t.leading.truncate(1);
        t.hide_from(1);
        assert_eq!(t.parts.len(), 6, "the parts are retained for reuse…");
        assert!(
            t.parts[2..].iter().all(|p| p.host.is_none()),
            "…but BOTH runs of every departed row show nothing"
        );
    }

    /// A rebuild swaps the runs and must not take the sprites with them: the
    /// parts own compositor visuals parented into the node, so dropping them
    /// would orphan what is on screen and mint a second set beside it.
    #[test]
    fn adopting_new_runs_keeps_every_part() {
        let mut t = row_text(2);
        t.row(1);
        assert_eq!(t.parts.len(), 4);
        t.adopt(vec![None], vec![None]);
        assert_eq!(t.len(), 1, "the runs are the new, shorter set…");
        assert_eq!(t.parts.len(), 4, "…and every part survived the swap");
    }

    /// The part index and the item index are separate questions, and a toggle is
    /// the control that proves it: two shaped labels, ONE of which is showing.
    ///
    /// If `slot` grew a part per ITEM, flipping the switch would place the new
    /// word into a second part and leave the old one lit — both states visible
    /// at once, which is exactly the failure the two-index signature exists to
    /// make unrepresentable.
    #[test]
    fn a_toggles_two_labels_share_one_part() {
        let mut t = item_text(2);
        // Off, then on: the state picks the ITEM, never the part.
        t.slot(0, 0, false);
        assert_eq!(t.parts.len(), 1, "the off label mints exactly one part");
        t.slot(0, 1, false);
        assert_eq!(
            t.parts.len(),
            1,
            "flipping to the on label must reuse that part, not grow a second"
        );
    }

    /// A segment bar is the other half: N items ARE N parts, because all of them
    /// are on screen together.
    #[test]
    fn a_segment_bar_grows_one_part_per_item() {
        let mut t = item_text(4);
        for i in 0..4 {
            t.slot(i, i, i == 2);
        }
        assert_eq!(t.parts.len(), 4);
    }

    /// Parts are never shrunk, so a bar that loses segments has to hide the
    /// departed ones explicitly — otherwise their words stay on screen with no
    /// segment under them.
    #[test]
    fn parts_outlive_the_items_that_grew_them() {
        let mut t = item_text(3);
        for i in 0..3 {
            t.slot(i, i, false);
        }
        t.layouts.truncate(1);
        t.hide_from(1);
        assert_eq!(t.parts.len(), 3, "the parts are retained for reuse…");
        assert!(
            t.parts[1..].iter().all(|p| p.host.is_none()),
            "…but the two beyond the surviving item show nothing"
        );
    }

    /// A control that shaped no emphasis weight must still render its words.
    /// `strong` is empty for every kind but the segment bar, so a `true` here
    /// asking for a run that was never built has to fall through to the rest
    /// weight rather than return `None` and hide the label.
    #[test]
    fn asking_for_a_weight_that_was_never_shaped_falls_back() {
        let mut t = item_text(1);
        // Both vectors hold `None` at index 0 here, so this pins the SELECTION,
        // not the run: the fallback arm is the one that must be reached.
        assert!(t.strong.is_empty());
        let (_, run) = t.slot(0, 0, true);
        assert!(run.is_none(), "no run was built, so none comes back");

        // And the measure pass reads the rest runs when there are no strong ones
        // — sizing a control to an empty vector would collapse it to nothing.
        assert_eq!(t.measurable().count(), 0);
    }
}
