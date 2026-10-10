//! The show thread (A7): the one thread that changes a running show. The
//! receiver's events, the quizmaster's commands and the passing of time
//! reach it through one queue, so they are handled one at a time in the
//! order they came (`pump`), and a command waits for a journal write here,
//! never on the windows' thread. Ending a show is one of those commands, so
//! one given before it still counts in the show (G18). Starting a show
//! takes the show state from a worker thread instead: it waits for files,
//! and no show runs whose order it could break.
//!
//! A journal write that does not finish (the disk does not answer) holds
//! the thread up: a watchdog tells the Director, and a command that waits
//! too long is given up, never made later (G17).

use crate::error::{AppError, CmdResult};
use crate::pump::{Pump, Timed};
use crate::{execute_at, hardware, lock_show, now_ms, off_ui, Inner};
use buzzers::receiver::ReceiverEvent;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// The longest the thread waits for an event before it ticks.
const TICK: Duration = Duration::from_millis(50);
/// How often the windows get the running clock (seconds and points on screen).
const CLOCK_EVENT_EVERY: Duration = Duration::from_millis(250);
/// How long a due command that failed waits before it is tried again (A9).
const RETRY_DUE_AFTER: Duration = Duration::from_secs(1);
/// How long the thread may take over one thing before it counts as stuck,
/// and a command waits for it before it is given up. A journal write takes
/// milliseconds; one to a disk that does not answer may never finish (G17).
pub const STUCK_AFTER: Duration = Duration::from_secs(3);
/// How often the watchdog looks whether the thread is stuck.
const WATCH_EVERY: Duration = Duration::from_millis(500);
/// Tells the Director that saving the show fails (an `AppError`) or works again (null).
pub const NOT_SAVED_EVENT: &str = "show-not-saved";

/// Something for the show thread to handle.
pub enum ShowEvent {
    Receiver(ReceiverEvent),
    /// A quizmaster's command, given at `at_ms` (`run`).
    Job { at_ms: u64, work: Box<dyn FnOnce(&AppHandle) + Send> },
}

impl From<ReceiverEvent> for ShowEvent {
    fn from(event: ReceiverEvent) -> ShowEvent {
        ShowEvent::Receiver(event)
    }
}

impl Timed for ShowEvent {
    fn at_ms(&self) -> u64 {
        match self {
            ShowEvent::Receiver(ReceiverEvent::Press { at_ms, .. }) | ShowEvent::Job { at_ms, .. } => *at_ms,
            ShowEvent::Receiver(ReceiverEvent::Raw(report)) => report.at_ms,
            ShowEvent::Receiver(ReceiverEvent::Status(_)) => 0,
        }
    }
}

/// The show thread's queue, for commands to join.
struct ShowQueue(Sender<ShowEvent>);

/// Starts the show thread on `events`, the queue that `queue` sends to (the
/// receiver thread sends to it too), and its watchdog. It reads `Hardware`
/// from the app, so it starts once that is managed.
pub fn start(app: &AppHandle, queue: Sender<ShowEvent>, events: Receiver<ShowEvent>) {
    app.manage(ShowQueue(queue));
    let watch = Watch::default();
    app.manage(watch.clone());
    let (show_app, show_watch) = (app.clone(), watch.clone());
    std::thread::Builder::new()
        .name("show".into())
        .spawn(move || {
            let (app, watch) = (show_app, show_watch);
            let mut pump = Pump::new(events);
            let mut ticker = Ticker { last_clock_event: Instant::now(), retry_due_at: None, watch: watch.clone() };
            // The app keeps a sender as long as it runs.
            while pump.once(TICK, now_ms, |event| watch.busy(|| handle(&app, event)), |now| watch.busy(|| ticker.tick(&app, now))) {}
        })
        .expect("spawn show thread");
    let app = app.clone();
    std::thread::Builder::new()
        .name("show-watchdog".into())
        .spawn(move || loop {
            std::thread::sleep(WATCH_EVERY);
            watch.tell_if_stuck(&app);
        })
        .expect("spawn show watchdog");
}

fn handle(app: &AppHandle, event: ShowEvent) {
    match event {
        ShowEvent::Receiver(event) => hardware::handle_event(app, event),
        ShowEvent::Job { work, .. } => work(app),
    }
}

/// Runs `job` on the show thread after everything queued before it, with
/// the show state and the moment it was given, read before it waits (S7),
/// and gives its result. A job the thread has not begun within
/// `STUCK_AFTER` is given up, `ShowBusy`, and never runs: the quizmaster
/// may give it again, and it must not happen twice. One that has begun is
/// waited for, as it may change the show (G17).
pub async fn run<T: Send + 'static>(
    app: &AppHandle,
    job: impl FnOnce(&AppHandle, &mut Inner, u64) -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    let at_ms = now_ms();
    let (reply, answer) = mpsc::sync_channel(1);
    // Taken by whichever comes first: the thread to run the job, or the caller to give it up.
    let taken = Arc::new(AtomicBool::new(false));
    let work = {
        let taken = taken.clone();
        Box::new(move |app: &AppHandle| {
            if !taken.swap(true, Ordering::AcqRel) {
                // The caller may have gone meanwhile; the show changed all the same.
                let _ = reply.send(job(app, &mut lock_show(app), at_ms));
            }
        })
    };
    app.state::<ShowQueue>().0.send(ShowEvent::Job { at_ms, work }).map_err(|_| AppError::ShowStopped)?;
    off_ui(move || {
        let result = match answer.recv_timeout(STUCK_AFTER) {
            Err(RecvTimeoutError::Timeout) if !taken.swap(true, Ordering::AcqRel) => return Err(AppError::ShowBusy),
            Err(RecvTimeoutError::Timeout) => answer.recv().ok(),
            done => done.ok(),
        };
        result.ok_or(AppError::ShowStopped)?
    })
    .await
}

/// Why the running show is not saved now, as the Director was last told
/// (`NOT_SAVED_EVENT`). Read without the show state, which a write that
/// hangs holds.
pub fn not_saved(app: &AppHandle) -> Option<AppError> {
    app.state::<Watch>().0.lock().unwrap().told.clone()
}

/// What the watchdog knows of the show thread, shared with it.
#[derive(Clone, Default)]
struct Watch(Arc<Mutex<Watched>>);

#[derive(Default)]
struct Watched {
    /// Since when the thread is on what it is doing, if anything.
    busy_since: Option<Instant>,
    /// What the Director was last told about saving the show (A9).
    told: Option<AppError>,
}

impl Watch {
    /// Runs `work` on the show thread, marked busy meanwhile.
    fn busy<T>(&self, work: impl FnOnce() -> T) -> T {
        self.0.lock().unwrap().busy_since = Some(Instant::now());
        let done = work();
        self.0.lock().unwrap().busy_since = None;
        done
    }

    /// Tells the Director why the show is not saved now, if that changed.
    fn tell(&self, app: &AppHandle, problem: Option<AppError>) {
        self.0.lock().unwrap().tell(app, problem);
    }

    /// Tells the Director that saving does not finish, while the thread has
    /// been busy for `STUCK_AFTER`. The thread tells it otherwise when it is
    /// free again (`Ticker::tick`).
    fn tell_if_stuck(&self, app: &AppHandle) {
        let mut watched = self.0.lock().unwrap();
        if watched.busy_since.is_some_and(|since| since.elapsed() >= STUCK_AFTER) {
            watched.tell(app, Some(AppError::SaveHangs));
        }
    }
}

impl Watched {
    fn tell(&mut self, app: &AppHandle, problem: Option<AppError>) {
        if self.told != problem {
            let _ = app.emit(NOT_SAVED_EVENT, &problem);
            self.told = problem;
        }
    }
}

/// The show thread's bookkeeping between rounds.
struct Ticker {
    last_clock_event: Instant,
    /// A due command that failed is tried again from then on, not 20 times a second (A9).
    retry_due_at: Option<Instant>,
    watch: Watch,
}

impl Ticker {
    /// Issues a time-driven command due at `now`, such as "time is up", or
    /// else sends the running clock to the windows now and then. Then tells
    /// the Director when saving the show starts or stops failing, or no
    /// longer hangs (`Watch::tell_if_stuck`).
    fn tick(&mut self, app: &AppHandle, now: u64) {
        let mut inner = lock_show(app);
        match inner.game.as_ref().and_then(|g| g.due_command(now)) {
            Some(command) => {
                if self.retry_due_at.is_none_or(|at| Instant::now() >= at) {
                    // Why it failed is the Director's to show (`save_problem`).
                    let failed = execute_at(app, &mut inner, command, now).is_err();
                    self.retry_due_at = failed.then(|| Instant::now() + RETRY_DUE_AFTER);
                }
            }
            None => {
                self.retry_due_at = None;
                if self.last_clock_event.elapsed() >= CLOCK_EVENT_EVERY {
                    self.last_clock_event = Instant::now();
                    // As the windows' views are (`emit_state`), at the time
                    // it is, not the round's: a burst may have taken a while.
                    let clock = inner.game.as_ref().and_then(|g| g.clock_view(now_ms())).filter(|c| c.running);
                    if let Some(clock) = clock {
                        let _ = app.emit("clock-tick", clock);
                    }
                }
            }
        }
        self.watch.tell(app, inner.save_problem.clone());
    }
}
