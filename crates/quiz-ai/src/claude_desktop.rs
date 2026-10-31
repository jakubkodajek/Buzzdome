//! Connects Claude Desktop to the app's MCP server (`mcp`) the way Claude
//! Desktop adds a local server: an entry under `mcpServers` in its
//! configuration file, `claude_desktop_config.json`, with the command that
//! starts the server ("Connect to local MCP servers", modelcontextprotocol.io).
//! Claude Desktop reads the file when it starts.
//!
//! On Windows the file has two homes. Claude Desktop from its classic
//! installer keeps it in `Claude` in the roaming application data
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
/// Claude Desktop's folder in the application data.
const CLAUDE_FOLDER: &str = "Claude";
/// How the folder of a packaged Claude Desktop in `%LOCALAPPDATA%\Packages`
/// starts: its package family name, `Claude_<publisher id>`, which Windows
/// compares ignoring case ("Package identity overview", learn.microsoft.com).
const PACKAGE_PREFIX: &str = "Claude_";

/// What connecting changed.
// Ordered from the biggest change, so that the least of what connecting
// changed in each file is what it changed in all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
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
    #[error("Claude Desktop is not installed for this user")]
    NotInstalled,
    #[error("{} is not a Claude Desktop configuration it could be added to", .0.display())]
    NotAConfig(PathBuf),
    #[error(transparent)]
    File(#[from] QuizFileError),
}

/// The configuration files Claude Desktop reads for this user, one for each
/// way it is installed: in the roaming application data, or the packaged
/// app's copy of it, on Windows, in Application Support on macOS. Only
/// files whose folder is there, as Claude Desktop makes its folder when it
/// first starts; empty where it is not installed and on other systems,
/// where it does not run.
pub fn config_files() -> Vec<PathBuf> {
    let var = |name| std::env::var_os(name).map(PathBuf::from);
    if cfg!(windows) {
        windows_config_files(var("APPDATA").as_deref(), var("LOCALAPPDATA").as_deref())
    } else if cfg!(target_os = "macos") {
        var("HOME").and_then(|home| config_in(&home.join("Library/Application Support").join(CLAUDE_FOLDER))).into_iter().collect()
    } else {
        Vec::new()
    }
}

/// The files Claude Desktop reads on Windows, given the roaming (`roaming`)
/// and local (`local`) application data: the file in the real folder,
/// and the one in each packaged app's copy unless it is not there yet and
/// the real one is, which the packaged app then reads instead (see the
/// module's notes). Writing both keeps the app found whichever it reads.
fn windows_config_files(roaming: Option<&Path>, local: Option<&Path>) -> Vec<PathBuf> {
    let real = roaming.and_then(|roaming| config_in(&roaming.join(CLAUDE_FOLDER)));
    let real_exists = real.as_deref().is_some_and(Path::is_file);
    let packages = local.and_then(|local| fs::read_dir(local.join("Packages")).ok());
    let packaged = packages
        .into_iter()
        .flatten()
        .flatten()
        .filter(|package| package.file_name().to_string_lossy().get(..PACKAGE_PREFIX.len()).is_some_and(|start| start.eq_ignore_ascii_case(PACKAGE_PREFIX)))
        .filter_map(|package| config_in(&package.path().join("LocalCache").join("Roaming").join(CLAUDE_FOLDER)))
        .filter(|config| config.is_file() || !real_exists);
    real.into_iter().chain(packaged).collect()
}

/// The configuration file in Claude Desktop's folder `folder`, when the
/// folder is there.
fn config_in(folder: &Path) -> Option<PathBuf> {
    folder.is_dir().then(|| folder.join(CONFIG_FILE))
}

/// Whether one of the configuration files `configs` names this app's MCP
/// server, so that Claude Desktop runs it while it is open: false without
/// the files, the entry, or a configuration that can be read.
pub fn is_connected(configs: &[PathBuf]) -> bool {
    configs.iter().any(|config| {
        let settings = fs::read_to_string(config).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok());
        settings.is_some_and(|settings| settings["mcpServers"].get(SERVER_KEY).is_some())
    })
}

/// Adds the server that `command --mcp` starts, working in `roots`, to each
/// of the configuration files `configs` (`config_files`), keeping
/// everything else in them. A file is made if Claude Desktop has not
/// written one yet. Nothing is written when there is no file, as Claude
/// Desktop is not installed, or when one of them is not a configuration,
/// which is left as it is.
pub fn connect(configs: &[PathBuf], command: &Path, roots: &Roots) -> Result<ClaudeConnected, ConnectError> {
    if configs.is_empty() {
        return Err(ConnectError::NotInstalled);
    }
    let args: Vec<String> = std::iter::once(MCP_ARG.to_string()).chain(roots.args()).collect();
    let entry = json!({ "command": command, "args": args });
    let changes = configs.iter().map(|config| with_server(config, &entry)).collect::<Result<Vec<_>, _>>()?;
    for (config, (_, text)) in configs.iter().zip(&changes) {
        if let Some(text) = text {
            quiz_file::write_atomic(config, text.as_bytes())?;
        }
    }
    let connection = changes.iter().map(|(connection, _)| *connection).min().unwrap_or(ClaudeConnection::Unchanged);
    let folders = roots.folders().iter().map(|folder| folder.display().to_string()).collect();
    Ok(ClaudeConnected { connection, folders })
}

/// What putting `entry` in the configuration file `config` changes, and
/// the file's new text when it does.
fn with_server(config: &Path, entry: &Value) -> Result<(ClaudeConnection, Option<String>), ConnectError> {
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
    let connection = match servers.get(SERVER_KEY) {
        Some(found) if found == entry => return Ok((ClaudeConnection::Unchanged, None)),
        Some(_) => ClaudeConnection::Updated,
        None => ClaudeConnection::Added,
    };
    servers.insert(SERVER_KEY.into(), entry.clone());
    Ok((connection, Some(serde_json::to_string_pretty(&settings).expect("JSON values serialize"))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roots::{allow, ROOT_ARG};
    use crate::workbench::tests::scratch;

    #[test]
    fn the_server_is_added_beside_the_others_and_moved_with_the_app() {
        let dir = scratch("claude-config");
        let config = [dir.join(CONFIG_FILE)];
        fs::write(&config[0], r#"{ "mcpServers": { "files": { "command": "npx", "args": ["fs"] } }, "theme": "dark" }"#).unwrap();
        let app = dir.join("Buzzdome/buzzdome.exe");
        let quizzes = dir.join("quizzes");
        fs::create_dir_all(&quizzes).unwrap();
        let roots = Roots::new(vec![allow(&quizzes, &[]).unwrap()]);
        let root = roots.folders()[0].clone();
        assert_eq!(connect(&config, &app, &roots).unwrap().connection, ClaudeConnection::Added);
        assert_eq!(connect(&config, &app, &roots).unwrap().connection, ClaudeConnection::Unchanged);
        let written: Value = serde_json::from_str(&fs::read_to_string(&config[0]).unwrap()).unwrap();
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
        let config = [dir.join(CONFIG_FILE)];
        assert!(!is_connected(&config));
        assert!(!is_connected(&[]));
        for text in ["{ broken", "[1]", r#"{ "mcpServers": [] }"#, r#"{ "mcpServers": { "files": {} } }"#] {
            fs::write(&config[0], text).unwrap();
            assert!(!is_connected(&config), "{text}");
        }
        connect(&config, Path::new("/b"), &Roots::default()).unwrap();
        assert!(is_connected(&config));
        assert!(is_connected(&[dir.join("other").join(CONFIG_FILE), config[0].clone()]));
    }

    #[test]
    fn a_first_configuration_is_made_but_only_where_claude_desktop_is_installed() {
        let dir = scratch("claude-first");
        let config = [dir.join(CONFIG_FILE)];
        assert_eq!(connect(&config, Path::new("/Applications/Buzzdome.app/Contents/MacOS/buzzdome"), &Roots::default()).unwrap().connection, ClaudeConnection::Added);
        assert!(fs::read_to_string(&config[0]).unwrap().contains("--mcp"));
        assert!(matches!(connect(&[], Path::new("/b"), &Roots::default()), Err(ConnectError::NotInstalled)));
    }

    #[test]
    fn a_file_that_is_no_configuration_is_left_as_it_is() {
        let dir = scratch("claude-broken");
        let config = [dir.join(CONFIG_FILE)];
        for text in ["{ broken", "[1]", r#"{ "mcpServers": [] }"#] {
            fs::write(&config[0], text).unwrap();
            assert!(matches!(connect(&config, Path::new("/b"), &Roots::default()), Err(ConnectError::NotAConfig(_))), "{text}");
            assert_eq!(fs::read_to_string(&config[0]).unwrap(), text);
        }
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
        windows_config_files(Some(roaming), Some(local))
    }

    #[test]
    fn on_windows_the_classic_and_the_packaged_claude_desktop_are_both_found() {
        let (roaming, local, packaged) = windows_user("claude-windows");
        let real = roaming.join(CLAUDE_FOLDER);
        assert_eq!(found(&roaming, &local), Vec::<PathBuf>::new(), "not installed");
        assert_eq!(windows_config_files(None, None), Vec::<PathBuf>::new());
        fs::create_dir_all(&packaged).unwrap();
        assert_eq!(found(&roaming, &local), [packaged.join(CONFIG_FILE)], "packaged, started once");
        fs::create_dir_all(&real).unwrap();
        assert_eq!(found(&roaming, &local), [real.join(CONFIG_FILE), packaged.join(CONFIG_FILE)], "both, neither file written yet");
        fs::write(real.join(CONFIG_FILE), "{}").unwrap();
        assert_eq!(found(&roaming, &local), [real.join(CONFIG_FILE)], "the packaged app reads the real file while its copy has none");
        fs::write(packaged.join(CONFIG_FILE), "").unwrap();
        assert_eq!(found(&roaming, &local), [real.join(CONFIG_FILE), packaged.join(CONFIG_FILE)], "its own file hides the real one");
        fs::remove_dir_all(&real).unwrap();
        assert_eq!(windows_config_files(None, Some(&local)), [packaged.join(CONFIG_FILE)]);
    }

    #[test]
    fn a_packaged_claude_desktop_is_known_by_its_package_name_in_any_case() {
        let (roaming, local, _) = windows_user("claude-windows-names");
        let copy = |package: &str| local.join("Packages").join(package).join("LocalCache/Roaming").join(CLAUDE_FOLDER);
        for package in ["CLAUDE_pzs8sxrjxfjjc", "ClaudeHelper_pzs8sxrjxfjjc"] {
            fs::create_dir_all(copy(package)).unwrap();
        }
        assert_eq!(found(&roaming, &local), [copy("CLAUDE_pzs8sxrjxfjjc").join(CONFIG_FILE)]);
    }

    #[test]
    fn connecting_writes_every_file_claude_desktop_may_read_or_none() {
        let (roaming, local, packaged) = windows_user("claude-windows-connect");
        let real = roaming.join(CLAUDE_FOLDER);
        fs::create_dir_all(&real).unwrap();
        fs::create_dir_all(&packaged).unwrap();
        fs::write(packaged.join(CONFIG_FILE), r#"{ "preferences": { "menuBarEnabled": false } }"#).unwrap();
        let configs = found(&roaming, &local);
        assert_eq!(connect(&configs, Path::new("C:/Buzzdome/buzzdome.exe"), &Roots::default()).unwrap().connection, ClaudeConnection::Added);
        assert!(is_connected(&[real.join(CONFIG_FILE)]) && is_connected(&[packaged.join(CONFIG_FILE)]));
        let kept: Value = serde_json::from_str(&fs::read_to_string(packaged.join(CONFIG_FILE)).unwrap()).unwrap();
        assert_eq!(kept["preferences"], json!({ "menuBarEnabled": false }));
        fs::write(real.join(CONFIG_FILE), r#"{ "mcpServers": { "buzzdome": { "command": "C:/old/buzzdome.exe" } } }"#).unwrap();
        assert_eq!(connect(&configs, Path::new("C:/Buzzdome/buzzdome.exe"), &Roots::default()).unwrap().connection, ClaudeConnection::Updated);
        assert_eq!(connect(&configs, Path::new("C:/Buzzdome/buzzdome.exe"), &Roots::default()).unwrap().connection, ClaudeConnection::Unchanged);
        fs::write(real.join(CONFIG_FILE), "[1]").unwrap();
        let before = fs::read_to_string(packaged.join(CONFIG_FILE)).unwrap();
        assert!(matches!(connect(&configs, Path::new("D:/buzzdome.exe"), &Roots::default()), Err(ConnectError::NotAConfig(_))));
        assert_eq!(fs::read_to_string(packaged.join(CONFIG_FILE)).unwrap(), before, "nothing written");
    }
}
