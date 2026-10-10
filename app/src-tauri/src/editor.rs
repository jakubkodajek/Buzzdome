//! The quiz editor's draft: an unpacked quiz the editor changes in place.
//! Every change is written at once, so closing or crashing the app loses
//! nothing; saving packs the draft into a `.quiz` file.
//!
//! Under `<local data>/editor/`, `draft.json` names the current draft folder
//! (`quiz.json` and `media/`) and remembers where it is saved. A new or
//! opened quiz gets a folder of its own and `draft.json` switches to it only
//! once it is complete, so a failure leaves the old draft as it was. Old
//! folders are deleted afterwards; one the web view still reads a file from
//! (Windows refuses that) goes the next time.
//!
//! The folder's name is the draft's id: a command for a draft that another
//! has replaced since is refused (A16). A `draft.json` that cannot be read
//! is never taken for "no draft": nothing is deleted until the quizmaster
//! discards the draft (A17).

use crate::dialogs::{self, Picker};
use crate::error::{AppError, CmdResult};
use crate::{app_local_data, app_log, lock_show, media_player, off_ui, roaming_leftover, show_running};
use quiz_core::{Category, MediaKind, Quiz, ValidationError};
use quiz_file::{FileAction, SheetExport, SheetImport};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, Window};
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
    /// What opening the file left out, until the draft is saved over it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dropped: Vec<ValidationError>,
}

/// The draft as the editor loads it.
#[derive(Debug, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorDraft {
    /// Names the draft (its folder); commands for it are refused once
    /// another draft has replaced it.
    pub id: String,
    pub quiz: Quiz,
    /// The file it was opened from or last saved to.
    pub source_path: Option<String>,
    /// It has changes that are not in that file.
    pub unsaved: bool,
    /// Folder of the draft's media, ending with a path separator.
    pub media_root: String,
    pub problems: Vec<ValidationError>,
    /// What the file has that the quiz cannot keep (`quiz_core::ReadQuiz`),
    /// so saving leaves it out.
    pub dropped: Vec<ValidationError>,
}

const EDITOR_FOLDER: &str = "editor";

pub(crate) fn editor_dir(app: &AppHandle) -> CmdResult<PathBuf> {
    app_local_data(app, EDITOR_FOLDER)
}

/// Moves the drafts that earlier versions kept in the roaming folder
/// (Windows) to where they are kept now, so an unsaved quiz is not lost.
pub fn move_roaming_drafts(app: &AppHandle) {
    let (Some(old), Ok(dir)) = (roaming_leftover(app, EDITOR_FOLDER), editor_dir(app)) else { return };
    if dir.exists() {
        return;
    }
    let moved = dir.parent().map_or(Ok(()), fs::create_dir_all).and_then(|()| fs::rename(&old, &dir));
    if let Err(e) = moved {
        app_log::note(&format!("moving editor drafts from {}: {e}", old.display()));
    }
}

/// The current draft's pointer, or none when there is no draft. One that
/// cannot be read is an error, not "no draft", so that nothing deletes the
/// draft it names (A17). Not `AppFile`: a pointer has no default to go on with.
fn read_pointer(dir: &Path) -> CmdResult<Option<Pointer>> {
    let path = dir.join(POINTER_FILE);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(AppError::file(FileAction::Read, &path, e)),
    };
    let pointer: Pointer = serde_json::from_slice(&bytes).map_err(|e| AppError::DraftNotRead { detail: e.to_string() })?;
    // The file is ours, but a folder name must never lead out of `dir`.
    let plain = pointer.folder.strip_prefix(FOLDER_PREFIX).is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit()));
    if !plain {
        return Err(AppError::DraftNotRead { detail: format!("folder {:?}", pointer.folder) });
    }
    Ok(Some(pointer))
}

fn write_pointer(dir: &Path, pointer: &Pointer) -> CmdResult<()> {
    let json = serde_json::to_string_pretty(pointer).expect("a pointer serializes: strings, a flag and problems");
    Ok(quiz_file::write_atomic(&dir.join(POINTER_FILE), json.as_bytes())?)
}

/// The folder and pointer of the draft `id`, while it is the current one.
fn current(dir: &Path, id: &str) -> CmdResult<(PathBuf, Pointer)> {
    let pointer = read_pointer(dir)?.ok_or(AppError::NoDraft)?;
    if pointer.folder != id {
        return Err(AppError::DraftChanged);
    }
    Ok((dir.join(&pointer.folder), pointer))
}

/// Runs `work` on the editor's folder, one editor command at a time and off
/// the async workers: it reads and writes files (`off_ui`).
async fn with_drafts<T: Send + 'static>(app: AppHandle, work: impl FnOnce(&AppHandle, &Path) -> CmdResult<T> + Send + 'static) -> CmdResult<T> {
    off_ui(move || {
        let _one_at_a_time = lock(&app);
        work(&app, &editor_dir(&app)?)
    })
    .await
}

/// Runs `work` on the draft `id` (its folder and pointer), refused once
/// another draft has replaced it (A16).
async fn on_draft<T: Send + 'static>(
    app: AppHandle,
    id: String,
    work: impl FnOnce(&AppHandle, &Path, PathBuf, Pointer) -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    with_drafts(app, move |app, dir| {
        let (root, pointer) = current(dir, &id)?;
        work(app, dir, root, pointer)
    })
    .await
}

/// Holds the editor's folder, as every editor command does. Taken before
/// the show state, never while holding it (`crate::lock_show`).
pub(crate) fn lock(app: &AppHandle) -> MutexGuard<'_, ()> {
    app.state::<EditorLock>().inner().0.lock().unwrap()
}

/// Folder of the draft `id`, for a test run; the caller holds `lock`.
pub(crate) fn draft_root(app: &AppHandle, id: &str) -> CmdResult<PathBuf> {
    current(&editor_dir(app)?, id).map(|(root, _)| root)
}

/// Deletes the draft folders but `keep` and the one a test run plays from.
fn prune(app: &AppHandle, dir: &Path, keep: Option<&Path>) {
    let playing = lock_show(app).media_root.clone();
    let keep: Vec<&Path> = keep.into_iter().chain(playing.as_deref()).collect();
    if let Err(e) = quiz_file::prune(dir, &keep) {
        app_log::note(&format!("cleaning old editor drafts: {e}"));
    }
}

/// A name for a new draft folder that is not taken.
fn new_folder(dir: &Path) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    (now..).map(|n| format!("{FOLDER_PREFIX}{n}")).find(|name| !dir.join(name).exists()).expect("a free name")
}

/// Makes `pointer` the current draft, then deletes the other folders.
fn switch_to(app: &AppHandle, dir: &Path, pointer: &Pointer) -> CmdResult<()> {
    write_pointer(dir, pointer)?;
    prune(app, dir, Some(&dir.join(&pointer.folder)));
    Ok(())
}

/// A folder for a new draft in place of the current one, refused while
/// the current one cannot be read: it would be deleted unseen (A17).
fn new_draft_folder(dir: &Path) -> CmdResult<(String, PathBuf)> {
    read_pointer(dir)?;
    let folder = new_folder(dir);
    let root = dir.join(&folder);
    Ok((folder, root))
}

/// Describes the draft in `root` for the editor.
fn describe(root: &Path, pointer: Pointer, quiz: Quiz) -> EditorDraft {
    EditorDraft {
        problems: quiz_file::problems(&quiz, root),
        id: pointer.folder,
        quiz,
        source_path: pointer.source_path,
        unsaved: pointer.unsaved,
        media_root: root.join("").to_string_lossy().into_owned(),
        dropped: pointer.dropped,
    }
}

/// The draft left from last time, if any.
#[tauri::command]
pub async fn editor_load(app: AppHandle) -> CmdResult<Option<EditorDraft>> {
    with_drafts(app, |_, dir| {
        let Some(pointer) = read_pointer(dir)? else { return Ok(None) };
        let root = dir.join(&pointer.folder);
        let quiz = quiz_file::read_quiz_json(&root)?;
        Ok(Some(describe(&root, pointer, quiz)))
    })
    .await
}

/// Starts a new, empty quiz in place of the draft.
#[tauri::command]
pub async fn editor_new(app: AppHandle, title: String) -> CmdResult<EditorDraft> {
    with_drafts(app, move |app, dir| {
        let (folder, root) = new_draft_folder(dir)?;
        let quiz = Quiz::new(title, Vec::new());
        fs::create_dir_all(&root).map_err(|e| AppError::file(FileAction::CreateFolder, &root, e))?;
        quiz_file::write_quiz_json(&root, &quiz)?;
        let pointer = Pointer { folder, source_path: None, unsaved: false, dropped: Vec::new() };
        switch_to(app, dir, &pointer)?;
        Ok(describe(&root, pointer, quiz))
    })
    .await
}

/// Opens a quiz file in place of the draft: `path`, a recent file, or one
/// the quizmaster picks (none when they cancel). A path a window names is
/// only read, within `quiz_file::open_into`'s limits. Refused during a
/// show, whose journal must not wait for the disk while a large file
/// unpacks (A20).
#[tauri::command]
pub async fn editor_open(app: AppHandle, window: Window, path: Option<String>, picker: Picker) -> CmdResult<Option<EditorDraft>> {
    let path = match path {
        Some(path) => PathBuf::from(path),
        None => match dialogs::pick_file(window, picker, vec![quiz_file::EXTENSION, "json"]).await? {
            Some(path) => path,
            None => return Ok(None),
        },
    };
    with_drafts(app, move |app, dir| {
        if show_running(app) {
            return Err(AppError::ShowRunning);
        }
        let (folder, root) = new_draft_folder(dir)?;
        let opened = quiz_file::open_into(&path, &root)?;
        // A quiz that opening changed has to be saved to keep the change.
        let source_path = Some(path.to_string_lossy().into_owned());
        let pointer = Pointer { folder, source_path, unsaved: opened.changed, dropped: opened.dropped };
        switch_to(app, dir, &pointer)?;
        Ok(Some(describe(&root, pointer, opened.quiz)))
    })
    .await
}

/// Stores the edited quiz in the draft `draft` and returns what keeps it
/// from being played. `unsaved` tells whether it differs from the saved file.
#[tauri::command]
pub async fn editor_update(app: AppHandle, draft: String, quiz: Quiz, unsaved: bool) -> CmdResult<Vec<ValidationError>> {
    on_draft(app, draft, move |_, dir, root, pointer| {
        quiz_file::write_quiz_json(&root, &quiz)?;
        if pointer.unsaved != unsaved {
            write_pointer(dir, &Pointer { unsaved, ..pointer })?;
        }
        Ok(quiz_file::problems(&quiz, &root))
    })
    .await
}

/// Saves the quiz of the draft `draft` with its media as a quiz file: over
/// the quiz file it came from, unless `save_as`, else where the quizmaster
/// picks (none when they cancel; a plain JSON quiz is never written over).
/// Gives the file's path.
#[tauri::command]
pub async fn editor_save(app: AppHandle, window: Window, draft: String, quiz: Quiz, save_as: bool, picker: Picker) -> CmdResult<Option<String>> {
    let source = on_draft(app.clone(), draft.clone(), |_, _, _, pointer| Ok(pointer.source_path.map(PathBuf::from))).await?;
    let path = match source {
        Some(source) if !save_as && is_quiz_file(&source) => source,
        source => match dialogs::pick_save(window, picker, quiz_file::EXTENSION, source).await? {
            Some(path) => path,
            None => return Ok(None),
        },
    };
    on_draft(app, draft, move |_, dir, root, pointer| {
        quiz_file::write_quiz_json(&root, &quiz)?;
        quiz_file::save(&quiz, &root, &path)?;
        let saved = path.to_string_lossy().into_owned();
        write_pointer(dir, &Pointer { source_path: Some(saved.clone()), unsaved: false, dropped: Vec::new(), ..pointer })?;
        Ok(Some(saved))
    })
    .await
}

/// Whether `path` names a quiz file (not a plain JSON quiz).
fn is_quiz_file(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case(quiz_file::EXTENSION))
}

/// Copies a picture, sound or video of `kinds` that the quizmaster picks
/// into the draft `draft` and returns its reference (none when they cancel).
#[tauri::command]
pub async fn editor_add_media(app: AppHandle, window: Window, draft: String, kinds: Vec<MediaKind>, picker: Picker) -> CmdResult<Option<String>> {
    let Some(path) = dialogs::pick_file(window, picker, media_player::extensions(&kinds)).await? else { return Ok(None) };
    media_player::of_kind(&media_player::file_name(&path), &kinds)?;
    on_draft(app, draft, move |_, _, root, _| Ok(Some(quiz_file::import_media(&root, quiz_file::MediaSource::File(&path))?))).await
}

/// A name for a new category that none of `categories` has (spec QZ-10).
#[tauri::command]
pub fn editor_category_name(categories: Vec<Category>, base: String) -> String {
    quiz_core::fresh_category_name(&categories, &base)
}

/// Reads questions from a spreadsheet the quizmaster picks (none when they
/// cancel) and copies the media they use into the draft; the editor
/// inserts the slides as one change (spec QZ-8), giving a row that names
/// one of the draft's categories its look (Z10).
#[tauri::command]
pub async fn editor_import_sheet(app: AppHandle, window: Window, draft: String, picker: Picker) -> CmdResult<Option<SheetImport>> {
    let Some(path) = dialogs::pick_file(window, picker, quiz_file::SHEET_EXTENSIONS.to_vec()).await? else { return Ok(None) };
    // The sheet reader may take until its time limit: the draft stays free
    // for other commands meanwhile.
    let read = path.clone();
    let tables = off_ui(move || crate::sheet_reader::read_tables(&read)).await?;
    on_draft(app, draft, move |_, _, root, _| {
        let categories = quiz_file::read_quiz_json(&root)?.categories;
        // Shuffles the answers of a question with none marked right.
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        Ok(Some(quiz_file::import_sheet(&path, &tables, &categories, &root, seed)?))
    })
    .await
}

/// The character between the cells of a CSV file. Excel splits a CSV file
/// it opens at the system's list separator, a semicolon where the decimal
/// separator is a comma (as in Czech).
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ListSeparator {
    Semicolon,
    Comma,
}

/// Writes the quiz of the draft `draft` as a CSV file the quizmaster picks
/// (none when they cancel), with its media next to it (spec QZ-8).
#[tauri::command]
pub async fn editor_export_sheet(
    app: AppHandle,
    window: Window,
    draft: String,
    quiz: Quiz,
    separator: ListSeparator,
    picker: Picker,
) -> CmdResult<Option<SheetExport>> {
    let separator = match separator {
        ListSeparator::Semicolon => b';',
        ListSeparator::Comma => b',',
    };
    let Some(path) = dialogs::pick_save(window, picker, quiz_file::CSV_EXTENSION, None).await? else { return Ok(None) };
    on_draft(app, draft, move |_, _, root, _| Ok(Some(quiz_file::export_sheet(&quiz, &root, &path, separator)?))).await
}

/// Discards the draft, also one that cannot be read: the quizmaster chose to.
#[tauri::command]
pub async fn editor_close(app: AppHandle) -> CmdResult<()> {
    with_drafts(app, |app, dir| {
        let pointer = dir.join(POINTER_FILE);
        match fs::remove_file(&pointer) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(AppError::file(FileAction::Write, &pointer, e)),
            _ => {}
        }
        prune(app, dir, None);
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("buzzdome-editor-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_command_for_a_draft_that_another_replaced_is_refused() {
        let dir = editor_dir("replaced");
        assert_eq!(current(&dir, "draft-1").unwrap_err(), AppError::NoDraft);
        let pointer = Pointer { folder: "draft-2".into(), source_path: None, unsaved: false, dropped: Vec::new() };
        write_pointer(&dir, &pointer).unwrap();
        assert_eq!(current(&dir, "draft-1").unwrap_err(), AppError::DraftChanged);
        assert_eq!(current(&dir, "draft-2").unwrap().0, dir.join("draft-2"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_pointer_that_does_not_read_is_no_reason_to_start_afresh() {
        let dir = editor_dir("unread");
        assert!(read_pointer(&dir).unwrap().is_none(), "no pointer, no draft");
        for broken in ["{ not json", r#"{"folder":"../elsewhere","sourcePath":null,"unsaved":true}"#] {
            fs::write(dir.join(POINTER_FILE), broken).unwrap();
            assert!(matches!(read_pointer(&dir), Err(AppError::DraftNotRead { .. })), "{broken}");
            assert!(matches!(new_draft_folder(&dir), Err(AppError::DraftNotRead { .. })), "{broken}");
            assert!(matches!(current(&dir, "draft-1"), Err(AppError::DraftNotRead { .. })), "{broken}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
