//! Crash-safe show sessions. Each show gets a folder with a copy of the quiz and
//! an append-only journal of accepted commands, synced to disk on every write.
//! Replaying the journal on the quiz copy rebuilds the show exactly.

use quiz_core::{Quiz, Stamped};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const QUIZ_FILE: &str = "quiz.json";
const META_FILE: &str = "meta.json";
const JOURNAL_FILE: &str = "journal.jsonl";

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
    pub fn create(root: &Path, quiz: &Quiz, meta: &SessionMeta) -> std::io::Result<Session> {
        let dir = root.join(meta.started_ms.to_string());
        fs::create_dir_all(&dir)?;
        write_synced(&dir.join(QUIZ_FILE), &to_json(quiz)?)?;
        write_synced(&dir.join(META_FILE), &to_json(meta)?)?;
        let journal = OpenOptions::new().create(true).append(true).open(dir.join(JOURNAL_FILE))?;
        Ok(Session { journal })
    }

    /// Reopens the most recent session for appending, with its quiz and journal.
    pub fn resume_latest(root: &Path) -> std::io::Result<Option<Resumed>> {
        let Some(dir) = latest_dir(root)? else { return Ok(None) };
        let quiz = Quiz::from_json(&fs::read_to_string(dir.join(QUIZ_FILE))?)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let meta: SessionMeta = serde_json::from_str(&fs::read_to_string(dir.join(META_FILE))?)?;
        let (journal, valid_len) = read_journal(&dir.join(JOURNAL_FILE))?;
        let file = OpenOptions::new().create(true).append(true).open(dir.join(JOURNAL_FILE))?;
        // Cut off a torn last line so new commands start on a clean line.
        file.set_len(valid_len)?;
        Ok(Some(Resumed { session: Session { journal: file }, quiz, journal, meta }))
    }

    pub fn latest_summary(root: &Path) -> std::io::Result<Option<SessionSummary>> {
        let Some(dir) = latest_dir(root)? else { return Ok(None) };
        let meta: SessionMeta = serde_json::from_str(&fs::read_to_string(dir.join(META_FILE))?)?;
        let command_count = read_journal(&dir.join(JOURNAL_FILE))?.0.len();
        Ok(Some(SessionSummary { meta, command_count }))
    }

    /// Appends one accepted command and waits until it is on disk.
    pub fn append(&mut self, stamped: &Stamped) -> std::io::Result<()> {
        let mut line = serde_json::to_string(stamped)?;
        line.push('\n');
        self.journal.write_all(line.as_bytes())?;
        self.journal.sync_data()
    }
}

fn to_json<T: Serialize>(value: &T) -> std::io::Result<String> {
    Ok(serde_json::to_string_pretty(value)?)
}

fn write_synced(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()
}

/// Session folders are named by start time in milliseconds.
fn latest_dir(root: &Path) -> std::io::Result<Option<PathBuf>> {
    if !root.exists() {
        return Ok(None);
    }
    let mut latest: Option<(u64, PathBuf)> = None;
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        let Some(started) = path.file_name().and_then(|n| n.to_str()).and_then(|n| n.parse::<u64>().ok()) else {
            continue;
        };
        if path.join(QUIZ_FILE).exists() && latest.as_ref().is_none_or(|(t, _)| started > *t) {
            latest = Some((started, path));
        }
    }
    Ok(latest.map(|(_, path)| path))
}

/// Reads the journal and returns the commands plus the byte length of the
/// valid part. A torn last line (crash during a write) is dropped.
fn read_journal(path: &Path) -> std::io::Result<(Vec<Stamped>, u64)> {
    if !path.exists() {
        return Ok((Vec::new(), 0));
    }
    let text = fs::read_to_string(path)?;
    let mut commands = Vec::new();
    let mut valid_len = 0;
    for line in text.split_inclusive('\n') {
        if !line.ends_with('\n') {
            break;
        }
        if !line.trim().is_empty() {
            match serde_json::from_str(line.trim()) {
                Ok(stamped) => commands.push(stamped),
                Err(_) => break,
            }
        }
        valid_len += line.len() as u64;
    }
    Ok((commands, valid_len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiz_core::{Button, Command};

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("quiz-show-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn resumes_the_latest_session_and_drops_a_torn_line() {
        let root = temp_root("resume");
        let quiz = Quiz { schema_version: 1, title: "T".into(), slides: vec![] };
        for started_ms in [1, 2] {
            let meta = SessionMeta {
                quiz_title: format!("show {started_ms}"),
                source_path: None,
                started_ms,
                media_root: Some(format!("/media/{started_ms}")),
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
}
