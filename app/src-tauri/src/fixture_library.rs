//! The fixture library the Lights tab adds fixtures from (spec LT-9): the
//! one built into the app, or a newer one downloaded from the Open Fixture
//! Library and kept in the app's data (`fixture-library.json`, our format).
//! A download goes through the same conversion as the built-in library
//! (`lights::library::archive`), so the same rules keep lasers out and
//! channels off strobes and programs at rest.

use crate::error::{AppError, CmdResult};
use crate::{app_log, off_ui};
use lights::library::archive::{self, ArchiveError};
use lights::library::{Library, LibraryMatches, LibrarySource};
use quiz_file::FileAction;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Manager};
use ts_rs::TS;

/// Where a downloaded library is kept, in the app's data folder.
pub const LIBRARY_FILE: &str = "fixture-library.json";
/// Every fixture definition of the Open Fixture Library, as a zip (its
/// website's "ofl" export).
const DOWNLOAD_URL: &str = "https://open-fixture-library.org/download.ofl";
/// Most bytes a download may have; it had about 4 MB in 2026 [no source -
/// my estimate].
const MAX_DOWNLOAD_BYTES: u64 = 64 << 20;
/// How long a download may take in all; the site makes the zip when asked.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(180);
/// Fewest models a download must give (the built-in library has over 600):
/// fewer means it was cut short or the source changed, so it is not used.
pub const MIN_DOWNLOADED_MODELS: usize = 300;

pub struct FixtureLibrary {
    path: PathBuf,
    /// The library in use, read on first use.
    current: Mutex<Option<Arc<Library>>>,
    /// Held while a download is made, so two do not overwrite each other.
    updating: Mutex<()>,
}

impl FixtureLibrary {
    pub fn new(path: PathBuf) -> FixtureLibrary {
        FixtureLibrary { path, current: Mutex::new(None), updating: Mutex::new(()) }
    }

    /// The library in use; the first time, the kept download unless the
    /// built-in one is newer (an app update brought it) or it does not read.
    fn current(&self) -> Arc<Library> {
        self.current.lock().unwrap().get_or_insert_with(|| Arc::new(load(&self.path))).clone()
    }

    fn set(&self, library: Library) -> Arc<Library> {
        let library = Arc::new(library);
        *self.current.lock().unwrap() = Some(library.clone());
        library
    }
}

fn load(path: &Path) -> Library {
    let built_in = lights::library::built_in();
    let kept = match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str::<Library>(&text).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return built_in.clone(),
        Err(e) => Err(e.to_string()),
    };
    match kept {
        Ok(kept) if kept.source.date_ms > built_in.source.date_ms => kept,
        Ok(_) => built_in.clone(),
        Err(detail) => {
            app_log::note(&format!("downloaded fixture library not read, using the built-in one: {detail}"));
            built_in.clone()
        }
    }
}

/// The library in use, as the Lights tab credits it.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInfo {
    pub source: LibrarySource,
    pub models: usize,
    pub modes: usize,
    /// Downloaded, not the one built into the app.
    pub downloaded: bool,
}

impl LibraryInfo {
    fn of(library: &Library) -> LibraryInfo {
        LibraryInfo { source: library.source.clone(), models: library.fixtures.len(), modes: library.modes(), downloaded: library.source.commit.is_empty() }
    }
}

/// The fixture models of the library in use that match `query`. Off the
/// windows' thread: the library is read the first time it is asked for.
#[tauri::command]
pub async fn lights_library(app: AppHandle, query: String) -> CmdResult<LibraryMatches> {
    off_ui(move || Ok(lights::library::search(&app.state::<FixtureLibrary>().current(), &query))).await
}

/// The library in use: where it comes from, its licence and size.
#[tauri::command]
pub async fn lights_library_info(app: AppHandle) -> CmdResult<LibraryInfo> {
    off_ui(move || Ok(LibraryInfo::of(&app.state::<FixtureLibrary>().current()))).await
}

/// Downloads the Open Fixture Library, converts it and keeps it, then uses
/// it. What is in use stays when any step fails.
#[tauri::command]
pub async fn lights_library_update(app: AppHandle) -> CmdResult<LibraryInfo> {
    off_ui(move || {
        let state = app.state::<FixtureLibrary>();
        let _updating = state.updating.lock().unwrap();
        let zip = download()?;
        let built_in = &lights::library::built_in().source;
        let source = LibrarySource { commit: String::new(), date_ms: unix_ms(), ..built_in.clone() };
        let (library, counts) = archive::from_zip(&zip, source).map_err(|e| match e {
            ArchiveError::NotRead(detail) => AppError::LibraryNotRead { detail },
            ArchiveError::FormatChanged { version } => AppError::LibraryFormatChanged { version },
        })?;
        if library.fixtures.len() < MIN_DOWNLOADED_MODELS {
            return Err(AppError::LibraryTooSmall { models: library.fixtures.len(), least: MIN_DOWNLOADED_MODELS });
        }
        keep(&state.path, &library)?;
        app_log::note(&format!(
            "fixture library downloaded: {} models, {} modes; left out {} redirects, {} lasers, {} without modes, {} unreadable",
            library.fixtures.len(),
            library.modes(),
            counts.redirects,
            counts.lasers,
            counts.without_modes,
            counts.unreadable
        ));
        Ok(LibraryInfo::of(&state.set(library)))
    })
    .await
}

/// Goes back to the library built into the app, removing a kept download.
#[tauri::command]
pub async fn lights_library_reset(app: AppHandle) -> CmdResult<LibraryInfo> {
    off_ui(move || {
        let state = app.state::<FixtureLibrary>();
        let _updating = state.updating.lock().unwrap();
        match std::fs::remove_file(&state.path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(AppError::file(FileAction::Delete, &state.path, e)),
        }
        Ok(LibraryInfo::of(&state.set(lights::library::built_in().clone())))
    })
    .await
}

fn download() -> CmdResult<Vec<u8>> {
    let failed = |e: ureq::Error| AppError::LibraryDownload { detail: e.to_string() };
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(DOWNLOAD_TIMEOUT)).build().into();
    let mut response = agent.get(DOWNLOAD_URL).call().map_err(failed)?;
    response.body_mut().with_config().limit(MAX_DOWNLOAD_BYTES).read_to_vec().map_err(failed)
}

/// Writes `library` to `path` so that a crash leaves the old file or the new one.
fn keep(path: &Path, library: &Library) -> CmdResult<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| AppError::file(FileAction::CreateFolder, dir, e))?;
    }
    Ok(quiz_file::write_atomic(path, library.to_json().as_bytes())?)
}

fn unix_ms() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Where a downloaded library is kept.
pub fn library_path(app: &AppHandle) -> CmdResult<PathBuf> {
    crate::app_data(app, LIBRARY_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fixture-library-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_kept_download_is_used_only_while_it_is_newer_than_the_built_in_library() {
        let dir = dir("kept");
        let path = dir.join(LIBRARY_FILE);
        let built_in = lights::library::built_in();
        assert_eq!(&load(&path), built_in);

        let mut newer = built_in.clone();
        newer.fixtures.truncate(3);
        newer.source.commit.clear();
        newer.source.date_ms = built_in.source.date_ms + 1;
        keep(&path, &newer).unwrap();
        assert_eq!(load(&path), newer);
        assert!(LibraryInfo::of(&newer).downloaded);

        let mut older = newer.clone();
        older.source.date_ms = built_in.source.date_ms - 1;
        keep(&path, &older).unwrap();
        assert_eq!(&load(&path), built_in);

        std::fs::write(&path, "{ not a library").unwrap();
        assert_eq!(&load(&path), built_in);
        assert!(!LibraryInfo::of(built_in).downloaded);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
