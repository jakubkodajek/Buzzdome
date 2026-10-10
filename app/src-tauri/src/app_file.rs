//! Small JSON files the app keeps in its data folder (the receiver settings,
//! the team names): written whole and atomically (S4), and read back so that
//! a broken file is kept aside and reported instead of silently replaced.

use crate::error::{AppError, CmdResult};
use quiz_file::FileAction;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;
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

/// The file at `path`, or the default when there is none. A file that cannot
/// be read gives the default too, and is renamed aside (so what it held can
/// still be recovered by hand) and reported.
pub fn load<T: DeserializeOwned + Default>(path: &Path) -> (T, Option<SettingsLost>) {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (T::default(), None),
        Err(e) => return (T::default(), Some(SettingsLost { detail: e.to_string(), kept_as: None })),
    };
    match serde_json::from_str(&text) {
        Ok(value) => (value, None),
        Err(e) => {
            let aside = path.with_extension("json.broken");
            let kept_as = std::fs::rename(path, &aside).ok().map(|()| aside.display().to_string());
            (T::default(), Some(SettingsLost { detail: e.to_string(), kept_as }))
        }
    }
}

/// Writes `value` to `path`, creating its folder, so that a crash leaves the
/// old file or the new one.
pub fn store<T: Serialize>(path: &Path, value: &T) -> CmdResult<()> {
    let json = serde_json::to_string_pretty(value).map_err(AppError::system)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| AppError::file(FileAction::CreateFolder, dir, e))?;
    }
    Ok(quiz_file::write_atomic(path, json.as_bytes())?)
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
        assert_eq!(load::<Kept>(&path), (Kept::default(), None));
        let kept = Kept { names: vec!["Hawks".into()] };
        store(&path, &kept).unwrap();
        assert_eq!(load::<Kept>(&path), (kept, None));

        std::fs::write(&path, "{ not json").unwrap();
        let (value, lost) = load::<Kept>(&path);
        assert_eq!(value, Kept::default());
        let aside = path.with_extension("json.broken");
        assert_eq!(lost.and_then(|lost| lost.kept_as), Some(aside.display().to_string()));
        assert_eq!(std::fs::read_to_string(&aside).unwrap(), "{ not json");
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
