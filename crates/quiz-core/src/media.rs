//! Media a quiz uses: pictures, sounds and videos stored in the quiz file's
//! `media/` folder and referenced from slides as `media/<name>`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Prefix of every media reference, the folder inside a quiz file.
pub const MEDIA_PREFIX: &str = "media/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    fn name(self) -> &'static str {
        match self {
            MediaKind::Image => "picture",
            MediaKind::Audio => "sound",
            MediaKind::Video => "video",
        }
    }
}

/// True when `reference` names a file inside the media folder with no way out
/// of it: `media/` followed by plain names separated by `/`.
pub fn is_safe_reference(reference: &str) -> bool {
    let Some(name) = reference.strip_prefix(MEDIA_PREFIX) else { return false };
    let plain = |part: &str| !part.is_empty() && part != "." && part != "..";
    name.split('/').all(plain) && !reference.contains(['\\', ':']) && !reference.chars().any(char::is_control)
}

/// Checks that `reference` is a safe media file name (see `is_safe_reference`)
/// and that its type is one of `kinds`.
pub fn check_reference(reference: &str, kinds: &[MediaKind]) -> Result<(), String> {
    if !reference.starts_with(MEDIA_PREFIX) {
        return Err(format!("\"{reference}\" is not in the {MEDIA_PREFIX} folder"));
    }
    if !is_safe_reference(reference) {
        return Err(format!("\"{reference}\" is not a valid media file name"));
    }
    let name = &reference[MEDIA_PREFIX.len()..];
    let extension = name.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default();
    if kinds.iter().any(|kind| kind.extensions().contains(&extension.as_str())) {
        return Ok(());
    }
    let expected: Vec<String> = kinds
        .iter()
        .map(|kind| format!("{} ({})", kind.name(), kind.extensions().join(", ")))
        .collect();
    Err(format!("\"{reference}\" must be a {}", expected.join(" or ")))
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
        for bad in [
            "flag.png",
            "/media/flag.png",
            "media/../quiz.png",
            "media/./flag.png",
            "media//flag.png",
            "media/",
            "media\\flag.png",
            "media/C:flag.png",
        ] {
            assert!(check_reference(bad, IMAGE).is_err(), "{bad}");
        }
    }

    #[test]
    fn refuses_the_wrong_file_type() {
        assert!(check_reference("media/song.mp3", IMAGE).is_err());
        assert!(check_reference("media/noextension", IMAGE).is_err());
    }
}
