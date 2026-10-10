//! The app as its own sheet reader: started with `--read-sheet`, it reads
//! the file and reports the tables without opening a window.

use quiz_show_lib::sheet_reader::read_tables_with;
use std::path::Path;
use std::time::Duration;

#[test]
fn the_app_reads_a_spreadsheet_in_a_process_of_its_own() {
    let app = Path::new(env!("CARGO_BIN_EXE_quiz-show"));
    let dir = std::env::temp_dir().join(format!("quiz-sheet-reader-app-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let csv = dir.join("quiz.csv");
    std::fs::write(&csv, "Q;A1;A2\nCapital?;[Prague];Brno\n").unwrap();
    let tables = read_tables_with(app, &csv, Duration::from_secs(60)).unwrap();
    assert_eq!(tables[0].rows, [vec!["Q", "A1", "A2"], vec!["Capital?", "[Prague]", "Brno"]]);

    // What the reader refuses comes back as it would in the app's own process.
    std::fs::write(&csv, format!("Q{}\n", ";x".repeat(quiz_file::MAX_SHEET_COLUMNS))).unwrap();
    let refused = read_tables_with(app, &csv, Duration::from_secs(60)).unwrap_err();
    assert!(matches!(refused, quiz_show_lib::AppError::SpreadsheetTooLarge { .. }), "{refused:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}
