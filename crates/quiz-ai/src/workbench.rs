//! What an assistant does with quizzes on this computer, for the MCP tools
//! and the command line alike. A quiz is worked on as a folder, `quiz.json`
//! with its media in `media/` (docs/quiz-format.md), which the app opens as
//! it is; `pack` makes a `.quiz` file of it to hand on or play.

use quiz_core::{MediaKind, Quiz, ReadQuiz, SlideType, ValidationError};
use quiz_file::{FileAction, MediaSource, QuizFileError, EXTENSION};
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Most media files `list_media` lists.
pub const MAX_LISTED: usize = 1000;
/// How deep below its folder `list_media` looks.
pub const MAX_LIST_DEPTH: usize = 4;
/// Largest picture `picture` hands over. Assistants take pictures of up to
/// 5 MB as base64, which is a third larger than the file.
pub const MAX_PICTURE_BYTES: u64 = 3 << 20;
/// What `picture` hands over: the types assistants read, by extension.
const PICTURE_TYPES: [(&str, &str); 5] =
    [("png", "image/png"), ("jpg", "image/jpeg"), ("jpeg", "image/jpeg"), ("gif", "image/gif"), ("webp", "image/webp")];

#[derive(Debug, thiserror::Error)]
pub enum WorkError {
    #[error(transparent)]
    File(#[from] QuizFileError),
    #[error("{} exists already", .0.display())]
    Exists(PathBuf),
    #[error("{} is not a .{EXTENSION} file", .0.display())]
    NotQuizFile(PathBuf),
    #[error("{} is not a picture, sound or video the app plays", .0.display())]
    NotMedia(PathBuf),
    #[error("{} is not a PNG, JPEG, GIF or WebP picture", .0.display())]
    NotPicture(PathBuf),
    #[error("{} is larger than {} MB", .0.display(), MAX_PICTURE_BYTES >> 20)]
    PictureTooLarge(PathBuf),
}

impl WorkError {
    fn io(action: FileAction, path: &Path, source: io::Error) -> WorkError {
        WorkError::File(QuizFileError::Io { action, path: path.to_path_buf(), source })
    }
}

pub type Result<T> = std::result::Result<T, WorkError>;

/// A quiz and what keeps it from being saved or played.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub title: String,
    pub slides: usize,
    pub questions: usize,
    /// What keeps the quiz from being saved, and most of it from being
    /// played (`quiz_file::problems`); empty when it is ready.
    pub problems: Vec<String>,
    /// What reading the quiz left out because it does not fit the format
    /// (`ReadQuiz::dropped`), such as a colour that is not hex.
    pub left_out: Vec<String>,
}

impl Check {
    fn of(read: &ReadQuiz, root: &Path) -> Check {
        let words = |errors: &[ValidationError]| errors.iter().map(ToString::to_string).collect();
        let quiz = &read.quiz;
        Check {
            title: quiz.title.clone(),
            slides: quiz.slides.len(),
            questions: quiz.slides.iter().filter(|slide| slide.content.slide_type() == SlideType::Question).count(),
            problems: words(&quiz_file::problems(quiz, root)),
            left_out: words(&read.dropped),
        }
    }

    /// Whether the quiz can be saved and played as it is.
    pub fn is_ready(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Checks the quiz at `path`: a quiz folder, or a quiz file (`.quiz`, or a
/// plain quiz JSON with its media next to it), which is unpacked into a
/// temporary folder to check its media.
pub fn check(path: &Path) -> Result<Check> {
    if path.is_dir() {
        return Ok(Check::of(&quiz_file::read_quiz(path)?, path));
    }
    let temp = temp_dir("check");
    // The quiz as the editor opens it, which gives media whose names do not
    // work on every system names that do.
    let checked = quiz_file::open_into(path, &temp)
        .map_err(WorkError::from)
        .map(|opened| Check::of(&ReadQuiz { quiz: opened.quiz, dropped: opened.dropped }, &temp));
    let _ = fs::remove_dir_all(&temp);
    checked
}

/// Writes `json`, a whole quiz, as the `quiz.json` of the folder `folder`,
/// which is made if missing; media stay as they are. What does not fit the
/// format is left out (`Check::left_out`); JSON that is no quiz at all is
/// refused and nothing is written.
pub fn write_quiz(folder: &Path, json: &str) -> Result<Check> {
    let read = Quiz::read_json(json).map_err(QuizFileError::from)?;
    fs::create_dir_all(folder).map_err(|e| WorkError::io(FileAction::CreateFolder, folder, e))?;
    quiz_file::write_quiz_json(folder, &read.quiz)?;
    Ok(Check::of(&read, folder))
}

/// A media file copied into a quiz folder.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Added {
    pub from: PathBuf,
    /// What the quiz names it by, `media/<name>`.
    pub reference: String,
}

/// Copies the media files `files` into the folder `folder`, which is made
/// if missing, and returns what the quiz names each by. A file with a name
/// that does not work on every system gets the closest one that does, and a
/// different file of the same name a number (`quiz_file::import_media`).
/// Nothing is copied unless every file is one the app plays.
pub fn add_media(folder: &Path, files: &[PathBuf]) -> Result<Vec<Added>> {
    if let Some(file) = files.iter().find(|file| !file.is_file() || media_kind(file).is_none()) {
        return Err(WorkError::NotMedia(file.clone()));
    }
    files
        .iter()
        .map(|from| {
            let reference = quiz_file::import_media(folder, MediaSource::File(from))?;
            Ok(Added { from: from.clone(), reference })
        })
        .collect()
}

/// Makes the quiz file `dest` (`.quiz`) of the quiz folder `folder`, with
/// the media its slides use. A quiz with problems is refused: they are
/// listed, and the file is not written.
pub fn pack(folder: &Path, dest: &Path) -> Result<Check> {
    if !dest.extension().is_some_and(|extension| extension.eq_ignore_ascii_case(EXTENSION)) {
        return Err(WorkError::NotQuizFile(dest.to_path_buf()));
    }
    let read = quiz_file::read_quiz(folder)?;
    quiz_file::save(&read.quiz, folder, dest)?;
    Ok(Check::of(&read, folder))
}

/// Unpacks the quiz file `file` into the folder `folder` to work on it.
/// The folder must not exist yet: nothing is ever unpacked over a quiz.
pub fn unpack(file: &Path, folder: &Path) -> Result<Check> {
    if fs::symlink_metadata(folder).is_ok() {
        return Err(WorkError::Exists(folder.to_path_buf()));
    }
    quiz_file::open_into(file, folder)?;
    Ok(Check::of(&quiz_file::read_quiz(folder)?, folder))
}

/// The `quiz.json` of the quiz folder `folder`, as it is written.
pub fn read_quiz(folder: &Path) -> Result<String> {
    let path = folder.join(quiz_file::QUIZ_JSON);
    fs::read_to_string(&path).map_err(|e| WorkError::io(FileAction::Read, &path, e))
}

/// A media file found on this computer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFile {
    pub path: PathBuf,
    pub kind: MediaKind,
    pub bytes: u64,
}

/// The media files in a folder.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub files: Vec<MediaFile>,
    /// More files are there than `MAX_LISTED`, or deeper than `MAX_LIST_DEPTH`.
    pub truncated: bool,
}

/// The pictures, sounds and videos the app plays in `folder` and the
/// folders in it, up to `MAX_LIST_DEPTH` down, by path. Hidden files and
/// folders are passed over.
pub fn list_media(folder: &Path) -> Result<Listing> {
    let mut listing = Listing { files: Vec::new(), truncated: false };
    let mut folders = vec![(folder.to_path_buf(), 0)];
    while let Some((dir, depth)) = folders.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| WorkError::io(FileAction::Read, &dir, e))?;
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                if depth < MAX_LIST_DEPTH {
                    folders.push((path, depth + 1));
                } else {
                    listing.truncated = true;
                }
            } else if let Some(kind) = media_kind(&path) {
                if listing.files.len() == MAX_LISTED {
                    listing.truncated = true;
                    break;
                }
                let bytes = entry.metadata().map_or(0, |meta| meta.len());
                listing.files.push(MediaFile { path, kind, bytes });
            }
        }
    }
    listing.files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(listing)
}

/// A picture to show the assistant.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub bytes: Vec<u8>,
    pub mime_type: &'static str,
}

/// Reads the picture `path` for the assistant to look at.
pub fn picture(path: &Path) -> Result<Picture> {
    let extension = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let Some(&(_, mime_type)) = PICTURE_TYPES.iter().find(|(known, _)| *known == extension) else {
        return Err(WorkError::NotPicture(path.to_path_buf()));
    };
    let size = fs::metadata(path).map_err(|e| WorkError::io(FileAction::Read, path, e))?.len();
    if size > MAX_PICTURE_BYTES {
        return Err(WorkError::PictureTooLarge(path.to_path_buf()));
    }
    let bytes = fs::read(path).map_err(|e| WorkError::io(FileAction::Read, path, e))?;
    Ok(Picture { bytes, mime_type })
}

fn media_kind(path: &Path) -> Option<MediaKind> {
    MediaKind::of(&path.file_name()?.to_string_lossy())
}

/// A folder name in the system's temporary folder that nothing has.
fn temp_dir(purpose: &str) -> PathBuf {
    static MADE: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!("buzzdome-{purpose}-{}-{}", std::process::id(), MADE.fetch_add(1, Ordering::Relaxed)))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    /// An empty folder of its own for each test.
    pub(crate) fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("buzzdome-quiz-ai-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    pub(crate) fn quiz_with_picture(picture: &str) -> String {
        json!({
            "schemaVersion": quiz_core::SCHEMA_VERSION,
            "title": "Capitals",
            "slides": [
                { "id": "r1", "type": "billboard", "title": "Round 1" },
                { "id": "q1", "type": "question", "mode": "everyone", "text": "Capital of Moravia?",
                  "answers": ["Brno", "Olomouc"], "correct": [{ "answer": 0 }], "picture": picture },
                { "id": "end", "type": "endOfRound", "title": "End of round 1" }
            ]
        })
        .to_string()
    }

    #[test]
    fn a_quiz_written_with_its_media_packs_and_unpacks_unchanged() {
        let dir = scratch("roundtrip");
        let photo = dir.join("brno photo.jpg");
        fs::write(&photo, b"jpeg").unwrap();
        let folder = dir.join("quiz");
        let added = add_media(&folder, std::slice::from_ref(&photo)).unwrap();
        assert_eq!(added, vec![Added { from: photo, reference: "media/brno photo.jpg".into() }]);

        let check = write_quiz(&folder, &quiz_with_picture(&added[0].reference)).unwrap();
        assert!(check.is_ready(), "{check:?}");
        assert_eq!((check.slides, check.questions), (3, 1));

        let file = dir.join("capitals.quiz");
        pack(&folder, &file).unwrap();
        assert!(check_is_ready(&file));
        let copy = dir.join("copy");
        unpack(&file, &copy).unwrap();
        assert_eq!(fs::read(copy.join("media/brno photo.jpg")).unwrap(), b"jpeg");
        assert_eq!(Quiz::from_json(&read_quiz(&copy).unwrap()).unwrap(), Quiz::from_json(&read_quiz(&folder).unwrap()).unwrap());
        assert!(matches!(unpack(&file, &copy), Err(WorkError::Exists(_))));
    }

    fn check_is_ready(path: &Path) -> bool {
        check(path).unwrap().is_ready()
    }

    #[test]
    fn a_quiz_with_problems_is_written_but_not_packed() {
        let dir = scratch("problems");
        let folder = dir.join("quiz");
        let check = write_quiz(&folder, &quiz_with_picture("media/missing.png")).unwrap();
        assert_eq!(check.problems, vec!["slide q1: media file media/missing.png: the file is missing".to_string()]);
        let file = dir.join("broken.quiz");
        assert!(matches!(pack(&folder, &file), Err(WorkError::File(QuizFileError::Invalid(_)))));
        assert!(!file.exists());
        assert!(matches!(pack(&folder, &dir.join("broken.zip")), Err(WorkError::NotQuizFile(_))));
    }

    #[test]
    fn json_that_is_no_quiz_writes_nothing() {
        let dir = scratch("no-quiz");
        let folder = dir.join("quiz");
        assert!(write_quiz(&folder, r#"{ "title": "No version" }"#).is_err());
        assert!(write_quiz(&folder, "[1, 2]").is_err());
        assert!(!folder.exists());
    }

    #[test]
    fn what_does_not_fit_the_format_is_left_out_and_said() {
        let dir = scratch("left-out");
        let mut quiz: serde_json::Value = serde_json::from_str(&quiz_with_picture("media/x.png")).unwrap();
        quiz["slides"][0]["style"] = json!({ "headline": { "color": "gold" } });
        quiz["slides"][1].as_object_mut().unwrap().remove("picture");
        let check = write_quiz(&dir, &quiz.to_string()).unwrap();
        assert!(check.is_ready());
        assert_eq!(check.left_out, vec!["slide r1: the style value style.headline cannot be read".to_string()]);
    }

    #[test]
    fn only_media_the_app_plays_are_copied_and_none_when_one_is_not() {
        let dir = scratch("not-media");
        let (song, notes) = (dir.join("song.mp3"), dir.join("notes.txt"));
        fs::write(&song, b"mp3").unwrap();
        fs::write(&notes, b"text").unwrap();
        let folder = dir.join("quiz");
        assert!(matches!(add_media(&folder, &[song.clone(), notes]), Err(WorkError::NotMedia(_))));
        assert!(matches!(add_media(&folder, &[dir.join("gone.mp3")]), Err(WorkError::NotMedia(_))));
        assert!(!folder.exists());
        // The same file twice is one file; another of the same name gets a number.
        let other = dir.join("other");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("song.mp3"), b"another").unwrap();
        let added = add_media(&folder, &[song.clone(), song, other.join("song.mp3")]).unwrap();
        let references: Vec<&str> = added.iter().map(|a| a.reference.as_str()).collect();
        assert_eq!(references, ["media/song.mp3", "media/song.mp3", "media/song-2.mp3"]);
    }

    #[test]
    fn media_are_listed_by_path_without_hidden_files_or_others() {
        let dir = scratch("list");
        fs::create_dir_all(dir.join("round 2/.cache")).unwrap();
        for file in ["a.PNG", "notes.txt", ".hidden.mp3", "round 2/clip.mp4", "round 2/.cache/x.mp3"] {
            fs::write(dir.join(file), b"x").unwrap();
        }
        let listing = list_media(&dir).unwrap();
        let found: Vec<(PathBuf, MediaKind)> = listing.files.into_iter().map(|f| (f.path, f.kind)).collect();
        assert_eq!(found, vec![(dir.join("a.PNG"), MediaKind::Image), (dir.join("round 2/clip.mp4"), MediaKind::Video)]);
        assert!(!listing.truncated);
    }

    #[test]
    fn a_picture_is_handed_over_only_when_an_assistant_reads_it() {
        let dir = scratch("picture");
        fs::write(dir.join("p.JPG"), b"jpeg").unwrap();
        fs::write(dir.join("p.svg"), b"<svg/>").unwrap();
        assert_eq!(picture(&dir.join("p.JPG")).unwrap(), Picture { bytes: b"jpeg".to_vec(), mime_type: "image/jpeg" });
        assert!(matches!(picture(&dir.join("p.svg")), Err(WorkError::NotPicture(_))));
        let big = fs::File::create(dir.join("big.png")).unwrap();
        big.set_len(MAX_PICTURE_BYTES + 1).unwrap();
        assert!(matches!(picture(&dir.join("big.png")), Err(WorkError::PictureTooLarge(_))));
    }
}
