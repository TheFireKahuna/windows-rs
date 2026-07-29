## Windows Present

Windows Present owns the composition swapchain: an `IPresentationManager`, the buffer pool it
retires, and the regions that draw into it. A region is the path for content that changes without
user input — a meter, a spectrum, an animation the compositor cannot express — and the buffers it
presents can reach a display plane directly, leaving DWM out of the loop.

Regions on one presentation manager share a present, which costs them independent flip; a region
that must flip owns its own. Buffers are ordinary Direct3D 11 textures, shared and displayable, so
a drawing crate targets them through its own device.
