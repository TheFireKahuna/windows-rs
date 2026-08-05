## Windows Text

Windows Text owns DirectWrite: the factory, text formats and layouts, shaping, measurement, font
fallback and cluster geometry.

Shaping and drawing happen on different threads, so a shaped run leaves this crate as plain data —
glyph indices, advances, offsets, and the id of the face DirectWrite chose. The crate that draws it
depends on no DirectWrite type from here, and nothing thread-affine crosses.

A line is drawn as **one glyph run per segment** — a segment being a maximal single-face span of a
line — rather than one per glyph, so an ordinary line of text costs a single draw and a single
coverage tile. Coverage carries no colour; the paint supplies it.

Every extent this crate reports is in DIPs, so whoever rasterizes a run holds the scale.
