//! The app as its own sheet reader: started with `--read-sheet`, it reads
//! the file and reports the tables without opening a window.

use buzzdome_lib::sheet_reader::{read_tables_with, Limits, LIMITS};
use buzzdome_lib::AppError;
use std::path::Path;

#[test]
fn the_app_reads_a_spreadsheet_in_a_process_of_its_own() {
    let app = Path::new(env!("CARGO_BIN_EXE_buzzdome"));
    let dir = std::env::temp_dir().join(format!("quiz-sheet-reader-app-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let csv = dir.join("quiz.csv");
    std::fs::write(&csv, "Q;A1;A2\nCapital?;[Prague];Brno\n").unwrap();
    let tables = read_tables_with(app, &csv, LIMITS).unwrap();
    assert_eq!(tables[0].rows, [vec!["Q", "A1", "A2"], vec!["Capital?", "[Prague]", "Brno"]]);

    // What the reader refuses comes back as it would in the app's own process.
    std::fs::write(&csv, format!("Q{}\n", ";x".repeat(quiz_file::MAX_SHEET_COLUMNS))).unwrap();
    let refused = read_tables_with(app, &csv, LIMITS).unwrap_err();
    assert!(matches!(refused, AppError::SpreadsheetTooLarge { .. }), "{refused:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_reader_holding_more_memory_than_allowed_is_stopped_and_the_sheet_refused() {
    let app = Path::new(env!("CARGO_BIN_EXE_buzzdome"));
    let dir = std::env::temp_dir().join(format!("quiz-sheet-reader-memory-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // A sheet the reader takes a moment over, so it is still running when checked.
    let csv = dir.join("quiz.csv");
    let rows: String = (1..quiz_file::MAX_SHEET_ROWS).map(|n| format!("Question {n}?;[Yes];No;Maybe\n")).collect();
    std::fs::write(&csv, format!("Q;A1;A2;A3\n{rows}")).unwrap();
    assert_eq!(read_tables_with(app, &csv, LIMITS).unwrap()[0].rows.len(), quiz_file::MAX_SHEET_ROWS);
    // Any process holds more than a byte.
    let refused = read_tables_with(app, &csv, Limits { memory: 1, ..LIMITS }).unwrap_err();
    assert_eq!(refused, AppError::SpreadsheetTooComplex { max_mb: 0 });
    std::fs::remove_dir_all(&dir).unwrap();
}
