# windows-present

> The composition swapchain: a presentation manager, its buffer pool, and the regions that draw into it.

- Not published to crates.io
- [Getting started](../../crates/libs/present/readme.md)
- [Source](https://github.com/microsoft/windows-rs/tree/master/crates/libs/present)

`windows-present` owns `IPresentationManager` and nothing else — it is the path for content that
changes without user input, where a buffer can reach a display plane directly and leave DWM out of
the loop for most refreshes.

Buffers are ordinary Direct3D 11 textures, shared and displayable, so a drawing crate targets them
through its own device. Regions sharing one presentation manager also share a present, which costs
them independent flip; a region that must flip owns its own manager.

The presentation namespace is not in the vendored Win32 metadata and is compiled from
`metadata/presentation.rdl`, whose header records the vtable-order provenance.
