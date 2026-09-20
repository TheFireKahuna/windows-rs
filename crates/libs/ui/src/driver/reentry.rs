//! Where one input pass stands, as the window procedure sees it.
//!
//! The procedure runs inside the system's nested pumps, so a pass can already be on the stack
//! when a frame wake, a pointer message or a removed key arrives. Three questions are asked
//! from there and none of them may borrow the tick: whether a pass is running, whether a wake
//! that arrived during one still owes a pass, and whether the discrete input a key would be
//! offered behind has been routed yet.

use core::cell::Cell;
use windows_core::Result;

#[derive(Default)]
pub(super) struct Reentry {
    running: Cell<bool>,
    again: Cell<bool>,
    serviced: Cell<bool>,
}

impl Reentry {
    /// Claims the pass, reporting whether the caller owns it.
    ///
    /// A caller that does not own it has recorded that another pass is owed, so a wake taken
    /// from a nested pump is never dropped.
    pub(super) fn enter(&self) -> bool {
        if self.running.replace(true) {
            self.again.set(true);
            return false;
        }
        self.serviced.set(false);
        true
    }

    /// Takes the owed pass, reporting whether there was one. Clears the service mark, because
    /// the next pass routes its own input.
    pub(super) fn take_again(&self) -> bool {
        self.serviced.set(false);
        self.again.replace(false)
    }

    /// Releases the pass.
    pub(super) fn exit(&self) {
        self.running.set(false);
        self.again.set(false);
        self.serviced.set(false);
    }

    /// Marks this pass's discrete input routed and its text focus applied.
    pub(super) fn serviced(&self) {
        self.serviced.set(true);
    }

    /// Reports whether a removed key may be offered to TSF without a pass of its own.
    ///
    /// Outside a pass it may not: the click or Tab in front of it is still in the doorbell.
    /// Inside one it may, once that pass has routed its input and moved text focus — which is
    /// the state every nested pump a text service, the clipboard or automation opens runs in.
    pub(super) fn may_offer(&self) -> bool {
        self.running.get() && self.serviced.get()
    }

    /// Reports whether a pass is on the stack.
    pub(super) fn running(&self) -> bool {
        self.running.get()
    }

    /// Runs `pass` until none is owed, doing nothing where one is already on the stack.
    ///
    /// # Errors
    ///
    /// `pass` failed. Whatever was still owed is abandoned with it.
    pub(super) fn passes(&self, mut pass: impl FnMut() -> Result<()>) -> Result<()> {
        if !self.enter() {
            return Ok(());
        }
        let outcome = loop {
            let ran = pass();
            if ran.is_err() || !self.take_again() {
                break ran;
            }
        };
        self.exit();
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wake_taken_during_a_pass_is_owed_rather_than_dropped() {
        let phase = Reentry::default();
        assert!(phase.enter(), "the first caller owns the pass");
        assert!(!phase.enter(), "a nested wake does not");
        assert!(phase.take_again(), "and is owed one");
        assert!(!phase.take_again(), "exactly once");
        phase.exit();
        assert!(phase.enter());
    }

    #[test]
    fn a_pass_runs_again_for_the_wake_its_own_nested_pump_took() {
        let phase = Reentry::default();
        let runs = Cell::new(0u32);
        phase
            .passes(|| {
                runs.set(runs.get() + 1);
                if runs.get() == 1 {
                    // What a nested pump's frame wake reaches, with this pass on the stack.
                    assert!(!phase.enter());
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(runs.get(), 2, "the owed pass ran before the first returned");
        assert!(phase.enter(), "and the pass was released");
    }

    #[test]
    fn a_failed_pass_abandons_what_was_owed_and_releases_the_pass() {
        let phase = Reentry::default();
        let runs = Cell::new(0u32);
        let failed = phase.passes(|| {
            runs.set(runs.get() + 1);
            assert!(!phase.enter());
            Err(windows_core::Error::empty())
        });
        assert!(failed.is_err());
        assert_eq!(runs.get(), 1);
        assert!(phase.enter());
    }

    #[test]
    fn a_key_is_offered_only_inside_a_pass_that_has_serviced_its_input() {
        let phase = Reentry::default();
        assert!(!phase.may_offer(), "outside a pass, the doorbell is unread");
        assert!(phase.enter());
        assert!(!phase.may_offer(), "before the router runs, focus is stale");
        phase.serviced();
        assert!(phase.may_offer());
        assert!(!phase.enter());
        assert!(phase.take_again(), "the owed pass routes its own input");
        assert!(!phase.may_offer());
        phase.exit();
        assert!(!phase.may_offer());
    }
}
