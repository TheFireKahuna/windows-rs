//! Layout/props demo for the self-hosted DirectComposition + Direct2D backend.
//!
//! Exercises the retained per-node composition tree and the completed visual /
//! layout props: a two-column `Grid` of `Border` cards, nested
//! `Border ▸ Grid ▸ StackPanel`, per-child alignment, an opacity-faded node, and
//! a `Canvas` with absolutely-positioned chips — plus an accent `Button` that
//! still hovers / presses (spring ink) and counts clicks. Idle is true idle (a
//! blocking `GetMessageW` pump): zero CPU at rest.
//!
//! Run with:
//!   cargo run -p windows-reactor --example dcomp_layout --features dcomp-backend

#[cfg(feature = "dcomp-backend")]
fn main() -> windows_reactor::Result<()> {
    use windows_reactor::*;

    fn card(title: &str, body: Element) -> Border {
        border(
            vstack((
                text_block(title)
                    .font_size(15.0)
                    .semibold()
                    .foreground(Color::rgb(0xEA, 0xEA, 0xEC)),
                body,
            ))
            .spacing(10.0),
        )
        .background(Color::rgb(0x22, 0x22, 0x28))
        .corner_radius(12.0)
        .padding(Thickness::uniform(16.0))
    }

    /// A pill positioned absolutely inside a `Canvas`.
    fn chip(label: &str, x: f64, y: f64) -> Element {
        let e: Element = border(text_block(label).font_size(11.0).foreground(Color::rgb(0xDD, 0xE6, 0xF2)))
            .background(Color::rgb(0x2E, 0x3C, 0x52))
            .corner_radius(6.0)
            .padding(Thickness::uniform(6.0))
            .into();
        e.canvas_left(x).canvas_top(y)
    }

    fn app(cx: &mut RenderCx) -> Element {
        let (count, set_count) = cx.use_state::<i32>(0);
        let label = if count == 0 {
            "Apply".to_string()
        } else {
            format!("Applied {count}×")
        };

        // Left card: nested Border ▸ StackPanel with an accent button and a
        // center-aligned status pill.
        let left: Element = card(
            "Controls",
            vstack((
                text_block("Nested Border \u{25B8} Grid \u{25B8} StackPanel")
                    .font_size(12.0)
                    .foreground(Color::rgb(0x9A, 0x9A, 0xA2)),
                button(label)
                    .accent()
                    .on_click(move || set_count.call(count + 1)),
                text_block("centered")
                    .font_size(11.0)
                    .foreground(Color::rgb(0x7A, 0x7A, 0x82))
                    .horizontal_alignment(HorizontalAlignment::Center),
            ))
            .spacing(12.0)
            .into(),
        )
        .vertical_alignment(VerticalAlignment::Top)
        .grid_column(0)
        .into();

        // Right card: an opacity-faded note over a Canvas of positioned chips.
        let right: Element = card(
            "Visuals",
            vstack((
                text_block("faded node (opacity 0.4)")
                    .font_size(12.0)
                    .foreground(Color::rgb(0xC8, 0xC8, 0xD0))
                    .opacity(0.4),
                Element::from(Canvas::new([
                    chip("x=8 y=8", 8.0, 8.0),
                    chip("x=96 y=40", 96.0, 40.0),
                    chip("x=40 y=74", 40.0, 74.0),
                ]))
                .height(108.0),
            ))
            .spacing(10.0)
            .into(),
        )
        .vertical_alignment(VerticalAlignment::Top)
        .grid_column(1)
        .into();

        let body = grid((left, right))
            .columns([GridLength::STAR, GridLength::STAR])
            .column_spacing(16.0);

        // Full-bleed stretch leaf in a Star×Star grid cell: a bare Border with no
        // explicit size and the default (Stretch) alignment must FILL the cell on
        // both axes — the layout-collapse regression this backend had to fix. It
        // renders as a thin tinted band that spans the whole width.
        let fill_band: Element = border(
            text_block("full-bleed stretch leaf (fills its Star×Star cell)")
                .font_size(11.0)
                .foreground(Color::rgb(0x9A, 0xD0, 0xC0)),
        )
        .background(Color::rgb(0x1E, 0x2A, 0x26))
        .corner_radius(8.0)
        .padding(Thickness::uniform(8.0))
        .grid_row(0)
        .grid_column(0)
        .into();
        let band = grid((fill_band,))
            .rows([GridLength::STAR])
            .columns([GridLength::STAR]);

        border(
            vstack((
                text_block("DComp backend \u{2014} layout & props")
                    .font_size(22.0)
                    .semibold(),
                Element::from(body),
                Element::from(band),
            ))
            .spacing(16.0),
        )
        .background(Color::rgb(0x16, 0x16, 0x1B))
        .corner_radius(14.0)
        .padding(Thickness::uniform(24.0))
        .margin(Thickness::uniform(28.0))
        .into()
    }

    DCompHost::render("DComp layout", app)
}

#[cfg(not(feature = "dcomp-backend"))]
fn main() {
    eprintln!(
        "dcomp_layout requires the `dcomp-backend` feature:\n  \
         cargo run -p windows-reactor --example dcomp_layout --features dcomp-backend"
    );
}
