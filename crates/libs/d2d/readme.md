## Windows Direct2D

Windows Direct2D provides the Direct2D drawing stack: a `Gpu` pairing a Direct3D 11 device with a
Direct2D device, the drawing bracket and the targets bound inside it, path geometry and geometry
realizations, brushes, layers, sprite batches, and device-loss classification.

Four rules shape the surface.

**The device is the context.** `Gpu` owns the one `ID2D1DeviceContext` and no method makes another.
A second context on the same device costs a second `BeginDraw` and a Direct3D device-context-state
swap every time it draws, so a cached intermediate is rendered by retargeting the open bracket.

**The bracket is a value, and it is exclusive.** `Pass` is RAII over `BeginDraw`/`EndDraw` and spans
however many targets a pass touches; `Pass::draw` takes `&mut self`, so exactly one target is bound
at a time and an unpopped layer cannot cross a retarget. `Pass::end` reports the tag that was active
when a call latched an error. One error discards every later draw in the bracket, so the tag is what
separates a wrong draw from a draw killed by an earlier one.

**One place mints a target, and no signature names a format.** Every render target is FP16
(`DXGI_FORMAT_R16G16B16A16_FLOAT`); the alpha mode comes from an `Opacity` enum named after what the
content does rather than after the API. Colour stays in linear scRGB, and values above paper white
and outside Rec.709 reach the compositor whatever surface they were rendered into.

**There is no colour type here.** Brushes and gradient stops take a `windows_color::Scrgb`, which is
layout-compatible with `D2D1_COLOR_F` and reachable only from the output transform, so a
scene-referred value reaches Direct2D having passed that transform exactly once.

It draws, and does not decide *when* to draw. A surface that changes without user input belongs to
[`windows-present`](https://github.com/microsoft/windows-rs/tree/master/crates/libs/present), and one
that changes on discrete events belongs in a retained composition tree, which the optional
`composition` feature draws into.
