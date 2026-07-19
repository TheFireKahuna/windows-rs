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
        let (gain, set_gain) = cx.use_state::<f64>(0.0);

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

        // ── Badges: the InfoBadge's two forms, hosted in the button ──────
        let badges = hstack((
            button("Inbox").badge(Badge::count(12)),
            button("Sync").badge(Badge::count(3)).accent(),
            button("Live").badge(Badge::dot().leading()).subtle(),
            button("Errors").badge(Badge::count(7).tint(Color::rgb(0xd1, 0x34, 0x38))),
            button("Downloads").icon(Symbol::Download).badge(Badge::count(2)),
            button("").icon(Symbol::Mail).badge(Badge::dot()),
        ))
        .spacing(10.0)
        .into();

        // ── Authored fill / stroke: the pills and chips a host builds out of
        //    a button now that `Background` reaches the family ─────────────
        let tinted = hstack((
            button("+6 dB")
                .pill()
                .background(Color::rgba(0x4c, 0x9a, 0xff, 0x2e))
                .foreground(Color::rgb(0x9c, 0xc8, 0xff)),
            button("Bypassed")
                .pill()
                .background(Color::rgba(0xff, 0xff, 0xff, 0x10))
                .border_brush(Color::rgba(0xff, 0xff, 0xff, 0x1a))
                .foreground(TXT2),
            button("Graphic EQ")
                .pill()
                .badge(Badge::dot().tint(Color::rgb(0x7a, 0xd1, 0x8c)).leading())
                .background(Color::rgba(0x7a, 0xd1, 0x8c, 0x26))
                .foreground(TXT2),
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
            ToggleButton::new("Mute", muted).on_checked({
                let s = set_muted.clone();
                move |v| s.call(v)
            }),
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

        // A rich flyout hosts LIVE controls: the slider drags, the toggle
        // flips, and both drive the same state the rest of the page reads.
        let rich = hstack((button("Band panel")
            .accent()
            .flyout_def(
                FlyoutDef::rich(
                    border(
                        vstack((
                            text_block("Band 3 — 250 Hz").font_size(13.0).semibold().foreground(TXT),
                            text_block(format!("Gain {gain:.1} dB"))
                                .font_size(12.0)
                                .foreground(TXT2),
                            Slider::new(gain)
                                .range(-12.0, 12.0)
                                .on_value_changed(move |v| set_gain.call(v))
                                .width(240.0),
                            ToggleSwitch::new(muted)
                                .on_toggled(move |v| set_muted.call(v))
                                .width(44.0)
                                .height(24.0),
                        ))
                        .spacing(8.0),
                    )
                    .padding(Thickness::uniform(4.0)),
                )
                .placement(FlyoutPlacementMode::Bottom),
            ),))
        .spacing(10.0)
        .into();

        let cards = vec![
            group("Styles", styles),
            group("Disabled", disabled),
            group("With icons", icons),
            group("Badges", badges),
            group("Authored fill · stroke", tinted),
            group("Corner radius", radii),
            group("Toggle · Repeat", family),
            group("Flyouts", flyouts),
            group("Rich flyout — live controls", rich),
        ];

        let content = grid((
            text_block("Button family — DirectComposition")
                .font_size(15.0)
                .semibold()
                .foreground(TXT)
                .grid_row(0),
            text_block(format!("clicks {clicks} · muted {muted} · menu {last} · gain {gain:.1}"))
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

    DCompHost::render("DComp buttons", app)
}

#[cfg(not(feature = "dcomp-backend"))]
fn main() {
    eprintln!(
        "dcomp_buttons requires the `dcomp-backend` feature:\n  \
         cargo run -p windows-reactor --example dcomp_buttons --features dcomp-backend"
    );
}
