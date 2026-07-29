# Metadata

Two kinds of file live here, and the difference matters before you edit either.

## Generated snapshots

`win32/`, `wdk/` and `winrt/` are **generated** — an RDL snapshot of the API surface scraped from
the Windows SDK headers and winmds, one file per defining header. Do not hand-edit them; see
[`win32/readme.md`](win32/readme.md). To add surface, add the header to the manifest in
`crates/tools/win32/src/main.rs` and re-run `cargo run -p tool_win32`.

## Hand-authored files

The `.rdl` files at this level are **transcriptions**, written and maintained by hand:

| File | Transcribes | Why it is not scraped |
|---|---|---|
| `metadata.rdl` | — | the vocabulary seed the scraper itself resolves against |
| `dispatcherqueue.rdl` | `dispatcherqueue.h` | the corpus carries the WinRT `DispatcherQueue` but not the Win32 call that mints a controller for a thread |
| `inputscope.rdl` | `InputScope.h` | the corpus scrapes `msctf.h` and `textstor.h`, so TSF is present but has no way to say what kind of text a field holds |
| `presentation.rdl` | `Presentation.h` | the composition swapchain namespace is absent entirely |
| `uiautomation.rdl` | `UiAutomationCoreApi.h`, `UIAutomationClient.h` | the corpus has UI Automation's types but neither the provider entry points nor the identifier constants |
| `windowsgraphicsinterop.rdl` | `Windows.Graphics.Interop.h` | the rest of the composition interop surface is scraped; this one interface is not |

**Why transcribe rather than extend the scrape.** Adding a header to `tool_win32`'s manifest is the
right mechanism when the surface belongs in the corpus, but re-running the scrape rewrites every
partition against whatever SDK and libclang are installed — tens of thousands of lines of churn,
across files nobody asked to change, to acquire a handful of symbols. A transcription keeps the
blast radius to exactly the symbols it names, and its diff is reviewable.

The trade is that a transcription can drift from the header it was taken from, and nothing detects
that. So each file states which header it transcribes and what it deliberately omits, and anything
it omits must resolve from the corpus rather than being redefined here — a duplicate definition is
what a filter naming it will hit, in the generated file rather than at the filter line.

`helpers::compile_authored_metadata` compiles these into one throwaway winmd under `target/`, which
the generators name alongside `default`. It is never committed: the `.rdl` is the source.
