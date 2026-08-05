## windows-color

Colour-space mathematics and the display output transform for a wide-gamut, HDR
user interface on Windows.

Light is scene-linear and unbounded in a wide working space throughout, and one display
transform runs at the end. Standard dynamic range is that transform with different
numbers in it, not a separate code path.

Two types carry the design:

* [`Radiance`] — scene-referred light. Linear Rec.2020 primaries, absolute cd/m²,
  unbounded. Mixing, coverage, gradients and washes all compute here.
* [`Scrgb`] — display-referred output. Linear Rec.709 primaries, scRGB scale. What
  Direct2D and the Windows compositor accept.

[`OutputTransform::apply`] is the only function producing an [`Scrgb`], and nothing
converts one back, so the display transform runs exactly once per colour: applying it
twice is not expressible, and applying it zero times fails to compile.

Colours are authored in BT.2100 [`Ictcp`] at absolute luminance, which is hue-linear, so
gamut compression holds hue exactly while it moves chroma.

This crate has no dependencies.
