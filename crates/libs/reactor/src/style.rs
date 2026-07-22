use std::{
    any::{Any, TypeId},
    borrow::Cow,
    cell::Cell,
    collections::HashMap,
    sync::OnceLock,
    time::Duration,
};

use rustc_hash::FxHashMap;

use super::*;

impl Thickness {
    pub const fn uniform(v: f64) -> Self {
        Self {
            left: v,
            top: v,
            right: v,
            bottom: v,
        }
    }
    pub const fn xy(x: f64, y: f64) -> Self {
        Self {
            left: x,
            top: y,
            right: x,
            bottom: y,
        }
    }
}

impl From<f64> for Thickness {
    fn from(v: f64) -> Self {
        Self::uniform(v)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum GridLength {
    Auto,
    Pixel(f64),
    Star(f64),
    /// As many equal tracks as fit, each at least this many DIPs wide, sharing
    /// the leftover space — CSS `repeat(auto-fill, minmax(N, 1fr))`.
    ///
    /// The track *count* is solved from the container's own size, so a panel
    /// that reflows with its width says so declaratively instead of measuring
    /// itself, deriving a column count and re-placing every child — which puts
    /// layout downstream of layout and costs a second full pass every resize.
    ///
    /// Meaningful as the sole entry in a track list; children take their cells
    /// from grid auto-placement, so they carry no row/column of their own.
    AutoFill(f64),
}

impl GridLength {
    pub const STAR: Self = Self::Star(1.0);
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct WindowSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct InnerConstraints {
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub max_width: Option<f64>,
    pub max_height: Option<f64>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ScalarTransition {
    pub duration: Duration,
}

impl ScalarTransition {
    pub const fn new(duration: Duration) -> Self {
        Self { duration }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Vector3Transition {
    pub duration: Duration,
    pub components: Vector3Axes,
}

impl Vector3Transition {
    pub const fn new(duration: Duration) -> Self {
        Self {
            duration,
            components: Vector3Axes::All,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Vector3Axes {
    X,
    Y,
    Z,
    Xy,
    All,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct ImplicitTransitions {
    pub opacity: Option<ScalarTransition>,
    pub rotation: Option<ScalarTransition>,
    pub scale: Option<Vector3Transition>,
    pub translation: Option<Vector3Transition>,
}

impl ImplicitTransitions {
    pub fn is_empty(&self) -> bool {
        self.opacity.is_none()
            && self.rotation.is_none()
            && self.scale.is_none()
            && self.translation.is_none()
    }
}

/// Implicit transitions applied to layout-managed properties of an
/// element (offset/size). Fed to the backend via `set_layout_animation`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LayoutAnimationConfig {
    pub duration: Duration,
    pub use_spring: bool,
    pub damping_ratio: f32,
    pub period: f32,
    pub animate_offset: bool,
    pub animate_size: bool,
}

impl Default for LayoutAnimationConfig {
    fn default() -> Self {
        Self {
            duration: Duration::from_millis(300),
            use_spring: false,
            damping_ratio: 0.6,
            period: 0.08,
            animate_offset: true,
            animate_size: false,
        }
    }
}

impl LayoutAnimationConfig {
    pub fn linear(duration: Duration) -> Self {
        Self {
            duration,
            ..Self::default()
        }
    }

    pub fn spring() -> Self {
        Self {
            use_spring: true,
            ..Self::default()
        }
    }

    pub fn animate_size(mut self, v: bool) -> Self {
        self.animate_size = v;
        self
    }

    pub fn animate_offset(mut self, v: bool) -> Self {
        self.animate_offset = v;
        self
    }
}

/// One-shot property animation (opacity / scale / …) driven by
/// `Backend::run_property_animation`. Also the payload of enter/exit
/// transitions (`ElementExt::transition`).
///
/// `opacity`/`scale` are the animation's end values; `from_opacity`/`from_scale`
/// optionally pin the start. A `None` start animates from the property's
/// current value — the right default for state changes and retargeting — while
/// an explicit start makes mount/unmount effects deterministic (a fade-in must
/// start at 0 regardless of the visual's resting opacity).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AnimationConfig {
    pub opacity: Option<f64>,
    pub scale: Option<f64>,
    /// Starting opacity; `None` starts from the current value.
    pub from_opacity: Option<f64>,
    /// Starting uniform scale; `None` starts from the current value.
    pub from_scale: Option<f64>,
    pub duration: Duration,
    pub easing: Easing,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            opacity: None,
            scale: None,
            from_opacity: None,
            from_scale: None,
            duration: Duration::from_millis(300),
            easing: Easing::EaseOut,
        }
    }
}

impl AnimationConfig {
    /// Fade from fully transparent to fully opaque.
    pub fn fade_in(duration: Duration) -> Self {
        Self {
            opacity: Some(1.0),
            from_opacity: Some(0.0),
            duration,
            easing: Easing::EaseOut,
            ..Self::default()
        }
    }

    /// Fade from the current opacity to fully transparent.
    pub fn fade_out(duration: Duration) -> Self {
        Self {
            opacity: Some(0.0),
            duration,
            easing: Easing::EaseIn,
            ..Self::default()
        }
    }

    /// Fade in while growing from a slight shrink — the Fluent "pop" entrance.
    pub fn pop_in(duration: Duration) -> Self {
        Self {
            opacity: Some(1.0),
            from_opacity: Some(0.0),
            scale: Some(1.0),
            from_scale: Some(0.96),
            duration,
            easing: Easing::EaseOut,
        }
    }

    /// Fade out while shrinking slightly — the matching exit.
    pub fn pop_out(duration: Duration) -> Self {
        Self {
            opacity: Some(0.0),
            scale: Some(0.96),
            duration,
            easing: Easing::EaseIn,
            ..Self::default()
        }
    }

    /// Set the end scale (uniform).
    pub fn with_scale(mut self, to: f64) -> Self {
        self.scale = Some(to);
        self
    }

    /// Pin the starting opacity.
    pub fn starting_opacity(mut self, from: f64) -> Self {
        self.from_opacity = Some(from);
        self
    }

    /// Pin the starting scale (uniform).
    pub fn starting_scale(mut self, from: f64) -> Self {
        self.from_scale = Some(from);
        self
    }

    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Whether running this config would visibly change anything.
    pub fn is_visible_effect(&self) -> bool {
        self.opacity.is_some() || self.scale.is_some()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Easing {
    Linear,
    EaseOut,
    EaseIn,
    EaseInOut,
}

/// Combined animation block stored on [`Modifiers`]`.animations`. All
/// fields are optional and applied independently by the backend.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct AnimationModifiers {
    pub implicit_transitions: Option<ImplicitTransitions>,
    pub layout_animation: Option<LayoutAnimationConfig>,
    pub property_animation: Option<AnimationConfig>,
    pub enter_transition: Option<AnimationConfig>,
    pub exit_transition: Option<AnimationConfig>,
}

impl AnimationModifiers {
    pub fn is_empty(&self) -> bool {
        self.implicit_transitions
            .as_ref()
            .is_none_or(|t| t.is_empty())
            && self.layout_animation.is_none()
            && self.property_animation.is_none()
            && self.enter_transition.is_none()
            && self.exit_transition.is_none()
    }
}

/// Effective color scheme reported by
/// [`RenderCx::use_color_scheme`].
#[derive(Copy, Clone, Debug, Default, Hash, PartialEq, Eq)]
pub enum ColorScheme {
    #[default]
    Light,
    Dark,
}

/// Process-global, not thread-local: the effective scheme is a fact about the
/// process — app-side hooks (`use_color_scheme`) and front-side chrome paint
/// both read it, and under the render-thread split those sit on different
/// threads. One atomic serves both; a thread-local here would leave whichever
/// side didn't get the push painting the old palette.
static CURRENT_COLOR_SCHEME: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(0);

fn scheme_from_u8(v: u8) -> ColorScheme {
    if v == 1 { ColorScheme::Dark } else { ColorScheme::Light }
}

/// Read the host's last-known effective [`ColorScheme`].
pub fn current_color_scheme() -> ColorScheme {
    scheme_from_u8(CURRENT_COLOR_SCHEME.load(std::sync::atomic::Ordering::Acquire))
}

/// Update the effective [`ColorScheme`]; called by the host when the effective
/// theme changes (and once during startup/attach). Release-ordered so the
/// invalidation fan-out that follows a theme flip observes the new value on
/// every thread it reaches.
pub fn set_current_color_scheme(scheme: ColorScheme) {
    let v = match scheme {
        ColorScheme::Light => 0,
        ColorScheme::Dark => 1,
    };
    CURRENT_COLOR_SCHEME.store(v, std::sync::atomic::Ordering::Release);
}

/// Requested application theme: an app-level override of the OS light/dark
/// setting. `Default` follows the system; `Light`/`Dark` force the scheme.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum RequestedTheme {
    /// Use the system default (inherits from the OS app-theme setting).
    #[default]
    Default,
    /// Force light theme.
    Light,
    /// Force dark theme.
    Dark,
}

/// The app's requested theme override — the single source of truth both hosts
/// resolve against. Process-global for the same reason as the scheme: the
/// setter fires from app code and the host resolves it at apply time, and
/// under the render-thread split those are different threads. Holding it here
/// (rather than in host-specific state) also makes a call before the host
/// exists naturally pending: the host reads it at startup/attach.
static REQUESTED_THEME: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Host-installed hook that applies a theme change to the live window. Absent
/// until a host runs (the stored request applies then). `Send + Sync` is part
/// of the contract: the setter may fire on a thread that is not the one the
/// host pumps on, so the hook must carry the change over itself (post to the
/// window, never touch host state directly).
static THEME_APPLIER: std::sync::Mutex<
    Option<std::sync::Arc<dyn Fn(RequestedTheme) + Send + Sync>>,
> = std::sync::Mutex::new(None);

/// Set the application theme override. Takes effect immediately when a host is
/// live; otherwise it is applied when one starts.
pub fn set_requested_theme(theme: RequestedTheme) {
    let v = match theme {
        RequestedTheme::Default => 0,
        RequestedTheme::Light => 1,
        RequestedTheme::Dark => 2,
    };
    REQUESTED_THEME.store(v, std::sync::atomic::Ordering::Release);
    let applier = THEME_APPLIER.lock().ok().and_then(|a| a.clone());
    if let Some(applier) = applier {
        applier(theme);
    }
}

/// Read the app's requested theme override.
pub fn requested_theme() -> RequestedTheme {
    match REQUESTED_THEME.load(std::sync::atomic::Ordering::Acquire) {
        1 => RequestedTheme::Light,
        2 => RequestedTheme::Dark,
        _ => RequestedTheme::Default,
    }
}

/// Install (or clear) the host hook [`set_requested_theme`] routes through. The
/// hook may fire from inside event dispatch, and — under the render-thread
/// split — from a thread that is not the host's: it must defer by carrying the
/// change to the host's own thread (post through its message pump), never by
/// borrowing host state synchronously.
pub(crate) fn set_theme_applier(
    applier: Option<std::sync::Arc<dyn Fn(RequestedTheme) + Send + Sync>>,
) {
    if let Ok(mut slot) = THEME_APPLIER.lock() {
        *slot = applier;
    }
}

// ── URI launch ───────────────────────────────────────────────────────────────
//
// Backend-neutral, which is why it lives here and not in the DComp backend: it
// is app policy, not rendering. The WinUI backend never reaches this — it hands
// the URI to `HyperlinkButton.SetNavigateUri` and XAML mediates the navigation
// inside its own trust context — but the declaration is not made backend-shaped
// for that, any more than [`set_requested_theme`] is.

type UriLauncher = Box<dyn Fn(&str) -> bool + Send + Sync + 'static>;

/// The app-installed launcher. Set once before the window exists; a later
/// registration is ignored (matches the visibility / display-change setters).
static URI_LAUNCHER: OnceLock<UriLauncher> = OnceLock::new();

/// Install the process-global URI launcher a [`HyperlinkButton`] activation
/// routes through. Call **before** the window is created (like
/// [`crate::set_window_visibility_callback`]); only the first registration is
/// kept.
///
/// **There is no default.** Without a launcher a hyperlink is inert: it paints,
/// it focuses, it fires its `Click` handler and its UIA `Invoke` — and nothing
/// is launched. This crate has no URI-launch primitive of its own and does not
/// acquire one by falling back to the shell.
///
/// The launcher receives the control's `NavigateUri` verbatim and returns
/// whether it handled it. Both halves are the point: the app DECIDES (which
/// schemes, which hosts, whether a confirmation prompt is due) and the app ACTS
/// (`ShellExecuteW`, an in-app browser, a queued work item). Returning `false`
/// means "not handled" and is a normal, silent outcome — this crate does not
/// take a declined URI anywhere else.
///
/// # What the app is taking responsibility for
///
/// The string is **untrusted**: it came from whatever built the element tree,
/// which for a data-driven UI may be a config file, an IPC payload, or a remote
/// document. This crate deliberately does **not** parse it, resolve it,
/// percent-decode it, canonicalise it, or judge its scheme — every one of those
/// is a policy decision (`file:`, `ms-settings:`, a custom protocol handler
/// registered by another app) whose right answer depends on what the host
/// application is and what it trusts, and a wrong answer baked in here would be
/// a wrong answer no app could override. The only thing rejected before the
/// launcher is called is a string that is not structurally a URI reference at
/// all — see [`launch_uri`]. Scheme allow-listing is the launcher's job, and it
/// is not optional: handing this straight to `ShellExecuteW` executes whatever
/// protocol handler the machine has registered.
///
/// Three things the structural gate specifically does **not** cover, because
/// each is a judgement only the app can make:
///
/// - **Percent-encoding is not decoded.** `%00`, `%0A` and `%2E%2E%2F` reach the
///   launcher as those literal characters. The gate rejects a literal control
///   character; it cannot reject one that does not exist until something
///   decodes. If your launcher percent-decodes — or hands the string to
///   something that does — apply the same structural check again on the decoded
///   form, and expect an embedded NUL there.
/// - **The authority is not judged.** IDN, punycode and homoglyphs pass through:
///   `https://xn--80ak6aa92e.com/` is a well-formed URI reference and this crate
///   has no basis for deciding it is not the host you meant. An app that shows
///   the target to a user should render the decoded form, or the ASCII form,
///   deliberately — not whichever one it happened to get.
/// - **Nothing is canonicalised.** Dot segments, case, and duplicate slashes
///   arrive as authored. Screen the string you will actually launch, not a
///   normalisation of it that you then discard.
///
/// # Being called is not evidence of a user gesture
///
/// Activation converges on one path, which a pointer release, a Space/Enter
/// press and a UI-Automation `Invoke` all share — deliberately, so a screen
/// reader follows a link exactly as a click does. UIA is cross-process: any
/// process able to automate this window can drive that `Invoke` with no user
/// present. That is not a new capability (a process that can automate your UI
/// can already press the button), and the URI is still one your own tree
/// supplied, so the launcher is not being handed anything an attacker authored.
/// But it does mean a launcher must not treat the call itself as proof a human
/// asked. If a scheme needs consent, prompt for it; do not infer it.
///
/// # Contract
///
/// When *this crate* routes a `HyperlinkButton` activation, the launcher runs
/// **on the UI thread**, from the message pump, outside any backend borrow —
/// never during layout, paint, or event dispatch. That is a property of the
/// routing, not of the seam: [`launch_uri`] is public and the `Send + Sync`
/// bound is real, so an app calling it directly can reach the launcher from any
/// thread, concurrently with itself, and re-entrantly from inside it. Nothing
/// here serialises those; write the launcher to tolerate them.
///
/// - It must not block. A synchronous `ShellExecuteW` on a cold handler can
///   stall for seconds; the window is frozen for exactly that long. Post the
///   real work to another thread and return `true`.
/// - It must not re-enter the reactor (no `set_state` from inside it); use the
///   dispatcher, as any other off-thread producer would.
/// - It may panic without taking the process down: the call is wrapped in the
///   same fault boundary as an event handler, and the fault is reported under
///   the `"uri launcher"` context. The URI is **not** part of that report — a
///   fault handler that logs, and a hyperlink carrying a capability URL, must
///   not combine into a leak. Two limits on that, both inherited from the fault
///   module rather than chosen here: the boundary deliberately does not catch
///   during a render pass (so a launcher reached by calling [`launch_uri`] from
///   inside a component body is not covered), and the fault handler is a
///   `winui-backend` facility — on the DComp backend a caught panic goes to
///   stderr. The context string is still URI-free either way, but the panic
///   *message* is yours: do not put the URI in it.
///
/// Returns whether THIS registration is the one now installed. `false` means a
/// launcher was already present and yours was discarded — check it. First-wins
/// is the fail-safe direction (a component loaded later cannot silently swap out
/// the policy the app installed at startup), but it means losing the race is
/// indistinguishable from winning it unless you look: [`uri_launcher_installed`]
/// would answer `true` either way, and an app that assumed its own allow-list
/// was in force would render links as followable while someone else's policy
/// decided where they go.
pub fn set_uri_launcher(launcher: impl Fn(&str) -> bool + Send + Sync + 'static) -> bool {
    URI_LAUNCHER.set(Box::new(launcher)).is_ok()
}

/// Whether a launcher has been installed with [`set_uri_launcher`].
///
/// Lets a UI reflect the truth rather than lie about it — a link that cannot be
/// followed can be rendered as plain text instead of an affordance that does
/// nothing.
pub fn uri_launcher_installed() -> bool {
    URI_LAUNCHER.get().is_some()
}

/// Offer `uri` to the installed launcher; returns whether it was handled.
///
/// `false` covers all three of "no launcher installed" (the default),
/// "structurally not a URI reference", and "the launcher declined" — none of
/// which this crate treats as an error, and all of which end the same way:
/// nothing is launched.
///
/// The structural gate is deliberately the *only* filtering done here, and it
/// is not a security judgement — it rejects strings that cannot be a URI
/// reference under RFC 3986 no matter whose policy applies: the empty string,
/// anything with leading or trailing whitespace, and anything containing a
/// character RFC 3986 §2 does not admit literally — see
/// `not_a_uri_character`. That is the class of byte that would let one "URI"
/// become two arguments or two log lines further down, or render as one host
/// and parse as another. Everything a policy could reasonably differ on — the
/// scheme, the authority, the path, the encoding — is passed through untouched
/// for the launcher to judge.
///
/// The string handed to the launcher is the string that cleared the gate,
/// byte-for-byte. Nothing is trimmed, decoded or normalised on the way through,
/// so the launcher's allow-list and whatever eventually parses the URI are
/// looking at the same characters.
///
/// Public because it is the one chokepoint: an app rendering its own link-like
/// affordance should route through here rather than reaching for the shell
/// separately, so a single installed policy governs every launch.
pub fn launch_uri(uri: &str) -> bool {
    if uri.is_empty()
        // Rejected, NOT trimmed. Trimming and passing the trimmed string would
        // be a policy edit; passing the untrimmed one after testing `trim()` for
        // emptiness — which is what this used to do — is worse than either: the
        // app's allow-list reads a different string from the one the shell
        // eventually parses. `" file:///C:/Windows/System32/cmd.exe"` cleared
        // the old gate, and a launcher screening with `!uri.starts_with("file:")`
        // — the exact shape this module documents — called it handled.
        || uri.starts_with(char::is_whitespace)
        || uri.ends_with(char::is_whitespace)
        || uri.chars().any(not_a_uri_character)
    {
        return false;
    }
    let Some(launcher) = URI_LAUNCHER.get() else {
        return false;
    };
    // The launcher is app code and may panic; a panic here would otherwise
    // cross the window procedure's `extern "system"` boundary and abort. The
    // context string carries no part of the URI.
    let handled = Cell::new(false);
    fault::catch("uri launcher", || handled.set(launcher(uri)));
    handled.get()
}

/// A character that cannot appear literally in a URI reference, so rejecting it
/// is the same structural call the control-character test already makes rather
/// than a new policy one.
///
/// RFC 3986 §2 fixes the whole repertoire — ALPHA / DIGIT / the unreserved
/// marks / the reserved set / `%` — and nothing below is in it. To carry any of
/// these a URI must percent-encode them, and the encoded form passes through
/// here untouched, which is the point: this rejects the *literal* character, it
/// does not decode or judge what the URI is trying to say.
///
/// Why these and not merely `is_control`: `char::is_control` is Unicode category
/// Cc alone, so the old gate stopped CR and LF and NUL and U+0085 while letting
/// through the characters that do the same damage from outside Cc.
///
/// - **U+2028 LINE SEPARATOR, U+2029 PARAGRAPH SEPARATOR** — line terminators to
///   JavaScript and to several log formats. One URI, two log lines: precisely
///   what this gate exists to stop, and it did not stop it.
/// - **Other non-space whitespace** (U+00A0 NBSP, U+3000 IDEOGRAPHIC SPACE, …) —
///   a separator to anything tokenising on Unicode whitespace, and invisible or
///   near-invisible in the string the reviewer read.
/// - **Bidi controls and invisibles** (U+202E RIGHT-TO-LEFT OVERRIDE, the
///   isolates, U+200B ZWSP, U+FEFF, U+00AD SOFT HYPHEN) — these make the
///   rendered string and the parsed string differ, so the host a user sees is
///   not the host that is launched, and two different URIs can render
///   identically.
///
/// A plain U+0020 in the interior is deliberately still allowed: it is visible,
/// an app can see it and decide, and `file:///C:/Program Files/…` is a real
/// thing apps launch. Leading and trailing whitespace is rejected outright by
/// the caller — no URI has it, and it is what makes the app's reading of the
/// scheme diverge from the shell's.
fn not_a_uri_character(c: char) -> bool {
    c.is_control()
        || (c.is_whitespace() && c != ' ')
        || matches!(c,
            '\u{00AD}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}')
}

/// Symbolic reference to a WinUI XAML theme resource (resolved at apply
/// time so the binding tracks light/dark switches).
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ThemeRef {
    Accent,
    AccentSecondary,
    AccentTertiary,
    AccentDisabled,
    PrimaryText,
    SecondaryText,
    TertiaryText,
    DisabledText,
    AccentText,
    SolidBackground,
    CardBackground,
    SmokeFill,
    SubtleFill,
    LayerFill,
    ControlFill,
    ControlFillSecondary,
    ControlFillTertiary,
    ControlFillDisabled,
    ControlFillInputActive,
    CardStroke,
    SurfaceStroke,
    DividerStroke,
    ControlStroke,
    ControlStrokeSecondary,
    SystemAttention,
    SystemSuccess,
    SystemCaution,
    SystemCritical,
    SystemNeutral,
    SystemSolidNeutral,
    SystemAttentionBackground,
    SystemSuccessBackground,
    SystemCautionBackground,
    SystemCriticalBackground,
    SystemNeutralBackground,
    SystemSolidAttention,
    Custom(Cow<'static, str>),
}

impl ThemeRef {
    pub fn custom(key: impl Into<Cow<'static, str>>) -> Self {
        Self::Custom(key.into())
    }

    pub fn resource_key(&self) -> &str {
        match self {
            Self::Accent => "AccentFillColorDefaultBrush",
            Self::AccentSecondary => "AccentFillColorSecondaryBrush",
            Self::AccentTertiary => "AccentFillColorTertiaryBrush",
            Self::AccentDisabled => "AccentFillColorDisabledBrush",

            Self::PrimaryText => "TextFillColorPrimaryBrush",
            Self::SecondaryText => "TextFillColorSecondaryBrush",
            Self::TertiaryText => "TextFillColorTertiaryBrush",
            Self::DisabledText => "TextFillColorDisabledBrush",
            Self::AccentText => "AccentTextFillColorPrimaryBrush",

            Self::SolidBackground => "SolidBackgroundFillColorBaseBrush",
            Self::CardBackground => "CardBackgroundFillColorDefaultBrush",
            Self::SmokeFill => "SmokeFillColorDefaultBrush",
            Self::SubtleFill => "SubtleFillColorSecondaryBrush",
            Self::LayerFill => "LayerFillColorDefaultBrush",

            Self::ControlFill => "ControlFillColorDefaultBrush",
            Self::ControlFillSecondary => "ControlFillColorSecondaryBrush",
            Self::ControlFillTertiary => "ControlFillColorTertiaryBrush",
            Self::ControlFillDisabled => "ControlFillColorDisabledBrush",
            Self::ControlFillInputActive => "ControlFillColorInputActiveBrush",

            Self::CardStroke => "CardStrokeColorDefaultBrush",
            Self::SurfaceStroke => "SurfaceStrokeColorDefaultBrush",
            Self::DividerStroke => "DividerStrokeColorDefaultBrush",
            Self::ControlStroke => "ControlStrokeColorDefaultBrush",
            Self::ControlStrokeSecondary => "ControlStrokeColorSecondaryBrush",

            Self::SystemAttention => "SystemFillColorAttentionBrush",
            Self::SystemSuccess => "SystemFillColorSuccessBrush",
            Self::SystemCaution => "SystemFillColorCautionBrush",
            Self::SystemCritical => "SystemFillColorCriticalBrush",
            Self::SystemNeutral => "SystemFillColorNeutralBrush",
            Self::SystemSolidNeutral => "SystemFillColorSolidNeutralBrush",
            Self::SystemAttentionBackground => "SystemFillColorAttentionBackgroundBrush",
            Self::SystemSuccessBackground => "SystemFillColorSuccessBackgroundBrush",
            Self::SystemCautionBackground => "SystemFillColorCautionBackgroundBrush",
            Self::SystemCriticalBackground => "SystemFillColorCriticalBackgroundBrush",
            Self::SystemNeutralBackground => "SystemFillColorNeutralBackgroundBrush",
            Self::SystemSolidAttention => "SystemFillColorSolidAttentionBackgroundBrush",

            Self::Custom(s) => s.as_ref(),
        }
    }
}

/// Brush slot that can be either a literal [`Color`]
/// or a [`ThemeRef`]; used for `background` / `foreground` modifiers.
///
/// [`Color`] now carries `f32` linear channels (no `Eq`), so this derives only
/// `PartialEq` — the reconciler diffs it by exact value, which is correct because
/// tokens are computed deterministically.
#[derive(Clone, Debug, PartialEq)]
pub enum BrushBinding {
    Direct(Color),
    Theme(ThemeRef),
}

impl From<Color> for BrushBinding {
    fn from(c: Color) -> Self {
        Self::Direct(c)
    }
}

impl From<ThemeRef> for BrushBinding {
    fn from(v: ThemeRef) -> Self {
        Self::Theme(v)
    }
}

#[expect(non_upper_case_globals)]
pub mod tokens {
    use super::ThemeRef;

    pub const Accent: ThemeRef = ThemeRef::Accent;

    pub const AccentSecondary: ThemeRef = ThemeRef::AccentSecondary;

    pub const AccentTertiary: ThemeRef = ThemeRef::AccentTertiary;

    pub const AccentDisabled: ThemeRef = ThemeRef::AccentDisabled;

    pub const PrimaryText: ThemeRef = ThemeRef::PrimaryText;

    pub const SecondaryText: ThemeRef = ThemeRef::SecondaryText;

    pub const TertiaryText: ThemeRef = ThemeRef::TertiaryText;

    pub const DisabledText: ThemeRef = ThemeRef::DisabledText;

    pub const AccentText: ThemeRef = ThemeRef::AccentText;

    pub const SolidBackground: ThemeRef = ThemeRef::SolidBackground;

    pub const CardBackground: ThemeRef = ThemeRef::CardBackground;

    pub const SmokeFill: ThemeRef = ThemeRef::SmokeFill;

    pub const SubtleFill: ThemeRef = ThemeRef::SubtleFill;

    pub const LayerFill: ThemeRef = ThemeRef::LayerFill;

    pub const ControlFill: ThemeRef = ThemeRef::ControlFill;

    pub const ControlFillSecondary: ThemeRef = ThemeRef::ControlFillSecondary;

    pub const ControlFillTertiary: ThemeRef = ThemeRef::ControlFillTertiary;

    pub const ControlFillDisabled: ThemeRef = ThemeRef::ControlFillDisabled;

    pub const ControlFillInputActive: ThemeRef = ThemeRef::ControlFillInputActive;

    pub const CardStroke: ThemeRef = ThemeRef::CardStroke;

    pub const SurfaceStroke: ThemeRef = ThemeRef::SurfaceStroke;

    pub const DividerStroke: ThemeRef = ThemeRef::DividerStroke;

    pub const ControlStroke: ThemeRef = ThemeRef::ControlStroke;

    pub const ControlStrokeSecondary: ThemeRef = ThemeRef::ControlStrokeSecondary;

    pub const SystemAttention: ThemeRef = ThemeRef::SystemAttention;

    pub const SystemSuccess: ThemeRef = ThemeRef::SystemSuccess;

    pub const SystemCaution: ThemeRef = ThemeRef::SystemCaution;

    pub const SystemCritical: ThemeRef = ThemeRef::SystemCritical;

    pub const SystemNeutral: ThemeRef = ThemeRef::SystemNeutral;

    pub const SystemSolidNeutral: ThemeRef = ThemeRef::SystemSolidNeutral;
}

/// Visual modifiers shared by every widget; carried on each element struct
/// and applied uniformly via `FrameworkElement`-level setters at the
/// backend.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Modifiers {
    pub margin: Option<Thickness>,
    pub padding: Option<Thickness>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub min_width: Option<f64>,
    pub max_width: Option<f64>,
    pub min_height: Option<f64>,
    pub max_height: Option<f64>,
    pub horizontal_alignment: Option<HorizontalAlignment>,
    pub vertical_alignment: Option<VerticalAlignment>,
    pub opacity: Option<f64>,
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub font_family: Option<String>,
    pub font_size: Option<f64>,
    pub theme_bindings: Option<Box<FxHashMap<Prop, ThemeRef>>>,
    pub animations: Option<Box<AnimationModifiers>>,
    pub attached: Option<AttachedProps>,
    pub accessibility: Option<Box<AccessibilityModifiers>>,
    pub keyboard_accelerators: Vec<KeyboardAccelerator>,
    pub tooltip: Option<Box<Tooltip>>,
    pub pointer_handlers: Option<Box<PointerHandlers>>,
    pub allow_drop: Option<bool>,
    pub drag_handlers: Option<Box<DragHandlers>>,
    /// Fast-path for grid row/column placement — avoids the `AttachedProps`
    /// HashMap + Box + thread_local overhead for the most common attached prop.
    pub grid: Option<GridPlacement>,
    pub resources: HashMap<String, String>,
}

impl Modifiers {
    pub fn is_empty(&self) -> bool {
        self.margin.is_none()
            && self.padding.is_none()
            && self.width.is_none()
            && self.height.is_none()
            && self.min_width.is_none()
            && self.max_width.is_none()
            && self.min_height.is_none()
            && self.max_height.is_none()
            && self.horizontal_alignment.is_none()
            && self.vertical_alignment.is_none()
            && self.opacity.is_none()
            && self.background.is_none()
            && self.foreground.is_none()
            && self.font_family.is_none()
            && self.font_size.is_none()
            && self.theme_bindings.as_ref().is_none_or(|m| m.is_empty())
            && self.animations.as_ref().is_none_or(|a| a.is_empty())
            && self.attached.as_ref().is_none_or(|a| a.is_empty())
            && self.accessibility.as_deref().is_none_or(|a| a.is_empty())
            && self.keyboard_accelerators.is_empty()
            && self.tooltip.is_none()
            && self
                .pointer_handlers
                .as_deref()
                .is_none_or(|p| p.is_empty())
            && self.allow_drop.is_none()
            && self.drag_handlers.as_deref().is_none_or(|d| d.is_empty())
            && self.grid.is_none()
            && self.resources.is_empty()
    }
}

/// Type-erased bag of attached properties (e.g. [`GridPlacement`]) keyed
/// by [`TypeId`]; values must be inserted via [`AttachedProps::set`].
#[derive(Default, Debug)]
pub struct AttachedProps(FxHashMap<TypeId, Box<dyn AttachedValue>>);

impl Clone for AttachedProps {
    fn clone(&self) -> Self {
        let mut copy = FxHashMap::default();
        for (k, v) in &self.0 {
            copy.insert(*k, v.clone_box());
        }
        Self(copy)
    }
}

impl PartialEq for AttachedProps {
    fn eq(&self, other: &Self) -> bool {
        if self.0.len() != other.0.len() {
            return false;
        }
        for (k, v) in &self.0 {
            let Some(ov) = other.0.get(k) else {
                return false;
            };
            if !v.eq_box(ov.as_any()) {
                return false;
            }
        }
        true
    }
}

impl AttachedProps {
    pub fn set<T: Clone + PartialEq + 'static>(&mut self, v: T) {
        self.0.insert(TypeId::of::<T>(), Box::new(v));
    }
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.0
            .get(&TypeId::of::<T>())
            .and_then(|b| b.as_any().downcast_ref::<T>())
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GridPlacement {
    pub row: i32,
    pub column: i32,
    pub row_span: i32,
    pub column_span: i32,
}

impl Default for GridPlacement {
    fn default() -> Self {
        Self {
            row: 0,
            column: 0,
            row_span: 1,
            column_span: 1,
        }
    }
}

/// Trait object carrying clone/eq in its vtable so `AttachedProps` doesn't
/// need a separate type-registry thread-local.
trait AttachedValue: Any {
    fn clone_box(&self) -> Box<dyn AttachedValue>;
    fn eq_box(&self, other: &dyn Any) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T: Clone + PartialEq + 'static> AttachedValue for T {
    fn clone_box(&self) -> Box<dyn AttachedValue> {
        Box::new(self.clone())
    }
    fn eq_box(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| self == o)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl std::fmt::Debug for dyn AttachedValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AttachedValue")
    }
}

// --- Pointer event handlers ---

/// Bundle of per-element pointer / tap callbacks; each slot is
/// individually optional.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct PointerHandlers {
    pub on_tapped: Option<Callback<()>>,
    pub on_right_tapped: Option<Callback<()>>,
    pub on_pointer_pressed: Option<Callback<PointerEventInfo>>,
    pub on_pointer_released: Option<Callback<PointerEventInfo>>,
    pub on_pointer_moved: Option<Callback<PointerEventInfo>>,
    pub on_pointer_entered: Option<Callback<PointerEventInfo>>,
    pub on_pointer_exited: Option<Callback<()>>,
    pub on_pointer_wheel: Option<Callback<PointerEventInfo>>,
}

impl PointerHandlers {
    pub fn is_empty(&self) -> bool {
        self.on_tapped.is_none()
            && self.on_right_tapped.is_none()
            && self.on_pointer_pressed.is_none()
            && self.on_pointer_released.is_none()
            && self.on_pointer_moved.is_none()
            && self.on_pointer_entered.is_none()
            && self.on_pointer_exited.is_none()
            && self.on_pointer_wheel.is_none()
    }
}

/// Which axis a wheel event travelled on.
///
/// [`Vertical`](WheelAxis::Vertical) is the classic wheel (`WM_MOUSEWHEEL` /
/// WinUI `PointerWheelChanged`); [`Horizontal`](WheelAxis::Horizontal) is the
/// tilt-wheel or touchpad sideways pan (`WM_MOUSEHWHEEL`).
///
/// The two axes do **not** share a sign convention, because the platform's
/// don't: a positive vertical delta is *up / away from the user*, a positive
/// horizontal delta is *to the right*. Deltas are passed through raw rather
/// than normalised to some common "forward", so a sink reads each axis with
/// the convention its users already expect.
///
/// `Vertical` is the [`Default`] deliberately: a sink written before this
/// enum existed, and every non-wheel pointer callback (which leaves
/// `wheel_delta` at 0), sees precisely the axis it always implicitly assumed.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum WheelAxis {
    #[default]
    Vertical,
    Horizontal,
}

/// Pointer state captured at a pointer callback (`PointerPressed`,
/// `PointerReleased`, `PointerMoved`, `PointerEntered`, or
/// `PointerWheelChanged`). `x`/`y` are the pointer position in DIPs, relative
/// to the top-left of the element the handler is attached to. Non-mouse
/// pointer kinds report all three button flags as `false`. `wheel_delta` is
/// the raw `MouseWheelDelta` (120 per detent, signed) and is only meaningful
/// in a wheel callback; [`wheel_axis`](Self::wheel_axis) says which axis it
/// travelled on and is [`WheelAxis::Vertical`] everywhere else.
///
/// A sink that wants exactly one axis should read it through
/// [`wheel_delta_on`](Self::wheel_delta_on) rather than `wheel_delta`, so a
/// sideways tilt cannot drive a control that only ever meant to respond to the
/// vertical wheel.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct PointerEventInfo {
    pub x: f64,
    pub y: f64,
    pub is_left_button_pressed: bool,
    pub is_right_button_pressed: bool,
    pub is_middle_button_pressed: bool,
    pub wheel_delta: i32,
    pub wheel_axis: WheelAxis,
}

impl PointerEventInfo {
    /// The wheel delta if it arrived on `axis`, otherwise 0.
    ///
    /// This is the opt-in read: a control that adjusts a value on the vertical
    /// wheel calls `wheel_delta_on(WheelAxis::Vertical)` and is inert under a
    /// horizontal tilt, without having to match on the axis itself.
    pub fn wheel_delta_on(&self, axis: WheelAxis) -> i32 {
        if self.wheel_axis == axis { self.wheel_delta } else { 0 }
    }
}

// --- Accessibility ---

/// Which UI Automation views an element appears in.
///
/// A client walks one of three trees over the same structure. Excluding an
/// element does NOT hide its children: a view walker hoists them onto the
/// nearest ancestor that is still in the view, which is exactly what makes a
/// layout wrapper disappear without taking its content with it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccessibilityView {
    /// In the Content and Control views — anything a user reads or operates.
    #[default]
    Content,
    /// In the Control view only: structure a client can walk to, but nothing to
    /// read out. For a container that groups controls without labelling them.
    Control,
    /// In neither — reachable only by a Raw walk. Pure presentation: spacing
    /// borders, layout grids, decorative rules and dots.
    Raw,
}

impl AccessibilityView {
    /// `(IsControlElement, IsContentElement)`.
    pub(crate) fn flags(self) -> (bool, bool) {
        match self {
            Self::Content => (true, true),
            Self::Control => (true, false),
            Self::Raw => (false, false),
        }
    }
}

/// UI Automation properties applied to every widget kind via
/// [`Modifiers::accessibility`].
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct AccessibilityModifiers {
    pub automation_name: Option<String>,
    pub automation_id: Option<String>,
    pub help_text: Option<String>,
    pub live_setting: Option<AutomationLiveSetting>,
    pub heading_level: Option<AutomationHeadingLevel>,
    /// Overrides the view a backend would otherwise derive. `None` lets the
    /// backend decide — which for pure layout scaffolding means [`Raw`].
    ///
    /// [`Raw`]: AccessibilityView::Raw
    pub accessibility_view: Option<AccessibilityView>,
}

impl AccessibilityModifiers {
    pub fn is_empty(&self) -> bool {
        self.automation_name.is_none()
            && self.automation_id.is_none()
            && self.help_text.is_none()
            && self.live_setting.is_none()
            && self.heading_level.is_none()
            && self.accessibility_view.is_none()
    }
}

// --- Tooltip ---

/// Tooltip configuration applied via WinUI `ToolTipService`. Build from
/// a plain string or `Tooltip::rich(element)` for templated content.
#[derive(Clone, Debug, PartialEq)]
pub struct Tooltip {
    pub content: TooltipContent,
    pub placement: Option<TooltipPlacement>,
}

impl Tooltip {
    /// Plain-text tooltip; WinUI wraps the string in a default
    /// `ToolTip` `TextBlock`.
    pub fn text(s: impl Into<String>) -> Self {
        Self {
            content: TooltipContent::Text(s.into()),
            placement: None,
        }
    }

    /// Rich tooltip; `element` is mounted as the `Content` of a
    /// `ToolTip` instance at apply time.
    pub fn rich(element: impl Into<Element>) -> Self {
        Self {
            content: TooltipContent::Rich(Box::new(element.into())),
            placement: None,
        }
    }

    pub fn placement(mut self, p: TooltipPlacement) -> Self {
        self.placement = Some(p);
        self
    }
}

impl<S: Into<String>> From<S> for Tooltip {
    fn from(s: S) -> Self {
        Self::text(s)
    }
}

/// Tooltip payload: a plain string or a templated child element.
#[derive(Clone, Debug, PartialEq)]
pub enum TooltipContent {
    Text(String),
    Rich(Box<Element>),
}

/// Rust mirror of `Microsoft.UI.Xaml.Controls.Primitives.PlacementMode`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TooltipPlacement {
    Top,
    Bottom,
    Left,
    Right,
    Mouse,
}
