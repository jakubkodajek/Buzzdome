//! Desktop shell: owns the live game, journals every accepted command before
//! the windows see it, drives the buzzer lights and the projector window.

mod app_file;
mod editor;
mod error;
mod hardware;
mod media_player;
mod pump;
mod session;
mod teams;
mod themes;

use buzzers::receiver::HardwareDriver;
use buzzers::{BuzzerDriver, VirtualDriver};
use error::{AppError, CmdResult};
use hardware::{Hardware, ReceiverEvents};
use quiz_core::{Command, Effect, Game, GameView, Quiz, Stamped};
use serde::Serialize;
use ts_rs::TS;
use session::{Resumed, Session, SessionMeta, SessionSummary};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};

const DEMO_QUIZ: &str = include_str!("../../../samples/demo-quiz.json");
const HOST_LABEL: &str = "host";
const TICK: Duration = Duration::from_millis(50);
/// How often the windows get the running clock (seconds and points on screen).
const CLOCK_EVENT_EVERY: Duration = Duration::from_millis(250);

struct Inner {
    game: Option<Game>,
    session: Option<Session>,
    /// Folder with the running quiz's media files, served to the windows.
    media_root: Option<PathBuf>,
    driver: Box<dyn BuzzerDriver>,
    /// The team names prepared for the next show (`teams`).
    teams: Vec<String>,
}

struct AppState(Mutex<Inner>);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

pub(crate) fn app_data(app: &AppHandle, folder: &str) -> CmdResult<PathBuf> {
    Ok(app.path().app_data_dir()?.join(folder))
}

fn sessions_root(app: &AppHandle) -> CmdResult<PathBuf> {
    app_data(app, "sessions")
}

/// Unpacked quiz files (quiz-file packages).
fn quizzes_cache(app: &AppHandle) -> CmdResult<PathBuf> {
    app_data(app, "quizzes")
}

/// Lets the windows load files from `root` (and nothing else of the disk).
fn serve_media(app: &AppHandle, inner: &mut Inner, root: Option<PathBuf>) -> CmdResult<()> {
    if let Some(root) = &root {
        app.asset_protocol_scope().allow_directory(root, true)?;
    }
    inner.media_root = root;
    Ok(())
}

fn emit_state(app: &AppHandle, game: Option<&Game>) {
    let view = game.map(|g| g.view(now_ms()));
    let _ = app.emit("game-state", view);
}

/// Tells the windows that a different show (or none) is loaded, so they
/// reload the quiz.
fn emit_show_changed(app: &AppHandle, inner: &Inner) {
    let _ = app.emit("show-changed", ());
    // A show has its own team names, and the prepared ones are back after it.
    let _ = app.emit("teams-changed", teams::current(inner));
    emit_state(app, inner.game.as_ref());
}

/// Locks `mutex` and says when the caller asked for it, read from `clock`
/// before waiting: a command happened when it was given, not when it got the
/// show state, which a burst of journaled buzzer presses can hold for a
/// while (S7).
fn timed_lock<T>(mutex: &Mutex<T>, clock: impl FnOnce() -> u64) -> (MutexGuard<'_, T>, u64) {
    let at_ms = clock();
    (mutex.lock().unwrap(), at_ms)
}

/// Applies a command that happened at `at_ms` on a copy, journals it, then
/// commits. The show never shows a state that is not on disk.
fn execute_at(app: &AppHandle, inner: &mut Inner, command: Command, at_ms: u64) -> CmdResult<()> {
    let game = inner.game.as_ref().ok_or(AppError::NoShow)?;
    let stamped = Stamped { at_ms, command };
    let mut next = game.clone();
    let effects = next.apply(&stamped)?;
    if let Some(session) = inner.session.as_mut() {
        session.append(&stamped).map_err(|e| AppError::ShowNotSaved { detail: e.to_string() })?;
    }
    inner.game = Some(next);
    for effect in effects {
        match effect {
            Effect::Lights { lights } => {
                if let Err(e) = inner.driver.set_lights(&lights) {
                    eprintln!("buzzer lights: {e}");
                }
            }
            Effect::Sound { cue } => {
                let _ = app.emit("sound-cue", cue);
            }
        }
    }
    emit_state(app, inner.game.as_ref());
    Ok(())
}

/// Locks the show state to start a show, refusing while one runs. Learn
/// mode ends first, because in a show every press must play; it is ended
/// before the lock is taken, as no code may wait for the learn lock while
/// holding this one.
fn lock_for_new_show<'a>(app: &AppHandle, state: &'a AppState) -> CmdResult<MutexGuard<'a, Inner>> {
    hardware::end_learning(app);
    let inner = state.0.lock().unwrap();
    if inner.game.is_some() {
        return Err(AppError::ShowRunning);
    }
    Ok(inner)
}

/// Starts a show of `quiz`, whose media (if any) are in `media_root`.
fn start_show(
    app: &AppHandle,
    inner: &mut Inner,
    quiz: Quiz,
    source_path: Option<String>,
    media_root: Option<PathBuf>,
) -> CmdResult<()> {
    quiz_file::check(&quiz, media_root.as_deref().unwrap_or(Path::new("")))?;
    let meta = SessionMeta {
        quiz_title: quiz.title.clone(),
        source_path,
        started_ms: now_ms(),
        media_root: media_root.as_ref().map(|root| root.to_string_lossy().into_owned()),
    };
    let session = Session::create(&sessions_root(app)?, &quiz, &meta).map_err(|e| AppError::ShowNotSaved { detail: e.to_string() })?;
    serve_media(app, inner, media_root)?;
    begin_show(app, inner, Game::new(quiz), Some(session))?;
    // Only the latest show can be resumed, so other unpacked quizzes can go.
    let keep: Vec<&Path> = inner.media_root.iter().map(PathBuf::as_path).collect();
    if let Err(e) = quiz_file::prune(&quizzes_cache(app)?, &keep) {
        eprintln!("cleaning unpacked quizzes: {e}");
    }
    Ok(())
}

/// Makes `game` the running show, journaled in `session` (none in a test
/// run), and gives it the prepared team names as its first command.
fn begin_show(app: &AppHandle, inner: &mut Inner, game: Game, session: Option<Session>) -> CmdResult<()> {
    inner.game = Some(game);
    inner.session = session;
    let teams = (!inner.teams.is_empty()).then(|| Command::SetTeams { names: inner.teams.clone() });
    // The show runs even if its journal refuses the names; the Director says why.
    let named = teams.map_or(Ok(()), |command| execute_at(app, inner, command, now_ms()));
    emit_show_changed(app, inner);
    named
}

#[tauri::command]
fn get_view(state: State<AppState>) -> Option<GameView> {
    let inner = state.0.lock().unwrap();
    inner.game.as_ref().map(|g| g.view(now_ms()))
}

/// The whole quiz, for the Director's slide list and previews.
#[tauri::command]
fn get_quiz(state: State<AppState>) -> Option<Quiz> {
    let inner = state.0.lock().unwrap();
    inner.game.as_ref().map(|g| g.quiz().clone())
}

/// Folder of the running quiz's media, ending with a path separator, so the
/// windows can append a media reference to it.
#[tauri::command]
fn media_root(state: State<AppState>) -> Option<String> {
    let inner = state.0.lock().unwrap();
    inner.media_root.as_ref().map(|root| root.join("").to_string_lossy().into_owned())
}

/// Opens a quiz file (`.quiz` or a plain quiz JSON) and starts a show with it.
#[tauri::command]
async fn start_show_from_file(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<()> {
    // Unpacking big media takes a moment: it runs off the UI thread and
    // before the show is locked.
    let package = quiz_file::open(Path::new(&path), &quizzes_cache(&app)?)?;
    start_show(&app, &mut *lock_for_new_show(&app, &state)?, package.quiz, Some(path), Some(package.root))
}

/// Plays the editor's quiz without a journal, so the last real show stays
/// the one that can be resumed.
#[tauri::command]
fn editor_test_run(app: AppHandle, state: State<AppState>, quiz: Quiz) -> CmdResult<()> {
    let root = editor::draft_root(&app)?;
    quiz_file::check(&quiz, &root)?;
    let mut inner = lock_for_new_show(&app, &state)?;
    serve_media(&app, &mut inner, Some(root))?;
    begin_show(&app, &mut inner, Game::new(quiz), None)
}

#[tauri::command]
fn start_demo_show(app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    let quiz = Quiz::from_json(DEMO_QUIZ)?;
    start_show(&app, &mut *lock_for_new_show(&app, &state)?, quiz, None, None)
}

#[tauri::command]
fn last_session(app: AppHandle) -> CmdResult<Option<SessionSummary>> {
    Session::latest_summary(&sessions_root(&app)?).map_err(|e| AppError::ShowNotRead { detail: e.to_string() })
}

/// Resumes the last show. Returns a warning for the quizmaster when the show
/// runs but something is missing (its media were deleted).
#[tauri::command]
fn resume_last_session(app: AppHandle, state: State<AppState>) -> CmdResult<Option<AppError>> {
    // Locked first: the running show's journal must not be reopened.
    let mut inner = lock_for_new_show(&app, &state)?;
    let Resumed { session, quiz, journal, meta } = Session::resume_latest(&sessions_root(&app)?)
        .map_err(|e| AppError::ShowNotRead { detail: e.to_string() })?
        .ok_or(AppError::NoShowToResume)?;
    let media_root = meta.media_root.map(PathBuf::from);
    // The show must go on: without its media it still runs, and the Director says why.
    let warning = match &media_root {
        Some(root) => quiz_file::check(&quiz, root).err().map(AppError::from),
        None => None,
    };
    serve_media(&app, &mut inner, media_root.filter(|root| root.is_dir()))?;
    let game = Game::replay(quiz, &journal);
    let recovery = game.recovery_command();
    inner.game = Some(game);
    inner.session = Some(session);
    emit_show_changed(&app, &inner);
    if let Some(command) = recovery {
        execute_at(&app, &mut inner, command, now_ms())?;
    }
    Ok(warning)
}

/// Closes the running show. Its journal stays on disk, so it can be resumed.
#[tauri::command]
fn end_show(app: AppHandle, state: State<AppState>) {
    let mut inner = state.0.lock().unwrap();
    inner.game = None;
    inner.session = None;
    inner.media_root = None;
    emit_show_changed(&app, &inner);
}

#[tauri::command]
fn dispatch(app: AppHandle, state: State<AppState>, command: Command) -> CmdResult<()> {
    let (mut inner, at_ms) = timed_lock(&state.0, now_ms);
    execute_at(&app, &mut inner, command, at_ms)
}

/// Feeds a press the receiver got at `at_ms` into the running show, if any.
/// Returns whether the show took it.
pub(crate) fn press_from_hardware(app: &AppHandle, command: Command, at_ms: u64) -> bool {
    let state = app.state::<AppState>();
    let mut inner = state.0.lock().unwrap();
    if inner.game.is_none() {
        return false;
    }
    match execute_at(app, &mut inner, command, at_ms) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("buzzer press: {e}");
            false
        }
    }
}

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
struct MonitorInfo {
    index: usize,
    /// None when the system gives no name; the UI numbers the monitor then.
    name: Option<String>,
    width: u32,
    height: u32,
}

// The window commands are async: Tauri runs them off the main thread, which
// creating a window from a synchronous command would deadlock on Windows
// (WebView2; see the `WebviewWindowBuilder::new` docs).

#[tauri::command]
async fn list_monitors(app: AppHandle) -> CmdResult<Vec<MonitorInfo>> {
    let monitors = app.available_monitors()?;
    Ok(monitors
        .iter()
        .enumerate()
        .map(|(index, m)| MonitorInfo {
            index,
            name: m.name().cloned(),
            width: m.size().width,
            height: m.size().height,
        })
        .collect())
}

/// Opens (or moves) the projector window, full screen on the chosen monitor.
#[tauri::command]
async fn open_host_window(app: AppHandle, monitor: Option<usize>, fullscreen: bool) -> CmdResult<()> {
    let window = match app.get_webview_window(HOST_LABEL) {
        Some(window) => window,
        None => WebviewWindowBuilder::new(&app, HOST_LABEL, WebviewUrl::App("index.html".into()))
            .title("Quiz Show")
            .inner_size(1280.0, 720.0)
            .build()?,
    };
    window.set_fullscreen(false)?;
    if let Some(index) = monitor {
        let monitors = window.available_monitors()?;
        let target = monitors.get(index).ok_or(AppError::NoSuchMonitor { index })?;
        window.set_position(*target.position())?;
    }
    window.set_fullscreen(fullscreen)?;
    window.set_focus()?;
    let _ = app.emit("host-window", true);
    Ok(())
}

#[tauri::command]
fn host_window_open(app: AppHandle) -> bool {
    app.get_webview_window(HOST_LABEL).is_some()
}

#[tauri::command]
async fn close_host_window(app: AppHandle) -> CmdResult<()> {
    match app.get_webview_window(HOST_LABEL) {
        Some(window) => Ok(window.close()?),
        None => Ok(()),
    }
}

/// The one thread that turns the receiver's events and the passing of time
/// into show commands: every event received so far is handled before a
/// time-driven command such as "time is up" (`pump::pump_once`). Without a
/// receiver it only keeps time.
fn spawn_show_pump(app: AppHandle, mut events: Option<ReceiverEvents>) {
    std::thread::Builder::new()
        .name("show-pump".into())
        .spawn(move || {
            let mut last_clock_event = Instant::now();
            loop {
                let connected = match &events {
                    Some(rx) => pump::pump_once(rx, TICK, |event| hardware::handle_event(&app, event), || tick(&app, &mut last_clock_event)),
                    None => {
                        std::thread::sleep(TICK);
                        tick(&app, &mut last_clock_event);
                        true
                    }
                };
                if !connected {
                    events = None;
                }
            }
        })
        .expect("spawn show pump");
}

/// Issues a time-driven command that is due, such as "time is up", or else
/// sends the running clock to the windows now and then.
fn tick(app: &AppHandle, last_clock_event: &mut Instant) {
    let state = app.state::<AppState>();
    let mut inner = state.0.lock().unwrap();
    let now = now_ms();
    if let Some(command) = inner.game.as_ref().and_then(|g| g.due_command(now)) {
        if let Err(e) = execute_at(app, &mut inner, command, now) {
            eprintln!("ticker: {e}");
        }
    } else if last_clock_event.elapsed() >= CLOCK_EVENT_EVERY {
        *last_clock_event = Instant::now();
        let clock = inner.game.as_ref().and_then(|g| g.clock_view(now)).filter(|c| c.running);
        if let Some(clock) = clock {
            let _ = app.emit("clock-tick", clock);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // However the projector closes (also from its title bar), the Director hears of it.
        .on_window_event(|window, event| {
            if window.label() == HOST_LABEL && matches!(event, WindowEvent::Destroyed) {
                let _ = window.app_handle().emit("host-window", false);
            }
        })
        .manage(editor::EditorLock::default())
        .manage(AppState(Mutex::new(Inner {
            game: None,
            session: None,
            media_root: None,
            driver: Box::new(VirtualDriver),
            teams: Vec::new(),
        })))
        .setup(|app| {
            let (hardware, events) = Hardware::start(app.handle());
            if let Some(handle) = hardware.handle() {
                app.state::<AppState>().0.lock().unwrap().driver = Box::new(HardwareDriver(handle));
            }
            // The event pump reads `Hardware` from the app, so it starts after `manage`.
            app.manage(hardware);
            spawn_show_pump(app.handle().clone(), events);
            // The windows load theme pictures and fonts from the built-in packages.
            let packages = app.path().resource_dir()?.join(themes::RESOURCE_FOLDER);
            app.asset_protocol_scope().allow_directory(&packages, true)?;
            let builtin = themes::builtin_themes(|id| packages.join(id).join("").to_string_lossy().into_owned())?;
            app.manage(themes::Themes(builtin));
            app.state::<AppState>().0.lock().unwrap().teams = teams::teams_path(app.handle()).map(|path| teams::load(&path)).unwrap_or_default();
            let library = media_player::MediaState::open(app_data(app.handle(), media_player::LIBRARY_FOLDER)?);
            app.asset_protocol_scope().allow_directory(library.root(), true)?;
            app.manage(library);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_view,
            get_quiz,
            media_root,
            start_show_from_file,
            start_demo_show,
            last_session,
            resume_last_session,
            end_show,
            dispatch,
            list_monitors,
            open_host_window,
            close_host_window,
            host_window_open,
            themes::list_themes,
            teams::prepared_teams,
            teams::add_teams,
            teams::remove_team,
            media_player::media_library,
            media_player::media_add,
            media_player::media_add_dropped,
            media_player::media_remove,
            media_player::sound_board,
            media_player::sound_board_change,
            media_player::media_now,
            media_player::media_control,
            editor::editor_load,
            editor::editor_new,
            editor::editor_open,
            editor::editor_update,
            editor::editor_save,
            editor::editor_add_media,
            editor::editor_close,
            editor_test_run,
            hardware::receiver_info,
            hardware::receiver_devices,
            hardware::receiver_select,
            hardware::receiver_learn_start,
            hardware::receiver_learn_cancel,
            hardware::receiver_learn_finish,
            hardware::receiver_forget_learned,
            hardware::receiver_export_log,
            hardware::receiver_udev_rule,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc};

    #[test]
    fn a_command_is_timed_before_it_waits_for_the_show_state() {
        let shared = Arc::new(Mutex::new(()));
        let held = shared.lock().unwrap();
        let (timed, was_timed) = mpsc::channel();
        let waiter = {
            let shared = Arc::clone(&shared);
            std::thread::spawn(move || {
                // The "time" read is whether someone else still held the state then.
                let (_inner, held_then) = timed_lock(&shared, || {
                    let held_then = u64::from(shared.try_lock().is_err());
                    timed.send(()).unwrap();
                    held_then
                });
                held_then
            })
        };
        // The state is let go only once the time is read, so the answer does not depend on timing;
        // a clock read after locking would never come while it is held.
        let read_while_held = was_timed.recv_timeout(Duration::from_secs(5));
        drop(held);
        assert!(read_while_held.is_ok(), "the time was read only after waiting for the state");
        assert_eq!(waiter.join().unwrap(), 1);
    }
}
