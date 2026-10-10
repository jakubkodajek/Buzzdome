//! Spreadsheets are read in a process of their own: the app started again
//! as the sheet reader (`--read-sheet <file>`). A workbook goes through a
//! large third-party parser that reads some formats whole, so a damaged or
//! crafted file could use up memory or make it panic; in the reader that
//! ends only the reader, never a running show. The reader writes what it
//! read to its standard output as JSON (`quiz_file::SheetRead`).

use crate::error::{AppError, CmdResult};
use quiz_core::Table;
use quiz_file::{FileAction, SheetRead};
use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// The argument that starts the app as the sheet reader.
const READ_SHEET: &str = "--read-sheet";
/// How long the reader may take; a sheet of questions takes well under a second.
const TIME_LIMIT: Duration = Duration::from_secs(60);
/// The most the reader may report: the tables of the largest sheet it reads, as JSON.
const MAX_REPORT_BYTES: u64 = 4 * quiz_file::MAX_SHEET_BYTES;

/// Called first in `main`: when the app was started as the sheet reader,
/// reads the file, reports what it read and ends the process.
pub fn serve_if_asked() {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(OsStr::new(READ_SHEET)) {
        return;
    }
    let reported = args.next().is_some_and(|path| serde_json::to_writer(std::io::stdout().lock(), &SheetRead::of(Path::new(&path))).is_ok());
    std::process::exit(if reported { 0 } else { 1 });
}

/// Reads the spreadsheet at `path` in the sheet reader (see `quiz_file::read_tables`).
pub fn read_tables(path: &Path) -> CmdResult<Vec<Table>> {
    let app = std::env::current_exe().map_err(|_| AppError::SpreadsheetReaderFailed)?;
    read_tables_with(&app, path, TIME_LIMIT)
}

/// Reads with `reader`, a program that serves `--read-sheet`, stopped
/// after `time_limit`. Public for the integration test, which runs the app.
pub fn read_tables_with(reader: &Path, path: &Path, time_limit: Duration) -> CmdResult<Vec<Table>> {
    // A file that cannot be opened is reported as such, not as a failed reader.
    std::fs::File::open(path).map_err(|e| AppError::file(FileAction::Read, path, e))?;
    let mut child = Command::new(reader)
        .arg(READ_SHEET)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| AppError::SpreadsheetReaderFailed)?;
    let output = child.stdout.take().expect("the reader's output is piped");
    // Taken while it is written, so a full pipe never holds the reader up.
    let report = thread::spawn(move || {
        let mut bytes = Vec::new();
        output.take(MAX_REPORT_BYTES).read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = Instant::now() + time_limit;
    let finished = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };
    match report.join() {
        Ok(Ok(bytes)) if finished => {
            let read: SheetRead = serde_json::from_slice(&bytes).map_err(|_| AppError::SpreadsheetReaderFailed)?;
            Ok(read.into_result()?)
        }
        _ => Err(AppError::SpreadsheetReaderFailed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reader_that_fails_or_overruns_is_reported_and_a_missing_file_as_such() {
        let dir = std::env::temp_dir().join(format!("quiz-sheet-reader-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("quiz.csv");
        std::fs::write(&csv, "Q\nOne?\n").unwrap();
        // The test program does not serve `--read-sheet`: it refuses the argument.
        let not_a_reader = std::env::current_exe().unwrap();
        assert_eq!(read_tables_with(&not_a_reader, &csv, TIME_LIMIT), Err(AppError::SpreadsheetReaderFailed));
        assert_eq!(read_tables_with(&not_a_reader, &csv, Duration::ZERO), Err(AppError::SpreadsheetReaderFailed));
        assert_eq!(read_tables_with(&dir.join("no-such-program"), &csv, TIME_LIMIT), Err(AppError::SpreadsheetReaderFailed));
        assert!(matches!(read_tables_with(&not_a_reader, &dir.join("gone.csv"), TIME_LIMIT), Err(AppError::File { .. })));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
