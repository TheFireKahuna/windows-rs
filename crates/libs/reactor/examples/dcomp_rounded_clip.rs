//! A radiused `Border` cuts its subtree to its own corners — no app opt-in.
//!
//! Both cards below are the same `Border` holding the same oversized child, and
//! the app writes the same props to each. The only difference is the authored
//! corner radius, which is also the only thing the backend consults: a resolved
//! radius above zero mints a `RectangleClip` on the node's container, so the
//! child is cut to the rounded box. The square card mints nothing and the child
//! bleeds out of it, which is what every radiused card used to do too.
//!
//! Run with:
//!   cargo run -p windows-reactor --example dcomp_rounded_clip

fn main() -> windows_reactor::Result<()> {
    use windows_reactor::*;

    /// A child deliberately larger than the card that holds it, so whether the
    /// card clips is visible at a glance rather than inferred.
    fn overflowing() -> Element {
        border(
            text_block("child overflows its card")
                .font_size(13.0)
                .foreground(Color::rgb(0x10, 0x10, 0x14)),
        )
        .background(Color::rgb(0x4C, 0xC2, 0xFF))
        .padding(Thickness::uniform(18.0))
        .width(360.0)
        .height(190.0)
        .into()
    }

    fn card(radius: f64) -> Element {
        border(overflowing())
            .background(Color::rgb(0x2A, 0x2A, 0x33))
            .corner_radius(radius)
            .width(260.0)
            .height(150.0)
            .into()
    }

    fn app(_cx: &mut RenderCx) -> Element {
        let rounded = vstack((
            text_block("corner_radius(28) — clipped")
                .font_size(13.0)
                .foreground(Color::rgb(0xEA, 0xEA, 0xEC)),
            card(28.0),
        ))
        .spacing(10.0);

        let square = vstack((
            text_block("corner_radius(0) — no clip")
                .font_size(13.0)
                .foreground(Color::rgb(0xEA, 0xEA, 0xEC)),
            card(0.0),
        ))
        .spacing(10.0);

        border(hstack((rounded, square)).spacing(80.0))
            .background(Color::rgb(0x16, 0x16, 0x1C))
            .padding(Thickness::uniform(40.0))
            .into()
    }

    DCompHost::render("Rounded clip", app)
}
