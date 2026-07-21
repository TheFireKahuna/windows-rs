//! The custom caption band on the self-hosted DirectComposition backend.
//!
//! The one control the other examples never mount, and the reason its chrome
//! went unverified for as long as it did: `dcomp_controls` shows caption buttons
//! in the UIA tree, but those are synthetic items on the root — no `TitleBar`
//! node, so nothing of this band is exercised.
//!
//! What it puts on screen, all of it retained (one hover-wash part, six glyph
//! sprite runs, no surface):
//!
//! * the title + subtitle pair, whose widths are COUPLED — narrow the window
//!   and the title ellipsizes first, then the subtitle loses its room;
//! * the leading back button, toggleable between enabled and disabled (it greys
//!   rather than disappearing, so the band never reflows);
//! * the min / max-restore / close cluster, whose middle glyph swaps on
//!   maximize without reshaping.
//!
//! Run with:
//!   cargo run -p windows-reactor --example dcomp_caption

fn main() -> windows_reactor::Result<()> {
    use windows_reactor::*;

    const TXT: Color = Color::rgb(0xff, 0xff, 0xff);
    const TXT2: Color = Color::rgb(0xaa, 0xaa, 0xaa);
    const PANEL: Color = Color::rgb(0x1c, 0x1c, 0x1c);

    fn app(cx: &mut RenderCx) -> Element {
        let (back_on, set_back_on) = cx.use_state::<bool>(true);
        let (back_enabled, set_back_enabled) = cx.use_state::<bool>(true);
        let (long, set_long) = cx.use_state::<bool>(false);

        // A long title is the case that matters: the subtitle's origin is
        // whatever the title was CLAMPED to, so the two only diverge once the
        // title stops fitting.
        let title = if long {
            "A Deliberately Long Window Title For Ellipsis"
        } else {
            "Reactor"
        };

        let caption: Element = TitleBar::new(title)
            .subtitle("Living room")
            .back_button_visible(back_on)
            .back_button_enabled(back_enabled)
            .into();

        // Deliberately narrow, and stacked rather than in a row: the band is
        // only as wide as the window when nothing below it demands more, and a
        // body that forced the tree wider would push the button cluster off
        // screen and stop the title ever reaching its clamp — which is the one
        // thing this example exists to show.
        let body: Element = border(
            vstack((
                text_block("Narrow the window to clamp the title.")
                    .font_size(12.0)
                    .foreground(TXT),
                text_block("Maximize to swap the middle glyph.")
                    .font_size(12.0)
                    .foreground(TXT2),
                ToggleSwitch::new(back_on)
                    .on_toggled(move |v| set_back_on.call(v))
                    .on_content("Back shown")
                    .off_content("Back hidden")
                    .height(24.0),
                ToggleSwitch::new(back_enabled)
                    .on_toggled(move |v| set_back_enabled.call(v))
                    .on_content("Back enabled")
                    .off_content("Back disabled")
                    .height(24.0),
                ToggleSwitch::new(long)
                    .on_toggled(move |v| set_long.call(v))
                    .on_content("Long title")
                    .off_content("Short title")
                    .height(24.0),
            ))
            .spacing(8.0),
        )
        .background(PANEL)
        .padding(Thickness::uniform(16.0))
        .into();

        // A STAR column, explicitly: an unspecified grid column sizes to its
        // items' MAX-content, which for a caption band is its whole natural
        // title block — so the band would be handed a track wider than the
        // window and never reach the width it is supposed to clamp against.
        grid((caption.grid_row(0), body.grid_row(1)))
            .rows([GridLength::Auto, GridLength::STAR])
            .columns([GridLength::STAR])
            .into()
    }

    DCompHost::render("DComp caption", app)
}

