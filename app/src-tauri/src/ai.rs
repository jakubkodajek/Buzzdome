//! AI assistants (docs/ai-assistants.md). Started with `--mcp`, the app is
//! the MCP server that an AI app runs to write quizzes (`quiz_ai::mcp`) and
//! nothing else, working only in the folder the quizmaster chose; the
//! editor's File › AI assistants page chooses that folder, connects AI apps
//! (`quiz_ai::assistants`) and keeps the quizmaster's instructions for every
//! quiz (`quiz_ai::settings`).

use crate::app_file::{AppFile, SettingsLost};
use crate::app_log;
use crate::dialogs;
use crate::error::{AppError, CmdResult};
use crate::off_ui;
use quiz_ai::assistants::{Assistant, ConnectChange, ConnectError, ConnectionState, ServerCommand};
use quiz_ai::mcp::{self, MCP_ARG};
use quiz_ai::roots::{self, RootError};
use quiz_ai::settings::{default_instructions, AiSettings, Language, MAX_INSTRUCTIONS_CHARS, SETTINGS_FILE};
use quiz_file::FileAction;
use serde::Serialize;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State, Window};
use ts_rs::TS;

/// Called first in `run`: when an AI app started the app as its MCP server,
/// serves it on standard input and output until the app closes them, and
/// ends the process. `version` is the app's, and `identifier` its bundle
/// identifier, which names its data folders: the AI settings are kept
/// there, and none of them is ever a root.
pub fn serve_mcp_if_asked(version: &str, identifier: &str) {
    if crate::started_as(MCP_ARG).is_none() {
        return;
    }
    let app_data = roots::app_data_dirs(identifier);
    let settings = app_data.first().map(|data| data.join(SETTINGS_FILE)).unwrap_or_default();
    let (roots, refused) = AiSettings::read(&settings).roots(&app_data);
    // AI apps keep what a server writes to standard error in their logs.
    if let Some(error) = refused {
        eprintln!("{error}");
    }
    let context = mcp::Context { version: version.to_string(), roots, settings };
    let served = mcp::serve(io::stdin().lock(), io::stdout().lock(), &context);
    std::process::exit(if served.is_ok() { 0 } else { 1 });
}

/// The AI settings, kept in `SETTINGS_FILE` in the app's data, where the
/// server reads them.
pub struct SavedAiSettings {
    file: Mutex<AppFile>,
    settings: Mutex<AiSettings>,
}

impl SavedAiSettings {
    pub fn load(path: PathBuf) -> SavedAiSettings {
        let (file, settings) = AppFile::load::<AiSettings>(path);
        if let Some(lost) = file.lost() {
            app_log::note(&format!("AI settings not read: {} (kept as {:?})", lost.detail, lost.kept_as));
        }
        SavedAiSettings { file: Mutex::new(file), settings: Mutex::new(settings) }
    }

    fn get(&self) -> AiSettings {
        self.settings.lock().unwrap().clone()
    }

    /// Changes the settings by `change` and keeps them on disk when that
    /// changed them; the settings as they are then.
    fn change(&self, change: impl FnOnce(&mut AiSettings)) -> CmdResult<AiSettings> {
        // Held through the write, so the file ends with the latest settings.
        let file = self.file.lock().unwrap();
        let mut settings = self.get();
        let before = settings.clone();
        change(&mut settings);
        if settings != before {
            file.store(&settings)?;
            *self.settings.lock().unwrap() = settings.clone();
        }
        Ok(settings)
    }
}

/// What the editor's AI assistants page shows.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AiPage {
    /// The folder assistants work in; none when the system names no
    /// documents folder for the default one.
    pub folder: Option<String>,
    /// The quizmaster's own instructions; none while they are the default ones.
    pub instructions: Option<String>,
    /// The default instructions, in the language the page asked in.
    pub default_instructions: String,
    pub max_instructions_chars: usize,
    pub assistants: Vec<AssistantInfo>,
    /// Why the settings were lost when the app started.
    pub settings_error: Option<SettingsLost>,
    /// The system (`std::env::consts::OS`), whose steps the page shows.
    pub platform: String,
}

/// An AI app the page connects.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AssistantInfo {
    pub assistant: Assistant,
    pub name: String,
    pub state: ConnectionState,
    /// The configuration files connecting writes; none when the app is not
    /// installed (Claude Desktop, known by its folder).
    pub config_files: Vec<String>,
}

/// What connecting an AI app did.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AiConnected {
    pub assistant: Assistant,
    pub name: String,
    pub change: ConnectChange,
    /// The folder the app works in.
    pub folder: String,
}

#[tauri::command]
pub async fn ai_page(app: AppHandle, language: Language) -> CmdResult<AiPage> {
    let command = server_command()?;
    off_ui(move || {
        adopt_earlier_folder(&app)?;
        let saved = app.state::<SavedAiSettings>();
        let (settings, settings_error) = (saved.get(), saved.file.lock().unwrap().lost().cloned());
        let info = |assistant: Assistant| AssistantInfo {
            assistant,
            name: assistant.name().into(),
            state: assistant.state(&command),
            config_files: assistant.config_files().iter().map(|config| shown(config)).collect(),
        };
        Ok(AiPage {
            folder: settings.folder().as_deref().map(shown),
            instructions: settings.instructions.clone(),
            default_instructions: default_instructions(language).into(),
            max_instructions_chars: MAX_INSTRUCTIONS_CHARS,
            assistants: Assistant::ALL.into_iter().map(info).collect(),
            settings_error,
            platform: std::env::consts::OS.into(),
        })
    })
    .await
}

/// Lets the quizmaster pick the folder assistants work in, which AI apps
/// use once they start the server again. None when they cancel the picker.
#[tauri::command]
pub async fn ai_choose_folder(window: Window, app: AppHandle) -> CmdResult<Option<String>> {
    let start = app.state::<SavedAiSettings>().get().folder().filter(|folder| folder.is_dir());
    let Some(folder) = dialogs::pick_folder(window, start).await? else { return Ok(None) };
    let app_data = app_data_dirs(&app);
    off_ui(move || {
        roots::allow(&folder, &app_data).map_err(root_error)?;
        app.state::<SavedAiSettings>().change(|settings| settings.folder = Some(folder.clone()))?;
        Ok(Some(shown(&folder)))
    })
    .await
}

/// Adds the app as an MCP server to `assistant`'s configuration.
#[tauri::command]
pub async fn ai_connect(app: AppHandle, assistant: Assistant, language: Language) -> CmdResult<AiConnected> {
    off_ui(move || {
        let folder = ready(&app, language)?;
        let change = assistant.connect(&server_command()?).map_err(|e| match e {
            ConnectError::NotInstalled(name) => AppError::AiAppMissing { app: name.into() },
            ConnectError::NotAConfig(path) => AppError::AiConfigUnreadable { path: shown(&path) },
            ConnectError::File(e) => e.into(),
        })?;
        Ok(AiConnected { assistant, name: assistant.name().into(), change, folder })
    })
    .await
}

/// The configuration with the app as an MCP server, for the quizmaster to
/// paste into an AI app the page does not connect.
#[tauri::command]
pub async fn ai_server_config(app: AppHandle, language: Language) -> CmdResult<String> {
    off_ui(move || {
        ready(&app, language)?;
        Ok(server_command()?.json_config())
    })
    .await
}

/// Keeps `instructions` as the quizmaster's own for every quiz; empty
/// gives the default ones back.
#[tauri::command]
pub async fn ai_set_instructions(state: State<'_, SavedAiSettings>, instructions: String, language: Language) -> CmdResult<()> {
    state.change(|settings| settings.set_instructions(&instructions, language)).map(drop)
}

/// Whether an AI app runs this app's server, which it keeps running in the
/// version it started until it starts it again.
pub fn connected_somewhere() -> bool {
    server_command().is_ok_and(|command| Assistant::ALL.iter().any(|assistant| assistant.state(&command) != ConnectionState::NotConnected))
}

/// Takes the folder chosen in Buzzdome 0.2.0, which an AI app's entry gives
/// the server with `--root`, while none was chosen since: the server now
/// takes it from the settings only, so it is kept across the update.
fn adopt_earlier_folder(app: &AppHandle) -> CmdResult<()> {
    let saved = app.state::<SavedAiSettings>();
    if saved.get().folder.is_some() {
        return Ok(());
    }
    let app_data = app_data_dirs(app);
    let earlier = Assistant::ALL.into_iter().filter_map(Assistant::earlier_folder).find(|folder| roots::allow(folder, &app_data).is_ok());
    if let Some(folder) = earlier {
        app_log::note(&format!("AI folder taken from an earlier connection: {}", folder.display()));
        saved.change(|settings| {
            settings.folder.get_or_insert(folder);
        })?;
    }
    Ok(())
}

/// Gets the settings ready for an AI app to start the server: keeps the
/// quizmaster's `language`, which the default instructions follow, and
/// makes sure the server can work in the folder. The folder, as shown.
fn ready(app: &AppHandle, language: Language) -> CmdResult<String> {
    let settings = app.state::<SavedAiSettings>().change(|settings| settings.language = language)?;
    settings.root(&app_data_dirs(app)).map_err(root_error)?;
    Ok(settings.folder().as_deref().map(shown).unwrap_or_default())
}

/// The command that starts this app as the server.
fn server_command() -> CmdResult<ServerCommand> {
    std::env::current_exe().map(|program| ServerCommand::new(&program)).map_err(AppError::system)
}

/// The app's data folders, which are never a root.
fn app_data_dirs(app: &AppHandle) -> Vec<PathBuf> {
    [app.path().app_data_dir(), app.path().app_local_data_dir(), app.path().app_log_dir()].into_iter().flatten().collect()
}

fn root_error(error: RootError) -> AppError {
    match error {
        RootError::NoDocumentsFolder => AppError::NoDocumentsFolder,
        RootError::AppData { path, .. } => AppError::AiFolderNotAllowed { path: shown(&path) },
        RootError::Unusable { path, source } => AppError::file(FileAction::Read, &path, source),
        RootError::NotAFolder(path) => AppError::file(FileAction::Read, &path, io::Error::from(io::ErrorKind::NotADirectory)),
    }
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}
