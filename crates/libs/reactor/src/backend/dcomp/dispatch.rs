//! The **app thread** of the render-thread split, and its dispatcher.
//!
//! The front thread (HWND + pump + input + compositor, `host.rs`) never runs
//! app logic. This module owns the other half: a spawned thread that holds
//! the [`RenderHost`] — reconciler, hooks, components, app state — and parks
//! on a condvar-driven job queue. The two halves share nothing but `Send`
//! data: the reconciler records into the [`RecordingBackend`]'s command
//! buffer, which each `post_render` ships front via
//! [`host::post_commit`](super::host::post_commit); input ships
//! [`Intent`](super::record::Intent)s back through [`deliver_intents`].
//!
//! The app thread has **no message pump and no WinRT apartment obligation**
//! (COM is initialized MTA for incidental free-threaded use — Direct2D
//! devices, WIC). It must never touch the Compositor, TSF, or any other
//! STA-affine object — those live front, reached only through the command
//! buffer and the front-serviced op queues (e.g. `super::pointer`).
//!
//! Two queues, mirroring the old pump-thread dispatcher exactly:
//! - a **`Send` queue** ([`AppQueue`]) any thread posts to — marshalled state
//!   writes (`use_async_state`, config listeners, the viz device-loss kick),
//!   intents, size/theme notifications from the front WndProc;
//! - a **local queue** ([`AppDispatcher`]) the render host schedules its own
//!   passes on (normal before low), touched only from the app thread itself.
//!
//! A drain runs cross-thread jobs first, then local jobs, then parks. Local
//! work can only appear while the thread is awake (nothing else can push to
//! it), so the park condition never misses a local wake.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use super::record::{Intent, RecordingBackend};
use crate::backend::{Dispatcher, SendDispatcher, UiMarshaller};
use crate::engine::RenderHost;
use crate::{Component, DispatcherQueuePriority, WindowSize};

/// One cross-thread job for the app thread.
type Job = Box<dyn FnOnce() + Send>;

/// The `Send` face of the app thread: a job queue plus the condvar its run
/// loop parks on. The front thread (and the marshaller, from any thread)
/// posts here; the app thread drains.
pub(crate) struct AppQueue {
    jobs: Mutex<VecDeque<Job>>,
    cv: Condvar,
    quit: AtomicBool,
}

impl AppQueue {
    fn new() -> Self {
        Self {
            jobs: Mutex::new(VecDeque::new()),
            cv: Condvar::new(),
            quit: AtomicBool::new(false),
        }
    }

    /// Queue `job` and wake the run loop. Callable from any thread; jobs run
    /// in post order. A job posted after [`post_quit`](Self::post_quit) is
    /// dropped unrun.
    pub(crate) fn post(&self, job: Job) {
        if let Ok(mut q) = self.jobs.lock() {
            q.push_back(job);
        }
        self.cv.notify_one();
    }

    /// Tell the run loop to exit. Jobs still queued are dropped — quit is the
    /// host tearing down, and every job is a notification with nowhere left
    /// to deliver.
    pub(crate) fn post_quit(&self) {
        self.quit.store(true, Ordering::Release);
        self.cv.notify_one();
    }
}

/// App-thread-local queue for the render host's own scheduling: normal-priority
/// work runs before low (the render loop re-enqueues its dirty-continuation at
/// low so queued input jobs interleave).
#[derive(Default)]
struct AppLocal {
    normal: RefCell<VecDeque<Box<dyn FnOnce()>>>,
    low: RefCell<VecDeque<Box<dyn FnOnce()>>>,
}

impl AppLocal {
    fn push(&self, priority: DispatcherQueuePriority, f: Box<dyn FnOnce()>) {
        match priority {
            DispatcherQueuePriority::Low => self.low.borrow_mut().push_back(f),
            _ => self.normal.borrow_mut().push_back(f),
        }
    }

    fn pop(&self) -> Option<Box<dyn FnOnce()>> {
        if let Some(f) = self.normal.borrow_mut().pop_front() {
            return Some(f);
        }
        self.low.borrow_mut().pop_front()
    }

    fn has_work(&self) -> bool {
        !self.normal.borrow().is_empty() || !self.low.borrow().is_empty()
    }
}

/// The [`Dispatcher`] handed to the app thread's `RenderHost`. Only the app
/// thread itself calls `enqueue` (render scheduling, state setters), so it is
/// a plain local push — the run loop drains it before parking, and local work
/// can only appear while the loop is awake.
pub(crate) struct AppDispatcher {
    local: Rc<AppLocal>,
}

impl Dispatcher for AppDispatcher {
    fn enqueue(&self, priority: DispatcherQueuePriority, f: Box<dyn FnOnce()>) -> bool {
        self.local.push(priority, f);
        true
    }
}

/// The [`SendDispatcher`] behind the app thread's [`UiMarshaller`]: marshalled
/// closures land on the app queue from any thread. This is what re-targets
/// `use_async_state`, config listeners and the viz device-loss kick at the app
/// thread — they re-render components, which is app-side work.
struct AppSend {
    queue: Arc<AppQueue>,
}

impl SendDispatcher for AppSend {
    fn enqueue_send(
        &self,
        _priority: DispatcherQueuePriority,
        f: Box<dyn FnOnce() + Send + 'static>,
    ) -> bool {
        self.queue.post(f);
        true
    }
}

/// Per-app-thread state reachable from posted jobs. Jobs cross as plain
/// `Send` closures, so anything needing the render host goes through this
/// thread-local rather than capturing it — the mirror of the front's `DCOMP`.
pub(crate) struct AppShared {
    pub(crate) render_host: RenderHost<RecordingBackend, AppDispatcher>,
}

thread_local! {
    static APP: RefCell<Option<Rc<AppShared>>> = const { RefCell::new(None) };
}

/// The app-thread shared state, or `None` off the app thread / after quit.
pub(crate) fn app_shared() -> Option<Rc<AppShared>> {
    APP.with(|c| c.borrow().clone())
}

/// Resolve front-queued intents against the recorder's app-side maps and run
/// the handler jobs. Runs on the app thread (posted by the front's
/// `run_intents`); jobs execute with no borrow held, so a handler that
/// re-enters render state finds nothing locked.
///
/// No job drives a frame tick any more. That existed for the surface sinks —
/// their pixels could not appear until the app had run, so the batch forced a
/// tick to shorten the wait. A gesture publishes its own visual front-side
/// before this is even posted, so there is nothing here left to hurry.
pub(crate) fn deliver_intents(intents: Vec<Intent>) {
    let Some(a) = app_shared() else { return };
    let jobs = a
        .render_host
        .with_reconciler_mut(|r| r.backend.resolve_intents(intents));
    for job in jobs {
        job.run();
    }
}

/// Spawn the app thread: build the `RenderHost` on it (the host is `!Send` —
/// it must be *born* there), install the marshaller **from that thread** (the
/// `UI_RERENDER` slot is thread-local and must land app-side), wire
/// `post_render` to ship each reconcile's command buffer to the front `hwnd`,
/// kick the first render, and enter the park loop. Returns the queue for the
/// front to post into and the join handle for shutdown.
pub(crate) fn spawn_app_thread(
    hwnd: isize,
    root: Box<dyn Component + Send>,
    size: WindowSize,
    dpi: u32,
) -> (Arc<AppQueue>, std::thread::JoinHandle<()>) {
    let queue = Arc::new(AppQueue::new());
    let q = Arc::clone(&queue);
    let join = std::thread::Builder::new()
        .name("reactor-app".into())
        .spawn(move || {
            // MTA, for incidental free-threaded COM (Direct2D devices, WIC).
            // Never an STA: this thread has no pump to service one.
            const COINIT_MULTITHREADED: u32 = 0;
            unsafe {
                let _ = crate::bindings::CoInitializeEx(
                    std::ptr::null(),
                    COINIT_MULTITHREADED,
                );
            }

            let local = Rc::new(AppLocal::default());
            let dispatcher = AppDispatcher {
                local: Rc::clone(&local),
            };
            let marshaller = UiMarshaller::new(Arc::new(AppSend {
                queue: Arc::clone(&q),
            }) as Arc<dyn SendDispatcher>);

            let root: Box<dyn Component> = root;
            let render_host = RenderHost::new(RecordingBackend::new(), root, dispatcher);
            // From this thread, so `UI_RERENDER` and the render-cx marshaller
            // install app-side (they capture the calling thread).
            render_host.set_marshaller(Some(marshaller));
            render_host.set_inner_size(size);
            render_host.set_dpi(dpi);

            // The commit edge: each reconcile's recorded commands ship to the
            // front thread, which replays them into the real backend, lays
            // out, paints, and returns any intents.
            let pr = render_host.clone_inner();
            render_host.set_post_render(move |root_id| {
                let cmds = pr.with_reconciler_mut(|r| r.backend.take_cmds());
                super::host::post_commit(hwnd, cmds, root_id);
            });

            // Size events queued by the front-side layout solve drain here.
            let q2 = Arc::clone(&q);
            super::size::set_delivery(Some(Arc::new(move || {
                q2.post(Box::new(super::size::deliver_pending));
            })));

            APP.with(|c| {
                *c.borrow_mut() = Some(Rc::new(AppShared {
                    render_host: render_host.clone_inner(),
                }));
            });
            render_host.kick();

            run_loop(&q, &local);

            APP.with(|c| *c.borrow_mut() = None);
        })
        .expect("spawn reactor app thread");
    (queue, join)
}

/// Drain cross-thread jobs, then local render work (normal before low), then
/// park until the next post. True idle: zero CPU while both queues are empty.
fn run_loop(queue: &AppQueue, local: &AppLocal) {
    loop {
        if queue.quit.load(Ordering::Acquire) {
            return;
        }
        loop {
            let job = match queue.jobs.lock() {
                Ok(mut q) => q.pop_front(),
                Err(_) => return,
            };
            match job {
                Some(job) => job(),
                None => break,
            }
        }
        while let Some(f) = local.pop() {
            f();
        }
        let Ok(mut guard) = queue.jobs.lock() else {
            return;
        };
        while guard.is_empty() && !local.has_work() {
            if queue.quit.load(Ordering::Acquire) {
                return;
            }
            guard = match queue.cv.wait(guard) {
                Ok(g) => g,
                Err(_) => return,
            };
        }
    }
}
