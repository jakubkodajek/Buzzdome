//! The team names prepared for shows (spec SF-5). They are kept in the app's
//! data folder, so they can be typed before a show starts and outlive it:
//! ending a show, opening another quiz or a test run keeps them. A show
//! starts with them as its first journaled `SetTeams`, and a change during
//! a show reaches the show (journaled first, S1) and then the file.

use crate::error::CmdResult;
use crate::{app_data, app_file, execute_at, now_ms, timed_lock, AppState, Inner};
use quiz_core::{game::same_name, team_names, Command, MAX_TEAMS};
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
/// file that cannot be read is kept aside and the app starts without names.
pub fn load(path: &Path) -> Vec<String> {
    let (file, lost) = app_file::load::<TeamsFile>(path);
    if let Some(lost) = lost {
        eprintln!("team names not read: {} (kept as {:?})", lost.detail, lost.kept_as);
    }
    // A file edited by hand may hold more names than a show takes: the first ones count.
    team_names(file.names.iter().take(MAX_TEAMS).map(String::as_str)).unwrap_or_default()
}

/// The team names prepared for the next show, or the running show's.
pub fn current(inner: &Inner) -> &[String] {
    inner.game.as_ref().map_or(&inner.teams, |game| game.teams())
}

/// Makes `teams` the prepared names: in the running show first, journaled,
/// then for the windows and on disk.
fn change(app: &AppHandle, inner: &mut Inner, teams: Vec<String>, at_ms: u64) -> CmdResult<()> {
    if inner.game.is_some() {
        execute_at(app, inner, Command::SetTeams { names: teams.clone() }, at_ms)?;
    }
    inner.teams = teams;
    let _ = app.emit("teams-changed", current(inner));
    app_file::store(&teams_path(app)?, &TeamsFile { names: inner.teams.clone() })
}

/// The team names prepared for shows.
#[tauri::command]
pub fn prepared_teams(state: State<AppState>) -> Vec<String> {
    current(&state.0.lock().unwrap()).to_vec()
}

/// Adds team names after the prepared ones; names already there are kept once.
#[tauri::command]
pub fn add_teams(app: AppHandle, state: State<AppState>, names: Vec<String>) -> CmdResult<()> {
    let (mut inner, at_ms) = timed_lock(&state.0, now_ms);
    let teams = team_names(current(&inner).iter().chain(&names).map(String::as_str))?;
    change(&app, &mut inner, teams, at_ms)
}

/// Removes a prepared team name. A buzzer paired with it keeps its player's name.
#[tauri::command]
pub fn remove_team(app: AppHandle, state: State<AppState>, name: String) -> CmdResult<()> {
    let (mut inner, at_ms) = timed_lock(&state.0, now_ms);
    let teams = current(&inner).iter().filter(|team| !same_name(team, &name)).cloned().collect();
    change(&app, &mut inner, teams, at_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_names_read_back_as_a_show_takes_them() {
        let dir = std::env::temp_dir().join(format!("teams-{}", std::process::id()));
        let path = dir.join(TEAMS_FILE);
        assert!(load(&path).is_empty());
        let mut names: Vec<String> = vec![" Hawks ".into(), "hawks".into(), String::new()];
        names.extend((0..MAX_TEAMS + 5).map(|i| format!("Team {i}")));
        app_file::store(&path, &TeamsFile { names }).unwrap();
        let teams = load(&path);
        assert_eq!(teams.len(), MAX_TEAMS - 2, "the first {MAX_TEAMS} lines, without the repeat and the blank");
        assert_eq!(teams[..2], ["Hawks".to_string(), "Team 0".to_string()]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
