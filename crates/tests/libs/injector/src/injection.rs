//! The one injection object, and the lifecycle each device class has on it.
//!
//! `InputInjector` covers mouse, pen, touch and the keyboard. Touch and pen additionally
//! have an **initialize/uninitialize pair**, and that pair is not ceremony: a pen sample
//! injected without `InitializePenInjection` is accepted and goes nowhere, which is the
//! failure mode a return code cannot show and the one that costs an afternoon. So the
//! initialize is owned here, refcounted per class, and released when the last stream of
//! that class drops — never per stream, because uninitializing under a live stream would
//! silently stop delivering.
//!
//! **Visualization is `None`.** The system's touch circles and pen feedback are pixels a
//! visual diff would see, drawn over the thing being compared.

use crate::bindings::*;
use crate::{Error, Result};

/// The injector's handle on the platform, and what has been initialized on it.
pub(crate) struct Injection {
    injector: InputInjector,
    pen: u32,
}

impl Injection {
    /// Obtains the injection object, or says why the session would not give one.
    pub(crate) fn open() -> Result<Self> {
        // Activation initializes this thread's apartment on its own if nothing has —
        // `FactoryCache` falls back to `CoIncrementMTAUsage` — so a harness has no apartment
        // discipline to get wrong.
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

    pub(crate) fn release_pen(&mut self) {
        self.pen = self.pen.saturating_sub(1);
        if self.pen == 0 {
            _ = self.injector.UninitializePenInjection();
        }
    }
}

impl Drop for Injection {
    /// Releases whatever a panicking test left initialized.
    fn drop(&mut self) {
        if self.pen > 0 {
            _ = self.injector.UninitializePenInjection();
        }
    }
}
