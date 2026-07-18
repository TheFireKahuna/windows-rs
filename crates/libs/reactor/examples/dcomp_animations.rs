//! Compositor-animation demo for the self-hosted DirectComposition backend.
//!
//! Exercises the declarative animation API end-to-end on real DWM-evaluated
//! animations — the app does no per-frame work for any of them:
//!
//! - the whole panel **fades in on mount** (enter transition on the root),
//! - the "Now you see me" card **pops in** when shown and — the interesting
//!   part — **pops out as a flattened exit ghost** when hidden: the destroyed
//!   subtree is snapshotted into one visual-surface sprite and fades as a
//!   single layer while the app is already done with it,
//! - the "Dimmable" card animates opacity **implicitly** whenever its
//!   `.opacity(..)` prop changes (`with_opacity_transition`),
//! - toggling the card also shifts its sibling — wrapped in a **layout glide**
//!   (`with_layout_animation`), the move is a compositor spring, not a jump.
//!
//! Durations are slower than production taste so the motion is easy to see
//! (and to screenshot mid-flight).
//!
//! Run with:
//!   cargo run -p windows-reactor --example dcomp_animations --features dcomp-backend

#[cfg(feature = "dcomp-backend")]
fn main() -> windows_reactor::Result<()> {
    use std::time::Duration;
    use windows_reactor::*;

    fn app(cx: &mut RenderCx) -> Element {
        let (show, set_show) = cx.use_state::<bool>(true);
        let (dim, set_dim) = cx.use_state::<bool>(false);

        let toggle = button(if show { "Hide card" } else { "Show card" })
            .accent()
            .on_click(move || set_show.call(!show));

        // Mount/unmount transitions: pop in on show; on hide the backend
        // detaches the subtree as a flattened snapshot ghost and plays the
        // exit on it, compositor-side, after the node is already destroyed.
        //
        // NB the `.with_key(..)`: a conditional child NEEDS a stable key for
        // its exit to fire. `Element::Empty` is filtered from the live child
        // list, so the positional differ would otherwise MORPH this node into
        // its next sibling on hide and destroy the last child instead — the
        // exit-transitioned node would never be destroyed at all.
        let ghost_card = show.then(|| {
            border(vstack((
                text_block("Now you see me").font_size(18.0).semibold(),
                text_block("Unmounting plays a flattened exit ghost")
                    .font_size(12.0)
                    .foreground(Color::rgb(0x9A, 0x9A, 0xA2)),
            ))
            .spacing(6.0))
            .background(Color::rgb(0x2E, 0x34, 0x44))
            .corner_radius(12.0)
            .padding(Thickness::uniform(18.0))
            .transition(
                Some(AnimationConfig::pop_in(Duration::from_millis(400))),
                Some(AnimationConfig::pop_out(Duration::from_millis(1200))),
            )
            .with_key("ghost-card")
        });

        // Implicit transition: the `.opacity(..)` prop change below GLIDES —
        // the compositor animates any change to the property automatically.
        let dim_card = border(vstack((
            text_block("Dimmable").font_size(18.0).semibold(),
            button(if dim { "Undim" } else { "Dim" }).on_click(move || set_dim.call(!dim)),
        ))
        .spacing(10.0))
        .background(Color::rgb(0x24, 0x24, 0x2A))
        .corner_radius(12.0)
        .padding(Thickness::uniform(18.0))
        .opacity(if dim { 0.35 } else { 1.0 })
        .with_opacity_transition(Duration::from_millis(350))
        // Layout glide: when the card above unmounts, this card's slot moves —
        // a compositor spring carries it there instead of a jump.
        .with_layout_animation(LayoutAnimationConfig::spring())
        .with_key("dim-card");

        let column = vstack((
            text_block("Compositor animations — zero app ticks")
                .font_size(22.0)
                .semibold(),
            toggle,
            ghost_card.map(Element::from).unwrap_or(Element::Empty),
            dim_card,
        ))
        .spacing(14.0);

        // The whole panel fades in on mount (one animation on this container;
        // every descendant rides the group opacity).
        border(column)
            .padding(Thickness::uniform(32.0))
            .transition(
                Some(AnimationConfig::fade_in(Duration::from_millis(500))),
                None,
            )
            .into()
    }

    DCompHost::render("NewAPO — animations", app)
}

#[cfg(not(feature = "dcomp-backend"))]
fn main() {
    eprintln!(
        "dcomp_animations requires the `dcomp-backend` feature:\n  \
         cargo run -p windows-reactor --example dcomp_animations --features dcomp-backend"
    );
}
