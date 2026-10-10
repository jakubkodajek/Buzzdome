//! The team names prepared for shows (spec SF-5). They are kept in the app's
//! data folder, so they can be typed before a show starts and outlive it:
//! ending a show, opening another quiz or a test run keeps them. A show
//! starts with them as its first journaled `SetTeams`, and a change during
//! a show reaches the show (journaled first, S1) and then the file. The
//! rules work on `Inner` and a path, so they are tested without windows.

use crate::app_file::{self, SettingsLost};
use crate::error::CmdResult;
use crate::session::Session;
use crate::{app_data, apply_journaled, deliver, now_ms, timed_lock, AppState, Inner};
use quiz_core::{game::same_name, team_names, Command, Effect, Game, MAX_TEAMS};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};

const TEAMS_FILE: &str = "teams.json";

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeamsFile {
    #[serde(default)]
    names: Vec<String>,
}

pub fn teams_path(app: &AppHandle) -> CmdResult<PathBuf> {
    app_data(app, TEAMS_FILE)
}

/// The team names saved at `path`, as a show takes them (`team_names`). A
/// file that cannot be read is kept aside and the app starts without names;
/// why is shown in the Teams tab.
pub fn load(path: &Path) -> (Vec<String>, Option<SettingsLost>) {
    let (file, lost) = app_file::load::<TeamsFile>(path);
    // A file edited by hand may hold more names than a show takes: the first ones count.
    (team_names(file.names.iter().take(MAX_TEAMS).map(String::as_str)).unwrap_or_default(), lost)
}

/// The team names prepared for the next show, or the running show's.
pub fn current(inner: &Inner) -> &[String] {
    inner.game.as_ref().map_or(&inner.teams, |game| game.teams())
}

/// Makes `game` the running show, journaled in `session`, and gives it the
/// prepared names as its first command. On an error the show runs without
/// them, as its journal refused them.
pub fn begin(inner: &mut Inner, game: Game, session: Option<Session>, at_ms: u64) -> CmdResult<Vec<Effect>> {
    inner.game = Some(game);
    inner.session = session;
    if inner.teams.is_empty() {
        return Ok(Vec::new());
    }
    apply_journaled(inner, Command::SetTeams { names: inner.teams.clone() }, at_ms)
}

/// Makes `teams` the prepared names: in the running show first (journaled;
/// when that fails nothing changes), then in `inner` and in the file at
/// `path`. Gives the show's effects and whether the file was written.
fn change(inner: &mut Inner, teams: Vec<String>, at_ms: u64, path: &Path) -> CmdResult<(Vec<Effect>, CmdResult<()>)> {
    let effects = match inner.game {
        Some(_) => apply_journaled(inner, Command::SetTeams { names: teams.clone() }, at_ms)?,
        None => Vec::new(),
    };
    inner.teams = teams;
    Ok((effects, app_file::store(path, &TeamsFile { names: inner.teams.clone() })))
}

/// `change`, then tells the buzzers and the windows.
fn change_and_tell(app: &AppHandle, inner: &mut Inner, teams: Vec<String>, at_ms: u64) -> CmdResult<()> {
    let (effects, stored) = change(inner, teams, at_ms, &teams_path(app)?)?;
    if inner.game.is_some() {
        deliver(app, inner, effects);
    }
    let _ = app.emit("teams-changed", current(inner));
    stored
}

/// Each edit starts from the names as they are now, so two quick edits both count.
fn added(inner: &Inner, names: &[String]) -> CmdResult<Vec<String>> {
    Ok(team_names(current(inner).iter().chain(names).map(String::as_str))?)
}

fn removed(inner: &Inner, name: &str) -> Vec<String> {
    current(inner).iter().filter(|team| !same_name(team, name)).cloned().collect()
}

/// Replaces the prepared names, as `Command::SetTeams` from the windows or a plugin.
pub fn set(app: &AppHandle, inner: &mut Inner, names: Vec<String>, at_ms: u64) -> CmdResult<()> {
    let teams = team_names(names.iter().map(String::as_str))?;
    change_and_tell(app, inner, teams, at_ms)
}

/// The team names prepared for shows.
#[tauri::command]
pub fn prepared_teams(state: State<AppState>) -> Vec<String> {
    current(&state.0.lock().unwrap()).to_vec()
}

/// Why the saved team names could not be read when the app started, if so.
#[tauri::command]
pub fn teams_lost(state: State<AppState>) -> Option<SettingsLost> {
    state.0.lock().unwrap().teams_lost.clone()
}

/// Adds team names after the prepared ones; names already there are kept once.
#[tauri::command]
pub fn add_teams(app: AppHandle, state: State<AppState>, names: Vec<String>) -> CmdResult<()> {
    let (mut inner, at_ms) = timed_lock(&state.0, now_ms);
    let teams = added(&inner, &names)?;
    change_and_tell(&app, &mut inner, teams, at_ms)
}

/// Removes a prepared team name. A buzzer paired with it keeps its player's name.
#[tauri::command]
pub fn remove_team(app: AppHandle, state: State<AppState>, name: String) -> CmdResult<()> {
    let (mut inner, at_ms) = timed_lock(&state.0, now_ms);
    let teams = removed(&inner, &name);
    change_and_tell(&app, &mut inner, teams, at_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionMeta;
    use buzzers::VirtualDriver;
    use quiz_core::Quiz;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("teams-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn inner() -> Inner {
        Inner { game: None, session: None, media_root: None, driver: Box::new(VirtualDriver), teams: Vec::new(), teams_lost: None }
    }

    fn session(root: &Path) -> Session {
        let meta = SessionMeta { quiz_title: "Quiz".into(), source_path: None, started_ms: 1, media_root: None, rules_version: quiz_core::RULES_VERSION };
        Session::create(root, &Quiz::new("Quiz", Vec::new()), &meta).unwrap()
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| name.to_string()).collect()
    }

    fn journal(root: &Path) -> Vec<Command> {
        Session::resume_latest(root).unwrap().unwrap().journal.into_iter().map(|stamped| stamped.command).collect()
    }

    #[test]
    fn saved_names_read_back_as_a_show_takes_them() {
        let dir = temp_dir("load");
        let path = dir.join(TEAMS_FILE);
        assert_eq!(load(&path), (Vec::new(), None));
        let mut names: Vec<String> = vec![" Hawks ".into(), "hawks".into(), String::new()];
        names.extend((0..MAX_TEAMS + 5).map(|i| format!("Team {i}")));
        app_file::store(&path, &TeamsFile { names }).unwrap();
        let (teams, lost) = load(&path);
        assert_eq!(teams.len(), MAX_TEAMS - 2, "the first {MAX_TEAMS} lines, without the repeat and the blank");
        assert_eq!(teams[..2], ["Hawks".to_string(), "Team 0".to_string()]);
        assert_eq!(lost, None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn two_quick_edits_both_count_and_reach_the_file() {
        let dir = temp_dir("edits");
        let path = dir.join(TEAMS_FILE);
        let mut inner = inner();
        let teams = added(&inner, &names(&["Hawks"])).unwrap();
        change(&mut inner, teams, 0, &path).unwrap().1.unwrap();
        let teams = added(&inner, &names(&["Owls", "hawks"])).unwrap();
        change(&mut inner, teams, 0, &path).unwrap().1.unwrap();
        assert_eq!(inner.teams, names(&["Hawks", "Owls"]));
        let teams = removed(&inner, "HAWKS");
        change(&mut inner, teams, 0, &path).unwrap().1.unwrap();
        assert_eq!(load(&path).0, names(&["Owls"]));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_change_in_a_show_is_journaled_before_it_is_kept() {
        let dir = temp_dir("show");
        let (path, root) = (dir.join(TEAMS_FILE), dir.join("sessions"));
        let mut inner = inner();
        begin(&mut inner, Game::new(Quiz::new("Quiz", Vec::new())), Some(session(&root)), 0).unwrap();
        change(&mut inner, names(&["Hawks"]), 5, &path).unwrap().1.unwrap();
        assert_eq!(current(&inner), names(&["Hawks"]));
        assert_eq!(journal(&root), [Command::SetTeams { names: names(&["Hawks"]) }]);

        // A change the journal refuses changes nothing, on disk neither.
        inner.session.as_mut().unwrap().break_for_test();
        assert!(change(&mut inner, names(&["Owls"]), 6, &path).is_err());
        assert_eq!((current(&inner), &inner.teams[..]), (&names(&["Hawks"])[..], &names(&["Hawks"])[..]));
        assert_eq!(load(&path).0, names(&["Hawks"]));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_show_starts_with_the_prepared_names_as_its_first_command() {
        let dir = temp_dir("begin");
        let root = dir.join("sessions");
        let mut inner = inner();
        inner.teams = names(&["Hawks", "Owls"]);
        begin(&mut inner, Game::new(Quiz::new("Quiz", Vec::new())), Some(session(&root)), 0).unwrap();
        assert_eq!(current(&inner), inner.teams);
        assert_eq!(journal(&root), [Command::SetTeams { names: names(&["Hawks", "Owls"]) }]);

        // When the journal refuses them, the show runs without them.
        let mut broken = session(&dir.join("other"));
        broken.break_for_test();
        let mut inner = Inner { game: None, ..inner };
        assert!(begin(&mut inner, Game::new(Quiz::new("Quiz", Vec::new())), Some(broken), 0).is_err());
        assert!(inner.game.is_some() && current(&inner).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
