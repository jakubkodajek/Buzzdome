//! Connects AI apps on this computer to the app's MCP server (`mcp`). Each
//! keeps the local servers it starts in a configuration file of its own, as
//! an entry with the command that starts the server; connecting adds the
//! app's entry there and keeps everything else in the file. The entry is
//! the same for every app, `<the app> --mcp`, as the server reads the
//! folder and the instructions from the app's settings (`settings`).
//!
//! - Claude Desktop: `mcpServers` in `claude_desktop_config.json`, in
//!   Application Support on macOS and the roaming application data on
//!   Windows, which it reads when it starts ("Connect to local MCP servers",
//!   modelcontextprotocol.io). On Linux, in beta there, Anthropic names
//!   `~/.config/Claude` for its logs only ("Configuration format",
//!   claude.com/docs/third-party/claude-desktop/configuration-format); the
//!   file is taken to be in that folder too.
//! - Antigravity: `mcpServers` in `~/.gemini/config/mcp_config.json`
//!   ("MCP", antigravity.google/docs/mcp).
//! - Codex: a `[mcp_servers.<name>]` table in `~/.codex/config.toml`, which
//!   the ChatGPT desktop app, the Codex command line and its IDE extension
//!   share ("Model Context Protocol", learn.chatgpt.com/docs/extend/mcp).
//!   `CODEX_HOME` moves the folder.
//!
//! Each app is known by the folder it makes when it first starts: one that
//! is not installed has no configuration files.
//!
//! On Windows Claude Desktop's file has two homes. Claude Desktop from its
//! classic installer keeps it in `Claude` in the roaming application data
//! (`%APPDATA%`), the one place the page above names. The packaged (MSIX)
//! Claude Desktop, which the installer from claude.ai/download sets up,
//! sees that folder through Windows' file system virtualization: the files
//! it makes there go to its own copy of the folder,
//! `Packages\Claude_<publisher>\LocalCache\Roaming\Claude` in the local
//! application data (`%LOCALAPPDATA%`), opening a file looks in that copy
//! first and in the real folder only when the copy has none, and files
//! already in the real folder are changed in place ("Understanding how
//! packaged desktop apps run on Windows", learn.microsoft.com). So the
//! packaged app reads the configuration in its copy whenever there is one,
//! which hides the real file (anthropics/claude-code#26073).
//!
//! Users report that the packaged Claude Desktop moved its data with an
//! update in October 2026 (3.1623), to `Claude-Data\claude_<publisher>` in
//! the local application data (anthropics/claude-code#103860), and that its
//! Edit Config then opens the configuration there (PixelHabits/mx#13).
//! Anthropic does not document it. So connecting writes that file too,
//! beside the older ones, and the app is found whichever one it reads.

use crate::mcp::{MCP_ARG, SERVER_NAME};
use quiz_file::{FileAction, QuizFileError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// Claude Desktop's configuration file.
const CLAUDE_CONFIG: &str = "claude_desktop_config.json";
/// Claude Desktop's folder in the application data.
const CLAUDE_FOLDER: &str = "Claude";
/// How the folder of a packaged Claude Desktop in `%LOCALAPPDATA%\Packages`
/// starts: its package family name, `Claude_<publisher id>`, which Windows
/// compares ignoring case ("Package identity overview", learn.microsoft.com).
const PACKAGE_PREFIX: &str = "Claude_";
/// The folder in `%LOCALAPPDATA%` where current packaged Claude Desktops keep
/// their data, one folder each, named after the package family in lower case.
const CLAUDE_DATA: &str = "Claude-Data";
/// The argument with which Buzzdome 0.2.0 gave the server a folder chosen in
/// the app; the folder is now in the AI settings.
const EARLIER_ROOT_ARG: &str = "--root";

/// An AI app the app can add itself to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Assistant {
    ClaudeDesktop,
    Antigravity,
    Codex,
}

/// How an AI app keeps its servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    /// JSON, an object under `mcpServers`.
    Json,
    /// TOML, a table under `mcp_servers`.
    Toml,
}

/// Whether an AI app starts the app's server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionState {
    NotConnected,
    Connected,
    /// The app's entry starts another program, such as the app before it
    /// moved, or one of its files has no entry: connecting again fixes it.
    Outdated,
}

/// What connecting changed.
// Ordered from the biggest change, so that the least of what connecting
// changed in each file is what it changed in all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ConnectChange {
    Added,
    /// The entry started another program, such as the app before it moved.
    Updated,
    Unchanged,
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("{0} is not installed for this user")]
    NotInstalled(&'static str),
    #[error("{} is not a configuration the server could be added to", .0.display())]
    NotAConfig(PathBuf),
    #[error(transparent)]
    File(#[from] QuizFileError),
}

/// The command an AI app runs to start the server: the app, with `--mcp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerCommand {
    command: String,
    args: Vec<String>,
}

impl ServerCommand {
    /// The command that starts `program` (the running app) as the server.
    pub fn new(program: &Path) -> ServerCommand {
        ServerCommand { command: program.display().to_string(), args: vec![MCP_ARG.to_string()] }
    }

    /// The configuration most AI apps take, with the server under
    /// `mcpServers`, for the quizmaster to give an app the app cannot add
    /// itself to.
    pub fn json_config(&self) -> String {
        let config = json!({ "mcpServers": { SERVER_NAME: { "command": self.command, "args": self.args } } });
        serde_json::to_string_pretty(&config).expect("JSON values serialize")
    }
}

impl Assistant {
    pub const ALL: [Assistant; 3] = [Assistant::ClaudeDesktop, Assistant::Antigravity, Assistant::Codex];

    /// The app's name, as it calls itself.
    pub fn name(self) -> &'static str {
        match self {
            Assistant::ClaudeDesktop => "Claude Desktop",
            Assistant::Antigravity => "Antigravity",
            Assistant::Codex => "Codex",
        }
    }

    fn format(self) -> Format {
        match self {
            Assistant::ClaudeDesktop | Assistant::Antigravity => Format::Json,
            Assistant::Codex => Format::Toml,
        }
    }

    /// The configuration files the app reads for this user, which connecting
    /// writes, and makes when they are not there yet: none where the app is
    /// not installed, as its folder is not there (see the module's notes).
    pub fn config_files(self) -> Vec<PathBuf> {
        self.config_files_with(&|name| std::env::var_os(name).map(PathBuf::from).filter(|path| path.is_absolute()))
    }

    /// `config_files`, with `var` giving the folder an environment variable
    /// names.
    fn config_files_with(self, var: &dyn Fn(&str) -> Option<PathBuf>) -> Vec<PathBuf> {
        let home = var(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
        let installed = |folder: PathBuf| folder.is_dir().then_some(folder);
        match self {
            Assistant::ClaudeDesktop if cfg!(windows) => windows_claude_configs(var("APPDATA").as_deref(), var("LOCALAPPDATA").as_deref()),
            Assistant::ClaudeDesktop => {
                let data = if cfg!(target_os = "macos") { home.map(|home| home.join("Library").join("Application Support")) } else { home.map(|home| home.join(".config")) };
                data.and_then(|data| claude_config_in(&data.join(CLAUDE_FOLDER))).into_iter().collect()
            }
            Assistant::Antigravity => home.and_then(|home| installed(home.join(".gemini"))).map(|gemini| gemini.join("config").join("mcp_config.json")).into_iter().collect(),
            Assistant::Codex => var("CODEX_HOME").or_else(|| home.map(|home| home.join(".codex"))).and_then(installed).map(|codex| codex.join("config.toml")).into_iter().collect(),
        }
    }

    /// Whether the app starts `command` as the server.
    pub fn state(self, command: &ServerCommand) -> ConnectionState {
        state_in(self.format(), &self.config_files(), command)
    }

    /// Adds `command` as the server to each of the app's configuration files
    /// (`config_files`).
    pub fn connect(self, command: &ServerCommand) -> Result<ConnectChange, ConnectError> {
        let configs = self.config_files();
        if configs.is_empty() {
            return Err(ConnectError::NotInstalled(self.name()));
        }
        connect_in(self.format(), &configs, command)
    }

    /// The folder that the app's entry gives the server with `--root`, as
    /// Buzzdome 0.2.0 wrote it when the quizmaster chose a folder.
    pub fn earlier_folder(self) -> Option<PathBuf> {
        earlier_folder_in(self.format(), &self.config_files())
    }
}

/// The files Claude Desktop reads on Windows, given the roaming (`roaming`)
/// and local (`local`) application data: the file in the real folder; the
/// one in each packaged app's copy of it, unless it is not there yet and the
/// real one is, which the packaged app then reads instead; and the one in
/// each packaged app's own data folder (see the module's notes). Writing
/// them all keeps the app found whichever it reads.
fn windows_claude_configs(roaming: Option<&Path>, local: Option<&Path>) -> Vec<PathBuf> {
    let real = roaming.and_then(|roaming| claude_config_in(&roaming.join(CLAUDE_FOLDER)));
    let real_exists = real.as_deref().is_some_and(Path::is_file);
    let packaged_in = |folder: &str| local.map(|local| packaged_claude_folders(&local.join(folder))).unwrap_or_default();
    let copies = packaged_in("Packages")
        .into_iter()
        .filter_map(|package| claude_config_in(&package.join("LocalCache").join("Roaming").join(CLAUDE_FOLDER)))
        .filter(|config| config.is_file() || !real_exists);
    let data = packaged_in(CLAUDE_DATA).into_iter().map(|folder| folder.join(CLAUDE_CONFIG));
    real.into_iter().chain(copies).chain(data).collect()
}

/// The folders in `parent` named after a packaged Claude Desktop, whose
/// names start with `PACKAGE_PREFIX` in any case, in order of name.
fn packaged_claude_folders(parent: &Path) -> Vec<PathBuf> {
    let named = |name: &str| name.get(..PACKAGE_PREFIX.len()).is_some_and(|start| start.eq_ignore_ascii_case(PACKAGE_PREFIX));
    let mut folders: Vec<PathBuf> = fs::read_dir(parent)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| named(&entry.file_name().to_string_lossy()))
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    folders.sort();
    folders
}

/// The configuration file in Claude Desktop's folder `folder`, when the
/// folder is there.
fn claude_config_in(folder: &Path) -> Option<PathBuf> {
    folder.is_dir().then(|| folder.join(CLAUDE_CONFIG))
}

/// Whether the configuration files `configs`, in `format`, start `command`:
/// connected when each of them does, not connected when none has the
/// server or none can be read.
fn state_in(format: Format, configs: &[PathBuf], command: &ServerCommand) -> ConnectionState {
    let entry = |config: &PathBuf| fs::read_to_string(config).ok().and_then(|text| server_entry(format, &text, command).ok()).and_then(|(found, _)| found);
    let entries: Vec<Option<bool>> = configs.iter().map(entry).collect();
    if entries.iter().all(Option::is_none) {
        ConnectionState::NotConnected
    } else if entries.iter().all(|found| *found == Some(true)) {
        ConnectionState::Connected
    } else {
        ConnectionState::Outdated
    }
}

/// Adds `command` to each of the configuration files `configs`, in
/// `format`, making those that are not there yet. Nothing is written when
/// one of them is not a configuration, which is left as it is.
fn connect_in(format: Format, configs: &[PathBuf], command: &ServerCommand) -> Result<ConnectChange, ConnectError> {
    let changes = configs.iter().map(|config| with_server(format, config, command)).collect::<Result<Vec<_>, _>>()?;
    for (config, (_, text)) in configs.iter().zip(&changes) {
        if let Some(text) = text {
            let file = linked_file(config);
            if let Some(folder) = file.parent() {
                fs::create_dir_all(folder).map_err(|source| QuizFileError::Io { action: FileAction::CreateFolder, path: folder.to_path_buf(), source })?;
            }
            quiz_file::write_atomic_keeping_mode(&file, text.as_bytes())?;
        }
    }
    Ok(changes.iter().map(|(change, _)| *change).min().unwrap_or(ConnectChange::Unchanged))
}

/// The file that writing `config` changes: the one a link there leads to,
/// as the user keeps it there (dotfiles kept in one folder), so that the
/// link stays a link.
fn linked_file(config: &Path) -> PathBuf {
    if !fs::symlink_metadata(config).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return config.to_path_buf();
    }
    // A link to a file that is not there yet leads where it names.
    fs::canonicalize(config)
        .or_else(|_| fs::read_link(config).map(|target| config.parent().map_or_else(|| target.clone(), |folder| folder.join(&target))))
        .unwrap_or_else(|_| config.to_path_buf())
}

/// The folder that the server's entry gives with `EARLIER_ROOT_ARG`, in the
/// first of the configuration files `configs`, in `format`, that has one.
fn earlier_folder_in(format: Format, configs: &[PathBuf]) -> Option<PathBuf> {
    configs.iter().find_map(|config| {
        let args = entry_args(format, &fs::read_to_string(config).ok()?)?;
        let root = args.iter().position(|arg| arg == EARLIER_ROOT_ARG)?;
        args.get(root + 1).map(PathBuf::from)
    })
}

/// The arguments of the server's entry in the configuration text `text`.
fn entry_args(format: Format, text: &str) -> Option<Vec<String>> {
    match format {
        Format::Json => {
            let settings: Value = serde_json::from_str(text).ok()?;
            let args = settings.get("mcpServers")?.get(SERVER_NAME)?.get("args")?.as_array()?;
            args.iter().map(|arg| arg.as_str().map(String::from)).collect()
        }
        Format::Toml => {
            let config: toml_edit::DocumentMut = text.parse().ok()?;
            let args = config.get("mcp_servers")?.get(SERVER_NAME)?.get("args")?.as_array()?;
            args.iter().map(|arg| arg.as_str().map(String::from)).collect()
        }
    }
}

/// What adding `command` to the configuration file `config` changes, and
/// the file's new text when it changes it.
fn with_server(format: Format, config: &Path, command: &ServerCommand) -> Result<(ConnectChange, Option<String>), ConnectError> {
    let text = match fs::read_to_string(config) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(source) => return Err(QuizFileError::Io { action: FileAction::Read, path: config.to_path_buf(), source }.into()),
    };
    let (found, changed) = server_entry(format, &text, command).map_err(|NotAConfig| ConnectError::NotAConfig(config.to_path_buf()))?;
    Ok(match found {
        Some(true) => (ConnectChange::Unchanged, None),
        Some(false) => (ConnectChange::Updated, Some(changed)),
        None => (ConnectChange::Added, Some(changed)),
    })
}

/// A configuration's text holds something else than a configuration.
struct NotAConfig;

/// The server's entry in the configuration text `text`, in `format`:
/// whether it starts `command`, none without an entry; and the text with
/// the entry starting `command`, keeping everything else, the entry's other
/// settings too.
fn server_entry(format: Format, text: &str, command: &ServerCommand) -> Result<(Option<bool>, String), NotAConfig> {
    match format {
        Format::Json => json_entry(text, command),
        Format::Toml => toml_entry(text, command),
    }
}

fn json_entry(text: &str, command: &ServerCommand) -> Result<(Option<bool>, String), NotAConfig> {
    let mut settings = match serde_json::from_str(text) {
        Ok(Value::Object(settings)) => settings,
        _ if text.trim().is_empty() => Map::new(),
        _ => return Err(NotAConfig),
    };
    let servers = settings.entry("mcpServers").or_insert_with(|| json!({})).as_object_mut().ok_or(NotAConfig)?;
    let found = servers.get(SERVER_NAME).map(|entry| entry["command"] == command.command.as_str() && entry["args"] == json!(command.args));
    let entry = servers.entry(SERVER_NAME).or_insert_with(|| json!({}));
    if !entry.is_object() {
        *entry = json!({});
    }
    let entry = entry.as_object_mut().expect("made an object");
    entry.insert("command".into(), json!(command.command));
    entry.insert("args".into(), json!(command.args));
    Ok((found, serde_json::to_string_pretty(&settings).expect("JSON values serialize")))
}

fn toml_entry(text: &str, command: &ServerCommand) -> Result<(Option<bool>, String), NotAConfig> {
    use toml_edit::{Array, DocumentMut, Item, Table};
    let mut config: DocumentMut = text.parse().map_err(|_| NotAConfig)?;
    let implicit = || {
        let mut table = Table::new();
        table.set_implicit(true);
        Item::Table(table)
    };
    let servers = config.entry("mcp_servers").or_insert_with(implicit).as_table_like_mut().ok_or(NotAConfig)?;
    let found = servers.get(SERVER_NAME).map(|entry| {
        let args = entry.get("args").and_then(Item::as_array);
        entry.get("command").and_then(Item::as_str) == Some(command.command.as_str())
            && args.is_some_and(|args| args.iter().map(toml_edit::Value::as_str).eq(command.args.iter().map(|arg| Some(arg.as_str()))))
    });
    let entry = servers.entry(SERVER_NAME).or_insert(toml_edit::table());
    if !entry.is_table_like() {
        *entry = toml_edit::table();
    }
    let entry = entry.as_table_like_mut().expect("made a table");
    entry.insert("command", toml_edit::value(command.command.as_str()));
    entry.insert("args", toml_edit::value(command.args.iter().map(String::as_str).collect::<Array>()));
    Ok((found, config.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::tests::scratch;

    fn command(program: &str) -> ServerCommand {
        ServerCommand::new(Path::new(program))
    }

    #[test]
    fn the_server_is_added_beside_the_others_and_moved_with_the_app() {
        let dir = scratch("claude-config");
        let config = [dir.join(CLAUDE_CONFIG)];
        fs::write(&config[0], r#"{ "mcpServers": { "files": { "command": "npx", "args": ["fs"] } }, "theme": "dark" }"#).unwrap();
        let app = dir.join("Buzzdome/buzzdome.exe");
        let server = ServerCommand::new(&app);
        assert_eq!(connect_in(Format::Json, &config, &server).unwrap(), ConnectChange::Added);
        assert_eq!(connect_in(Format::Json, &config, &server).unwrap(), ConnectChange::Unchanged);
        let written: Value = serde_json::from_str(&fs::read_to_string(&config[0]).unwrap()).unwrap();
        assert_eq!(
            written,
            json!({
                "mcpServers": {
                    "files": { "command": "npx", "args": ["fs"] },
                    "buzzdome": { "command": app, "args": ["--mcp"] },
                },
                "theme": "dark",
            })
        );
        assert_eq!(serde_json::from_str::<Value>(&server.json_config()).unwrap()["mcpServers"], json!({ "buzzdome": written["mcpServers"]["buzzdome"] }));
        let moved = ServerCommand::new(&dir.join("moved/buzzdome.exe"));
        assert_eq!(state_in(Format::Json, &config, &moved), ConnectionState::Outdated);
        assert_eq!(connect_in(Format::Json, &config, &moved).unwrap(), ConnectChange::Updated);
        assert_eq!(state_in(Format::Json, &config, &moved), ConnectionState::Connected);
    }

    #[test]
    fn an_entry_that_names_the_folders_is_updated_keeping_its_other_settings() {
        let dir = scratch("claude-config-roots");
        let config = [dir.join(CLAUDE_CONFIG)];
        fs::write(&config[0], r#"{ "mcpServers": { "buzzdome": { "command": "/b", "args": ["--mcp", "--root", "/q"], "env": { "A": "1" } } } }"#).unwrap();
        assert_eq!(state_in(Format::Json, &config, &command("/b")), ConnectionState::Outdated);
        assert_eq!(connect_in(Format::Json, &config, &command("/b")).unwrap(), ConnectChange::Updated);
        let written: Value = serde_json::from_str(&fs::read_to_string(&config[0]).unwrap()).unwrap();
        assert_eq!(written["mcpServers"]["buzzdome"], json!({ "command": "/b", "args": ["--mcp"], "env": { "A": "1" } }));
    }

    #[test]
    fn a_configuration_that_starts_the_server_is_connected() {
        let dir = scratch("claude-connected");
        let config = [dir.join(CLAUDE_CONFIG)];
        assert_eq!(state_in(Format::Json, &config, &command("/b")), ConnectionState::NotConnected);
        assert_eq!(state_in(Format::Json, &[], &command("/b")), ConnectionState::NotConnected);
        for text in ["{ broken", "[1]", r#"{ "mcpServers": [] }"#, r#"{ "mcpServers": { "files": {} } }"#] {
            fs::write(&config[0], text).unwrap();
            assert_eq!(state_in(Format::Json, &config, &command("/b")), ConnectionState::NotConnected, "{text}");
        }
        connect_in(Format::Json, &config, &command("/b")).unwrap();
        assert_eq!(state_in(Format::Json, &config, &command("/b")), ConnectionState::Connected);
        let other = dir.join("other").join(CLAUDE_CONFIG);
        assert_eq!(state_in(Format::Json, &[other, config[0].clone()], &command("/b")), ConnectionState::Outdated, "one file without it");
    }

    #[test]
    fn a_first_configuration_is_made_with_its_folder() {
        let dir = scratch("ai-config-first");
        let config = [dir.join(".gemini/config/mcp_config.json")];
        assert_eq!(connect_in(Format::Json, &config, &command("/Applications/Buzzdome.app/Contents/MacOS/buzzdome")).unwrap(), ConnectChange::Added);
        assert!(fs::read_to_string(&config[0]).unwrap().contains("--mcp"));
    }

    #[test]
    fn a_file_that_is_no_configuration_is_left_as_it_is() {
        let dir = scratch("claude-broken");
        let config = [dir.join(CLAUDE_CONFIG)];
        for text in ["{ broken", "[1]", r#"{ "mcpServers": [] }"#] {
            fs::write(&config[0], text).unwrap();
            assert!(matches!(connect_in(Format::Json, &config, &command("/b")), Err(ConnectError::NotAConfig(_))), "{text}");
            assert_eq!(fs::read_to_string(&config[0]).unwrap(), text);
        }
        let config = [dir.join("config.toml")];
        for text in ["model = ", "mcp_servers = 3"] {
            fs::write(&config[0], text).unwrap();
            assert!(matches!(connect_in(Format::Toml, &config, &command("/b")), Err(ConnectError::NotAConfig(_))), "{text}");
            assert_eq!(fs::read_to_string(&config[0]).unwrap(), text);
        }
    }

    #[test]
    fn codex_gets_a_table_and_keeps_its_comments_and_other_servers() {
        let dir = scratch("codex-config");
        let config = [dir.join("config.toml")];
        let before = "# My settings\nmodel = \"gpt-5\"\n\n[mcp_servers.docs]\ncommand = \"npx\" # the docs server\nargs = [\"docs\"]\n";
        fs::write(&config[0], before).unwrap();
        let app = command("C:\\Program Files\\Buzzdome\\buzzdome.exe");
        assert_eq!(state_in(Format::Toml, &config, &app), ConnectionState::NotConnected);
        assert_eq!(connect_in(Format::Toml, &config, &app).unwrap(), ConnectChange::Added);
        let written = fs::read_to_string(&config[0]).unwrap();
        assert!(written.starts_with(before), "{written}");
        let parsed: toml_edit::DocumentMut = written.parse().unwrap();
        assert_eq!(parsed["mcp_servers"]["buzzdome"]["command"].as_str(), Some("C:\\Program Files\\Buzzdome\\buzzdome.exe"));
        let args: Vec<_> = parsed["mcp_servers"]["buzzdome"]["args"].as_array().unwrap().iter().map(|arg| arg.as_str().unwrap()).collect();
        assert_eq!(args, ["--mcp"]);
        assert_eq!(state_in(Format::Toml, &config, &app), ConnectionState::Connected);
        assert_eq!(connect_in(Format::Toml, &config, &app).unwrap(), ConnectChange::Unchanged);
        fs::write(&config[0], "[mcp_servers.buzzdome]\ncommand = \"/old\"\nargs = [\"--mcp\", \"--root\", \"/q\"]\nstartup_timeout_sec = 30\n").unwrap();
        assert_eq!(connect_in(Format::Toml, &config, &command("/b")).unwrap(), ConnectChange::Updated);
        assert_eq!(fs::read_to_string(&config[0]).unwrap(), "[mcp_servers.buzzdome]\ncommand = \"/b\"\nargs = [\"--mcp\"]\nstartup_timeout_sec = 30\n");
    }

    #[test]
    fn a_first_codex_configuration_has_only_the_servers_table() {
        let dir = scratch("codex-first");
        let config = [dir.join(".codex/config.toml")];
        connect_in(Format::Toml, &config, &command("/b")).unwrap();
        assert_eq!(fs::read_to_string(&config[0]).unwrap(), "[mcp_servers.buzzdome]\ncommand = \"/b\"\nargs = [\"--mcp\"]\n");
    }

    /// The folders of a Windows user: the roaming and the local application
    /// data, and a packaged Claude Desktop's copy of the roaming one.
    fn windows_user(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let dir = scratch(name);
        let (roaming, local) = (dir.join("Roaming"), dir.join("Local"));
        let packaged = local.join("Packages/Claude_pzs8sxrjxfjjc/LocalCache/Roaming").join(CLAUDE_FOLDER);
        fs::create_dir_all(&roaming).unwrap();
        fs::create_dir_all(local.join("Packages/Microsoft.WindowsCalculator_8wekyb3d8bbwe/LocalCache/Roaming").join(CLAUDE_FOLDER)).unwrap();
        (roaming, local, packaged)
    }

    fn found(roaming: &Path, local: &Path) -> Vec<PathBuf> {
        windows_claude_configs(Some(roaming), Some(local))
    }

    #[test]
    fn on_windows_the_classic_and_the_packaged_claude_desktop_are_both_found() {
        let (roaming, local, packaged) = windows_user("claude-windows");
        let real = roaming.join(CLAUDE_FOLDER);
        assert_eq!(found(&roaming, &local), Vec::<PathBuf>::new(), "not installed");
        assert_eq!(windows_claude_configs(None, None), Vec::<PathBuf>::new());
        fs::create_dir_all(&packaged).unwrap();
        assert_eq!(found(&roaming, &local), [packaged.join(CLAUDE_CONFIG)], "packaged, started once");
        fs::create_dir_all(&real).unwrap();
        assert_eq!(found(&roaming, &local), [real.join(CLAUDE_CONFIG), packaged.join(CLAUDE_CONFIG)], "both, neither file written yet");
        fs::write(real.join(CLAUDE_CONFIG), "{}").unwrap();
        assert_eq!(found(&roaming, &local), [real.join(CLAUDE_CONFIG)], "the packaged app reads the real file while its copy has none");
        fs::write(packaged.join(CLAUDE_CONFIG), "").unwrap();
        assert_eq!(found(&roaming, &local), [real.join(CLAUDE_CONFIG), packaged.join(CLAUDE_CONFIG)], "its own file hides the real one");
        fs::remove_dir_all(&real).unwrap();
        assert_eq!(windows_claude_configs(None, Some(&local)), [packaged.join(CLAUDE_CONFIG)]);
    }

    #[test]
    fn a_current_packaged_claude_desktop_is_found_in_its_data_folder() {
        let (roaming, local, packaged) = windows_user("claude-windows-data");
        let data = local.join(CLAUDE_DATA).join("claude_pzs8sxrjxfjjc");
        fs::create_dir_all(&data).unwrap();
        fs::create_dir_all(local.join(CLAUDE_DATA).join("logs")).unwrap();
        fs::write(local.join(CLAUDE_DATA).join("claude_notes.txt"), "").unwrap();
        assert_eq!(found(&roaming, &local), [data.join(CLAUDE_CONFIG)], "started once, its file not written yet");
        let real = roaming.join(CLAUDE_FOLDER);
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join(CLAUDE_CONFIG), "{}").unwrap();
        fs::create_dir_all(&packaged).unwrap();
        fs::write(packaged.join(CLAUDE_CONFIG), "{}").unwrap();
        assert_eq!(found(&roaming, &local), [real.join(CLAUDE_CONFIG), packaged.join(CLAUDE_CONFIG), data.join(CLAUDE_CONFIG)], "the older homes are kept");
    }

    #[test]
    fn a_packaged_claude_desktop_is_known_by_its_package_name_in_any_case() {
        let (roaming, local, _) = windows_user("claude-windows-names");
        let copy = |package: &str| local.join("Packages").join(package).join("LocalCache/Roaming").join(CLAUDE_FOLDER);
        for package in ["CLAUDE_pzs8sxrjxfjjc", "ClaudeHelper_pzs8sxrjxfjjc"] {
            fs::create_dir_all(copy(package)).unwrap();
            fs::create_dir_all(local.join(CLAUDE_DATA).join(package.to_lowercase())).unwrap();
        }
        let data = local.join(CLAUDE_DATA).join("claude_pzs8sxrjxfjjc");
        assert_eq!(found(&roaming, &local), [copy("CLAUDE_pzs8sxrjxfjjc").join(CLAUDE_CONFIG), data.join(CLAUDE_CONFIG)]);
    }

    #[test]
    fn antigravity_and_codex_are_known_by_their_folders() {
        let home = scratch("ai-apps-installed");
        let codex_home = home.join("codex-home");
        let files = |assistant: Assistant, codex_home_set: bool| {
            assistant.config_files_with(&|name| match name {
                "HOME" | "USERPROFILE" => Some(home.clone()),
                "CODEX_HOME" if codex_home_set => Some(codex_home.clone()),
                _ => None,
            })
        };
        assert_eq!(files(Assistant::Antigravity, false), Vec::<PathBuf>::new(), "not installed");
        assert_eq!(files(Assistant::Codex, false), Vec::<PathBuf>::new(), "not installed");
        fs::create_dir_all(home.join(".gemini")).unwrap();
        fs::create_dir_all(home.join(".codex")).unwrap();
        assert_eq!(files(Assistant::Antigravity, false), [home.join(".gemini").join("config").join("mcp_config.json")]);
        assert_eq!(files(Assistant::Codex, false), [home.join(".codex").join("config.toml")]);
        assert_eq!(files(Assistant::Codex, true), Vec::<PathBuf>::new(), "CODEX_HOME names a folder that is not there");
        fs::create_dir_all(&codex_home).unwrap();
        assert_eq!(files(Assistant::Codex, true), [codex_home.join("config.toml")]);
    }

    #[test]
    fn the_folder_an_earlier_entry_named_is_found() {
        let dir = scratch("ai-earlier-folder");
        let claude = [dir.join("missing").join(CLAUDE_CONFIG), dir.join(CLAUDE_CONFIG)];
        assert_eq!(earlier_folder_in(Format::Json, &claude), None);
        fs::write(&claude[1], r#"{ "mcpServers": { "buzzdome": { "command": "/b", "args": ["--mcp"] } } }"#).unwrap();
        assert_eq!(earlier_folder_in(Format::Json, &claude), None);
        fs::write(&claude[1], r#"{ "mcpServers": { "buzzdome": { "command": "/b", "args": ["--mcp", "--root", "D:\\Quizzes"] } } }"#).unwrap();
        assert_eq!(earlier_folder_in(Format::Json, &claude), Some(PathBuf::from("D:\\Quizzes")));
        let codex = [dir.join("config.toml")];
        fs::write(&codex[0], "[mcp_servers.buzzdome]\ncommand = \"/b\"\nargs = [\"--mcp\", \"--root\", \"/q\"]\n").unwrap();
        assert_eq!(earlier_folder_in(Format::Toml, &codex), Some(PathBuf::from("/q")));
        fs::write(&codex[0], "[mcp_servers.buzzdome]\nargs = [\"--mcp\", \"--root\"]\n").unwrap();
        assert_eq!(earlier_folder_in(Format::Toml, &codex), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_configuration_is_written_where_the_link_leads_keeping_its_mode() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = scratch("ai-config-link");
        let dotfiles = dir.join("dotfiles");
        fs::create_dir_all(&dotfiles).unwrap();
        let (real, link) = (dotfiles.join("config.toml"), dir.join("config.toml"));
        fs::write(&real, "model = \"gpt-5\"\n").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&real, &link).unwrap();
        assert_eq!(connect_in(Format::Toml, std::slice::from_ref(&link), &command("/b")).unwrap(), ConnectChange::Added);
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "the link stays");
        assert!(fs::read_to_string(&real).unwrap().contains("[mcp_servers.buzzdome]"));
        assert_eq!(fs::metadata(&real).unwrap().permissions().mode() & 0o777, 0o600);
        let (missing, dangling) = (dotfiles.join("claude.json"), dir.join(CLAUDE_CONFIG));
        symlink(&missing, &dangling).unwrap();
        connect_in(Format::Json, std::slice::from_ref(&dangling), &command("/b")).unwrap();
        assert!(fs::symlink_metadata(&dangling).unwrap().file_type().is_symlink() && missing.is_file(), "a link to a file not there yet makes it");
    }

    #[test]
    fn connecting_writes_every_file_claude_desktop_may_read_or_none() {
        let (roaming, local, packaged) = windows_user("claude-windows-connect");
        let real = roaming.join(CLAUDE_FOLDER);
        fs::create_dir_all(&real).unwrap();
        fs::create_dir_all(&packaged).unwrap();
        fs::write(packaged.join(CLAUDE_CONFIG), r#"{ "preferences": { "menuBarEnabled": false } }"#).unwrap();
        let configs = found(&roaming, &local);
        let app = command("C:/Buzzdome/buzzdome.exe");
        assert_eq!(connect_in(Format::Json, &configs, &app).unwrap(), ConnectChange::Added);
        assert_eq!(state_in(Format::Json, &configs, &app), ConnectionState::Connected);
        let kept: Value = serde_json::from_str(&fs::read_to_string(packaged.join(CLAUDE_CONFIG)).unwrap()).unwrap();
        assert_eq!(kept["preferences"], json!({ "menuBarEnabled": false }));
        fs::write(real.join(CLAUDE_CONFIG), r#"{ "mcpServers": { "buzzdome": { "command": "C:/old/buzzdome.exe" } } }"#).unwrap();
        assert_eq!(state_in(Format::Json, &configs, &app), ConnectionState::Outdated);
        assert_eq!(connect_in(Format::Json, &configs, &app).unwrap(), ConnectChange::Updated);
        assert_eq!(connect_in(Format::Json, &configs, &app).unwrap(), ConnectChange::Unchanged);
        fs::write(real.join(CLAUDE_CONFIG), "[1]").unwrap();
        let before = fs::read_to_string(packaged.join(CLAUDE_CONFIG)).unwrap();
        assert!(matches!(connect_in(Format::Json, &configs, &command("D:/buzzdome.exe")), Err(ConnectError::NotAConfig(_))));
        assert_eq!(fs::read_to_string(packaged.join(CLAUDE_CONFIG)).unwrap(), before, "nothing written");
    }
}
