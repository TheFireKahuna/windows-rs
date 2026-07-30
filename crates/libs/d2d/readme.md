## Windows Direct2D

Windows Direct2D provides the Direct2D drawing stack: a `Gpu` pairing a Direct3D 11 device with a
Direct2D device, the drawing bracket and the targets bound inside it, path geometry and geometry
realizations, brushes, layers, sprite batches, and device-loss classification.

Four things shape the whole surface:

**The device *is* the context.** `Gpu` owns the one `ID2D1DeviceContext` and there is no way to make
another. A second context on the same device costs a second `BeginDraw` and a Direct3D
device-context-state swap every time it draws, so a cached intermediate is rendered by retargeting
the open bracket rather than by having a context of its own.

**The bracket is a value, and it is exclusive.** `Pass` is RAII over `BeginDraw`/`EndDraw` and spans
however many targets a pass touches; `Pass::draw` takes `&mut self`, so exactly one target is bound
at a time and an unpopped layer cannot cross a retarget. `Pass::end` reports the tag that was active
when a call latched an error — which, because one error discards every later draw in the bracket, is
the only way to tell a wrong draw from a draw killed by an earlier one.

**One place mints a target, and no signature names a format.** Every render target is FP16
(`DXGI_FORMAT_R16G16B16A16_FLOAT`); the alpha mode comes from an `Opacity` enum named after what the
content does rather than after the API. So colour stays in linear scRGB, values above paper white and
outside Rec.709 survive to the compositor, and a surface allocation cannot be the thing that loses
them.

**There is no colour type here.** Brushes and gradient stops take a `windows_color::Scrgb`, which is
layout-compatible with `D2D1_COLOR_F` and reachable only from the output transform — so a
scene-referred value cannot reach Direct2D without being transformed, and cannot be transformed
twice.

It draws; it does not decide *when* to draw. A surface that changes without user input belongs to
[`windows-present`](https://github.com/microsoft/windows-rs/tree/master/crates/libs/present); one
that changes on discrete events belongs in a retained composition tree, which the optional
`composition` feature draws into.
