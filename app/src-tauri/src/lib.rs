//! Desktop shell: owns the live game, journals every accepted command before
//! the windows see it and drives the buzzer lights. `app_windows` keeps the
//! Director and projector windows.

mod ai;
mod app_file;
mod app_icon;
mod app_log;
mod app_windows;
#[cfg(test)]
mod crash_drill;
mod dialogs;
mod editor;
mod error;
mod fixture_library;
mod fonts;
mod hardware;
mod media_player;
mod pump;
mod recent_files;
mod remote;
mod session;
pub mod sheet_reader;
mod show_thread;
mod stage_lights;
mod teams;
mod themes;

use buzzers::clock::ShowClock;
use buzzers::receiver::HardwareDriver;
use buzzers::{BuzzerDriver, Press, VirtualDriver};
use dialogs::Picker;
use error::CmdResult;
pub use error::AppError;
use hardware::Hardware;
use quiz_core::{BuzzerId, BuzzerMode, Command, Effect, Game, GameView, Light, Quiz, Stamped};
use recent_files::RecentFiles;
use session::{Resumed, Session, SessionMeta, SessionSummary};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, LazyLock, Mutex, MutexGuard, TryLockError};
use tauri::{AppHandle, Emitter, Manager, Window};

const DEMO_QUIZ: &str = include_str!("../../../samples/demo-quiz.json");

struct Inner {
    game: Option<Game>,
    session: Option<Session>,
    /// Folder with the running quiz's media files, served to the windows.
    media_root: Option<PathBuf>,
    driver: Box<dyn BuzzerDriver>,
    /// The team names prepared for the next show (`teams`).
    teams: Vec<String>,
    /// Where the prepared names are kept, and why they were not read, if
    /// so (for the Teams tab). Known once the app has started.
    teams_file: Option<app_file::AppFile>,
    /// Why the running show's last journal write failed, if it did; none
    /// once one works again. The Director shows it.
    save_problem: Option<AppError>,
    /// When the running show began on the show clock (`before_show`).
    began_ms: u64,
    /// The windows' close request, forgotten when a show begins or ends.
    close_request: app_windows::CloseRequest,
}

impl Inner {
    /// No show, the on-screen buzzers and no team names: the app as it starts.
    fn new() -> Inner {
        Inner {
            game: None,
            session: None,
            media_root: None,
            driver: Box::new(VirtualDriver),
            teams: Vec::new(),
            teams_file: None,
            save_problem: None,
            began_ms: 0,
            close_request: app_windows::CloseRequest::default(),
        }
    }

    /// Whether a command or press given at `at_ms` came before the running
    /// show began, while it started: it was meant for no show or the one
    /// before, and does not reach this one.
    fn before_show(&self, at_ms: u64) -> bool {
        at_ms < self.began_ms
    }
}

struct AppState(Mutex<Inner>);

/// The show state. The app's locks are taken in one order: the editor's
/// (`editor::lock`), then this one, then the learn lock (`hardware`). So
/// code holding the learn lock never waits for this one, which a journal
/// write can hold for a while. A settings change
/// (`Hardware::change_settings`) is never taken under this one, and is
/// taken before the learn lock. The close request
/// (`app_windows::CloseRequest`) is taken under this one when a show begins
/// or ends, and holds no other lock; so does what the stage lights were
/// last told (`stage_lights`), taken under this one with every change.
fn lock_show(app: &AppHandle) -> MutexGuard<'_, Inner> {
    app.state::<AppState>().inner().0.lock().unwrap()
}

/// The clock every command and press is stamped with, shared with the
/// receiver thread (`Stamped::at_ms`).
static SHOW_CLOCK: LazyLock<ShowClock> = LazyLock::new(ShowClock::start);

fn now_ms() -> u64 {
    SHOW_CLOCK.now_ms()
}

/// Runs `work` on a thread that may wait, never the windows' thread: Tauri
/// runs synchronous commands there, so one that waits for the show state,
/// which a journal write holds, would freeze both windows.
async fn off_ui<T: Send + 'static>(work: impl FnOnce() -> CmdResult<T> + Send + 'static) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(work).await?
}

/// Reads the show state off the windows' thread (`off_ui`).
async fn read_show<T: Send + 'static>(app: AppHandle, read: impl FnOnce(&Inner) -> T + Send + 'static) -> CmdResult<T> {
    off_ui(move || Ok(read(&lock_show(&app)))).await
}

/// `folder` as text ending with a path separator, so the windows can
/// append a media reference to it.
pub(crate) fn folder_prefix(folder: &Path) -> String {
    folder.join("").to_string_lossy().into_owned()
}

/// A folder of the app's data, which a roaming profile on Windows takes
/// along to the user's other computers: settings, sessions, the media library.
pub(crate) fn app_data(app: &AppHandle, folder: &str) -> CmdResult<PathBuf> {
    Ok(app.path().app_data_dir()?.join(folder))
}

/// A folder of the app's data for this computer only (Local, not Roaming,
/// on Windows; elsewhere the same as `app_data`): unpacked quizzes and
/// editor drafts, which can be large.
pub(crate) fn app_local_data(app: &AppHandle, folder: &str) -> CmdResult<PathBuf> {
    Ok(app.path().app_local_data_dir()?.join(folder))
}

/// `folder` where earlier versions kept it (`app_data`), while it is still
/// there and that is not where it is kept now (`app_local_data`).
pub(crate) fn roaming_leftover(app: &AppHandle, folder: &str) -> Option<PathBuf> {
    let old = app_data(app, folder).ok()?;
    (old != app_local_data(app, folder).ok()? && old.exists()).then_some(old)
}

fn sessions_root(app: &AppHandle) -> CmdResult<PathBuf> {
    app_data(app, "sessions")
}

const QUIZZES_FOLDER: &str = "quizzes";

/// Unpacked quiz files (quiz-file packages).
fn quizzes_cache(app: &AppHandle) -> CmdResult<PathBuf> {
    app_local_data(app, QUIZZES_FOLDER)
}

/// Held while a quiz is unpacked into `quizzes_cache` or old ones are
/// deleted from it, so neither runs during the other.
static QUIZZES_CACHE: Mutex<()> = Mutex::new(());

/// Held while a quiz file opens and its show starts (`one_start`).
static STARTING: Mutex<()> = Mutex::new(());

/// The folders a show's media can be in, unpacked quizzes and editor
/// drafts: the windows may load files from these, allowed once at start,
/// and from no other folder of the disk.
fn media_folders(app: &AppHandle) -> CmdResult<[PathBuf; 2]> {
    Ok([quizzes_cache(app)?, editor::editor_dir(app)?])
}

/// Whether the media folder `root` is outside the `folders` the windows may
/// load from (`media_folders`), with links followed (`leads_inside`). One
/// that is gone is missing, not refused.
fn refused(root: &Path, folders: &[PathBuf]) -> bool {
    let inside: std::io::Result<Vec<bool>> = folders.iter().map(|folder| quiz_file::leads_inside(root, folder)).collect();
    inside.is_ok_and(|inside| !inside.contains(&true))
}

fn emit_state(app: &AppHandle, game: Option<&Game>) {
    let view = game.map(|g| g.view(now_ms()));
    let _ = app.emit("game-state", view);
}

/// A different show (or none) is loaded: the windows hear of it and reload the quiz.
fn show_changed(app: &AppHandle, inner: &Inner) {
    let _ = app.emit("show-changed", ());
    // A show has its own team names, and the prepared ones are back after it.
    let _ = app.emit("teams-changed", teams::current(inner));
    emit_state(app, inner.game.as_ref());
}

/// Applies a command that happened at `at_ms` on a copy, journals it, then
/// commits. The show never shows a state that is not on disk. Gives false
/// for a press that changes nothing, which is neither journaled nor shown.
fn execute_at(app: &AppHandle, inner: &mut Inner, command: Command, at_ms: u64) -> CmdResult<bool> {
    if inner.before_show(at_ms) {
        return Err(AppError::NoShow);
    }
    let Some(effects) = apply_journaled(inner, command, at_ms)? else {
        return Ok(false);
    };
    deliver(app, inner, effects);
    Ok(true)
}

/// Applies `command` to a copy of the running show and journals it; only
/// then does the copy replace the show (S1). Gives the effects, for
/// `deliver`, or none for a press that changes nothing (`Game::ignores`):
/// a burst of those costs no journal write. Keeps whether the journal
/// write failed, for the Director.
fn apply_journaled(inner: &mut Inner, command: Command, at_ms: u64) -> CmdResult<Option<Vec<Effect>>> {
    let game = inner.game.as_ref().ok_or(AppError::NoShow)?;
    let stamped = Stamped { at_ms, command };
    if game.ignores(&stamped) {
        return Ok(None);
    }
    let applied = apply_and_journal(game, inner.session.as_mut(), &stamped);
    match &applied {
        Ok(_) if inner.session.is_some() => inner.save_problem = None,
        Err(e) if e.is_not_saved() => inner.save_problem = Some(e.clone()),
        _ => {}
    }
    let (next, effects) = applied?;
    inner.game = Some(next);
    Ok(Some(effects))
}

/// Makes `game` the running show, or none, journaled in `session` (none in
/// a test run), beginning now. A new journal has no failed write yet, and
/// the next close during a show asks again. Every show begins and ends here.
fn replace_show(inner: &mut Inner, game: Option<Game>, session: Option<Session>) {
    // The buzzers show the new show's lights as it stands and every other
    // one goes dark, whatever they showed before: another show's lights,
    // those a receiver kept while the app restarted, or another program's.
    let lights = game.as_ref().map(Game::lights).unwrap_or_default();
    inner.game = game;
    inner.session = session;
    inner.save_problem = None;
    inner.began_ms = now_ms();
    // A close request left unanswered was about the show before.
    inner.close_request.forget();
    send_mode(inner);
    noted(inner.driver.reset_lights(&lights));
}

/// Tells the buzzers what they take now (`Game::buzzer_mode`). With no show
/// they register as in a lobby, so the Receiver tab sees their presses.
fn send_mode(inner: &mut Inner) {
    let mode = inner.game.as_ref().map_or(BuzzerMode::Register, Game::buzzer_mode);
    inner.driver.set_mode(mode);
}

/// Applies `stamped` to a copy of `game` and journals it in `session` (none
/// in a test run). Gives the copy, to replace the show, and its effects.
fn apply_and_journal(game: &Game, session: Option<&mut Session>, stamped: &Stamped) -> CmdResult<(Game, Vec<Effect>)> {
    let mut next = game.clone();
    let effects = next.apply(stamped)?;
    if let Some(session) = session {
        session.append(stamped)?;
    }
    Ok((next, effects))
}

/// Sends a show change's effects to the buzzers and the windows, then the new state.
fn deliver(app: &AppHandle, inner: &mut Inner, effects: Vec<Effect>) {
    send_effects(app, inner, effects);
    emit_state(app, inner.game.as_ref());
}

/// Sends a show change's effects to the buzzer lights, the stage lights and
/// the windows' sound.
fn send_effects(app: &AppHandle, inner: &mut Inner, effects: Vec<Effect>) {
    light_buzzers(inner, &effects);
    stage_lights::follow(app, inner.game.as_ref(), &effects, now_ms());
    for effect in effects {
        if let Effect::Sound { cue } = effect {
            let _ = app.emit("sound-cue", cue);
        }
    }
}

/// Tells the buzzers what a show change's effects call for. The mode goes
/// before the lights: by the time the buzzers light ready, the receiver
/// thread holds a round's presses for the buzzers' timers, and tells one
/// made in the step before by the light it showed (`buzzers::enjoy303s`).
/// The change's lights go at once.
fn light_buzzers(inner: &mut Inner, effects: &[Effect]) {
    send_mode(inner);
    let lights = Effect::lights(effects);
    if !lights.is_empty() {
        set_lights(inner, &lights);
    }
}

/// Lights the buzzers; a failure is logged and never stops the show.
fn set_lights(inner: &mut Inner, lights: &[(BuzzerId, Light)]) {
    noted(inner.driver.set_lights(lights));
}

/// Logs a failure to light the buzzers, which never stops the show.
fn noted(lit: Result<(), buzzers::DriverError>) {
    if let Err(e) = lit {
        app_log::note(&format!("buzzer lights: {e}"));
    }
}

/// Locks the show state to start a show, refusing while one runs. Learn
/// mode ends, because in a show every press must play; learn mode starts
/// only while holding the show state (`hardware::receiver_learn_start`),
/// so it cannot start again until the show has.
fn lock_for_new_show(app: &AppHandle) -> CmdResult<MutexGuard<'_, Inner>> {
    let inner = lock_show(app);
    if inner.game.is_some() {
        return Err(AppError::ShowRunning);
    }
    hardware::end_learning(app);
    Ok(inner)
}

/// Starts a show of `quiz`, whose media (if any) are in `media_root`.
fn start_show(app: &AppHandle, quiz: Quiz, source_path: Option<String>, media_root: Option<PathBuf>) -> CmdResult<()> {
    quiz_file::check_playable(&quiz, media_root.as_deref().unwrap_or(Path::new("")))?;
    let named = {
        let mut inner = lock_for_new_show(app)?;
        let meta = SessionMeta {
            quiz_title: quiz.title.clone(),
            source_path,
            started_ms: now_ms(),
            media_root: media_root.as_ref().map(|root| root.to_string_lossy().into_owned()),
            rules_version: quiz_core::RULES_VERSION,
        };
        let session = Session::create(&sessions_root(app)?, &quiz, &meta).map_err(|e| AppError::ShowNotSaved { detail: e.to_string() })?;
        inner.media_root = media_root.clone();
        begin_show(app, &mut inner, Game::new(quiz), Some(session))
    };
    // Only the latest show can be resumed, so other unpacked quizzes can go,
    // also when the show runs without the names its journal refused. Not
    // while holding the show state: deleting a big quiz takes a while.
    if let Ok(cache) = quizzes_cache(app) {
        let _cache = QUIZZES_CACHE.lock().unwrap();
        let keep: Vec<&Path> = media_root.iter().map(PathBuf::as_path).collect();
        if let Err(e) = quiz_file::prune(&cache, &keep) {
            app_log::note(&format!("cleaning unpacked quizzes: {e}"));
        }
        // Where earlier versions unpacked (Windows): their show is not the latest either.
        if let Some(old) = roaming_leftover(app, QUIZZES_FOLDER) {
            if let Err(e) = fs::remove_dir_all(&old) {
                app_log::note(&format!("removing {}: {e}", old.display()));
            }
        }
    }
    named
}

/// Makes `game` the running show, journaled in `session` (none in a test
/// run), and gives it the prepared team names as its first command.
fn begin_show(app: &AppHandle, inner: &mut Inner, game: Game, session: Option<Session>) -> CmdResult<()> {
    // The show runs even if its journal refuses the names; the Director says why.
    let named = teams::begin(inner, game, session, now_ms());
    if let Ok(effects) = &named {
        send_effects(app, inner, effects.clone());
    }
    show_changed(app, inner);
    named.map(drop)
}

/// Whether a show (or a test run) is running. Not to be called while
/// holding the show state.
fn show_running(app: &AppHandle) -> bool {
    lock_show(app).game.is_some()
}

// The commands that read or start the show are async, so Tauri runs them
// off the windows' thread, and they take the show state on a thread that
// may wait (`off_ui`). Those that change or end a running show run on
// the show thread (`show_thread::run`).

#[tauri::command]
async fn get_view(app: AppHandle) -> CmdResult<Option<GameView>> {
    read_show(app, |inner| inner.game.as_ref().map(|g| g.view(now_ms()))).await
}

/// The whole quiz, for the Director's slide list and previews.
#[tauri::command]
async fn get_quiz(app: AppHandle) -> CmdResult<Option<Quiz>> {
    read_show(app, |inner| inner.game.as_ref().map(|g| g.quiz().clone())).await
}

/// Folder of the running quiz's media, ending with a path separator, so the
/// windows can append a media reference to it.
#[tauri::command]
async fn media_root(app: AppHandle) -> CmdResult<Option<String>> {
    read_show(app, |inner| inner.media_root.as_deref().map(folder_prefix)).await
}

/// Why the running show's journal cannot be written, or that a write does
/// not finish, if so; also sent with `show-not-saved` whenever it changes.
#[tauri::command]
async fn show_not_saved(app: AppHandle) -> CmdResult<Option<AppError>> {
    Ok(show_thread::not_saved(&app))
}

/// Opens a quiz file the quizmaster picks (`.quiz` or a plain quiz JSON)
/// and starts a show with it; false when they cancel.
#[tauri::command]
async fn start_show_from_file(app: AppHandle, window: Window, picker: Picker) -> CmdResult<bool> {
    let Some(path) = dialogs::pick_file(window, picker, quiz_file::OPEN_EXTENSIONS.to_vec()).await? else { return Ok(false) };
    off_ui(move || start_show_from(&app, &path).map(|()| true)).await
}

/// Starts a show with the `recent` quiz file, which has to be in the list
/// (`RecentFiles`). A file that is no longer there leaves the list.
#[tauri::command]
async fn start_show_from_recent(app: AppHandle, recent: String) -> CmdResult<()> {
    off_ui(move || {
        let path = app.state::<RecentFiles>().get(&recent)?;
        start_show_from(&app, &path)
    })
    .await
}

/// Opens the quiz file `path` and starts a show with it, one start at a
/// time (`one_start`). The file goes first in the recent list once it
/// opens, and leaves it once it is gone.
fn start_show_from(app: &AppHandle, path: &Path) -> CmdResult<()> {
    one_start(&STARTING, || {
        let source_path = path.to_string_lossy().into_owned();
        let recent = app.state::<RecentFiles>();
        // Unpacking big media takes a moment: it runs before the show is locked.
        let opened = {
            let _cache = QUIZZES_CACHE.lock().unwrap();
            quiz_file::open(path, &quizzes_cache(app)?)
        };
        let package = opened.inspect_err(|_| recent.forget_if_gone(&source_path))?;
        recent.remember(&source_path);
        start_show(app, package.quiz, Some(source_path), Some(package.root))
    })
}

/// Runs `start` unless another start holds `starting`, so a second click
/// does not unpack the quiz again while the first show begins: it is
/// refused with `ShowStarting`, which the Director does not report.
fn one_start(starting: &Mutex<()>, start: impl FnOnce() -> CmdResult<()>) -> CmdResult<()> {
    let _starting = match starting.try_lock() {
        Ok(guard) => guard,
        // A start that panicked leaves nothing half done that this one needs.
        Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(TryLockError::WouldBlock) => return Err(AppError::ShowStarting),
    };
    start()
}

/// Plays the editor's quiz without a journal, so the last real show stays
/// the one that can be resumed. It begins at the slide `from`, if given,
/// in the same step: a jump the editor sent once the show began would
/// carry the step its window saw in the show before, and be dropped
/// (`dispatch`).
#[tauri::command]
async fn editor_test_run(app: AppHandle, draft: String, quiz: Quiz, from: Option<usize>) -> CmdResult<()> {
    off_ui(move || {
        if let Some(slide) = from.filter(|&slide| slide >= quiz.slides.len()) {
            return Err(AppError::NoSuchSlide { slide });
        }
        // Held until the show serves the draft's media, which the editor then keeps.
        let _editor = editor::lock(&app);
        let root = editor::draft_root(&app, &draft)?;
        quiz_file::check_playable(&quiz, &root)?;
        let mut inner = lock_for_new_show(&app)?;
        inner.media_root = Some(root);
        begin_show(&app, &mut inner, Game::new(quiz), None)?;
        // A slide of the quiz, from the lobby: the jump is never refused.
        from.map_or(Ok(()), |slide| execute_at(&app, &mut inner, Command::GoTo { slide }, now_ms()).map(drop))
    })
    .await
}

#[tauri::command]
async fn start_demo_show(app: AppHandle) -> CmdResult<()> {
    off_ui(move || start_show(&app, Quiz::from_json(DEMO_QUIZ)?, None, None)).await
}

#[tauri::command]
async fn last_session(app: AppHandle) -> CmdResult<Option<SessionSummary>> {
    off_ui(move || Ok(Session::latest_summary(&sessions_root(&app)?)?)).await
}

/// What the quizmaster is told about a resumed show that runs: its media
/// are missing (deleted, say) or in a folder the windows may not load from
/// (`refused`), and it began under other rules.
fn resume_warnings(quiz: &Quiz, media_root: Option<&Path>, refused: Option<&Path>, rules_changed: bool) -> Vec<AppError> {
    let missing = media_root.and_then(|root| quiz_file::check_playable(quiz, root).err()).map(AppError::from);
    let refused = refused.map(|folder| AppError::MediaFolderRefused { folder: folder.display().to_string() });
    missing.into_iter().chain(refused).chain(rules_changed.then_some(AppError::RulesChanged)).collect()
}

/// The last show, rebuilt from its journal and ready to run.
struct ResumedShow {
    game: Game,
    session: Session,
    /// The quiz's media folder, if it is still there.
    media_root: Option<PathBuf>,
    /// What the recovery (`Game::recovery_command`) sends to the buzzers and windows.
    effects: Vec<Effect>,
    /// For the quizmaster (`resume_warnings`).
    warnings: Vec<AppError>,
}

/// Rebuilds the latest show under `root` from its journal. A show that
/// stopped mid-question has the question restarted, journaled at `at_ms`,
/// before anything is shown: no window sees the stopped question. Its
/// media are served only from the `media_folders`.
fn resume_show(root: &Path, at_ms: u64, media_folders: &[PathBuf]) -> CmdResult<ResumedShow> {
    let Resumed { mut session, quiz, journal, meta } = Session::resume_latest(root)?.ok_or(AppError::NoShowToResume)?;
    // A journal edited by hand, or one where an earlier version unpacked,
    // may name another folder.
    let (media_root, refused) = match meta.media_root.as_deref().map(PathBuf::from) {
        Some(named) if refused(&named, media_folders) => (None, Some(named)),
        named => (named, None),
    };
    // The show must go on: without its media it still runs, and the Director says why.
    let warnings = resume_warnings(&quiz, media_root.as_deref(), refused.as_deref(), meta.rules_changed());
    let mut game = Game::replay(quiz, &journal, meta.rules_version);
    let mut effects = Vec::new();
    if let Some(command) = game.recovery_command() {
        (game, effects) = apply_and_journal(&game, Some(&mut session), &Stamped { at_ms, command })?;
    }
    Ok(ResumedShow { game, session, media_root: media_root.filter(|root| root.is_dir()), effects, warnings })
}

/// Resumes the last show. Returns warnings for the quizmaster when the show
/// runs but not quite as before (`resume_warnings`).
#[tauri::command]
async fn resume_last_session(app: AppHandle) -> CmdResult<Vec<AppError>> {
    off_ui(move || {
        // Locked first: the running show's journal must not be reopened.
        let mut inner = lock_for_new_show(&app)?;
        let show = resume_show(&sessions_root(&app)?, now_ms(), &media_folders(&app)?)?;
        inner.media_root = show.media_root;
        replace_show(&mut inner, Some(show.game), Some(show.session));
        send_effects(&app, &mut inner, show.effects);
        show_changed(&app, &inner);
        Ok(show.warnings)
    })
    .await
}

/// Closes the running show. Its journal stays on disk, so it can be
/// resumed. In order with the show's commands, so a step given before it
/// still counts and none given after it reaches the show.
#[tauri::command]
async fn end_show(app: AppHandle) -> CmdResult<()> {
    show_thread::run(&app, |app, inner, at_ms| {
        if inner.before_show(at_ms) {
            return Ok(());
        }
        replace_show(inner, None, None);
        inner.media_root = None;
        show_changed(app, inner);
        Ok(())
    })
    .await
}

/// The commands the projector window may give: those its keys stand for
/// (`commandForKey`, whose tests check it against this list in the
/// bindings). Scores, names and jumps are the Director's alone.
const PROJECTOR_COMMANDS: [&str; 7] = ["press", "next", "judgeRight", "judgeWrong", "pauseClock", "resumeClock", "showLeaderboard"];

/// The buzzers the projector's keys press, 1 to 9 and 0 for 10, always
/// with the big button (`commandForKey`).
const KEY_BUZZERS: std::ops::RangeInclusive<quiz_core::BuzzerId> = 1..=10;

/// Whether the window labelled `window` may give `command`: the Director
/// any, the projector only what its keys stand for, and a window the app
/// does not know none.
fn may_give(window: &str, command: &Command) -> bool {
    match window {
        app_windows::DIRECTOR_LABEL => true,
        app_windows::HOST_LABEL => match command {
            Command::Press { buzzer, button } => *button == quiz_core::Button::Big && KEY_BUZZERS.contains(buzzer),
            command => {
                let tag = || serde_json::to_value(command).ok()?.get("type")?.as_str().map(str::to_owned);
                tag().is_some_and(|tag| PROJECTOR_COMMANDS.contains(&tag.as_str()))
            }
        },
        _ => false,
    }
}

/// A window's command that moves the show on was meant for the step it
/// showed (`seen`, `Game::step_no`), and the show has taken another since.
fn step_gone(game: Option<&Game>, seen: Option<u32>) -> bool {
    seen.is_some_and(|step| game.is_some_and(|game| game.step_no() != step))
}

/// Runs a window's `command`. One meant for a step the show has left
/// (`step_gone`) is dropped, as the window's step guard drops one sent too
/// soon, and the window is sent the show as it stands: one that missed a
/// view (it opened as the step was taken) catches up, so its next press counts.
#[tauri::command]
async fn dispatch(app: AppHandle, window: Window, command: Command, seen: Option<u32>) -> CmdResult<()> {
    if !may_give(window.label(), &command) {
        return Err(AppError::NotFromProjector);
    }
    if let Command::Press { buzzer, button } = command {
        return show_thread::press(&app, Press::new(buzzer, button));
    }
    show_thread::run(&app, move |app, inner, at_ms| {
        if step_gone(inner.game.as_ref(), seen) {
            emit_state(app, inner.game.as_ref());
            return Ok(());
        }
        match command {
            // The names a show is given are the prepared ones, also kept on disk.
            Command::SetTeams { names } => teams::set(app, inner, names, at_ms),
            command => execute_at(app, inner, command, at_ms).map(drop),
        }
    })
    .await
}

/// Feeds a press made at `at_ms`, on a buzzer, a key or an on-screen
/// buzzer, into the running show, if any. Returns whether the show took it.
/// Why one failed is the Director's to show (`save_problem`).
pub(crate) fn play_press(app: &AppHandle, press: Press, at_ms: u64) -> bool {
    let command = Command::Press { buzzer: press.buzzer, button: press.button };
    matches!(execute_at(app, &mut lock_show(app), command, at_ms), Ok(true))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    ai::serve_mcp_if_asked(&context.package_info().version.to_string(), &context.config().identifier);
    let builder = tauri::Builder::default()
        // First, so a second start ends before it touches the show: it brings
        // the running app's Director forward instead.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| app_windows::show_director(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(app_windows::navigation_guard());
    // Only macOS has an app menu, whose Quit asks during a show.
    #[cfg(target_os = "macos")]
    let builder = builder.menu(app_windows::app_menu);
    let inner = Inner::new();
    builder
        .on_window_event(app_windows::on_window_event)
        .on_menu_event(app_windows::on_menu_event)
        .manage(inner.close_request.clone())
        .manage(editor::EditorLock::default())
        .manage(AppState(Mutex::new(inner)))
        .setup(|app| {
            app_log::start(app.path().app_log_dir()?, &app.package_info().version.to_string());
            editor::move_roaming_drafts(app.handle());
            // One queue for the receiver's events and the quizmaster's commands.
            let (queue, events) = mpsc::channel();
            let hardware = Hardware::start(app.handle(), queue.clone(), *SHOW_CLOCK);
            if let Some(handle) = hardware.handle() {
                let mut inner = lock_show(app.handle());
                inner.driver = Box::new(HardwareDriver(handle));
                send_mode(&mut inner);
            }
            app.manage(hardware);
            app.manage(stage_lights::StageLights::start(app.handle(), stage_lights::lights_path(app.handle())?));
            app.manage(fixture_library::FixtureLibrary::new(fixture_library::library_path(app.handle())?));
            show_thread::start(app.handle(), queue, events);
            // The windows load theme pictures and fonts from the built-in
            // packages, and a show's media from where they can be.
            let packages = app.path().resource_dir()?.join(themes::RESOURCE_FOLDER);
            app.asset_protocol_scope().allow_directory(&packages, true)?;
            for folder in media_folders(app.handle())? {
                app.asset_protocol_scope().allow_directory(&folder, true)?;
            }
            let builtin = themes::builtin_themes(|id| folder_prefix(&packages.join(id)))?;
            app.manage(themes::Themes(builtin));
            if let Ok(path) = teams::teams_path(app.handle()) {
                let mut inner = lock_show(app.handle());
                let (teams, file) = teams::load(path);
                (inner.teams, inner.teams_file) = (teams, Some(file));
            }
            let library = media_player::MediaState::open(app_data(app.handle(), media_player::LIBRARY_FOLDER)?);
            app.asset_protocol_scope().allow_directory(library.root(), true)?;
            app.manage(library);
            app.manage(app_icon::AppIcon::load(app_data(app.handle(), app_icon::ICON_FILE)?));
            app.manage(recent_files::RecentFiles::load(app_local_data(app.handle(), recent_files::RECENT_FILES)?));
            for window in app.webview_windows().values() {
                app_icon::apply(app.handle(), window);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_view,
            get_quiz,
            media_root,
            show_not_saved,
            start_show_from_file,
            start_show_from_recent,
            start_demo_show,
            last_session,
            resume_last_session,
            end_show,
            dispatch,
            app_windows::list_monitors,
            app_windows::open_host_window,
            app_windows::close_host_window,
            app_windows::host_window_open,
            app_windows::report_host_problem,
            app_windows::quit_app,
            app_windows::close_declined,
            app_windows::log_problem,
            app_windows::set_menu_words,
            themes::list_themes,
            fonts::installed_fonts,
            teams::prepared_teams,
            teams::teams_lost,
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
            media_player::media_ended,
            media_player::media_volume,
            media_player::media_set_volume,
            app_icon::app_icon,
            app_icon::set_app_icon,
            editor::editor_load,
            editor::editor_new,
            editor::editor_open,
            recent_files::recent_files,
            editor::editor_update,
            editor::editor_save,
            editor::editor_add_media,
            editor::editor_category_name,
            editor::editor_import_sheet,
            editor::editor_export_sheet,
            editor::editor_close,
            editor_test_run,
            ai::ai_save_skill,
            ai::ai_connect_claude,
            hardware::receiver_info,
            hardware::receiver_devices,
            hardware::receiver_select,
            hardware::receiver_learn_start,
            hardware::receiver_learn_cancel,
            hardware::receiver_learn_finish,
            hardware::receiver_forget_learned,
            hardware::receiver_set_remote_action,
            hardware::receiver_reset_remote,
            hardware::receiver_export_log,
            hardware::receiver_udev_rule,
            stage_lights::lights_info,
            stage_lights::lights_store,
            stage_lights::lights_devices,
            fixture_library::lights_library,
            fixture_library::lights_library_info,
            fixture_library::lights_library_update,
            fixture_library::lights_library_reset,
            stage_lights::lights_hold,
            stage_lights::lights_blackout,
            stage_lights::lights_universe,
        ])
        .run(context)
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_windows::{CloseAnswer, CLOSE_ANSWER_WAIT};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Instant;

    #[test]
    fn a_second_start_meanwhile_is_refused_and_unpacks_nothing() {
        let (starting, unpacked) = (Mutex::new(()), AtomicUsize::new(0));
        let (begun, has_begun) = mpsc::channel();
        let (finish, may_finish) = mpsc::channel();
        std::thread::scope(|scope| {
            let (starting, unpacked) = (&starting, &unpacked);
            let first = scope.spawn(move || {
                one_start(starting, || {
                    unpacked.fetch_add(1, Ordering::SeqCst);
                    begun.send(()).unwrap();
                    may_finish.recv().unwrap();
                    Ok(())
                })
            });
            has_begun.recv().unwrap();
            let second = one_start(starting, || {
                unpacked.fetch_add(1, Ordering::SeqCst);
                Ok(())
            });
            assert_eq!(second, Err(AppError::ShowStarting));
            finish.send(()).unwrap();
            assert_eq!(first.join().unwrap(), Ok(()));
        });
        assert_eq!(unpacked.load(Ordering::SeqCst), 1, "only the first start unpacked");
        assert_eq!(one_start(&starting, || Ok(())), Ok(()), "the next start runs once the first is done");
    }

    #[test]
    fn a_start_runs_after_one_that_panicked() {
        let starting = Mutex::new(());
        let panicked = std::panic::catch_unwind(|| one_start(&starting, || panic!("the unpacking failed")));
        assert!(panicked.is_err());
        assert_eq!(one_start(&starting, || Ok(())), Ok(()));
    }

    #[test]
    fn the_demo_quiz_is_current_and_plays() {
        let quiz = Quiz::from_json(DEMO_QUIZ).unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(DEMO_QUIZ).unwrap()["schemaVersion"], quiz_core::SCHEMA_VERSION);
        assert_eq!(quiz.validate(), Ok(()));
    }

    /// The app's state with a show of the demo quiz journaled under `root`,
    /// given `commands` a second apart.
    fn journaled_show(root: &Path, commands: Vec<Command>) -> Inner {
        journaled_show_under(quiz_core::RULES_VERSION, root, commands)
    }

    /// `journaled_show` for a show begun under `rules_version`.
    fn journaled_show_under(rules_version: u32, root: &Path, commands: Vec<Command>) -> Inner {
        journaled_show_of(Quiz::from_json(DEMO_QUIZ).unwrap(), rules_version, root, commands)
    }

    /// `journaled_show_under` for a show of `quiz`.
    fn journaled_show_of(quiz: Quiz, rules_version: u32, root: &Path, commands: Vec<Command>) -> Inner {
        let meta = SessionMeta { quiz_title: quiz.title.clone(), source_path: None, started_ms: 1, media_root: None, rules_version };
        let session = Session::create(root, &quiz, &meta).unwrap();
        let game = Game::replay(quiz, &[], rules_version);
        let mut inner = Inner { game: Some(game), session: Some(session), ..Inner::new() };
        for (n, command) in commands.into_iter().enumerate() {
            apply_journaled(&mut inner, command, 1_000 * (n as u64 + 1)).unwrap();
        }
        inner
    }

    /// Runs a show as `journaled_show` does, then stops without closing
    /// anything, as a crash does. Gives the show as the windows last saw it.
    fn run_show_and_crash(root: &Path, commands: Vec<Command>) -> Game {
        journaled_show(root, commands).game.unwrap()
    }

    fn journal_len(root: &Path) -> usize {
        Session::resume_latest(root).unwrap().unwrap().journal.len()
    }

    fn sessions_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("resume-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// The demo show up to its first question, with two players and a Czech name.
    fn up_to_the_first_question() -> Vec<Command> {
        vec![
            Command::Press { buzzer: 1, button: quiz_core::Button::Big },
            Command::AddPlayer { buzzer: 2, name: "Žluťoučký kůň".into() },
            Command::Next, // the billboard
            Command::Next, // the question
            Command::Next, // its clock
            Command::Press { buzzer: 1, button: quiz_core::Button::A },
        ]
    }

    #[test]
    fn media_are_served_only_from_the_apps_own_folders() {
        let base = sessions_dir("served");
        let (cache, drafts, elsewhere) = (base.join("quizzes"), base.join("editor"), base.join("home"));
        for folder in [cache.join("abc"), drafts.join("draft-1"), elsewhere.clone()] {
            fs::create_dir_all(folder).unwrap();
        }
        let folders = [cache.clone(), drafts.clone()];
        assert!(!refused(&cache.join("abc"), &folders));
        assert!(!refused(&drafts.join("draft-1"), &folders));
        assert!(refused(&elsewhere, &folders));
        assert!(refused(&cache.join("abc/../../home"), &folders), "no way out with ..");
        assert!(!refused(&cache.join("gone"), &folders), "a folder that is gone is missing, not refused");
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn a_show_resumes_after_a_crash_as_the_windows_last_saw_it() {
        let root = sessions_dir("same");
        let mut commands = up_to_the_first_question();
        commands.extend([Command::Press { buzzer: 2, button: quiz_core::Button::C }, Command::Next]);
        let before = run_show_and_crash(&root, commands);
        assert!(matches!(before.phase(), quiz_core::Phase::Question(quiz_core::QuestionStep::Revealed { .. })), "{:?}", before.phase());

        let resumed = resume_show(&root, 60_000, &[]).unwrap();
        assert_eq!(resumed.game, before);
        assert!(resumed.effects.is_empty());
        assert_eq!(resumed.warnings, []);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_question_a_crash_stopped_restarts_before_any_window_sees_it() {
        let root = sessions_dir("mid-question");
        let before = run_show_and_crash(&root, up_to_the_first_question());
        let slide = before.slide_index();
        assert_eq!(before.recovery_command(), Some(Command::RestartQuestion), "{:?}", before.phase());

        let resumed = resume_show(&root, 60_000, &[]).unwrap();
        // What the windows will see is on disk: the restart is journaled.
        let on_disk = Session::resume_latest(&root).unwrap().unwrap();
        assert_eq!(on_disk.journal.last(), Some(&Stamped { at_ms: 60_000, command: Command::RestartQuestion }));
        assert_eq!(Game::replay(on_disk.quiz, &on_disk.journal, quiz_core::RULES_VERSION), resumed.game);
        assert_eq!(resumed.game.slide_index(), slide);
        assert_ne!(resumed.game, before, "the question started again");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_step_command_meant_for_a_step_the_show_left_is_dropped() {
        let quiz = Quiz::from_json(DEMO_QUIZ).unwrap();
        let journal: Vec<Stamped> = up_to_the_first_question().into_iter().map(|command| Stamped { at_ms: 1_000, command }).collect();
        let game = Game::replay(quiz, &journal, quiz_core::RULES_VERSION);
        let now = game.step_no();
        assert!(!step_gone(Some(&game), Some(now)));
        assert!(step_gone(Some(&game), Some(now - 1)), "meant for the step before");
        assert!(!step_gone(Some(&game), None), "a command that does not move the show on");
        assert!(!step_gone(None, Some(now)), "no show: the command is refused as before");
    }

    #[test]
    fn a_show_that_stopped_during_the_wait_resumes_holding_the_step() {
        let root = sessions_dir("wait");
        let mut quiz = Quiz::from_json(DEMO_QUIZ).unwrap();
        quiz.settings = quiz_core::QuizSettings { all_answered: quiz_core::AllAnswered::NextSlide, ..Default::default() };
        let mut commands = up_to_the_first_question();
        commands.push(Command::Press { buzzer: 2, button: quiz_core::Button::C });
        let before = journaled_show_of(quiz, quiz_core::RULES_VERSION, &root, commands).game.unwrap();
        assert!(before.view(7_000).auto_step.is_some(), "the step was on its way");

        let resumed = resume_show(&root, 60_000, &[]).unwrap();
        // The question waits for the quizmaster, and the hold is on disk.
        let on_disk = Session::resume_latest(&root).unwrap().unwrap();
        assert_eq!(on_disk.journal.last(), Some(&Stamped { at_ms: 60_000, command: Command::PauseClock }));
        assert_eq!(Game::replay(on_disk.quiz, &on_disk.journal, quiz_core::RULES_VERSION), resumed.game);
        assert_eq!((resumed.game.slide_index(), resumed.game.phase()), (before.slide_index(), before.phase()));
        assert_eq!(resumed.game.due_command(60_000), None);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_show_begun_under_rules_version_1_resumes_with_its_restart_by_go_to() {
        let root = sessions_dir("rules-1");
        let mut inner = journaled_show_under(1, &root, up_to_the_first_question());
        let slide = inner.game.as_ref().unwrap().slide_index();
        // Version 1 restarted the question after a crash with a jump to it;
        // today's rules refuse that jump, so it is written as it was.
        let session = inner.session.as_mut().unwrap();
        session.append(&Stamped { at_ms: 60_000, command: Command::GoTo { slide } }).unwrap();
        session.append(&Stamped { at_ms: 61_000, command: Command::Next }).unwrap();
        session.append(&Stamped { at_ms: 62_000, command: Command::Press { buzzer: 2, button: quiz_core::Button::C } }).unwrap();
        drop(inner);

        let resumed = resume_show(&root, 120_000, &[]).unwrap();
        assert_eq!(resumed.warnings, [AppError::RulesChanged]);
        // Replayed with its jump, the question ran again, so it is restarted once more.
        let on_disk = Session::resume_latest(&root).unwrap().unwrap();
        assert_eq!(on_disk.journal.last(), Some(&Stamped { at_ms: 120_000, command: Command::RestartQuestion }));
        assert_eq!(Game::replay(on_disk.quiz, &on_disk.journal, on_disk.meta.rules_version), resumed.game);
        assert_eq!(resumed.game.slide_index(), slide);
        assert_eq!(resumed.game.phase(), &quiz_core::Phase::Question(quiz_core::QuestionStep::Ready));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_show_begun_under_rules_version_2_keeps_its_rules_across_resumes() {
        let root = sessions_dir("rules-2");
        drop(journaled_show_under(2, &root, up_to_the_first_question()));
        let resumed = resume_show(&root, 60_000, &[]).unwrap();
        assert_eq!(resumed.warnings, [AppError::RulesChanged]);
        let mut inner = Inner { game: Some(resumed.game), session: Some(resumed.session), ..Inner::new() };
        apply_journaled(&mut inner, Command::Next, 61_000).unwrap(); // the restarted question's clock
        apply_journaled(&mut inner, Command::Press { buzzer: 1, button: quiz_core::Button::A }, 62_000).unwrap();
        // The last player still to answer is switched off: today that closes
        // the question, under version 2 the quizmaster closes it with Next.
        apply_journaled(&mut inner, Command::SetPlayerEnabled { buzzer: 2, enabled: false }, 63_000).unwrap();
        let game = inner.game.as_ref().unwrap();
        assert!(matches!(game.phase(), quiz_core::Phase::Question(quiz_core::QuestionStep::Running { .. })), "{:?}", game.phase());
        apply_journaled(&mut inner, Command::Next, 64_000).unwrap();
        apply_journaled(&mut inner, Command::Next, 65_000).unwrap();
        let live = inner.game.take().unwrap();
        drop(inner);

        let again = resume_show(&root, 120_000, &[]).unwrap();
        assert_eq!(again.game, live, "the second resume brings back the show the windows saw");
        assert!(again.effects.is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_resumed_show_plays_on_when_the_new_runs_clock_reads_earlier() {
        // The system clock went back between the runs, so the new run's show
        // clock reads before the journal's last command (6 s).
        let root = sessions_dir("clock-back");
        run_show_and_crash(&root, up_to_the_first_question());
        let resumed = resume_show(&root, 500, &[]).unwrap();
        let mut inner = Inner { game: Some(resumed.game), session: Some(resumed.session), ..Inner::new() };
        apply_journaled(&mut inner, Command::Next, 600).unwrap(); // the restarted question's clock
        let press = Command::Press { buzzer: 2, button: quiz_core::Button::C };
        assert!(apply_journaled(&mut inner, press, 700).unwrap().is_some(), "the press counts");
        let game = inner.game.as_ref().unwrap();
        assert_eq!(game.responses().keys().collect::<Vec<_>>(), [&2]);
        // Time is up 20 s after the new start, not after the old one.
        assert_eq!(game.due_command(20_000), None);
        assert_eq!(game.due_command(21_000), Some(Command::CloseQuestion));
        // The journal, with both runs' times, replays into the same show.
        let on_disk = Session::resume_latest(&root).unwrap().unwrap();
        assert_eq!(&Game::replay(on_disk.quiz, &on_disk.journal, quiz_core::RULES_VERSION), game);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn only_presses_that_change_the_show_are_journaled() {
        let root = sessions_dir("ignored");
        let mut inner = journaled_show(&root, up_to_the_first_question());
        let before = journal_len(&root);
        // Buzzer 1 has answered: pressing again changes nothing.
        let again = Command::Press { buzzer: 1, button: quiz_core::Button::B };
        assert_eq!(apply_journaled(&mut inner, again, 7_000), Ok(None));
        assert_eq!(journal_len(&root), before);
        // A buzzer the show does not know yet registers.
        let new = Command::Press { buzzer: 9, button: quiz_core::Button::B };
        assert!(apply_journaled(&mut inner, new, 7_100).unwrap().is_some());
        assert_eq!(journal_len(&root), before + 1);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_failed_journal_write_is_kept_for_the_director_until_one_works() {
        let root = sessions_dir("not-saved");
        let mut inner = journaled_show(&root, vec![Command::Press { buzzer: 1, button: quiz_core::Button::Big }]);
        let name = |n: u16| Command::AddPlayer { buzzer: n, name: format!("Team {n}") };
        assert_eq!(inner.save_problem, None);
        inner.session.as_mut().unwrap().break_for_test();
        assert_eq!(apply_journaled(&mut inner, name(2), 2_000), Err(AppError::JournalBroken));
        assert_eq!(inner.save_problem, Some(AppError::JournalBroken));
        // A command the show refuses says nothing of the disk.
        assert_eq!(apply_journaled(&mut inner, Command::JudgeRight, 3_000), Err(AppError::NotNow));
        assert_eq!(inner.save_problem, Some(AppError::JournalBroken));
        inner.session.as_mut().unwrap().repair_for_test();
        assert!(apply_journaled(&mut inner, name(2), 4_000).is_ok());
        assert_eq!(inner.save_problem, None);

        // Another show starts with a journal that has not failed.
        inner.session.as_mut().unwrap().break_for_test();
        assert!(apply_journaled(&mut inner, name(3), 5_000).is_err());
        replace_show(&mut inner, None, None);
        assert_eq!(inner.save_problem, None);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_projector_may_give_only_what_its_keys_stand_for() {
        let press = Command::Press { buzzer: 3, button: quiz_core::Button::Big };
        let keys = [press, Command::Next, Command::JudgeRight, Command::JudgeWrong, Command::PauseClock, Command::ResumeClock, Command::ShowLeaderboard { visible: true }];
        for command in &keys {
            assert!(may_give(app_windows::HOST_LABEL, command), "{command:?}");
        }
        let directors = [
            Command::AdjustScore { buzzer: 3, delta: 100 },
            Command::SetTeams { names: vec!["A".into()] },
            Command::GoTo { slide: 2 },
            Command::GoToFinal,
            Command::StartOver,
            Command::ResetScores,
            Command::AddPlayer { buzzer: 3, name: "A".into() },
            Command::RenamePlayer { buzzer: 3, name: "A".into() },
            Command::PairTeam { buzzer: 3, name: "A".into() },
            Command::SetPlayerEnabled { buzzer: 3, enabled: false },
            Command::CloseQuestion,
            Command::RestartQuestion,
            Command::PlayVideo,
        ];
        for command in &directors {
            assert!(!may_give(app_windows::HOST_LABEL, command), "{command:?}");
        }
        for (buzzer, button) in [(11, quiz_core::Button::Big), (0, quiz_core::Button::Big), (3, quiz_core::Button::A)] {
            assert!(!may_give(app_windows::HOST_LABEL, &Command::Press { buzzer, button }), "a press no key gives: {buzzer} {button:?}");
        }
        for command in keys.iter().chain(&directors) {
            assert!(may_give(app_windows::DIRECTOR_LABEL, command), "{command:?}");
            assert!(!may_give("editor", command), "a window the app does not know: {command:?}");
        }
    }

    /// For the tests of the projector's keys: the commands typed as command
    /// tags, so a tag that is not one does not compile, and the buzzers its
    /// keys may press.
    #[test]
    fn export_projector_commands() {
        let Ok(dir) = std::env::var("TS_RS_EXPORT_DIR") else { return };
        let ts = format!(
            "// Generated by `cargo test` from PROJECTOR_COMMANDS and KEY_BUZZERS in src-tauri/src/lib.rs. Do not edit this file manually.\n\n\
             import type {{ Command }} from \"./Command\";\n\n\
             /** The commands the projector window may give. */\n\
             export const PROJECTOR_COMMANDS: readonly Command[\"type\"][] = {};\n\n\
             /** The buzzers the projector's keys may press, always with the big button. */\n\
             export const KEY_BUZZERS: readonly number[] = {};\n",
            serde_json::to_string(&PROJECTOR_COMMANDS).unwrap(),
            serde_json::to_string(&KEY_BUZZERS.collect::<Vec<_>>()).unwrap(),
        );
        fs::write(Path::new(&dir).join("ProjectorCommands.ts"), ts).unwrap();
    }

    #[test]
    fn a_close_request_left_unanswered_is_forgotten_when_a_show_begins_or_ends() {
        let mut inner = Inner::new();
        let request = inner.close_request.clone();
        let now = Instant::now();
        let demo = || Game::new(Quiz::from_json(DEMO_QUIZ).unwrap());
        assert_eq!(request.answer(now), CloseAnswer::Ask);
        assert_eq!(request.answer(now), CloseAnswer::Wait);
        replace_show(&mut inner, Some(demo()), None);
        assert_eq!(request.answer(now), CloseAnswer::Ask, "a show began");
        assert_eq!(request.answer(now + CLOSE_ANSWER_WAIT), CloseAnswer::Close);
        replace_show(&mut inner, None, None);
        assert_eq!(request.answer(now + CLOSE_ANSWER_WAIT), CloseAnswer::Ask, "the show ended");
        // The show resumed after a crash begins here too (`resume_last_session`).
        replace_show(&mut inner, Some(demo()), None);
        assert_eq!(request.answer(now + CLOSE_ANSWER_WAIT), CloseAnswer::Ask, "a show resumed");
    }

    /// Notes what the buzzers are told, in order.
    struct Recording(Arc<Mutex<Vec<String>>>);

    impl BuzzerDriver for Recording {
        fn name(&self) -> &'static str {
            "recording"
        }

        fn has_lights(&self) -> bool {
            true
        }

        fn set_lights(&mut self, lights: &[(BuzzerId, Light)]) -> Result<(), buzzers::DriverError> {
            self.0.lock().unwrap().push(format!("lights {lights:?}"));
            Ok(())
        }

        fn reset_lights(&mut self, lights: &[(BuzzerId, Light)]) -> Result<(), buzzers::DriverError> {
            self.0.lock().unwrap().push(format!("lights {lights:?}, others dark"));
            Ok(())
        }

        fn set_mode(&mut self, mode: BuzzerMode) {
            self.0.lock().unwrap().push(format!("mode {mode:?}"));
        }
    }

    #[test]
    fn the_buzzers_hear_the_mode_before_the_lights() {
        let told = Arc::new(Mutex::new(Vec::new()));
        let mut inner = Inner { driver: Box::new(Recording(told.clone())), ..Inner::new() };
        let quiz = Quiz::from_json(DEMO_QUIZ).unwrap();
        let fastest_finger = quiz
            .slides
            .iter()
            .position(|slide| matches!(&slide.content, quiz_core::SlideContent::Question(q) if q.mode == quiz_core::AnswerMode::FastestFinger))
            .unwrap();
        replace_show(&mut inner, Some(Game::new(quiz)), None);
        // Whatever another program left on them, the buzzers go dark.
        assert_eq!(*told.lock().unwrap(), ["mode Register", "lights [], others dark"]);
        let mut step = |command, at_ms| {
            told.lock().unwrap().clear();
            let effects = apply_journaled(&mut inner, command, at_ms).unwrap().unwrap();
            light_buzzers(&mut inner, &effects);
            told.lock().unwrap().clone()
        };
        assert_eq!(step(Command::Press { buzzer: 3, button: quiz_core::Button::Big }, 1_000), ["mode Register", "lights [(3, Ready)]"]);
        step(Command::GoTo { slide: fastest_finger }, 2_000);
        // A fastest-finger clock starts: the buzzers take presses as buzzes before they turn ready.
        assert_eq!(step(Command::Next, 3_000), ["mode FastestFinger", "lights [(3, Ready)]"]);
        told.lock().unwrap().clear();
        replace_show(&mut inner, None, None);
        assert_eq!(*told.lock().unwrap(), ["mode Register", "lights [], others dark"], "the show ended");
    }

    #[test]
    fn what_was_given_before_a_show_began_does_not_reach_it() {
        let mut inner = Inner::new();
        let given = now_ms();
        std::thread::sleep(std::time::Duration::from_millis(5));
        replace_show(&mut inner, Some(Game::new(Quiz::from_json(DEMO_QUIZ).unwrap())), None);
        assert!(inner.before_show(given));
        assert!(!inner.before_show(now_ms()));
    }

    #[test]
    fn a_resumed_show_warns_of_missing_media_and_of_other_rules_both() {
        let mut quiz = Quiz::from_json(DEMO_QUIZ).unwrap();
        let question = quiz.slides.iter_mut().find(|slide| matches!(slide.content, quiz_core::SlideContent::Question(_))).unwrap();
        question.picture = Some("media/gone.png".into());
        let empty = std::env::temp_dir().join(format!("resume-warnings-{}", std::process::id()));
        std::fs::create_dir_all(&empty).unwrap();
        let warnings = resume_warnings(&quiz, Some(&empty), None, true);
        assert!(matches!(warnings[..], [AppError::QuizInvalid { .. }, AppError::RulesChanged]), "{warnings:?}");
        assert_eq!(resume_warnings(&quiz, None, None, false), []);
        let warnings = resume_warnings(&quiz, None, Some(&empty), false);
        assert!(matches!(warnings[..], [AppError::MediaFolderRefused { .. }]), "{warnings:?}");
        std::fs::remove_dir_all(&empty).unwrap();
    }
}
