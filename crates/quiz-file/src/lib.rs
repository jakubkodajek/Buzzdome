//! Quiz files. A `.quiz` file is a ZIP with `quiz.json` at its root and the
//! pictures, sounds and videos it uses in `media/` (format: docs/quiz-format.md).
//! A plain `quiz.json` works too; its media are read from next to it.
//!
//! Opening a file unpacks it into a package folder in the app's cache, named
//! by a hash of its contents. The show reads media from there, so it keeps
//! working (and resumes after a crash) even if the original file is moved,
//! and opening the same file twice reuses the same folder.

mod sheet;

pub use sheet::{export_sheet, import_sheet, read_tables, SheetExport, SheetImport, SheetRead, MAX_SHEET_BYTES, MAX_SHEET_COLUMNS, MAX_SHEET_ROWS};

use quiz_core::{
    is_safe_reference, portable_name, portable_reference, MediaIssue, Problem, Quiz, QuizJsonError, SheetError, ValidationError,
    MEDIA_PREFIX,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use ts_rs::TS;
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
const TRASH_PREFIX: &str = ".trash-";

/// What was being done to a file when the disk failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum FileAction {
    Read,
    Write,
    CreateFolder,
    Move,
    Delete,
}

#[derive(Debug, thiserror::Error)]
pub enum QuizFileError {
    #[error("cannot {action:?} {}: {source}", path.display())]
    Io { action: FileAction, path: PathBuf, source: io::Error },
    #[error("the file is not a valid quiz file: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{QUIZ_JSON} cannot be read: {0}")]
    Json(#[from] QuizJsonError),
    #[error("the quiz file has no {QUIZ_JSON}")]
    MissingQuizJson,
    #[error("the quiz file unpacks to more than {} MB", MAX_UNPACKED_BYTES >> 20)]
    TooLarge,
    #[error("the quiz file has more than {MAX_ENTRIES} files")]
    TooManyFiles,
    #[error("the quiz has problems: {}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))]
    Invalid(Vec<ValidationError>),
    #[error("the spreadsheet cannot be read: {0}")]
    Spreadsheet(String),
    #[error("the spreadsheet is larger than {} MB or has more than {} rows or {} columns", sheet::MAX_SHEET_BYTES >> 20, sheet::MAX_SHEET_ROWS, sheet::MAX_SHEET_COLUMNS)]
    SpreadsheetTooLarge,
    #[error("{0}")]
    Sheet(#[from] SheetError),
}

type Result<T> = std::result::Result<T, QuizFileError>;

/// Adds what was being done, and to which file, to an I/O error.
trait Context<T> {
    fn context(self, action: FileAction, path: &Path) -> Result<T>;
}

impl<T> Context<T> for io::Result<T> {
    fn context(self, action: FileAction, path: &Path) -> Result<T> {
        self.map_err(|source| QuizFileError::Io { action, path: path.to_path_buf(), source })
    }
}

/// A quiz unpacked into a folder: `root/quiz.json` and `root/media/...`.
#[derive(Debug, Clone)]
pub struct Package {
    pub quiz: Quiz,
    pub root: PathBuf,
}

/// Opens a quiz file (`.quiz`, or a plain `quiz.json`), checks it, and unpacks
/// it into a package folder under `cache`. The folder is named by the content
/// hash (see `place_package`) and must not be changed afterwards; opening the
/// same file reuses it.
pub fn open(path: &Path, cache: &Path) -> Result<Package> {
    let incoming = incoming_dir(cache)?;
    let result = unpack(path, &incoming).and_then(|unpacked| {
        // The show plays the file as it is: a media name it would have to
        // rename is a problem to fix in the editor.
        let quiz = read_checked(&incoming)?;
        let root = place_package(cache, &unpacked.hash, &incoming)?;
        Ok(Package { quiz, root })
    });
    if result.is_err() {
        remove_dir(&incoming);
    }
    result
}

/// Most package folders tried for one content hash.
const MAX_PACKAGE_FOLDERS: usize = 16;

/// Moves the package unpacked in `incoming` to its folder under `cache`,
/// named `hash`, or reuses a folder that holds the same content already. A
/// stale folder (left half-deleted by a `prune` that could not finish) is
/// cleared out of the way; one that cannot be, such as on Windows while a
/// file in it is still open, is left for `prune`, and the package goes to
/// `<hash>-1`, `<hash>-2`, ... instead.
fn place_package(cache: &Path, hash: &str, incoming: &Path) -> Result<PathBuf> {
    for n in 0..MAX_PACKAGE_FOLDERS {
        let root = if n == 0 { cache.join(hash) } else { cache.join(format!("{hash}-{n}")) };
        if read_checked(&root).is_ok() {
            remove_dir(incoming);
            return Ok(root);
        }
        set_aside(&root, cache);
        if fs::symlink_metadata(&root).is_err() {
            fs::rename(incoming, &root).context(FileAction::Move, &root)?;
            return Ok(root);
        }
    }
    let root = cache.join(hash);
    Err(io::Error::from(io::ErrorKind::AlreadyExists)).context(FileAction::Move, &root)
}

/// A quiz opened for editing.
#[derive(Debug)]
pub struct Opened {
    pub quiz: Quiz,
    /// Opening changed the quiz (renamed media), so it differs from the file.
    pub changed: bool,
}

/// Opens a quiz file into `dest`, which must not exist, replacing nothing
/// until the file has been unpacked. For a package that will be edited (the
/// editor's draft), so the quiz is not checked: a file an older version made
/// may break rules added since, and `problems` tells the author what to fix.
/// Media in a `.quiz` file whose names are not portable (see
/// `quiz_core::portable_reference`) get the closest portable name.
pub fn open_into(path: &Path, dest: &Path) -> Result<Opened> {
    let parent = dest.parent().unwrap_or(Path::new("."));
    let incoming = incoming_dir(parent)?;
    let result = unpack(path, &incoming).and_then(|unpacked| {
        let mut quiz = read_quiz_json(&incoming)?;
        let changed = !unpacked.renamed.is_empty();
        if changed {
            quiz.rename_media(&unpacked.renamed);
            write_quiz_json(&incoming, &quiz)?;
        }
        fs::rename(&incoming, dest).context(FileAction::Move, dest)?;
        Ok(Opened { quiz, changed })
    });
    if result.is_err() {
        remove_dir(&incoming);
    }
    result
}

/// What unpacking wrote.
struct Unpacked {
    /// Hash of the names and contents of the files written.
    hash: String,
    /// Media stored under another name than the quiz uses (see `renames`).
    renamed: BTreeMap<String, String>,
}

/// Unpacks a quiz file into `dest` without checking the quiz.
fn unpack(path: &Path, dest: &Path) -> Result<Unpacked> {
    if is_zip(path)? {
        unpack_zip(path, dest)
    } else {
        copy_json(path, dest)
    }
}

fn incoming_dir(parent: &Path) -> Result<PathBuf> {
    fs::create_dir_all(parent).context(FileAction::CreateFolder, parent)?;
    Ok(parent.join(format!("{INCOMING_PREFIX}{}-{}", std::process::id(), unique_suffix())))
}

/// Moves the stale folder `dir` out of the way into `cache`, then deletes
/// it. Moving works where deleting may not (a read-only file, or on Windows
/// a file another program still holds open); what cannot be deleted now,
/// `prune` deletes later.
fn set_aside(dir: &Path, cache: &Path) {
    if fs::symlink_metadata(dir).is_err() {
        return;
    }
    let trash = cache.join(format!("{TRASH_PREFIX}{}-{}", std::process::id(), unique_suffix()));
    match fs::rename(dir, &trash) {
        Ok(()) => remove_dir(&trash),
        Err(_) => remove_dir(dir),
    }
}

/// Reads and checks the package in `root`: the quiz is valid and every media
/// file it uses is there.
pub fn read_checked(root: &Path) -> Result<Quiz> {
    let quiz = read_quiz_json(root)?;
    check(&quiz, root)?;
    Ok(quiz)
}

/// Reads `quiz.json` from the package folder `root` without checking the
/// quiz: a draft may be unfinished.
pub fn read_quiz_json(root: &Path) -> Result<Quiz> {
    let path = root.join(QUIZ_JSON);
    let json = fs::read_to_string(&path).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => QuizFileError::MissingQuizJson,
        _ => QuizFileError::Io { action: FileAction::Read, path, source: e },
    })?;
    Ok(Quiz::from_json(&json)?)
}

/// Checks the quiz and that every media file it uses exists under `root`.
pub fn check(quiz: &Quiz, root: &Path) -> Result<()> {
    let found = problems(quiz, root);
    if found.is_empty() {
        Ok(())
    } else {
        Err(QuizFileError::Invalid(found))
    }
}

/// Everything that keeps the quiz from being played: the core's rules, then
/// media files missing under `root`.
pub fn problems(quiz: &Quiz, root: &Path) -> Vec<ValidationError> {
    if let Err(errors) = quiz.validate() {
        return errors;
    }
    // References are safe paths now, so joining them stays inside `root`.
    quiz.slides
        .iter()
        .flat_map(|slide| slide.media().into_iter().map(move |(file, _)| (slide, file)))
        .filter(|(_, file)| !root.join(file).is_file())
        .map(|(slide, file)| ValidationError {
            slide_id: Some(slide.id.clone()),
            problem: Problem::Media { file: file.to_string(), issue: MediaIssue::Missing },
        })
        .collect()
}

/// Writes `quiz.json` into the package folder `root` (see `write_atomic`).
/// The quiz is not checked: a draft may be unfinished.
pub fn write_quiz_json(root: &Path, quiz: &Quiz) -> Result<()> {
    write_atomic(&root.join(QUIZ_JSON), quiz_json(quiz).as_bytes())
}

fn quiz_json(quiz: &Quiz) -> String {
    serde_json::to_string_pretty(quiz).expect("a quiz serializes: it has no maps and no custom serializers that can fail")
}

/// Replaces the file `dest` with `contents` so that a crash leaves either the
/// old file or the new one, never a torn one.
pub fn write_atomic(dest: &Path, contents: &[u8]) -> Result<()> {
    replace_file(dest, |temp| {
        File::create(temp)
            .and_then(|mut file| file.write_all(contents).and_then(|()| file.sync_all()))
            .context(FileAction::Write, temp)
    })
}

/// Writes `dest` through a temporary file next to it, renamed over `dest`
/// only once `write` has finished.
fn replace_file(dest: &Path, write: impl FnOnce(&Path) -> Result<()>) -> Result<()> {
    let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let temp = dest.with_file_name(format!(".{name}.saving"));
    let written = write(&temp).and_then(|()| fs::rename(&temp, dest).context(FileAction::Write, dest));
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    // Errors name the file being saved, not its temporary twin.
    written.map_err(|e| match e {
        QuizFileError::Io { action, path, source } if path == temp => QuizFileError::Io { action, path: dest.into(), source },
        other => other,
    })?;
    sync_dir(dest.parent().unwrap_or(Path::new(".")))
}

/// Makes the files created, renamed or deleted in `dir` durable. Only Unix
/// can open a directory to sync it; on Windows this does nothing.
pub fn sync_dir(dir: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(dir).and_then(|d| d.sync_all()).context(FileAction::Write, dir)?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// A media file to copy in: one on disk, or the bytes of one dropped into a
/// window, which tells its name but not its path.
#[derive(Debug, Clone, Copy)]
pub enum MediaSource<'a> {
    File(&'a Path),
    Bytes { name: &'a str, bytes: &'a [u8] },
}

impl MediaSource<'_> {
    fn name(&self) -> String {
        match self {
            MediaSource::File(path) => path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            MediaSource::Bytes { name, .. } => name.to_string(),
        }
    }

    /// Whether the file at `path` holds the same bytes.
    fn same_as(&self, path: &Path) -> Result<bool> {
        match self {
            MediaSource::File(source) => same_contents(path, source).context(FileAction::Read, source),
            MediaSource::Bytes { bytes, .. } => same_bytes(path, bytes).context(FileAction::Read, path),
        }
    }

    fn write_to(&self, root: &Path, reference: &str) -> Result<u64> {
        match *self {
            MediaSource::File(source) => {
                let mut file = File::open(source).context(FileAction::Read, source)?;
                write_file(root, reference, &mut file, source, MAX_UNPACKED_BYTES, &mut Sha256::new())
            }
            MediaSource::Bytes { mut bytes, .. } => {
                let dest = root.join(reference);
                write_file(root, reference, &mut bytes, &dest, MAX_UNPACKED_BYTES, &mut Sha256::new())
            }
        }
    }
}

/// Copies a media file into the package folder `root` and returns its
/// reference (`media/<name>`). A file with the same name and contents is
/// reused; a different one with the same name gets a number added.
pub fn import_media(root: &Path, source: MediaSource) -> Result<String> {
    place_file(root, &format!("{MEDIA_PREFIX}{}", portable_name(&source.name())), source)
}

/// Copies `source` to `folder/name` and returns the name. A file with that
/// name and the same contents is reused; a different one keeps its place
/// and the copy gets a number added (see `numbered`). A copy that fails
/// leaves nothing behind: it is written under a name no media file has
/// (`.part`) and renamed when complete.
fn place_file(folder: &Path, name: &str, source: MediaSource) -> Result<String> {
    for n in 1.. {
        let name = numbered(name, n);
        let path = folder.join(&name);
        if !path.exists() {
            let partial = format!("{name}.part");
            let copied = source.write_to(folder, &partial).and_then(|_| fs::rename(folder.join(&partial), &path).context(FileAction::Move, &path));
            if copied.is_err() {
                let _ = fs::remove_file(folder.join(&partial));
            }
            copied?;
            return Ok(name);
        }
        if source.same_as(&path)? {
            return Ok(name);
        }
    }
    unreachable!("the loop returns")
}

/// The `n`th name to try for a file: `name` itself, then with `-n` before its
/// extension, which keeps a portable name portable.
fn numbered(name: &str, n: u32) -> String {
    if n == 1 {
        return name.to_string();
    }
    let file_name = name.rfind('/').map_or(0, |slash| slash + 1);
    match name[file_name..].rfind('.') {
        Some(dot) if dot > 0 => {
            let (stem, extension) = name.split_at(file_name + dot);
            format!("{stem}-{n}{extension}")
        }
        _ => format!("{name}-{n}"),
    }
}

/// Portable names for the media references among `names` that are not safe
/// but can be made so (see `quiz_core::portable_reference`), old name to new.
/// A new name takes none of `names` nor another new one, ignoring letter case
/// as Windows and macOS disks do.
fn renames<'a>(names: impl Iterator<Item = &'a str> + Clone) -> BTreeMap<String, String> {
    let mut taken: HashSet<String> = names.clone().map(str::to_lowercase).collect();
    let mut renamed = BTreeMap::new();
    for name in names {
        if is_safe_reference(name) || renamed.contains_key(name) {
            continue;
        }
        let Some(portable) = portable_reference(name) else { continue };
        let new = (1..).map(|n| numbered(&portable, n)).find(|new| taken.insert(new.to_lowercase())).expect("a free name");
        renamed.insert(name.to_string(), new);
    }
    renamed
}

/// Compares a file with `bytes` without reading the file whole into memory.
fn same_bytes(path: &Path, bytes: &[u8]) -> io::Result<bool> {
    if fs::metadata(path)?.len() != bytes.len() as u64 {
        return Ok(false);
    }
    let mut file = File::open(path)?;
    let mut read = vec![0; 1 << 16];
    for expected in bytes.chunks(read.len()) {
        let part = &mut read[..expected.len()];
        file.read_exact(part)?;
        if part != expected {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Compares two files without reading either whole into memory.
fn same_contents(a: &Path, b: &Path) -> io::Result<bool> {
    const CHUNK: u64 = 1 << 20;
    if fs::metadata(a)?.len() != fs::metadata(b)?.len() {
        return Ok(false);
    }
    let (mut a, mut b) = (File::open(a)?, File::open(b)?);
    let (mut x, mut y) = (Vec::new(), Vec::new());
    loop {
        x.clear();
        y.clear();
        (&mut a).take(CHUNK).read_to_end(&mut x)?;
        (&mut b).take(CHUNK).read_to_end(&mut y)?;
        if x != y {
            return Ok(false);
        }
        if x.is_empty() {
            return Ok(true);
        }
    }
}

/// Writes `quiz` with the media it uses (read from `media_root`) to `dest` as
/// a quiz file. The file is replaced only once the new one is complete.
pub fn save(quiz: &Quiz, media_root: &Path, dest: &Path) -> Result<()> {
    check(quiz, media_root)?;
    replace_file(dest, |temp| write_zip(quiz, media_root, temp, MAX_UNPACKED_BYTES))
}

/// Deletes every package folder in `cache` except `keep`, and leftovers of
/// interrupted opens. A folder that cannot be deleted yet (on Windows, a
/// file in it is still open) is left for the next time; the first such error
/// is returned once the others are gone.
pub fn prune(cache: &Path, keep: &[&Path]) -> io::Result<()> {
    if !cache.exists() {
        return Ok(());
    }
    let mut first_error = None;
    for entry in fs::read_dir(cache)? {
        let path = entry?.path();
        if path.is_dir() && !keep.iter().any(|k| *k == path) {
            if let Err(e) = fs::remove_dir_all(&path) {
                first_error.get_or_insert(e);
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}

/// Writes the quiz file to `temp`. Refuses a quiz larger than `max_bytes`
/// unpacked or with too many files, which no reader would open.
fn write_zip(quiz: &Quiz, media_root: &Path, temp: &Path, max_bytes: u64) -> Result<()> {
    let media = quiz.media_files();
    if media.len() + 1 > MAX_ENTRIES {
        return Err(QuizFileError::TooManyFiles);
    }
    let json = quiz_json(quiz);
    let mut total = json.len() as u64;
    for reference in &media {
        let path = media_root.join(reference);
        total += fs::metadata(&path).context(FileAction::Read, &path)?.len();
    }
    if total > max_bytes {
        return Err(QuizFileError::TooLarge);
    }
    // A failing ZIP writer is a failing disk here.
    let write_error = |e: zip::result::ZipError| {
        let source = match e {
            zip::result::ZipError::Io(e) => e,
            other => io::Error::other(other),
        };
        QuizFileError::Io { action: FileAction::Write, path: temp.into(), source }
    };
    let mut zip = ZipWriter::new(File::create(temp).context(FileAction::Write, temp)?);
    zip.start_file(QUIZ_JSON, SimpleFileOptions::default().compression_method(CompressionMethod::Deflated))
        .map_err(write_error)?;
    zip.write_all(json.as_bytes()).context(FileAction::Write, temp)?;
    for reference in media {
        let path = media_root.join(reference);
        let mut file = File::open(&path).context(FileAction::Read, &path)?;
        let size = file.metadata().context(FileAction::Read, &path)?.len();
        // Pictures, sounds and videos are compressed already.
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .large_file(size >= u64::from(u32::MAX));
        zip.start_file(reference, options).map_err(write_error)?;
        copy(&mut file, &path, &mut zip, temp, |_| Ok(()))?;
    }
    zip.finish().map_err(write_error)?.sync_all().context(FileAction::Write, temp)
}

/// Copies `from` (the file `source`) to `to` (the file `dest`), handing every
/// chunk to `inspect` first, and returns the number of bytes. Unlike
/// `io::copy`, an error names the side that failed.
fn copy(
    from: &mut impl Read,
    source: &Path,
    to: &mut impl Write,
    dest: &Path,
    mut inspect: impl FnMut(&[u8]) -> Result<()>,
) -> Result<u64> {
    let mut buffer = vec![0; 1 << 16];
    let mut copied = 0;
    loop {
        let n = match from.read(&mut buffer) {
            Ok(0) => return Ok(copied),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e).context(FileAction::Read, source),
        };
        inspect(&buffer[..n])?;
        to.write_all(&buffer[..n]).context(FileAction::Write, dest)?;
        copied += n as u64;
    }
}

fn is_zip(path: &Path) -> Result<bool> {
    let mut magic = [0u8; 4];
    let mut file = File::open(path).context(FileAction::Read, path)?;
    match file.read_exact(&mut magic) {
        Ok(()) => Ok(&magic == ZIP_MAGIC),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e).context(FileAction::Read, path),
    }
}

/// Unpacks `quiz.json` and `media/` into `dest`, media under a portable name
/// (see `renames`). Other entries (folders, links, files some ZIP tools add,
/// names that would leave the media folder) are skipped.
fn unpack_zip(path: &Path, dest: &Path) -> Result<Unpacked> {
    let file = File::open(path).context(FileAction::Read, path)?;
    let mut archive = ZipArchive::new(file)?;
    if archive.len() > MAX_ENTRIES {
        return Err(QuizFileError::TooManyFiles);
    }
    let names = archive.file_names().map(|name| name.map(|name| entry_name(&name))).collect::<std::result::Result<Vec<_>, _>>()?;
    let renamed = renames(names.iter().map(String::as_str));
    let mut hasher = Sha256::new();
    let mut budget = MAX_UNPACKED_BYTES;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() || entry.is_symlink() {
            continue;
        }
        let name = entry_name(&entry.name()?);
        let stored = if name == QUIZ_JSON || is_safe_reference(&name) { Some(name.as_str()) } else { renamed.get(&name).map(String::as_str) };
        let Some(stored) = stored else { continue };
        budget -= write_file(dest, stored, &mut entry, path, budget, &mut hasher)?;
    }
    Ok(Unpacked { hash: finish(hasher), renamed })
}

/// A ZIP entry's name with `/` between folders, as some ZIP tools on Windows
/// write `\`.
fn entry_name(raw: &str) -> String {
    raw.replace('\\', "/")
}

/// Copies a plain quiz JSON file and the media it uses (next to it) into
/// `dest`. Media names are not renamed: the author wrote them by hand, and
/// `problems` lists the bad ones.
fn copy_json(path: &Path, dest: &Path) -> Result<Unpacked> {
    let json = fs::read_to_string(path).context(FileAction::Read, path)?;
    let quiz = Quiz::from_json(&json)?;
    let source_root = path.parent().unwrap_or(Path::new("."));
    let mut hasher = Sha256::new();
    let mut budget = MAX_UNPACKED_BYTES;
    budget -= write_file(dest, QUIZ_JSON, &mut json.as_bytes(), path, budget, &mut hasher)?;
    for reference in quiz.media_files() {
        // Only safe names are read: nothing outside the folder, and no
        // device a name like `CON` opens on Windows.
        if !is_safe_reference(reference) {
            continue;
        }
        let source = source_root.join(reference);
        let Ok(mut media) = File::open(&source) else {
            // `problems` reports it as missing.
            continue;
        };
        budget -= write_file(dest, reference, &mut media, &source, budget, &mut hasher)?;
    }
    Ok(Unpacked { hash: finish(hasher), renamed: BTreeMap::new() })
}

/// Writes one file of a package from `contents` (read from the file
/// `source`), adds its name and contents to the hash and returns its size.
/// Fails if it would exceed `budget` bytes.
fn write_file(
    dest: &Path,
    name: &str,
    contents: &mut impl Read,
    source: &Path,
    budget: u64,
    hasher: &mut Sha256,
) -> Result<u64> {
    let path = dest.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context(FileAction::CreateFolder, parent)?;
    }
    let mut file = File::create(&path).context(FileAction::Write, &path)?;
    hasher.update(name.as_bytes());
    hasher.update([0]);
    let mut left = budget;
    let copied = copy(contents, source, &mut file, &path, |chunk| {
        left = left.checked_sub(chunk.len() as u64).ok_or(QuizFileError::TooLarge)?;
        hasher.update(chunk);
        Ok(())
    })?;
    hasher.update(copied.to_le_bytes());
    // A package folder is used as it is after a restart, so it must be complete on disk.
    file.sync_all().context(FileAction::Write, &path)?;
    Ok(copied)
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
    use quiz_core::{AnswerMode, CorrectAnswer, Question, Slide, SlideContent, MAX_EVERYONE_ANSWERS, MAX_POINTS};

    /// The problem `check` reports when the quiz's flag picture is missing.
    fn flag_missing() -> Vec<ValidationError> {
        let problem = Problem::Media { file: "media/flag.png".into(), issue: MediaIssue::Missing };
        vec![ValidationError { slide_id: Some("intro".into()), problem }]
    }

    /// A fresh, empty folder for one test.
    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("quiz-file-{name}-{}-{}", std::process::id(), unique_suffix()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn quiz_with_picture(picture: &str) -> Quiz {
        let mut slide = Slide::new("intro", SlideContent::Billboard { title: "Round 1".into(), text: String::new() });
        slide.picture = Some(picture.into());
        Quiz::new("Test", vec![slide])
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

    #[cfg(unix)]
    #[test]
    fn a_stale_folder_that_cannot_be_deleted_is_moved_aside() {
        use std::os::unix::fs::PermissionsExt;
        let quiz = quiz_with_picture("media/flag.png");
        let media = media_folder("stale-media", &[("media/flag.png", b"png bytes")]);
        let file = temp("stale-out").join("test.quiz");
        save(&quiz, &media, &file).unwrap();
        let cache = temp("stale-cache");
        let first = open(&file, &cache).unwrap();
        // The package is broken, and a folder in it cannot be emptied.
        fs::remove_file(first.root.join(QUIZ_JSON)).unwrap();
        let locked = first.root.join("locked");
        fs::create_dir(&locked).unwrap();
        fs::write(locked.join("file"), b"x").unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
        let unlock = |cache: &Path| {
            for entry in fs::read_dir(cache).unwrap() {
                let locked = entry.unwrap().path().join("locked");
                if locked.exists() {
                    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
                }
            }
        };
        if fs::remove_file(locked.join("file")).is_ok() {
            // Running as root, which may delete anything: nothing to test.
            unlock(&cache);
            return;
        }
        let again = open(&file, &cache).unwrap();
        assert_eq!(again.root, first.root);
        assert_eq!(again.quiz, quiz);
        unlock(&cache);
        prune(&cache, &[&again.root]).unwrap();
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
    }

    /// On Windows a folder with an open file in it can be neither renamed
    /// nor emptied, as when a previous show's video still plays from it.
    #[cfg(windows)]
    #[test]
    fn a_stale_folder_held_open_leaves_the_package_to_another_folder() {
        use std::os::windows::fs::OpenOptionsExt;
        let quiz = quiz_with_picture("media/flag.png");
        let media = media_folder("held-media", &[("media/flag.png", b"png bytes")]);
        let file = temp("held-out").join("test.quiz");
        save(&quiz, &media, &file).unwrap();
        let cache = temp("held-cache");
        let first = open(&file, &cache).unwrap();
        fs::remove_file(first.root.join(QUIZ_JSON)).unwrap();
        let held = fs::OpenOptions::new().read(true).share_mode(0).open(first.root.join("media/flag.png")).unwrap();

        // The stale folder stays while the file is open, so the package goes elsewhere.
        let again = open(&file, &cache).unwrap();
        assert_eq!(again.quiz, quiz);
        // Opening once more reuses that folder.
        assert_eq!(open(&file, &cache).unwrap().root, again.root);
        drop(held);
        prune(&cache, &[&again.root]).unwrap();
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
    }

    #[test]
    fn saving_needs_every_media_file() {
        let quiz = quiz_with_picture("media/flag.png");
        let file = temp("save-missing").join("test.quiz");
        let err = save(&quiz, &temp("save-missing-media"), &file).unwrap_err();
        assert!(matches!(err, QuizFileError::Invalid(ref p) if *p == flag_missing()), "{err}");
        assert!(!file.exists());
    }

    #[test]
    fn saving_refuses_a_quiz_no_reader_would_open() {
        let quiz = quiz_with_picture("media/flag.png");
        let media = media_folder("save-big", &[("media/flag.png", &[0u8; 1000])]);
        let temp = temp("save-big-out").join("test.quiz");
        assert!(matches!(write_zip(&quiz, &media, &temp, 1000), Err(QuizFileError::TooLarge)));
        assert!(write_zip(&quiz, &media, &temp, 2000).is_ok());
    }

    #[test]
    fn a_failed_save_names_the_file_not_its_temporary_copy() {
        let folder = temp("save-blocked");
        let dest = folder.join("test.quiz");
        // A folder where the temporary file would go makes creating it fail.
        fs::create_dir_all(folder.join(".test.quiz.saving")).unwrap();
        let err = write_atomic(&dest, b"x").unwrap_err();
        assert!(matches!(err, QuizFileError::Io { action: FileAction::Write, ref path, .. } if *path == dest), "{err}");
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
        assert!(matches!(err, QuizFileError::Invalid(ref p) if *p == flag_missing()), "{err}");
        assert!(!cache.parent().unwrap().join("evil.png").exists());
        // A failed open leaves nothing behind.
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 0);
    }

    #[test]
    fn opening_again_repairs_a_package_folder_left_half_deleted() {
        let media = media_folder("half-media", &[("media/flag.png", b"png bytes")]);
        let file = temp("half-file").join("test.quiz");
        save(&quiz_with_picture("media/flag.png"), &media, &file).unwrap();
        let cache = temp("half-cache");
        let package = open(&file, &cache).unwrap();
        // A `prune` that stopped half way, e.g. at a file Windows kept locked.
        fs::remove_file(package.root.join("media/flag.png")).unwrap();

        let again = open(&file, &cache).unwrap();
        assert_eq!(again.root, package.root);
        assert_eq!(fs::read(again.root.join("media/flag.png")).unwrap(), b"png bytes");
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
        let outer = temp("json-escape");
        fs::write(outer.join("secret.png"), b"secret").unwrap();
        let folder = outer.join("quiz");
        fs::create_dir_all(&folder).unwrap();
        let quiz = quiz_with_picture("media/../../secret.png");
        fs::write(folder.join("quiz.json"), serde_json::to_vec(&quiz).unwrap()).unwrap();
        assert!(matches!(open(&folder.join("quiz.json"), &temp("json-escape-cache")), Err(QuizFileError::Invalid(_))));

        // The editor opens it to be fixed, without the file outside.
        let draft = temp("json-escape-draft").join("draft");
        let Opened { quiz: opened, changed } = open_into(&folder.join("quiz.json"), &draft).unwrap();
        assert_eq!((&opened, changed), (&quiz, false));
        assert!(!draft.join("media").exists());
        let problem = Problem::Media { file: "media/../../secret.png".into(), issue: MediaIssue::BadName };
        assert_eq!(problems(&opened, &draft), [ValidationError { slide_id: Some("intro".into()), problem }]);
    }

    /// A quiz file made before the answer and points limits: an
    /// everyone-answers question with five answers and too many points.
    fn quiz_breaking_newer_rules() -> Quiz {
        let question = Question {
            text: "?".into(),
            mode: AnswerMode::Everyone,
            answers: vec!["a".into(); MAX_EVERYONE_ANSWERS + 1],
            correct: vec![CorrectAnswer::full(0)],
            time_limit_s: 20,
            points: MAX_POINTS * 2,
            points_end: None,
            penalty: 0,
        };
        Quiz::new("Old", vec![Slide::new("q", SlideContent::Question(question))])
    }

    #[test]
    fn the_editor_opens_a_quiz_the_show_refuses_and_lists_what_to_fix() {
        let quiz = quiz_breaking_newer_rules();
        let file = zip_of("old-rules", &[(QUIZ_JSON, &serde_json::to_vec(&quiz).unwrap())]);
        assert!(matches!(open(&file, &temp("old-rules-cache")), Err(QuizFileError::Invalid(_))));

        let draft = temp("old-rules-draft").join("draft");
        let Opened { quiz: opened, changed } = open_into(&file, &draft).unwrap();
        assert_eq!((&opened, changed), (&quiz, false));
        let found: Vec<Problem> = problems(&opened, &draft).into_iter().map(|e| e.problem).collect();
        assert_eq!(
            found,
            [Problem::TooManyAnswers { max: MAX_EVERYONE_ANSWERS }, Problem::PointsOutOfRange { max: MAX_POINTS }]
        );
    }

    #[test]
    fn the_editor_gives_media_names_older_versions_allowed_portable_names() {
        let mut quiz = quiz_with_picture("media/what?.png");
        let mut second = quiz.slides[0].clone();
        second.id = "second".into();
        second.picture = Some("media/what_.png".into());
        quiz.slides.push(second);
        let json = serde_json::to_vec(&quiz).unwrap();
        let file = zip_of(
            "old-names",
            &[(QUIZ_JSON, &json), ("media/what?.png", b"question"), ("media/what_.png", b"underscore"), ("media/../x.png", b"x")],
        );
        // The show plays files as they are.
        let err = open(&file, &temp("old-names-cache")).unwrap_err();
        let bad_name = Problem::Media { file: "media/what?.png".into(), issue: MediaIssue::BadName };
        assert!(matches!(err, QuizFileError::Invalid(ref p) if p.iter().any(|e| e.problem == bad_name)), "{err}");

        let draft = temp("old-names-draft").join("draft");
        let Opened { quiz: opened, changed } = open_into(&file, &draft).unwrap();
        assert!(changed);
        // The new name takes no name already used.
        let pictures: Vec<_> = opened.slides.iter().map(|s| s.picture.as_deref().unwrap()).collect();
        assert_eq!(pictures, ["media/what_-2.png", "media/what_.png"]);
        assert_eq!(fs::read(draft.join("media/what_-2.png")).unwrap(), b"question");
        assert_eq!(fs::read(draft.join("media/what_.png")).unwrap(), b"underscore");
        assert_eq!(read_quiz_json(&draft).unwrap(), opened);
        assert_eq!(problems(&opened, &draft), []);
    }

    #[test]
    fn renaming_ignores_letter_case_and_numbers_before_the_extension() {
        let renamed = renames(["media/A_.png", "media/a?.png", "media/a*.png", "media/x/../y.png"].into_iter());
        let expected = [("media/a*.png", "media/a_-3.png"), ("media/a?.png", "media/a_-2.png")];
        assert_eq!(renamed, expected.map(|(old, new)| (old.to_string(), new.to_string())).into());
        assert_eq!(numbered("media/round.1/a.b.png", 2), "media/round.1/a.b-2.png");
        assert_eq!(numbered("media/round.1/noext", 3), "media/round.1/noext-3");
        assert_eq!(numbered("media/.hidden", 2), "media/.hidden-2");
    }

    #[test]
    fn unpacking_stops_at_the_size_limit() {
        let dest = temp("budget");
        let mut hasher = Sha256::new();
        let source = Path::new("big.mp4");
        let err = write_file(&dest, "media/big.mp4", &mut &[0u8; 100][..], source, 99, &mut hasher).unwrap_err();
        assert!(matches!(err, QuizFileError::TooLarge));
        assert_eq!(write_file(&dest, "media/ok.mp4", &mut &[0u8; 100][..], source, 100, &mut hasher).unwrap(), 100);
    }

    #[test]
    fn imports_media_under_safe_unique_names() {
        let root = temp("import");
        let sources = media_folder(
            "import-src",
            &[("a/Flag.png", b"one"), ("b/Flag.png", b"two"), ("c/we:ird?.jpg", b"x"), ("d/CON.png", b"x")],
        );
        let import = |path: &str| import_media(&root, MediaSource::File(&sources.join(path))).unwrap();
        assert_eq!(import("a/Flag.png"), "media/Flag.png");
        // Same name, same contents: reused. Same name, other contents: numbered.
        assert_eq!(import("a/Flag.png"), "media/Flag.png");
        assert_eq!(import("b/Flag.png"), "media/Flag-2.png");
        assert_eq!(fs::read(root.join("media/Flag-2.png")).unwrap(), b"two");
        let weird = import("c/we:ird?.jpg");
        assert_eq!(weird, "media/we_ird_.jpg");
        assert!(is_safe_reference(&weird));
        assert_eq!(import("d/CON.png"), "media/_CON.png");
    }

    #[test]
    fn a_media_copy_that_fails_leaves_no_file() {
        let root = temp("import-failed");
        // A folder opens like a file on some systems and fails on the first read.
        let folder = temp("import-failed-src").join("Song.mp3");
        fs::create_dir_all(&folder).unwrap();
        assert!(import_media(&root, MediaSource::File(&folder)).is_err());
        let left: Vec<_> = fs::read_dir(root.join("media")).map(|dir| dir.flatten().map(|entry| entry.file_name()).collect()).unwrap_or_default();
        assert!(left.is_empty(), "left behind: {left:?}");
    }

    #[test]
    fn imports_a_dropped_file_from_its_bytes_like_one_on_disk() {
        let root = temp("import-bytes");
        let dropped = |name: &str, bytes: &[u8]| import_media(&root, MediaSource::Bytes { name, bytes }).unwrap();
        assert_eq!(dropped("Potlesk.mp3", b"one"), "media/Potlesk.mp3");
        assert_eq!(dropped("Potlesk.mp3", b"one"), "media/Potlesk.mp3");
        assert_eq!(dropped("Potlesk.mp3", b"ONE"), "media/Potlesk-2.mp3");
        assert_eq!(dropped("a/b?.mp3", b"x"), "media/a_b_.mp3");
        // Longer than one read buffer, differing only at the end.
        let long = vec![7u8; 200_000];
        let mut other = long.clone();
        other[199_999] = 8;
        assert_eq!(dropped("long.wav", &long), "media/long.wav");
        assert_eq!(dropped("long.wav", &long), "media/long.wav");
        assert_eq!(dropped("long.wav", &other), "media/long-2.wav");
        assert_eq!(fs::read(root.join("media/long-2.wav")).unwrap()[199_999], 8);
    }

    #[test]
    fn a_draft_is_written_and_reopened_for_editing() {
        let media = media_folder("draft-media", &[("media/flag.png", b"png")]);
        let file = temp("draft-file").join("test.quiz");
        save(&quiz_with_picture("media/flag.png"), &media, &file).unwrap();

        let draft = temp("draft").join("draft");
        let mut quiz = open_into(&file, &draft).unwrap().quiz;
        quiz.title = "Edited".into();
        write_quiz_json(&draft, &quiz).unwrap();
        assert_eq!(read_checked(&draft).unwrap().title, "Edited");
        // Opening into an existing folder is refused, nothing is replaced.
        assert!(open_into(&file, &draft).is_err());
    }

    #[test]
    fn problems_name_the_slide_with_missing_media() {
        let found = problems(&quiz_with_picture("media/flag.png"), &temp("problems"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].slide_id.as_deref(), Some("intro"));
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
