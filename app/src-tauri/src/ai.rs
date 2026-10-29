//! AI assistants (docs/ai-assistants.md). Started with `--mcp`, the app is
//! the MCP server that a desktop assistant runs to write quizzes
//! (`quiz_ai::mcp`) and nothing else, working only in the folders named by
//! `--root` (`quiz_ai::roots`); the editor's File › AI assistants page saves
//! the skill package for chat assistants and connects Claude Desktop with
//! the folder the quizmaster chooses. A skill uploaded to an assistant stays
//! as it was when saved, so the app remembers which one it saved and says
//! when an update has changed it (`SavedSkill`).

use crate::app_file::AppFile;
use crate::app_log;
use crate::dialogs::{self, Picker};
use crate::error::{AppError, CmdResult};
use crate::off_ui;
use quiz_ai::claude_desktop::{self, ClaudeConnected, ConnectError, MCP_ARG};
use quiz_ai::roots::{self, RootError, Roots};
use quiz_file::FileAction;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State, Window};

/// The folder in the quizmaster's documents that Claude Desktop works in
/// unless they choose another.
const QUIZ_FOLDER: &str = "Buzzdome";

/// Called first in `run`: when an assistant started the app as its MCP
/// server, serves it on standard input and output until the assistant
/// closes them, and ends the process. `version` is the app's, and
/// `identifier` its bundle identifier, which names its data folders: those
/// are never a root.
pub fn serve_mcp_if_asked(version: &str, identifier: &str) {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).map(|arg| arg.as_os_str()) != Some(OsStr::new(MCP_ARG)) {
        return;
    }
    let (roots, refused) = Roots::from_args(&args[2..], &roots::app_data_dirs(identifier));
    // Claude Desktop keeps what a server writes to standard error in its log.
    for error in refused {
        eprintln!("{error}");
    }
    let served = quiz_ai::mcp::serve(std::io::stdin().lock(), std::io::stdout().lock(), version, &roots);
    std::process::exit(if served.is_ok() { 0 } else { 1 });
}

/// Where the fingerprint of the skill last saved is kept, in the app's data folder.
pub const SKILL_FILE: &str = "ai-skill.json";

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillFile {
    /// `quiz_ai::skill::fingerprint` of the package last saved; none before the first.
    #[serde(default)]
    saved: Option<String>,
}

/// Which skill package the quizmaster saved last, if any.
pub struct SavedSkill {
    /// Held while a save is written, so the last package saved is the one kept.
    file: Mutex<AppFile>,
    /// The fingerprint kept; never held while the disk works.
    saved: Mutex<Option<String>>,
}

impl SavedSkill {
    /// What is kept at `path`. A file that cannot be read counts as no
    /// skill saved: the app then says nothing of its skill until the next save.
    pub fn load(path: PathBuf) -> SavedSkill {
        let (file, saved) = AppFile::load::<SkillFile>(path);
        if let Some(lost) = file.lost() {
            app_log::note(&format!("saved AI skill not read: {} (kept as {:?})", lost.detail, lost.kept_as));
        }
        SavedSkill { file: Mutex::new(file), saved: Mutex::new(saved.saved) }
    }

    /// Whether a skill was saved and this version's differs from it.
    fn outdated(&self, current: &str) -> bool {
        self.saved.lock().unwrap().as_deref().is_some_and(|saved| saved != current)
    }

    /// Keeps `fingerprint` as the skill saved.
    fn store(&self, fingerprint: String) -> CmdResult<()> {
        let file = self.file.lock().unwrap();
        file.store(&SkillFile { saved: Some(fingerprint.clone()) })?;
        *self.saved.lock().unwrap() = Some(fingerprint);
        Ok(())
    }
}

/// Writes the skill package where the quizmaster picks; false when they
/// cancel. The package saved is remembered (`ai_skill_outdated`).
#[tauri::command]
pub async fn ai_save_skill(window: Window, app: AppHandle, picker: Picker) -> CmdResult<bool> {
    let picker = Picker { file_name: Some(quiz_ai::skill::NAME.into()), ..picker };
    let Some(path) = dialogs::pick_save(window, picker, quiz_ai::skill::EXTENSION, None).await? else { return Ok(false) };
    off_ui(move || {
        quiz_ai::skill::write(&path)?;
        app.state::<SavedSkill>().store(quiz_ai::skill::fingerprint())?;
        Ok(true)
    })
    .await
}

/// Whether the skill the quizmaster saved last differs from this version's,
/// as after an update that changed it: the copy uploaded to an assistant
/// stays as it was, so it should be saved and uploaded again.
#[tauri::command]
pub fn ai_skill_outdated(saved: State<SavedSkill>) -> bool {
    saved.outdated(&quiz_ai::skill::fingerprint())
}

/// Adds this app as an MCP server to Claude Desktop's configuration, working
/// in the folder the quizmaster picks when `choose`, else in `QUIZ_FOLDER`
/// in their documents, which is made. None when they cancel the picker.
#[tauri::command]
pub async fn ai_connect_claude(window: Window, app: AppHandle, choose: bool) -> CmdResult<Option<ClaudeConnected>> {
    let documents = app.path().document_dir().ok();
    let folder = if choose {
        let Some(folder) = dialogs::pick_folder(window, documents).await? else { return Ok(None) };
        folder
    } else {
        let folder = documents.ok_or(AppError::NoDocumentsFolder)?.join(QUIZ_FOLDER);
        std::fs::create_dir_all(&folder).map_err(|e| AppError::file(FileAction::CreateFolder, &folder, e))?;
        folder
    };
    let app_data: Vec<PathBuf> = [app.path().app_data_dir(), app.path().app_local_data_dir(), app.path().app_log_dir()].into_iter().flatten().collect();
    off_ui(move || {
        let root = roots::allow(&folder, &app_data).map_err(|e| match e {
            RootError::AppData { path, .. } => AppError::AiFolderNotAllowed { path: path.display().to_string() },
            RootError::Unusable { path, source } => AppError::file(FileAction::Read, &path, source),
            RootError::NotAFolder(path) => AppError::file(FileAction::Read, &path, io::Error::from(io::ErrorKind::NotADirectory)),
        })?;
        let config = claude_desktop::config_path().ok_or(AppError::ClaudeDesktopMissing)?;
        let command = std::env::current_exe().map_err(AppError::system)?;
        claude_desktop::connect(&config, &command, &Roots::new(vec![root])).map(Some).map_err(|e| match e {
            ConnectError::NotInstalled(_) => AppError::ClaudeDesktopMissing,
            ConnectError::NotAConfig(path) => AppError::ClaudeConfigUnreadable { path: path.display().to_string() },
            ConnectError::File(e) => e.into(),
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skill_is_outdated_only_once_saved_and_changed() {
        let dir = std::env::temp_dir().join(format!("ai-skill-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(SKILL_FILE);
        let _ = std::fs::remove_file(&path);
        let skill = SavedSkill::load(path.clone());
        assert!(!skill.outdated("b"), "no skill saved yet");
        skill.store("a".into()).unwrap();
        assert!(!skill.outdated("a"));
        assert!(skill.outdated("b"));
        // Kept for the next start, as after an update.
        assert!(SavedSkill::load(path.clone()).outdated("b"));
        std::fs::write(&path, "{ broken").unwrap();
        assert!(!SavedSkill::load(path).outdated("b"), "an unreadable file counts as no skill saved");
    }

    /// The command line (`buzzdome-quiz mcp`) keeps the app's data out of
    /// its roots by the identifier quiz-ai knows, which must be the app's.
    #[test]
    fn quiz_ai_knows_the_apps_identifier() {
        let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["identifier"], quiz_ai::roots::APP_IDENTIFIER);
    }
}
