//! AI assistants (docs/ai-assistants.md). Started with `--mcp`, the app is
//! the MCP server that a desktop assistant runs to write quizzes
//! (`quiz_ai::mcp`) and nothing else; the editor's File › AI assistants page
//! saves the skill package for chat assistants and connects Claude Desktop.

use crate::dialogs::{self, Picker};
use crate::error::{AppError, CmdResult};
use crate::off_ui;
use quiz_ai::claude_desktop::{self, ClaudeConnection, ConnectError, MCP_ARG};
use std::ffi::OsStr;
use tauri::Window;

/// Called first in `run`: when an assistant started the app as its MCP
/// server, serves it on standard input and output until the assistant
/// closes them, and ends the process. `version` is the app's.
pub fn serve_mcp_if_asked(version: &str) {
    if std::env::args_os().nth(1).as_deref() != Some(OsStr::new(MCP_ARG)) {
        return;
    }
    let served = quiz_ai::mcp::serve(std::io::stdin().lock(), std::io::stdout().lock(), version);
    std::process::exit(if served.is_ok() { 0 } else { 1 });
}

/// Writes the skill package where the quizmaster picks; false when they cancel.
#[tauri::command]
pub async fn ai_save_skill(window: Window, picker: Picker) -> CmdResult<bool> {
    let picker = Picker { file_name: Some(quiz_ai::skill::NAME.into()), ..picker };
    let Some(path) = dialogs::pick_save(window, picker, quiz_ai::skill::EXTENSION, None).await? else { return Ok(false) };
    off_ui(move || Ok(quiz_ai::skill::write(&path).map(|()| true)?)).await
}

/// Adds this app as an MCP server to Claude Desktop's configuration.
#[tauri::command]
pub async fn ai_connect_claude() -> CmdResult<ClaudeConnection> {
    off_ui(|| {
        let config = claude_desktop::config_path().ok_or(AppError::ClaudeDesktopMissing)?;
        let app = std::env::current_exe().map_err(AppError::system)?;
        claude_desktop::connect(&config, &app).map_err(|e| match e {
            ConnectError::NotInstalled(_) => AppError::ClaudeDesktopMissing,
            ConnectError::NotAConfig(path) => AppError::ClaudeConfigUnreadable { path: path.display().to_string() },
            ConnectError::File(e) => e.into(),
        })
    })
    .await
}
