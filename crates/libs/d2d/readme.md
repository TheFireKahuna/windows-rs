## Windows Direct2D

Windows Direct2D provides the Direct2D drawing stack: a `GpuDevice` pairing a Direct3D 11 device
with a Direct2D 1.3 device, an RAII drawing session, path geometry and geometry realizations,
brushes, bitmap targets and sprite batches, and device-loss classification.

All render targets are FP16 (`DXGI_FORMAT_R16G16B16A16_FLOAT`) with premultiplied alpha, so colour
stays in linear scRGB and values above paper white survive to the compositor.

It draws; it does not decide *when* to draw. A surface that changes without user input belongs to
[`windows-present`](https://github.com/microsoft/windows-rs/tree/master/crates/libs/present); one
that changes on discrete events belongs in a retained composition tree.
