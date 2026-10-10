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
//! the thread up: a watchdog tells the Director, and a command the thread
//! has not begun when it gets stuck is given up, never made later (G17,
//! J18). A press is never given up (J17): it waits in the queue, as a
//! receiver's does, and counts by its time once the write finishes.

use crate::error::{AppError, CmdResult};
use crate::pump::{Pump, Timed};
use crate::{execute_at, hardware, lock_show, now_ms, off_ui, play_press, Inner};
use buzzers::receiver::ReceiverEvent;
use buzzers::Press;
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
/// and the commands waiting for it are given up. A journal write takes
/// milliseconds; one to a disk that does not answer may never finish (G17).
pub const STUCK_AFTER: Duration = Duration::from_secs(3);
/// How often the watchdog, and a command waiting for the thread, look
/// whether it is stuck.
const WATCH_EVERY: Duration = Duration::from_millis(500);
/// Tells the Director that saving the show fails (an `AppError`) or works again (null).
pub const NOT_SAVED_EVENT: &str = "show-not-saved";

/// Something for the show thread to handle.
pub enum ShowEvent {
    Receiver(ReceiverEvent),
    /// A keyboard or on-screen buzzer's press, made at `at_ms` (`press`).
    Press { press: Press, at_ms: u64 },
    /// A quizmaster's command, given at `at_ms` (`run`).
    Job { at_ms: u64, work: Work<AppHandle> },
}

impl From<ReceiverEvent> for ShowEvent {
    fn from(event: ReceiverEvent) -> ShowEvent {
        ShowEvent::Receiver(event)
    }
}

impl Timed for ShowEvent {
    fn at_ms(&self) -> u64 {
        match self {
            ShowEvent::Receiver(ReceiverEvent::Press { at_ms, .. }) | ShowEvent::Press { at_ms, .. } | ShowEvent::Job { at_ms, .. } => *at_ms,
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
    let watch = Watch::new({
        let app = app.clone();
        move |problem| {
            let _ = app.emit(NOT_SAVED_EVENT, problem);
        }
    });
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
    std::thread::Builder::new()
        .name("show-watchdog".into())
        .spawn(move || loop {
            std::thread::sleep(WATCH_EVERY);
            watch.tell_if_stuck(Instant::now());
        })
        .expect("spawn show watchdog");
}

fn handle(app: &AppHandle, event: ShowEvent) {
    match event {
        ShowEvent::Receiver(event) => hardware::handle_event(app, event),
        ShowEvent::Press { press, at_ms } => {
            play_press(app, press, at_ms);
        }
        ShowEvent::Job { work, .. } => work(app),
    }
}

fn queue(app: &AppHandle, event: ShowEvent) -> CmdResult<()> {
    app.state::<ShowQueue>().0.send(event).map_err(|_| AppError::ShowStopped)
}

/// Queues a keyboard or on-screen buzzer's press, made now, to count as a
/// receiver's does: in order with them, never given up (J17).
pub fn press(app: &AppHandle, press: Press) -> CmdResult<()> {
    queue(app, ShowEvent::Press { press, at_ms: now_ms() })
}

/// Runs `job` on the show thread after everything queued before it, with
/// the show state and the moment it was given, read before it waits (S7),
/// and gives its result. A job the thread has not begun when it gets stuck
/// is given up, `ShowBusy`, and never runs (`Handed::wait`).
pub async fn run<T: Send + 'static>(
    app: &AppHandle,
    job: impl FnOnce(&AppHandle, &mut Inner, u64) -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    let at_ms = now_ms();
    let (work, handed) = hand_over(move |app: &AppHandle| job(app, &mut lock_show(app), at_ms));
    queue(app, ShowEvent::Job { at_ms, work })?;
    let watch = app.state::<Watch>().inner().clone();
    off_ui(move || match handed.wait(WATCH_EVERY, || watch.stuck(Instant::now())) {
        Outcome::Done(result) => result,
        Outcome::GivenUp => Err(AppError::ShowBusy),
        Outcome::Stopped => Err(AppError::ShowStopped),
    })
    .await
}

/// What the show thread runs for a job, with what it runs with (the app).
type Work<C> = Box<dyn FnOnce(&C) + Send>;

/// The caller's side of a job handed to the show thread (`hand_over`).
struct Handed<T> {
    answer: Receiver<T>,
    /// Taken by whichever comes first: the thread to run the job, or the caller to give it up.
    taken: Arc<AtomicBool>,
}

/// How a job handed to the show thread ended for its caller.
#[derive(Debug, PartialEq)]
enum Outcome<T> {
    Done(T),
    /// The thread got stuck before it began the job, which never runs.
    GivenUp,
    /// The thread is gone, or the job failed to finish.
    Stopped,
}

/// Splits `job` into the work the show thread runs and its caller's side.
/// The work runs the job only if the caller has not given it up, and the
/// caller gives it up only if the work has not begun it, so the job runs
/// once or never: the quizmaster may give it again, and it must not happen
/// twice (G17).
fn hand_over<C, T: Send + 'static>(job: impl FnOnce(&C) -> T + Send + 'static) -> (Work<C>, Handed<T>) {
    let (reply, answer) = mpsc::sync_channel(1);
    let taken = Arc::new(AtomicBool::new(false));
    let work: Work<C> = {
        let taken = taken.clone();
        Box::new(move |context: &C| {
            if !taken.swap(true, Ordering::AcqRel) {
                // The caller may have gone meanwhile; the show changed all the same.
                let _ = reply.send(job(context));
            }
        })
    };
    (work, Handed { answer, taken })
}

impl<T> Handed<T> {
    /// Waits for the job's result. Until the thread begins it, looks whether
    /// the thread is `stuck` each time `every` passes, and gives it up then.
    /// A thread busy with a burst of quick writes queued before it is waited
    /// for, however long they take together (J18); a job begun is waited
    /// for, as it may change the show.
    fn wait(self, every: Duration, stuck: impl Fn() -> bool) -> Outcome<T> {
        loop {
            match self.answer.recv_timeout(every) {
                Ok(done) => return Outcome::Done(done),
                Err(RecvTimeoutError::Disconnected) => return Outcome::Stopped,
                Err(RecvTimeoutError::Timeout) => {
                    if stuck() && !self.taken.swap(true, Ordering::AcqRel) {
                        return Outcome::GivenUp;
                    }
                }
            }
        }
    }
}

/// Why the running show is not saved now, as the Director was last told
/// (`NOT_SAVED_EVENT`). Read without the show state, which a write that
/// hangs holds.
pub fn not_saved(app: &AppHandle) -> Option<AppError> {
    app.state::<Watch>().watched.lock().unwrap().told.clone()
}

/// What the watchdog knows of the show thread, shared with it, and how the
/// Director is told (`NOT_SAVED_EVENT`). Its lock is taken under the show
/// state (`Ticker::tick`), never the other way round, and the Director is
/// told while it is held, so the changes reach it in the order `told`
/// takes them: telling only queues an event for the windows.
#[derive(Clone)]
struct Watch {
    watched: Arc<Mutex<Watched>>,
    notify: Arc<Notify>,
}

/// Tells the Director why the show is not saved now, or that it is again.
type Notify = dyn Fn(&Option<AppError>) + Send + Sync;

#[derive(Default)]
struct Watched {
    /// Since when the thread is on what it is doing, if anything.
    busy_since: Option<Instant>,
    /// What the Director was last told about saving the show (A9).
    told: Option<AppError>,
}

impl Watch {
    fn new(notify: impl Fn(&Option<AppError>) + Send + Sync + 'static) -> Watch {
        Watch { watched: Arc::default(), notify: Arc::new(notify) }
    }

    /// Runs `work` on the show thread, marked busy meanwhile.
    fn busy<T>(&self, work: impl FnOnce() -> T) -> T {
        self.watched.lock().unwrap().busy_since = Some(Instant::now());
        let done = work();
        self.watched.lock().unwrap().busy_since = None;
        done
    }

    /// Whether one thing has held the thread up for `STUCK_AFTER` by `now`.
    fn stuck(&self, now: Instant) -> bool {
        self.watched.lock().unwrap().stuck(now)
    }

    /// Tells the Director why the show is not saved now, if that changed.
    fn tell(&self, problem: Option<AppError>) {
        self.watched.lock().unwrap().tell(problem, &*self.notify);
    }

    /// Tells the Director that saving does not finish, while the thread is
    /// stuck by `now`. The thread tells it otherwise when it is free again
    /// (`Ticker::tick`).
    fn tell_if_stuck(&self, now: Instant) {
        let mut watched = self.watched.lock().unwrap();
        if watched.stuck(now) {
            watched.tell(Some(AppError::SaveHangs), &*self.notify);
        }
    }
}

impl Watched {
    fn stuck(&self, now: Instant) -> bool {
        self.busy_since.is_some_and(|since| now.saturating_duration_since(since) >= STUCK_AFTER)
    }

    fn tell(&mut self, problem: Option<AppError>, notify: &Notify) {
        if self.told != problem {
            notify(&problem);
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
        self.watch.tell(inner.save_problem.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// How often a test's caller looks whether the thread is stuck.
    const EVERY: Duration = Duration::from_millis(1);

    #[test]
    fn a_job_given_up_never_runs() {
        let (work, handed) = hand_over(|runs: &AtomicUsize| runs.fetch_add(1, Ordering::SeqCst));
        assert_eq!(handed.wait(EVERY, || true), Outcome::GivenUp);
        // The thread gets to it once the write that held it up finishes.
        let runs = AtomicUsize::new(0);
        work(&runs);
        assert_eq!(runs.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_job_behind_a_busy_thread_that_is_not_stuck_is_waited_for() {
        let (work, handed) = hand_over(|n: &u32| n + 1);
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            work(&41);
        });
        assert_eq!(handed.wait(EVERY, || false), Outcome::Done(42));
        thread.join().unwrap();
    }

    #[test]
    fn a_job_begun_is_waited_for_however_long_it_takes() {
        let (begun, has_begun) = mpsc::channel();
        let (work, handed) = hand_over(move |_: &()| {
            begun.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(30));
            "made"
        });
        let thread = std::thread::spawn(move || work(&()));
        has_begun.recv().unwrap();
        // The job is what holds the thread up: it may change the show, so it is not given up.
        assert_eq!(handed.wait(EVERY, || true), Outcome::Done("made"));
        thread.join().unwrap();
    }

    #[test]
    fn a_job_the_thread_drops_unrun_says_the_show_stopped() {
        let (work, handed) = hand_over(|_: &()| ());
        drop(work);
        assert_eq!(handed.wait(EVERY, || false), Outcome::Stopped);
    }

    /// A job in a test queue.
    struct Queued(Work<()>);

    impl Timed for Queued {
        fn at_ms(&self) -> u64 {
            0
        }
    }

    #[test]
    fn jobs_run_in_the_order_they_were_queued() {
        // As a step given just before `end_show`, which still counts in the show (G18).
        let (queue, events) = mpsc::channel();
        let order = Arc::new(Mutex::new(Vec::new()));
        let handed: Vec<Handed<()>> = ["step", "end"]
            .into_iter()
            .map(|name| {
                let order = order.clone();
                let (work, handed) = hand_over(move |_: &()| order.lock().unwrap().push(name));
                queue.send(Queued(work)).unwrap();
                handed
            })
            .collect();
        Pump::new(events).once(Duration::ZERO, || 0, |Queued(work)| work(&()), |_| {});
        assert_eq!(*order.lock().unwrap(), ["step", "end"]);
        for handed in handed {
            assert_eq!(handed.wait(EVERY, || false), Outcome::Done(()));
        }
    }

    #[test]
    fn the_director_is_told_while_one_thing_holds_the_thread_up_and_once_it_is_free() {
        let told = Arc::new(Mutex::new(Vec::new()));
        let watch = Watch::new({
            let told = told.clone();
            move |problem: &Option<AppError>| told.lock().unwrap().push(problem.clone())
        });
        watch.busy(|| {
            watch.tell_if_stuck(Instant::now() + STUCK_AFTER / 2);
            assert!(told.lock().unwrap().is_empty(), "not stuck yet");
            assert!(!watch.stuck(Instant::now() + STUCK_AFTER / 2));
            watch.tell_if_stuck(Instant::now() + STUCK_AFTER);
            watch.tell_if_stuck(Instant::now() + STUCK_AFTER * 2);
            assert!(watch.stuck(Instant::now() + STUCK_AFTER));
        });
        assert!(!watch.stuck(Instant::now() + STUCK_AFTER * 10), "free again");
        watch.tell_if_stuck(Instant::now() + STUCK_AFTER * 10);
        // The thread's next tick says what it knows of saving.
        watch.tell(None);
        assert_eq!(*told.lock().unwrap(), [Some(AppError::SaveHangs), None]);
        assert_eq!(watch.watched.lock().unwrap().told, None);
    }
}
