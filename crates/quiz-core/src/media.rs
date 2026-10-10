//! Media a quiz uses: pictures, sounds and videos stored in the quiz file's
//! `media/` folder and referenced from slides as `media/<name>`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Prefix of every media reference, the folder inside a quiz file.
pub const MEDIA_PREFIX: &str = "media/";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum MediaKind {
    Image,
    Audio,
    Video,
}

impl MediaKind {
    const ALL: [MediaKind; 3] = [MediaKind::Image, MediaKind::Audio, MediaKind::Video];

    /// File types every supported web view can show (lower case).
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            MediaKind::Image => &["png", "jpg", "jpeg", "webp", "gif", "svg"],
            MediaKind::Audio => &["mp3", "wav", "ogg", "m4a"],
            MediaKind::Video => &["mp4", "webm"],
        }
    }

    /// The kind of the file `path` by its extension; none for other files.
    pub fn of(path: &str) -> Option<MediaKind> {
        let extension = extension(path);
        MediaKind::ALL.into_iter().find(|kind| kind.extensions().contains(&extension.as_str()))
    }
}

/// True when `reference` names a file inside the media folder with no way out
/// of it: `media/` followed by portable names (see `is_portable_name`)
/// separated by `/`.
pub fn is_safe_reference(reference: &str) -> bool {
    reference.strip_prefix(MEDIA_PREFIX).is_some_and(is_portable_path)
}

/// True when `path` is portable names (see `is_portable_name`) separated by
/// `/`: relative, and with no way out of the folder it is read from.
pub(crate) fn is_portable_path(path: &str) -> bool {
    path.split('/').all(is_portable_name)
}

/// The extension of the file a `/`-separated path names, in lower case;
/// empty when it has none.
pub(crate) fn extension(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default()
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

/// The safe reference closest to `reference`, for a media name that files
/// made before the portable-name rule may use: each part made portable (see
/// `portable_name`); a safe reference stays as it is. `None` when the name is
/// not plainly inside the media folder (a backslash, or a part of only dots
/// and spaces, which some systems read as `.` or `..`): no renaming makes it
/// safe to read.
pub fn portable_reference(reference: &str) -> Option<String> {
    let name = reference.strip_prefix(MEDIA_PREFIX)?;
    let parts: Vec<&str> = name.split('/').collect();
    if name.contains('\\') || parts.iter().any(|part| part.trim_end_matches(['.', ' ']).is_empty()) {
        return None;
    }
    Some(format!("{MEDIA_PREFIX}{}", parts.into_iter().map(portable_name).collect::<Vec<_>>().join("/")))
}

/// What is wrong with a media reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
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
    let extension = extension(reference);
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

/// Lowest loudness gain of a sound, in dB (SN-9).
pub const MIN_GAIN_DB: i32 = -30;
/// Highest loudness gain of a sound, in dB (SN-9).
pub const MAX_GAIN_DB: i32 = 20;

/// A stretch of a media file, in milliseconds from its start. The default is
/// the whole file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    #[serde(default)]
    pub from_ms: u32,
    /// `None` runs to the end of the file.
    #[serde(default)]
    pub to_ms: Option<u32>,
}

impl Span {
    /// True when nothing would play: it ends where or before it starts.
    pub fn is_empty(&self) -> bool {
        self.to_ms.is_some_and(|to| to <= self.from_ms)
    }
}

/// A sound that belongs to a slide (a music question, a jingle).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SoundClip {
    pub file: String,
    #[serde(default)]
    pub start: SoundStart,
    /// Play `part` again from its start until the slide ends.
    #[serde(default)]
    pub repeat: bool,
    /// Volume in percent, 0 to 100.
    #[serde(default = "full_volume")]
    pub volume: u8,
    /// The part of the file that plays (SN-8).
    #[serde(default)]
    pub part: Span,
    /// Loudness gain in dB, on top of `volume`, that brings the sound to the
    /// quiz's loudness (SN-9). `MIN_GAIN_DB` to `MAX_GAIN_DB`.
    #[serde(default)]
    pub gain_db: f32,
    /// Played once when the answer is revealed; questions only (SN-8).
    #[serde(default)]
    pub replay: Option<Replay>,
}

impl SoundClip {
    /// `file` with the options a quiz file that names only the file gets.
    pub fn new(file: impl Into<String>) -> SoundClip {
        SoundClip { file: file.into(), start: SoundStart::OnSlide, repeat: false, volume: full_volume(), part: Span::default(), gain_db: 0.0, replay: None }
    }
}

/// The part of a question's sound played again once its answer is revealed,
/// such as the chorus of the song the players had to name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Replay {
    #[serde(default)]
    pub part: Span,
    /// Time between the reveal and the start of `part`.
    #[serde(default)]
    pub delay_ms: u32,
}

/// Slowest playback of a video, in percent of its normal speed.
pub const MIN_VIDEO_SPEED: u16 = 25;
/// Fastest playback of a video, in percent of its normal speed.
pub const MAX_VIDEO_SPEED: u16 = 250;
/// Longest wait between showing a slide and starting its video.
pub const MAX_INTRO_PAUSE_MS: u32 = 60_000;
/// The wait before a video an author adds (in the editor or from a
/// spreadsheet) starts, so the quizmaster can read the question first; the
/// reference's default. A file that does not say starts its video at once.
pub const NEW_VIDEO_INTRO_PAUSE_MS: u32 = 1_000;
/// Longest wait between the reveal and a replay part.
pub const MAX_REPLAY_DELAY_MS: u32 = 60_000;

/// A change to a question's video that is strongest when the question
/// starts and clears as its answer time runs out, so the players see more
/// the longer they wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum VideoEffect {
    Blur,
    /// Large square blocks of colour that grow finer.
    Mosaic,
}

/// A video shown on a slide in place of a picture. It pauses and goes on
/// with the show (`Game::on_hold`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct VideoClip {
    pub file: String,
    /// Start by itself, `intro_pause_ms` after the slide is shown; otherwise
    /// the quizmaster starts it from the Director.
    #[serde(default = "yes")]
    pub autoplay: bool,
    /// Time between showing the slide and starting the video, so the
    /// quizmaster can read the question first. Up to `MAX_INTRO_PAUSE_MS`.
    /// Files from before it start the video at once, as they did; a video an
    /// author adds gets `NEW_VIDEO_INTRO_PAUSE_MS`.
    #[serde(default)]
    pub intro_pause_ms: u32,
    #[serde(default)]
    pub muted: bool,
    /// Play `part` again from its start until the slide ends.
    #[serde(default)]
    pub repeat: bool,
    /// Volume in percent, 0 to 100.
    #[serde(default = "full_volume")]
    pub volume: u8,
    /// Playback speed in percent, `MIN_VIDEO_SPEED` to `MAX_VIDEO_SPEED`.
    #[serde(default = "normal_speed")]
    pub speed: u16,
    /// The part of the file that plays.
    #[serde(default)]
    pub part: Span,
    /// Questions only.
    #[serde(default)]
    pub effect: Option<VideoEffect>,
    /// Played once when the answer is revealed; questions only.
    #[serde(default)]
    pub replay: Option<Replay>,
}

impl VideoClip {
    /// `file` with the options a quiz file that names only the file gets.
    pub fn new(file: impl Into<String>) -> VideoClip {
        VideoClip {
            file: file.into(),
            autoplay: true,
            intro_pause_ms: 0,
            muted: false,
            repeat: false,
            volume: full_volume(),
            speed: normal_speed(),
            part: Span::default(),
            effect: None,
            replay: None,
        }
    }
}

/// What sounds and videos have in common: how loud they play, the part of
/// the file that plays and the part replayed after the reveal.
pub trait Playback {
    /// In percent, 0 to 100.
    fn volume(&self) -> u8;
    fn part(&self) -> &Span;
    fn replay(&self) -> Option<&Replay>;
    /// The kind of file it plays.
    fn kind(&self) -> MediaKind;

    /// The parts of the file it plays.
    fn parts(&self) -> impl Iterator<Item = &Span> {
        std::iter::once(self.part()).chain(self.replay().map(|r| &r.part))
    }
}

impl Playback for SoundClip {
    fn volume(&self) -> u8 {
        self.volume
    }
    fn part(&self) -> &Span {
        &self.part
    }
    fn replay(&self) -> Option<&Replay> {
        self.replay.as_ref()
    }
    fn kind(&self) -> MediaKind {
        MediaKind::Audio
    }
}

impl Playback for VideoClip {
    fn volume(&self) -> u8 {
        self.volume
    }
    fn part(&self) -> &Span {
        &self.part
    }
    fn replay(&self) -> Option<&Replay> {
        self.replay.as_ref()
    }
    fn kind(&self) -> MediaKind {
        MediaKind::Video
    }
}

/// The kinds of file a background may be.
pub(crate) const BACKGROUND_KINDS: &[MediaKind] = &[MediaKind::Image, MediaKind::Video];

/// A picture or video behind a slide's content, in place of the theme's
/// (spec section 5). The options apply to videos only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Background {
    pub file: String,
    /// Start again at the end; otherwise the last frame stays.
    #[serde(default = "yes")]
    pub repeat: bool,
    /// Play without the video's sound.
    #[serde(default = "yes")]
    pub muted: bool,
    /// Pause while a player answers, while the clock is paused and once the
    /// question is over (BG-4).
    #[serde(default)]
    pub sync: bool,
}

impl Background {
    /// `file` with the default options: a silent loop.
    pub fn new(file: impl Into<String>) -> Background {
        Background { file: file.into(), repeat: true, muted: true, sync: false }
    }
}

/// A slide's background as `#[serde(with)]` module: written as schema version
/// 3 writes it, read also as the bare file name versions 1 and 2 wrote.
pub(crate) mod background_any_version {
    use super::Background;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(background: &Option<Background>, serializer: S) -> Result<S::Ok, S::Error> {
        background.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Background>, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Stored {
            File(String),
            Full(Background),
        }
        Ok(Option::<Stored>::deserialize(deserializer)?.map(|stored| match stored {
            Stored::File(file) => Background::new(file),
            Stored::Full(background) => background,
        }))
    }
}

fn full_volume() -> u8 {
    100
}

fn yes() -> bool {
    true
}

fn normal_speed() -> u16 {
    100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_a_picture_a_sound_or_a_video_by_its_extension() {
        assert_eq!(MediaKind::of("media/Flag.PNG"), Some(MediaKind::Image));
        assert_eq!(MediaKind::of("Potlesk.mp3"), Some(MediaKind::Audio));
        assert_eq!(MediaKind::of("media/a.b/intro.webm"), Some(MediaKind::Video));
        assert_eq!(MediaKind::of("notes.txt"), None);
        assert_eq!(MediaKind::of("media/mp3"), None);
    }

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
    fn old_media_names_get_the_closest_safe_reference() {
        assert_eq!(portable_reference("media/round 1/flag.png").as_deref(), Some("media/round 1/flag.png"));
        assert_eq!(portable_reference("media/what?.png").as_deref(), Some("media/what_.png"));
        assert_eq!(portable_reference("media/CON.png").as_deref(), Some("media/_CON.png"));
        assert_eq!(portable_reference("media/round 1./a*b.mp3").as_deref(), Some("media/round 1/a_b.mp3"));
        for name in ["media/what?.png", "media/x.", "media/nul .png", "media/C:flag.png", "media/a\tb.png"] {
            assert!(is_safe_reference(&portable_reference(name).unwrap()), "{name}");
        }
        for outside in ["flag.png", "media/../quiz.json", "media/./a.png", "media//a.png", "media/", "media/.. /a.png", "media/a\\..\\b.png"] {
            assert_eq!(portable_reference(outside), None, "{outside}");
        }
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
