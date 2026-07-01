//! Control-library demo for the self-hosted DirectComposition + Direct2D
//! backend. Exercises the drawn control set — a `NavigationView` icon rail, a
//! `ToggleSwitch`, a `SelectorBar` (segmented), a `ComboBox`/Select with a
//! light-dismissed popup, a `CheckBox`, a `Slider`, a `ProgressBar`, and a
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
        let toggle = ToggleSwitch::new(eq_on)
            .on_toggled(move |v| set_eq_on.call(v))
            .width(44.0)
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
            .on_value_changed(move |v| set_gain.call(v))
            .width(200.0)
            .height(24.0)
            .into();

        let progress = ProgressBar::new(gain).height(8.0).width(200.0).into();

        // A tall stack so the ScrollViewer overflows and scrolls.
        let mut cards: Vec<Element> = vec![
            card(&format!("EQ Active  ({})", if eq_on { "on" } else { "off" }), toggle),
            card(&format!("Analyzer  ({analyzer})"), segmented),
            card("Filter type", select),
            card(&format!("Output gain  ({gain:.0}%)"), slider),
            card("Level", progress),
            card("Dynamics", check),
        ];
        for i in 1..=8 {
            cards.push(card(
                &format!("Band {i}"),
                text_block(format!("{} Hz", 60 * i)).font_size(12.0).foreground(TXT2).into(),
            ));
        }

        let content = vstack((
            text_block(format!("Section: {section}"))
                .font_size(15.0)
                .semibold()
                .foreground(TXT),
            text_block("Scroll with the wheel · Tab to focus · Space/Enter to toggle")
                .font_size(12.0)
                .foreground(TXT2),
            scroll_viewer(vstack(cards).spacing(12.0)),
        ))
        .spacing(12.0);

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

    DCompHost::render("NewAPO — DComp controls", app)
}

#[cfg(not(feature = "dcomp-backend"))]
fn main() {
    eprintln!(
        "dcomp_controls requires the `dcomp-backend` feature:\n  \
         cargo run -p windows-reactor --example dcomp_controls --features dcomp-backend"
    );
}
