//! Errors of the app's commands. Each has a code and parameters, never an
//! English sentence: the UI words them in the quizmaster's language
//! (`errorText` in `app/src/i18n.ts`). `detail` fields carry the system's own
//! message, which the UI shows as it is.

use crate::session::{NotSaved, ResumeError};
use buzzers::learned::LearnError;
use quiz_core::{BoardError, BuzzerId, Button, GameError, QuizJsonError, SheetError, ValidationError, MAX_TEAMS};
use quiz_file::{FileAction, QuizFileError};
use serde::Serialize;
use std::path::Path;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[ts(export)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum AppError {
    // The show.
    #[error("no show is running")]
    NoShow,
    #[error("a show is running; end it first")]
    ShowRunning,
    #[error("there is no show to resume")]
    NoShowToResume,
    #[error("this command does not apply in the current step")]
    NotNow,
    #[error("unknown buzzer {buzzer}")]
    UnknownBuzzer { buzzer: BuzzerId },
    #[error("no slide {slide}")]
    NoSuchSlide { slide: usize },
    #[error("buzzer {buzzer} already exists")]
    DuplicateBuzzer { buzzer: BuzzerId },
    #[error("more than {max} team names")]
    TooManyTeams { max: usize },
    /// The show's journal cannot be written, so the change was not made.
    #[error("the show cannot be saved: {detail}")]
    ShowNotSaved { detail: String },
    /// An earlier write to the show's journal failed and could not be taken
    /// back, so nothing more is saved until the show is resumed.
    #[error("the show's journal could not be repaired after a failed write")]
    JournalBroken,
    #[error("the last show cannot be read: {detail}")]
    ShowNotRead { detail: String },
    /// Line `line` of the last show's journal does not read, so the show is
    /// not resumed and its journal is left as it is.
    #[error("line {line} of the last show's journal does not read")]
    JournalDamaged { line: usize },
    /// A file the app keeps (`app_file`) could not be read when the app
    /// started, so it is not written over until the app reads it again.
    #[error("{path} could not be read, so it is not written over")]
    FileLeftUnread { path: String },
    /// A resumed show began under other rules (`RULES_VERSION`); it runs.
    #[error("the show began in another version of the app; its scores may differ")]
    RulesChanged,

    // Quiz files and the editor.
    #[error("cannot {action:?} {path}: {detail}")]
    File { action: FileAction, path: String, detail: String },
    #[error("the file is not a quiz file: {detail}")]
    NotAQuizFile { detail: String },
    #[error("the quiz file has no quiz.json")]
    MissingQuizJson,
    #[error("quiz.json cannot be read: {detail}")]
    BadQuizJson { detail: String },
    #[error("the quiz file uses schema version {found}, this app supports up to {supported}")]
    NewerSchema { found: u32, supported: u32 },
    #[error("the quiz file unpacks to more than {max_mb} MB")]
    #[serde(rename_all = "camelCase")]
    TooLarge { max_mb: u64 },
    #[error("the quiz file has more than {max} files")]
    TooManyFiles { max: usize },
    #[error("the quiz has problems: {problems:?}")]
    QuizInvalid { problems: Vec<ValidationError> },
    #[error("no quiz is open in the editor")]
    NoDraft,
    #[error("the spreadsheet cannot be read: {detail}")]
    NotASpreadsheet { detail: String },
    #[error("the spreadsheet is larger than {max_mb} MB, holds more than {max_text_mb} MB of text or has more than {max_rows} rows or {max_columns} columns")]
    #[serde(rename_all = "camelCase")]
    SpreadsheetTooLarge { max_mb: u64, max_text_mb: usize, max_rows: usize, max_columns: usize },
    /// The sheet reader process (`sheet_reader.rs`) needed more memory than
    /// it may have: a file small enough whose cells the parser spreads out.
    #[error("reading the spreadsheet needs more than {max_mb} MB of memory")]
    #[serde(rename_all = "camelCase")]
    SpreadsheetTooComplex { max_mb: u64 },
    /// The sheet reader process (`sheet_reader.rs`) stopped, overran or could not start.
    #[error("the spreadsheet reader failed")]
    SpreadsheetReaderFailed,
    #[error("no sheet has a Q column")]
    NoQuestionColumn,

    // The media player.
    #[error("{name} is not a picture, sound or video")]
    NotAMediaFile { name: String },
    #[error("{reference} is not in the media library")]
    NotInLibrary { reference: String },
    #[error("{name} is too large to drop (over {mb} MB)")]
    DroppedFileTooLarge { name: String, mb: u64 },
    #[error("there is no pad {pad} on the sound board")]
    NoSuchPad { pad: usize },

    // The buzzer receiver.
    #[error("USB-HID is not available")]
    UsbUnavailable,
    #[error("USB devices cannot be listed: {detail}")]
    UsbListFailed { detail: String },
    #[error("the receiver is not connected")]
    ReceiverNotConnected,
    #[error("learning was not started")]
    LearnNotStarted,
    #[error("press the button at least {min} times")]
    LearnTooFewPresses { min: usize },
    #[error("the recorded presses differ in length")]
    LearnInconsistent,
    #[error("these presses look the same as buzzer {buzzer} button {button:?}")]
    LearnConflict { buzzer: BuzzerId, button: Button },

    // Windows and the operating system.
    #[error("monitor {index} not found")]
    NoSuchMonitor { index: usize },
    /// The system gave the app no folder for its data (team names, sessions, the media library).
    #[error("the app has no data folder")]
    NoDataFolder,
    /// Something the app cannot do anything about (a window or a system
    /// folder failed); `detail` is the system's message.
    #[error("{detail}")]
    System { detail: String },
}

pub type CmdResult<T> = Result<T, AppError>;

impl AppError {
    /// A file operation that failed with `error`.
    pub fn file(action: FileAction, path: &Path, error: impl ToString) -> AppError {
        AppError::File { action, path: path.display().to_string(), detail: error.to_string() }
    }

    pub fn system(error: impl ToString) -> AppError {
        AppError::System { detail: error.to_string() }
    }
}

impl From<GameError> for AppError {
    fn from(e: GameError) -> AppError {
        match e {
            GameError::NotNow => AppError::NotNow,
            GameError::UnknownBuzzer(buzzer) => AppError::UnknownBuzzer { buzzer },
            GameError::NoSuchSlide(slide) => AppError::NoSuchSlide { slide },
            GameError::DuplicateBuzzer(buzzer) => AppError::DuplicateBuzzer { buzzer },
            GameError::TooManyTeams => AppError::TooManyTeams { max: MAX_TEAMS },
        }
    }
}

impl From<BoardError> for AppError {
    fn from(e: BoardError) -> AppError {
        match e {
            BoardError::NoSuchPad(pad) => AppError::NoSuchPad { pad },
            BoardError::NotInLibrary(reference) => AppError::NotInLibrary { reference },
        }
    }
}

impl From<QuizJsonError> for AppError {
    fn from(e: QuizJsonError) -> AppError {
        match e {
            QuizJsonError::NewerSchema { found, supported } => AppError::NewerSchema { found, supported },
            QuizJsonError::Syntax(detail) => AppError::BadQuizJson { detail },
        }
    }
}

impl From<NotSaved> for AppError {
    fn from(e: NotSaved) -> AppError {
        match e {
            NotSaved::Io(e) => AppError::ShowNotSaved { detail: e.to_string() },
            NotSaved::Broken => AppError::JournalBroken,
        }
    }
}

impl From<ResumeError> for AppError {
    fn from(e: ResumeError) -> AppError {
        match e {
            ResumeError::Io(e) => AppError::ShowNotRead { detail: e.to_string() },
            ResumeError::Meta(e) => AppError::ShowNotRead { detail: e.to_string() },
            ResumeError::Quiz(e) => e.into(),
            ResumeError::Damaged { line } => AppError::JournalDamaged { line },
        }
    }
}

impl From<QuizFileError> for AppError {
    fn from(e: QuizFileError) -> AppError {
        match e {
            QuizFileError::Io { action, path, source } => AppError::file(action, &path, source),
            QuizFileError::Zip(e) => AppError::NotAQuizFile { detail: e.to_string() },
            QuizFileError::Json(e) => e.into(),
            QuizFileError::MissingQuizJson => AppError::MissingQuizJson,
            QuizFileError::TooLarge => AppError::TooLarge { max_mb: quiz_file::MAX_UNPACKED_BYTES >> 20 },
            QuizFileError::TooManyFiles => AppError::TooManyFiles { max: quiz_file::MAX_ENTRIES },
            QuizFileError::Invalid(problems) => AppError::QuizInvalid { problems },
            QuizFileError::Spreadsheet(detail) => AppError::NotASpreadsheet { detail },
            QuizFileError::SpreadsheetTooLarge => {
                AppError::SpreadsheetTooLarge {
                    max_mb: quiz_file::MAX_SHEET_BYTES >> 20,
                    max_text_mb: quiz_file::MAX_SHEET_TEXT_BYTES >> 20,
                    max_rows: quiz_file::MAX_SHEET_ROWS,
                    max_columns: quiz_file::MAX_SHEET_COLUMNS,
                }
            }
            QuizFileError::Sheet(SheetError::NoQuestionColumn) => AppError::NoQuestionColumn,
        }
    }
}

impl From<LearnError> for AppError {
    fn from(e: LearnError) -> AppError {
        match e {
            LearnError::TooFewPresses => AppError::LearnTooFewPresses { min: buzzers::learned::MIN_SAMPLES },
            LearnError::Inconsistent => AppError::LearnInconsistent,
            LearnError::Conflict { buzzer, button } => AppError::LearnConflict { buzzer, button },
        }
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> AppError {
        AppError::system(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_reach_the_ui_as_codes_with_parameters() {
        let json = |e: AppError| serde_json::to_value(e).unwrap();
        assert_eq!(json(AppError::NotNow), serde_json::json!({ "code": "notNow" }));
        assert_eq!(json(AppError::TooLarge { max_mb: 4096 }), serde_json::json!({ "code": "tooLarge", "maxMb": 4096 }));
        let file = AppError::file(FileAction::CreateFolder, Path::new("x"), "denied");
        assert_eq!(
            json(file),
            serde_json::json!({ "code": "file", "action": "createFolder", "path": "x", "detail": "denied" })
        );
    }
}
