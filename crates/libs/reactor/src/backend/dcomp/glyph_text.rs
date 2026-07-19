//! A label drawn as one composition sprite per glyph.
//!
//! The glyph atlas ([`super::glyph_atlas`]) holds one mask per glyph; this
//! places them. Every glyph is a [`SpriteVisual`] whose brush is a
//! [`CompositionMaskBrush`] — the atlas mask cutting a **shared** FP16 colour
//! source. Once placed, nothing about the label reaches a `BeginDraw` again:
//!
//! - a **recolour** rebuilds one source surface and points every glyph's mask
//!   brush at it, so it costs N `SetSource` calls and zero rasters;
//! - **enable/disable** is the container's opacity;
//! - a **text change** re-places sprites against masks that are already cached
//!   whenever the letters have been seen before.
//!
//! One source per label, not per glyph, is the whole point of the sharing: the
//! source is a 4×4 solid, and minting one per glyph would put an allocation on
//! a path whose reason to exist is that it has none.
//!
//! ## Z-order
//!
//! Glyph sprites are inserted at the top of the node's children, so they sit
//! above both the chrome parts *and* the hover/press ink. That is a deliberate
//! departure from the painted label, which sat under the ink and was lightened
//! by it: a wash belongs on the surface behind text, not over the text, and
//! WinUI's own button states recolour the background alone.
//!
//! The ordering is positional, not declared — it holds because these are
//! inserted after `parts::ensure` has built the ink, and it would break if
//! anything re-stacked the node's children afterwards. [`TextPart::restack`]
//! re-asserts it for a caller that has done so.

use windows_canvas_core::TextLayout;
use windows_core::Interface;
use windows_numerics::{Vector2, Vector3};

use super::bootstrap::Compositing;
use super::glyph_atlas::{pen_phase, GlyphAtlas};
use super::node::Node;
use super::parts::build_solid_surface;
use crate::system_bindings::{
    CompositionBrush, CompositionMaskBrush, CompositionSurfaceBrush, ICompositionObject,
    ICompositor2, IVisual, SpriteVisual, Visual,
};

/// One glyph's sprite and the mask brush that colours it.
struct GlyphSprite {
    sprite: SpriteVisual,
    vis: IVisual,
    mask: CompositionMaskBrush,
    /// Raw pointer of the atlas mask currently bound. Identity is the right
    /// test here: the atlas hands back a clone of the same COM object for a
    /// cache hit, so an unchanged glyph compares equal and re-binds nothing.
    bound: Option<*mut core::ffi::c_void>,
    offset: Option<(f32, f32)>,
    size: Option<(f32, f32)>,
    shown: bool,
}

impl GlyphSprite {
    fn new(comp: &Compositing, node: &Node) -> Option<Self> {
        let sprite = comp.new_sprite().ok()?;
        let vis: IVisual = sprite.cast().ok()?;
        let compositor = sprite.cast::<ICompositionObject>().ok()?.Compositor().ok()?;
        let mask = compositor.cast::<ICompositor2>().ok()?.CreateMaskBrush().ok()?;
        sprite.SetBrush(&mask.cast::<CompositionBrush>().ok()?).ok()?;
        node.container
            .Children()
            .ok()?
            .InsertAtTop(&sprite.cast::<Visual>().ok()?)
            .ok()?;
        Some(Self {
            sprite,
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

/// The retained glyph sprites of one label.
#[derive(Default)]
pub(crate) struct TextPart {
    glyphs: Vec<GlyphSprite>,
    /// How many of `glyphs` the last sync actually used. The rest stay
    /// allocated and hidden — a label that shortens will very likely lengthen
    /// again, and a composition visual is cheaper to hide than to rebuild.
    live: usize,
    /// The one FP16 colour source every glyph's mask brush reads.
    source: Option<CompositionSurfaceBrush>,
    /// `(colour bits, scale bits)` the source was built for.
    source_for: Option<([u32; 4], u32)>,
    /// The atlas epoch the bound masks came from; a bump invalidates them all.
    epoch: u32,
}

fn color_bits(c: crate::Color) -> [u32; 4] {
    [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()]
}

impl TextPart {
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
    pub(crate) fn sync(
        &mut self,
        comp: &Compositing,
        atlas: &mut GlyphAtlas,
        node: &Node,
        layout: &TextLayout,
        origin: (f32, f32),
        color: crate::Color,
        scale: f32,
    ) {
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
            let baseline_y = origin.1 + run.baseline_origin.y;
            let mut pen_x = origin.0 + run.baseline_origin.x;
            for (i, &glyph) in run.glyph_indices.iter().enumerate() {
                let (whole_px, phase) = pen_phase(pen_x, scale);
                if let Some(raster) =
                    atlas.get(comp, &run.font_face, glyph, run.font_em_size, scale, phase)
                {
                    // Grow on demand; a label only pays for the glyphs it has.
                    if slot == self.glyphs.len() {
                        match GlyphSprite::new(comp, node) {
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
                    let (w, h) = raster.geom.size_dip;
                    g.place(
                        whole_px as f32 / scale - raster.geom.baseline_dip.0,
                        baseline_y - raster.geom.baseline_dip.1,
                        w,
                        h,
                    );
                    g.show(true);
                    slot += 1;
                }
                // Advance by the shaped advance, plus this glyph's own nudge if
                // the run carries offsets.
                pen_x += run.glyph_advances.get(i).copied().unwrap_or(0.0);
                if let Some(off) = run.glyph_offsets.get(i) {
                    pen_x += off.advance_offset;
                }
            }
        }
        self.live = slot;
        self.hide_from(slot);
    }

    fn hide_from(&mut self, from: usize) {
        let start = from.min(self.glyphs.len());
        for g in &mut self.glyphs[start..] {
            g.show(false);
        }
        self.live = self.live.min(from);
    }

    /// Re-assert the glyphs above everything else in the node's children, for a
    /// caller that has re-stacked them. See the module header.
    pub(crate) fn restack(&self, node: &Node) {
        let Ok(children) = node.container.Children() else {
            return;
        };
        for g in self.glyphs.iter().take(self.live) {
            if let Ok(v) = g.sprite.cast::<Visual>() {
                let _ = children.InsertAtTop(&v);
            }
        }
    }
}
