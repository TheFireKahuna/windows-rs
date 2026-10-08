# windows-census

Named counters placed inside the graphics wrappers (`windows-composition`,
`windows-d2d`, `windows-text`, `windows-present`, `windows-scene`), so every object
created and every draw call issued through them is counted where it happens, and a
caller cannot reach the API without passing one.

```rust
# let (n, v) = (3, 7);
windows_census::count!("d2d.path");          // one more
windows_census::count!("d2d.sprite", n);     // n more
windows_census::level!("scene.visuals", v);  // the value now
windows_census::total!("scene.props", v);    // a running total kept elsewhere
let _span = windows_census::span!("d2d.end_draw"); // wall time to the end of the scope
```

Without the `enabled` feature each macro evaluates its value argument and nothing
else, and a span reads no clock. With it, the first hit of a site registers it and
every hit is one relaxed atomic add or store; a span adds a clock read at each end. [`read`] reports every site hit so far.
