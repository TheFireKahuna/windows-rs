//! Stage 2 spine demo for the self-hosted DirectComposition + Direct2D backend.
//!
//! Builds a small real reactor component tree (a `Border` card wrapping a
//! `StackPanel` of text + an accent `Button`) through the public reactor API and
//! runs it through `DCompHost` — so it renders in a real FP16 scRGB HDR window on
//! the system compositor. The button hovers/presses (spring ink) and prints on
//! click. Idle is true idle (a blocking `GetMessageW` pump): zero CPU at rest.
//!
//! Run with:
//!   cargo run -p windows-reactor --example dcomp_hello --features dcomp-backend

#[cfg(feature = "dcomp-backend")]
fn main() -> windows_reactor::Result<()> {
    use windows_reactor::*;

    fn app(cx: &mut RenderCx) -> Element {
        let (count, set_count) = cx.use_state::<i32>(0);

        let label = if count == 0 {
            "Click me".to_string()
        } else {
            format!("Clicked {count}×")
        };

        let card = vstack((
            text_block("DComp backend — live")
                .font_size(22.0)
                .semibold(),
            text_block("Win32 + system compositor + Direct2D 1.3, FP16 scRGB")
                .font_size(13.0)
                .foreground(Color::rgb(0x9A, 0x9A, 0xA2)),
            button(label).accent().on_click(move || {
                println!("dcomp_hello: button clicked ({} → {})", count, count + 1);
                set_count.call(count + 1);
            }),
        ))
        .spacing(14.0);

        border(card)
            .background(Color::rgb(0x24, 0x24, 0x2A))
            .corner_radius(14.0)
            .padding(Thickness::uniform(24.0))
            .margin(Thickness::uniform(40.0))
            .into()
    }

    DCompHost::render("NewAPO — DComp spine", app)
}

#[cfg(not(feature = "dcomp-backend"))]
fn main() {
    eprintln!(
        "dcomp_hello requires the `dcomp-backend` feature:\n  \
         cargo run -p windows-reactor --example dcomp_hello --features dcomp-backend"
    );
}
