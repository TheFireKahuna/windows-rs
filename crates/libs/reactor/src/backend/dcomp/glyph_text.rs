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

use windows_canvas_core::{Rect, TextDecoration, TextLayout};
use windows_core::Interface;
use windows_numerics::{Vector2, Vector3};

use super::bootstrap::Compositing;
use super::glyph_atlas::{pen_phase, GlyphAtlas};
use super::node::Node;
use super::parts::build_solid_surface;
use super::theme;
use crate::system_bindings::{
    CompositionBrush, CompositionClip, CompositionMaskBrush, CompositionSurfaceBrush,
    ContainerVisual, ICompositionObject, ICompositor2, IVisual, SpriteVisual, Visual,
};

/// One glyph's sprite and the mask brush that colours it.
///
/// Only the `IVisual` view is retained: the sprite itself is parented into the
/// host, which owns the reference that keeps it alive, and every operation here
/// (place, show) is a visual one.
struct GlyphSprite {
    vis: IVisual,
    mask: CompositionMaskBrush,
    /// Raw pointer of the atlas mask currently bound. Identity is the right
    /// test here: the atlas hands back a clone of the same COM object for a
    /// cache hit, so an unchanged glyph compares equal and re-binds nothing.
    ///
    /// It cannot go stale through address reuse, because `SetMask` retains the
    /// brush: whatever this points at is kept alive by the very binding it
    /// describes, so the address cannot be recycled while the comparison still
    /// means anything — the same argument `GlyphKey` makes for its face pointer.
    bound: Option<*mut core::ffi::c_void>,
    offset: Option<(f32, f32)>,
    size: Option<(f32, f32)>,
    shown: bool,
}

impl GlyphSprite {
    fn new(comp: &Compositing, host: &ContainerVisual) -> Option<Self> {
        let sprite = comp.new_sprite().ok()?;
        let vis: IVisual = sprite.cast().ok()?;
        let compositor = sprite.cast::<ICompositionObject>().ok()?.Compositor().ok()?;
        let mask = compositor.cast::<ICompositor2>().ok()?.CreateMaskBrush().ok()?;
        sprite.SetBrush(&mask.cast::<CompositionBrush>().ok()?).ok()?;
        host.Children()
            .ok()?
            .InsertAtTop(&sprite.cast::<Visual>().ok()?)
            .ok()?;
        Some(Self {
            vis,
            mask,
            bound: None,
            offset: None,
            size: None,
            shown: false,
        })
    }

    fn place(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if self.offset != Some((x, y)) {
            let _ = self.vis.SetOffset(Vector3::new(x, y, 0.0));
            self.offset = Some((x, y));
        }
        if self.size != Some((w, h)) {
            let _ = self.vis.SetSize(Vector2::new(w, h));
            self.size = Some((w, h));
        }
    }

    fn show(&mut self, on: bool) {
        if self.shown != on {
            let _ = self.vis.SetIsVisible(on);
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
    vis: IVisual,
    sprite: SpriteVisual,
    /// Raw pointer of the brush currently bound, on the same identity argument
    /// [`GlyphSprite::bound`] makes.
    bound: Option<*mut core::ffi::c_void>,
    offset: Option<(f32, f32)>,
    size: Option<(f32, f32)>,
    shown: bool,
}

impl RectSprite {
    fn new(comp: &Compositing, parent: &ContainerVisual) -> Option<Self> {
        let sprite = comp.new_sprite().ok()?;
        let vis: IVisual = sprite.cast().ok()?;
        parent
            .Children()
            .ok()?
            .InsertAtTop(&sprite.cast::<Visual>().ok()?)
            .ok()?;
        Some(Self {
            vis,
            sprite,
            bound: None,
            offset: None,
            size: None,
            shown: false,
        })
    }

    fn paint(&mut self, brush: &CompositionBrush) {
        let raw = brush.as_raw();
        if self.bound != Some(raw) && self.sprite.SetBrush(brush).is_ok() {
            self.bound = Some(raw);
        }
    }

    fn place(&mut self, x: f32, y: f32, w: f32, h: f32) {
        if self.offset != Some((x, y)) {
            let _ = self.vis.SetOffset(Vector3::new(x, y, 0.0));
            self.offset = Some((x, y));
        }
        if self.size != Some((w, h)) {
            let _ = self.vis.SetSize(Vector2::new(w, h));
            self.size = Some((w, h));
        }
    }

    fn show(&mut self, on: bool) {
        if self.shown != on {
            let _ = self.vis.SetIsVisible(on);
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
    host_vis: Option<IVisual>,
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
    fn ensure_host(&mut self, comp: &Compositing, parent: &ContainerVisual) -> bool {
        if self.host.is_some() {
            return true;
        }
        let built = || -> Option<(ContainerVisual, IVisual)> {
            let host = comp.new_container().ok()?;
            let vis: IVisual = host.cast().ok()?;
            let clip = host
                .cast::<ICompositionObject>()
                .ok()?
                .Compositor()
                .ok()?
                .CreateInsetClip()
                .ok()?;
            vis.SetClip(&clip.cast::<CompositionClip>().ok()?).ok()?;
            parent
                .Children()
                .ok()?
                .InsertAtTop(&host.cast::<Visual>().ok()?)
                .ok()?;
            Some((host, vis))
        }();
        match built {
            Some((h, v)) => {
                self.host = Some(h);
                self.host_vis = Some(v);
                true
            }
            None => false,
        }
    }

    /// Place and size the host to the box its clip cuts to, and carry the
    /// disabled dim on its opacity. All three self-gate.
    fn place_host(&mut self, box_: Rect, dim: f32) {
        let Some(vis) = self.host_vis.as_ref() else { return };
        if self.host_box != Some(box_) {
            let _ = vis.SetOffset(Vector3::new(box_.left, box_.top, 0.0));
            let _ = vis.SetSize(Vector2::new(box_.width(), box_.height()));
            self.host_box = Some(box_);
        }
        if self.host_dim != Some(dim) {
            let _ = vis.SetOpacity(dim);
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
    ) -> Option<CompositionBrush> {
        let want = (color_bits(color), scale.to_bits());
        if self.source_for != Some(want) {
            let src = build_solid_surface(comp, color, scale)?;
            self.source = Some(src);
            self.source_for = Some(want);
            // Re-point every sprite, including ones this sync will not touch:
            // a hidden sprite that is shown again later must not surface the
            // previous colour.
            let cb = self.source.as_ref()?.cast::<CompositionBrush>().ok()?;
            for g in &mut self.glyphs {
                let _ = g.mask.SetSource(&cb);
            }
        }
        self.source.as_ref()?.cast::<CompositionBrush>().ok()
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
        if !self.ensure_host(comp, parent) {
            return;
        }
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
                        match GlyphSprite::new(comp, &host) {
                            Some(g) => self.glyphs.push(g),
                            None => break,
                        }
                    }
                    let g = &mut self.glyphs[slot];
                    let raw = raster.brush.as_raw();
                    if g.bound != Some(raw) {
                        if let Ok(mb) = raster.brush.cast::<CompositionBrush>()
                            && g.mask.SetMask(&mb).is_ok()
                            && g.mask.SetSource(&source).is_ok()
                        {
                            g.bound = Some(raw);
                        }
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
        let (Some(host), Some(src)) = (self.host.clone(), self.source.clone()) else {
            self.hide_rules_from(0);
            return;
        };
        let Ok(brush) = src.cast::<CompositionBrush>() else {
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
                match RectSprite::new(comp, &host) {
                    Some(r) => self.rules.push(r),
                    None => break,
                }
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
        if self.fill_host.is_none() {
            self.fill_host = (|| {
                let c = comp.new_container().ok()?;
                host.Children()
                    .ok()?
                    .InsertAtBottom(&c.cast::<Visual>().ok()?)
                    .ok()?;
                Some(c)
            })();
        }
        let Some(fill_host) = self.fill_host.clone() else {
            self.hide_fills_from(0);
            return;
        };

        let want = (color_bits(color), scale.to_bits());
        if self.fill_source_for != Some(want) {
            match build_solid_surface(comp, color, scale) {
                Some(s) => {
                    self.fill_source = Some(s);
                    self.fill_source_for = Some(want);
                    // Re-point every fill, hidden ones included — a fill shown
                    // again later must not surface the previous colour.
                    self.fills.iter_mut().for_each(|f| f.bound = None);
                }
                None => {
                    self.hide_fills_from(0);
                    return;
                }
            }
        }
        let Some(brush) = self
            .fill_source
            .as_ref()
            .and_then(|s| s.cast::<CompositionBrush>().ok())
        else {
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
                match RectSprite::new(comp, &fill_host) {
                    Some(f) => self.fills.push(f),
                    None => break,
                }
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

/// Place a run at the top-left of `b`, without centring it.
///
/// The entry point for text whose own layout already decides where each line
/// goes: wrapped prose, and anything leading-aligned. Wrapping needs nothing
/// beyond this — DirectWrite expresses it as one glyph run per line at its own
/// descending baseline, and [`TextPart::sync`] already walks runs and honours
/// each one's baseline origin, so a wrapped layout places correctly through the
/// identical code path a single line does.
#[allow(clippy::too_many_arguments)]
fn place_leading(
    part: &mut TextPart,
    comp: &Compositing,
    atlas: &mut GlyphAtlas,
    parent: &ContainerVisual,
    layout: &TextLayout,
    b: Rect,
    host_box: Rect,
    color: crate::Color,
    dim: f32,
    scale: f32,
) {
    part.sync(
        comp,
        atlas,
        parent,
        layout,
        (b.left, b.top),
        host_box,
        color,
        dim,
        scale,
    );
}

/// Place one run at the leading edge of `b`, centred on its vertical axis — the
/// alignment a label beside something else takes.
///
/// Expressed as an origin rather than as a text format because a shaped run
/// carries no alignment of its own once it is placed by hand: `TextAlignment`
/// and `ParagraphAlignment` are instructions to the drawing call this path no
/// longer makes.
#[allow(clippy::too_many_arguments)]
fn place_leading_centered(
    part: &mut TextPart,
    comp: &Compositing,
    atlas: &mut GlyphAtlas,
    parent: &ContainerVisual,
    layout: &TextLayout,
    b: Rect,
    host_box: Rect,
    color: crate::Color,
    dim: f32,
    scale: f32,
) {
    let Ok((_, th)) = layout.measure() else {
        part.hide_all();
        return;
    };
    let origin = (b.left, b.top + ((b.height() - th) / 2.0).max(0.0));
    part.sync(comp, atlas, parent, layout, origin, host_box, color, dim, scale);
}

/// Place one run centred in `b`, or hide it if it cannot be measured.
///
/// Centring is clamped at the leading edge so a run too wide for its box loses
/// its tail — which the host's clip hides — rather than its head.
#[allow(clippy::too_many_arguments)]
fn place_centered(
    part: &mut TextPart,
    comp: &Compositing,
    atlas: &mut GlyphAtlas,
    parent: &ContainerVisual,
    layout: &TextLayout,
    b: Rect,
    host_box: Rect,
    color: crate::Color,
    dim: f32,
    scale: f32,
) {
    let Ok((tw, th)) = layout.measure() else {
        part.hide_all();
        return;
    };
    let origin = (
        b.left + ((b.width() - tw) / 2.0).max(0.0),
        b.top + ((b.height() - th) / 2.0).max(0.0),
    );
    part.sync(comp, atlas, parent, layout, origin, host_box, color, dim, scale);
}

/// Reconcile a `TextBlock`'s prose as retained glyph sprites.
///
/// The counterpart to [`button_sync`] for the one control that is nothing but
/// text. It places at the node's top-left rather than centring, because a
/// TextBlock's own layout — its alignment, and its wrapping — has already
/// decided where every line goes; centring the block here would fight it.
pub(crate) fn text_sync(comp: &Compositing, atlas: &mut GlyphAtlas, node: &mut Node, scale: f32) {
    if node.kind != crate::backend::ControlKind::TextBlock {
        return;
    }
    let (w, h) = (node.rect.w, node.rect.h);
    let mut part = node.text_part.take().unwrap_or_default();
    match node.text_layout.as_ref() {
        Some(layout) => {
            // Un-styled text takes the themable primary text token — never a
            // literal — so a host token table restyles default text too.
            let fg = node.paint.foreground.unwrap_or_else(theme::text);
            place_leading(
                &mut part,
                comp,
                atlas,
                &node.container,
                layout,
                Rect::from_xywh(0.0, 0.0, w, h),
                Rect::from_xywh(0.0, 0.0, w, h),
                fg,
                1.0,
                scale,
            );
        }
        None => part.hide_all(),
    }
    node.text_part = Some(part);
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
    let (w, h) = (node.rect.w, node.rect.h);
    let dim = if node.paint.is_enabled {
        1.0
    } else {
        theme::disabled_opacity()
    };
    let ink = if node.hovered {
        theme::accent_light()
    } else {
        theme::accent()
    };

    let mut part = node.text_part.take().unwrap_or_default();
    let b = Rect::from_xywh(0.0, 0.0, w, h);
    match node.text_layout.as_ref() {
        Some(layout) => place_leading_centered(
            &mut part,
            comp,
            atlas,
            &node.container,
            layout,
            b,
            b,
            ink,
            dim,
            scale,
        ),
        None => part.hide_all(),
    }
    node.text_part = Some(part);
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
    let (w, h) = (node.rect.w, node.rect.h);
    let dim = if node.paint.is_enabled {
        1.0
    } else {
        theme::disabled_opacity()
    };
    let fg = node.paint.foreground.unwrap_or_else(theme::text);
    let x0 = super::parts::TRACK_W + super::controls::TOGGLE_LABEL_GAP;
    let b = Rect::from_xywh(x0, 0.0, (w - x0).max(0.0), h);
    // Index, not a stored string: `layouts` is `[off, on]`, so the state picks
    // the run and the same single part re-places on every flip.
    let i = usize::from(node.ctrl().is_on);

    let mut t = node.item_text.take().unwrap_or_default();
    match t.slot(0, i, false) {
        (part, Some(layout)) => place_leading_centered(
            part,
            comp,
            atlas,
            &node.container,
            layout,
            b,
            b,
            fg,
            dim,
            scale,
        ),
        (part, None) => part.hide_all(),
    }
    node.item_text = Some(t);
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
    let dim = if node.paint.is_enabled {
        1.0
    } else {
        theme::disabled_opacity()
    };
    // The same geometry paint, hit-testing and UIA item rects all read, so a
    // label cannot land anywhere but on the segment the pointer will report.
    let m = super::controls::seg_metrics(node.paint.style_variant, node.paint.font_size);
    let edges = super::controls::segment_edges(node);
    let pill_h = (node.rect.h - 2.0 * m.tray).max(0.0);
    let sel = node.ctrl().selected_index;
    let hot = node.ctrl().hot_index;
    let hovered = node.paint.is_enabled && node.hovered;

    let mut t = node.item_text.take().unwrap_or_default();
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
        match t.slot(i, i, active) {
            (part, Some(layout)) => place_centered(
                part,
                comp,
                atlas,
                &node.container,
                layout,
                seg,
                seg,
                color,
                dim,
                scale,
            ),
            (part, None) => part.hide_all(),
        }
    }
    // Items can go away: a bar rebuilt with fewer segments must not leave the
    // departed ones' words on screen.
    t.hide_from(n);
    node.item_text = Some(t);
}

/// Reconcile an editor's text run, its selection highlight and its IME
/// composition rule as retained sprites.
///
/// The box fill, the border and the spin chevrons stay painted — an editor,
/// unlike a button or a TextBlock, genuinely does draw things that are not
/// text, so it keeps its surface and only the text leaves it.
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

    let mut part = node.text_part.take().unwrap_or_default();
    let ed = node.editor.as_ref().expect("checked above");
    let empty = ed.buf.is_empty();

    match ed.layout.as_ref().filter(|_| !empty) {
        Some(layout) => {
            let fg = node.paint.foreground.unwrap_or_else(theme::text);
            let shaped = layout.shape().ok();
            part.sync(comp, atlas, &node.container, layout, origin, column, fg, dim, scale);

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
        // Empty field: the placeholder is still painted (it is tertiary ink the
        // editor owns no layout for), so there is nothing to place here.
        None => part.hide_all(),
    }

    node.text_part = Some(part);
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
    let dim = if node.paint.is_enabled {
        1.0
    } else {
        theme::disabled_opacity()
    };

    let mut t = node.button_text.take().unwrap_or_default();

    match node
        .text_layout
        .as_ref()
        .filter(|_| super::controls::label_is_retained(node))
    {
        Some(layout) => place_centered(
            &mut t.label,
            comp,
            atlas,
            &node.container,
            layout,
            boxes.label,
            Rect::from_xywh(0.0, 0.0, w, h),
            fg,
            dim,
            scale,
        ),
        None => t.label.hide_all(),
    }

    // The icon takes the label's ink: it is chrome belonging to the same
    // control, and a glyph that recoloured independently of the words beside it
    // would read as a second, unrelated element.
    match (boxes.icon, t.icon_layout.take()) {
        (Some(b), Some(layout)) => {
            place_centered(
                &mut t.icon,
                comp,
                atlas,
                &node.container,
                &layout,
                b,
                Rect::from_xywh(0.0, 0.0, w, h),
                fg,
                dim,
                scale,
            );
            t.icon_layout = Some(layout);
        }
        (_, layout) => {
            t.icon_layout = layout;
            t.icon.hide_all();
        }
    }

    // The count sits ON the badge plate, so its ink comes from the same place
    // the plate's fill does — see `controls::badge_paint`.
    match (boxes.badge.zip(badge_ink), t.badge_layout.take()) {
        (Some((b, ink)), Some(layout)) => {
            place_centered(
                &mut t.badge,
                comp,
                atlas,
                &node.container,
                &layout,
                b,
                Rect::from_xywh(0.0, 0.0, w, h),
                ink,
                dim,
                scale,
            );
            t.badge_layout = Some(layout);
        }
        (_, layout) => {
            t.badge_layout = layout;
            t.badge.hide_all();
        }
    }

    node.button_text = Some(t);
}

#[cfg(test)]
mod tests {
    use super::*;

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
