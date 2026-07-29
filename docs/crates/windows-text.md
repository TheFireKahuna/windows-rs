# windows-text

> DirectWrite shaping and measurement, and the coverage rasterizer that turns a shaped run into an alpha-only tile.

- Not published to crates.io
- [Getting started](../../crates/libs/text/readme.md)
- [Source](https://github.com/microsoft/windows-rs/tree/master/crates/libs/text)

`windows-text` owns the DirectWrite factory, text formats and layouts, glyph run extraction, and
`IDWriteGlyphRunAnalysis`-based rasterization to a CPU alpha texture.

Rasterization produces one coverage tile per **run** — a maximal same-paint span of a line — rather
than one per glyph, so an ordinary line of text costs a single sprite rather than one per character.
Coverage carries no colour; the paint supplies it. A tile is keyed on the scale it was rasterized
at, so a DPI change re-rasterizes rather than resampling.

Shaping is structural and placement is layout: a run is shaped once and placed many times, and
moving it must never reshape it.
