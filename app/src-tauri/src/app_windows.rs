//! The app's two windows: the Director, the quizmaster's (from
//! `tauri.conf.json`), and the projector, which the Director opens. A live
//! show must lose neither by accident: the Director asks before it closes
//! during a show and the app ends with it, the windows stay on the
//! app's own pages, and the projector opens on another screen than
//! the Director's.

use crate::error::CmdResult;
use crate::{app_icon, app_log, off_ui, AppState};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuEvent};
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Emitter, Manager, Monitor, Runtime, State, Url, WebviewUrl, WebviewWindowBuilder, Window, WindowEvent};
use ts_rs::TS;

/// The projector window.
pub const HOST_LABEL: &str = "host";
/// The quizmaster's window (`tauri.conf.json`).
pub const DIRECTOR_LABEL: &str = "director";
/// Asks the Director to confirm that the app closes during a show.
const CLOSE_REQUESTED_EVENT: &str = "close-requested";
/// How long a close request waits for the Director's answer: a Director
/// whose page failed or hangs never gives one, so a request made after that
/// closes the app without asking.
pub(crate) const CLOSE_ANSWER_WAIT: Duration = Duration::from_secs(10);
/// How long an unanswered close request lasts. A later close asks again, so
/// a request whose answer was lost (its dialog failed to open) does not let
/// an accidental close go through without asking an hour later.
const CLOSE_REQUEST_LASTS: Duration = Duration::from_secs(60);
/// The app menu's Quit on macOS, which asks during a show.
const QUIT_MENU_ID: &str = "quit";

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

/// The projector window as it is now, for the check before a show (spec SF-2).
#[derive(Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ProjectorWindow {
    /// It is on the monitor the Director is on.
    on_director_monitor: bool,
    /// How many monitors the computer has.
    monitors: usize,
    fullscreen: bool,
}

/// Where the projector window is now; none while it is closed.
#[tauri::command]
pub async fn projector_window(app: AppHandle) -> CmdResult<Option<ProjectorWindow>> {
    let Some(host) = app.get_webview_window(HOST_LABEL) else {
        return Ok(None);
    };
    // A window closing meanwhile answers no more: as closed.
    let (Ok(monitor), Ok(fullscreen)) = (host.current_monitor(), host.is_fullscreen()) else {
        return Ok(None);
    };
    let on = monitor.as_ref().map(monitor_id);
    Ok(Some(ProjectorWindow {
        on_director_monitor: on.is_some() && on == director_monitor(&app),
        monitors: app.available_monitors()?.len(),
        fullscreen,
    }))
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
/// It goes in the log too, to look into after the show, off the windows'
/// thread (`off_ui`): a disk that does not answer freezes no window.
#[tauri::command]
pub async fn report_host_problem(app: AppHandle, message: String) -> CmdResult<()> {
    off_ui(move || {
        app_log::note(&format!("the projector: {message}"));
        let _ = app.emit("host-problem", message);
        Ok(())
    })
    .await
}

/// The most of a window's problem that goes in the log: a window can send
/// text of any size, and a long one would push the rest out of the log.
const LOGGED_PROBLEM_CHARS: usize = 2_000;

/// Notes in the log a problem a window has nowhere to show, such as the
/// Director's close dialog not opening once its screen failed. Written off
/// the windows' thread (`off_ui`), as a window may note many.
#[tauri::command]
pub async fn log_problem(window: Window, message: String) -> CmdResult<()> {
    off_ui(move || {
        app_log::note(&format!("the {} window: {}", window.label(), logged_problem(&message)));
        Ok(())
    })
    .await
}

/// `message` as it goes in the log: cut after `LOGGED_PROBLEM_CHARS`.
fn logged_problem(message: &str) -> String {
    match message.char_indices().nth(LOGGED_PROBLEM_CHARS) {
        Some((cut, _)) => format!("{}… ({} bytes)", &message[..cut], message.len()),
        None => message.to_owned(),
    }
}

/// Ends the app, once the quizmaster confirmed it during a show. Every
/// command is on disk already, so the show can be resumed.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// When the Director was asked to confirm closing the app, until it answers.
/// The windows and the show state share it: a request left unanswered is
/// about the show running when it was made, so the show state forgets it
/// when a show begins or ends (`replace_show`).
#[derive(Clone, Default)]
pub struct CloseRequest(Arc<Mutex<Option<Instant>>>);

impl CloseRequest {
    /// What a request to close the app during a show does `now`.
    pub(crate) fn answer(&self, now: Instant) -> CloseAnswer {
        close_answer(&mut self.0.lock().unwrap(), now)
    }

    /// The next close asks again.
    pub fn forget(&self) {
        *self.0.lock().unwrap() = None;
    }
}

/// The quizmaster chose not to close the app: the next close asks again.
#[tauri::command]
pub fn close_declined(request: State<CloseRequest>) {
    request.forget();
}

/// What a request to close the app during a show does, given when the
/// Director was `asked` and has not answered yet.
#[derive(Debug, PartialEq)]
pub(crate) enum CloseAnswer {
    /// Ask the Director to confirm it.
    Ask,
    /// The Director is asking already: no second dialog.
    Wait,
    /// The Director did not answer a recent request in time: close.
    Close,
}

fn close_answer(asked: &mut Option<Instant>, now: Instant) -> CloseAnswer {
    match asked.map(|at| now.saturating_duration_since(at)) {
        Some(waited) if waited < CLOSE_ANSWER_WAIT => CloseAnswer::Wait,
        Some(waited) if waited < CLOSE_REQUEST_LASTS => CloseAnswer::Close,
        _ => {
            *asked = Some(now);
            CloseAnswer::Ask
        }
    }
}

/// Whether the app may close now. During a show it waits for the Director
/// to confirm it (`close-requested`, then `quit_app` or `close_declined`),
/// asking once, unless a recent request went unanswered (`close_answer`).
fn may_close(app: &AppHandle) -> bool {
    if !show_may_run(app) || app.get_webview_window(DIRECTOR_LABEL).is_none() {
        return true;
    }
    match app.state::<CloseRequest>().answer(Instant::now()) {
        CloseAnswer::Ask => {
            let _ = app.emit_to(DIRECTOR_LABEL, CLOSE_REQUESTED_EVENT, ());
            false
        }
        CloseAnswer::Wait => false,
        CloseAnswer::Close => {
            app_log::note("the Director did not answer a request to close during a show: the app closes");
            true
        }
    }
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
/// it (`may_close`), and the app ends with the Director, so a full-screen
/// projector is never left alone. However the projector closes (also from
/// its title bar), the Director hears of it.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let app = window.app_handle();
    match (window.label(), event) {
        (DIRECTOR_LABEL, WindowEvent::CloseRequested { api, .. }) => {
            if !may_close(app) {
                api.prevent_close();
            }
        }
        (DIRECTOR_LABEL, WindowEvent::Destroyed) => app.exit(0),
        (HOST_LABEL, WindowEvent::Destroyed) => {
            let _ = app.emit("host-window", false);
        }
        _ => {}
    }
}

/// The words of the macOS app menu in the UI's language
/// (`set_menu_words`): the menu's own items, and those Tauri gives fixed
/// English titles.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MenuWords {
    /// The app's Quit, with the app's name.
    pub quit: String,
    /// With the app's name, as `hide`.
    pub about: String,
    pub services: String,
    pub hide: String,
    pub hide_others: String,
    pub file: String,
    pub close_window: String,
    pub edit: String,
    pub undo: String,
    pub redo: String,
    pub cut: String,
    pub copy: String,
    pub paste: String,
    pub select_all: String,
    pub view: String,
    pub fullscreen: String,
    pub window: String,
    pub minimize: String,
    pub maximize: String,
}

/// The macOS app menu, as Tauri's default one but with a Quit of the app's
/// own: the system's Quit ends the app at once, with no event to wait on,
/// so Cmd+Q would end a show without asking. Quit from the Dock still
/// does; the show stays saved and can be resumed. In English until the
/// Director words it.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn app_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    worded_menu(app, None)
}

/// The app menu in `words`, or in English: the menu's own items as here,
/// the others with Tauri's titles.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn worded_menu<R: Runtime>(app: &AppHandle<R>, words: Option<&MenuWords>) -> tauri::Result<Menu<R>> {
    use tauri::menu::{AboutMetadata, MenuBuilder, MenuItemBuilder, PredefinedMenuItem as Item, SubmenuBuilder};
    let name = &app.package_info().name;
    let word = |pick: fn(&MenuWords) -> &String| words.map(|words| pick(words).as_str());
    let title = |pick: fn(&MenuWords) -> &String, english: &str| word(pick).unwrap_or(english).to_owned();
    let about = AboutMetadata { name: Some(name.clone()), version: Some(app.package_info().version.to_string()), ..Default::default() };
    let quit = MenuItemBuilder::with_id(QUIT_MENU_ID, title(|w| &w.quit, &format!("Quit {name}"))).accelerator("CmdOrCtrl+Q").build(app)?;
    let app_menu = SubmenuBuilder::new(app, name)
        .item(&Item::about(app, word(|w| &w.about), Some(about))?)
        .separator()
        .item(&Item::services(app, word(|w| &w.services))?)
        .separator()
        .item(&Item::hide(app, word(|w| &w.hide))?)
        .item(&Item::hide_others(app, word(|w| &w.hide_others))?)
        .separator()
        .item(&quit)
        .build()?;
    let file = SubmenuBuilder::new(app, title(|w| &w.file, "File")).item(&Item::close_window(app, word(|w| &w.close_window))?).build()?;
    let edit = SubmenuBuilder::new(app, title(|w| &w.edit, "Edit"))
        .item(&Item::undo(app, word(|w| &w.undo))?)
        .item(&Item::redo(app, word(|w| &w.redo))?)
        .separator()
        .item(&Item::cut(app, word(|w| &w.cut))?)
        .item(&Item::copy(app, word(|w| &w.copy))?)
        .item(&Item::paste(app, word(|w| &w.paste))?)
        .item(&Item::select_all(app, word(|w| &w.select_all))?)
        .build()?;
    let view = SubmenuBuilder::new(app, title(|w| &w.view, "View")).item(&Item::fullscreen(app, word(|w| &w.fullscreen))?).build()?;
    let window = SubmenuBuilder::new(app, title(|w| &w.window, "Window"))
        .item(&Item::minimize(app, word(|w| &w.minimize))?)
        .item(&Item::maximize(app, word(|w| &w.maximize))?)
        .separator()
        .item(&Item::close_window(app, word(|w| &w.close_window))?)
        .build()?;
    MenuBuilder::new(app).items(&[&app_menu, &file, &edit, &view, &window]).build()
}

/// Words the macOS app menu in the UI's language; other systems have no app menu.
#[tauri::command]
pub fn set_menu_words(app: AppHandle, words: MenuWords) -> CmdResult<()> {
    #[cfg(target_os = "macos")]
    app.set_menu(worded_menu(&app, Some(&words))?)?;
    #[cfg(not(target_os = "macos"))]
    let _ = (app, words);
    Ok(())
}

/// The app menu's Quit closes the app as the Director's close button does.
pub fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    if event.id() == QUIT_MENU_ID && may_close(app) {
        app.exit(0);
    }
}

/// Brings the Director forward when the app is started again, and
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

/// Keeps every window on the app's own pages: a link, a file dropped
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
            assert!(projector.iter().any(|c| c == command), "the projector shows the show and its keys step it: {command}");
        }
        for command in ["editor_save", "editor_open", "start_show_from_file", "start_show_from_recent", "receiver_select", "quit_app"] {
            assert!(!projector.iter().any(|c| c == command), "the projector opens no file and changes no setting: {command}");
        }
    }

    #[test]
    fn a_long_problem_is_cut_in_the_log() {
        let most = "é".repeat(LOGGED_PROBLEM_CHARS);
        assert_eq!(logged_problem(&most), most);
        let long = format!("{most}é");
        assert_eq!(logged_problem(&long), format!("{most}… ({} bytes)", long.len()));
    }

    #[test]
    fn a_close_request_asks_once_and_closes_when_it_goes_unanswered() {
        let start = Instant::now();
        let mut asked = None;
        assert_eq!(close_answer(&mut asked, start), CloseAnswer::Ask);
        assert_eq!(asked, Some(start));
        // A second close while the Director asks opens no second dialog.
        assert_eq!(close_answer(&mut asked, start + Duration::from_secs(3)), CloseAnswer::Wait);
        // A Director that never answers does not keep the app open.
        assert_eq!(close_answer(&mut asked, start + CLOSE_ANSWER_WAIT), CloseAnswer::Close);
        // A request whose answer was lost does not let a much later close through.
        let later = start + CLOSE_REQUEST_LASTS;
        assert_eq!(close_answer(&mut asked, later), CloseAnswer::Ask);
        assert_eq!(asked, Some(later));
        assert_eq!(close_answer(&mut asked, later + Duration::from_secs(3)), CloseAnswer::Wait);
        // Once the quizmaster declined (`close_declined`), or a show began or ended (`replace_show`), the next close asks again.
        asked = None;
        assert_eq!(close_answer(&mut asked, later + Duration::from_secs(5)), CloseAnswer::Ask);
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
