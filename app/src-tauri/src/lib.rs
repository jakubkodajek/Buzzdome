//! Desktop shell: owns the live game, journals every accepted command before
//! the windows see it, drives the buzzer lights and the projector window.

mod hardware;
mod session;

use buzzers::receiver::HardwareDriver;
use buzzers::{BuzzerDriver, VirtualDriver};
use hardware::Hardware;
use quiz_core::{Command, Effect, Game, GameView, Quiz, Stamped};
use serde::Serialize;
use ts_rs::TS;
use session::{Resumed, Session, SessionMeta, SessionSummary};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

const DEMO_QUIZ: &str = include_str!("../../../samples/demo-quiz.json");
const HOST_LABEL: &str = "host";
const DIRECTOR_LABEL: &str = "director";
const TICK: Duration = Duration::from_millis(50);
/// How often the windows get the running clock (seconds and points on screen).
const CLOCK_EVENT_EVERY: Duration = Duration::from_millis(250);

struct Inner {
    game: Option<Game>,
    session: Option<Session>,
    /// Folder with the running quiz's media files, served to the windows.
    media_root: Option<PathBuf>,
    driver: Box<dyn BuzzerDriver>,
}

struct AppState(Mutex<Inner>);

type CmdResult<T> = Result<T, String>;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn app_data(app: &AppHandle, folder: &str) -> CmdResult<PathBuf> {
    app.path().app_data_dir().map(|dir| dir.join(folder)).map_err(|e| e.to_string())
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
        app.asset_protocol_scope().allow_directory(root, true).map_err(|e| e.to_string())?;
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
    emit_state(app, inner.game.as_ref());
}

/// Applies a command on a copy, journals it, then commits. The show never
/// shows a state that is not on disk.
fn execute(app: &AppHandle, inner: &mut Inner, command: Command) -> CmdResult<()> {
    let game = inner.game.as_ref().ok_or("no show is running")?;
    let stamped = Stamped { at_ms: now_ms(), command };
    let mut next = game.clone();
    let effects = next.apply(&stamped).map_err(|e| e.to_string())?;
    if let Some(session) = inner.session.as_mut() {
        session.append(&stamped).map_err(|e| format!("could not save the show: {e}"))?;
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

/// Starts a show of `quiz`, whose media (if any) are in `media_root`.
fn start_show(
    app: &AppHandle,
    inner: &mut Inner,
    quiz: Quiz,
    source_path: Option<String>,
    media_root: Option<PathBuf>,
) -> CmdResult<()> {
    quiz_file::check(&quiz, media_root.as_deref().unwrap_or(Path::new(""))).map_err(|e| e.to_string())?;
    let meta = SessionMeta {
        quiz_title: quiz.title.clone(),
        source_path,
        started_ms: now_ms(),
        media_root: media_root.as_ref().map(|root| root.to_string_lossy().into_owned()),
    };
    let session = Session::create(&sessions_root(app)?, &quiz, &meta).map_err(|e| e.to_string())?;
    serve_media(app, inner, media_root)?;
    inner.game = Some(Game::new(quiz));
    inner.session = Some(session);
    emit_show_changed(app, inner);
    // Only the latest show can be resumed, so other unpacked quizzes can go.
    let keep: Vec<&Path> = inner.media_root.iter().map(PathBuf::as_path).collect();
    if let Err(e) = quiz_file::prune(&quizzes_cache(app)?, &keep) {
        eprintln!("cleaning unpacked quizzes: {e}");
    }
    Ok(())
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
fn start_show_from_file(app: AppHandle, state: State<AppState>, path: String) -> CmdResult<()> {
    // Unpacking can take a moment for big media; the show is not locked meanwhile.
    let package = quiz_file::open(Path::new(&path), &quizzes_cache(&app)?).map_err(|e| e.to_string())?;
    start_show(&app, &mut state.0.lock().unwrap(), package.quiz, Some(path), Some(package.root))
}

#[tauri::command]
fn start_demo_show(app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    let quiz = Quiz::from_json(DEMO_QUIZ)?;
    start_show(&app, &mut state.0.lock().unwrap(), quiz, None, None)
}

#[tauri::command]
fn last_session(app: AppHandle) -> CmdResult<Option<SessionSummary>> {
    Session::latest_summary(&sessions_root(&app)?).map_err(|e| e.to_string())
}

/// Resumes the last show. Returns a warning for the quizmaster when the show
/// runs but something is missing (its media were deleted).
#[tauri::command]
fn resume_last_session(app: AppHandle, state: State<AppState>) -> CmdResult<Option<String>> {
    let Resumed { session, quiz, journal, meta } = Session::resume_latest(&sessions_root(&app)?)
        .map_err(|e| e.to_string())?
        .ok_or("there is no show to resume")?;
    let media_root = meta.media_root.map(PathBuf::from);
    // The show must go on: without its media it still runs, and the Director says why.
    let warning = match &media_root {
        Some(root) => quiz_file::check(&quiz, root).err().map(|e| e.to_string()),
        None => None,
    };
    let mut inner = state.0.lock().unwrap();
    serve_media(&app, &mut inner, media_root.filter(|root| root.is_dir()))?;
    let game = Game::replay(quiz, &journal);
    let recovery = game.recovery_command();
    inner.game = Some(game);
    inner.session = Some(session);
    emit_show_changed(&app, &inner);
    if let Some(command) = recovery {
        execute(&app, &mut inner, command)?;
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
    execute(&app, &mut state.0.lock().unwrap(), command)
}

/// Feeds a press from the buzzer receiver into the running show, if any.
/// Returns whether the show took it.
pub(crate) fn press_from_hardware(app: &AppHandle, command: Command) -> bool {
    let state = app.state::<AppState>();
    let mut inner = state.0.lock().unwrap();
    if inner.game.is_none() {
        return false;
    }
    match execute(app, &mut inner, command) {
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
    name: String,
    width: u32,
    height: u32,
}

#[tauri::command]
fn list_monitors(app: AppHandle) -> CmdResult<Vec<MonitorInfo>> {
    let window = app.get_webview_window(DIRECTOR_LABEL).ok_or("director window missing")?;
    let monitors = window.available_monitors().map_err(|e| e.to_string())?;
    Ok(monitors
        .iter()
        .enumerate()
        .map(|(index, m)| MonitorInfo {
            index,
            name: m.name().cloned().unwrap_or_else(|| format!("Display {}", index + 1)),
            width: m.size().width,
            height: m.size().height,
        })
        .collect())
}

/// Opens (or moves) the projector window, full screen on the chosen monitor.
#[tauri::command]
fn open_host_window(app: AppHandle, monitor: Option<usize>, fullscreen: bool) -> CmdResult<()> {
    let window = match app.get_webview_window(HOST_LABEL) {
        Some(window) => window,
        None => WebviewWindowBuilder::new(&app, HOST_LABEL, WebviewUrl::App("index.html".into()))
            .title("Quiz Show")
            .inner_size(1280.0, 720.0)
            .build()
            .map_err(|e| e.to_string())?,
    };
    window.set_fullscreen(false).map_err(|e| e.to_string())?;
    if let Some(index) = monitor {
        let monitors = window.available_monitors().map_err(|e| e.to_string())?;
        let target = monitors.get(index).ok_or("monitor not found")?;
        window.set_position(*target.position()).map_err(|e| e.to_string())?;
    }
    window.set_fullscreen(fullscreen).map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
fn host_window_open(app: AppHandle) -> bool {
    app.get_webview_window(HOST_LABEL).is_some()
}

#[tauri::command]
fn close_host_window(app: AppHandle) -> CmdResult<()> {
    match app.get_webview_window(HOST_LABEL) {
        Some(window) => window.close().map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

/// Issues time-driven commands such as "time is up", and sends the running
/// clock to the windows.
fn spawn_ticker(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_clock_event = Instant::now();
        loop {
            std::thread::sleep(TICK);
            let state = app.state::<AppState>();
            let mut inner = state.0.lock().unwrap();
            let now = now_ms();
            if let Some(command) = inner.game.as_ref().and_then(|g| g.due_command(now)) {
                if let Err(e) = execute(&app, &mut inner, command) {
                    eprintln!("ticker: {e}");
                }
            } else if last_clock_event.elapsed() >= CLOCK_EVENT_EVERY {
                last_clock_event = Instant::now();
                let clock = inner.game.as_ref().and_then(|g| g.clock_view(now)).filter(|c| c.running);
                if let Some(clock) = clock {
                    let _ = app.emit("clock-tick", clock);
                }
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState(Mutex::new(Inner {
            game: None,
            session: None,
            media_root: None,
            driver: Box::new(VirtualDriver),
        })))
        .setup(|app| {
            let hardware = Hardware::start(app.handle());
            if let Some(handle) = hardware.handle() {
                app.state::<AppState>().0.lock().unwrap().driver = Box::new(HardwareDriver(handle));
            }
            app.manage(hardware);
            spawn_ticker(app.handle().clone());
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
