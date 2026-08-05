# windows-d2d

> Direct2D drawing: the device pair, the drawing bracket, geometry and geometry realizations, brushes, bitmap targets and sprite batches.

- Not published to crates.io
- [Getting started](../../crates/libs/d2d/readme.md)
- [Source](https://github.com/microsoft/windows-rs/tree/master/crates/libs/d2d)

`windows-d2d` owns the Direct2D stack and the Direct3D 11 / DXGI surface a Direct2D device is
built on. Render targets are FP16, with the alpha mode following the content's own opacity, so
colour stays in linear scRGB and values above paper white reach the compositor.

DirectWrite appears here only where a Direct2D signature names it — a text layout to draw, a glyph
run to fill. Shaping and rasterization belong to [`windows-text`](windows-text.md), and a shaped run
crosses between the two as plain data rather than as a DirectWrite struct.

A COM object arriving from another crate is accepted as `&impl Interface` and cast on the way in,
which is what lets a presentation buffer from [`windows-present`](windows-present.md) become a
Direct2D target without either crate depending on the other's bindings.
