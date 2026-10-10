//! The Director's media player and sound board (spec SN-7): a library of
//! media files and the board's pads, kept per computer in the app's data
//! folder and shared by every quiz. Files are copied in, so the library
//! keeps working when the originals move, and removing one from the list
//! deletes the copy. What plays is no show state, so it is not journaled
//! (`quiz_core::MediaPlayer`); the windows hear of it by events. The
//! player's volume, kept with the library, is the level everything the
//! show plays sounds at, set while it plays.
//!
//! Lock order: `MediaState::imports`, then `MediaState::library`, and
//! `MediaState::player_file` before `MediaState::library`. Files are copied
//! holding only `imports`, so the board and the player answer while a big
//! video comes in.

use crate::app_file::AppFile;
use crate::error::{AppError, CmdResult};
use crate::now_ms;
use quiz_core::{
    is_safe_reference, BoardChange, MediaKind, MediaPlayer, NowPlaying, PlayerCommand, SoundBoard, MAX_DROPPED_MB, MAX_PLAYER_VOLUME, MEDIA_PREFIX,
};
use quiz_file::{FileAction, MediaSource};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;

/// The library's folder in the app's data folder: its files in `media/`, the board beside them.
pub const LIBRARY_FOLDER: &str = "library";
const BOARD_FILE: &str = "soundboard.json";
const PLAYER_FILE: &str = "player.json";
/// The header that names a file dropped into the window (percent-encoded, as headers are ASCII).
const DROPPED_NAME_HEADER: &str = "file-name";
/// How long a new volume waits before it is kept, so that the steps of one
/// drag of the slider, which come faster than a disk writes, are written once.
const KEEP_VOLUME_AFTER: Duration = Duration::from_millis(250);

pub struct MediaState {
    root: PathBuf,
    /// Where the sound board is kept.
    board_file: AppFile,
    /// Where the player's volume is kept; held while it is written.
    player_file: Mutex<AppFile>,
    /// The volume changed since it was last kept.
    volume_changed: AtomicBool,
    imports: Mutex<()>,
    library: Mutex<Library>,
}

struct Library {
    /// The media files in the library (`media/<name>`), in the order the list shows them.
    files: Vec<String>,
    board: SoundBoard,
    player: MediaPlayer,
    settings: PlayerSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayerSettings {
    /// Percent, 0 to `MAX_PLAYER_VOLUME`.
    #[serde(default = "full_volume")]
    volume: u8,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        PlayerSettings { volume: full_volume() }
    }
}

fn full_volume() -> u8 {
    MAX_PLAYER_VOLUME
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MediaLibrary {
    /// The library's folder, ending with a path separator, so the windows
    /// can append a file's reference to it.
    pub root: String,
    pub files: Vec<LibraryFile>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFile {
    pub reference: String,
    pub kind: MediaKind,
}

impl MediaState {
    /// The library in `root` as it was left. A board or volume file that
    /// cannot be read is kept aside and the board starts empty, the volume full.
    pub fn open(root: PathBuf) -> MediaState {
        let files = media_files(&root);
        let (board_file, board) = AppFile::load::<SoundBoard>(root.join(BOARD_FILE));
        let (player_file, settings) = AppFile::load::<PlayerSettings>(root.join(PLAYER_FILE));
        for (what, file) in [("sound board", &board_file), ("player volume", &player_file)] {
            if let Some(lost) = file.lost() {
                eprintln!("{what} not read: {} (kept as {:?})", lost.detail, lost.kept_as);
            }
        }
        let board = board.normalized(&files);
        let settings = PlayerSettings { volume: settings.volume.min(MAX_PLAYER_VOLUME) };
        let library = Mutex::new(Library { files, board, player: MediaPlayer::default(), settings });
        MediaState { root, board_file, player_file: Mutex::new(player_file), volume_changed: AtomicBool::new(false), imports: Mutex::new(()), library }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn view(&self, library: &Library) -> MediaLibrary {
        let files = library.files.iter().filter_map(|reference| Some(LibraryFile { reference: reference.clone(), kind: MediaKind::of(reference)? })).collect();
        MediaLibrary { root: self.root.join("").to_string_lossy().into_owned(), files }
    }

    /// Reads the library's files again after some came or went, and tells the windows.
    fn files_changed(&self, app: &AppHandle, library: &mut Library) -> CmdResult<()> {
        library.files = media_files(&self.root);
        library.board.library_changed(&library.files);
        library.player.library_changed(&library.files);
        let _ = app.emit("media-library", self.view(library));
        let _ = app.emit("media-playing", library.player.now());
        self.board_changed(app, library)
    }

    fn board_changed(&self, app: &AppHandle, library: &Library) -> CmdResult<()> {
        let _ = app.emit("sound-board", &library.board);
        self.board_file.store(&library.board)
    }

    /// Sets the player's volume, at most `MAX_PLAYER_VOLUME` percent, and
    /// `tell`s it holding the lock, so the windows hear the volumes in the
    /// order they were set. True when it was kept before, so this change is
    /// the one to keep it again; the changes after it, till then, are kept with it.
    fn set_volume(&self, volume: u8, tell: impl FnOnce(u8)) -> bool {
        let mut library = self.library.lock().unwrap();
        library.settings.volume = volume.min(MAX_PLAYER_VOLUME);
        tell(library.settings.volume);
        !self.volume_changed.swap(true, Ordering::SeqCst)
    }

    /// Writes the volume as it is now, for every change since it was last written.
    fn keep_volume(&self) -> CmdResult<()> {
        let file = self.player_file.lock().unwrap();
        // Cleared before the volume is read: a change after the read keeps itself.
        self.volume_changed.store(false, Ordering::SeqCst);
        let settings = self.library.lock().unwrap().settings;
        file.store(&settings)
    }
}

/// The media files in the library at `root`, sorted by name whatever its case.
fn media_files(root: &Path) -> Vec<String> {
    let entries = std::fs::read_dir(root.join(MEDIA_PREFIX)).into_iter().flatten().flatten();
    let mut files: Vec<String> = entries
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .map(|name| format!("{MEDIA_PREFIX}{name}"))
        // Leaves out what is no media, and copies still coming in (`.part`).
        .filter(|reference| is_safe_reference(reference) && MediaKind::of(reference).is_some())
        .collect();
    files.sort_by_cached_key(|reference| (reference.to_lowercase(), reference.clone()));
    files
}

/// Copies `source`, named `name`, into the library at `root`.
fn import(root: &Path, name: &str, source: MediaSource) -> CmdResult<String> {
    if MediaKind::of(name).is_none() {
        return Err(AppError::NotAMediaFile { name: name.to_string() });
    }
    Ok(quiz_file::import_media(root, source)?)
}

#[tauri::command]
pub fn media_library(state: State<MediaState>) -> MediaLibrary {
    state.view(&state.library.lock().unwrap())
}

/// Copies files into the library and returns their references in order.
/// Each file that can be copied is; the first failure is reported after.
#[tauri::command]
pub async fn media_add(app: AppHandle, state: State<'_, MediaState>, paths: Vec<String>) -> CmdResult<Vec<String>> {
    let _one_at_a_time = state.imports.lock().unwrap();
    let mut added = Vec::new();
    let mut failed = None;
    for path in paths.iter().map(Path::new) {
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        match import(&state.root, &name, MediaSource::File(path)) {
            Ok(reference) => added.push(reference),
            Err(e) => {
                failed.get_or_insert(e);
            }
        }
    }
    state.files_changed(&app, &mut state.library.lock().unwrap())?;
    failed.map_or(Ok(added), Err)
}

/// Copies a file dropped into the window, sent as its bytes with its name
/// in a header, into the library and returns its reference.
#[tauri::command]
pub async fn media_add_dropped(app: AppHandle, state: State<'_, MediaState>, request: Request<'_>) -> CmdResult<String> {
    let InvokeBody::Raw(bytes) = request.body() else { return Err(AppError::system("a dropped file comes as its bytes")) };
    let encoded = request.headers().get(DROPPED_NAME_HEADER).and_then(|value| value.to_str().ok()).unwrap_or_default();
    let name = percent_encoding::percent_decode_str(encoded).decode_utf8_lossy();
    if bytes.len() as u64 > MAX_DROPPED_MB << 20 {
        return Err(AppError::DroppedFileTooLarge { name: name.into_owned(), mb: MAX_DROPPED_MB });
    }
    let _one_at_a_time = state.imports.lock().unwrap();
    let reference = import(&state.root, &name, MediaSource::Bytes { name: &name, bytes })?;
    state.files_changed(&app, &mut state.library.lock().unwrap())?;
    Ok(reference)
}

/// Removes a file from the library, deleting its copy, and from its pads.
#[tauri::command]
pub fn media_remove(app: AppHandle, state: State<MediaState>, reference: String) -> CmdResult<()> {
    let mut library = state.library.lock().unwrap();
    if !library.files.contains(&reference) {
        return Err(AppError::NotInLibrary { reference });
    }
    let path = state.root.join(&reference);
    let removed = std::fs::remove_file(&path).map_err(|e| AppError::file(FileAction::Delete, &path, e));
    // Whatever happened to the file, the list shows what is there.
    state.files_changed(&app, &mut library)?;
    removed
}

#[tauri::command]
pub fn sound_board(state: State<MediaState>) -> SoundBoard {
    state.library.lock().unwrap().board.clone()
}

#[tauri::command]
pub fn sound_board_change(app: AppHandle, state: State<MediaState>, change: BoardChange) -> CmdResult<()> {
    let mut library = state.library.lock().unwrap();
    let Library { files, board, .. } = &mut *library;
    board.apply(change, files)?;
    state.board_changed(&app, &library)
}

/// What the media player plays, if anything.
#[tauri::command]
pub fn media_now(state: State<MediaState>) -> Option<NowPlaying> {
    state.library.lock().unwrap().player.now().cloned()
}

#[tauri::command]
pub fn media_control(app: AppHandle, state: State<MediaState>, command: PlayerCommand) -> CmdResult<()> {
    let at_ms = now_ms();
    let mut library = state.library.lock().unwrap();
    let Library { files, board, player, .. } = &mut *library;
    player.apply(command, board, files, at_ms)?;
    let _ = app.emit("media-playing", player.now());
    Ok(())
}

/// The player's volume in percent.
#[tauri::command]
pub fn media_volume(state: State<MediaState>) -> u8 {
    state.library.lock().unwrap().settings.volume
}

/// Sets the player's volume, at most `MAX_PLAYER_VOLUME` percent; the
/// windows change what plays at once. It is kept for the next start a
/// moment later, off the main thread, with the changes that come meanwhile.
#[tauri::command]
pub async fn media_set_volume(app: AppHandle, state: State<'_, MediaState>, volume: u8) -> CmdResult<()> {
    let told = |volume| {
        let _ = app.emit("media-volume", volume);
    };
    if !state.set_volume(volume, told) {
        return Ok(());
    }
    tauri::async_runtime::spawn_blocking(move || {
        thread::sleep(KEEP_VOLUME_AFTER);
        app.state::<MediaState>().keep_volume()
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_library_lists_its_media_files_by_name_and_keeps_its_board() {
        let root = std::env::temp_dir().join(format!("media-library-{}", std::process::id()));
        let media = root.join(MEDIA_PREFIX);
        std::fs::create_dir_all(media.join("folder.mp3")).unwrap();
        for name in ["b.mp3", "A.png", "c.MP4", "notes.txt", "d.wav.part"] {
            std::fs::write(media.join(name), b"x").unwrap();
        }
        let state = MediaState::open(root.clone());
        let library = state.library.lock().unwrap();
        assert_eq!(library.files, ["media/A.png", "media/b.mp3", "media/c.MP4"]);
        let view = state.view(&library);
        assert_eq!(view.files[2], LibraryFile { reference: "media/c.MP4".into(), kind: MediaKind::Video });
        assert!(view.root.ends_with(std::path::MAIN_SEPARATOR));
        // A new board fills itself.
        assert_eq!(library.board.pad(1).unwrap().source, Some(quiz_core::PadSource::File { reference: "media/b.mp3".into() }));
        drop(library);

        assert_eq!(library_volume(&root), MAX_PLAYER_VOLUME);

        let board = SoundBoard { click: quiz_core::PadClick::Pause, ..SoundBoard::default() };
        AppFile::load::<SoundBoard>(root.join(BOARD_FILE)).0.store(&board).unwrap();
        assert_eq!(MediaState::open(root.clone()).library.lock().unwrap().board.click, quiz_core::PadClick::Pause);
        // The volume is kept, and one past full (a file edited by hand) plays at full.
        for (kept, read) in [(40, 40), (250, MAX_PLAYER_VOLUME)] {
            std::fs::write(root.join(PLAYER_FILE), format!(r#"{{"volume": {kept}}}"#)).unwrap();
            assert_eq!(library_volume(&root), read);
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_steps_of_one_drag_are_kept_by_its_first_step_with_the_last_volume() {
        let root = std::env::temp_dir().join(format!("media-volume-{}", std::process::id()));
        let state = MediaState::open(root.clone());
        let set = |volume| state.set_volume(volume, |_| ());
        assert!(set(40));
        assert!(!set(30));
        assert!(!set(250));
        state.keep_volume().unwrap();
        assert_eq!(library_volume(&root), MAX_PLAYER_VOLUME);
        // Kept, the next change keeps itself again.
        assert!(set(20));
        state.keep_volume().unwrap();
        assert_eq!(library_volume(&root), 20);
        std::fs::remove_dir_all(&root).unwrap();
    }

    fn library_volume(root: &Path) -> u8 {
        MediaState::open(root.to_path_buf()).library.lock().unwrap().settings.volume
    }
}
