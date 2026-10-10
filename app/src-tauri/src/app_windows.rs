//! The app's two windows: the Director, the quizmaster's (from
//! `tauri.conf.json`), and the projector, which the Director opens. A live
//! show must lose neither by accident: the Director asks before it closes
//! during a show and the app ends with it (A10), the windows stay on the
//! app's own pages (A11), and the projector opens on another screen than
//! the Director's (A13).

use crate::error::CmdResult;
use crate::{app_icon, app_log, AppState};
use serde::Serialize;
use std::sync::TryLockError;
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Emitter, Manager, Monitor, RunEvent, Runtime, Url, WebviewUrl, WebviewWindowBuilder, Window, WindowEvent};
use ts_rs::TS;

/// The projector window.
pub const HOST_LABEL: &str = "host";
/// The quizmaster's window (`tauri.conf.json`).
pub const DIRECTOR_LABEL: &str = "director";
/// Asks the Director to confirm that the app closes during a show.
const CLOSE_REQUESTED_EVENT: &str = "close-requested";

#[derive(Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    /// Names the monitor while it stays where it is (`monitor_id`).
    id: String,
    /// None when the system gives no name; the UI numbers the monitor then.
    name: Option<String>,
    width: u32,
    height: u32,
    /// The Director is on this monitor.
    director: bool,
}

/// A monitor by its name and position: the same monitor plugged in again
/// where it was keeps it, while its index in the list may change.
fn monitor_id(monitor: &Monitor) -> String {
    let at = monitor.position();
    format!("{}@{},{}", monitor.name().map_or("", String::as_str), at.x, at.y)
}

/// The monitor the Director is on, if known.
fn director_monitor(app: &AppHandle) -> Option<String> {
    let director = app.get_webview_window(DIRECTOR_LABEL)?;
    director.current_monitor().ok().flatten().as_ref().map(monitor_id)
}

// The window commands are async: Tauri runs them off the main thread, which
// creating a window from a synchronous command would deadlock on Windows
// (WebView2; see the `WebviewWindowBuilder::new` docs).

#[tauri::command]
pub async fn list_monitors(app: AppHandle) -> CmdResult<Vec<MonitorInfo>> {
    let director = director_monitor(&app);
    Ok(app
        .available_monitors()?
        .iter()
        .map(|m| MonitorInfo {
            id: monitor_id(m),
            name: m.name().cloned(),
            width: m.size().width,
            height: m.size().height,
            director: director.as_deref() == Some(monitor_id(m).as_str()),
        })
        .collect())
}

/// Where the projector opens among the monitors `ids`: the `chosen` one
/// while it is there, else the first one the Director (on `director`) is
/// not on, else the Director's own. Gives its index, and whether it may go
/// full screen: not over the Director, unless the quizmaster chose that.
fn projector_place(ids: &[String], director: Option<&str>, chosen: Option<&str>) -> Option<(usize, bool)> {
    if let Some(index) = chosen.and_then(|chosen| ids.iter().position(|id| id == chosen)) {
        return Some((index, true));
    }
    match ids.iter().position(|id| Some(id.as_str()) != director) {
        Some(index) => Some((index, true)),
        None => (!ids.is_empty()).then_some((0, false)),
    }
}

/// Opens (or moves) the projector window on the `monitor` chosen (by
/// `MonitorInfo::id`), or on a screen the Director is not on, full screen
/// if so asked (`projector_place`). Monitors are listed now, so one plugged
/// in since the Director listed them counts.
#[tauri::command]
pub async fn open_host_window(app: AppHandle, monitor: Option<String>, fullscreen: bool) -> CmdResult<()> {
    let window = match app.get_webview_window(HOST_LABEL) {
        Some(window) => window,
        None => {
            let window = WebviewWindowBuilder::new(&app, HOST_LABEL, WebviewUrl::App("index.html".into()))
                .title("Buzzdome")
                .inner_size(1280.0, 720.0)
                .build()?;
            app_icon::apply(&app, &window);
            window
        }
    };
    window.set_fullscreen(false)?;
    let monitors = app.available_monitors()?;
    let ids: Vec<String> = monitors.iter().map(monitor_id).collect();
    let place = projector_place(&ids, director_monitor(&app).as_deref(), monitor.as_deref());
    if let Some((index, may_fill)) = place {
        window.set_position(*monitors[index].position())?;
        window.set_fullscreen(fullscreen && may_fill)?;
    }
    window.set_focus()?;
    let _ = app.emit("host-window", true);
    Ok(())
}

#[tauri::command]
pub fn host_window_open(app: AppHandle) -> bool {
    app.get_webview_window(HOST_LABEL).is_some()
}

#[tauri::command]
pub async fn close_host_window(app: AppHandle) -> CmdResult<()> {
    match app.get_webview_window(HOST_LABEL) {
        Some(window) => Ok(window.close()?),
        None => Ok(()),
    }
}

/// Passes on a problem the projector met, such as a background that does
/// not load, worded for the Director to show: the projector has nowhere to.
#[tauri::command]
pub fn report_host_problem(app: AppHandle, message: String) {
    let _ = app.emit("host-problem", message);
}

/// Ends the app, once the quizmaster confirmed it during a show. Every
/// command is on disk already, so the show can be resumed.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Whether a show may be running, without waiting for the show state: this
/// runs on the windows' thread, and a journal write may hold the state,
/// which then is held for a show.
fn show_may_run(app: &AppHandle) -> bool {
    match app.state::<AppState>().0.try_lock() {
        Ok(inner) => inner.game.is_some(),
        Err(TryLockError::WouldBlock | TryLockError::Poisoned(_)) => true,
    }
}

/// Closing the Director during a show waits for the quizmaster to confirm
/// it (`close-requested`, then `quit_app`), and the app ends with the
/// Director, so a full-screen projector is never left alone. However the
/// projector closes (also from its title bar), the Director hears of it.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let app = window.app_handle();
    match (window.label(), event) {
        (DIRECTOR_LABEL, WindowEvent::CloseRequested { api, .. }) if show_may_run(app) => {
            api.prevent_close();
            let _ = app.emit_to(DIRECTOR_LABEL, CLOSE_REQUESTED_EVENT, ());
        }
        (DIRECTOR_LABEL, WindowEvent::Destroyed) => app.exit(0),
        (HOST_LABEL, WindowEvent::Destroyed) => {
            let _ = app.emit("host-window", false);
        }
        _ => {}
    }
}

/// An exit the quizmaster asked for (such as Cmd+Q on macOS; `code` is
/// none) waits for the Director's confirmation during a show too.
pub fn on_run_event(app: &AppHandle, event: RunEvent) {
    if let RunEvent::ExitRequested { code: None, api, .. } = event {
        if show_may_run(app) && app.get_webview_window(DIRECTOR_LABEL).is_some() {
            api.prevent_exit();
            let _ = app.emit_to(DIRECTOR_LABEL, CLOSE_REQUESTED_EVENT, ());
        }
    }
}

/// Brings the Director forward when the app is started again (Z2), and
/// builds it again if it is gone.
pub fn show_director(app: &AppHandle) {
    if let Some(director) = app.get_webview_window(DIRECTOR_LABEL) {
        let _ = director.unminimize();
        let _ = director.show();
        let _ = director.set_focus();
        return;
    }
    let app = app.clone();
    // Not on this thread, the event loop's: a new window waits for it.
    tauri::async_runtime::spawn(async move {
        let Some(config) = app.config().app.windows.iter().find(|w| w.label == DIRECTOR_LABEL) else { return };
        match WebviewWindowBuilder::from_config(&app, config).and_then(|builder| builder.build()) {
            Ok(window) => app_icon::apply(&app, &window),
            Err(e) => app_log::note(&format!("director window: {e}")),
        }
    });
}

/// Keeps every window on the app's own pages (A11): a link, a file dropped
/// where nothing takes it or a stray script cannot replace the Director or
/// the projector. The first page of a window loads before this applies.
pub fn navigation_guard<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("navigation-guard")
        .on_navigation(|webview, url| {
            let dev = if tauri::is_dev() { webview.app_handle().config().build.dev_url.clone() } else { None };
            is_app_url(url, dev.as_ref())
        })
        .build()
}

/// Whether `url` is one of the app's pages: served by Tauri
/// (`tauri://localhost`, or `http(s)://tauri.localhost` on Windows) or, in a
/// development build, by the development server at `dev`.
fn is_app_url(url: &Url, dev: Option<&Url>) -> bool {
    let served = match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" | "https" => url.host_str() == Some("tauri.localhost"),
        _ => false,
    };
    served || dev.is_some_and(|dev| dev.origin() == url.origin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_windows_stay_on_the_apps_pages() {
        let url = |s: &str| Url::parse(s).unwrap();
        let dev = url("http://localhost:1420/");
        for page in ["tauri://localhost/index.html", "http://tauri.localhost/", "https://tauri.localhost/index.html#x"] {
            assert!(is_app_url(&url(page), None), "{page}");
        }
        assert!(is_app_url(&url("http://localhost:1420/index.html"), Some(&dev)));
        for page in [
            "file:///C:/Users/q/Music/intro.mp3",
            "https://example.com/",
            "http://localhost:1420/",
            "http://tauri.localhost.example.com/",
            "tauri://elsewhere/",
            "about:blank",
        ] {
            assert!(!is_app_url(&url(page), None), "{page}");
        }
        assert!(!is_app_url(&url("http://localhost:5173/"), Some(&dev)));
    }

    /// The text between `start` and the next `end` after it.
    fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
        let from = text.find(start).expect(start) + start.len();
        &text[from..from + text[from..].find(end).expect(end)]
    }

    /// The app's commands (`allow-…`) a capability file grants.
    fn granted(capability: &str) -> Vec<String> {
        let capability: serde_json::Value = serde_json::from_str(capability).unwrap();
        let permissions = capability["permissions"].as_array().unwrap().iter().map(|p| p.as_str().unwrap());
        permissions.filter_map(|p| p.strip_prefix("allow-")).map(|c| c.replace('-', "_")).collect()
    }

    #[test]
    fn the_windows_get_exactly_the_commands_there_are() {
        let registered: Vec<&str> =
            between(include_str!("lib.rs"), "generate_handler![", "]").split(',').map(str::trim).filter(|c| !c.is_empty()).map(|c| c.rsplit("::").next().unwrap()).collect();
        let declared: Vec<&str> = between(include_str!("../build.rs"), "COMMANDS: &[&str] = &[", "];").split(',').map(|c| c.trim().trim_matches('"')).filter(|c| !c.is_empty()).collect();
        assert_eq!(declared, registered, "build.rs lists the commands generate_handler! registers");
        assert_eq!(granted(include_str!("../capabilities/director.json")), registered, "the Director may call every command");
        let projector = granted(include_str!("../capabilities/projector.json"));
        assert!(projector.iter().all(|c| registered.contains(&c.as_str())), "{projector:?}");
        for command in ["dispatch", "get_view", "close_host_window"] {
            assert!(projector.iter().any(|c| c == command), "the projector shows the show and its keys step it (A13): {command}");
        }
        for command in ["editor_save", "editor_open", "start_show_from_file", "receiver_select", "quit_app"] {
            assert!(!projector.iter().any(|c| c == command), "the projector opens no file and changes no setting: {command}");
        }
    }

    #[test]
    fn the_projector_opens_where_chosen_else_away_from_the_director() {
        let ids: Vec<String> = ["laptop@0,0", "projector@1920,0"].map(String::from).to_vec();
        // Chosen and there: full screen, even over the Director.
        assert_eq!(projector_place(&ids, Some("laptop@0,0"), Some("laptop@0,0")), Some((0, true)));
        assert_eq!(projector_place(&ids, Some("laptop@0,0"), Some("projector@1920,0")), Some((1, true)));
        // Not chosen, or no longer there: the screen the Director is not on.
        assert_eq!(projector_place(&ids, Some("laptop@0,0"), None), Some((1, true)));
        assert_eq!(projector_place(&ids, Some("projector@1920,0"), Some("gone@0,0")), Some((0, true)));
        // Plugged in late: only the Director's screen at first, so a window over it.
        assert_eq!(projector_place(&ids[..1], Some("laptop@0,0"), None), Some((0, false)));
        assert_eq!(projector_place(&ids, None, None), Some((0, true)));
        assert_eq!(projector_place(&[], None, None), None);
    }
}
