//! Crash-safe show sessions. Each show gets a folder with a copy of the quiz and
//! an append-only journal of accepted commands, synced to disk on every write.
//! Replaying the journal on the quiz copy rebuilds the show exactly.
//!
//! The journal is opened for plain writing, not in append mode: on Windows an
//! append-mode handle lacks the write-data access that truncating needs
//! (Rust std, `library/std/src/sys/fs/windows.rs`, `get_access_mode`), and
//! truncating is how a torn line is cut off on resume and a failed write is
//! taken back. Every line is written at the end of the whole lines instead.

use quiz_core::{Quiz, QuizJsonError, Stamped};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const QUIZ_FILE: &str = "quiz.json";
const META_FILE: &str = "meta.json";
const JOURNAL_FILE: &str = "journal.jsonl";
/// The journal as it was before resuming cut a torn last line off it.
const TORN_JOURNAL_FILE: &str = "journal.jsonl.bak";

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
    /// Whether replaying the show's journal may give another show than the one it recorded.
    pub fn rules_changed(&self) -> bool {
        self.rules_version != quiz_core::RULES_VERSION
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
    /// Line `line` (from 1) of the journal is whole but does not read: a
    /// newer app wrote it, or the disk damaged it. Replaying without it
    /// would rebuild another show, so the show is not resumed and its
    /// journal is left as it is.
    #[error("line {line} of the show's journal does not read")]
    Damaged { line: usize },
}

/// Why a command was not journaled, so it must be rejected.
#[derive(Debug, thiserror::Error)]
pub enum NotSaved {
    #[error(transparent)]
    Io(#[from] io::Error),
    /// An earlier write failed and could not be taken back, so nothing more
    /// is appended until the show is resumed, which cuts the torn line off.
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
    pub fn create(root: &Path, quiz: &Quiz, meta: &SessionMeta) -> io::Result<Session> {
        let dir = new_dir(root, meta.started_ms)?;
        // A power cut must not lose the new folder (or a sessions folder made
        // just now) or the journal's entry in it, only their unsynced data.
        for created in root.parent().into_iter().chain([root]) {
            sync_dir(created)?;
        }
        write_atomic(&dir.join(QUIZ_FILE), quiz)?;
        write_atomic(&dir.join(META_FILE), meta)?;
        let journal = open_journal(&dir.join(JOURNAL_FILE))?;
        sync_dir(&dir)?;
        let len = journal.metadata()?.len();
        Ok(Session { journal, len, broken: false })
    }

    /// Reopens the most recent session for appending, with its quiz and journal.
    pub fn resume_latest(root: &Path) -> Result<Option<Resumed>, ResumeError> {
        let Some(dir) = latest_dir(root)? else { return Ok(None) };
        let quiz = Quiz::from_json(&fs::read_to_string(dir.join(QUIZ_FILE))?)?;
        let meta = read_meta(&dir)?;
        let path = dir.join(JOURNAL_FILE);
        let (journal, whole_len) = read_journal(&path)?;
        // A torn last line, which no window ever saw, is cut off so new
        // commands start on a clean line; the journal as it was is kept first,
        // before the journal is opened for writing.
        let torn = fs::metadata(&path).is_ok_and(|journal| journal.len() > whole_len);
        if torn {
            fs::copy(&path, dir.join(TORN_JOURNAL_FILE))?;
        }
        let file = open_journal(&path)?;
        if torn {
            file.set_len(whole_len)?;
            file.sync_data()?;
        }
        let session = Session { journal: file, len: whole_len, broken: false };
        Ok(Some(Resumed { session, quiz, journal, meta }))
    }

    pub fn latest_summary(root: &Path) -> Result<Option<SessionSummary>, ResumeError> {
        let Some(dir) = latest_dir(root)? else { return Ok(None) };
        let meta = read_meta(&dir)?;
        let command_count = read_journal(&dir.join(JOURNAL_FILE))?.0.len();
        Ok(Some(SessionSummary { meta, command_count }))
    }

    /// Makes every later append fail, as after a write that could not be taken back.
    #[cfg(test)]
    pub fn repair_for_test(&mut self) {
        self.broken = false;
    }

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

fn write_atomic<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    quiz_file::write_atomic(path, json.as_bytes()).map_err(io::Error::other)
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
fn new_dir(root: &Path, started_ms: u64) -> io::Result<PathBuf> {
    fs::create_dir_all(root)?;
    let last = session_dirs(root)?.into_iter().map(|(number, _)| number).max();
    let mut number = last.map_or(started_ms, |last| started_ms.max(last + 1));
    loop {
        let dir = root.join(number.to_string());
        match fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => number += 1,
            Err(e) => return Err(e),
        }
    }
}

/// The latest show's folder, if any has its quiz.
fn latest_dir(root: &Path) -> io::Result<Option<PathBuf>> {
    let mut dirs = session_dirs(root)?;
    dirs.retain(|(_, path)| path.join(QUIZ_FILE).exists());
    Ok(dirs.into_iter().max_by_key(|(number, _)| *number).map(|(_, path)| path))
}

/// Reads the journal: its commands, and the length of its whole lines. A
/// last line without its newline was being written when the app stopped,
/// so it was never accepted and is left out; a whole line that does not
/// read refuses the journal (`ResumeError::Damaged`).
fn read_journal(path: &Path) -> Result<(Vec<Stamped>, u64), ResumeError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
        Err(e) => return Err(e.into()),
    };
    let whole = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |last| last + 1);
    let mut commands = Vec::new();
    for (index, line) in bytes[..whole].split_inclusive(|&b| b == b'\n').enumerate() {
        let line = line.trim_ascii();
        if !line.is_empty() {
            commands.push(serde_json::from_slice(line).map_err(|_| ResumeError::Damaged { line: index + 1 })?);
        }
    }
    Ok((commands, whole as u64))
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
        assert_eq!(fs::read(dir.join(TORN_JOURNAL_FILE)).unwrap(), torn, "the journal as it was is kept");
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
        assert!(!dir.join(TORN_JOURNAL_FILE).exists());
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
    fn a_show_from_before_the_rules_version_was_kept_says_its_rules_changed() {
        let old: SessionMeta = serde_json::from_str(r#"{"quizTitle":"T","sourcePath":null,"startedMs":1,"mediaRoot":null}"#).unwrap();
        assert_eq!(old.rules_version, 0);
        assert!(old.rules_changed());
        let now = SessionMeta { rules_version: quiz_core::RULES_VERSION, ..old };
        assert!(!now.rules_changed());
    }

    #[test]
    fn an_append_that_cannot_be_taken_back_says_so() {
        let mut journal = FlakyJournal { fail_after: Some(2), fail_truncate: true, ..Default::default() };
        let mut len = 0;
        assert!(matches!(append_line(&mut journal, &mut len, b"one\n"), Err(AppendError::Unrepaired(_))));
    }
}
