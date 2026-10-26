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

    /// Takes `path` off the list once the file is gone (`is_gone`).
    pub fn forget_if_gone(&self, path: &str) {
        if is_gone(Path::new(path)) {
            self.change(|paths| paths.retain(|kept| kept != path));
        }
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

/// Whether the file `path` is gone: its folder is there and it is not. A
/// file whose folder is not there either, on a USB stick that is not
/// plugged in, say, may come back.
fn is_gone(path: &Path) -> bool {
    let missing = path.metadata().is_err_and(|e| e.kind() == io::ErrorKind::NotFound);
    missing && path.parent().is_some_and(Path::is_dir)
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
    fn a_file_is_forgotten_once_it_is_gone_from_its_folder() {
        let dir = folder("gone");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(RECENT_FILES);
        let recent = RecentFiles::load(path.clone());
        let here = dir.join("here.quiz");
        std::fs::write(&here, b"quiz").unwrap();
        let names = [here, dir.join("gone.quiz"), dir.join("unplugged").join("round.quiz")];
        let [here, gone, unplugged] = names.map(|name| name.to_string_lossy().into_owned());
        for name in [&here, &gone, &unplugged] {
            recent.remember(name);
            recent.forget_if_gone(name);
        }
        assert_eq!(RecentFiles::load(path).paths(), [unplugged, here]);
        assert_eq!(recent.get(&gone).unwrap_err(), AppError::NotRecent);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
