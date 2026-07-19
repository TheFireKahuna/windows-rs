//! The button family, every variant at once — the visual check for the
//! DirectComposition button chrome.
//!
//! `dcomp_controls` exercises the wider control set but contains barely a
//! button; this exists so a change to the family's fill, border, radius,
//! ornament or flyout has one place that shows all of it in a single frame.
//!
//! Run with:
//!   cargo run -p windows-reactor --example dcomp_buttons --features dcomp-backend

#[cfg(feature = "dcomp-backend")]
fn main() -> windows_reactor::Result<()> {
    use windows_reactor::*;

    const PANEL: Color = Color::rgb(0x1c, 0x1c, 0x1c);
    const CARD: Color = Color::rgb(0x28, 0x28, 0x28);
    const TXT: Color = Color::rgb(0xff, 0xff, 0xff);
    const TXT2: Color = Color::rgb(0xaa, 0xaa, 0xaa);

    /// A titled row of buttons, so each group is legible on its own.
    fn group(title: &str, row: Element) -> Element {
        border(
            vstack((
                text_block(title).font_size(12.0).foreground(TXT2),
                row,
            ))
            .spacing(10.0),
        )
        .background(CARD)
        .corner_radius(8.0)
        .padding(Thickness::uniform(14.0))
        .into()
    }

    fn app(cx: &mut RenderCx) -> Element {
        let (clicks, set_clicks) = cx.use_state::<i32>(0);
        let (muted, set_muted) = cx.use_state::<bool>(false);
        let (last, set_last) = cx.use_state::<String>("—".to_string());

        // ── The four authored styles ─────────────────────────────────────
        let styles = hstack((
            button("Default").on_click({
                let s = set_clicks.clone();
                move || s.call(clicks + 1)
            }),
            button("Accent").accent(),
            button("Subtle").subtle(),
            button("TextLink").text_link(),
        ))
        .spacing(10.0)
        .into();

        // Same four again, disabled: the dim is a part opacity, so this is the
        // check that every variant dims and none of them vanish.
        let disabled = hstack((
            button("Default").enabled(false),
            button("Accent").accent().enabled(false),
            button("Subtle").subtle().enabled(false),
            button("TextLink").text_link().enabled(false),
        ))
        .spacing(10.0)
        .into();

        // ── Icons and ornaments ──────────────────────────────────────────
        let icons = hstack((
            button("Save").icon(Symbol::Save),
            button("Add").icon(Symbol::Add).accent(),
            button("").icon(Symbol::Setting),
            button("Refresh").icon(Symbol::Refresh).subtle(),
        ))
        .spacing(10.0)
        .into();

        // ── Radius: authored below the family default must survive, and
        //    `pill` must resolve against the measured height ───────────────
        let radii = hstack((
            button("r0").corner_radius(0.0),
            button("r2").corner_radius(2.0),
            button("r8 (default)"),
            button("pill").pill(),
            button("pill accent").pill().accent(),
        ))
        .spacing(10.0)
        .into();

        // ── Toggle / repeat ──────────────────────────────────────────────
        let family = hstack((
            ToggleButton::new("Mute", muted).on_checked(move |v| set_muted.call(v)),
            RepeatButton::new("Nudge −").on_click({
                let s = set_clicks.clone();
                move || s.call(clicks - 1)
            }),
            RepeatButton::new("Nudge +").on_click({
                let s = set_clicks.clone();
                move || s.call(clicks + 1)
            }),
        ))
        .spacing(10.0)
        .into();

        // ── Flyouts ──────────────────────────────────────────────────────
        let flyouts = hstack((
            button("Text flyout").flyout("Applies the curve to every channel."),
            button("Menu")
                .menu_flyout(vec![
                    menu_item("Copy").shortcut("Ctrl+C"),
                    menu_item("Paste").shortcut("Ctrl+V"),
                    MenuItemDef::Separator,
                    menu_item("Reset").danger(),
                ])
                .on_item_clicked(move |s| set_last.call(s)),
            DropDownButton::new("Preset")
                .menu_flyout(vec![menu_item("Flat"), menu_item("Loudness")]),
        ))
        .spacing(10.0)
        .into();

        let cards = vec![
            group("Styles", styles),
            group("Disabled", disabled),
            group("With icons", icons),
            group("Corner radius", radii),
            group("Toggle · Repeat", family),
            group("Flyouts", flyouts),
        ];

        let content = grid((
            text_block("Button family — DirectComposition")
                .font_size(15.0)
                .semibold()
                .foreground(TXT)
                .grid_row(0),
            text_block(format!("clicks {clicks} · muted {muted} · menu {last}"))
                .font_size(12.0)
                .foreground(TXT2)
                .grid_row(1),
            scroll_viewer(vstack(cards).spacing(12.0)).grid_row(2),
        ))
        .rows([GridLength::Auto, GridLength::Auto, GridLength::STAR])
        .row_spacing(12.0);

        border(content)
            .background(PANEL)
            .padding(Thickness::uniform(20.0))
            .into()
    }

    DCompHost::render("NewAPO — DComp buttons", app)
}

#[cfg(not(feature = "dcomp-backend"))]
fn main() {
    eprintln!(
        "dcomp_buttons requires the `dcomp-backend` feature:\n  \
         cargo run -p windows-reactor --example dcomp_buttons --features dcomp-backend"
    );
}
