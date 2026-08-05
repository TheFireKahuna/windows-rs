# Metadata

Two kinds of file live here: generated snapshots, which are rewritten by a tool, and hand-authored
transcriptions, which are edited directly.

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
| `syntheticinput.rdl` | `winuser.h` (Learn) | the 26100 SDK redacts the `CreateSyntheticPointerDevice2` types behind `Feature_TouchpadPublicApis3`, so the corpus that scrapes it has them nowhere to come from |
| `uiautomation.rdl` | `UiAutomationCoreApi.h`, `UIAutomationClient.h` | the corpus has UI Automation's types but neither the provider entry points nor the identifier constants |
| `windowsgraphicsinterop.rdl` | `Windows.Graphics.Interop.h` | the rest of the composition interop surface is scraped; this one interface is not |

**Why transcribe rather than extend the scrape.** Adding a header to `tool_win32`'s manifest is the
right mechanism when the surface belongs in the corpus. Re-running the scrape, though, rewrites
every partition against whatever SDK and libclang are installed, so acquiring a handful of symbols
costs tens of thousands of changed lines across unrelated files. A transcription changes only the
symbols it names.

A transcription can drift from the header it was taken from, and nothing detects that. Each file
therefore names the header it transcribes and what it omits. What it omits must resolve from the
corpus rather than being redefined here: a filter naming both winmds hits a duplicate definition,
and the error surfaces in the generated file rather than at the filter line.

`helpers::compile_authored_metadata` compiles these into one winmd under `target/`, which the
generators name alongside `default`. That winmd is never committed; the `.rdl` files are the source.
