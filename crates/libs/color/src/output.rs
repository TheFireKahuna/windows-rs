//! The display output transform — the last stage before the compositor.
//!
//! Three facts about the platform decide everything here.
//!
//! **DWM's canonical composition colour space is scRGB FP16** — BT.709 primaries,
//! linear gamma, half precision. It converts every application into that space before
//! blending, so that is what an application hands it.
//!
//! **scRGB's extended range is the wide-gamut mechanism, not a side effect.** Values
//! outside `[0, 1]` represent colours outside sRGB and luminance above 80 nits, and
//! there is no enforced clamp. Encoding in Rec.709 primaries therefore loses no
//! gamut: a 3x3 on floats is an exact change of basis.
//!
//! **Windows does not gamut-map or tonemap for an application.** Colours beyond the
//! display's gamut are numerically clipped, per channel, which rotates hue.
//!
//! And one distinction that is easy to get wrong, because "Advanced Color is on" does
//! **not** imply headroom: composition is scene-referred only on an HDR display. On a
//! wide-gamut SDR display it is display-referred — `1.0` is the panel's white and
//! there is nothing above it — so a WCG desktop needs the tone stage exactly as much
//! as a plain SDR one. It differs from plain SDR in the gamut it can reach, and in
//! nothing else.

use crate::matrix::{self, Mat3f};
use crate::{Gamut, Ictcp, REFERENCE_WHITE_NITS, Radiance, SCRGB_UNITY_NITS, Scrgb, ictcp};

/// Bisection steps for the tone stage's solve for `I`. The axis is `[0, 1]`, so 18
/// steps put the bracket at 4e-6 — far below anything a display resolves.
const INTENSITY_STEPS: u32 = 18;

/// What the window's current display can present.
///
/// The arms carry different data because they *are* different: a white level is
/// meaningful only where composition is scene-referred, and panel primaries are
/// reachable only where Windows colour-manages. Reading a white level on a
/// wide-gamut display is a compile error rather than a brightness bug.
///
/// Populated from `Windows.Graphics.Display.AdvancedColorInfo`'s
/// `CurrentAdvancedColorKind`, primaries and luminances. Not from DXGI:
/// `IDXGIOutput6` cannot distinguish a wide-gamut display from a plain SDR one, so a
/// DXGI-sourced capability would select Rec.709 and discard the whole wide gamut on
/// exactly the class of display auto colour management exists to serve.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum DisplayCapability {
    /// `StandardDynamicRange`. Not colour-managed: scRGB is interpreted as sRGB and
    /// everything outside `[0, 1]` clips, in both directions.
    Sdr,
    /// `WideColorGamut`. FP16 scRGB composition and auto colour management to the
    /// panel, but display-referred: `1.0` is the panel's white, nothing above it.
    WideGamut {
        /// The panel's primaries.
        gamut: Gamut,
    },
    /// `HighDynamicRange`. Scene-referred: `1.0` is 80 nits absolute, the OS SDR
    /// white level is real, and there is headroom to the panel's peak.
    HighDynamicRange {
        /// The panel's primaries.
        gamut: Gamut,
        /// `SdrWhiteLevelInNits` — where diffuse white should land.
        white_nits: f32,
        /// `MaxLuminanceInNits` — the small-area peak, which is the right ceiling
        /// while everything authored above diffuse white is small-area.
        peak_nits: f32,
    },
}

/// Everything the current display implies, folded into one cheap function.
///
/// A plain value: construct one per display-capability change and hand it to whoever
/// draws. There is no global state, so it is testable with no device and two
/// capabilities can be compared side by side in one process.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct OutputTransform {
    /// Authored nits -> presented nits.
    exposure: f32,
    /// Presented nits -> scRGB.
    encode: f32,
    /// The containment target for the gamut stage.
    gamut: Gamut,
    /// Rec.2020 -> Rec.709, for the encode.
    to_709: Mat3f,
    /// Presented diffuse white, in nits. The shoulder's anchor.
    white: f32,
    /// Presented headroom above diffuse white, in nits. Zero on a display-referred
    /// desktop, which degenerates the shoulder to a hue-preserving clamp.
    head: f32,
    /// Presented nits above which the shoulder engages. Infinite when the content
    /// already fits the display, which is how "no tonemap" stops being a mode.
    knee: f32,
    /// The exposed content peak, for the debug assertion in [`OutputTransform::apply`].
    peak_limit: f32,
}

impl OutputTransform {
    /// Resolve a capability into a transform.
    ///
    /// `content_peak_nits` is the application's **mastering statement**: the brightest
    /// *channel* its palette authors, which for a saturated colour is well above that
    /// colour's ICtCp intensity — [`Radiance::peak_nits`] over the resolved palette is
    /// the way to compute it. It decides whether the shoulder engages at all, so
    /// under-declaring it means content above the display's ceiling reaches the
    /// compositor and is clipped per channel. The debug assertion in
    /// [`OutputTransform::apply`] reports that.
    #[must_use]
    pub fn for_display(cap: DisplayCapability, content_peak_nits: f32) -> Self {
        let (exposure, encode, gamut, ceiling) = match cap {
            DisplayCapability::Sdr => (
                1.0,
                1.0 / REFERENCE_WHITE_NITS,
                Gamut::REC709,
                REFERENCE_WHITE_NITS,
            ),
            DisplayCapability::WideGamut { gamut } => {
                (1.0, 1.0 / REFERENCE_WHITE_NITS, gamut, REFERENCE_WHITE_NITS)
            }
            DisplayCapability::HighDynamicRange {
                gamut,
                white_nits,
                peak_nits,
            } => {
                let exposure = white_nits / REFERENCE_WHITE_NITS;
                let white = REFERENCE_WHITE_NITS * exposure;
                (
                    exposure,
                    1.0 / SCRGB_UNITY_NITS,
                    gamut,
                    peak_nits.max(white),
                )
            }
        };

        let white = REFERENCE_WHITE_NITS * exposure;
        let peak_limit = content_peak_nits.max(REFERENCE_WHITE_NITS) * exposure;

        Self {
            exposure,
            encode,
            gamut,
            to_709: Gamut::REC709.matrix_from_2020(),
            white,
            head: (ceiling - white).max(0.0),
            // Engage only when the content actually overflows the display. When it
            // fits — an HDR panel with real headroom — every colour passes at its
            // authored luminance and the speculars punch, instead of being
            // compressed for no reason.
            knee: if peak_limit > ceiling {
                white
            } else {
                f32::INFINITY
            },
            peak_limit,
        }
    }

    /// What to hold before the first capability arrives: the `Sdr` arm.
    ///
    /// Deliberately **not** an identity. An identity map presents diffuse white at
    /// scRGB 2.5375, and on a display-referred desktop the whole upper palette clips
    /// per channel and rotates hue. The `Sdr` arm clips nothing on any of the three
    /// desktops, and on an HDR panel merely renders at reference white until the real
    /// capability arrives one event later.
    ///
    /// There is no `Default` impl, because a safe transform cannot be built without
    /// knowing the content peak.
    #[must_use]
    pub fn pre_fit(content_peak_nits: f32) -> Self {
        Self::for_display(DisplayCapability::Sdr, content_peak_nits)
    }

    /// Authored scene light -> the value handed to the compositor.
    ///
    /// This is the **only** function producing an [`Scrgb`], and nothing converts one
    /// back, so the display transform runs exactly once per colour by construction
    /// rather than by discipline.
    ///
    /// Each stage spends only the resource that ran out. Luminance ran out — the
    /// display cannot go brighter — so the tone stage spends luminance and holds hue
    /// and chroma exactly. Chroma runs out only when the panel's primaries cannot make
    /// a chromaticity, and only then does the gamut stage spend chroma, holding hue
    /// and intensity.
    #[must_use]
    pub fn apply(&self, c: Radiance) -> Scrgb {
        // 1. Exposure: authored nits -> presented nits. A pure scale of the triple, so
        //    it holds everything.
        let mut v = [
            c.r * self.exposure,
            c.g * self.exposure,
            c.b * self.exposure,
        ];

        // 2. Tone: lower `I` until the peak channel reaches the ceiling, holding `Ct`
        //    and `Cp`. Hue and chroma survive exactly; only luminance is spent.
        //
        //    The peak is taken in the DISPLAY's primaries, not in the working space.
        //    The two differ, and not by a little: the Rec.2020 -> display matrix has
        //    negative off-diagonal terms, so a saturated colour's display-space channel
        //    runs above its working-space one. Bounding the working peak leaves the
        //    panel to clip exactly what this stage exists to protect.
        //
        //    And on the peak channel rather than on `I` directly, because `I` is
        //    achromatic and bounding it bounds nothing the container cares about: a
        //    saturated blue sits comfortably under an `I` knee while its blue channel
        //    is far over the ceiling and clips to cyan. The container bounds channels,
        //    so the channels decide how much to spend — and `I` is what pays.
        let peak = self.display_peak(v);
        debug_assert!(
            peak <= self.peak_limit * 1.001 + 1e-3,
            "a Radiance channel exceeds the declared content peak: {peak} presented nits \
             against a limit of {}. Raise content_peak_nits or lower the token.",
            self.peak_limit
        );
        if peak > self.knee {
            v = ictcp::to_2020(self.spend_intensity(ictcp::from_2020(v), self.shoulder(peak)));
        }

        // 3. Gamut: only when the display's primaries cannot make this chromaticity.
        //    Gated on one matrix apply and three sign tests, because the compression
        //    itself costs a PQ round trip.
        //
        //    This order is forced, and it is closed. Stage 2 raises chroma relative to
        //    intensity, so it can push a colour out of gamut — hence gamut second.
        //    Compression moves toward the achromatic axis, which lowers the peak
        //    channel, so it can never push back over the ceiling. Neither stage can
        //    undo the other's guarantee.
        let t = matrix::apply(&self.gamut.matrix_from_2020(), v);
        if t[0] < 0.0 || t[1] < 0.0 || t[2] < 0.0 {
            v = ictcp::to_2020(self.gamut.compress(ictcp::from_2020(v)));
        }

        // 4. Encode: Rec.2020 -> Rec.709 primaries, presented nits -> scRGB. Values
        //    outside [0, 1] are correct and are the point: negative components carry
        //    colours outside Rec.709, and components above 1.0 carry luminance above
        //    the display's white.
        let o = matrix::apply(&self.to_709, v);
        Scrgb {
            r: o[0] * self.encode,
            g: o[1] * self.encode,
            b: o[2] * self.encode,
            a: c.a,
        }
    }

    /// The brightest channel this colour asks the display for, in presented nits.
    #[inline]
    fn display_peak(&self, v: [f32; 3]) -> f32 {
        let t = matrix::apply(&self.gamut.matrix_from_2020(), v);
        t[0].max(t[1]).max(t[2])
    }

    /// Lower `I` until the display-space peak channel lands on `target`, holding `Ct`
    /// and `Cp` — so the colour arrives with the hue **and the chroma** it was authored
    /// with, at whatever luminance the display could give it.
    ///
    /// Scaling the linear triple instead would hold chromaticity, which sounds
    /// equivalent and is not: PQ is compressive, so a uniform scale in linear light
    /// shrinks the colour's ICtCp chroma. A specular rolled from 380 to 203 nits comes
    /// out roughly a tenth less colourful — a few JND under ΔE-ITP, and in the wrong
    /// direction. It reads washed out rather than merely dimmer, and it delivers a
    /// different colour than the palette authored, differently on every panel.
    ///
    /// Bisection, because the map from `I` to the peak channel runs through PQ and has
    /// no closed-form inverse. It is monotone — `I` is the intensity axis — which is
    /// asserted over a hue and chroma sweep rather than assumed.
    fn spend_intensity(&self, c: Ictcp, target: f32) -> Ictcp {
        let (mut lo, mut hi) = (0.0f32, c.i);
        for _ in 0..INTENSITY_STEPS {
            let mid = 0.5 * (lo + hi);
            if self.peak_at(Ictcp::new(mid, c.ct, c.cp)) <= target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Ictcp::new(lo, c.ct, c.cp)
    }

    /// The display-space peak channel of an ICtCp coordinate, in nits.
    #[inline]
    fn peak_at(&self, c: Ictcp) -> f32 {
        let v = self.gamut.rgb(c);
        v[0].max(v[1]).max(v[2])
    }

    /// The shoulder, in presented nits. Three properties, and they are the contract:
    ///
    /// 1. **Identity at and below diffuse white.** A user interface's diffuse white is
    ///    a hard anchor — it has to match every other window on the desktop — and the
    ///    ladder beneath it is the design. Only content *above* white is negotiable.
    /// 2. **Slope 1 and continuous at white**, so there is no visible break where the
    ///    shoulder starts.
    /// 3. **Asymptotic to the display's ceiling**, so nothing can exceed it however
    ///    bright it was authored, and with no headroom it degenerates to a clamp at
    ///    white. That clamp is spent out of `I` alone, so a red specular arrives as the
    ///    most colourful red the display can hold — where the compositor's per-channel
    ///    clip would have rotated it toward orange.
    ///
    /// This is deliberately *not* BT.2390's EETF. That curve maps a mastered source
    /// range onto a display range and compresses everything in between, which is right
    /// for HDR video — where no particular luminance is privileged — and wrong here:
    /// it would roll diffuse white itself down to about 0.89 of the display's white,
    /// leaving the interface visibly dimmer than every window beside it.
    fn shoulder(&self, m: f32) -> f32 {
        if self.head <= 0.0 {
            return self.white;
        }
        self.head.mul_add(
            -(-(m - self.white) / self.head).exp(),
            self.white + self.head,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::{D65, inv, mul, narrow, rgb_to_xyz};

    /// Rec.709 -> Rec.2020, so a result can be read back as an [`Ictcp`]. The contract
    /// this module has to meet is stated in hue and chroma, so the assertions are made
    /// there rather than on encoded channels.
    const M_709_TO_2020: Mat3f = narrow(mul(
        inv(rgb_to_xyz(
            (0.708, 0.292),
            (0.170, 0.797),
            (0.131, 0.046),
            D65,
        )),
        rgb_to_xyz((0.640, 0.330), (0.300, 0.600), (0.150, 0.060), D65),
    ));

    /// The display-referred arms' nits-to-scRGB scalar.
    const DISPLAY_REFERRED_ENCODE: f32 = 1.0 / REFERENCE_WHITE_NITS;

    /// Read a transform's output back as the colour it actually delivered.
    fn delivered(o: Scrgb, encode: f32) -> Ictcp {
        let nits = [o.r / encode, o.g / encode, o.b / encode];
        ictcp::from_2020(matrix::apply(&M_709_TO_2020, nits))
    }

    /// A palette's worth of authored light: a card surface, disabled and primary text,
    /// the accent, and a specular above white. Chroma values are modest, as real
    /// interface tokens are — a colour's ICtCp intensity is not its peak channel, and
    /// a heavily saturated one carries a channel far above its intensity.
    fn samples() -> Vec<Radiance> {
        vec![
            Ictcp::polar(3.7, 0.004, 250.0).to_radiance(1.0),
            Ictcp::polar(30.0, 0.0, 0.0).to_radiance(1.0),
            Ictcp::polar(165.0, 0.06, 220.0).to_radiance(1.0),
            Ictcp::polar(244.0, 0.0, 0.0).to_radiance(1.0),
            Ictcp::polar(380.0, 0.05, 15.0).to_radiance(1.0),
        ]
    }

    /// The brightest channel any sample authors — what an application computes over
    /// its own resolved palette to declare its content peak.
    fn content_peak() -> f32 {
        samples()
            .into_iter()
            .fold(0.0f32, |m, c| m.max(c.peak_nits()))
    }

    fn sdr() -> OutputTransform {
        OutputTransform::for_display(DisplayCapability::Sdr, content_peak())
    }
    fn wcg() -> OutputTransform {
        OutputTransform::for_display(
            DisplayCapability::WideGamut {
                gamut: Gamut::DISPLAY_P3,
            },
            content_peak(),
        )
    }
    fn hdr(white: f32, peak: f32) -> OutputTransform {
        OutputTransform::for_display(
            DisplayCapability::HighDynamicRange {
                gamut: Gamut::DISPLAY_P3,
                white_nits: white,
                peak_nits: peak,
            },
            content_peak(),
        )
    }

    /// The transform's linear regime: what `apply` would produce if the shoulder never
    /// engaged, obtained by evaluating far below the knee and scaling back up. Uses
    /// only the public API, and is basis-independent -- which matters, because the
    /// output is in Rec.709 and the input is in Rec.2020, so comparing channel `r` to
    /// channel `r` across that boundary compares two different things.
    fn untoned(out: &OutputTransform, c: Radiance) -> [f32; 3] {
        const T: f32 = 1e-3;
        let o = out.apply(Radiance::new(c.r * T, c.g * T, c.b * T, c.a));
        [o.r / T, o.g / T, o.b / T]
    }

    fn white() -> Radiance {
        Radiance::new(
            REFERENCE_WHITE_NITS,
            REFERENCE_WHITE_NITS,
            REFERENCE_WHITE_NITS,
            1.0,
        )
    }

    /// Diffuse white lands exactly where each desktop defines white. This is the whole
    /// luminance model in three assertions, and the one an interface cannot get wrong:
    /// a white that is not white reads as a dim application next to every other window.
    #[test]
    fn diffuse_white_lands_on_the_display_white() {
        // Display-referred: 1.0 IS the display's white.
        for out in [sdr(), wcg()] {
            let w = out.apply(white());
            assert!((w.r - 1.0).abs() < 1e-3, "display-referred white = {}", w.r);
        }
        // Scene-referred: white tracks the OS SDR white level, and scRGB 1.0 is 80
        // nits, so a 240-nit setting lands at 3.0.
        let w = hdr(240.0, 800.0).apply(white());
        assert!((w.r - 3.0).abs() < 3e-3, "scene-referred white = {}", w.r);
    }

    /// Everything at or below diffuse white passes through untouched, on every
    /// desktop. The shoulder spends the above-white range and nothing else.
    #[test]
    fn the_diffuse_ladder_is_exact() {
        for out in [sdr(), wcg()] {
            for nits in [1.0f32, 3.7, 30.0, 100.0, 203.0] {
                let o = out.apply(Radiance::new(nits, nits, nits, 1.0));
                let want = nits / REFERENCE_WHITE_NITS;
                assert!(
                    (o.r - want).abs() < 1e-4,
                    "{nits} nits -> {} want {want}",
                    o.r
                );
            }
        }
    }

    /// The headline claim: SDR is a tonemap, not a mode. An HDR capability whose white
    /// and peak are both the reference must present the same **luminance** as the SDR
    /// arm, colour for colour -- the exposure, gamut and tone stages are identical and
    /// only the encode differs.
    ///
    /// The encoded numbers are deliberately *not* equal, and that is not a caveat: the
    /// two desktops define scRGB `1.0` differently, so identical light is identical
    /// only after each is read back through its own encode. Asserting numeric equality
    /// here would be asserting that one of the two desktops is wrong about its units.
    #[test]
    fn sdr_is_not_a_mode() {
        let a = sdr();
        let b = OutputTransform::for_display(
            DisplayCapability::HighDynamicRange {
                gamut: Gamut::REC709,
                white_nits: REFERENCE_WHITE_NITS,
                peak_nits: REFERENCE_WHITE_NITS,
            },
            content_peak(),
        );
        for c in samples() {
            let (x, y) = (a.apply(c), b.apply(c));
            // Back to presented nits through each desktop's own definition of 1.0.
            let xn = [x.r, x.g, x.b].map(|v| v * REFERENCE_WHITE_NITS);
            let yn = [y.r, y.g, y.b].map(|v| v * SCRGB_UNITY_NITS);
            for k in 0..3 {
                assert!(
                    (xn[k] - yn[k]).abs() < 1e-2,
                    "{c:?}: sdr {xn:?} nits vs equivalent hdr {yn:?} nits"
                );
            }
        }
    }

    /// Nothing may reach the compositor above the display's ceiling on a
    /// display-referred desktop, because there is nothing above white to reach.
    ///
    /// The bound is asserted twice, in two different bases, because the transform
    /// bounds the peak in the *panel's* primaries and Rec.709 is only the wire
    /// encoding. On the plain SDR arm those coincide, so the channel bound is exact.
    /// On a wide-gamut panel they do not: a colour the panel reaches comfortably can
    /// read above 1.0 in Rec.709, because reaching the same chromaticity from
    /// narrower primaries takes more signal — and that value is correct, not an
    /// overflow. What must hold on both is luminance, which is basis-independent.
    #[test]
    fn nothing_exceeds_the_ceiling_when_display_referred() {
        // Rec.709 luminance weights; Y is a physical quantity and does not care which
        // primaries encoded it.
        fn luminance(o: Scrgb) -> f32 {
            0.2126 * o.r + 0.7152 * o.g + 0.0722 * o.b
        }

        let sdr = sdr();
        for c in samples() {
            let o = sdr.apply(c);
            let peak = o.r.max(o.g).max(o.b);
            assert!(peak <= 1.0 + 1e-4, "sdr {c:?} -> peak {peak}");
        }

        for out in [sdr, wcg()] {
            let white_y = luminance(out.apply(white()));
            for c in samples() {
                let y = luminance(out.apply(c));
                assert!(
                    y <= white_y + 1e-4,
                    "{c:?} -> luminance {y} over white {white_y}"
                );
            }
        }
    }

    /// And nothing may go negative when the target is Rec.709, for the same reason in
    /// the other direction.
    #[test]
    fn nothing_goes_negative_when_the_target_is_rec709() {
        let out = sdr();
        for c in samples() {
            let o = out.apply(c);
            assert!(
                o.r >= -1e-3 && o.g >= -1e-3 && o.b >= -1e-3,
                "{c:?} -> {o:?}"
            );
        }
    }

    /// The test the whole Rec.2020-working-space argument exists for: a colour inside
    /// the panel's gamut but outside Rec.709 must survive to the compositor as a
    /// **negative** scRGB component. If this ever passes trivially, the pipeline has
    /// quietly become Rec.709-limited and the wide gamut is gone.
    #[test]
    fn wide_gamut_survives_as_a_negative_component() {
        let c = Ictcp::polar(100.0, 0.25, 150.0).to_radiance(1.0);
        assert!(
            Gamut::DISPLAY_P3.contains(c.to_ictcp()),
            "fixture is not inside P3"
        );
        assert!(
            !Gamut::REC709.contains(c.to_ictcp()),
            "fixture is inside Rec.709"
        );

        let out = OutputTransform::for_display(
            DisplayCapability::WideGamut {
                gamut: Gamut::DISPLAY_P3,
            },
            c.peak_nits(),
        );
        let o = out.apply(c);
        assert!(
            o.r < 0.0 || o.g < 0.0 || o.b < 0.0,
            "wide-gamut colour lost its negative component: {o:?}"
        );
    }

    /// The contract, and the reason this module exists: compression spends the
    /// resource that ran out. Luminance ran out; hue and chroma did not, so they
    /// arrive intact.
    #[test]
    fn tone_spends_intensity_and_holds_hue_and_chroma() {
        let authored = Ictcp::polar(380.0, 0.04, 15.0);
        let c = authored.to_radiance(1.0);
        let out = OutputTransform::for_display(DisplayCapability::Sdr, c.peak_nits());
        assert!(
            c.peak_nits() > REFERENCE_WHITE_NITS,
            "fixture is not above white"
        );
        assert!(
            Gamut::REC709.contains(authored),
            "fixture must not trip the gamut stage, or chroma is legitimately spent"
        );

        let got = delivered(out.apply(c), DISPLAY_REFERRED_ENCODE);
        assert!(got.nits() < authored.nits(), "no luminance was spent");
        assert!(
            (got.hue() - authored.hue()).abs() < 0.05,
            "hue moved: {} -> {}",
            authored.hue(),
            got.hue()
        );
        assert!(
            (got.chroma() - authored.chroma()).abs() < 1e-4,
            "chroma was spent: {} -> {}",
            authored.chroma(),
            got.chroma()
        );
    }

    /// The same claim, measured against the design this replaces. A uniform scale of
    /// the linear triple holds chromaticity, which sounds equivalent and is not: PQ is
    /// compressive, so it sheds ICtCp chroma and the colour arrives washed out rather
    /// than merely dimmer. This records how much, so a later reader tempted back to the
    /// obvious implementation sees the number instead of re-deriving the argument.
    #[test]
    fn a_uniform_scale_would_have_shed_chroma() {
        let authored = Ictcp::polar(380.0, 0.04, 15.0);
        let c = authored.to_radiance(1.0);
        let out = OutputTransform::for_display(DisplayCapability::Sdr, c.peak_nits());

        let got = delivered(out.apply(c), DISPLAY_REFERRED_ENCODE);

        // What a uniform scale to the same peak would have delivered.
        let lin = untoned(&out, c);
        let o = out.apply(c);
        let s = o.r.max(o.g).max(o.b) / lin[0].max(lin[1]).max(lin[2]);
        let uniform = delivered(
            Scrgb {
                r: lin[0] * s,
                g: lin[1] * s,
                b: lin[2] * s,
                a: 1.0,
            },
            DISPLAY_REFERRED_ENCODE,
        );

        let shed = 100.0 * (1.0 - uniform.chroma() / authored.chroma());
        assert!(
            shed > 2.0,
            "the uniform scale was expected to shed chroma; it shed {shed}%"
        );
        assert!(
            got.chroma() > uniform.chroma(),
            "the delivered colour should be more colourful than the uniform scale's"
        );

        // And the trade, pinned so that nobody later "optimises" toward it. Holding
        // chroma costs MORE intensity to reach the same peak channel, and ΔE-ITP is
        // dominated by intensity, so the uniform scale is the ΔE-closer of the two by
        // construction — exactly as a per-channel clip beats hue-preserving gamut
        // compression on the same metric. Distance is not the objective. The objective
        // is that a token arrives with the hue and chroma it was authored with, so that
        // it is the same token on every panel; luminance is the only parameter the
        // display actually took away.
        assert!(
            authored.delta_itp(uniform) < authored.delta_itp(got),
            "expected the uniform scale to win on ΔE; got {} vs {}",
            authored.delta_itp(uniform),
            authored.delta_itp(got)
        );
    }

    /// The premise the tone stage's bisection rests on. `I` is the intensity axis, so
    /// the display-space peak should rise with it — but the LMS-to-display matrix has
    /// negative terms, so this is asserted over a hue and chroma sweep rather than
    /// assumed. Chroma is swept well past anything a palette authors.
    #[test]
    fn the_display_peak_is_monotonic_in_intensity() {
        for gamut in [Gamut::REC709, Gamut::DISPLAY_P3, Gamut::REC2020] {
            for hue_step in 0..36 {
                let hue = hue_step as f32 * 10.0;
                for chroma_step in 0..=8 {
                    let chroma = chroma_step as f32 * 0.05;
                    let axes = Ictcp::polar(1.0, chroma, hue);
                    let mut prev = f32::NEG_INFINITY;
                    for i_step in 0..=200 {
                        let i = i_step as f32 / 200.0;
                        let v = gamut.rgb(Ictcp::new(i, axes.ct, axes.cp));
                        let peak = v[0].max(v[1]).max(v[2]);
                        assert!(
                            peak >= prev - 1e-3,
                            "peak fell at I={i} (hue {hue}, chroma {chroma}): {peak} < {prev}"
                        );
                        prev = peak;
                    }
                }
            }
        }
    }

    /// The payoff over the per-channel clip it replaces: the roll holds the channel
    /// ratios that carry hue, and the clip crushes them.
    #[test]
    fn beats_the_clip_on_hue() {
        let c = Ictcp::polar(300.0, 0.03, 250.0).to_radiance(1.0);
        let out = OutputTransform::for_display(DisplayCapability::Sdr, c.peak_nits());
        assert!(
            Gamut::REC709.contains(c.to_ictcp()),
            "fixture must not trip the gamut stage"
        );

        // What the colour would have been with no tone stage, and what the
        // compositor's per-channel clip would then do to it.
        let lin = untoned(&out, c);
        assert!(
            lin.iter().any(|&v| v > 1.0),
            "fixture does not actually clip"
        );
        let clipped = delivered(
            Scrgb {
                r: lin[0].min(1.0),
                g: lin[1].min(1.0),
                b: lin[2].min(1.0),
                a: 1.0,
            },
            DISPLAY_REFERRED_ENCODE,
        );

        let authored = c.to_ictcp();
        let rolled = delivered(out.apply(c), DISPLAY_REFERRED_ENCODE);
        assert!(
            (rolled.hue() - authored.hue()).abs() < 0.05,
            "the roll moved hue: {} -> {}",
            authored.hue(),
            rolled.hue()
        );
        assert!(
            (clipped.hue() - authored.hue()).abs() > 1.0,
            "the clip should have visibly moved hue, moved {}",
            (clipped.hue() - authored.hue()).abs()
        );
    }

    /// Brighter content stays brighter. A curve that inverts a pair would scramble the
    /// palette's luminance hierarchy, which is the one thing a designer relies on.
    #[test]
    fn the_curve_is_monotonic() {
        let out = OutputTransform::for_display(DisplayCapability::Sdr, 700.0);
        let mut prev = -1.0f32;
        for step in 0..=700 {
            let n = step as f32;
            let o = out.apply(Radiance::new(n, n, n, 1.0));
            assert!(
                o.r >= prev - 1e-6,
                "not monotonic at {n} nits: {} < {prev}",
                o.r
            );
            prev = o.r;
        }
    }

    /// Content that already fits its display is not touched at all: the knee is
    /// infinite and the transform is a pure scale. This is "no tonemap" without an
    /// enum arm to select it, and it is what lets speculars punch on a real HDR panel.
    #[test]
    fn content_that_fits_is_a_pure_scale() {
        let out = hdr(240.0, 1000.0);
        for c in samples() {
            assert!(Gamut::DISPLAY_P3.contains(c.to_ictcp()), "fixture left P3");
            let o = out.apply(c);
            // "Pure scale" is a statement about the map, not about any one channel:
            // halving the input halves the output exactly.
            let half = out.apply(Radiance::new(c.r * 0.5, c.g * 0.5, c.b * 0.5, c.a));
            for (full, h) in [(o.r, half.r), (o.g, half.g), (o.b, half.b)] {
                assert!(
                    (full - 2.0 * h).abs() < 1e-5 * full.abs().max(1.0),
                    "{c:?} -> {o:?}, wanted linearity"
                );
            }
        }
    }

    /// A dimmer panel rolls harder. The map reads the peak, so "works on any display"
    /// is a queried parameter rather than an assumption.
    #[test]
    fn a_lower_panel_peak_rolls_more() {
        let c = Ictcp::polar(380.0, 0.05, 15.0).to_radiance(1.0);
        let bright = hdr(240.0, 1500.0).apply(c).r;
        let dim = hdr(240.0, 260.0).apply(c).r;
        assert!(
            dim < bright,
            "260-nit peak {dim} should roll below 1500-nit {bright}"
        );
    }

    /// The property that licenses compositing *after* the transform: DWM forms only
    /// convex combinations, and a convex combination of two in-range values is in
    /// range. Post-transform blending can therefore introduce neither a clip nor an
    /// out-of-gamut value.
    #[test]
    fn convex_blends_stay_in_range() {
        let out = sdr();
        let cs: Vec<Scrgb> = samples().into_iter().map(|c| out.apply(c)).collect();
        for a in &cs {
            for b in &cs {
                for step in 0..=10 {
                    let t = step as f32 / 10.0;
                    for ch in [
                        a.r + (b.r - a.r) * t,
                        a.g + (b.g - a.g) * t,
                        a.b + (b.b - a.b) * t,
                    ] {
                        assert!((-1e-3..=1.0 + 1e-3).contains(&ch), "blend left range: {ch}");
                    }
                }
            }
        }
    }

    /// With no headroom the shoulder degenerates to a clamp at white — and the clamp
    /// is a uniform scale of the triple, so a red specular stays red and only its
    /// brightness caps. That is the whole difference from the compositor's clip.
    #[test]
    fn no_headroom_clamps_at_white_without_moving_hue() {
        let c = Ictcp::polar(380.0, 0.05, 15.0).to_radiance(1.0);
        let out = OutputTransform::for_display(DisplayCapability::Sdr, c.peak_nits());
        let o = out.apply(c);
        let peak = o.r.max(o.g).max(o.b);
        assert!((peak - 1.0).abs() < 1e-4, "did not land on white: {peak}");

        let authored = c.to_ictcp();
        let got = delivered(o, DISPLAY_REFERRED_ENCODE);
        assert!(
            (got.hue() - authored.hue()).abs() < 0.05,
            "the clamp moved hue"
        );
        assert!(
            (got.chroma() - authored.chroma()).abs() < 1e-4,
            "the clamp spent chroma: {} -> {}",
            authored.chroma(),
            got.chroma()
        );
    }

    /// Debug-only: the assertion it exercises is compiled out of a release build,
    /// where an under-declared peak degrades to the compositor's clip rather than to a
    /// panic. Reporting it is a development affordance, not a runtime guarantee.
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "exceeds the declared content peak")]
    fn over_peak_content_is_reported() {
        let out = OutputTransform::for_display(DisplayCapability::Sdr, REFERENCE_WHITE_NITS);
        let _ = out.apply(Radiance::new(900.0, 900.0, 900.0, 1.0));
    }
}
