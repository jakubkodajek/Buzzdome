//! Connects Claude Desktop to the app's MCP server (`mcp`) the way Claude
//! Desktop adds a local server: an entry under `mcpServers` in its
//! configuration file, `claude_desktop_config.json`, with the command that
//! starts the server ("Connect to local MCP servers", modelcontextprotocol.io).
//! Claude Desktop reads the file when it starts.

use crate::roots::Roots;
use quiz_file::{FileAction, QuizFileError};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// The server's name in Claude Desktop's list.
pub const SERVER_KEY: &str = crate::mcp::SERVER_NAME;
/// The argument that starts the app as the MCP server.
pub const MCP_ARG: &str = "--mcp";
const CONFIG_FILE: &str = "claude_desktop_config.json";

/// What connecting changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ClaudeConnection {
    Added,
    /// The entry started another program, such as the app before it
    /// moved, or worked in other folders.
    Updated,
    Unchanged,
}

/// What connecting did, and where the assistant may now work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeConnected {
    pub connection: ClaudeConnection,
    /// The folders Claude Desktop may work in (`Roots::folders`).
    pub folders: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("Claude Desktop is not installed for this user: {} is missing", .0.display())]
    NotInstalled(PathBuf),
    #[error("{} is not a Claude Desktop configuration it could be added to", .0.display())]
    NotAConfig(PathBuf),
    #[error(transparent)]
    File(#[from] QuizFileError),
}

/// Where Claude Desktop keeps its configuration for this user: in the
/// roaming application data on Windows, in Application Support on macOS.
/// None on other systems, where it does not run.
pub fn config_path() -> Option<PathBuf> {
    let folder = if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA")?)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support")
    } else {
        return None;
    };
    Some(folder.join("Claude").join(CONFIG_FILE))
}

/// Whether the configuration file `config` names this app's MCP server, so
/// that Claude Desktop runs it while it is open: false without the file,
/// the entry, or a configuration that can be read.
pub fn is_connected(config: &Path) -> bool {
    let settings = fs::read_to_string(config).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok());
    settings.is_some_and(|settings| settings["mcpServers"].get(SERVER_KEY).is_some())
}

/// Adds the server that `command --mcp` starts, working in `roots`, to the
/// configuration file `config`, keeping everything else in it. The file is made if Claude
/// Desktop has not written one yet, but its folder must be there: without
/// it, Claude Desktop is not installed. A file that is not a configuration
/// is left as it is.
pub fn connect(config: &Path, command: &Path, roots: &Roots) -> Result<ClaudeConnected, ConnectError> {
    let folders = roots.folders().iter().map(|folder| folder.display().to_string()).collect();
    Ok(ClaudeConnected { connection: add_server(config, command, roots)?, folders })
}

fn add_server(config: &Path, command: &Path, roots: &Roots) -> Result<ClaudeConnection, ConnectError> {
    let folder = config.parent().unwrap_or(Path::new("."));
    if !folder.is_dir() {
        return Err(ConnectError::NotInstalled(folder.to_path_buf()));
    }
    let mut settings = match fs::read_to_string(config) {
        Ok(text) if text.trim().is_empty() => Map::new(),
        Ok(text) => match serde_json::from_str(&text) {
            Ok(Value::Object(settings)) => settings,
            _ => return Err(ConnectError::NotAConfig(config.to_path_buf())),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Map::new(),
        Err(source) => return Err(QuizFileError::Io { action: FileAction::Read, path: config.to_path_buf(), source }.into()),
    };
    let servers = settings.entry("mcpServers").or_insert_with(|| json!({}));
    let Some(servers) = servers.as_object_mut() else {
        return Err(ConnectError::NotAConfig(config.to_path_buf()));
    };
    let args: Vec<String> = std::iter::once(MCP_ARG.to_string()).chain(roots.args()).collect();
    let entry = json!({ "command": command, "args": args });
    let connection = match servers.get(SERVER_KEY) {
        Some(found) if *found == entry => return Ok(ClaudeConnection::Unchanged),
        Some(_) => ClaudeConnection::Updated,
        None => ClaudeConnection::Added,
    };
    servers.insert(SERVER_KEY.into(), entry);
    let text = serde_json::to_string_pretty(&settings).expect("JSON values serialize");
    quiz_file::write_atomic(config, text.as_bytes())?;
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roots::{allow, ROOT_ARG};
    use crate::workbench::tests::scratch;

    #[test]
    fn the_server_is_added_beside_the_others_and_moved_with_the_app() {
        let dir = scratch("claude-config");
        let config = dir.join(CONFIG_FILE);
        fs::write(&config, r#"{ "mcpServers": { "files": { "command": "npx", "args": ["fs"] } }, "theme": "dark" }"#).unwrap();
        let app = dir.join("Buzzdome/buzzdome.exe");
        let quizzes = dir.join("quizzes");
        fs::create_dir_all(&quizzes).unwrap();
        let roots = Roots::new(vec![allow(&quizzes, &[]).unwrap()]);
        let root = roots.folders()[0].clone();
        assert_eq!(connect(&config, &app, &roots).unwrap().connection, ClaudeConnection::Added);
        assert_eq!(connect(&config, &app, &roots).unwrap().connection, ClaudeConnection::Unchanged);
        let written: Value = serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(
            written,
            json!({
                "mcpServers": {
                    "files": { "command": "npx", "args": ["fs"] },
                    "buzzdome": { "command": app, "args": ["--mcp", ROOT_ARG, root] },
                },
                "theme": "dark",
            })
        );
        assert_eq!(connect(&config, &dir.join("moved/buzzdome.exe"), &roots).unwrap().connection, ClaudeConnection::Updated);
        assert_eq!(connect(&config, &dir.join("moved/buzzdome.exe"), &Roots::default()).unwrap().connection, ClaudeConnection::Updated);
    }

    #[test]
    fn a_configuration_that_names_the_server_is_connected() {
        let dir = scratch("claude-connected");
        let config = dir.join(CONFIG_FILE);
        assert!(!is_connected(&config));
        for text in ["{ broken", "[1]", r#"{ "mcpServers": [] }"#, r#"{ "mcpServers": { "files": {} } }"#] {
            fs::write(&config, text).unwrap();
            assert!(!is_connected(&config), "{text}");
        }
        connect(&config, Path::new("/b"), &Roots::default()).unwrap();
        assert!(is_connected(&config));
    }

    #[test]
    fn a_first_configuration_is_made_but_only_where_claude_desktop_is_installed() {
        let dir = scratch("claude-first");
        let config = dir.join(CONFIG_FILE);
        assert_eq!(connect(&config, Path::new("/Applications/Buzzdome.app/Contents/MacOS/buzzdome"), &Roots::default()).unwrap().connection, ClaudeConnection::Added);
        assert!(fs::read_to_string(&config).unwrap().contains("--mcp"));
        let missing = dir.join("no-claude").join(CONFIG_FILE);
        assert!(matches!(connect(&missing, Path::new("/b"), &Roots::default()), Err(ConnectError::NotInstalled(_))));
        assert!(!missing.exists());
    }

    #[test]
    fn a_file_that_is_no_configuration_is_left_as_it_is() {
        let dir = scratch("claude-broken");
        let config = dir.join(CONFIG_FILE);
        for text in ["{ broken", "[1]", r#"{ "mcpServers": [] }"#] {
            fs::write(&config, text).unwrap();
            assert!(matches!(connect(&config, Path::new("/b"), &Roots::default()), Err(ConnectError::NotAConfig(_))), "{text}");
            assert_eq!(fs::read_to_string(&config).unwrap(), text);
        }
    }
}
