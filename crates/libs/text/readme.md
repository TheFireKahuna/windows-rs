## Windows Text

Windows Text owns DirectWrite: the factory, text formats and layouts, shaping and measurement, and
the coverage rasterizer that turns a shaped run into an alpha-only tile.

Rasterization produces **one coverage tile per run** — a run being a maximal same-paint span of a
line — rather than one per glyph, so an ordinary line of text costs a single sprite. Coverage
carries no colour; the paint supplies it.

A shaped run leaves this crate as plain data, so the crate that draws it never depends on a
DirectWrite type from here.
