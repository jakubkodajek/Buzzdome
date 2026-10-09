//! Media a quiz uses: pictures, sounds and videos stored in the quiz file's
//! `media/` folder and referenced from slides as `media/<name>`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Prefix of every media reference, the folder inside a quiz file.
pub const MEDIA_PREFIX: &str = "media/";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum MediaKind {
    Image,
    Audio,
    Video,
}

impl MediaKind {
    /// File types every supported web view can show (lower case).
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            MediaKind::Image => &["png", "jpg", "jpeg", "webp", "gif", "svg"],
            MediaKind::Audio => &["mp3", "wav", "ogg", "m4a"],
            MediaKind::Video => &["mp4", "webm"],
        }
    }
}

/// True when `reference` names a file inside the media folder with no way out
/// of it: `media/` followed by portable names (see `is_portable_name`)
/// separated by `/`.
pub fn is_safe_reference(reference: &str) -> bool {
    reference.strip_prefix(MEDIA_PREFIX).is_some_and(|name| name.split('/').all(is_portable_name))
}

/// Characters some supported system refuses in a file name.
fn is_forbidden_in_names(c: char) -> bool {
    c.is_control() || "\\/:*?\"<>|".contains(c)
}

/// True when `part` (one folder or file name) can be stored on Windows, macOS
/// and Linux alike: not empty, no forbidden character, no trailing dot or
/// space (which also rules out `.` and `..`), and not a reserved device name.
fn is_portable_name(part: &str) -> bool {
    !part.is_empty() && !part.ends_with(['.', ' ']) && !part.chars().any(is_forbidden_in_names) && !is_reserved_on_windows(part)
}

/// Device names Windows reserves with any extension, per Microsoft's "Naming
/// Files, Paths, and Namespaces": CON, PRN, AUX, NUL, COM0-9, LPT0-9 (and the
/// superscript digits).
fn is_reserved_on_windows(part: &str) -> bool {
    let stem = part.split('.').next().unwrap_or(part).trim_end().to_ascii_uppercase();
    let numbered = |prefix| stem.strip_prefix(prefix).is_some_and(|n: &str| n.chars().count() == 1 && "0123456789¹²³".contains(n));
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") || numbered("COM") || numbered("LPT")
}

/// Turns any file name into a portable one, as close to it as possible, so
/// that `media/<result>` is a safe reference.
pub fn portable_name(name: &str) -> String {
    let replaced: String = name.chars().map(|c| if is_forbidden_in_names(c) { '_' } else { c }).collect();
    let trimmed = replaced.trim_end_matches(['.', ' ']);
    match trimmed {
        "" => "file".into(),
        reserved if is_reserved_on_windows(reserved) => format!("_{reserved}"),
        fine => fine.into(),
    }
}

/// What is wrong with a media reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MediaIssue {
    #[error("not in the {MEDIA_PREFIX} folder")]
    NotInMediaFolder,
    #[error("not a valid media file name")]
    BadName,
    #[error("must be one of {expected:?}")]
    WrongType { expected: Vec<MediaKind> },
    #[error("the file is missing")]
    Missing,
}

/// Checks that `reference` is a safe media file name (see `is_safe_reference`)
/// and that its type is one of `kinds`.
pub fn check_reference(reference: &str, kinds: &[MediaKind]) -> Result<(), MediaIssue> {
    if !reference.starts_with(MEDIA_PREFIX) {
        return Err(MediaIssue::NotInMediaFolder);
    }
    if !is_safe_reference(reference) {
        return Err(MediaIssue::BadName);
    }
    let name = &reference[MEDIA_PREFIX.len()..];
    let extension = name.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default();
    if kinds.iter().any(|kind| kind.extensions().contains(&extension.as_str())) {
        return Ok(());
    }
    Err(MediaIssue::WrongType { expected: kinds.to_vec() })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum SoundStart {
    /// When the slide is shown.
    #[default]
    OnSlide,
    /// When the question clock starts; stops when the question closes.
    WithClock,
    /// When the quizmaster plays it from the Director.
    OnDemand,
}

/// A sound that belongs to a slide (a music question, a jingle).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SoundClip {
    pub file: String,
    #[serde(default)]
    pub start: SoundStart,
    /// Play again from the start until the slide ends.
    #[serde(default)]
    pub repeat: bool,
    /// Volume in percent, 0 to 100.
    #[serde(default = "full_volume")]
    pub volume: u8,
}

/// A video shown on a slide in place of a picture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct VideoClip {
    pub file: String,
    /// Start playing when the slide is shown.
    #[serde(default = "yes")]
    pub autoplay: bool,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub repeat: bool,
}

fn full_volume() -> u8 {
    100
}

fn yes() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMAGE: &[MediaKind] = &[MediaKind::Image];

    #[test]
    fn accepts_files_inside_the_media_folder() {
        assert!(check_reference("media/flag.png", IMAGE).is_ok());
        assert!(check_reference("media/round 1/Flag.JPG", IMAGE).is_ok());
        assert!(check_reference("media/song.mp3", &[MediaKind::Audio]).is_ok());
        assert!(check_reference("media/loop.mp4", &[MediaKind::Image, MediaKind::Video]).is_ok());
    }

    #[test]
    fn refuses_paths_that_leave_the_media_folder() {
        for bad in ["flag.png", "/media/flag.png", "media\\flag.png"] {
            assert_eq!(check_reference(bad, IMAGE), Err(MediaIssue::NotInMediaFolder), "{bad}");
        }
        for bad in ["media/../quiz.png", "media/./flag.png", "media//flag.png", "media/", "media/C:flag.png"] {
            assert_eq!(check_reference(bad, IMAGE), Err(MediaIssue::BadName), "{bad}");
        }
    }

    #[test]
    fn refuses_names_some_system_cannot_store() {
        for bad in ["media/a?.png", "media/a*b.png", "media/x\".png", "media/x.", "media/x ", "media/CON.png", "media/com1", "media/Lpt9.mp3", "media/nul .png"] {
            assert_eq!(check_reference(bad, IMAGE), Err(MediaIssue::BadName), "{bad}");
        }
        for fine in ["media/CONSOLE.png", "media/com10.png", "media/a.b.png", "media/ leading.png"] {
            assert!(is_safe_reference(fine), "{fine}");
        }
    }

    #[test]
    fn portable_names_are_safe_references() {
        let names = ["Flag.png", "we:ird?.jpg", "CON.png", "aux", "dots...", " . ", "", "a/b\\c.mp3", "tab\there", "COM¹.wav"];
        for name in names {
            let portable = portable_name(name);
            assert!(is_safe_reference(&format!("{MEDIA_PREFIX}{portable}")), "{name:?} -> {portable:?}");
        }
        assert_eq!(portable_name("Flag.png"), "Flag.png");
        assert_eq!(portable_name("we:ird?.jpg"), "we_ird_.jpg");
        assert_eq!(portable_name("CON.png"), "_CON.png");
    }

    #[test]
    fn refuses_the_wrong_file_type() {
        let wrong = Err(MediaIssue::WrongType { expected: vec![MediaKind::Image] });
        assert_eq!(check_reference("media/song.mp3", IMAGE), wrong);
        assert_eq!(check_reference("media/noextension", IMAGE), wrong);
    }

    /// The UI's file pickers and players need the extensions too. ts-rs
    /// exports types only, so this writes them next to the generated types.
    #[test]
    fn export_extensions_for_the_ui() {
        let Ok(dir) = std::env::var("TS_RS_EXPORT_DIR") else { return };
        let list = |kind: MediaKind| kind.extensions().iter().map(|e| format!("\"{e}\"")).collect::<Vec<_>>().join(", ");
        let ts = format!(
            "// Generated by `cargo test` from MediaKind in crates/quiz-core/src/media.rs. Do not edit this file manually.\n\n\
             export const MEDIA_EXTENSIONS = {{\n  image: [{}],\n  audio: [{}],\n  video: [{}],\n}} as const;\n",
            list(MediaKind::Image),
            list(MediaKind::Audio),
            list(MediaKind::Video),
        );
        std::fs::write(std::path::Path::new(&dir).join("MediaExtensions.ts"), ts).unwrap();
    }
}
