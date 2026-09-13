//! Thirteen source rules over the framework crates, each stating what the source may not
//! contain.
//!
//! A rule matches the hand-written source with comments and `#[cfg(test)]` modules blanked
//! out, so it sees production code only. Each rule prints every hit it finds.

use lints::{Source, deny, framework, generated, root};

/// Collects every hit of `needle` outside `allow`, formatted for a failure message.
fn hits(sources: &[Source], needle: &str, allow: &[&str]) -> Vec<String> {
    sources
        .iter()
        .filter(|source| !source.under(allow))
        .flat_map(|source| {
            source
                .find(needle)
                .into_iter()
                .map(move |(line, text)| format!("  {}:{line}: {text}", source.path))
        })
        .collect()
}

/// Collects the hits of every needle in `needles`, outside `allow`.
fn all(sources: &[Source], needles: &[&str], allow: &[&str]) -> Vec<String> {
    needles
        .iter()
        .flat_map(|needle| hits(sources, needle, allow))
        .collect()
}

// ── 1 ───────────────────────────────────────────────────────────────────────────

#[test]
fn no_timers() {
    // The framework runs no clock of its own: continuity comes from a compositor
    // animation, an interaction tracker or a presentation region. A timer wakes the front
    // thread whether or not anything moved, and the front thread costs nothing at idle.
    let sources = framework();
    let found = all(
        &sources,
        &[
            "SetTimer(",
            "CreateTimerQueueTimer",
            "DispatcherQueueTimer",
            "SetWaitableTimer",
            "thread::sleep",
            "sleep(Duration",
        ],
        &[],
    );
    deny(
        "no_timers",
        "the framework has no clock of its own: a tick comes from the compositor, a \
         tracker or a present, and a timer wakes whether or not anything moved",
        &found,
    );
}

// ── 2 ───────────────────────────────────────────────────────────────────────────

#[test]
fn no_color_brush() {
    // An 8-bit `Windows.UI.Color` carries no negative component and no value above white,
    // so a wide-gamut or specular colour is clamped at that boundary. Colour reaches the
    // compositor as FP16 surface content; the one permitted colour brush is the opaque
    // white coverage source a mask multiplies.
    let sources = framework();
    let found = hits(
        &sources,
        "create_color_brush",
        &[
            // Defines the wrapper, which is 1:1 with the platform surface.
            "crates/libs/composition/src/compositor.rs",
            // The one call site: `mask_brush`'s white coverage source, which carries no
            // colour.
            "crates/libs/scene/src/bind.rs",
        ],
    );
    deny(
        "no_color_brush",
        "an 8-bit colour brush cannot carry a negative component or a value above white; \
         colour reaches the compositor as FP16 surface content",
        &found,
    );
}

// ── 3 ───────────────────────────────────────────────────────────────────────────

#[test]
fn d2d_buffer_precision() {
    // Direct2D splits an effect graph into sections and gives no guarantee about where it
    // places an intermediate texture. An intermediate defaults to limited range, which
    // clamps the extended-range values the graph carries. The precision is a property of
    // the device context, set once where the context is made, so the rule has two halves:
    // every file that creates a context sets 16BPC_FLOAT on it, and an effect is
    // constructed only inside the crate that makes those contexts, so no effect can run on
    // a context that skipped the first half.
    let sources = framework();
    let mut found: Vec<String> = sources
        .iter()
        .filter(|source| !source.find("CreateDeviceContext").is_empty())
        .filter(|source| {
            source.find("SetRenderingControls").is_empty()
                || source.find("D2D1_BUFFER_PRECISION_16BPC_FLOAT").is_empty()
        })
        .map(|source| format!("  {}: creates a device context without 16BPC_FLOAT", source.path))
        .collect();
    found.extend(
        sources
            .iter()
            .filter(|source| !source.path.starts_with("crates/libs/d2d/src/"))
            .filter(|source| {
                !source.find("CreateEffect").is_empty() || !source.find("ID2D1Effect").is_empty()
            })
            .map(|source| format!("  {}: constructs a D2D effect outside windows-d2d", source.path)),
    );
    deny(
        "d2d_buffer_precision",
        "an effect graph runs at the precision its context was given, so a context is made          at 16BPC_FLOAT and an effect is made only on one of those",
        &found,
    );
}

// ── 4 ───────────────────────────────────────────────────────────────────────────

#[test]
fn no_scrgb_construction() {
    // `OutputTransform::apply` is the only supplier of a display-referred value, so the
    // display transform runs exactly once per colour. Constructing an `Scrgb` by hand
    // skips it.
    //
    // Three exemptions inside `windows-scene`, each a value that has already been through
    // the transform or is not a colour:
    //
    // - `quant.rs` re-materializes a transformed value from its quantized key;
    // - `cache.rs`'s white is the mask brush's coverage source;
    // - `backends.rs` builds that same white as a solid.
    //
    // The second assertion below is stricter and covers the widget layer, where a role
    // resolves to authored light and a display-referred colour has no meaning.
    let sources = framework();
    let allow = [
        "crates/libs/color/",
        "crates/libs/scene/src/quant.rs",
        "crates/libs/scene/src/cache.rs",
        "crates/libs/scene/src/backends.rs",
    ];
    // `-> Scrgb {` opens a function that returns one, which is not a construction.
    let found: Vec<String> = all(&sources, &["Scrgb {", "Scrgb::new"], &allow)
        .into_iter()
        .filter(|hit| !hit.contains("-> Scrgb"))
        .collect();
    deny(
        "no_scrgb_construction",
        "an Scrgb comes from OutputTransform::apply and from nowhere else, which is what \
         makes the display transform run exactly once by construction",
        &found,
    );

    let above: Vec<&Source> = sources
        .iter()
        .filter(|source| source.under(&["crates/libs/ui/"]))
        .collect();
    let found: Vec<String> = above
        .iter()
        .flat_map(|source| {
            ["Scrgb {", "Scrgb::new", "Scrgb"]
                .iter()
                .flat_map(move |needle| {
                    source
                        .find(needle)
                        .into_iter()
                        .map(move |(line, text)| format!("  {}:{line}: {text}", source.path))
                })
        })
        .collect();
    deny(
        "no_scrgb_construction",
        "nothing above the scene may even name a display-referred colour: a role resolves \
         to authored light",
        &found,
    );
}

// ── 5 ───────────────────────────────────────────────────────────────────────────

#[test]
fn wndproc_is_doorbell() {
    // A pointer message's handler signals the frame-clock consumer and returns: it does
    // not hit-test, walk the tree or allocate application containers. The platform's
    // discrete PointerPoint capture must happen while its message is current. Hover
    // constructs no point and costs (pointer moves × tree size),
    // and only the frame clock bounds the number of moves that reach the tree.
    let sources = framework();
    let found: Vec<String> = sources
        .iter()
        .flat_map(|source| {
            let procs = source
                .find("fn wndproc")
                .into_iter()
                .chain(source.find("fn window_proc"));
            procs.flat_map(move |(line, _)| {
                let body = body_after(source, line);
                [
                    "Vec::new",
                    "Vec::with_capacity",
                    "String::",
                    "Box::new",
                    ".hit(",
                ]
                .iter()
                .filter(|needle| body.contains(**needle))
                .map(move |needle| {
                    format!(
                        "  {}:{line}: the wndproc body contains {needle}",
                        source.path
                    )
                })
                .collect::<Vec<_>>()
            })
        })
        .collect();
    deny(
        "wndproc_is_doorbell",
        "a pointer arm captures its message and returns; hit testing and application \
         allocation belong to the service tick",
        &found,
    );
}

/// Returns the lines of `source` from `line` up to the next unindented line opening an
/// item.
///
/// The scan ends at the first line that starts with `}`, `p`, `f` or `#` and is not
/// indented, so it can run past one function to the end of the enclosing block. Callers
/// ask only whether a token appears anywhere in that span.
fn body_after(source: &Source, line: usize) -> String {
    source
        .code
        .lines()
        .skip(line)
        .take_while(|text| !text.starts_with(['}', 'p', 'f', '#']) || text.starts_with("    "))
        .collect::<Vec<_>>()
        .join("\n")
}

// ── 6 ───────────────────────────────────────────────────────────────────────────

#[test]
fn patch_is_send() {
    // The patch is the one downward channel from the app thread to the front thread. A
    // generated COM interface is `!Send`, so the const assertions that `SinkPatch` and
    // `Op` are `Send` are what prove no interface crossed it. A variable-length payload
    // travels as a span into a typed side-buffer rather than as an owned collection in a
    // variant, which keeps every op `Copy`.
    let root = root();
    let path = root.join("crates/libs/scene/src/patch.rs");
    let raw = std::fs::read_to_string(&path).expect("windows-scene has a patch module");
    let code = lints::strip_tests(&lints::strip_comments(&raw));

    assert!(
        code.contains("assert_send::<SinkPatch>()"),
        "patch_is_send — patch.rs must carry the const assertion that SinkPatch is Send"
    );
    assert!(
        code.contains("assert_send::<Op>()"),
        "patch_is_send — patch.rs must carry the const assertion that Op is Send"
    );

    let ops = code
        .split_once("pub enum Op {")
        .expect("patch.rs declares `pub enum Op`")
        .1;
    let ops = &ops[..ops.find("\n}").expect("the enum closes")];
    let found: Vec<String> = ["Vec<", "String", "Box<"]
        .iter()
        .filter(|owned| ops.contains(**owned))
        .map(|owned| format!("  an Op variant names {owned}"))
        .collect();
    deny(
        "patch_is_send",
        "every op is Copy and every variable-length payload is a Span into a side-buffer",
        &found,
    );
}

// ── 7 ───────────────────────────────────────────────────────────────────────────

#[test]
fn no_widget_colors() {
    // A widget builder accepts a role and a variant. It exposes no setter for a colour, a
    // font size, an alignment or a bare `f32`, so a widget cannot carry styling the theme
    // does not resolve.
    let sources = framework();
    let widgets: Vec<&Source> = sources
        .iter()
        .filter(|source| source.under(&["crates/libs/ui/src/widget/"]))
        .collect();
    let found: Vec<String> = widgets
        .iter()
        .flat_map(|source| {
            source
                .code
                .lines()
                .enumerate()
                .filter(|(_, text)| text.contains("pub fn "))
                .filter(|(_, text)| {
                    ["Radiance", "Scrgb", "font_size", "Align", ": f32) -> Self"]
                        .iter()
                        .any(|banned| text.contains(banned))
                })
                .map(move |(i, text)| format!("  {}:{}: {}", source.path, i + 1, text.trim()))
        })
        .collect();
    deny(
        "no_widget_colors",
        "a widget accepts a role and a variant, never a colour, a font size, a spacing or \
         an alignment",
        &found,
    );
}

// ── 8 ───────────────────────────────────────────────────────────────────────────

#[test]
fn no_child_layout() {
    // Gap, placement and track sizing belong to the container, which states them on the
    // child's behalf through `at`, `span`, `rows` and `cols`. A child-side setter for one
    // of them raises no diagnostic when the parent cannot honour it: `grid_row` on a flex
    // child writes a value nothing reads.
    //
    // `align_self` is absent from the needle list and is the one per-child layout
    // property. Every container class here honours cross-axis alignment, so it cannot
    // write a value nothing reads.
    let sources = framework();
    let found = all(
        &sources,
        &[
            "fn grid_row",
            "fn grid_column",
            "fn justify_self",
            "fn horizontal_alignment",
            "fn vertical_alignment",
        ],
        &["crates/libs/scene/"],
    );
    deny(
        "no_child_layout",
        "a layout property a child cannot honour belongs to the container, which states it \
         on the child's behalf",
        &found,
    );
}

// ── 9 ───────────────────────────────────────────────────────────────────────────

#[test]
fn slot_roots_closed() {
    // A parentless root is invisible to a parent-walk disposal, so an unmount does not
    // reach it. `orphan_group` is the only way to mint one and the overlay layer is its
    // only caller; a second call site would leave the walk non-exhaustive.
    let sources = framework();
    let found: Vec<String> = sources
        .iter()
        .filter(|source| source.path != "crates/libs/scene/src/model.rs")
        .flat_map(|source| {
            source
                .find("orphan_group(")
                .into_iter()
                .map(move |(line, text)| format!("  {}:{line}: {text}", source.path))
        })
        .collect();
    assert!(
        found.len() <= 1,
        "\nslot_roots_closed — a parentless root is minted in more than one place, so the \
         disposal walk cannot be exhaustive\n\n{}\n",
        found.join("\n")
    );
}

// ── 10 ──────────────────────────────────────────────────────────────────────────

#[test]
fn caption_from_hit_array() {
    // The drag strip is derived from the hit array rather than declared as a second rect.
    // A second rect drifts out of agreement with the controls inside it, and the title bar
    // then drags when a button is pressed.
    let sources = framework();
    let found: Vec<String> = sources
        .iter()
        .flat_map(|source| {
            source
                .find("WM_NCHITTEST")
                .into_iter()
                .filter_map(|(line, _)| {
                    let body = body_after(source, line);
                    // Only an arm that answers the message is checked; a bare mention in a
                    // match list is not one.
                    //
                    // The needle is the call `.hit(`, not the name `hit`. `body_after` runs
                    // to the end of the enclosing block, so a bare `hit(` would also match
                    // the accessor's own definition further down and pass a caption arm
                    // that answered from a literal rect.
                    if body.contains("HTCAPTION") && !body.contains(".hit(") {
                        Some(format!(
                            "  {}:{line}: the caption arm does not resolve through Scene::hit",
                            source.path
                        ))
                    } else {
                        None
                    }
                })
        })
        .collect();
    deny(
        "caption_from_hit_array",
        "the caption's drag strip is the hit array's answer, not a literal rect",
        &found,
    );
}

// ── 11 ──────────────────────────────────────────────────────────────────────────

#[test]
fn no_generated_edits() {
    // The rule covers the exemption boundary: the set of files excused from every other
    // rule is exactly what the binding filters declare, and each declared file exists. A
    // file cannot join the set by being called `bindings.rs`, and a filter cannot stop
    // producing one unnoticed.
    //
    // Whether the committed contents still match the tools' output is a separate check. A
    // nested cargo invocation blocks on the same build directory, so it runs outside the
    // test:
    //
    //     cargo run -p tool_bindings && cargo run -p tool_composition && git diff --exit-code
    let root = root();
    let declared = generated();
    let missing: Vec<String> = declared
        .iter()
        .filter(|rel| !root.join(rel).is_file())
        .map(|rel| format!("  {rel}: declared by a filter but not present"))
        .collect();
    deny(
        "no_generated_edits",
        "every file the binding tools declare must exist, or the exemption covers nothing",
        &missing,
    );

    // Nothing hand-written may sit under a declared output's name.
    let audited = framework();
    let smuggled: Vec<String> = audited
        .iter()
        .filter(|source| declared.contains(&source.path))
        .map(|source| format!("  {}: audited and generated at once", source.path))
        .collect();
    deny(
        "no_generated_edits",
        "the exempt set and the audited set must not overlap",
        &smuggled,
    );
}

// ── 12 ──────────────────────────────────────────────────────────────────────────

#[test]
fn no_reactor_dep() {
    // `windows-reactor` is the WinUI-hosting reconciler. Nothing in these crates hosts
    // XAML, and depending on it would add a second presentation model beside the
    // compositor scene.
    let root = root();
    let found: Vec<String> = lints::FRAMEWORK
        .iter()
        .filter_map(|crate_dir| {
            let manifest = root.join(crate_dir).join("Cargo.toml");
            let text = std::fs::read_to_string(&manifest).ok()?;
            text.lines()
                .find(|line| line.trim_start().starts_with("windows-reactor"))
                .map(|line| format!("  {crate_dir}/Cargo.toml: {}", line.trim()))
        })
        .collect();
    deny(
        "no_reactor_dep",
        "nothing in this stack hosts XAML, so nothing may depend on the reconciler that \
         does",
        &found,
    );
}

// ── 13 ──────────────────────────────────────────────────────────────────────────

/// A forbidden token, matched either anywhere it appears or only as a whole identifier.
enum Needle {
    /// Matched as a substring, so it carries whatever punctuation disambiguates it.
    Text(&'static str),
    /// Matched only where neither neighbour continues an identifier.
    Word(&'static str),
}

impl Needle {
    /// Returns every hit of this needle in `source`, as `(line, text)`.
    fn find(&self, source: &Source) -> Vec<(usize, String)> {
        match self {
            Self::Text(needle) => source.find(needle),
            Self::Word(needle) => source.find_word(needle),
        }
    }
}

/// The files the input thread and the scene thread own. Neither may reach the app half.
const THREAD_OWNED: [&str; 6] = [
    "crates/libs/ui/src/driver/input.rs",
    "crates/libs/ui/src/driver/scene.rs",
    "crates/libs/ui/src/input/",
    "crates/libs/ui/src/widget/state.rs",
    "crates/libs/ui/src/caption.rs",
    "crates/libs/ui/src/present/pick.rs",
];

/// What a thread-owned file may not name: the host, the signal graph, the build lowering,
/// the overlay stack, the model, the overlay table.
const THREAD_FORBIDDEN: [Needle; 6] = [
    Needle::Text("Host::"),
    Needle::Text("signal::"),
    Needle::Text("build::"),
    Needle::Text("overlay::"),
    Needle::Word("Model"),
    Needle::Word("Overlays"),
];

/// The files the app half owns. None may reach a compositor object, the scene, the
/// backends, the focus ring, the present registry or the front table.
const APP_OWNED: [&str; 4] = [
    "crates/libs/ui/src/build/",
    "crates/libs/ui/src/signal/",
    "crates/libs/ui/src/overlay/",
    "crates/libs/ui/src/driver/app.rs",
];

/// What an app-half file may not name.
///
/// `Scene` and `Front` are spelled with the punctuation that makes each a type rather than
/// a prefix, so `SceneEvent` and `FrontHandle` do not answer.
const APP_FORBIDDEN: [Needle; 9] = [
    Needle::Text("Scene::"),
    Needle::Text("&Scene"),
    Needle::Text("&mut Scene"),
    Needle::Text(": Scene"),
    Needle::Text("Backends"),
    Needle::Text("Compositor"),
    Needle::Text("FocusRing"),
    Needle::Text("REGISTRY"),
    Needle::Text("widget::Front"),
];

/// The clock the scene may not hold: its timing arrives with a frame, not from a read.
const SCENE_FORBIDDEN: [Needle; 2] = [
    Needle::Text("Instant::now"),
    Needle::Text("std::time::Instant"),
];

/// The files exempted while the thread split lands, each naming the unit that clears it.
///
/// Every entry names a file a rewrite already owns, so the exemption is an ordering rather
/// than a permission. `thread_ownership_allow_list_is_shrinking` fails on an entry whose
/// file has gone, which is what forces the list down as the units land.
const ALLOW_UNTIL_SPLIT: &[&str] = &[];

/// Collects every forbidden needle found in the sources under `files`, outside `allow`.
///
/// A `tests.rs` file is skipped. Each one is declared under `#[cfg(test)]` at its parent
/// module, so its whole body is a test module that `strip_tests` cannot reach, and a test
/// names the two halves it drives against each other.
fn owned(sources: &[Source], files: &[&str], needles: &[Needle], allow: &[&str]) -> Vec<String> {
    sources
        .iter()
        .filter(|source| source.under(files))
        .filter(|source| !source.under(allow))
        .filter(|source| !source.path.ends_with("/tests.rs"))
        .flat_map(|source| {
            needles.iter().flat_map(move |needle| {
                needle
                    .find(source)
                    .into_iter()
                    .map(move |(line, text)| format!("  {}:{line}: {text}", source.path))
            })
        })
        .collect()
}

#[test]
fn thread_ownership() {
    // `windows-ui` runs on three threads and the source is what keeps them apart. The input
    // thread and the scene thread service a window and a frame; neither may reach the host,
    // the signal graph, the build lowering or the overlay stack. The app half holds no
    // compositor object, no scene, no backends, no focus ring and no present registry, so
    // nothing it names carries thread affinity. `windows-scene` reads no clock: a delay is
    // due when the frame it was scheduled against arrives.
    //
    // A file that is not present is absent from the collection, so the rule reaches each of
    // the three driver modules the moment it lands.
    let sources = framework();

    let found = owned(
        &sources,
        &THREAD_OWNED,
        &THREAD_FORBIDDEN,
        ALLOW_UNTIL_SPLIT,
    );
    deny(
        "thread_ownership",
        "the input and scene threads service a window and a frame; the host, the signal \
         graph, the build lowering and the overlay stack live on the app thread",
        &found,
    );

    let found = owned(&sources, &APP_OWNED, &APP_FORBIDDEN, ALLOW_UNTIL_SPLIT);
    deny(
        "thread_ownership",
        "the app half names nothing carrying thread affinity: no compositor object, no \
         scene, no backends, no focus ring, no present registry and no front table",
        &found,
    );

    let found = owned(
        &sources,
        &["crates/libs/scene/src/"],
        &SCENE_FORBIDDEN,
        ALLOW_UNTIL_SPLIT,
    );
    deny(
        "thread_ownership",
        "the scene holds no clock: a delay is due when the frame it was scheduled against \
         arrives, not when a wall-clock read says so",
        &found,
    );
}

#[test]
fn thread_ownership_allow_list_is_shrinking() {
    // An exemption outlives the file it names only by being forgotten, so a path that has
    // gone fails here and the entry goes with it.
    let root = root();
    let stale: Vec<String> = ALLOW_UNTIL_SPLIT
        .iter()
        .filter(|rel| !root.join(rel).exists())
        .map(|rel| format!("  {rel}: exempted by thread_ownership but not present"))
        .collect();
    deny(
        "thread_ownership_allow_list_is_shrinking",
        "every path the thread-ownership exemption names must exist, or the exemption \
         covers nothing",
        &stale,
    );
}

#[test]
fn thread_ownership_matcher_bites() {
    // The rule's own coverage, over sources built in memory: an input-thread file reaching
    // the host is reported, and the names that merely begin with a forbidden one are not.
    let reaching = Source::new(
        "crates/libs/ui/src/input/service.rs",
        "fn dispatch() {\n    Host::with(|h| h.model().root());\n}\n",
    );
    let found = owned(
        &[reaching],
        &THREAD_OWNED,
        &THREAD_FORBIDDEN,
        ALLOW_UNTIL_SPLIT,
    );
    assert_eq!(
        found.len(),
        1,
        "\nthread_ownership — an input-thread file naming Host:: must be reported\n\n{}\n",
        found.join("\n")
    );

    let prefixes = Source::new(
        "crates/libs/ui/src/build/mount.rs",
        "fn take(events: &[SceneEvent], front: FrontHandle<ModelState>) {}\n",
    );
    let found = owned(&[prefixes], &APP_OWNED, &APP_FORBIDDEN, ALLOW_UNTIL_SPLIT);
    assert!(
        found.is_empty(),
        "\nthread_ownership — SceneEvent, FrontHandle and ModelState are not the forbidden \
         names\n\n{}\n",
        found.join("\n")
    );

    // A comment is blanked before matching, so prose that names a forbidden type does not
    // answer for the code beneath it.
    let commented = Source::new(
        "crates/libs/ui/src/signal/graph.rs",
        "// The Compositor holds the clock.\nfn arm() {}\n",
    );
    let found = owned(&[commented], &APP_OWNED, &APP_FORBIDDEN, ALLOW_UNTIL_SPLIT);
    assert!(
        found.is_empty(),
        "\nthread_ownership — a comment is not source\n\n{}\n",
        found.join("\n")
    );

    // `Model` and `Overlays` are word matches, so the app half's own types answer while a
    // longer name built from either does not.
    let named = Source::new(
        "crates/libs/ui/src/widget/state.rs",
        "fn sync(model: &mut Model, all: &Overlays) {}\n",
    );
    let found = owned(
        &[named],
        &THREAD_OWNED,
        &THREAD_FORBIDDEN,
        ALLOW_UNTIL_SPLIT,
    );
    assert_eq!(
        found.len(),
        2,
        "\nthread_ownership — Model and Overlays must both be reported\n\n{}\n",
        found.join("\n")
    );
}
