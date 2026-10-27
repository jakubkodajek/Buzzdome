//! AI assistants (docs/ai-assistants.md). Started with `--mcp`, the app is
//! the MCP server that a desktop assistant runs to write quizzes
//! (`quiz_ai::mcp`) and nothing else, working only in the folders named by
//! `--root` (`quiz_ai::roots`); the editor's File › AI assistants page saves
//! the skill package for chat assistants and connects Claude Desktop with
//! the folder the quizmaster chooses.

use crate::dialogs::{self, Picker};
use crate::error::{AppError, CmdResult};
use crate::off_ui;
use quiz_ai::claude_desktop::{self, ClaudeConnected, ConnectError, MCP_ARG};
use quiz_ai::roots::{self, RootError, Roots};
use quiz_file::FileAction;
use std::ffi::OsStr;
use std::io;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Window};

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

/// Writes the skill package where the quizmaster picks; false when they cancel.
#[tauri::command]
pub async fn ai_save_skill(window: Window, picker: Picker) -> CmdResult<bool> {
    let picker = Picker { file_name: Some(quiz_ai::skill::NAME.into()), ..picker };
    let Some(path) = dialogs::pick_save(window, picker, quiz_ai::skill::EXTENSION, None).await? else { return Ok(false) };
    off_ui(move || Ok(quiz_ai::skill::write(&path).map(|()| true)?)).await
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
    /// The command line (`buzzdome-quiz mcp`) keeps the app's data out of
    /// its roots by the identifier quiz-ai knows, which must be the app's.
    #[test]
    fn quiz_ai_knows_the_apps_identifier() {
        let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["identifier"], quiz_ai::roots::APP_IDENTIFIER);
    }
}
