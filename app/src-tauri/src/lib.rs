//! Desktop shell: owns the live game, journals every accepted command before
//! the windows see it, drives the buzzer lights and the projector window.

mod session;

use buzzers::{BuzzerDriver, VirtualDriver};
use quiz_core::{Command, Effect, Game, GameView, Quiz, Stamped};
use serde::Serialize;
use session::{Session, SessionMeta, SessionSummary};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

const DEMO_QUIZ: &str = include_str!("../../../samples/demo-quiz.json");
const HOST_LABEL: &str = "host";
const DIRECTOR_LABEL: &str = "director";
const TICK: Duration = Duration::from_millis(50);

struct Inner {
    game: Option<Game>,
    session: Option<Session>,
    driver: Box<dyn BuzzerDriver>,
}

struct AppState(Mutex<Inner>);

type CmdResult<T> = Result<T, String>;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn sessions_root(app: &AppHandle) -> CmdResult<PathBuf> {
    app.path().app_data_dir().map(|dir| dir.join("sessions")).map_err(|e| e.to_string())
}

fn emit_state(app: &AppHandle, game: Option<&Game>) {
    let view = game.map(|g| g.view(now_ms()));
    let _ = app.emit("game-state", view);
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

fn start_show(app: &AppHandle, inner: &mut Inner, quiz: Quiz, source_path: Option<String>) -> CmdResult<()> {
    quiz.validate().map_err(|errors| {
        errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n")
    })?;
    let meta = SessionMeta { quiz_title: quiz.title.clone(), source_path, started_ms: now_ms() };
    let session = Session::create(&sessions_root(app)?, &quiz, &meta).map_err(|e| e.to_string())?;
    inner.game = Some(Game::new(quiz));
    inner.session = Some(session);
    emit_state(app, inner.game.as_ref());
    Ok(())
}

#[tauri::command]
fn get_view(state: State<AppState>) -> Option<GameView> {
    let inner = state.0.lock().unwrap();
    inner.game.as_ref().map(|g| g.view(now_ms()))
}

#[tauri::command]
fn start_show_from_file(app: AppHandle, state: State<AppState>, path: String) -> CmdResult<()> {
    let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let quiz = Quiz::from_json(&json)?;
    start_show(&app, &mut state.0.lock().unwrap(), quiz, Some(path))
}

#[tauri::command]
fn start_demo_show(app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    let quiz = Quiz::from_json(DEMO_QUIZ)?;
    start_show(&app, &mut state.0.lock().unwrap(), quiz, None)
}

#[tauri::command]
fn last_session(app: AppHandle) -> CmdResult<Option<SessionSummary>> {
    Session::latest_summary(&sessions_root(&app)?).map_err(|e| e.to_string())
}

#[tauri::command]
fn resume_last_session(app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    let (session, quiz, journal) = Session::resume_latest(&sessions_root(&app)?)
        .map_err(|e| e.to_string())?
        .ok_or("there is no show to resume")?;
    let mut inner = state.0.lock().unwrap();
    let game = Game::replay(quiz, &journal);
    let recovery = game.recovery_command();
    inner.game = Some(game);
    inner.session = Some(session);
    match recovery {
        Some(command) => execute(&app, &mut inner, command),
        None => {
            emit_state(&app, inner.game.as_ref());
            Ok(())
        }
    }
}

#[tauri::command]
fn dispatch(app: AppHandle, state: State<AppState>, command: Command) -> CmdResult<()> {
    execute(&app, &mut state.0.lock().unwrap(), command)
}

#[derive(Serialize)]
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
fn close_host_window(app: AppHandle) -> CmdResult<()> {
    match app.get_webview_window(HOST_LABEL) {
        Some(window) => window.close().map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

/// Issues time-driven commands such as "time is up".
fn spawn_ticker(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(TICK);
        let state = app.state::<AppState>();
        let mut inner = state.0.lock().unwrap();
        let due = inner.game.as_ref().and_then(|g| g.due_command(now_ms()));
        if let Some(command) = due {
            if let Err(e) = execute(&app, &mut inner, command) {
                eprintln!("ticker: {e}");
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
            driver: Box::new(VirtualDriver),
        })))
        .setup(|app| {
            spawn_ticker(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_view,
            start_show_from_file,
            start_demo_show,
            last_session,
            resume_last_session,
            dispatch,
            list_monitors,
            open_host_window,
            close_host_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
