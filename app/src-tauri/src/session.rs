//! Crash-safe show sessions. Each show gets a folder with a copy of the quiz and
//! an append-only journal of accepted commands, synced to disk on every write.
//! Replaying the journal on the quiz copy rebuilds the show exactly.
//!
//! The journal is opened for plain writing, not in append mode: on Windows an
//! append-mode handle lacks the write-data access that truncating needs
//! (Rust std, `library/std/src/sys/fs/windows.rs`, `get_access_mode`), and
//! truncating is how a torn line is cut off on resume and a failed write is
//! taken back. Every line is written at the end of the whole lines instead.

use crate::app_log;
use quiz_core::{Quiz, QuizJsonError, Stamped};
use quiz_file::{Context, FileAction, QuizFileError};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const QUIZ_FILE: &str = "quiz.json";
const META_FILE: &str = "meta.json";
const JOURNAL_FILE: &str = "journal.jsonl";
/// Where the journal is kept as it was each time resuming cut a torn last
/// line off it (`torn/journal-1.jsonl`, `-2`, ...).
const TORN_DIR: &str = "torn";
/// A UTF-8 byte order mark, which an editor may put at the start of a file.
const BOM: &[u8] = b"\xEF\xBB\xBF";

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub quiz_title: String,
    pub source_path: Option<String>,
    pub started_ms: u64,
    /// Folder with the quiz's unpacked media (see the quiz-file crate).
    #[serde(default)]
    pub media_root: Option<String>,
    /// The `RULES_VERSION` the show began under; 0 for a show from before it was kept.
    #[serde(default)]
    pub rules_version: u32,
}

impl SessionMeta {
    /// Whether replaying the show's journal may give another show than the
    /// one it recorded: it began under rules from before
    /// `REPLAYS_AS_PLAYED_FROM`, or under a newer app's.
    pub fn rules_changed(&self) -> bool {
        !(quiz_core::REPLAYS_AS_PLAYED_FROM..=quiz_core::RULES_VERSION).contains(&self.rules_version)
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub meta: SessionMeta,
    pub command_count: usize,
}

pub struct Session {
    journal: File,
    /// Bytes of whole, accepted lines in the journal.
    len: u64,
    /// A failed write could not be taken back; nothing more may be appended.
    broken: bool,
}

/// Why the last show cannot be read or resumed.
#[derive(Debug, thiserror::Error)]
pub enum ResumeError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("meta.json: {0}")]
    Meta(#[from] serde_json::Error),
    /// The show's copy of the quiz does not read, such as one from a newer app.
    #[error(transparent)]
    Quiz(#[from] QuizJsonError),
    /// Line `line` (from 1) of the journal is whole but does not read as a
    /// command: a newer app wrote it, or the disk damaged it. Replaying
    /// without it would rebuild another show, so the show is not resumed
    /// and its journal is left as it is.
    #[error("line {line} of the show's journal does not read")]
    Damaged { line: usize },
}

/// Why a command was not journaled, so it must be rejected.
#[derive(Debug, thiserror::Error)]
pub enum NotSaved {
    #[error(transparent)]
    Io(#[from] io::Error),
    /// An earlier write failed and could not be taken back, so nothing more
    /// is appended until the show is resumed. Resuming cuts a torn line off;
    /// a line that was written whole (only syncing it failed) stays and is
    /// replayed, although the command was refused.
    #[error("the journal could not be repaired after a failed write")]
    Broken,
}

/// A session reopened after a restart, with everything needed to rebuild the show.
pub struct Resumed {
    pub session: Session,
    pub quiz: Quiz,
    pub journal: Vec<Stamped>,
    pub meta: SessionMeta,
}

impl Session {
    /// Starts a new session folder under `root`.
    pub fn create(root: &Path, quiz: &Quiz, meta: &SessionMeta) -> Result<Session, QuizFileError> {
        let dir = new_dir(root, meta.started_ms)?;
        // A power cut must not lose the new folder (or a sessions folder made
        // just now) or the journal's entry in it, only their unsynced data.
        for created in root.parent().into_iter().chain([root]) {
            quiz_file::sync_dir(created)?;
        }
        // The quiz last: a folder without it is a show that did not start (`latest_dir`).
        write_json(&dir.join(META_FILE), meta)?;
        write_json(&dir.join(QUIZ_FILE), quiz)?;
        let path = dir.join(JOURNAL_FILE);
        let journal = open_journal(&path).context(FileAction::Write, &path)?;
        quiz_file::sync_dir(&dir)?;
        let len = journal.metadata().context(FileAction::Read, &path)?.len();
        Ok(Session { journal, len, broken: false })
    }

    /// Reopens the most recent session for appending, with its quiz and journal.
    pub fn resume_latest(root: &Path) -> Result<Option<Resumed>, ResumeError> {
        let Some(dir) = latest_dir(root)? else { return Ok(None) };
        let quiz = Quiz::from_json(&fs::read_to_string(dir.join(QUIZ_FILE))?)?;
        let meta = read_meta(&dir)?;
        let path = dir.join(JOURNAL_FILE);
        let (journal, kept) = read_journal(&path)?;
        // A torn last line, which no window ever saw, is cut off so new
        // commands start on a clean line. The journal as it was is kept
        // aside first if it can be: it is only there to look into, so a
        // full disk does not stop the show from resuming.
        let torn = fs::metadata(&path).is_ok_and(|journal| journal.len() > kept);
        if torn {
            if let Err(e) = keep_aside(&path, &dir) {
                app_log::note(&format!("the torn journal {} was not kept aside: {e}", path.display()));
            }
        }
        let file = open_journal(&path)?;
        if torn {
            file.set_len(kept)?;
            file.sync_data()?;
        }
        let session = Session { journal: file, len: kept, broken: false };
        Ok(Some(Resumed { session, quiz, journal, meta }))
    }

    pub fn latest_summary(root: &Path) -> Result<Option<SessionSummary>, ResumeError> {
        let Some(dir) = latest_dir(root)? else { return Ok(None) };
        let meta = read_meta(&dir)?;
        let command_count = read_journal(&dir.join(JOURNAL_FILE))?.0.len();
        Ok(Some(SessionSummary { meta, command_count }))
    }

    /// Lets appends work again after `break_for_test`.
    #[cfg(test)]
    pub fn repair_for_test(&mut self) {
        self.broken = false;
    }

    /// Makes every later append fail, as after a write that could not be taken back.
    #[cfg(test)]
    pub fn break_for_test(&mut self) {
        self.broken = true;
    }

    /// Appends one accepted command and waits until it is on disk. On an
    /// error the command is not in the journal and must be rejected.
    pub fn append(&mut self, stamped: &Stamped) -> Result<(), NotSaved> {
        if self.broken {
            return Err(NotSaved::Broken);
        }
        let mut line = serde_json::to_string(stamped).map_err(io::Error::from)?;
        line.push('\n');
        append_line(&mut self.journal, &mut self.len, line.as_bytes()).inspect_err(|e| {
            if let AppendError::Unrepaired(_) = e {
                self.broken = true;
            }
        }).map_err(|e| NotSaved::Io(e.into_io()))
    }
}

/// Copies the journal at `path` into the session's `TORN_DIR`, under a name
/// no earlier copy has.
fn keep_aside(path: &Path, dir: &Path) -> io::Result<PathBuf> {
    let torn = dir.join(TORN_DIR);
    fs::create_dir_all(&torn)?;
    let mut number = 1u32;
    loop {
        let copy = torn.join(format!("journal-{number}.jsonl"));
        match OpenOptions::new().write(true).create_new(true).open(&copy) {
            Ok(mut file) => {
                io::copy(&mut File::open(path)?, &mut file)?;
                // On disk before the journal is cut, so a crash in between
                // loses neither: the copy, its name in `TORN_DIR` and that
                // folder's name in the session's.
                file.sync_all()?;
                sync_dir(&torn)?;
                sync_dir(dir)?;
                return Ok(copy);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => number += 1,
            Err(e) => return Err(e),
        }
    }
}

fn read_meta(dir: &Path) -> Result<SessionMeta, ResumeError> {
    Ok(serde_json::from_str(&fs::read_to_string(dir.join(META_FILE))?)?)
}

/// Opens a journal for writing (and creates it), without append mode: see the module comment.
fn open_journal(path: &Path) -> io::Result<File> {
    OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path)
}

/// What the journal is written to; tests use a file that fails on purpose.
trait JournalFile {
    /// Writes `line` starting at byte `at`.
    fn write_at(&mut self, at: u64, line: &[u8]) -> io::Result<()>;
    fn sync(&mut self) -> io::Result<()>;
    fn truncate(&mut self, len: u64) -> io::Result<()>;
}

impl JournalFile for File {
    fn write_at(&mut self, at: u64, line: &[u8]) -> io::Result<()> {
        // After a failed write the position is past the whole lines, so it is set every time.
        self.seek(SeekFrom::Start(at))?;
        self.write_all(line)
    }

    fn sync(&mut self) -> io::Result<()> {
        self.sync_data()
    }

    fn truncate(&mut self, len: u64) -> io::Result<()> {
        self.set_len(len)
    }
}

#[derive(Debug)]
enum AppendError {
    /// The line is not in the journal; the journal is as before.
    Failed(io::Error),
    /// The line could not be taken back off; the journal may end in a torn line.
    Unrepaired(io::Error),
}

impl AppendError {
    fn into_io(self) -> io::Error {
        match self {
            AppendError::Failed(e) | AppendError::Unrepaired(e) => e,
        }
    }
}

/// Appends `line` to a journal whose whole lines take `len` bytes, and syncs.
/// A failed write or sync is taken back off, so the journal never holds a
/// partial line (which would hide every later command on resume) nor a
/// command that was rejected (which would be replayed).
fn append_line(file: &mut impl JournalFile, len: &mut u64, line: &[u8]) -> Result<(), AppendError> {
    match file.write_at(*len, line).and_then(|()| file.sync()) {
        Ok(()) => {
            *len += line.len() as u64;
            Ok(())
        }
        Err(e) => match file.truncate(*len).and_then(|()| file.sync()) {
            Ok(()) => Err(AppendError::Failed(e)),
            Err(_) => Err(AppendError::Unrepaired(e)),
        },
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), QuizFileError> {
    let json = serde_json::to_string_pretty(value).map_err(io::Error::from).context(FileAction::Write, path)?;
    quiz_file::write_atomic(path, json.as_bytes())
}

fn sync_dir(dir: &Path) -> io::Result<()> {
    quiz_file::sync_dir(dir).map_err(io::Error::other)
}

/// The session folders under `root` and their numbers. A folder is named by
/// its show's start in milliseconds, or a number past the last folder's
/// when the clock went back (`new_dir`): the latest show has the highest.
fn session_dirs(root: &Path) -> io::Result<Vec<(u64, PathBuf)>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut dirs = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if let Some(number) = path.file_name().and_then(|n| n.to_str()).and_then(|n| n.parse::<u64>().ok()) {
            dirs.push((number, path));
        }
    }
    Ok(dirs)
}

/// Makes the folder of a show started at `started_ms`, with a name higher
/// than every folder's before it, so two shows never share one.
fn new_dir(root: &Path, started_ms: u64) -> Result<PathBuf, QuizFileError> {
    fs::create_dir_all(root).context(FileAction::CreateFolder, root)?;
    // A number past the highest there is, which only a folder made by hand
    // can be; past the highest of all, that folder is in the way.
    let after = |number: u64| {
        let in_the_way = root.join(number.to_string());
        number.checked_add(1).ok_or(io::Error::from(io::ErrorKind::AlreadyExists)).context(FileAction::CreateFolder, &in_the_way)
    };
    let mut number = match session_dirs(root).context(FileAction::Read, root)?.into_iter().map(|(number, _)| number).max() {
        Some(last) => started_ms.max(after(last)?),
        None => started_ms,
    };
    loop {
        let dir = root.join(number.to_string());
        match fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => number = after(number)?,
            Err(e) => return Err(e).context(FileAction::CreateFolder, &dir),
        }
    }
}

/// The latest show's folder, if any has its quiz.
fn latest_dir(root: &Path) -> io::Result<Option<PathBuf>> {
    let mut dirs = session_dirs(root)?;
    dirs.retain(|(_, path)| path.join(QUIZ_FILE).exists());
    Ok(dirs.into_iter().max_by_key(|(number, _)| *number).map(|(_, path)| path))
}

/// Reads the journal: its commands, and how many of its bytes hold them.
///
/// Each line is synced before the next is written, so only the last one can
/// be a line the app never accepted, and that is left out: one without its
/// newline, which was being written when the app stopped, or a whole one
/// that is no journal line at all (`is_journal_line`), which a power cut
/// leaves when the disk kept the newline but not the bytes before it. Any
/// other line that does not read as a command refuses the journal
/// (`ResumeError::Damaged`): a newer app wrote it, or the disk damaged it.
fn read_journal(path: &Path) -> Result<(Vec<Stamped>, u64), ResumeError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
        Err(e) => return Err(e.into()),
    };
    let start = if bytes.starts_with(BOM) { BOM.len() } else { 0 };
    let whole = bytes.iter().rposition(|&b| b == b'\n').map_or(start, |last| last + 1);
    let mut commands = Vec::new();
    let mut end = start;
    for (index, line) in bytes[start..whole].split_inclusive(|&b| b == b'\n').enumerate() {
        let line_start = end;
        end += line.len();
        let line = line.trim_ascii();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_slice(line) {
            Ok(command) => commands.push(command),
            Err(_) if end == bytes.len() && !is_journal_line(line) => return Ok((commands, line_start as u64)),
            Err(_) => return Err(ResumeError::Damaged { line: index + 1 }),
        }
    }
    Ok((commands, whole as u64))
}

/// Whether `line` is a journal line as some version of the app writes it:
/// an object with a time and a command, whatever the command. Every version
/// writes its lines so and every later one must keep to it, or a newer
/// app's last line would be cut off as torn. The app never writes a NUL.
fn is_journal_line(line: &[u8]) -> bool {
    !line.contains(&0)
        && serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(line).is_ok_and(|line| line.contains_key("atMs") && line.contains_key("command"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiz_core::{Button, Command};

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("buzzdome-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn meta(started_ms: u64) -> SessionMeta {
        SessionMeta {
            quiz_title: format!("show {started_ms}"),
            source_path: None,
            started_ms,
            media_root: None,
            rules_version: quiz_core::RULES_VERSION,
        }
    }

    /// Starts a show under `root` that journals `Next` and a named player.
    fn journaled_show(root: &Path, started_ms: u64) -> PathBuf {
        let mut session = Session::create(root, &Quiz::new("T", vec![]), &meta(started_ms)).unwrap();
        session.append(&Stamped { at_ms: 5, command: Command::Next }).unwrap();
        session.append(&Stamped { at_ms: 6, command: Command::AddPlayer { buzzer: 1, name: "Šneci".into() } }).unwrap();
        latest_dir(root).unwrap().unwrap()
    }

    #[test]
    fn a_line_torn_inside_a_czech_letter_is_cut_off_and_kept_aside() {
        let root = temp_root("torn-utf8");
        let dir = journaled_show(&root, 1);
        let journal = dir.join(JOURNAL_FILE);
        let whole = fs::read(&journal).unwrap();
        // The write stopped between the two bytes of "Š".
        let torn = [&whole[..], &b"{\"atMs\":7,\"command\":{\"type\":\"addPlayer\",\"buzzer\":2,\"name\":\"\xc5"[..]].concat();
        fs::write(&journal, &torn).unwrap();

        assert_eq!(Session::latest_summary(&root).unwrap().unwrap().command_count, 2);
        let resumed = Session::resume_latest(&root).unwrap().unwrap();
        assert_eq!(resumed.journal.len(), 2);
        assert_eq!(fs::read(&journal).unwrap(), whole, "the torn line is cut off");
        assert_eq!(fs::read(dir.join(TORN_DIR).join("journal-1.jsonl")).unwrap(), torn, "the journal as it was is kept");
        fs::remove_dir_all(&root).unwrap();
    }

    /// Writes `tail` after the journal of a show that journaled two commands,
    /// and gives the show's folder and the journal as it was before.
    fn show_ending_in(root: &Path, tail: &[u8]) -> (PathBuf, Vec<u8>) {
        let dir = journaled_show(root, 1);
        let journal = dir.join(JOURNAL_FILE);
        let whole = fs::read(&journal).unwrap();
        fs::write(&journal, [&whole[..], tail].concat()).unwrap();
        (dir, whole)
    }

    #[test]
    fn a_whole_last_line_that_is_not_json_is_cut_off_like_a_torn_one() {
        // A power cut kept the page with the newline but not the bytes before it.
        for (name, tail) in [
            ("nul", &b"\0\0\0\0\0\0\0\0\n"[..]),
            ("cut", b"{\"atMs\":7,\"comm\n"),
            ("nul-in-json", b"{\"atMs\":7,\"command\":{\"type\":\"next\"}}\0\n"),
            ("json-but-no-line", b"7\n"),
            ("object-but-no-line", b"{\"atMs\":7}\n"),
        ] {
            let root = temp_root(&format!("garbled-{name}"));
            let (dir, whole) = show_ending_in(&root, tail);
            assert_eq!(Session::latest_summary(&root).unwrap().unwrap().command_count, 2, "{name}");
            let resumed = Session::resume_latest(&root).unwrap().unwrap();
            assert_eq!(resumed.journal.len(), 2, "{name}");
            assert_eq!(fs::read(dir.join(JOURNAL_FILE)).unwrap(), whole, "{name}");
            assert!(dir.join(TORN_DIR).join("journal-1.jsonl").exists(), "{name}");
            fs::remove_dir_all(&root).unwrap();
        }
    }

    #[test]
    fn a_last_line_from_a_newer_app_refuses_the_resume() {
        let root = temp_root("newer-last");
        let tail = b"{\"atMs\":7,\"command\":{\"type\":\"newerCommand\"}}\n";
        let (dir, whole) = show_ending_in(&root, tail);
        assert!(matches!(Session::resume_latest(&root), Err(ResumeError::Damaged { line: 3 })));
        assert_eq!(fs::read(dir.join(JOURNAL_FILE)).unwrap(), [&whole[..], tail].concat());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_garbled_line_before_others_refuses_the_resume() {
        for (name, tail) in [
            ("then-a-line", &b"\0\0\0\n{\"atMs\":8,\"command\":{\"type\":\"next\"}}\n"[..]),
            ("then-a-torn-line", b"\0\0\0\n{\"atMs\":8,\"comm"),
            ("then-an-empty-line", b"\0\0\0\n\n"),
        ] {
            let root = temp_root(&format!("garbled-middle-{name}"));
            let (dir, whole) = show_ending_in(&root, tail);
            assert!(matches!(Session::resume_latest(&root), Err(ResumeError::Damaged { line: 3 })), "{name}");
            assert_eq!(fs::read(dir.join(JOURNAL_FILE)).unwrap(), [&whole[..], tail].concat(), "{name}");
            fs::remove_dir_all(&root).unwrap();
        }
    }

    #[test]
    fn journals_an_editor_wrote_or_with_nothing_whole_read() {
        let root = temp_root("shapes");
        let dir = journaled_show(&root, 1);
        let journal = dir.join(JOURNAL_FILE);
        let lines = fs::read_to_string(&journal).unwrap();
        // Saved again in an editor: CRLF line ends and a byte order mark.
        fs::write(&journal, [BOM, lines.replace('\n', "\r\n").as_bytes()].concat()).unwrap();
        assert_eq!(Session::resume_latest(&root).unwrap().unwrap().journal.len(), 2);
        // Nothing at all, a torn line, a byte order mark alone or before a
        // torn line, and a garbled line alone.
        let bom_torn = [BOM, b"{\"atMs\":1,\"comm"].concat();
        for (contents, kept) in [(&b""[..], 0), (b"{\"atMs\":1,\"comm", 0), (BOM, 3), (&bom_torn[..], 3), (b"\0\0\0\n", 0)] {
            fs::write(&journal, contents).unwrap();
            let mut resumed = Session::resume_latest(&root).unwrap().unwrap();
            assert!(resumed.journal.is_empty());
            assert_eq!(fs::metadata(&journal).unwrap().len(), kept);
            resumed.session.append(&Stamped { at_ms: 9, command: Command::Next }).unwrap();
            assert_eq!(Session::latest_summary(&root).unwrap().unwrap().command_count, 1);
        }
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_torn_line_is_cut_off_even_when_it_cannot_be_kept_aside() {
        let root = temp_root("no-backup");
        let (dir, whole) = show_ending_in(&root, b"{\"atMs\":7,\"comm");
        // A file where the copies' folder belongs, so the copy fails.
        fs::write(dir.join(TORN_DIR), b"").unwrap();
        assert_eq!(Session::resume_latest(&root).unwrap().unwrap().journal.len(), 2);
        assert_eq!(fs::read(dir.join(JOURNAL_FILE)).unwrap(), whole);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn each_torn_line_cut_off_is_kept_in_a_copy_of_its_own() {
        let root = temp_root("two-backups");
        let (dir, whole) = show_ending_in(&root, b"{\"atMs\":7,\"comm");
        drop(Session::resume_latest(&root).unwrap().unwrap());
        fs::write(dir.join(JOURNAL_FILE), [&whole[..], b"\0\0\n"].concat()).unwrap();
        drop(Session::resume_latest(&root).unwrap().unwrap());
        let copy = |n: u32| fs::read(dir.join(TORN_DIR).join(format!("journal-{n}.jsonl"))).unwrap();
        assert_eq!(copy(1), [&whole[..], b"{\"atMs\":7,\"comm"].concat());
        assert_eq!(copy(2), [&whole[..], b"\0\0\n"].concat());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn no_show_starts_past_the_highest_folder_number() {
        let root = temp_root("last-number");
        let highest = root.join(u64::MAX.to_string());
        fs::create_dir_all(&highest).unwrap();
        let refused = Session::create(&root, &Quiz::new("T", vec![]), &meta(1));
        assert!(matches!(refused, Err(QuizFileError::Io { action: FileAction::CreateFolder, path, .. }) if path == highest));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_show_that_cannot_start_says_which_file_and_why() {
        let root = temp_root("not-a-folder");
        fs::write(&root, "a file where the shows go").unwrap();
        let refused = Session::create(&root, &Quiz::new("T", vec![]), &meta(1));
        assert!(matches!(refused, Err(QuizFileError::Io { action: FileAction::CreateFolder, path, .. }) if path == root));
        fs::remove_file(&root).unwrap();
    }

    #[test]
    fn a_show_whose_start_stopped_before_its_quiz_leaves_the_last_show_to_resume() {
        let root = temp_root("start-stopped");
        journaled_show(&root, 70);
        // Stopped after the meta, which is written first.
        let stopped = root.join("80");
        fs::create_dir(&stopped).unwrap();
        fs::write(stopped.join(META_FILE), serde_json::to_string(&meta(80)).unwrap()).unwrap();
        let summary = Session::latest_summary(&root).unwrap().unwrap();
        assert_eq!((summary.meta.started_ms, summary.command_count), (70, 2));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_whole_line_that_does_not_read_refuses_the_resume_and_changes_nothing() {
        let root = temp_root("damaged");
        let dir = journaled_show(&root, 1);
        let journal = dir.join(JOURNAL_FILE);
        let text = fs::read_to_string(&journal).unwrap();
        let (first, second) = text.split_once('\n').unwrap();
        // A command from a newer app between two this app knows.
        let damaged = format!("{first}\n{{\"atMs\":6,\"command\":{{\"type\":\"newerCommand\"}}}}\n{second}");
        fs::write(&journal, &damaged).unwrap();

        assert!(matches!(Session::latest_summary(&root), Err(ResumeError::Damaged { line: 2 })));
        assert!(matches!(Session::resume_latest(&root), Err(ResumeError::Damaged { line: 2 })));
        assert_eq!(fs::read_to_string(&journal).unwrap(), damaged);
        assert!(!dir.join(TORN_DIR).exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_latest_show_resumes_also_after_the_clock_went_back() {
        let root = temp_root("clock");
        journaled_show(&root, 50);
        let mut later = Session::create(&root, &Quiz::new("T", vec![]), &meta(20)).unwrap();
        later.append(&Stamped { at_ms: 7, command: Command::Next }).unwrap();
        let summary = Session::latest_summary(&root).unwrap().unwrap();
        assert_eq!((summary.meta.started_ms, summary.command_count), (20, 1));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn two_shows_started_in_one_millisecond_get_a_folder_each() {
        let root = temp_root("same-ms");
        journaled_show(&root, 60);
        Session::create(&root, &Quiz::new("T", vec![]), &meta(60)).unwrap();
        let mut folders: Vec<u64> = session_dirs(&root).unwrap().into_iter().map(|(number, _)| number).collect();
        folders.sort();
        assert_eq!(folders, [60, 61]);
        assert_eq!(Session::latest_summary(&root).unwrap().unwrap().command_count, 0);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn resumes_the_latest_session_and_drops_a_torn_line() {
        let root = temp_root("resume");
        let quiz = Quiz::new("T", vec![]);
        for started_ms in [1, 2] {
            let meta = SessionMeta {
                quiz_title: format!("show {started_ms}"),
                source_path: None,
                started_ms,
                media_root: Some(format!("/media/{started_ms}")),
                rules_version: quiz_core::RULES_VERSION,
            };
            let mut session = Session::create(&root, &quiz, &meta).unwrap();
            session.append(&Stamped { at_ms: 5, command: Command::Next }).unwrap();
            session
                .append(&Stamped { at_ms: 6, command: Command::Press { buzzer: 1, button: Button::A } })
                .unwrap();
        }
        let mut torn = OpenOptions::new().append(true).open(root.join("2").join(JOURNAL_FILE)).unwrap();
        torn.write_all(b"{\"atMs\": 7, \"comm").unwrap();

        let summary = Session::latest_summary(&root).unwrap().unwrap();
        assert_eq!((summary.meta.quiz_title.as_str(), summary.command_count), ("show 2", 2));
        let mut resumed = Session::resume_latest(&root).unwrap().unwrap();
        assert_eq!(resumed.journal.len(), 2);
        assert_eq!(resumed.meta.media_root.as_deref(), Some("/media/2"));
        resumed.session.append(&Stamped { at_ms: 8, command: Command::Next }).unwrap();
        assert_eq!(Session::latest_summary(&root).unwrap().unwrap().command_count, 3);
        fs::remove_dir_all(&root).unwrap();
    }

    /// A journal in memory that fails its next write after `fail_after` bytes,
    /// or its next truncate.
    #[derive(Default)]
    struct FlakyJournal {
        bytes: Vec<u8>,
        fail_after: Option<usize>,
        fail_truncate: bool,
    }

    impl JournalFile for FlakyJournal {
        fn write_at(&mut self, at: u64, line: &[u8]) -> io::Result<()> {
            let at = at as usize;
            let (written, result) = match self.fail_after.take() {
                Some(n) => (&line[..n.min(line.len())], Err(io::Error::other("disk full"))),
                None => (line, Ok(())),
            };
            let end = at + written.len();
            if self.bytes.len() < end {
                self.bytes.resize(end, 0);
            }
            self.bytes[at..end].copy_from_slice(written);
            result
        }

        fn sync(&mut self) -> io::Result<()> {
            Ok(())
        }

        fn truncate(&mut self, len: u64) -> io::Result<()> {
            if self.fail_truncate {
                return Err(io::Error::other("cannot truncate"));
            }
            self.bytes.truncate(len as usize);
            Ok(())
        }
    }

    #[test]
    fn a_failed_append_leaves_no_partial_line() {
        let mut journal = FlakyJournal::default();
        let mut len = 0;
        append_line(&mut journal, &mut len, b"one\n").unwrap();
        journal.fail_after = Some(2);
        assert!(matches!(append_line(&mut journal, &mut len, b"two\n"), Err(AppendError::Failed(_))));
        append_line(&mut journal, &mut len, b"three\n").unwrap();
        assert_eq!(journal.bytes, b"one\nthree\n");
        assert_eq!(len, 10);
    }

    #[test]
    fn a_line_after_a_failed_write_lands_right_after_the_whole_lines() {
        let mut journal = FlakyJournal::default();
        let mut len = 0;
        append_line(&mut journal, &mut len, b"one\n").unwrap();
        // The failed write left bytes past the whole lines that were not taken back yet.
        journal.bytes.extend_from_slice(b"tw");
        append_line(&mut journal, &mut len, b"three\n").unwrap();
        assert_eq!(journal.bytes, b"one\nthree\n");
    }

    #[test]
    fn the_journal_file_can_be_cut_back_and_written_after() {
        let root = temp_root("truncate");
        fs::create_dir_all(&root).unwrap();
        let path = root.join(JOURNAL_FILE);
        let mut file = open_journal(&path).unwrap();
        let mut len = 0;
        append_line(&mut file, &mut len, b"one\n").unwrap();
        file.write_all(b"torn").unwrap();
        // What a failed write is taken back with, and what resume does to a torn line.
        file.truncate(len).unwrap();
        append_line(&mut file, &mut len, b"two\n").unwrap();
        drop(file);
        assert_eq!(fs::read(&path).unwrap(), b"one\ntwo\n");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn only_a_show_that_may_replay_otherwise_says_its_rules_changed() {
        let old: SessionMeta = serde_json::from_str(r#"{"quizTitle":"T","sourcePath":null,"startedMs":1,"mediaRoot":null}"#).unwrap();
        assert_eq!(old.rules_version, 0, "a show from before the rules version was kept");
        assert!(old.rules_changed());
        let under = |rules_version| SessionMeta { rules_version, ..old.clone() };
        assert!(under(quiz_core::REPLAYS_AS_PLAYED_FROM - 1).rules_changed());
        // Every change since is kept to the shows begun under it.
        for rules_version in quiz_core::REPLAYS_AS_PLAYED_FROM..=quiz_core::RULES_VERSION {
            assert!(!under(rules_version).rules_changed(), "{rules_version}");
        }
        assert!(under(quiz_core::RULES_VERSION + 1).rules_changed(), "a newer app's");
    }

    #[test]
    fn an_append_that_cannot_be_taken_back_says_so() {
        let mut journal = FlakyJournal { fail_after: Some(2), fail_truncate: true, ..Default::default() };
        let mut len = 0;
        assert!(matches!(append_line(&mut journal, &mut len, b"one\n"), Err(AppendError::Unrepaired(_))));
    }
}
