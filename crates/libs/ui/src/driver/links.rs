//! What the input, scene and app threads share: the four seams between them, the doorbells two
//! of them park on, and the few facts a thread needs from the window it never touches.
//!
//! Every buffer here is allocated once. A producer hands a filled buffer through a seam and
//! takes an empty one back through the spare beside it, so two buffers serve a pair of threads
//! for the life of the window. Where the spare is not back yet, the producer keeps appending
//! to the buffer it holds — the app thread instead skips a flush, since the host keeps its own
//! pending ops — and retries on its next pass. No thread waits on another.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering::*};

use windows_core::{Error, HRESULT, Result};
use windows_text::FontLadder;
use windows_window::{E_HANDLE, Event, Hwnd, Window};

use crate::seam::{Down, InputDown, Link, Ring, ToScene, Up};
use std::sync::Arc;

/// The links between the threads, shared by `Arc`.
pub(crate) struct Links {
    /// The frame, as a token: the worker threads' close and the frame wake.
    pub(crate) window: Hwnd,
    /// The content window, which the scene thread's composition target is bound to.
    pub(crate) content: Hwnd,
    /// App → scene: the patch and the rows beside it.
    pub(crate) down: Link<Down>,
    /// Input → scene: the tick's reports and the window facts they came with.
    pub(crate) to_scene: Link<ToScene>,
    /// Scene → app: events, intents and the discrete reports.
    pub(crate) up: Link<Up>,
    /// Scene → input: the hit array copy and the front tables' rows.
    pub(crate) input: Link<InputDown>,
    /// The scene thread's doorbell, rung by the app, the input thread and the present thread's
    /// binder. It rides in the scene thread's message wait.
    ///
    /// Behind an `Arc` because the present thread's binder holds one for the life of the
    /// presenter, and that binder is handed to a thread this crate does not own.
    pub(crate) scene_bell: Arc<Ring>,
    /// The app thread's doorbell, rung by the scene thread and by a `Cell::post`.
    ///
    /// Behind an `Arc` because the signal graph's cross-thread routing takes ownership of one:
    /// a producer on any thread stages its write and rings this.
    pub(crate) app_bell: Arc<Ring>,
    /// Set by the input thread when the pump has returned; each worker checks it after every
    /// wake and drains once more before leaving.
    pub(crate) stop: AtomicBool,
    /// Whether a provider is listening, so the tree is built only for somebody who reads it.
    pub(crate) uia_listening: AtomicBool,
    /// Set when a provider asked for a tree the app thread has not published yet.
    pub(crate) uia_requested: AtomicBool,
    /// Signalled by the scene thread once its scene exists or its start-up failed, so the input
    /// thread neither shows the window over nothing nor starts the app thread against a font
    /// ladder that was never made.
    pub(crate) scene_ready: Event,
    /// Signalled by the scene thread after its first apply, which is when the window has
    /// content to show.
    pub(crate) first_patch: Event,
    /// The font ladder the scene thread's backends hold, for the app thread's shaping engine.
    /// Two engines over one ladder agree on every face id; two ladders do not.
    pub(crate) ladder: Mutex<Option<FontLadder>>,
    /// The first failure any worker recorded: its code and message. `windows_core::Error` may
    /// carry a COM error object, so the two plain halves cross instead.
    failure: Mutex<Option<(HRESULT, String)>>,
}

impl Links {
    /// Creates the links, with every buffer allocated: one in each producer's hand and one in
    /// each spare.
    ///
    /// # Errors
    ///
    /// The window has no content window, or a doorbell's or a start-up event's kernel object
    /// could not be created.
    pub(crate) fn new(window: &Window) -> Result<Self> {
        Ok(Self {
            window: window.handle(),
            content: window
                .content()
                .ok_or_else(|| Error::new(E_HANDLE, "the window has no content window"))?,
            down: Link::new(),
            to_scene: Link::new(),
            up: Link::new(),
            input: Link::new(),
            scene_bell: Arc::new(Ring::new()?),
            app_bell: Arc::new(Ring::new()?),
            stop: AtomicBool::new(false),
            uia_listening: AtomicBool::new(false),
            uia_requested: AtomicBool::new(false),
            scene_ready: Event::auto_reset()?,
            first_patch: Event::auto_reset()?,
            ladder: Mutex::new(None),
            failure: Mutex::new(None),
        })
    }

    /// Records a worker's failure, keeping the first, and asks the window to close so the pump
    /// returns and the input thread joins the workers and reports it.
    pub(crate) fn fail(&self, e: &Error) {
        self.fail_with(e.code(), e.message());
    }

    /// As [`fail`](Self::fail), for a failure that is not a `windows_core::Error`: a panic.
    pub(crate) fn fail_with(&self, code: HRESULT, message: String) {
        if let Ok(mut held) = self.failure.lock() {
            held.get_or_insert((code, message));
        }
        // release: a worker that sees the flag must also see the failure behind it.
        self.stop.store(true, Release);
        self.window.close();
    }

    /// Returns the recorded failure as an error, if any worker failed.
    ///
    /// # Errors
    ///
    /// A worker thread recorded one.
    pub(crate) fn failure(&self) -> Result<()> {
        match self.failure.lock().ok().and_then(|held| held.clone()) {
            Some((code, message)) => Err(Error::new(code, message)),
            None => Ok(()),
        }
    }
}

/// Ends a worker thread's run: records a panic as a failure and closes the window.
///
/// Held on the worker's stack for the whole run, so a panic anywhere in it reaches the input
/// thread as a closed window and an error out of `run`, rather than as a window that keeps
/// pumping over a thread that is gone.
pub(crate) struct Guard<'a> {
    pub(crate) links: &'a Links,
    /// Events the thread's start-up promised to signal, whether or not it got that far.
    pub(crate) promised: &'a [&'a Event],
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.links
                .fail_with(E_HANDLE, "a worker thread panicked".into());
        }
        for event in self.promised {
            event.signal();
        }
    }
}
