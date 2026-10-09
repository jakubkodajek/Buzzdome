//! The quiz editor's draft: an unpacked quiz the editor changes in place.
//! Every change is written at once, so closing or crashing the app loses
//! nothing; saving packs the draft into a `.quiz` file.
//!
//! Under `<app data>/editor/`, `draft.json` names the current draft folder
//! (`quiz.json` and `media/`) and remembers where it is saved. A new or
//! opened quiz gets a folder of its own and `draft.json` switches to it only
//! once it is complete, so a failure leaves the old draft as it was. Old
//! folders are deleted afterwards; one the web view still reads a file from
//! (Windows refuses that) goes the next time.

use crate::{app_data, CmdResult};
use quiz_core::{Quiz, ValidationError, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;

const POINTER_FILE: &str = "draft.json";
const FOLDER_PREFIX: &str = "draft-";

/// One editor command at a time: they share the draft folder.
#[derive(Default)]
pub struct EditorLock(Mutex<()>);

/// Contents of `draft.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pointer {
    folder: String,
    source_path: Option<String>,
    unsaved: bool,
}

/// The draft as the editor loads it.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorDraft {
    pub quiz: Quiz,
    /// The file it was opened from or last saved to.
    pub source_path: Option<String>,
    /// It has changes that are not in that file.
    pub unsaved: bool,
    /// Folder of the draft's media, ending with a path separator.
    pub media_root: String,
    pub problems: Vec<ValidationError>,
}

fn err(e: impl ToString) -> String {
    e.to_string()
}

fn editor_dir(app: &AppHandle) -> CmdResult<PathBuf> {
    app_data(app, "editor")
}

fn read_pointer(dir: &Path) -> Option<Pointer> {
    let json = fs::read_to_string(dir.join(POINTER_FILE)).ok()?;
    let pointer: Pointer = serde_json::from_str(&json).ok()?;
    // The file is ours, but a folder name must never lead out of `dir`.
    let plain = pointer.folder.strip_prefix(FOLDER_PREFIX).is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit()));
    plain.then_some(pointer)
}

fn write_pointer(dir: &Path, pointer: &Pointer) -> CmdResult<()> {
    let json = serde_json::to_string_pretty(pointer).map_err(err)?;
    quiz_file::write_atomic(&dir.join(POINTER_FILE), json.as_bytes()).map_err(err)
}

/// The current draft's folder and its pointer.
fn current(app: &AppHandle) -> CmdResult<(PathBuf, Pointer)> {
    let dir = editor_dir(app)?;
    let pointer = read_pointer(&dir).ok_or("no quiz is open in the editor")?;
    Ok((dir.join(&pointer.folder), pointer))
}

/// Folder of the quiz in the editor, for a test run.
pub(crate) fn draft_root(app: &AppHandle) -> CmdResult<PathBuf> {
    current(app).map(|(root, _)| root)
}

/// A name for a new draft folder that is not taken.
fn new_folder(dir: &Path) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    (now..).map(|n| format!("{FOLDER_PREFIX}{n}")).find(|name| !dir.join(name).exists()).expect("a free name")
}

/// Makes `pointer` the current draft, then deletes the other folders.
fn switch_to(dir: &Path, pointer: &Pointer) -> CmdResult<()> {
    write_pointer(dir, pointer)?;
    if let Err(e) = quiz_file::prune(dir, &[&dir.join(&pointer.folder)]) {
        eprintln!("cleaning old editor drafts: {e}");
    }
    Ok(())
}

/// Lets the windows show the draft's media, and describes the draft.
fn describe(app: &AppHandle, root: &Path, pointer: Pointer, quiz: Quiz) -> CmdResult<EditorDraft> {
    app.asset_protocol_scope().allow_directory(root, true).map_err(err)?;
    Ok(EditorDraft {
        problems: quiz_file::problems(&quiz, root),
        quiz,
        source_path: pointer.source_path,
        unsaved: pointer.unsaved,
        media_root: root.join("").to_string_lossy().into_owned(),
    })
}

/// The draft left from last time, if any.
#[tauri::command]
pub async fn editor_load(app: AppHandle, lock: State<'_, EditorLock>) -> CmdResult<Option<EditorDraft>> {
    let _one_at_a_time = lock.0.lock().unwrap();
    let Ok((root, pointer)) = current(&app) else { return Ok(None) };
    let json = fs::read_to_string(root.join(quiz_file::QUIZ_JSON)).map_err(err)?;
    let quiz = Quiz::from_json(&json)?;
    describe(&app, &root, pointer, quiz).map(Some)
}

/// Starts a new, empty quiz in place of the draft.
#[tauri::command]
pub async fn editor_new(app: AppHandle, lock: State<'_, EditorLock>, title: String) -> CmdResult<EditorDraft> {
    let _one_at_a_time = lock.0.lock().unwrap();
    let dir = editor_dir(&app)?;
    let folder = new_folder(&dir);
    let root = dir.join(&folder);
    let quiz = Quiz { schema_version: SCHEMA_VERSION, title, slides: Vec::new() };
    fs::create_dir_all(&root).map_err(err)?;
    quiz_file::write_quiz_json(&root, &quiz).map_err(err)?;
    let pointer = Pointer { folder, source_path: None, unsaved: false };
    switch_to(&dir, &pointer)?;
    describe(&app, &root, pointer, quiz)
}

/// Opens a quiz file in place of the draft.
#[tauri::command]
pub async fn editor_open(app: AppHandle, lock: State<'_, EditorLock>, path: String) -> CmdResult<EditorDraft> {
    let _one_at_a_time = lock.0.lock().unwrap();
    let dir = editor_dir(&app)?;
    let folder = new_folder(&dir);
    let root = dir.join(&folder);
    let quiz = quiz_file::open_into(Path::new(&path), &root).map_err(err)?;
    let pointer = Pointer { folder, source_path: Some(path), unsaved: false };
    switch_to(&dir, &pointer)?;
    describe(&app, &root, pointer, quiz)
}

/// Stores the edited quiz in the draft and returns what keeps it from being
/// played. `unsaved` tells whether it differs from the saved file.
#[tauri::command]
pub async fn editor_update(
    app: AppHandle,
    lock: State<'_, EditorLock>,
    quiz: Quiz,
    unsaved: bool,
) -> CmdResult<Vec<ValidationError>> {
    let _one_at_a_time = lock.0.lock().unwrap();
    let (root, pointer) = current(&app)?;
    quiz_file::write_quiz_json(&root, &quiz).map_err(err)?;
    if pointer.unsaved != unsaved {
        write_pointer(&editor_dir(&app)?, &Pointer { unsaved, ..pointer })?;
    }
    Ok(quiz_file::problems(&quiz, &root))
}

/// Saves the quiz with its media to `path` as a quiz file.
#[tauri::command]
pub async fn editor_save(app: AppHandle, lock: State<'_, EditorLock>, quiz: Quiz, path: String) -> CmdResult<()> {
    let _one_at_a_time = lock.0.lock().unwrap();
    let (root, pointer) = current(&app)?;
    quiz_file::write_quiz_json(&root, &quiz).map_err(err)?;
    quiz_file::save(&quiz, &root, Path::new(&path)).map_err(err)?;
    write_pointer(&editor_dir(&app)?, &Pointer { source_path: Some(path), unsaved: false, ..pointer })
}

/// Copies a picture, sound or video into the draft and returns its reference.
#[tauri::command]
pub async fn editor_add_media(app: AppHandle, lock: State<'_, EditorLock>, path: String) -> CmdResult<String> {
    let _one_at_a_time = lock.0.lock().unwrap();
    let (root, _) = current(&app)?;
    quiz_file::import_media(&root, Path::new(&path)).map_err(err)
}

/// Discards the draft.
#[tauri::command]
pub async fn editor_close(app: AppHandle, lock: State<'_, EditorLock>) -> CmdResult<()> {
    let _one_at_a_time = lock.0.lock().unwrap();
    let dir = editor_dir(&app)?;
    match fs::remove_file(dir.join(POINTER_FILE)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(err(e)),
        _ => {}
    }
    if let Err(e) = quiz_file::prune(&dir, &[]) {
        eprintln!("cleaning old editor drafts: {e}");
    }
    Ok(())
}
