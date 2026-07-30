## windows-color

Colour-space mathematics and the display output transform for a wide-gamut, HDR
user interface on Windows.

The model is a game engine's, applied to UI: unbounded scene-linear light in a wide
working space throughout, and a single display transform at the end. Standard dynamic
range is that transform with different numbers in it, not a different code path.

Two types carry the whole design:

* [`Radiance`] — scene-referred light. Linear Rec.2020 primaries, absolute cd/m²,
  unbounded. Everything a UI computes — mixing, coverage, gradients, washes — happens
  here.
* [`Scrgb`] — display-referred output. Linear Rec.709 primaries, scRGB scale. The only
  thing Direct2D and the Windows compositor accept.

[`OutputTransform::apply`] is the only function producing an [`Scrgb`], and nothing
converts one back. Applying the display transform twice is therefore not expressible,
and applying it zero times fails to compile.

Colours are authored in BT.2100 [`Ictcp`] at absolute luminance, which is hue-linear
and so lets gamut compression hold hue exactly while it moves chroma.

This crate has no dependencies.
