//! Twelve source rules over the framework crates, each stating what the source may not
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
    // clamps the extended-range values the graph carries, so a file that constructs an
    // effect also sets the buffer precision.
    let sources = framework();
    let found: Vec<String> = sources
        .iter()
        .filter(|source| {
            !source.find("CreateEffect").is_empty() || !source.find("ID2D1Effect").is_empty()
        })
        .filter(|source| source.find("SetRenderingControls").is_empty())
        .map(|source| format!("  {}: constructs a D2D effect", source.path))
        .collect();
    deny(
        "d2d_buffer_precision",
        "an effect graph without an explicit 16BPC_FLOAT buffer precision clamps its own \
         intermediates",
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
    // not hit-test, walk the tree or allocate. Hover costs (pointer moves × tree size),
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
        "a pointer arm rings a bell and returns; hit testing and allocation belong to the \
         frame-clock consumer",
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
