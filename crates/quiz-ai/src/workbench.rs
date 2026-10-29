//! What an assistant does with quizzes on this computer, for the MCP tools
//! and the command line alike. A quiz is worked on as a folder, `quiz.json`
//! with its media in `media/` (docs/quiz-format.md), which the app opens as
//! it is; `pack` makes a `.quiz` file of it to hand on or play.

use quiz_core::{MediaKind, Quiz, ReadQuiz, SlideType, ValidationError};
use quiz_file::{FileAction, MediaSource, QuizFileError, EXTENSION};
use serde::Serialize;
use std::collections::VecDeque;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Most media files `list_media` lists.
pub const MAX_LISTED: usize = 1000;
/// How deep below its folder `list_media` looks.
pub const MAX_LIST_DEPTH: usize = 4;
/// Largest picture `picture` hands over. Assistants take pictures of up to
/// 5 MB as base64, which is a third larger than the file.
pub const MAX_PICTURE_BYTES: u64 = 3 << 20;
/// What `picture` hands over: the types assistants read, by extension.
const PICTURE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];

#[derive(Debug, thiserror::Error)]
pub enum WorkError {
    #[error(transparent)]
    File(#[from] QuizFileError),
    #[error("{} exists already", .0.display())]
    Exists(PathBuf),
    #[error("{} holds other files than a quiz: use a new or empty folder, or a quiz folder", .0.display())]
    NotQuizFolder(PathBuf),
    #[error("{} is a link that leads out of its quiz folder", .0.display())]
    LeadsOut(PathBuf),
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
    /// The check of a quiz read from the quiz folder `root`.
    fn of(read: &ReadQuiz, root: &Path) -> Check {
        Check::with(read, &quiz_file::problems(&read.quiz, root))
    }

    fn with(read: &ReadQuiz, problems: &[ValidationError]) -> Check {
        let words = |errors: &[ValidationError]| errors.iter().map(ToString::to_string).collect();
        let quiz = &read.quiz;
        Check {
            title: quiz.title.clone(),
            slides: quiz.slides.len(),
            questions: quiz.slides.iter().filter(|slide| slide.content.slide_type() == SlideType::Question).count(),
            problems: words(problems),
            left_out: words(&read.dropped),
        }
    }

    /// Whether the quiz can be saved and played as it is.
    pub fn is_ready(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Checks the quiz at `path`: a quiz folder, or a quiz file (`.quiz`, or a
/// plain quiz JSON with its media next to it) as the editor opens it, read
/// without unpacking its media (`quiz_file::inspect`).
pub fn check(path: &Path) -> Result<Check> {
    if path.is_dir() {
        return Ok(Check::of(&read_inside(path)?, path));
    }
    let inspected = quiz_file::inspect(path)?;
    Ok(Check::with(&inspected.read, &inspected.problems()))
}

/// Refuses the quiz folder `folder` when its `quiz.json`, its `media`
/// folder or one of the media files `named` (references such as
/// `media/x.png`) is a link or junction that leads out of it
/// (`quiz_file::leads_inside`): the assistant may have been given the
/// folder, not where its links go. What is not there is left to the caller.
fn stays_inside<'a>(folder: &Path, named: impl IntoIterator<Item = &'a str>) -> Result<()> {
    let media = quiz_core::MEDIA_PREFIX.trim_end_matches('/');
    // Only safe references are joined to the folder, so none climbs out by its name.
    let named = named.into_iter().filter(|reference| quiz_core::is_safe_reference(reference));
    for name in [quiz_file::QUIZ_JSON, media].into_iter().chain(named) {
        let path = folder.join(name);
        if fs::symlink_metadata(&path).is_ok() && !quiz_file::leads_inside(&path, folder).unwrap_or(false) {
            return Err(WorkError::LeadsOut(path));
        }
    }
    Ok(())
}

/// Reads the quiz in the quiz folder `folder` once `stays_inside` allows
/// it and every media file it uses.
fn read_inside(folder: &Path) -> Result<ReadQuiz> {
    stays_inside(folder, [])?;
    let read = quiz_file::read_quiz(folder)?;
    stays_inside(folder, read.quiz.media_files())?;
    Ok(read)
}

/// Refuses to write into `folder` unless it is a quiz folder, or can become
/// one: missing, empty, holding a `quiz.json`, or only media and hidden
/// files. So a quiz is never written among other files.
fn quiz_folder(folder: &Path) -> Result<()> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(WorkError::io(FileAction::Read, folder, e)),
    };
    let names: Vec<String> = entries.flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
    let media = quiz_core::MEDIA_PREFIX.trim_end_matches('/');
    if names.iter().any(|name| name == quiz_file::QUIZ_JSON) || names.iter().all(|name| name == media || name.starts_with('.')) {
        Ok(())
    } else {
        Err(WorkError::NotQuizFolder(folder.to_path_buf()))
    }
}

/// Writes `json`, a whole quiz, as the `quiz.json` of the folder `folder`,
/// which is made if missing (see `quiz_folder`); media stay as they are.
/// What does not fit the format is left out (`Check::left_out`); JSON that
/// is no quiz at all is refused and nothing is written.
pub fn write_quiz(folder: &Path, json: &str) -> Result<Check> {
    let read = Quiz::read_json(json).map_err(QuizFileError::from)?;
    quiz_folder(folder)?;
    stays_inside(folder, [])?;
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
/// if missing (see `quiz_folder`), and returns what the quiz names each by. A file with a name
/// that does not work on every system gets the closest one that does, and a
/// different file of the same name a number (`quiz_file::import_media`).
/// Nothing is copied unless every file is one the app plays.
pub fn add_media(folder: &Path, files: &[PathBuf]) -> Result<Vec<Added>> {
    if let Some(file) = files.iter().find(|file| !file.is_file() || media_kind(file).is_none()) {
        return Err(WorkError::NotMedia(file.clone()));
    }
    quiz_folder(folder)?;
    stays_inside(folder, [])?;
    files
        .iter()
        .map(|from| {
            let reference = quiz_file::import_media(folder, MediaSource::File(from))?;
            Ok(Added { from: from.clone(), reference })
        })
        .collect()
}

/// Makes the quiz file `dest` (`.quiz`) of the quiz folder `folder`, with
/// the media its slides use; over a file of that name only when `replace`.
/// A quiz with problems is refused: they are listed, and the file is not
/// written.
pub fn pack(folder: &Path, dest: &Path, replace: bool) -> Result<Check> {
    if !dest.extension().is_some_and(|extension| extension.eq_ignore_ascii_case(EXTENSION)) {
        return Err(WorkError::NotQuizFile(dest.to_path_buf()));
    }
    if !replace && fs::symlink_metadata(dest).is_ok() {
        return Err(WorkError::Exists(dest.to_path_buf()));
    }
    let read = read_inside(folder)?;
    quiz_file::save(&read.quiz, folder, dest)?;
    Ok(Check::of(&read, folder))
}

/// Unpacks the quiz file `file` into the folder `folder` to work on it.
/// The folder must not exist yet: nothing is ever unpacked over a quiz.
/// What the file has that the format cannot keep is said in `left_out`:
/// the folder's quiz.json is already written without it, so a quiz packed
/// from the folder over the file loses it.
pub fn unpack(file: &Path, folder: &Path) -> Result<Check> {
    if fs::symlink_metadata(folder).is_ok() {
        return Err(WorkError::Exists(folder.to_path_buf()));
    }
    let opened = quiz_file::open_into(file, folder)?;
    Ok(Check::of(&ReadQuiz { quiz: opened.quiz, dropped: opened.dropped }, folder))
}

/// The `quiz.json` of the quiz folder `folder`, as it is written, up to
/// `quiz_file::MAX_QUIZ_JSON_BYTES`.
pub fn read_quiz(folder: &Path) -> Result<String> {
    stays_inside(folder, [])?;
    Ok(quiz_file::read_json(&folder.join(quiz_file::QUIZ_JSON))?)
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
    /// More files are there than `MAX_LISTED`, deeper than `MAX_LIST_DEPTH`
    /// or in folders that cannot be read.
    pub truncated: bool,
}

/// The pictures, sounds and videos the app plays in `folder` and the
/// folders in it, up to `MAX_LIST_DEPTH` down, by path. The folders are
/// read level by level, so the nearest files are the ones listed when there
/// are more than `MAX_LISTED`. Hidden files and folders are passed over, and
/// so are folders in it that cannot be read (the system's own, such as
/// Windows' "Application Data").
pub fn list_media(folder: &Path) -> Result<Listing> {
    let mut listing = Listing { files: Vec::new(), truncated: false };
    let mut folders = VecDeque::from([(folder.to_path_buf(), 0)]);
    while let Some((dir, depth)) = folders.pop_front() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if depth == 0 => return Err(WorkError::io(FileAction::Read, &dir, e)),
            Err(_) => {
                listing.truncated = true;
                continue;
            }
        };
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                if depth < MAX_LIST_DEPTH {
                    folders.push_back((path, depth + 1));
                } else {
                    listing.truncated = true;
                }
            } else if let Some(kind) = media_kind(&path) {
                if listing.files.len() == MAX_LISTED {
                    listing.truncated = true;
                    listing.files.sort_by(|a, b| a.path.cmp(&b.path));
                    return Ok(listing);
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

/// Reads the picture `path` for the assistant to look at: only a file
/// named and made as a PNG, JPEG, GIF or WebP picture is read whole.
pub fn picture(path: &Path) -> Result<Picture> {
    let extension = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if !PICTURE_EXTENSIONS.contains(&extension.as_str()) {
        return Err(WorkError::NotPicture(path.to_path_buf()));
    }
    let read = |e| WorkError::io(FileAction::Read, path, e);
    let file = fs::File::open(path).map_err(read)?;
    if file.metadata().map_err(read)?.len() > MAX_PICTURE_BYTES {
        return Err(WorkError::PictureTooLarge(path.to_path_buf()));
    }
    let mut bytes = Vec::new();
    file.take(MAX_PICTURE_BYTES + 1).read_to_end(&mut bytes).map_err(read)?;
    if bytes.len() as u64 > MAX_PICTURE_BYTES {
        return Err(WorkError::PictureTooLarge(path.to_path_buf()));
    }
    let mime_type = picture_type(&bytes).ok_or_else(|| WorkError::NotPicture(path.to_path_buf()))?;
    Ok(Picture { bytes, mime_type })
}

/// The type of picture `bytes` are, by the signature each format starts with.
fn picture_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn media_kind(path: &Path) -> Option<MediaKind> {
    MediaKind::of(&path.file_name()?.to_string_lossy())
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
        pack(&folder, &file, false).unwrap();
        assert!(check_is_ready(&file));
        // A quiz file is replaced only when asked to.
        assert!(matches!(pack(&folder, &file, false), Err(WorkError::Exists(_))));
        pack(&folder, &file, true).unwrap();
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
        assert!(matches!(pack(&folder, &file, false), Err(WorkError::File(QuizFileError::Invalid(_)))));
        assert!(!file.exists());
        assert!(matches!(pack(&folder, &dir.join("broken.zip"), false), Err(WorkError::NotQuizFile(_))));
    }

    #[test]
    fn a_quiz_file_is_checked_without_unpacking_it() {
        let dir = scratch("check-file");
        let folder = dir.join("quiz");
        fs::create_dir_all(folder.join("media")).unwrap();
        fs::write(folder.join("media/a.png"), b"png").unwrap();
        write_quiz(&folder, &quiz_with_picture("media/a.png")).unwrap();
        let file = dir.join("a.quiz");
        pack(&folder, &file, false).unwrap();
        assert!(check(&file).unwrap().is_ready());
        // The plain quiz.json reads its media from next to it.
        assert!(check(&folder.join(quiz_file::QUIZ_JSON)).unwrap().is_ready());
        fs::remove_file(folder.join("media/a.png")).unwrap();
        assert_eq!(check(&folder.join(quiz_file::QUIZ_JSON)).unwrap().problems.len(), 1);
        let leftovers = fs::read_dir(std::env::temp_dir()).unwrap().flatten();
        assert!(!leftovers.into_iter().any(|entry| entry.file_name().to_string_lossy().starts_with(&format!("buzzdome-check-{}-", std::process::id()))));
    }

    #[test]
    fn a_quiz_is_written_only_into_a_quiz_folder_or_a_new_one() {
        let dir = scratch("quiz-folder");
        let quiz = quiz_with_picture("media/a.png");
        // Media first, then the quiz: a folder of media becomes a quiz folder.
        let folder = dir.join("quiz");
        fs::write(dir.join("a.png"), b"png").unwrap();
        add_media(&folder, &[dir.join("a.png")]).unwrap();
        write_quiz(&folder, &quiz).unwrap();
        write_quiz(&folder, &quiz).unwrap();
        // A folder with other files is no quiz folder.
        let documents = dir.join("documents");
        fs::create_dir_all(&documents).unwrap();
        fs::write(documents.join("tax.pdf"), b"pdf").unwrap();
        assert!(matches!(write_quiz(&documents, &quiz), Err(WorkError::NotQuizFolder(_))));
        assert!(matches!(add_media(&documents, &[dir.join("a.png")]), Err(WorkError::NotQuizFolder(_))));
        assert!(!documents.join(quiz_file::QUIZ_JSON).exists() && !documents.join("media").exists());
    }

    /// Links the quizmaster made inside a root that lead out of it: the
    /// tools neither read, pack nor write through them.
    #[cfg(unix)]
    #[test]
    fn nothing_is_read_or_written_through_a_link_out_of_the_quiz_folder() {
        use std::os::unix::fs::symlink;
        let dir = scratch("links-out");
        let outside = dir.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.png"), b"secret").unwrap();
        fs::write(outside.join(quiz_file::QUIZ_JSON), quiz_with_picture("media/secret.png")).unwrap();
        fs::write(dir.join("a.png"), b"png").unwrap();

        // A media folder that is a link.
        let linked_media = dir.join("linked-media");
        fs::create_dir_all(&linked_media).unwrap();
        symlink(&outside, linked_media.join("media")).unwrap();
        assert!(matches!(add_media(&linked_media, &[dir.join("a.png")]), Err(WorkError::LeadsOut(_))));
        assert!(!outside.join("a.png").exists());
        assert!(matches!(write_quiz(&linked_media, &quiz_with_picture("media/secret.png")), Err(WorkError::LeadsOut(_))));

        // A media file that is a link.
        let linked_file = dir.join("linked-file");
        fs::create_dir_all(linked_file.join("media")).unwrap();
        symlink(outside.join("secret.png"), linked_file.join("media/secret.png")).unwrap();
        fs::write(linked_file.join(quiz_file::QUIZ_JSON), quiz_with_picture("media/secret.png")).unwrap();
        let file = dir.join("out.quiz");
        assert!(matches!(pack(&linked_file, &file, false), Err(WorkError::LeadsOut(_))));
        assert!(!file.exists());
        assert!(matches!(check(&linked_file), Err(WorkError::LeadsOut(_))));

        // A quiz.json that is a link.
        let linked_json = dir.join("linked-json");
        fs::create_dir_all(&linked_json).unwrap();
        symlink(outside.join(quiz_file::QUIZ_JSON), linked_json.join(quiz_file::QUIZ_JSON)).unwrap();
        assert!(matches!(read_quiz(&linked_json), Err(WorkError::LeadsOut(_))));
        assert!(matches!(check(&linked_json), Err(WorkError::LeadsOut(_))));

        // A link that stays inside the folder is fine.
        let inner = dir.join("inner");
        fs::create_dir_all(inner.join("media")).unwrap();
        fs::write(inner.join("media/a.png"), b"png").unwrap();
        symlink(inner.join("media/a.png"), inner.join("media/b.png")).unwrap();
        fs::write(inner.join(quiz_file::QUIZ_JSON), quiz_with_picture("media/b.png")).unwrap();
        assert!(check(&inner).unwrap().is_ready());
    }

    #[test]
    fn a_quiz_json_too_large_is_not_read() {
        let dir = scratch("read-large");
        let big = fs::File::create(dir.join(quiz_file::QUIZ_JSON)).unwrap();
        big.set_len(quiz_file::MAX_QUIZ_JSON_BYTES + 1).unwrap();
        assert!(matches!(read_quiz(&dir), Err(WorkError::File(QuizFileError::QuizJsonTooLarge))));
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
    fn unpacking_says_what_the_file_had_that_the_folder_left_out() {
        let dir = scratch("unpack-left-out");
        let mut quiz: serde_json::Value = serde_json::from_str(&quiz_with_picture("media/x.png")).unwrap();
        quiz["slides"][0]["style"] = json!({ "headline": { "color": "gold" } });
        quiz["slides"][1].as_object_mut().unwrap().remove("picture");
        let file = dir.join("colleague.quiz");
        let mut zip = zip::ZipWriter::new(fs::File::create(&file).unwrap());
        zip.start_file(quiz_file::QUIZ_JSON, zip::write::SimpleFileOptions::default()).unwrap();
        std::io::Write::write_all(&mut zip, quiz.to_string().as_bytes()).unwrap();
        zip.finish().unwrap();
        let left_out = vec!["slide r1: the style value style.headline cannot be read".to_string()];
        assert_eq!(check(&file).unwrap().left_out, left_out);

        let folder = dir.join("quiz");
        assert_eq!(unpack(&file, &folder).unwrap().left_out, left_out);
        // The folder's quiz no longer has it, so only unpacking can say so.
        assert!(!read_quiz(&folder).unwrap().contains("gold"));
        assert_eq!(check(&folder).unwrap().left_out, Vec::<String>::new());
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
        assert!(list_media(&dir.join("missing")).is_err());
    }

    #[test]
    fn the_nearest_media_are_listed_up_to_the_cap() {
        let dir = scratch("list-cap");
        fs::create_dir_all(dir.join("a/deep")).unwrap();
        for n in 0..MAX_LISTED {
            fs::write(dir.join(format!("a/deep/{n}.png")), b"x").unwrap();
        }
        fs::write(dir.join("top.png"), b"x").unwrap();
        fs::write(dir.join("a/near.png"), b"x").unwrap();
        let listing = list_media(&dir).unwrap();
        assert!(listing.truncated);
        assert_eq!(listing.files.len(), MAX_LISTED);
        assert!(listing.files.iter().any(|file| file.path == dir.join("top.png")));
        assert!(listing.files.iter().any(|file| file.path == dir.join("a/near.png")));
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_that_cannot_be_read_is_passed_over() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("list-locked");
        fs::create_dir_all(dir.join("locked")).unwrap();
        fs::write(dir.join("locked/x.png"), b"x").unwrap();
        fs::write(dir.join("a.png"), b"x").unwrap();
        fs::set_permissions(dir.join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
        let listing = list_media(&dir);
        fs::set_permissions(dir.join("locked"), fs::Permissions::from_mode(0o755)).unwrap();
        let listing = listing.unwrap();
        // Root reads any folder: then nothing is passed over.
        if fs::read_dir(dir.join("locked")).is_ok() && listing.files.len() == 2 {
            return;
        }
        assert_eq!(listing.files.len(), 1);
        assert!(listing.truncated);
    }

    pub(crate) const PNG: &[u8] = b"\x89PNG\r\n\x1a\n....";

    #[test]
    fn a_picture_is_handed_over_only_when_an_assistant_reads_it() {
        let dir = scratch("picture");
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10];
        fs::write(dir.join("p.JPG"), jpeg).unwrap();
        fs::write(dir.join("p.png"), PNG).unwrap();
        fs::write(dir.join("p.webp"), b"RIFF\0\0\0\0WEBPVP8 ").unwrap();
        fs::write(dir.join("p.svg"), b"<svg/>").unwrap();
        fs::write(dir.join("notes.png"), b"a text named as a picture").unwrap();
        assert_eq!(picture(&dir.join("p.JPG")).unwrap(), Picture { bytes: jpeg.to_vec(), mime_type: "image/jpeg" });
        // The signature decides the type, not the name.
        fs::copy(dir.join("p.png"), dir.join("png.gif")).unwrap();
        assert_eq!(picture(&dir.join("png.gif")).unwrap().mime_type, "image/png");
        assert_eq!(picture(&dir.join("p.webp")).unwrap().mime_type, "image/webp");
        assert!(matches!(picture(&dir.join("p.svg")), Err(WorkError::NotPicture(_))));
        assert!(matches!(picture(&dir.join("notes.png")), Err(WorkError::NotPicture(_))));
        let big = fs::File::create(dir.join("big.png")).unwrap();
        big.set_len(MAX_PICTURE_BYTES + 1).unwrap();
        assert!(matches!(picture(&dir.join("big.png")), Err(WorkError::PictureTooLarge(_))));
    }
}
