//! `backend/dcomp/mod.rs` — the prop contract: a set, then its `Unset`, must
//! leave the node exactly as it was born.
//!
//! The reconciler diffs a conditional prop away by sending `PropValue::Unset`.
//! If the backend ignores that, the node keeps the last value forever — a mode
//! toggle showing both segments accent-filled after a switch, a Button that
//! stays greyed out because the `IsEnabled` binding it lost was the only thing
//! ever setting it back.
//!
//! Restoring the value is only half the requirement; restoring the RIGHT value
//! is the other half, and the birth values here are pointedly not all zero: a
//! Button is born with a 6 DIP corner radius, 12/6 padding and a 30 DIP minimum
//! height, a ToggleSwitch with a 40×20 intrinsic box, text at a real size and
//! weight, `max` at 100, `is_active` true, `selected_index` and `content_align`
//! at -1, a NavigationView back button visible and its pane 320 DIP wide. A
//! reset written as "set it to zero" would pass a test that only checked the
//! value moved.
//!
//! So each case compares the FULL node digest against a freshly created node of
//! the same kind — one that has never seen the prop at all. That also catches a
//! reset which fixes its own field and disturbs a neighbour.
//!
//! **These tests are not headless** — see `arena_ids.rs`.

use windows_reactor::dcomp_test_api::ArenaHarness;
use windows_reactor::{Badge, Color, ControlKind as K, Prop, PropValue as V, Thickness};

fn harness() -> ArenaHarness {
    ArenaHarness::new().expect(
        "windowless Compositor unavailable — these tests assert nothing without it; \
         run them in an interactive session",
    )
}

fn thickness(v: f64) -> Thickness {
    Thickness {
        left: v,
        top: v,
        right: v,
        bottom: v,
    }
}

/// One case: a control kind, the props to put BOTH nodes into the same state
/// first, and then the prop-plus-value only the subject receives.
struct Case {
    kind: K,
    /// Applied to the reference node and the subject alike, so the comparison
    /// stays "identical but for this one prop" rather than "against a virgin".
    /// Empty for all but the props whose meaning depends on another.
    setup: Vec<(Prop, V)>,
    prop: Prop,
    value: V,
}

/// A case with no setup — the subject differs from the reference by one prop.
fn c(kind: K, prop: Prop, value: V) -> Case {
    Case {
        kind,
        setup: Vec::new(),
        prop,
        value,
    }
}

/// Every case below pairs a control kind with a value that is NOT that kind's
/// birth value for the prop.
///
/// The non-zero-birth props are all here, because those are the ones where a
/// plausible-looking reset is silently wrong. The rest are a representative
/// sweep across each storage tier — paint, Taffy style, the loose node fields,
/// `Ctrl`, and `Extras`.
fn cases() -> Vec<Case> {
    let mut cases: Vec<Case> = vec![
        // ── Paint, where the birth value is not the zero value ───────────
        c(K::Button, Prop::CornerRadius, V::F64(20.0)),
        c(K::Button, Prop::IsEnabled, V::Bool(false)),
        c(K::TextBlock, Prop::FontSize, V::F64(40.0)),
        c(K::TextBlock, Prop::FontWeight, V::U16(700)),
        c(K::ToggleSwitch, Prop::FontSize, V::F64(40.0)),
        // ── Paint, zero-valued ───────────────────────────────────────────
        c(K::Border, Prop::Background, V::Color(windows_reactor::Color::rgb(1, 2, 3))),
        c(K::Border, Prop::BorderThickness, V::Thickness(thickness(3.0))),
        c(K::TextBlock, Prop::FontFamily, V::Str("Consolas".into())),
        c(K::TextBlock, Prop::TextWrapping, V::I32(2)),
        c(K::TextBlock, Prop::Text, V::Str("hello".into())),
        c(K::Button, Prop::StyleVariant, V::I32(2)),
        c(K::Rectangle, Prop::Fill, V::Color(windows_reactor::Color::rgb(9, 9, 9))),
        c(K::Line, Prop::StrokeThickness, V::F64(4.0)),
        // ── Taffy style, where the birth style is not the default style ──
        c(K::Button, Prop::Padding, V::Thickness(thickness(40.0))),
        c(K::Button, Prop::MinHeight, V::F64(300.0)),
        c(K::ToggleSwitch, Prop::MinWidth, V::F64(300.0)),
        c(K::ToggleSwitch, Prop::MinHeight, V::F64(300.0)),
        c(K::CheckBox, Prop::MinWidth, V::F64(300.0)),
        c(K::Slider, Prop::MinHeight, V::F64(300.0)),
        c(K::Knob, Prop::MinWidth, V::F64(300.0)),
        c(K::NavigationView, Prop::Padding, V::Thickness(thickness(40.0))),
        c(K::Expander, Prop::Padding, V::Thickness(thickness(40.0))),
        c(K::TitleBar, Prop::Padding, V::Thickness(thickness(40.0))),
        c(K::TextBox, Prop::MinHeight, V::F64(300.0)),
        // ── Taffy style, zero-valued ─────────────────────────────────────
        c(K::Grid, Prop::Width, V::F64(100.0)),
        c(K::Grid, Prop::Height, V::F64(100.0)),
        c(K::Grid, Prop::MaxWidth, V::F64(100.0)),
        c(K::Grid, Prop::Margin, V::Thickness(thickness(5.0))),
        c(K::Grid, Prop::ColumnSpacing, V::F64(7.0)),
        c(K::Grid, Prop::RowSpacing, V::F64(7.0)),
        c(K::StackPanel, Prop::Orientation, V::I32(1)),
        c(K::StackPanel, Prop::Spacing, V::F64(9.0)),
        // Grid placement: birth is line 1 (XAML's overlapping cell 0,0), not
        // `auto`, which would auto-flow the child into the next free cell.
        c(K::Border, Prop::AttachedGridRow, V::I32(3)),
        c(K::Border, Prop::AttachedGridColumn, V::I32(3)),
        c(K::Border, Prop::AttachedGridRowSpan, V::I32(2)),
        c(K::Border, Prop::AttachedGridColumnSpan, V::I32(2)),
        // ── Loose node fields ────────────────────────────────────────────
        // -1 (unset), not 0 — 0 is a real WinRT `Left` / `Top`.
        c(K::Border, Prop::HorizontalAlignment, V::I32(1)),
        c(K::Border, Prop::VerticalAlignment, V::I32(1)),
        c(K::Border, Prop::AttachedCanvasZIndex, V::I32(4)),
        c(
            K::Grid,
            Prop::GridRows,
            V::GridLengths(vec![windows_reactor::GridLength::Star(1.0)]),
        ),
        // ── Ctrl, where the birth value is not the zero value ────────────
        c(K::Slider, Prop::Maximum, V::F64(5.0)),
        c(K::Meter, Prop::IsActive, V::Bool(false)),
        c(K::SelectorBar, Prop::SelectedIndex, V::I32(3)),
        c(K::TextBox, Prop::HorizontalContentAlignment, V::I32(1)),
        // ── Ctrl, zero-valued ────────────────────────────────────────────
        c(K::ToggleSwitch, Prop::IsOn, V::Bool(true)),
        c(K::CheckBox, Prop::IsChecked, V::Bool(true)),
        c(K::Slider, Prop::Minimum, V::F64(-5.0)),
        c(K::Slider, Prop::Value, V::F64(3.0)),
        c(K::Slider, Prop::Step, V::F64(0.5)),
        c(K::Slider, Prop::FillOrigin, V::F64(1.0)),
        c(K::Slider, Prop::FillColor, V::Color(windows_reactor::Color::rgb(4, 5, 6))),
        c(K::Meter, Prop::Marker, V::F64(2.0)),
        c(
            K::Meter,
            Prop::GradientStops,
            V::GradientStops(vec![(0.5, windows_reactor::Color::rgb(1, 1, 1))]),
        ),
        c(K::Knob, Prop::StartAngle, V::F64(1.0)),
        c(K::Knob, Prop::EndAngle, V::F64(2.0)),
        c(K::Knob, Prop::Ticks, V::F64List(vec![1.0, 2.0])),
        c(K::Knob, Prop::TickLabels, V::ValueLabels(vec![(1.0, "a".into())])),
        c(K::Knob, Prop::MajorEvery, V::F64(6.0)),
        c(K::Knob, Prop::Accent, V::Color(windows_reactor::Color::rgb(7, 8, 9))),
        c(K::Knob, Prop::Unit, V::Str("dB".into())),
        c(K::Knob, Prop::SubText, V::Str("x2".into())),
        c(K::ProgressBar, Prop::IsIndeterminate, V::Bool(true)),
        c(K::Expander, Prop::IsExpanded, V::Bool(true)),
        c(K::ComboBox, Prop::PlaceholderText, V::Str("pick".into())),
        c(K::ComboBox, Prop::Items, V::StrList(vec!["a".into(), "b".into()])),
        c(K::NavigationView, Prop::SelectedTag, V::Str("t".into())),
        c(K::NumberBox, Prop::LargeChange, V::F64(10.0)),
        // ── Extras, where the birth value is not the zero value ──────────
        c(K::NavigationView, Prop::IsBackButtonVisible, V::Bool(false)),
        c(K::NavigationView, Prop::IsPaneToggleButtonVisible, V::Bool(false)),
        c(K::NavigationView, Prop::IsSettingsVisible, V::Bool(false)),
        c(K::NavigationView, Prop::IsPaneOpen, V::Bool(false)),
        c(K::NavigationView, Prop::OpenPaneLength, V::F64(100.0)),
        c(K::NavigationView, Prop::PaneDisplayMode, V::I32(3)),
        c(K::ScrollViewer, Prop::HorizontalScrollBarVisibility, V::I32(3)),
        c(K::ScrollViewer, Prop::VerticalScrollBarVisibility, V::I32(3)),
        c(K::PasswordBox, Prop::IsPasswordRevealButtonEnabled, V::Bool(false)),
        c(K::PasswordBox, Prop::PasswordRevealMode, V::I32(2)),
        c(K::RepeatButton, Prop::Delay, V::I32(5)),
        c(K::RepeatButton, Prop::Interval, V::I32(1)),
        // ── Extras, zero-valued ──────────────────────────────────────────
        c(K::TitleBar, Prop::Title, V::Str("NewAPO".into())),
        c(K::TitleBar, Prop::Subtitle, V::Str("beta".into())),
        c(K::TitleBar, Prop::Tall, V::Bool(true)),
        c(K::TitleBar, Prop::IsBackButtonEnabled, V::Bool(true)),
        c(K::NavigationView, Prop::IsBackEnabled, V::Bool(true)),
        c(K::NavigationView, Prop::PaneTitle, V::Str("Chains".into())),
        c(K::NavigationView, Prop::AutoSuggestBox, V::Bool(true)),
        c(
            K::NavigationView,
            Prop::AutoSuggestItems,
            V::StrList(vec!["one".into()]),
        ),
        c(
            K::NavigationView,
            Prop::AutoSuggestPlaceholder,
            V::Str("search".into()),
        ),
        c(K::Button, Prop::Icon, V::I32(42)),
        // Every field of a `Badge` set away from its default at once, so a
        // reset that restores the struct but keeps, say, the tint is caught.
        c(
            K::Button,
            Prop::Badge,
            V::Badge(
                Badge::count(7).tint(Color::rgb(255, 0, 128)).leading(),
            ),
        ),
        // The dot form separately: its `count: None` is the same value the
        // struct's absence would produce if the reset stored `Some(default)`
        // instead of `None`.
        c(K::Button, Prop::Badge, V::Badge(Badge::dot())),
        // `Prop::FlyoutContent` is deliberately absent: a flyout does not
        // travel as a prop on this backend. It splits at the record seam into
        // a `Cmd::SetFlyout` declaration plus an app-side entry, so its set /
        // clear round trip is covered by `flyout.rs`, not here.
        c(K::Button, Prop::FlyoutPlacement, V::I32(3)),
        c(K::HyperlinkButton, Prop::NavigateUri, V::Str("https://newapo.dev".into())),
        c(K::ToggleSwitch, Prop::OnContent, V::Str("On".into())),
        c(K::ToggleSwitch, Prop::OffContent, V::Str("Off".into())),
        c(K::ComboBox, Prop::IsEditable, V::Bool(true)),
        c(K::TextBox, Prop::AcceptsReturn, V::Bool(true)),
        c(K::TextBlock, Prop::IsTextSelectionEnabled, V::Bool(true)),
    ];

    // `Precision` only means anything on a NumberBox that HAS a value: the set
    // arm re-formats the seeded text, so a bare Precision write would seed
    // "0.0000" into a field that never had a value and no reset of Precision
    // could honestly undo that — the buffer belongs to `Value`. Give both
    // nodes the value first, and the case then asks the real question: does
    // dropping Precision put the text back to the default 2 digits?
    cases.push(Case {
        kind: K::NumberBox,
        setup: vec![(Prop::Value, V::F64(3.0))],
        prop: Prop::Precision,
        value: V::I32(4),
    });
    cases
}

/// Set a prop, then `Unset` it, and land back on a virgin node of that kind.
///
/// This is the whole contract in one assertion, and it is stated against a
/// SEPARATE freshly-created node rather than against a snapshot of the same
/// node, so "restore what it was" and "restore what a node of this kind is
/// born with" cannot quietly become different claims.
#[test]
fn setting_a_prop_then_unsetting_it_restores_the_birth_state() {
    let mut a = harness();
    for Case {
        kind,
        setup,
        prop,
        value,
    } in cases()
    {
        let reference = a.insert(kind).unwrap();
        let node = a.insert(kind).unwrap();
        for (p, v) in &setup {
            a.apply_prop(reference, *p, v);
            a.apply_prop(node, *p, v);
        }
        let born = a.node_digest(reference).unwrap();

        a.apply_prop(node, prop, &value);
        assert_ne!(
            a.node_digest(node).unwrap(),
            born,
            "{kind:?} / {prop:?}: the test value did not change anything, so the \
             reset below would pass without doing a thing — pick a value that \
             differs from the birth value"
        );

        a.apply_prop(node, prop, &V::Unset);
        assert_eq!(
            a.node_digest(node).unwrap(),
            born,
            "{kind:?} / {prop:?}: Unset left the node different from one that \
             never received the prop"
        );
    }
}

/// Unsetting a prop on a node that never had it is a no-op — and specifically
/// must not ALLOCATE the state it is resetting.
///
/// `Ctrl` and its `Extras` are lazily boxed precisely because most nodes hold
/// neither, and an absent box already READS the birth value. A reset that went
/// through the write accessor would materialise hundreds of bytes to store a
/// value that is already in effect, on every node the reconciler happens to
/// diff a prop away from — quietly undoing the lazy box for the busiest trees.
#[test]
fn unsetting_a_prop_never_materializes_the_state_it_resets() {
    let mut a = harness();
    for Case { kind, prop, .. } in cases() {
        let id = a.insert(kind).unwrap();
        let born = a.node_digest(id).unwrap();

        a.apply_prop(id, prop, &V::Unset);

        assert_eq!(
            a.ctrl_allocated(id),
            Some(false),
            "{kind:?} / {prop:?}: an Unset on a virgin node allocated a Ctrl"
        );
        assert_eq!(
            a.extras_allocated(id),
            Some(false),
            "{kind:?} / {prop:?}: an Unset on a virgin node allocated an Extras"
        );
        assert_eq!(
            a.node_digest(id).unwrap(),
            born,
            "{kind:?} / {prop:?}: an Unset on a virgin node changed it"
        );
    }
}

/// The lazy `Extras` box behaves like the `Ctrl` box it lives in: absent reads
/// as default, and only a real write allocates it.
#[test]
fn extras_allocates_only_on_write() {
    let mut a = harness();

    let id = a.insert(K::NavigationView).unwrap();
    assert_eq!(a.ctrl_allocated(id), Some(false));
    assert_eq!(a.extras_allocated(id), Some(false));

    // A plain `Ctrl` write must not drag the second tier in with it — that is
    // the entire reason it is a second tier.
    a.apply_prop(id, Prop::SelectedIndex, &V::I32(1));
    assert_eq!(a.ctrl_allocated(id), Some(true));
    assert_eq!(
        a.extras_allocated(id),
        Some(false),
        "a Ctrl write allocated the Extras tier as well"
    );

    a.apply_prop(id, Prop::PaneTitle, &V::Str("Chains".into()));
    assert_eq!(a.extras_allocated(id), Some(true), "the write must allocate");
}

/// The §7.2 revision gate for control values, node half: input-originated
/// value writes bump the node's revision (`fire_value_changed`), and an app
/// echo stamped `based_on` an older revision is stale — `set_value_stamped`
/// drops it through this exact predicate instead of snapping the chrome back.
#[test]
fn value_echo_gate_follows_input_revision() {
    let mut a = harness();
    let id = a.insert(K::Slider).unwrap();

    // No input yet: every programmatic write applies (rev 0 vs based_on 0).
    assert_eq!(a.accepts_value_echo(id, 0), Some(true));

    let r1 = a.bump_value_rev(id);
    let r2 = a.bump_value_rev(id);
    assert_eq!((r1, r2), (1, 2), "revisions must be monotonic from 1");

    assert_eq!(
        a.accepts_value_echo(id, r1),
        Some(false),
        "an echo based on rev 1 is stale once input reached rev 2"
    );
    assert_eq!(
        a.accepts_value_echo(id, r2),
        Some(true),
        "an echo based on the latest delivered revision applies"
    );
}
