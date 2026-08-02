## Windows Present

Presentation regions: the path for content that changes **without user input** — a meter, a
spectrum, a scrolling history. Everything else, however visually rich, changes only when the user
edits or drags it, which is event rate, and belongs in a retained visual tree where it is free
between events, hit-testable and in the accessibility tree.

The reason to move is the compositor's dirty-region rule: a dirty region drags *every* visual it
intersects into the draw pass, regardless of how few of them moved. Measured on an 85-bar spectrum
analyzer, one publish cost ~170 visual draws at ~5.6 µs each, and neither the plot's area nor the
number of bars that changed moved that figure. A presented buffer collapses the whole rectangle to
one visual holding one texture, which the compositor samples and never looks inside: DWM's share of
a core went **9.47% to 1.18%**.

What it trades is that the content is pixels — not hit-testable, carrying no automation, and
untouchable by a compositor animation — and that it costs `depth + 2` full-size buffers at 8 bytes
a pixel.

### The shape

```rust,ignore
let presenter = Presenter::spawn(Tuning::default(), output_transform, Box::new(bind))?;
presenter.mount(
    RegionSpec { key, queue: Queue::Solo, extent },
    epoch.clone(),
    input.clone(),
    move |gpu| Ok(Box::new(Analyzer::new(gpu)?)),   // runs ON the present thread
);
```

A `Frame` is the framework's entire knowledge of a consumer, and **it carries no data types** — no
source trait, no facet union. A renderer closes over its own reader and registers an `Epoch` as its
wake source, so adding a consumer is zero framework edits.

### What the thread does with a wake

It draws `depth` frames per wake rather than one, inside **one** Direct2D bracket, and presents each
into its own scheduled slot. At one frame per wake the present and the bracket were 87% of a
compositor frame before anything was drawn, and nearly all of that is fixed per pass, so it divides:
7.12% of a core at depth 1, **5.00% at 3**. Three separate mechanisms pay for it — amortizing the
bracket (79.82 µs a call to 10.97), scheduling each present into its slot rather than asking for "as
early as possible" (~10% of a present), and declining the VSync interrupt on every present but the
one the next pass waits behind (~26 µs of CPU each).

Nothing binds until everything has drawn, and that is not a convention: `submit` takes a `Flushed`,
whose only constructor closes the pass.

A steady producer at display rate posts **nothing** to the front thread.
