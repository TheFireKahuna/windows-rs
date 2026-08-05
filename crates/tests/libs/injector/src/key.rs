//! The keys a focus or dismiss assertion needs.
//!
//! A key has no position, no contact and no path, so nothing is carried between calls and
//! there is no stream type here. `Tab`, `Esc` and `Enter` are asserted against the same
//! window as the pointer contracts, and `InputInjector` already covers the keyboard.
//!
//! The set is closed: the binding filter names exactly these keys, so nothing outside
//! [`Key`] can be pressed.

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
    /// Invokes the focused control, and toggles a control that toggles.
    Space,
    /// An arrow key, which a composite control consumes rather than moving focus with.
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

/// Injects one key transition: a release when `up`, a press otherwise.
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
