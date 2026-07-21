//! Control-library demo for the self-hosted DirectComposition + Direct2D
//! backend. Exercises the drawn control set — a `NavigationView` icon rail, a
//! `ToggleSwitch`, a `SelectorBar` (segmented), a `ComboBox`/Select with a
//! light-dismissed popup, a `CheckBox`, a `Slider`, determinate and
//! indeterminate progress (bar + ring, looping on the compositor), a
//! `HyperlinkButton`, an `Expander`, the three box-derived `Shape` kinds
//! (rectangle / ellipse / line), and a
//! `ScrollViewer` whose content overflows and scrolls on the compositor — all
//! reacting to pointer + keyboard (Tab focus ring, Space/Enter, arrows) and
//! idling at true zero CPU (blocking `GetMessageW` pump; springs self-stop).
//!
//! Run with:
//!   cargo run -p windows-reactor --example dcomp_controls --features dcomp-backend

#[cfg(feature = "dcomp-backend")]
fn main() -> windows_reactor::Result<()> {
    use windows_reactor::*;

    // Palette (mirrors the parity token table used by the backend chrome).
    const CARD: Color = Color::rgb(0x28, 0x28, 0x28);
    const PANEL: Color = Color::rgb(0x1c, 0x1c, 0x1c);
    const TXT: Color = Color::rgb(0xff, 0xff, 0xff);
    const TXT2: Color = Color::rgb(0xaa, 0xaa, 0xaa);

    /// One labelled control row, as a card with the control right-aligned.
    fn card(title: &str, control: Element) -> Element {
        border(
            grid((
                text_block(title).font_size(13.0).foreground(TXT).grid_column(0),
                control
                    .grid_column(1)
                    .horizontal_alignment(HorizontalAlignment::Right),
            ))
            .columns([GridLength::STAR, GridLength::Auto])
            .column_spacing(16.0),
        )
        .background(CARD)
        .corner_radius(8.0)
        .padding(Thickness::uniform(12.0))
        .into()
    }

    fn app(cx: &mut RenderCx) -> Element {
        let (eq_on, set_eq_on) = cx.use_state::<bool>(true);
        let (limiter, set_limiter) = cx.use_state::<bool>(false);
        let (filter, set_filter) = cx.use_state::<i32>(0);
        let (gain, set_gain) = cx.use_state::<f64>(50.0);
        let (analyzer, set_analyzer) = cx.use_state::<String>("Post".to_string());
        let (section, set_section) = cx.use_state::<String>("eq".to_string());

        // ── The control cards ────────────────────────────────────────────
        // State labels, and no explicit width: a labelled switch sizes to its
        // track plus the gap plus the WIDER of the two words, so pinning a
        // width here would both hide that arithmetic and clip whichever label
        // is longer. "Enabled"/"Offline" differ in width on purpose — a switch
        // that reflowed the row when it flipped would be visible immediately.
        let toggle = ToggleSwitch::new(eq_on)
            .on_toggled(move |v| set_eq_on.call(v))
            .on_content("Enabled")
            .off_content("Offline")
            .height(24.0)
            .into();

        let segmented = SelectorBar::new(vec![
            selector_bar_item("Pre"),
            selector_bar_item("Post"),
            selector_bar_item("Off"),
        ])
        .on_selection_changed(move |s| set_analyzer.call(s))
        .width(170.0)
        .height(28.0)
        .into();

        let select = ComboBox::new(["Peaking", "Low Shelf", "High Shelf", "Notch", "All-Pass"])
            .selected_index(filter)
            .on_selection_changed(move |i| set_filter.call(i))
            .width(170.0)
            .height(28.0)
            .into();

        let check = CheckBox::new(limiter)
            .content("Brick-wall limiter")
            .on_checked(move |v| set_limiter.call(v))
            .height(24.0)
            .width(180.0)
            .into();

        let slider = Slider::new(gain)
            .range(0.0, 100.0)
            .on_value_changed({
                let s = set_gain.clone();
                move |v| s.call(v)
            })
            .width(200.0)
            .height(24.0)
            .into();

        const ACCENT: Color = Color::rgb(0x15, 0xaf, 0xec);
        const HOT: Color = Color::rgb(0xf5, 0x9e, 0x0b);
        const ALARM: Color = Color::rgb(0xef, 0x44, 0x44);

        let set_gain_k = set_gain.clone();
        let knob = Knob::new(gain)
            .range(0.0, 100.0)
            .ticks(vec![0.0, 25.0, 50.0, 75.0, 100.0])
            .tick_labels(vec![(0.0, "0".into()), (50.0, "50".into()), (100.0, "100".into())])
            .major_every(50.0)
            .stops(vec![(0.0, ACCENT), (0.7, HOT), (1.0, ALARM)])
            .accent(ACCENT)
            .text(format!("{gain:.0}"))
            .unit("%")
            .on_value_changed(move |v| set_gain_k.call(v))
            .width(140.0)
            .height(140.0)
            .into();

        let meter = Meter::new(gain)
            .range(0.0, 100.0)
            .marker(80.0)
            .marker_color(ALARM)
            .stops(vec![(0.0, ACCENT), (0.8, HOT), (1.0, ALARM)])
            .width(200.0)
            .height(12.0)
            .into();

        let progress = ProgressBar::new(gain).height(8.0).width(200.0).into();
        let busy = ProgressBar::indeterminate().height(8.0).width(200.0).into();
        let ring = ProgressRing::indeterminate().width(28.0).height(28.0).into();
        // The determinate ring, tracking the same signal the bar and meter do.
        // It is here because nothing else instantiates one: the widget's
        // determinate branch draws a different arc from the indeterminate one
        // and had no on-screen coverage anywhere in the workspace.
        let ring_value = ProgressRing::new(gain).width(28.0).height(28.0).into();
        let link = HyperlinkButton::new("Release notes")
            .navigate_uri("https://example.com")
            .height(24.0)
            .width(120.0)
            .into();

        let advanced = Expander::new(
            vstack((
                text_block("Oversampling: 4x").font_size(12.0).foreground(TXT2),
                text_block("Dither: TPDF").font_size(12.0).foreground(TXT2),
            ))
            .spacing(6.0),
        )
        .header("Advanced")
        .into();

        // Badges: the bare status dot and a count pill, inline with a label.
        let badges = hstack((
            InfoBadge::dot(),
            InfoBadge::numeric(3),
            InfoBadge::numeric(128),
        ))
        .spacing(10.0)
        .vertical_alignment(VerticalAlignment::Center)
        .into();

        // One InfoBar per severity. The last is closable and carries a long
        // message, so it wraps to a second line and the band grows with it —
        // the height-follows-width path the layout measure exists for.
        let (bar_open, set_bar_open) = cx.use_state::<bool>(true);
        let bars: Vec<Element> = vec![
            InfoBar::new("Workspace loaded")
                .message("12 documents · 3 folders")
                .informational()
                .is_closable(false)
                .into(),
            InfoBar::new("Formatting complete")
                .message("Applied 12 edits.")
                .success()
                .is_closable(false)
                .into(),
            InfoBar::new("Limit exceeded")
                .message("One attachment exceeded the size limit.")
                .warning()
                .is_closable(false)
                .into(),
            InfoBar::new("Server unavailable")
                .message(
                    "The configured server stopped responding, so changes are held \
                     locally until it returns or another server is selected.",
                )
                .error()
                .is_open(bar_open)
                .on_closed(move || set_bar_open.call(false))
                .into(),
        ];

        // The three shape kinds that derive their geometry from the node box —
        // a filled+stroked rounded rect, an ellipse, and a bare line. All three
        // were immediate-mode painters and none of them was exercised by any
        // example, which is how a Rectangle's `.fill()` came to be silently
        // inert. Here so the conversion has a witness.
        let shapes: Element = hstack((
            Shape::rectangle()
                .fill(Color::rgb(0x3a, 0x7a, 0xd0))
                .stroke(Color::rgb(0xdd, 0xdd, 0xdd))
                .stroke_thickness(1.0)
                .corner_radius(6.0)
                .width(56.0)
                .height(32.0),
            Shape::ellipse()
                .fill(Color::rgb(0xd0, 0x7a, 0x3a))
                .stroke(Color::rgb(0xdd, 0xdd, 0xdd))
                .stroke_thickness(1.5)
                .width(48.0)
                .height(32.0),
            Shape::line(0.0, 16.0, 56.0, 16.0).width(56.0).height(32.0),
        ))
        .spacing(12.0)
        .into();

        // A tall stack so the ScrollViewer overflows and scrolls.
        let mut cards: Vec<Element> = vec![
            card("Shapes", shapes),
            card(&format!("EQ Active  ({})", if eq_on { "on" } else { "off" }), toggle),
            card(&format!("Analyzer  ({analyzer})"), segmented),
            card("Filter type", select),
            card(&format!("Output gain  ({gain:.0}%)"), slider),
            card("Gain (knob)", knob),
            card("Meter", meter),
            card("Level", progress),
            card("Analyzing…", busy),
            card("Loading", ring),
            card("Progress (ring)", ring_value),
            card("Dynamics", check),
            card("Unread", badges),
            card("About", link),
            advanced,
        ];
        // Full-bleed, not in a card: an InfoBar is its own card.
        cards.extend(bars);
        for i in 1..=8 {
            cards.push(card(
                &format!("Band {i}"),
                text_block(format!("{} Hz", 60 * i)).font_size(12.0).foreground(TXT2).into(),
            ));
        }

        // Header rows auto-size; the STAR row hands the scroll viewer the
        // REMAINING viewport height — a ScrollViewer only scrolls when a
        // parent bounds it (in a plain vstack it stretches to its content).
        let content = grid((
            text_block(format!("Section: {section}"))
                .font_size(15.0)
                .semibold()
                .foreground(TXT)
                .grid_row(0),
            text_block("Scroll with the wheel · Tab to focus · Space/Enter to toggle")
                .font_size(12.0)
                .foreground(TXT2)
                .grid_row(1),
            // The overlay thumb is pinned VISIBLE rather than left on its
            // auto-hide default: it is retained chrome like everything else
            // here, and a demo that conceals it the moment the pointer leaves
            // gives a screenshot no way to show it at all.
            scroll_viewer(vstack(cards).spacing(12.0))
                .vertical_scroll_bar_visibility(ScrollBarVisibility::Visible)
                .grid_row(2),
        ))
        .rows([GridLength::Auto, GridLength::Auto, GridLength::STAR])
        .row_spacing(12.0);

        let body = border(content)
            .background(PANEL)
            .padding(Thickness::uniform(20.0));

        // ── NavigationView shell (icon rail + content) ───────────────────
        NavigationView::new(
            [
                NavViewItem::new("Equalizer").icon(Symbol::Home).tag("eq"),
                NavViewItem::new("Effects").icon(Symbol::Find).tag("fx"),
                NavViewItem::new("Routing").icon(Symbol::Globe).tag("route"),
                NavViewItem::new("Settings").icon(Symbol::Setting).tag("settings"),
            ],
            body,
        )
        .selected_tag(section)
        .on_selection_changed(move |tag| set_section.call(tag))
        .into()
    }

    DCompHost::render("DComp controls", app)
}

#[cfg(not(feature = "dcomp-backend"))]
fn main() {
    eprintln!(
        "dcomp_controls requires the `dcomp-backend` feature:\n  \
         cargo run -p windows-reactor --example dcomp_controls --features dcomp-backend"
    );
}
