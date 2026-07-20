//! End-to-end probe for [`ElementHandle::live_text`] — text a producer thread
//! replaces without a reconcile.
//!
//! Every other check on that path is unit-level: the queue coalesces, the drain
//! releases its claim before applying, the node accepts new words. None of them
//! prove a glyph on screen ever changes. This does, and it proves the property
//! that motivates the whole mechanism rather than merely that the text moves:
//!
//! - **The live line ticks at ~60 Hz.** Its value comes from a plain
//!   `std::thread`, which holds no part of the visual tree.
//! - **The render counter does not.** It increments only in `app`, so it counts
//!   reconciles. If it stays at 1 while the value above it changes, the update
//!   provably did not pass through a render — which is the only reason this
//!   mechanism exists.
//!
//! The live value is also deliberately *width-varying* (it grows a digit, then a
//! sign) because an oversized live run is clipped by its host, silently. The
//! block therefore mounts with the widest string it will ever hold, so the
//! layout pass sizes the box for the extreme rather than for the first value. Get
//! that wrong and this example shows it: the tail of the longest reading goes
//! missing while the short ones look perfect.
//!
//! Run with:
//!   cargo run -p windows-reactor --example dcomp_live_text --features dcomp-backend

#[cfg(feature = "dcomp-backend")]
fn main() -> windows_reactor::Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Mutex, OnceLock};
    use windows_reactor::*;

    /// Counts reconciles. Read by the tree it counts, so it has to be global —
    /// a `use_state` would make every increment cause the render it is trying
    /// to observe.
    static RENDERS: AtomicU64 = AtomicU64::new(0);

    /// Publishes, and the reconcile count when the first one landed.
    ///
    /// Renders alone prove nothing: startup, DPI and every resize drive the
    /// reconciler too, so a raw count is a number the reader has to take on
    /// faith. The pair is self-evident — if publishes climb into the thousands
    /// while renders-since sits at zero, no publish can have gone through a
    /// render, and no argument about what the other renders were is needed.
    static PUBLISHES: AtomicU64 = AtomicU64::new(0);
    static RENDERS_AT_FIRST_PUBLISH: AtomicU64 = AtomicU64::new(u64::MAX);

    /// The producers' handles, published on mount. The ticker starts only once
    /// both are set, so it never publishes to an id the backend has not
    /// created.
    static VALUE: OnceLock<Mutex<Option<LiveText>>> = OnceLock::new();
    static TALLY: OnceLock<Mutex<Option<LiveText>>> = OnceLock::new();

    fn slot(cell: &'static OnceLock<Mutex<Option<LiveText>>>) -> &'static Mutex<Option<LiveText>> {
        cell.get_or_init(|| Mutex::new(None))
    }

    /// The widest string each live block can ever hold. Both mount with these so
    /// the layout pass reserves a box the extreme fits in — the value block
    /// grows a sign and a digit as it sweeps, and the tally only ever grows.
    const WIDEST_VALUE: &str = "-100.0 dB";
    const WIDEST_TALLY: &str = "publishes: 000000 · renders since: 000000";

    fn app(_cx: &mut RenderCx) -> Element {
        RENDERS.fetch_add(1, Ordering::Relaxed);

        let value = text_block(WIDEST_VALUE)
            .font_size(34.0)
            .font_weight(600)
            .on_mounted(|h| {
                *slot(&VALUE).lock().unwrap() = Some(h.live_text());
                start_ticker();
            });

        // Live too, and necessarily so: a static tally would only refresh on the
        // renders it is counting, so it would report itself stale — the one
        // reading that must not come from a reconcile is the count of them.
        let tally = text_block(WIDEST_TALLY)
            .font_size(13.0)
            .foreground(Color::rgb(0x9A, 0x9A, 0xA2))
            .on_mounted(|h| {
                *slot(&TALLY).lock().unwrap() = Some(h.live_text());
                start_ticker();
            });

        let card = vstack((
            text_block("live_text — value without a reconcile")
                .font_size(20.0)
                .semibold(),
            text_block("Both readings below are written by a plain std::thread.")
                .font_size(13.0)
                .foreground(Color::rgb(0x9A, 0x9A, 0xA2)),
            value,
            tally,
            text_block("Publishes climbing while renders-since stays 0 means no value passed through a render.")
                .font_size(12.0)
                .foreground(Color::rgb(0x6E, 0x6E, 0x76)),
        ))
        .spacing(12.0);

        border(card)
            .background(Color::rgb(0x24, 0x24, 0x2A))
            .corner_radius(14.0)
            .padding(Thickness::uniform(24.0))
            .margin(Thickness::uniform(40.0))
            .into()
    }

    /// Spawn the producer, once. Both blocks mount and both ask, so this is
    /// called twice; it starts on the second, when every handle exists.
    ///
    /// The value sweeps the full width range its box was sized for, so a box
    /// reserved for the *first* value instead of the widest shows up as a
    /// truncated tail partway through each sweep.
    fn start_ticker() {
        if slot(&VALUE).lock().unwrap().is_none() || slot(&TALLY).lock().unwrap().is_none() {
            return;
        }
        std::thread::spawn(|| {
            let mut n: i32 = 0;
            loop {
                let v = -(n % 1001) as f32 / 10.0;
                let published = PUBLISHES.fetch_add(1, Ordering::Relaxed);
                let base = RENDERS_AT_FIRST_PUBLISH.load(Ordering::Relaxed);
                let base = if published == 0 {
                    let now = RENDERS.load(Ordering::Relaxed);
                    RENDERS_AT_FIRST_PUBLISH.store(now, Ordering::Relaxed);
                    now
                } else {
                    base
                };
                let since = RENDERS.load(Ordering::Relaxed).saturating_sub(base);

                if let Some(h) = *slot(&VALUE).lock().unwrap() {
                    h.set(&format!("{v:.1} dB"));
                }
                if let Some(h) = *slot(&TALLY).lock().unwrap() {
                    h.set(&format!("publishes: {} · renders since: {since}", published + 1));
                }

                n += 7;
                std::thread::sleep(std::time::Duration::from_millis(16));
            }
        });
    }

    DCompHost::render("live_text probe", app)
}

#[cfg(not(feature = "dcomp-backend"))]
fn main() {
    eprintln!(
        "dcomp_live_text requires the `dcomp-backend` feature:\n  \
         cargo run -p windows-reactor --example dcomp_live_text --features dcomp-backend"
    );
}
