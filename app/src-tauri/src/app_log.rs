//! The app's log file, `buzzdome.log` in the app's log folder. A release
//! build has no console on Windows and ends at once on a panic
//! (`panic = "abort"`), so what went wrong would leave no trace otherwise.
//! Every line carries the show clock's time, as the journal's commands do
//! (`Stamped::at_ms`), so a crash can be placed in the show.

use crate::now_ms;
use std::backtrace::Backtrace;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

const LOG_FILE: &str = "buzzdome.log";
/// What a full log is renamed to, replacing the one before.
const OLD_LOG_FILE: &str = "buzzdome.old.log";
/// A log this big starts over (`OLD_LOG_FILE`), so it stays small.
const MAX_LOG_BYTES: u64 = 512 * 1024;

/// The log folder, once known; one line is written at a time.
static LOG_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Writes the log in `dir` from now on, panics included; the first line
/// names the app's `version`.
pub fn start(dir: PathBuf, version: &str) {
    *LOG_DIR.lock().unwrap_or_else(PoisonError::into_inner) = Some(dir);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        note(&format!("panic on thread {}: {info}\n{}", thread.name().unwrap_or("unnamed"), Backtrace::force_capture()));
        previous(info);
    }));
    note(&format!("Buzzdome {version} started"));
}

/// Writes `message` to the log, and to the console where there is one.
pub fn note(message: &str) {
    // Not `eprintln!`, which panics when the console is gone.
    let _ = writeln!(std::io::stderr(), "{message}");
    let line = format!("[{}] {message}\n", now_ms());
    if let Some(dir) = LOG_DIR.lock().unwrap_or_else(PoisonError::into_inner).as_deref() {
        // Nowhere to say that the log cannot be written.
        let _ = append(dir, &line);
    }
}

fn append(dir: &Path, line: &str) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(LOG_FILE);
    if fs::metadata(&path).is_ok_and(|m| m.len() >= MAX_LOG_BYTES) {
        fs::rename(&path, dir.join(OLD_LOG_FILE))?;
    }
    OpenOptions::new().create(true).append(true).open(path)?.write_all(line.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_log_starts_over_and_keeps_the_one_before() {
        let dir = std::env::temp_dir().join(format!("buzzdome-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        append(&dir, "first\n").unwrap();
        append(&dir, "second\n").unwrap();
        assert_eq!(fs::read_to_string(dir.join(LOG_FILE)).unwrap(), "first\nsecond\n");
        fs::write(dir.join(LOG_FILE), vec![b'x'; MAX_LOG_BYTES as usize]).unwrap();
        append(&dir, "third\n").unwrap();
        assert_eq!(fs::read_to_string(dir.join(LOG_FILE)).unwrap(), "third\n");
        assert_eq!(fs::metadata(dir.join(OLD_LOG_FILE)).unwrap().len(), MAX_LOG_BYTES);
        fs::remove_dir_all(&dir).unwrap();
    }
}
