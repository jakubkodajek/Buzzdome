//! Small JSON files the app keeps in its data folder (the receiver settings,
//! the team names, the sound board): written whole and atomically (S4), and
//! read back so that a broken file is kept aside and reported instead of
//! silently replaced.

use crate::error::{AppError, CmdResult};
use quiz_file::FileAction;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// A file the app keeps could not be read, so it started without what the
/// file held.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SettingsLost {
    /// The system's message.
    pub detail: String,
    /// Where the unreadable file was kept, if it could be moved aside.
    pub kept_as: Option<String>,
}

/// Most names tried for keeping a broken file aside.
const MAX_ASIDE_NAMES: usize = 100;

/// A file the app keeps, as the app found it when it read it.
#[derive(Debug, Clone)]
pub struct AppFile {
    path: PathBuf,
    /// Why what the file held is not in the app, when it could not be read.
    lost: Option<SettingsLost>,
    /// The file is still there unread (it could not be read, or not be moved
    /// aside), so writing it would replace what it holds.
    unread: bool,
}

impl AppFile {
    /// Reads the file at `path`: what it holds, or the default when there is
    /// none. A file that does not hold what it should is renamed aside (so
    /// the next save does not overwrite it, and what it held can still be
    /// recovered by hand) and reported; the app starts with the default. One
    /// that cannot be read at all (another program holds it, say) is left as
    /// it is, reported, and not written until the app reads it again.
    pub fn load<T: DeserializeOwned + Default>(path: PathBuf) -> (AppFile, T) {
        let (detail, readable) = match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(value) => return (AppFile { path, lost: None, unread: false }, value),
                Err(e) => (e.to_string(), true),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (AppFile { path, lost: None, unread: false }, T::default()),
            Err(e) => (e.to_string(), false),
        };
        let kept_as = readable.then(|| move_aside(&path)).flatten();
        let unread = kept_as.is_none();
        let lost = SettingsLost { detail, kept_as: kept_as.map(|aside| aside.display().to_string()) };
        (AppFile { path, lost: Some(lost), unread }, T::default())
    }

    /// Why what the file held was lost when it was read.
    pub fn lost(&self) -> Option<&SettingsLost> {
        self.lost.as_ref()
    }

    /// Writes `value`, creating the file's folder, so that a crash leaves
    /// the old file or the new one. Refused for a file left unread.
    pub fn store<T: Serialize>(&self, value: &T) -> CmdResult<()> {
        if self.unread {
            return Err(AppError::FileLeftUnread { path: self.path.display().to_string() });
        }
        let json = serde_json::to_string_pretty(value).map_err(AppError::system)?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| AppError::file(FileAction::CreateFolder, dir, e))?;
        }
        Ok(quiz_file::write_atomic(&self.path, json.as_bytes())?)
    }
}

/// Renames the file at `path` to the first free name of `<name>.broken`,
/// `<name>.broken-2`, ..., so an earlier broken file stays too. None when it
/// could not be moved.
fn move_aside(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let free = (1..=MAX_ASIDE_NAMES)
        .map(|n| path.with_file_name(if n == 1 { format!("{name}.broken") } else { format!("{name}.broken-{n}") }))
        .find(|aside| std::fs::symlink_metadata(aside).is_err())?;
    std::fs::rename(path, &free).ok()?;
    Some(free)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Kept {
        names: Vec<String>,
    }

    #[test]
    fn a_file_reads_back_as_written_and_a_broken_one_is_kept_aside() {
        let dir = std::env::temp_dir().join(format!("app-file-{}", std::process::id()));
        let path = dir.join("nested").join("kept.json");
        let (file, value) = AppFile::load::<Kept>(path.clone());
        assert_eq!((value, file.lost()), (Kept::default(), None));
        let kept = Kept { names: vec!["Hawks".into()] };
        file.store(&kept).unwrap();
        let (file, value) = AppFile::load::<Kept>(path.clone());
        assert_eq!((value, file.lost()), (kept, None));

        std::fs::write(&path, "{ not json").unwrap();
        let (file, value) = AppFile::load::<Kept>(path.clone());
        assert_eq!(value, Kept::default());
        let aside = dir.join("nested").join("kept.json.broken");
        assert_eq!(file.lost().and_then(|lost| lost.kept_as.clone()), Some(aside.display().to_string()));
        assert_eq!(std::fs::read_to_string(&aside).unwrap(), "{ not json");
        assert!(!path.exists());
        // Kept aside, the file no longer stands in the way of saving.
        file.store(&Kept::default()).unwrap();

        // A file that is no text at all is kept aside too, next to the first one.
        std::fs::write(&path, b"\xff\xfe{").unwrap();
        let (file, value) = AppFile::load::<Kept>(path.clone());
        let second = dir.join("nested").join("kept.json.broken-2");
        assert_eq!((value, file.lost().and_then(|lost| lost.kept_as.clone())), (Kept::default(), Some(second.display().to_string())));
        assert_eq!(std::fs::read(&second).unwrap(), b"\xff\xfe{");
        assert_eq!(std::fs::read_to_string(&aside).unwrap(), "{ not json");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_that_cannot_be_read_is_left_as_it_is_and_not_written() {
        let dir = std::env::temp_dir().join(format!("app-file-unread-{}", std::process::id()));
        // A folder where the file should be cannot be read as one, as a file another program holds.
        let path = dir.join("kept.json");
        std::fs::create_dir_all(&path).unwrap();
        let (file, value) = AppFile::load::<Kept>(path.clone());
        assert_eq!(value, Kept::default());
        assert_eq!(file.lost().map(|lost| lost.kept_as.clone()), Some(None));
        assert!(path.is_dir(), "left where it was");
        assert_eq!(file.store(&Kept::default()), Err(AppError::FileLeftUnread { path: path.display().to_string() }));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
