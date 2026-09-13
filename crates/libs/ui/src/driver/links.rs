//! What the input, scene and app threads share: the mailboxes between them, the doorbells
//! two of them park on, and the few facts a thread needs from the window it never touches.
//!
//! Every buffer here is allocated once. A producer hands a filled buffer through a mailbox
//! and takes an empty one back through the spare beside it, so two buffers serve a pair of
//! threads for the life of the window. Where the spare is not back yet, the producer keeps
//! appending to the buffer it holds — the app thread instead skips a flush, since the model
//! keeps its own pending ops — and retries on its next pass. No thread waits on another.

use crate::seam::{Down, InputDown, Mailbox, Ring, ToScene, Up};
use core::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock};
use windows_core::{Error, HRESULT};
use windows_text::FontLadder;
use windows_window::{Event, Hwnd};

/// The links between the threads, shared by `Arc`.
pub(super) struct Links {
    /// The window, as a token: the scene thread's target and the worker threads' close.
    pub hwnd: Hwnd,

    /// App → scene: the patch and the rows beside it.
    pub down: Mailbox<Down>,
    pub down_spare: Mailbox<Down>,
    /// Input → scene: the tick's reports and the window facts they came with.
    pub to_scene: Mailbox<ToScene>,
    pub to_scene_spare: Mailbox<ToScene>,
    /// Scene → app: events, intents and the discrete reports.
    pub up: Mailbox<Up>,
    pub up_spare: Mailbox<Up>,
    /// Scene → input: the hit array copy and the front tables' rows.
    pub input_down: Mailbox<InputDown>,
    pub input_down_spare: Mailbox<InputDown>,

    /// The scene thread's doorbell, rung by the app, the input thread and the present
    /// thread's binder. It rides in the scene thread's message wait.
    pub scene_ring: Arc<Ring>,
    /// The app thread's doorbell, rung by the scene thread and by a `Cell::post`.
    pub app_ring: Arc<Ring>,

    /// Set by the input thread when the pump has returned; each worker checks it after
    /// every wake and drains once more before leaving.
    pub stop_app: AtomicBool,
    pub stop_scene: AtomicBool,

    /// Set by a producer that had a batch to send and no spare to send it in; the consumer
    /// rings the producer when it returns one only while this is set, so a returned spare
    /// nobody was waiting for wakes nobody.
    pub app_wants_down_spare: AtomicBool,
    pub scene_wants_up_spare: AtomicBool,
    pub scene_wants_input_spare: AtomicBool,

    /// Signalled by the scene thread once its scene exists or its start-up failed, so the
    /// input thread neither shows the window over nothing nor starts the app thread against
    /// a font ladder that was never made.
    pub scene_ready: Event,
    /// Signalled by the scene thread after its first apply, which is when the window has
    /// content to show.
    pub first_frame: Event,
    /// The font ladder the scene thread's backends hold, for the app thread's shaping engine.
    /// Two engines over one ladder agree on every face id; two ladders do not.
    pub ladder: OnceLock<FontLadder>,

    /// The first failure any worker recorded: its code and message. `windows_core::Error`
    /// may carry a COM error object, so the two plain halves cross instead.
    pub failure: Mutex<Option<(HRESULT, String)>>,
}

impl Links {
    /// Creates the links, with every buffer allocated: one in each producer's hand and one
    /// in each spare.
    ///
    /// # Errors
    ///
    /// A doorbell's or a start-up event's kernel object could not be created.
    pub(super) fn new(hwnd: Hwnd) -> windows_core::Result<Self> {
        let links = Self {
            hwnd,
            down: Mailbox::empty(),
            down_spare: Mailbox::empty(),
            to_scene: Mailbox::empty(),
            to_scene_spare: Mailbox::empty(),
            up: Mailbox::empty(),
            up_spare: Mailbox::empty(),
            input_down: Mailbox::empty(),
            input_down_spare: Mailbox::empty(),
            scene_ring: Arc::new(Ring::new()?),
            app_ring: Arc::new(Ring::new()?),
            stop_app: AtomicBool::new(false),
            stop_scene: AtomicBool::new(false),
            app_wants_down_spare: AtomicBool::new(false),
            scene_wants_up_spare: AtomicBool::new(false),
            scene_wants_input_spare: AtomicBool::new(false),
            scene_ready: Event::auto_reset()?,
            first_frame: Event::auto_reset()?,
            ladder: OnceLock::new(),
            failure: Mutex::new(None),
        };
        // The spares. Each producer's own first buffer is made where the producer starts.
        // A put into an empty mailbox cannot fail.
        _ = links.down_spare.put(Box::default());
        _ = links.to_scene_spare.put(Box::default());
        _ = links.up_spare.put(Box::default());
        _ = links.input_down_spare.put(Box::default());
        Ok(links)
    }

    /// Records a worker's failure, keeping the first, and asks the window to close so the
    /// pump returns and the input thread joins the workers and reports it.
    pub(super) fn fail(&self, error: &Error) {
        self.record(error.code(), error.message());
    }

    /// As [`fail`](Self::fail), for a failure that is not a `windows_core::Error`: a panic.
    pub(super) fn fail_with(&self, message: &str) {
        self.record(windows_window::E_HANDLE, message.to_owned());
    }

    fn record(&self, code: HRESULT, message: String) {
        if let Ok(mut slot) = self.failure.lock()
            && slot.is_none()
        {
            *slot = Some((code, message));
        }
        self.hwnd.close();
    }

    /// Returns the recorded failure as an error, if any worker failed.
    pub(super) fn failure(&self) -> Option<Error> {
        self.failure
            .lock()
            .ok()
            .and_then(|slot| slot.clone())
            .map(|(code, message)| Error::new(code, message))
    }
}

/// Ends a worker thread's run: records a panic as a failure and closes the window.
///
/// Held on the worker's stack for the whole run, so a panic anywhere in it reaches the
/// input thread as a closed window and an error out of `run`, rather than as a window that
/// keeps pumping over a thread that is gone.
pub(super) struct Guard<'a> {
    pub links: &'a Links,
    pub name: &'static str,
    /// Events the thread's start-up promised to signal, whether or not it got that far.
    pub release: &'a [&'a Event],
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.links.fail_with(&format!("the {} thread panicked", self.name));
        }
        for event in self.release {
            event.signal();
        }
    }
}
