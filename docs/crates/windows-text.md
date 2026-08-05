# windows-text

> DirectWrite shaping and measurement, and the glyph-run drawing that puts a shaped run on a surface as alpha-only coverage.

- Not published to crates.io
- [Getting started](../../crates/libs/text/readme.md)
- [Source](https://github.com/microsoft/windows-rs/tree/master/crates/libs/text)

`windows-text` owns the DirectWrite factory, text formats and layouts, glyph run extraction through
a recording renderer, and the cluster geometry a caret and a selection are built from.

A shaped run crosses a thread boundary as plain data: glyph indices, advances, offsets and the id of
the face DirectWrite chose, addressed by `(offset, count)` into pooled buffers. A line is drawn as
one glyph run per segment — a maximal single-face span — so an ordinary line of text costs one draw
rather than one per character. Coverage carries no colour; the paint supplies it.

Every extent the crate reports is in DIPs, so whoever rasterizes a run holds the scale and
re-rasterizes when it changes.

A run is shaped once and placed many times: measuring and pinning move the layout box, and only new
text or a new spec reshapes it.
