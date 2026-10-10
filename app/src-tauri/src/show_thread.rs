//! The show thread (A7): the one thread that changes a running show. The
//! receiver's events, the quizmaster's commands and the passing of time
//! reach it through one queue, so they are handled one at a time in the
//! order they came (`pump`), and a command waits for a journal write here,
//! never on the windows' thread. Starting and ending a show take the show
//! state from a worker thread instead: no running show has an order to keep.

use crate::error::{AppError, CmdResult};
use crate::pump::{Pump, Timed};
use crate::{execute_at, hardware, lock_show, now_ms, off_ui, Inner};
use buzzers::receiver::ReceiverEvent;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// The longest the thread waits for an event before it ticks.
const TICK: Duration = Duration::from_millis(50);
/// How often the windows get the running clock (seconds and points on screen).
const CLOCK_EVENT_EVERY: Duration = Duration::from_millis(250);
/// How long a due command that failed waits before it is tried again (A9).
const RETRY_DUE_AFTER: Duration = Duration::from_secs(1);
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
/// receiver thread sends to it too). It reads `Hardware` from the app, so it
/// starts once that is managed.
pub fn start(app: &AppHandle, queue: Sender<ShowEvent>, events: Receiver<ShowEvent>) {
    app.manage(ShowQueue(queue));
    let app = app.clone();
    std::thread::Builder::new()
        .name("show".into())
        .spawn(move || {
            let mut pump = Pump::new(events);
            let mut ticker = Ticker { last_clock_event: Instant::now(), retry_due_at: None, told: None };
            // The app keeps a sender as long as it runs.
            while pump.once(TICK, now_ms, |event| handle(&app, event), |now| ticker.tick(&app, now)) {}
        })
        .expect("spawn show thread");
}

fn handle(app: &AppHandle, event: ShowEvent) {
    match event {
        ShowEvent::Receiver(event) => hardware::handle_event(app, event),
        ShowEvent::Job { work, .. } => work(app),
    }
}

/// Runs `job` on the show thread after everything queued before it, with
/// the show state and the moment it was given, read before it waits (S7),
/// and gives its result.
pub async fn run<T: Send + 'static>(
    app: &AppHandle,
    job: impl FnOnce(&AppHandle, &mut Inner, u64) -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    let at_ms = now_ms();
    let (reply, answer) = mpsc::sync_channel(1);
    let work = Box::new(move |app: &AppHandle| {
        // The caller may have gone meanwhile; the show changed all the same.
        let _ = reply.send(job(app, &mut lock_show(app), at_ms));
    });
    app.state::<ShowQueue>().0.send(ShowEvent::Job { at_ms, work }).map_err(|_| AppError::ShowStopped)?;
    off_ui(move || answer.recv().map_err(|_| AppError::ShowStopped)?).await
}

/// The show thread's bookkeeping between rounds.
struct Ticker {
    last_clock_event: Instant,
    /// A due command that failed is tried again from then on, not 20 times a second (A9).
    retry_due_at: Option<Instant>,
    /// What the Director was last told about saving the show (A9).
    told: Option<AppError>,
}

impl Ticker {
    /// Issues a time-driven command due at `now`, such as "time is up", or
    /// else sends the running clock to the windows now and then. Then tells
    /// the Director when saving the show starts or stops failing.
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
        if inner.save_problem != self.told {
            self.told = inner.save_problem.clone();
            let _ = app.emit(NOT_SAVED_EVENT, &self.told);
        }
    }
}
