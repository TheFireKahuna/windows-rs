//! `InfoBar` + `InfoBadge` demo for the self-hosted DirectComposition + Direct2D
//! backend.
//!
//! Covers the parts of these two controls that are easy to get wrong:
//!
//! * every severity, so the icon/tint table is visible side by side;
//! * a **wrapping** bar — its message is long enough to need a second line, and
//!   the band grows to hold it, which is the height-follows-width path the
//!   layout measure exists for (resize the window to watch it re-wrap);
//! * a **closable** bar whose close button dismisses it, taking the band out of
//!   layout entirely so the stack below closes up;
//! * a bar with a title and no message, and one with a message and no title;
//! * both badge forms — the bare dot and numeric pills of one, two and three
//!   digits, so the stadium's round-at-any-width geometry is checkable.
//!
//! Run with:
//!   cargo run -p windows-reactor --example dcomp_infobar

fn main() -> windows_reactor::Result<()> {
    use windows_reactor::*;

    const PANEL: Color = Color::rgb(0x1c, 0x1c, 0x1c);
    const TXT: Color = Color::rgb(0xff, 0xff, 0xff);
    const TXT2: Color = Color::rgb(0xaa, 0xaa, 0xaa);

    fn heading(s: &str) -> Element {
        text_block(s).font_size(13.0).semibold().foreground(TXT).into()
    }

    fn app(cx: &mut RenderCx) -> Element {
        // The closable bar is a controlled prop: its `on_closed` drives this
        // state, and the state drives `is_open` back. Re-open it with the
        // button to watch the band come back and the stack re-flow.
        let (alert_open, set_alert_open) = cx.use_state::<bool>(true);
        let reopen = set_alert_open.clone();

        let bars = vstack(vec![
            heading("Severities"),
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
                .message("The configured server stopped responding.")
                .error()
                .is_closable(false)
                .into(),
            heading("Wrapping (resize the window)"),
            InfoBar::new("Offline")
                .message(
                    "The configured server stopped responding, so every pending change \
                     is held locally until the server returns or another one is \
                     selected in Settings.",
                )
                .warning()
                .is_closable(false)
                .into(),
            heading("Title only / message only"),
            InfoBar::new("Saved").success().is_closable(false).into(),
            InfoBar::default()
                .is_open(true)
                .message("A message with no title reads at the body weight.")
                .informational()
                .is_closable(false)
                .into(),
            heading("Closable — dismiss it and the stack closes up"),
            InfoBar::new("Update available")
                .message("Version 2.1 is ready to install.")
                .informational()
                .is_open(alert_open)
                .on_closed(move || set_alert_open.call(false))
                .into(),
            Button::new("Re-open the closable bar")
                .on_click(move || reopen.call(true))
                .width(220.0)
                .height(30.0)
                .into(),
        ])
        .spacing(10.0);

        let badges = vstack((
            heading("InfoBadge"),
            hstack((
                text_block("dot").font_size(12.0).foreground(TXT2).width(60.0),
                InfoBadge::dot(),
            ))
            .spacing(12.0)
            .vertical_alignment(VerticalAlignment::Center),
            hstack((
                text_block("numeric").font_size(12.0).foreground(TXT2).width(60.0),
                InfoBadge::numeric(1),
                InfoBadge::numeric(9),
                InfoBadge::numeric(42),
                InfoBadge::numeric(128),
                InfoBadge::numeric(0),
            ))
            .spacing(10.0)
            .vertical_alignment(VerticalAlignment::Center),
            // An app-coloured badge picks its own fill AND its own ink — the
            // pair a host controls together. The last one is the EQ band-badge
            // shape: a fixed square, so the stadium resolves to a circle.
            hstack((
                text_block("themed").font_size(12.0).foreground(TXT2).width(60.0),
                InfoBadge::numeric(7)
                    .background(Color::rgb(0xf1, 0x52, 0x52))
                    .foreground(Color::rgb(0xff, 0xff, 0xff)),
                InfoBadge::numeric(3)
                    .background(Color::rgb(0x41, 0xd9, 0xa4))
                    .foreground(Color::rgb(0x00, 0x00, 0x00)),
                InfoBadge::numeric(4)
                    .background(Color::rgb(0xf6, 0xa9, 0x11))
                    .foreground(Color::rgb(0x00, 0x00, 0x00))
                    .width(18.0)
                    .height(18.0),
            ))
            .spacing(10.0)
            .vertical_alignment(VerticalAlignment::Center),
        ))
        .spacing(10.0);

        border(
            scroll_viewer(vstack((bars, badges)).spacing(24.0))
        )
        .background(PANEL)
        .padding(Thickness::uniform(20.0))
        .into()
    }

    DCompHost::render("DComp InfoBar", app)
}

