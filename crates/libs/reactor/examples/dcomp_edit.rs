//! Editable-text demo for the self-hosted DirectComposition + Direct2D backend:
//! a custom DirectWrite + TSF/IMM editor (no native HWND edit-control island)
//! shared by every text `ControlKind`. Exercises a `NumberBox` (caret,
//! selection, clamp/round/format on commit, inline arithmetic like `12*3`, spin
//! buttons, arrow / wheel stepping, fires `ValueChanged`), a `TextBox` (live
//! `TextChanged`), and an `AutoSuggestBox` search field. Clipboard Ctrl+C/X/V,
//! Ctrl+A select-all, word moves (Ctrl+←/→), Home/End all work. The caret
//! blinks only while a field is focused — blur every field and CPU returns to
//! true zero (blocking pump, no at-rest timer).
//!
//! Run with:
//!   cargo run -p windows-reactor --example dcomp_edit --features dcomp-backend

#[cfg(feature = "dcomp-backend")]
fn main() -> windows_reactor::Result<()> {
    use windows_reactor::*;

    const CARD: Color = Color::rgb(0x28, 0x28, 0x28);
    const PANEL: Color = Color::rgb(0x1c, 0x1c, 0x1c);
    const TXT: Color = Color::rgb(0xff, 0xff, 0xff);
    const TXT2: Color = Color::rgb(0xaa, 0xaa, 0xaa);

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
        let (gain, set_gain) = cx.use_state::<f64>(-6.0);
        let (freq, set_freq) = cx.use_state::<f64>(1000.0);
        let (name, set_name) = cx.use_state::<String>("Preset A".to_string());
        let (query, set_query) = cx.use_state::<String>(String::new());

        // Wide NumberBox: shows the spin buttons; inline arithmetic on commit.
        let gain_box = NumberBox::new(gain)
            .range(-30.0, 30.0)
            .step(0.5)
            .large_change(6.0)
            .precision(1)
            .on_value_changed(move |v| set_gain.call(v))
            .width(140.0)
            .height(32.0)
            .into();

        // Narrow centered NumberBox (EQ-tile style): spin hidden, arrows/wheel work.
        let freq_box = NumberBox::new(freq)
            .range(20.0, 20000.0)
            .step(1.0)
            .precision(0)
            .text_alignment(HorizontalAlignment::Center)
            .on_value_changed(move |v| set_freq.call(v))
            .width(60.0)
            .height(32.0)
            .into();

        let name_box = text_box(name.clone())
            .placeholder_text("Preset name…")
            .on_text_changed(move |t| set_name.call(t))
            .width(200.0)
            .height(32.0)
            .into();

        // Filter a small catalog by the live query (case-insensitive substring) so
        // the suggestion dropdown shows matching rows as you type.
        const CATALOG: [&str; 8] = [
            "Warmth", "Clarity", "Presence", "Bass Boost", "Air", "Crossfeed",
            "Loudness", "Flat Reference",
        ];
        let q = query.to_lowercase();
        let matches: Vec<String> = if q.is_empty() {
            Vec::new()
        } else {
            CATALOG
                .iter()
                .filter(|s| s.to_lowercase().contains(&q))
                .map(|s| s.to_string())
                .collect()
        };
        let search: Element = border(
            auto_suggest_box(query.clone())
                .placeholder_text("Search…")
                .items(matches)
                .on_text_changed(move |t| set_query.call(t)),
        )
        .width(200.0)
        .height(32.0)
        .into();

        let content = vstack((
            text_block("Custom DirectWrite editor — click a field and type")
                .font_size(15.0)
                .semibold()
                .foreground(TXT),
            text_block("Tab to move · Ctrl+A/C/X/V · arrows + wheel on numbers · 12*3 then Enter")
                .font_size(12.0)
                .foreground(TXT2),
            card(&format!("Gain ({gain:.1} dB)"), gain_box),
            card(&format!("Frequency ({freq:.0} Hz)"), freq_box),
            card(&format!("Name (\"{name}\")"), name_box),
            card(&format!("Search (\"{query}\")"), search),
        ))
        .spacing(12.0);

        border(content)
            .background(PANEL)
            .padding(Thickness::uniform(20.0))
            .into()
    }

    DCompHost::render("NewAPO — DComp editor", app)
}

#[cfg(not(feature = "dcomp-backend"))]
fn main() {
    eprintln!(
        "dcomp_edit requires the `dcomp-backend` feature:\n  \
         cargo run -p windows-reactor --example dcomp_edit --features dcomp-backend"
    );
}
