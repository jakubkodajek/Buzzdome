//! Spreadsheets are read in a process of their own: the app started again
//! as the sheet reader (`--read-sheet <file>`). A workbook goes through a
//! large third-party parser that reads some formats whole, so a damaged or
//! crafted file could use up memory or make it panic; in the reader that
//! ends only the reader, never a running show, and the reader is stopped
//! once it takes too long or holds too much memory. The reader writes what
//! it read to its standard output as JSON (`quiz_file::SheetRead`).

use crate::error::{AppError, CmdResult};
use quiz_core::Table;
use quiz_file::{FileAction, QuizFileError, SheetRead};
use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// The argument that starts the app as the sheet reader.
const READ_SHEET: &str = "--read-sheet";
/// How often the reader's time and memory are checked.
const CHECK_EVERY: Duration = Duration::from_millis(10);

/// When the reader is stopped.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub time: Duration,
    /// Bytes of memory.
    pub memory: u64,
}

/// A sheet of questions takes well under a second and some megabytes. An
/// `.ods` file that repeats a row or cell, or an `.xls` file with cells far
/// apart, makes the parser take memory for every cell in between before
/// the reader can count them; past a gigabyte it is stopped and the sheet
/// refused as too large, before the show laptop runs short.
pub const LIMITS: Limits = Limits { time: Duration::from_secs(60), memory: 1 << 30 };

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
    read_tables_with(&app, path, LIMITS)
}

/// Reads with `reader`, a program that serves `--read-sheet`, stopped past
/// `limits`. Public for the integration test, which runs the app.
pub fn read_tables_with(reader: &Path, path: &Path, limits: Limits) -> CmdResult<Vec<Table>> {
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
        output.take(quiz_file::MAX_SHEET_REPORT_BYTES as u64).read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = Instant::now() + limits.time;
    let finished = loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break Ok(()),
            Ok(None) if memory_in_use(&child).is_some_and(|bytes| bytes > limits.memory) => break Err(QuizFileError::SpreadsheetTooLarge.into()),
            Ok(None) if Instant::now() < deadline => thread::sleep(CHECK_EVERY),
            _ => break Err(AppError::SpreadsheetReaderFailed),
        }
    };
    if finished.is_err() {
        // Ends the reader when it runs on, and takes its exit status either way.
        let _ = child.kill();
        let _ = child.wait();
    }
    let bytes = report.join().ok().and_then(Result::ok);
    finished?;
    let read: SheetRead = bytes.and_then(|bytes| serde_json::from_slice(&bytes).ok()).ok_or(AppError::SpreadsheetReaderFailed)?;
    Ok(read.into_result()?)
}

/// The memory `child` holds: its resident and swapped-out pages.
#[cfg(target_os = "linux")]
fn memory_in_use(child: &Child) -> Option<u64> {
    let status = std::fs::read_to_string(format!("/proc/{}/status", child.id())).ok()?;
    let kilobytes = |field: &str| -> Option<u64> { status.lines().find_map(|line| line.strip_prefix(field))?.trim().strip_suffix("kB")?.trim().parse().ok() };
    Some((kilobytes("VmRSS:")? + kilobytes("VmSwap:").unwrap_or(0)) * 1024)
}

/// The memory `child` holds: its physical footprint, compressed pages
/// included (what Activity Monitor shows).
#[cfg(target_os = "macos")]
fn memory_in_use(child: &Child) -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage_info_v2>::uninit();
    // SAFETY: proc_pid_rusage fills a rusage_info_v2 for the RUSAGE_INFO_V2
    // flavour, and it is read only when the call succeeds.
    unsafe {
        let read = libc::proc_pid_rusage(child.id() as libc::c_int, libc::RUSAGE_INFO_V2, usage.as_mut_ptr().cast());
        (read == 0).then(|| usage.assume_init().ri_phys_footprint)
    }
}

/// The memory `child` holds: its private committed memory, which Windows
/// counts against the system's commit limit as soon as it is allocated.
#[cfg(windows)]
fn memory_in_use(child: &Child) -> Option<u64> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX};
    let mut counters = PROCESS_MEMORY_COUNTERS_EX { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32, ..Default::default() };
    // SAFETY: the handle is the child's, open while `child` lives, and the
    // call writes at most `cb` bytes into `counters`.
    let read = unsafe { K32GetProcessMemoryInfo(child.as_raw_handle(), (&raw mut counters).cast::<PROCESS_MEMORY_COUNTERS>(), counters.cb) };
    (read != 0).then_some(counters.PrivateUsage as u64)
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
        assert_eq!(read_tables_with(&not_a_reader, &csv, LIMITS), Err(AppError::SpreadsheetReaderFailed));
        assert_eq!(read_tables_with(&not_a_reader, &csv, Limits { time: Duration::ZERO, ..LIMITS }), Err(AppError::SpreadsheetReaderFailed));
        assert_eq!(read_tables_with(&dir.join("no-such-program"), &csv, LIMITS), Err(AppError::SpreadsheetReaderFailed));
        assert!(matches!(read_tables_with(&not_a_reader, &dir.join("gone.csv"), LIMITS), Err(AppError::File { .. })));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
