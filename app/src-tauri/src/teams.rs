//! The team names prepared for shows (spec SF-5). They are kept in the app's
//! data folder, so they can be typed before a show starts and outlive it:
//! ending a show, opening another quiz or a test run keeps them. A show
//! starts with them as its first journaled `SetTeams`, and a change during
//! a show reaches the show (journaled first, S1) and then the file. The
//! rules work on `Inner` and a path, so they are tested without windows.

use crate::app_file::{AppFile, SettingsLost};
use crate::error::{AppError, CmdResult};
use crate::session::Session;
use crate::{app_data, apply_journaled, deliver, read_show, replace_show, show_thread, Inner};
use quiz_core::{game::same_name, team_names, Command, Effect, Game, MAX_TEAMS};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

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

/// The team names saved at `path`, as a show takes them (`team_names`), and
/// the file to keep them in. A file that cannot be read is kept aside and
/// the app starts without names; why is shown in the Teams tab.
pub fn load(path: PathBuf) -> (Vec<String>, AppFile) {
    let (file, saved) = AppFile::load::<TeamsFile>(path);
    // A file edited by hand may hold more names than a show takes: the first ones count.
    (team_names(saved.names.iter().take(MAX_TEAMS).map(String::as_str)).unwrap_or_default(), file)
}

/// The team names prepared for the next show, or the running show's.
pub fn current(inner: &Inner) -> &[String] {
    inner.game.as_ref().map_or(&inner.teams, |game| game.teams())
}

/// Makes `game` the running show, journaled in `session`, and gives it the
/// prepared names as its first command. On an error the show runs without
/// them, as its journal refused them.
pub fn begin(inner: &mut Inner, game: Game, session: Option<Session>, at_ms: u64) -> CmdResult<Vec<Effect>> {
    replace_show(inner, Some(game), session);
    if inner.teams.is_empty() {
        return Ok(Vec::new());
    }
    Ok(apply_journaled(inner, Command::SetTeams { names: inner.teams.clone() }, at_ms)?.unwrap_or_default())
}

/// Makes `teams` the prepared names: in the running show first (journaled;
/// when that fails nothing changes), then in `inner` and in its teams file.
/// Gives the show's effects and whether the file was written.
fn change(inner: &mut Inner, teams: Vec<String>, at_ms: u64) -> CmdResult<(Vec<Effect>, CmdResult<()>)> {
    let effects = match inner.game {
        Some(_) => apply_journaled(inner, Command::SetTeams { names: teams.clone() }, at_ms)?.unwrap_or_default(),
        None => Vec::new(),
    };
    inner.teams = teams;
    let file = TeamsFile { names: inner.teams.clone() };
    // The file is known once the app has started, before any command.
    let stored = inner.teams_file.as_ref().map_or(Err(AppError::NoDataFolder), |kept| kept.store(&file));
    Ok((effects, stored))
}

/// `change`, then tells the buzzers and the windows.
fn change_and_tell(app: &AppHandle, inner: &mut Inner, teams: Vec<String>, at_ms: u64) -> CmdResult<()> {
    let (effects, stored) = change(inner, teams, at_ms)?;
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

// Like the show's commands, these run off the windows' thread, and the
// edits on the show thread, in order with the show's commands.

/// The team names prepared for shows.
#[tauri::command]
pub async fn prepared_teams(app: AppHandle) -> CmdResult<Vec<String>> {
    read_show(app, |inner| current(inner).to_vec()).await
}

/// Why the saved team names could not be read when the app started, if so.
#[tauri::command]
pub async fn teams_lost(app: AppHandle) -> CmdResult<Option<SettingsLost>> {
    read_show(app, |inner| inner.teams_file.as_ref().and_then(AppFile::lost).cloned()).await
}

/// Adds team names after the prepared ones; names already there are kept once.
#[tauri::command]
pub async fn add_teams(app: AppHandle, names: Vec<String>) -> CmdResult<()> {
    show_thread::run(&app, move |app, inner, at_ms| {
        let teams = added(inner, &names)?;
        change_and_tell(app, inner, teams, at_ms)
    })
    .await
}

/// Removes a prepared team name. A buzzer paired with it keeps its player's name.
#[tauri::command]
pub async fn remove_team(app: AppHandle, name: String) -> CmdResult<()> {
    show_thread::run(&app, move |app, inner, at_ms| {
        let teams = removed(inner, &name);
        change_and_tell(app, inner, teams, at_ms)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionMeta;
    use quiz_core::Quiz;
    use std::path::Path;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("teams-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// The app's state with no names, keeping them in `path`.
    fn inner(path: &Path) -> Inner {
        let teams_file = Some(load(path.to_path_buf()).1);
        Inner { teams_file, ..Inner::new() }
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
        let (teams, file) = load(path.clone());
        assert_eq!((teams, file.lost()), (Vec::new(), None));
        let mut names: Vec<String> = vec![" Hawks ".into(), "hawks".into(), String::new()];
        names.extend((0..MAX_TEAMS + 5).map(|i| format!("Team {i}")));
        file.store(&TeamsFile { names }).unwrap();
        let (teams, file) = load(path);
        assert_eq!(teams.len(), MAX_TEAMS - 2, "the first {MAX_TEAMS} lines, without the repeat and the blank");
        assert_eq!(teams[..2], ["Hawks".to_string(), "Team 0".to_string()]);
        assert_eq!(file.lost(), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn two_quick_edits_both_count_and_reach_the_file() {
        let dir = temp_dir("edits");
        let path = dir.join(TEAMS_FILE);
        let mut inner = inner(&path);
        let teams = added(&inner, &names(&["Hawks"])).unwrap();
        change(&mut inner, teams, 0).unwrap().1.unwrap();
        let teams = added(&inner, &names(&["Owls", "hawks"])).unwrap();
        change(&mut inner, teams, 0).unwrap().1.unwrap();
        assert_eq!(inner.teams, names(&["Hawks", "Owls"]));
        let teams = removed(&inner, "HAWKS");
        change(&mut inner, teams, 0).unwrap().1.unwrap();
        assert_eq!(load(path.clone()).0, names(&["Owls"]));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_change_in_a_show_is_journaled_before_it_is_kept() {
        let dir = temp_dir("show");
        let (path, root) = (dir.join(TEAMS_FILE), dir.join("sessions"));
        let mut inner = inner(&path);
        begin(&mut inner, Game::new(Quiz::new("Quiz", Vec::new())), Some(session(&root)), 0).unwrap();
        change(&mut inner, names(&["Hawks"]), 5).unwrap().1.unwrap();
        assert_eq!(current(&inner), names(&["Hawks"]));
        assert_eq!(journal(&root), [Command::SetTeams { names: names(&["Hawks"]) }]);

        // A change the journal refuses changes nothing, on disk neither.
        inner.session.as_mut().unwrap().break_for_test();
        assert!(change(&mut inner, names(&["Owls"]), 6).is_err());
        assert_eq!((current(&inner), &inner.teams[..]), (&names(&["Hawks"])[..], &names(&["Hawks"])[..]));
        assert_eq!(load(path.clone()).0, names(&["Hawks"]));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_show_starts_with_the_prepared_names_as_its_first_command() {
        let dir = temp_dir("begin");
        let root = dir.join("sessions");
        let mut inner = inner(&dir.join(TEAMS_FILE));
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
