//! `style.rs` — the URI-launch seam a `HyperlinkButton` activation routes
//! through.
//!
//! The property under test is mostly a *negative* one, and negative properties
//! are the ones that rot silently: **a hyperlink with no installed launcher
//! must do nothing.** Not fall back to `ShellExecute`, not log-and-open — this
//! crate has no URI-launch primitive of its own and must not grow one by
//! accident. A regression that added a default would break no other test in the
//! suite, because every other test would keep passing while the app silently
//! gained a process-level launch it never asked for.
//!
//! The launcher is a process-global `OnceLock`, first-registration-wins, so
//! "before install" and "after install" are not two states this binary can be
//! in twice. The whole contract is therefore ONE sequenced test rather than
//! several that would race each other for the one slot — a parallel pair here
//! would pass or fail depending on thread scheduling, which is worse than not
//! testing it.
//!
//! What is NOT here: the activation routing itself (`input::activate` offering
//! the URI to this seam). That needs a live window and message pump — `post_ui`
//! is a `PostMessageW` — so it is not headless. What this file pins is the
//! policy seam that routing calls into, which is where every decision lives;
//! the routing side is one call at one site, on the path a click, a Space press
//! and a UIA `Invoke` already share.

use std::sync::Mutex;

use windows_reactor::{launch_uri, set_uri_launcher, uri_launcher_installed};

/// Every URI the installed launcher was handed, in order. A launcher that is
/// never called leaves this empty — which is how the structural-rejection cases
/// below prove the rejection happened BEFORE the app saw the string, rather
/// than the app happening to decline it.
static SEEN: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn seen() -> Vec<String> {
    SEEN.lock().unwrap().clone()
}

/// The default is inert, installation is observable, and the installed hook
/// both decides and acts.
///
/// One test, run in order, because the `OnceLock` makes the transition
/// one-way.
#[test]
fn the_uri_launch_seam_is_inert_until_the_app_installs_a_launcher() {
    // ── Before install: inert, and visibly so ────────────────────────────
    //
    // `uri_launcher_installed` is the observable an app uses to render a link
    // it cannot follow as plain text instead of a dead affordance.
    assert!(
        !uri_launcher_installed(),
        "a launcher was installed before this test ran — nothing in the crate \
         may install one on the app's behalf"
    );
    assert!(
        !launch_uri("https://newapo.dev"),
        "a URI was reported as handled with no launcher installed: the default \
         must be to do nothing, not to fall back to the shell"
    );
    assert!(
        seen().is_empty(),
        "something was invoked with no launcher installed"
    );

    // ── Install ──────────────────────────────────────────────────────────
    assert!(
        set_uri_launcher(|uri| {
            SEEN.lock().unwrap().push(uri.to_string());
            // Decide as well as act: the seam must carry a decline back.
            !uri.starts_with("file:")
        }),
        "the first registration must report that it won the slot"
    );
    assert!(
        uri_launcher_installed(),
        "installing a launcher is not observable"
    );

    // ── The hook decides AND acts ────────────────────────────────────────
    assert!(launch_uri("https://newapo.dev"), "an accepted URI reported unhandled");
    assert_eq!(seen(), vec!["https://newapo.dev"], "the URI did not reach the launcher");

    // A decline is a normal outcome, not an error, and nothing else in the
    // crate picks the URI up afterwards.
    assert!(
        !launch_uri("file:///C:/Windows/System32/cmd.exe"),
        "a declined URI reported handled"
    );
    assert_eq!(
        seen().len(),
        2,
        "the declined URI should still have been OFFERED — deciding is the \
         app's job, and it cannot decide what it is not shown"
    );

    // ── What the crate deliberately does NOT judge ───────────────────────
    //
    // The scheme, the authority and the encoding all pass through verbatim.
    // These are exactly the strings a scheme allow-list would reject, and the
    // point is that this crate does NOT reject them: baking that policy in
    // here would be a decision no app could override.
    SEEN.lock().unwrap().clear();
    let passed_through = [
        "ms-settings:sound",
        "shell:AppsFolder",
        "javascript:alert(1)",
        "vscode://file/C:/x",
        "HTTPS://EXAMPLE.COM/../../%2e%2e/etc",
        "unregistered-scheme:whatever",
        // Interior U+0020 stays legal. It is visible, so the app can see it and
        // decide, and `file:///C:/Program Files/…` is a real thing apps launch.
        "not a uri at all, just words",
        // Percent-encoding is NOT decoded here, so these are not control
        // characters yet and the gate has no basis to reject them. An app that
        // decodes owns re-checking the decoded form.
        "https://newapo.dev/%00",
        "https://newapo.dev/%0Ainjected",
        // Punycode/homoglyph authorities are well-formed URI references. Which
        // host you meant is not a structural question.
        "https://xn--80ak6aa92e.com/",
    ];
    for uri in passed_through {
        launch_uri(uri);
    }
    assert_eq!(
        seen().as_slice(),
        passed_through.as_slice(),
        "the crate filtered a URI on its own judgement — scheme policy belongs \
         to the app, verbatim, in the order offered"
    );

    // ── What the crate DOES reject, and why it is not policy ─────────────
    //
    // Only strings that are not a URI reference under RFC 3986 no matter whose
    // policy applies: nothing to launch, or a control character (NUL, CR, LF —
    // the bytes that turn one "URI" into two arguments or two log lines).
    // These must not reach the launcher at all.
    SEEN.lock().unwrap().clear();
    for bad in [
        "",
        "   ",
        "\t\n",
        "https://newapo.dev\0",
        "https://newapo.dev\r\nHost: elsewhere",
        "https://newapo.dev\u{85}",
        // Leading/trailing whitespace is REJECTED, not trimmed. Testing
        // `trim()` for emptiness and then handing over the untrimmed string
        // let the app's allow-list read a different string from the one the
        // shell would parse: this exact URI cleared the old gate, and the
        // launcher installed above — screening with `!uri.starts_with("file:")`
        // — reported it handled.
        "  file:///C:/Windows/System32/cmd.exe",
        "https://newapo.dev ",
        "\u{feff}file:///C:/Windows/System32/cmd.exe",
        // Line terminators from outside category Cc. `char::is_control` is Cc
        // ONLY, so these cleared a gate whose whole stated purpose was to stop
        // one URI becoming two log lines.
        "https://newapo.dev/\u{2028}console.log(1)",
        "https://newapo.dev/\u{2029}next",
        // Separators to anything tokenising on Unicode whitespace, and all but
        // invisible in the string a reviewer read.
        "https://newapo.dev/\u{a0}--flag",
        "https://newapo.dev/\u{3000}arg",
        // Render-vs-parse divergence: what the user sees is not what launches.
        "https://newapo.dev/\u{202e}kcatta",
        "https://newapo.dev\u{200b}.evil.test/",
        "https://newapo.dev/\u{ad}soft",
    ] {
        assert!(
            !launch_uri(bad),
            "a structurally invalid URI reported handled"
        );
    }
    assert!(
        seen().is_empty(),
        "a structurally invalid URI was handed to the app's launcher"
    );

    // ── First registration wins ──────────────────────────────────────────
    //
    // Matches `set_window_visibility_callback` / `set_display_change_callback`:
    // a later call cannot silently swap out the policy an app installed at
    // startup.
    // First-wins is the fail-safe direction: a component loaded later cannot
    // silently swap out the policy the app installed at startup. But losing is
    // only safe if the loser can TELL — `uri_launcher_installed` answers `true`
    // either way, so without a return value an app would render links as
    // followable believing its own allow-list was in force while someone else's
    // decided where they went.
    SEEN.lock().unwrap().clear();
    assert!(
        !set_uri_launcher(|_| panic!("the second registration replaced the first")),
        "a discarded registration reported success — losing the slot must be \
         observable, or an app cannot tell whose policy is in force"
    );
    assert!(launch_uri("https://newapo.dev"));
    assert_eq!(seen(), vec!["https://newapo.dev"]);
}
