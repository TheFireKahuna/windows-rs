//! The one injection object, and the lifecycle each device class has on it.
//!
//! `InputInjector` covers mouse, pen, touch and the keyboard. Touch and pen additionally have
//! an initialize/uninitialize pair: a pen sample injected without `InitializePenInjection` is
//! accepted, returns success and delivers nothing. The initialize is owned here and
//! refcounted per class, so it is released when the last stream of that class drops rather
//! than per stream — uninitializing under a live stream stops delivery without an error.
//!
//! Visualization is `None`. The system's touch circles and pen feedback are pixels drawn over
//! the window a visual comparison is made against.

use crate::bindings::*;
use crate::{Error, Result};

/// Holds the injection object and the per-class initializes taken out on it.
pub(crate) struct Injection {
    injector: InputInjector,
    pen: u32,
}

impl Injection {
    /// Obtains the injection object, or reports why the session refused one.
    pub(crate) fn open() -> Result<Self> {
        // Activation initializes this thread's apartment if nothing has: `FactoryCache` falls
        // back to `CoIncrementMTAUsage`, so a caller needs no apartment of its own.
        let injector =
            InputInjector::TryCreate().map_err(|e| Error::call("InputInjector::TryCreate", e))?;
        Ok(Self { injector, pen: 0 })
    }

    pub(crate) const fn injector(&self) -> &InputInjector {
        &self.injector
    }

    /// Initializes pen injection for as long as at least one pen stream is open. A pen sample
    /// injected without this is accepted and delivered nowhere.
    pub(crate) fn acquire_pen(&mut self) -> Result<()> {
        if self.pen == 0 {
            self.injector
                .InitializePenInjection(InjectedInputVisualizationMode::None)
                .map_err(|e| Error::call("InitializePenInjection", e))?;
        }
        self.pen += 1;
        Ok(())
    }

    /// Drops one pen stream's claim, uninitializing pen injection when the last one goes.
    pub(crate) fn release_pen(&mut self) {
        self.pen = self.pen.saturating_sub(1);
        if self.pen == 0 {
            _ = self.injector.UninitializePenInjection();
        }
    }
}

impl Drop for Injection {
    /// Releases any initialize still outstanding, including one a panicking drive left.
    fn drop(&mut self) {
        if self.pen > 0 {
            _ = self.injector.UninitializePenInjection();
        }
    }
}
