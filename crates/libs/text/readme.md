## Windows Text

Windows Text owns DirectWrite: the factory, text formats and layouts, shaping, measurement, font
fallback and cluster geometry.

Shaping and rasterization happen on different threads, so a shaped run leaves this crate as plain
data — glyph indices, advances, offsets, and the id of the face DirectWrite actually chose. The
crate that draws it never depends on a DirectWrite type from here, and nothing thread-affine can
cross.

Rasterization produces **one coverage tile per run** — a run being a maximal same-paint span of a
line — rather than one per glyph, so an ordinary line of text costs a single sprite. Coverage
carries no colour; the paint supplies it.
