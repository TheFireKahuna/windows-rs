//! The keys a focus or dismiss assertion needs.
//!
//! Not a pointer stream and not shaped like one: a key has no position, no contact and no
//! path, so there is nothing for a stream type to carry between calls. It is here because
//! `Tab`, `Esc` and `Enter` are asserted against the same window as the pointer contracts,
//! and because the injection object already covers the keyboard — a second harness existing
//! only to reach it would be a second harness.
//!
//! The set is closed on purpose. A harness carrying the whole virtual-key table invites a
//! test to drive something it is not testing, and the binding filter names exactly these.

use windows_collections::IIterable;

use crate::bindings::*;
use crate::{Error, Result};

/// A key this harness can press.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Key {
    /// Moves focus to the next control, or out of an overlay's scope at its end.
    Tab,
    /// Dismisses the innermost focus scope.
    Escape,
    /// Invokes the focused control.
    Enter,
    /// Invokes the focused control, and is the one a toggle answers to.
    Space,
    /// Arrow keys, which a composite control consumes rather than moving focus with.
    Left,
    /// See [`Key::Left`].
    Right,
    /// See [`Key::Left`].
    Up,
    /// See [`Key::Left`].
    Down,
}

impl Key {
    const fn vk(self) -> u16 {
        (match self {
            Self::Tab => VK_TAB,
            Self::Escape => VK_ESCAPE,
            Self::Enter => VK_RETURN,
            Self::Space => VK_SPACE,
            Self::Left => VK_LEFT,
            Self::Right => VK_RIGHT,
            Self::Up => VK_UP,
            Self::Down => VK_DOWN,
        }) as u16
    }
}

/// One key transition.
pub(crate) fn send(injector: &InputInjector, key: Key, up: bool) -> Result<()> {
    let info = InjectedInputKeyboardInfo::new()
        .map_err(|e| Error::call("InjectedInputKeyboardInfo::new", e))?;
    let fill = || -> windows_core::Result<()> {
        info.SetVirtualKey(key.vk())?;
        info.SetKeyOptions(if up {
            InjectedInputKeyOptions::KeyUp
        } else {
            InjectedInputKeyOptions::None
        })
    };
    fill().map_err(|e| Error::call("InjectedInputKeyboardInfo", e))?;
    injector
        .InjectKeyboardInput(&IIterable::<InjectedInputKeyboardInfo>::from(vec![Some(
            info,
        )]))
        .map_err(|e| Error::call("InjectKeyboardInput", e))
}
