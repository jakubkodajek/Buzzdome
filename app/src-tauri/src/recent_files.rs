//! The quiz files opened, saved or played lately, for the editor's FILE
//! menu and the Director's start, which plays one in a click.
//! Kept here, not in the page: the page opens one by its path, which has to
//! be in the list, so it can open only files the quizmaster once picked or
//! saved to, and never name a path of its own (a share on another computer,
//! say, which Windows would sign in to). A path, not a place in the list: a
//! page showing an older list would open another file.

use crate::app_file::AppFile;
use crate::app_log;
use crate::error::{AppError, CmdResult};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// The file the list is kept in, per computer: losing it loses nothing else.
pub const RECENT_FILES: &str = "recent-files.json";
/// How many files the list keeps.
const KEEP: usize = 10;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    paths: Vec<String>,
}

pub struct RecentFiles {
    file: Mutex<AppFile>,
    paths: Mutex<Vec<String>>,
}

impl RecentFiles {
    pub fn load(path: PathBuf) -> RecentFiles {
        let (file, saved) = AppFile::load::<Saved>(path);
        RecentFiles { file: Mutex::new(file), paths: Mutex::new(saved.paths.into_iter().take(KEEP).collect()) }
    }

    /// The files, latest first.
    pub fn paths(&self) -> Vec<String> {
        self.paths.lock().unwrap().clone()
    }

    /// The file `path`, if the list has it.
    pub fn get(&self, path: &str) -> CmdResult<PathBuf> {
        let listed = self.paths.lock().unwrap().iter().any(|kept| kept == path);
        listed.then(|| PathBuf::from(path)).ok_or(AppError::NotRecent)
    }

    /// Puts `path` first in the list and keeps the list on disk.
    pub fn remember(&self, path: &str) {
        self.change(|paths| *paths = put_first(paths, path));
    }

    /// Takes `path` off the list.
    pub fn forget(&self, path: &str) {
        self.change(|paths| paths.retain(|kept| kept != path));
    }

    /// What to tell the quizmaster when opening the quiz file `path` failed
    /// with `error`. A listed file gone from its folder leaves the list
    /// (`QuizFileGone`). One whose folder is not there either, on a USB
    /// stick that is not plugged in, say, may come back: it stays
    /// (`QuizFileUnreachable`), and the quizmaster can take it off
    /// (`forget_recent_file`). Any other failure, or a file not on the
    /// list, is `error` itself.
    pub fn explain(&self, path: &str, error: AppError) -> AppError {
        let file = Path::new(path);
        if !is_missing(file) || self.get(path).is_err() {
            return error;
        }
        let name = file.file_name().map_or_else(|| path.to_owned(), |name| name.to_string_lossy().into_owned());
        if is_gone(file) {
            self.forget(path);
            return AppError::QuizFileGone { name };
        }
        let folder = file.parent().map_or_else(String::new, |folder| folder.display().to_string());
        AppError::QuizFileUnreachable { name, folder }
    }

    /// Changes the list by `change` and keeps it on disk if it changed.
    /// Called off the windows' thread, which reads the list meanwhile; a
    /// list that cannot be written is only noted in the log.
    fn change(&self, change: impl FnOnce(&mut Vec<String>)) {
        // Held through the write, so the file ends with the latest list
        // when the editor and a show starting change it at once.
        let file = self.file.lock().unwrap();
        let paths = {
            let mut paths = self.paths.lock().unwrap();
            let before = paths.clone();
            change(&mut paths);
            if *paths == before {
                return;
            }
            paths.clone()
        };
        if let Err(e) = file.store(&Saved { paths }) {
            app_log::note(&format!("keeping the recent files: {e}"));
        }
    }
}

/// Whether there is no file at `path`, nor perhaps its folder.
fn is_missing(path: &Path) -> bool {
    path.metadata().is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
}

/// Whether the file `path` is gone: its folder is there and it is not. A
/// file whose folder is not there either, on a USB stick that is not
/// plugged in, say, may come back.
fn is_gone(path: &Path) -> bool {
    is_missing(path) && path.parent().is_some_and(Path::is_dir)
}

/// `paths` with `path` first, once, and at most `KEEP` of them.
fn put_first(paths: &[String], path: &str) -> Vec<String> {
    std::iter::once(path).chain(paths.iter().map(String::as_str).filter(|kept| *kept != path)).take(KEEP).map(str::to_owned).collect()
}

/// The quiz files opened, saved or played lately, latest first.
#[tauri::command]
pub fn recent_files(recent: tauri::State<RecentFiles>) -> Vec<String> {
    recent.paths()
}

/// Takes the quiz file `path` off the recent list; the file stays where it is.
#[tauri::command]
pub async fn forget_recent_file(app: AppHandle, path: String) -> CmdResult<()> {
    crate::off_ui(move || {
        app.state::<RecentFiles>().forget(&path);
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("buzzdome-recent-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_file_opened_again_moves_first_and_the_list_keeps_the_latest() {
        let names: Vec<String> = (0..KEEP).map(|n| format!("/q/{n}.quiz")).collect();
        let list = put_first(&names, "/q/3.quiz");
        assert_eq!(list[..2], ["/q/3.quiz", "/q/0.quiz"]);
        assert_eq!(list.len(), KEEP);
        let list = put_first(&list, "/q/new.quiz");
        assert_eq!((list[0].as_str(), list.len()), ("/q/new.quiz", KEEP));
        assert!(!list.contains(&format!("/q/{}.quiz", KEEP - 1)), "the oldest goes");
    }

    #[test]
    fn the_list_outlives_the_app_and_opens_only_the_files_it_has() {
        let dir = folder("kept");
        let path = dir.join(RECENT_FILES);
        let recent = RecentFiles::load(path.clone());
        assert_eq!(recent.get("/q/a.quiz").unwrap_err(), AppError::NotRecent);
        recent.remember("/q/a.quiz");
        recent.remember("/q/b.quiz");
        let again = RecentFiles::load(path.clone());
        assert_eq!(again.paths(), ["/q/b.quiz", "/q/a.quiz"]);
        assert_eq!(again.get("/q/a.quiz").unwrap(), PathBuf::from("/q/a.quiz"));
        assert_eq!(again.get("//server/q/a.quiz").unwrap_err(), AppError::NotRecent);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_file_is_forgotten_once_it_is_gone_from_its_folder_and_kept_while_its_folder_is_away() {
        let dir = folder("gone");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(RECENT_FILES);
        let recent = RecentFiles::load(path.clone());
        let here = dir.join("here.quiz");
        std::fs::write(&here, b"quiz").unwrap();
        let names = [here, dir.join("gone.quiz"), dir.join("unplugged").join("round.quiz")];
        let [here, gone, unplugged] = names.map(|name| name.to_string_lossy().into_owned());
        let failed = || AppError::NotAQuizFile { detail: "not a ZIP".into() };
        for name in [&here, &gone, &unplugged] {
            recent.remember(name);
        }
        assert_eq!(recent.explain(&here, failed()), failed(), "a file that is there failed for its own reason");
        let picked = dir.join("picked.quiz").to_string_lossy().into_owned();
        assert_eq!(recent.explain(&picked, failed()), failed(), "a file not on the list says nothing of the list");
        assert_eq!(recent.explain(&gone, failed()), AppError::QuizFileGone { name: "gone.quiz".into() });
        let away = AppError::QuizFileUnreachable { name: "round.quiz".into(), folder: dir.join("unplugged").display().to_string() };
        assert_eq!(recent.explain(&unplugged, failed()), away);
        assert_eq!(RecentFiles::load(path.clone()).paths(), [unplugged.clone(), here.clone()]);
        assert_eq!(recent.get(&gone).unwrap_err(), AppError::NotRecent);
        recent.forget(&unplugged);
        assert_eq!(RecentFiles::load(path).paths(), [here]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
