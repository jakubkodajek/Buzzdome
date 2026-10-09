//! Quiz files. A `.quiz` file is a ZIP with `quiz.json` at its root and the
//! pictures, sounds and videos it uses in `media/` (format: docs/quiz-format.md).
//! A plain `quiz.json` works too; its media are read from next to it.
//!
//! Opening a file unpacks it into a package folder in the app's cache, named
//! by a hash of its contents. The show reads media from there, so it keeps
//! working (and resumes after a crash) even if the original file is moved,
//! and opening the same file twice reuses the same folder.

use quiz_core::{is_safe_reference, Quiz, MEDIA_PREFIX};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// File extension of quiz files.
pub const EXTENSION: &str = "quiz";
/// The quiz itself, at the root of a quiz file and of a package folder.
pub const QUIZ_JSON: &str = "quiz.json";
/// Limits against damaged or hostile files.
pub const MAX_UNPACKED_BYTES: u64 = 4 << 30;
pub const MAX_ENTRIES: usize = 10_000;

const ZIP_MAGIC: &[u8; 4] = b"PK\x03\x04";
const INCOMING_PREFIX: &str = ".incoming-";

#[derive(Debug, thiserror::Error)]
pub enum QuizFileError {
    #[error("{context}: {source}")]
    Io { context: String, source: io::Error },
    #[error("the file is not a valid quiz file: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{QUIZ_JSON} cannot be read: {0}")]
    Json(String),
    #[error("the quiz file has no {QUIZ_JSON}")]
    MissingQuizJson,
    #[error("the quiz file unpacks to more than {} MB", MAX_UNPACKED_BYTES >> 20)]
    TooLarge,
    #[error("the quiz file has more than {MAX_ENTRIES} files")]
    TooManyFiles,
    #[error("the quiz has problems:\n{}", .0.join("\n"))]
    Invalid(Vec<String>),
}

type Result<T> = std::result::Result<T, QuizFileError>;

/// Adds what was being done to an I/O error.
trait Context<T> {
    fn context(self, what: impl FnOnce() -> String) -> Result<T>;
}

impl<T> Context<T> for io::Result<T> {
    fn context(self, what: impl FnOnce() -> String) -> Result<T> {
        self.map_err(|source| QuizFileError::Io { context: what(), source })
    }
}

/// A quiz unpacked into a folder: `root/quiz.json` and `root/media/...`.
#[derive(Debug, Clone)]
pub struct Package {
    pub quiz: Quiz,
    pub root: PathBuf,
}

/// Opens a quiz file (`.quiz`, or a plain `quiz.json`), checks it, and unpacks
/// it into a package folder under `cache`.
pub fn open(path: &Path, cache: &Path) -> Result<Package> {
    fs::create_dir_all(cache).context(|| format!("cannot create {}", cache.display()))?;
    let incoming = cache.join(format!("{INCOMING_PREFIX}{}-{}", std::process::id(), unique_suffix()));
    let unpacked = if is_zip(path)? { unpack_zip(path, &incoming) } else { copy_json(path, &incoming) };
    let result = unpacked.and_then(|hash| {
        let quiz = read_checked(&incoming)?;
        let root = cache.join(hash);
        if root.exists() {
            // The same content was opened before.
            remove_dir(&incoming);
        } else {
            fs::rename(&incoming, &root).context(|| format!("cannot move the quiz into {}", root.display()))?;
        }
        Ok(Package { quiz, root })
    });
    if result.is_err() {
        remove_dir(&incoming);
    }
    result
}

/// Reads and checks the package in `root`: the quiz is valid and every media
/// file it uses is there.
pub fn read_checked(root: &Path) -> Result<Quiz> {
    let json = fs::read_to_string(root.join(QUIZ_JSON)).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => QuizFileError::MissingQuizJson,
        _ => QuizFileError::Io { context: format!("cannot read {QUIZ_JSON}"), source: e },
    })?;
    let quiz = Quiz::from_json(&json).map_err(QuizFileError::Json)?;
    check(&quiz, root)?;
    Ok(quiz)
}

/// Checks the quiz and that every media file it uses exists under `root`.
pub fn check(quiz: &Quiz, root: &Path) -> Result<()> {
    let mut problems: Vec<String> = match quiz.validate() {
        Ok(()) => Vec::new(),
        Err(errors) => errors.iter().map(ToString::to_string).collect(),
    };
    if problems.is_empty() {
        // References are safe paths now, so joining them stays inside `root`.
        problems.extend(
            quiz.media_files()
                .into_iter()
                .filter(|file| !root.join(file).is_file())
                .map(|file| format!("media file missing: {file}")),
        );
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(QuizFileError::Invalid(problems))
    }
}

/// Writes `quiz` with the media it uses (read from `media_root`) to `dest` as
/// a quiz file. The file is replaced only once the new one is complete.
pub fn save(quiz: &Quiz, media_root: &Path, dest: &Path) -> Result<()> {
    check(quiz, media_root)?;
    let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let temp = dest.with_file_name(format!(".{name}.saving"));
    let written = write_zip(quiz, media_root, &temp).and_then(|()| {
        fs::rename(&temp, dest).context(|| format!("cannot replace {}", dest.display()))
    });
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written
}

/// Deletes every package folder in `cache` except `keep`, and leftovers of
/// interrupted opens.
pub fn prune(cache: &Path, keep: &[&Path]) -> io::Result<()> {
    if !cache.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(cache)? {
        let path = entry?.path();
        if path.is_dir() && !keep.iter().any(|k| *k == path) {
            fs::remove_dir_all(&path)?;
        }
    }
    Ok(())
}

fn write_zip(quiz: &Quiz, media_root: &Path, temp: &Path) -> Result<()> {
    let file = File::create(temp).context(|| format!("cannot write {}", temp.display()))?;
    let mut zip = ZipWriter::new(file);
    let json = serde_json::to_string_pretty(quiz).map_err(|e| QuizFileError::Json(e.to_string()))?;
    zip.start_file(QUIZ_JSON, SimpleFileOptions::default().compression_method(CompressionMethod::Deflated))?;
    zip.write_all(json.as_bytes()).context(|| format!("cannot write {QUIZ_JSON}"))?;
    for reference in quiz.media_files() {
        let path = media_root.join(reference);
        let mut media = File::open(&path).context(|| format!("cannot read {}", path.display()))?;
        let size = media.metadata().context(|| format!("cannot read {}", path.display()))?.len();
        // Pictures, sounds and videos are compressed already.
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .large_file(size >= u64::from(u32::MAX));
        zip.start_file(reference, options)?;
        io::copy(&mut media, &mut zip).context(|| format!("cannot write {reference}"))?;
    }
    let file = zip.finish()?;
    file.sync_all().context(|| format!("cannot write {}", temp.display()))
}

fn is_zip(path: &Path) -> Result<bool> {
    let mut magic = [0u8; 4];
    let mut file = File::open(path).context(|| format!("cannot open {}", path.display()))?;
    match file.read_exact(&mut magic) {
        Ok(()) => Ok(&magic == ZIP_MAGIC),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e).context(|| format!("cannot read {}", path.display())),
    }
}

/// Unpacks `quiz.json` and `media/` into `dest` and returns the content hash.
/// Other entries (folders, links, files some ZIP tools add) are skipped.
fn unpack_zip(path: &Path, dest: &Path) -> Result<String> {
    let file = File::open(path).context(|| format!("cannot open {}", path.display()))?;
    let mut archive = ZipArchive::new(file)?;
    if archive.len() > MAX_ENTRIES {
        return Err(QuizFileError::TooManyFiles);
    }
    let mut hasher = Sha256::new();
    let mut budget = MAX_UNPACKED_BYTES;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() || entry.is_symlink() {
            continue;
        }
        let name = entry.name()?.replace('\\', "/");
        if name != QUIZ_JSON && !(name.starts_with(MEDIA_PREFIX) && is_safe_reference(&name)) {
            continue;
        }
        let copied = write_file(dest, &name, &mut entry, budget, &mut hasher)?;
        budget -= copied;
    }
    Ok(finish(hasher))
}

/// Copies a plain quiz JSON file and the media it uses (next to it) into
/// `dest` and returns the content hash.
fn copy_json(path: &Path, dest: &Path) -> Result<String> {
    let json = fs::read_to_string(path).context(|| format!("cannot read {}", path.display()))?;
    let quiz = Quiz::from_json(&json).map_err(QuizFileError::Json)?;
    let source_root = path.parent().unwrap_or(Path::new("."));
    // Check before copying, so a bad reference cannot point outside the folder.
    if let Err(errors) = quiz.validate() {
        return Err(QuizFileError::Invalid(errors.iter().map(ToString::to_string).collect()));
    }
    let mut hasher = Sha256::new();
    let mut budget = MAX_UNPACKED_BYTES;
    budget -= write_file(dest, QUIZ_JSON, &mut json.as_bytes(), budget, &mut hasher)?;
    for reference in quiz.media_files() {
        let source = source_root.join(reference);
        let Ok(mut media) = File::open(&source) else {
            // Reported as missing by the check after unpacking.
            continue;
        };
        budget -= write_file(dest, reference, &mut media, budget, &mut hasher)?;
    }
    Ok(finish(hasher))
}

/// Writes one file of a package, adds its name and contents to the hash and
/// returns its size. Fails if it would exceed `budget` bytes.
fn write_file(dest: &Path, name: &str, contents: &mut impl Read, budget: u64, hasher: &mut Sha256) -> Result<u64> {
    let path = dest.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context(|| format!("cannot create {}", parent.display()))?;
    }
    let file = File::create(&path).context(|| format!("cannot write {}", path.display()))?;
    hasher.update(name.as_bytes());
    hasher.update([0]);
    let mut out = HashingWriter { inner: file, hasher };
    let copied = io::copy(&mut contents.take(budget + 1), &mut out).context(|| format!("cannot unpack {name}"))?;
    if copied > budget {
        return Err(QuizFileError::TooLarge);
    }
    out.hasher.update(copied.to_le_bytes());
    // A package folder is used as it is after a restart, so it must be complete on disk.
    out.inner.sync_all().context(|| format!("cannot write {}", path.display()))?;
    Ok(copied)
}

struct HashingWriter<'a> {
    inner: File,
    hasher: &'a mut Sha256,
}

impl Write for HashingWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// 128 bits of the hash, as the package folder name.
fn finish(hasher: Sha256) -> String {
    hasher.finalize().iter().take(16).map(|b| format!("{b:02x}")).collect()
}

fn unique_suffix() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos())
}

fn remove_dir(path: &Path) {
    if let Err(e) = fs::remove_dir_all(path) {
        if e.kind() != io::ErrorKind::NotFound {
            eprintln!("cannot remove {}: {e}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiz_core::{Slide, SlideContent};

    /// A fresh, empty folder for one test.
    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("quiz-file-{name}-{}-{}", std::process::id(), unique_suffix()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn quiz_with_picture(picture: &str) -> Quiz {
        let mut slide = Slide::new("intro", SlideContent::Billboard { title: "Round 1".into(), text: String::new() });
        slide.picture = Some(picture.into());
        Quiz { schema_version: 1, title: "Test".into(), slides: vec![slide] }
    }

    fn media_folder(name: &str, files: &[(&str, &[u8])]) -> PathBuf {
        let root = temp(name);
        for (file, bytes) in files {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        root
    }

    fn zip_of(name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let path = temp(name).join("test.quiz");
        let mut zip = ZipWriter::new(File::create(&path).unwrap());
        for (entry, bytes) in entries {
            zip.start_file(*entry, SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn saves_and_opens_a_quiz_with_its_media() {
        let quiz = quiz_with_picture("media/flag.png");
        let media = media_folder("save-media", &[("media/flag.png", b"png bytes"), ("media/unused.png", b"x")]);
        let file = temp("save-out").join("test.quiz");
        save(&quiz, &media, &file).unwrap();

        let cache = temp("save-cache");
        let package = open(&file, &cache).unwrap();
        assert_eq!(package.quiz, quiz);
        assert_eq!(fs::read(package.root.join("media/flag.png")).unwrap(), b"png bytes");
        // Only what the quiz uses is saved.
        assert!(!package.root.join("media/unused.png").exists());

        // The same file opens into the same folder, and nothing is left over.
        let again = open(&file, &cache).unwrap();
        assert_eq!(again.root, package.root);
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
    }

    #[test]
    fn saving_needs_every_media_file() {
        let quiz = quiz_with_picture("media/flag.png");
        let file = temp("save-missing").join("test.quiz");
        let err = save(&quiz, &temp("save-missing-media"), &file).unwrap_err();
        assert!(matches!(err, QuizFileError::Invalid(ref p) if p == &["media file missing: media/flag.png"]), "{err}");
        assert!(!file.exists());
    }

    #[test]
    fn opening_skips_entries_outside_the_package_and_reports_missing_media() {
        let json = serde_json::to_vec(&quiz_with_picture("media/flag.png")).unwrap();
        let file = zip_of(
            "unsafe",
            &[(QUIZ_JSON, &json), ("../evil.png", b"x"), ("media/../evil.png", b"x"), ("__MACOSX/x", b"x")],
        );
        let cache = temp("unsafe-cache");
        let err = open(&file, &cache).unwrap_err();
        assert!(matches!(err, QuizFileError::Invalid(ref p) if p == &["media file missing: media/flag.png"]), "{err}");
        assert!(!cache.parent().unwrap().join("evil.png").exists());
        // A failed open leaves nothing behind.
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 0);
    }

    #[test]
    fn opening_needs_quiz_json() {
        let file = zip_of("no-json", &[("media/flag.png", b"x")]);
        assert!(matches!(open(&file, &temp("no-json-cache")), Err(QuizFileError::MissingQuizJson)));
    }

    #[test]
    fn opens_a_plain_json_quiz_with_media_next_to_it() {
        let quiz = quiz_with_picture("media/flag.png");
        let folder = media_folder("json", &[("media/flag.png", b"png bytes")]);
        fs::write(folder.join("quiz.json"), serde_json::to_vec(&quiz).unwrap()).unwrap();
        let package = open(&folder.join("quiz.json"), &temp("json-cache")).unwrap();
        assert_eq!(fs::read(package.root.join("media/flag.png")).unwrap(), b"png bytes");
    }

    #[test]
    fn a_plain_json_quiz_cannot_reach_outside_its_folder() {
        let quiz = quiz_with_picture("media/../../secret.png");
        let folder = temp("json-escape");
        fs::write(folder.join("quiz.json"), serde_json::to_vec(&quiz).unwrap()).unwrap();
        assert!(matches!(open(&folder.join("quiz.json"), &temp("json-escape-cache")), Err(QuizFileError::Invalid(_))));
    }

    #[test]
    fn unpacking_stops_at_the_size_limit() {
        let dest = temp("budget");
        let mut hasher = Sha256::new();
        let err = write_file(&dest, "media/big.mp4", &mut &[0u8; 100][..], 99, &mut hasher).unwrap_err();
        assert!(matches!(err, QuizFileError::TooLarge));
        assert_eq!(write_file(&dest, "media/ok.mp4", &mut &[0u8; 100][..], 100, &mut hasher).unwrap(), 100);
    }

    #[test]
    fn prune_keeps_only_the_given_package() {
        let cache = temp("prune");
        for dir in ["a", "b", ".incoming-1-2"] {
            fs::create_dir_all(cache.join(dir)).unwrap();
        }
        prune(&cache, &[&cache.join("a")]).unwrap();
        let left: Vec<_> = fs::read_dir(&cache).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left, ["a"]);
    }
}
