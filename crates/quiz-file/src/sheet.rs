//! Questions from a spreadsheet and to one (spec QZ-8). An import reads an
//! Excel or OpenDocument workbook or a CSV file into tables, makes slides of
//! them (`quiz_core::import_tables`) and copies the media files their cells
//! name into the quiz. An export writes the slides as a CSV file and the
//! media next to it, under their own names.

use crate::{place_file, write_atomic, Context, FileAction, MediaSource, QuizFileError, Result};
use calamine::{open_workbook_auto, Data, DataRef, Reader, SheetType, SheetVisible, Sheets, XlsxError};
use quiz_core::{export_table, import_tables, Category, ImportIssue, ImportNote, Quiz, Slide, Table};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Component, Path, PathBuf};
use ts_rs::TS;

const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";

/// The largest spreadsheet file read, and the most its parts may unpack to
/// (a workbook is a ZIP archive): a sheet of questions is far smaller, and
/// an import runs in the same process as a show.
pub const MAX_SHEET_BYTES: u64 = 50 << 20;
/// The most rows an import reads, over all sheets.
pub const MAX_SHEET_ROWS: usize = 10_000;
/// The most columns a sheet may span: far more than the template has.
pub const MAX_SHEET_COLUMNS: usize = 200;

/// The slides of a spreadsheet, ready for the editor to insert, and what
/// the import could not take over as the sheet has it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SheetImport {
    pub slides: Vec<Slide>,
    pub notes: Vec<ImportNote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SheetExport {
    /// Media files next to the sheet, each once.
    pub media: usize,
    /// Media the quiz names that are not in it: their cells stay empty.
    pub missing: Vec<String>,
}

/// Makes slides of the `tables` read from the spreadsheet at `path` (see
/// `read_tables`) and copies the media they use into the package folder
/// `root`. Answers with none marked right are shuffled with `seed`, and a
/// row naming one of `categories` gets its id (see `import_tables`). A media
/// file is looked for where its cell says, relative to the spreadsheet, and
/// else next to it by its name, so a sheet made on another computer works
/// with its media beside it. A file taken from outside the sheet's folder
/// gets a note; a network or device path is never read.
pub fn import_sheet(path: &Path, tables: &[Table], categories: &[Category], root: &Path, seed: u64) -> Result<SheetImport> {
    let imported = import_tables(tables, categories, seed)?;
    let folder = path.parent().unwrap_or(Path::new(""));
    let mut notes = imported.notes;
    // The reference each file got and whether it came from outside the sheet's folder, or why it got none.
    let mut copied: BTreeMap<String, std::result::Result<(String, bool), ImportIssue>> = BTreeMap::new();
    let mut slides = Vec::new();
    for (place, mut slide) in imported.slides {
        let files: Vec<String> = slide.media().into_iter().map(|(file, _)| file.to_string()).collect();
        for file in files {
            if !copied.contains_key(&file) {
                let import = |source: &Path| crate::import_media(root, MediaSource::File(source));
                let reference = match find_media(folder, &file) {
                    Found::Beside(source) => Ok((import(&source)?, false)),
                    Found::Outside(source) => Ok((import(&source)?, true)),
                    Found::NetworkPath => Err(ImportIssue::NetworkPath { file: file.clone() }),
                    Found::Missing => Err(ImportIssue::FileNotFound { file: file.clone() }),
                };
                copied.insert(file.clone(), reference);
            }
            match &copied[&file] {
                Err(issue) => {
                    notes.add(issue.clone(), &place);
                    slide.remove_media(&file);
                }
                Ok((_, true)) => notes.add(ImportIssue::FileOutsideSheetFolder { file: file.clone() }, &place),
                Ok((_, false)) => {}
            }
        }
        slides.push(slide);
    }
    let renamed: BTreeMap<String, String> = copied.into_iter().filter_map(|(file, reference)| Some((file, reference.ok()?.0))).collect();
    for slide in &mut slides {
        slide.rename_media(&renamed);
    }
    Ok(SheetImport { slides, notes: notes.into_vec() })
}

/// Where the file a media cell names was found.
enum Found {
    /// In the sheet's folder or one below it.
    Beside(PathBuf),
    /// Where a full path, or a path out of the sheet's folder, says.
    Outside(PathBuf),
    /// Named by a network or device path and not next to the sheet.
    NetworkPath,
    Missing,
}

/// The file a media cell names: where it says, then by its name next to the
/// sheet. A network or device path (`\\server\share`, `\\?\`, `\\.\`,
/// `\??\`, or with forward slashes) is not looked at, as Windows would
/// contact the server, and sign in to it, just to see whether the file is
/// there.
fn find_media(folder: &Path, file: &str) -> Found {
    let mut start = file.chars();
    let network = matches!((start.next(), start.next()), (Some('\\' | '/'), Some('\\' | '/' | '?')));
    // A sheet made on Windows may name subfolders with backslashes.
    let portable = if cfg!(windows) { file.to_string() } else { file.replace('\\', "/") };
    let said = folder.join(&portable);
    if !network && said.is_file() {
        let inside = Path::new(&portable).components().all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
        return if inside { Found::Beside(said) } else { Found::Outside(said) };
    }
    // A path from another computer, in either system's form, may still name a file next to the sheet.
    let name = Path::new(file.rsplit(['/', '\\']).next().unwrap_or(file));
    let plain_name = matches!(name.components().collect::<Vec<_>>()[..], [Component::Normal(_)]);
    match folder.join(name) {
        beside if plain_name && beside.is_file() => Found::Beside(beside),
        _ if network => Found::NetworkPath,
        _ => Found::Missing,
    }
}

/// What reading a spreadsheet gave, as the app's sheet reader process
/// reports it. The app reads spreadsheets in a process of its own, so a
/// file that breaks a parser cannot end a running show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SheetRead {
    Tables(Vec<Table>),
    TooLarge,
    Unreadable(String),
}

impl SheetRead {
    /// Reads the spreadsheet at `path` (see `read_tables`).
    pub fn of(path: &Path) -> SheetRead {
        match read_tables(path) {
            Ok(tables) => SheetRead::Tables(tables),
            Err(QuizFileError::SpreadsheetTooLarge) => SheetRead::TooLarge,
            Err(QuizFileError::Spreadsheet(reason)) => SheetRead::Unreadable(reason),
            Err(other) => SheetRead::Unreadable(other.to_string()),
        }
    }

    pub fn into_result(self) -> Result<Vec<Table>> {
        match self {
            SheetRead::Tables(tables) => Ok(tables),
            SheetRead::TooLarge => Err(QuizFileError::SpreadsheetTooLarge),
            SheetRead::Unreadable(reason) => Err(QuizFileError::Spreadsheet(reason)),
        }
    }
}

/// The tables of a workbook's visible worksheets, or of a CSV file, within
/// `MAX_SHEET_BYTES`, `MAX_SHEET_ROWS` and `MAX_SHEET_COLUMNS`.
pub fn read_tables(path: &Path) -> Result<Vec<Table>> {
    read_tables_within(path, MAX_SHEET_BYTES, MAX_SHEET_ROWS)
}

fn read_tables_within(path: &Path, max_bytes: u64, max_rows: usize) -> Result<Vec<Table>> {
    let size = fs::metadata(path).context(FileAction::Read, path)?.len();
    if size > max_bytes || unpacked_size(path, max_bytes)? > max_bytes {
        return Err(QuizFileError::SpreadsheetTooLarge);
    }
    let extension = path.extension().map(|extension| extension.to_string_lossy().to_ascii_lowercase());
    if extension.as_deref() == Some("csv") {
        let bytes = fs::read(path).context(FileAction::Read, path)?;
        return Ok(vec![csv_table(&bytes, max_rows)?]);
    }
    let mut workbook = open_workbook_auto(path).map_err(unreadable)?;
    let sheets = workbook.sheets_metadata().to_vec();
    let mut tables: Vec<Table> = Vec::new();
    for sheet in sheets.into_iter().filter(|sheet| sheet.typ == SheetType::WorkSheet && sheet.visible == SheetVisible::Visible) {
        let rows_left = max_rows - tables.iter().map(|table| table.rows.len()).sum::<usize>();
        tables.push(sheet_table(&mut workbook, sheet.name, rows_left)?);
    }
    Ok(tables)
}

fn unreadable(e: impl std::fmt::Display) -> QuizFileError {
    QuizFileError::Spreadsheet(e.to_string())
}

/// A worksheet as a table of at most `max_rows` rows. An Excel sheet is
/// read cell by cell, so a cell far off is refused before the sheet takes
/// memory for the cells in between; the parser reads an `.xls` or `.ods`
/// sheet whole.
fn sheet_table(workbook: &mut Sheets<BufReader<fs::File>>, name: String, max_rows: usize) -> Result<Table> {
    let text = |value: &DataRef| cell_text(&Data::from(value.clone()));
    match workbook {
        Sheets::Xlsx(book) => match book.worksheet_cells_reader(&name) {
            Ok(mut cells) => {
                let cells = std::iter::from_fn(|| cells.next_cell().transpose());
                table(name, cells.map(|cell| cell.map(|cell| (cell.get_position(), text(cell.get_value()))).map_err(unreadable)), max_rows)
            }
            Err(XlsxError::NotAWorksheet(_)) => Ok(Table { name, first_row: 1, rows: Vec::new() }),
            Err(e) => Err(unreadable(e)),
        },
        Sheets::Xlsb(book) => {
            let mut cells = book.worksheet_cells_reader(&name).map_err(unreadable)?;
            let cells = std::iter::from_fn(|| cells.next_cell().transpose());
            table(name, cells.map(|cell| cell.map(|cell| (cell.get_position(), text(cell.get_value()))).map_err(unreadable)), max_rows)
        }
        whole => {
            let range = whole.worksheet_range(&name).map_err(unreadable)?;
            let (top, left) = range.start().unwrap_or((0, 0));
            let cells = range.used_cells().map(|(row, column, value)| Ok(((top + row as u32, left + column as u32), cell_text(value))));
            table(name, cells, max_rows)
        }
    }
}

/// A sheet's table from its cells and their (row, column), in any order.
/// Its rows run from the first row with a value to the last, and cells keep
/// their columns, so a note can name a column by its letters. Refused as
/// soon as the cells with a value span more than `max_rows` rows or
/// `MAX_SHEET_COLUMNS` columns.
fn table(name: String, cells: impl Iterator<Item = Result<((u32, u32), String)>>, max_rows: usize) -> Result<Table> {
    let mut filled = Vec::new();
    let (mut top, mut bottom, mut right) = (u32::MAX, 0, 0);
    for cell in cells {
        let ((row, column), text) = cell?;
        if text.is_empty() {
            continue;
        }
        (top, bottom, right) = (top.min(row), bottom.max(row), right.max(column));
        if (bottom - top) as usize >= max_rows || right as usize >= MAX_SHEET_COLUMNS {
            return Err(QuizFileError::SpreadsheetTooLarge);
        }
        filled.push((row, column, text));
    }
    if filled.is_empty() {
        return Ok(Table { name, first_row: 1, rows: Vec::new() });
    }
    let mut rows = vec![vec![String::new(); right as usize + 1]; (bottom - top) as usize + 1];
    for (row, column, text) in filled {
        rows[(row - top) as usize][column as usize] = text;
    }
    Ok(Table { name, first_row: top.saturating_add(1), rows })
}

/// What a workbook that is a ZIP archive (xlsx, xlsm, ods) unpacks to,
/// counted by unpacking it (an archive's own sizes may lie) up to just past
/// `max`; 0 for any other file, which its parser reads as it is.
fn unpacked_size(path: &Path, max: u64) -> Result<u64> {
    let file = fs::File::open(path).context(FileAction::Read, path)?;
    let Ok(mut archive) = zip::ZipArchive::new(file) else { return Ok(0) };
    let mut total = 0;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| QuizFileError::Spreadsheet(e.to_string()))?;
        total += std::io::copy(&mut entry.take(max + 1 - total), &mut std::io::sink()).context(FileAction::Read, path)?;
        if total > max {
            break;
        }
    }
    Ok(total)
}

/// A cell as text: a number as Excel shows it without formatting (`30`, `1.5`).
fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Error(_) => String::new(),
        Data::String(text) => text.replace("\r\n", "\n"),
        other => other.to_string(),
    }
}

/// A CSV file as a table. Excel writes it in the system's code page and
/// list separator unless told otherwise, so both are recognised: UTF-8
/// (with or without a byte order mark) or UTF-16 with one, else Windows-1250
/// (Central European); semicolons, commas or tabs, by the headings row.
/// Refused at the first record past `max_rows` or `MAX_SHEET_COLUMNS`.
fn csv_table(bytes: &[u8], max_rows: usize) -> Result<Table> {
    let text = decode(bytes);
    let mut reader = csv::ReaderBuilder::new().has_headers(false).flexible(true).delimiter(separator(&text)).from_reader(text.as_bytes());
    let mut rows: Vec<Vec<String>> = Vec::new();
    for record in reader.records() {
        let record = record.map_err(unreadable)?;
        if rows.len() == max_rows || record.len() > MAX_SHEET_COLUMNS {
            return Err(QuizFileError::SpreadsheetTooLarge);
        }
        rows.push(record.iter().map(|cell| cell.replace("\r\n", "\n")).collect());
    }
    Ok(Table { name: String::new(), first_row: 1, rows })
}

fn decode(bytes: &[u8]) -> Cow<'_, str> {
    if let Some((encoding, bom)) = encoding_rs::Encoding::for_bom(bytes) {
        return encoding.decode_without_bom_handling(&bytes[bom..]).0;
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => Cow::Borrowed(text),
        Err(_) => encoding_rs::WINDOWS_1250.decode_without_bom_handling(bytes).0,
    }
}

/// The separator the headings row uses most, outside quotes; a comma if none.
fn separator(text: &str) -> u8 {
    let mut counts = [(b';', 0), (b'\t', 0), (b',', 0)];
    let mut quoted = false;
    for c in text.bytes() {
        match c {
            b'"' => quoted = !quoted,
            b'\n' if !quoted => break,
            _ if !quoted => counts.iter_mut().filter(|(separator, _)| *separator == c).for_each(|(_, n)| *n += 1),
            _ => {}
        }
    }
    // The last of equal counts wins: a comma, as CSV means.
    counts.iter().max_by_key(|(_, n)| *n).map_or(b',', |&(separator, _)| separator)
}

/// Writes the quiz's slides to `dest` as a CSV file that Excel opens (UTF-8
/// with a byte order mark, cells separated by `separator`) and copies the
/// media they use, read from the package folder `root`, next to it under
/// their own names. Media that several slides share are written once; a
/// file of the same name with other contents already there is kept, and the
/// copy gets a number added.
pub fn export_sheet(quiz: &Quiz, root: &Path, dest: &Path, separator: u8) -> Result<SheetExport> {
    let folder = dest.parent().unwrap_or(Path::new(""));
    let mut names = BTreeMap::new();
    let mut missing = Vec::new();
    for reference in quiz.media_files() {
        let source = root.join(reference);
        if !source.is_file() {
            missing.push(reference.to_string());
            continue;
        }
        let name = reference.rsplit('/').next().unwrap_or(reference);
        names.insert(reference, place_file(folder, name, MediaSource::File(&source))?);
    }
    let rows = export_table(quiz, |reference| names.get(reference).cloned());
    let unwritable = |e: csv::Error| QuizFileError::Spreadsheet(e.to_string());
    let mut writer = csv::WriterBuilder::new().delimiter(separator).terminator(csv::Terminator::CRLF).from_writer(UTF8_BOM.to_vec());
    for row in rows {
        writer.write_record(&row).map_err(unwritable)?;
    }
    let bytes = writer.into_inner().map_err(|e| QuizFileError::Spreadsheet(e.to_string()))?;
    write_atomic(dest, &bytes)?;
    Ok(SheetExport { media: names.len(), missing })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiz_core::{Background, ImportIssue, Place, SlideContent, SoundClip};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("quiz-sheet-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn csv_files_are_read_in_the_encodings_and_separators_excel_writes() {
        let semicolons = b"Q;A1;A2\r\n\"P\xf8\xedli\x9a; \"\"\x9elu\x9dou\xe8k\xfd\"\"\";[ano];ne\r\n";
        let table = csv_table(semicolons, MAX_SHEET_ROWS).unwrap();
        assert_eq!(table.rows, [vec!["Q", "A1", "A2"], vec!["Příliš; \"žluťoučký\"", "[ano]", "ne"]]);

        let utf8 = "\u{feff}Q,NOTES\r\n\"Two\r\nlines\",ok\r\n".as_bytes();
        assert_eq!(csv_table(utf8, MAX_SHEET_ROWS).unwrap().rows[1], ["Two\nlines", "ok"]);

        let utf16: Vec<u8> = [0xff, 0xfe].into_iter().chain("Q\tA1\nŽ\tx\n".encode_utf16().flat_map(u16::to_le_bytes)).collect();
        assert_eq!(csv_table(&utf16, MAX_SHEET_ROWS).unwrap().rows[1], ["Ž", "x"]);
        assert_eq!((separator("Q"), separator("\"a;b\",c")), (b',', b','));
    }

    #[test]
    fn a_spreadsheet_too_large_or_with_too_many_rows_is_refused() {
        let dir = temp_dir("limits");
        fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("quiz.csv");
        fs::write(&csv, "Q\nOne?\nTwo?\n").unwrap();
        assert_eq!(read_tables_within(&csv, 100, 3).unwrap()[0].rows.len(), 3);
        assert!(matches!(read_tables_within(&csv, 100, 2), Err(QuizFileError::SpreadsheetTooLarge)));
        assert!(matches!(read_tables_within(&csv, 10, 3), Err(QuizFileError::SpreadsheetTooLarge)));

        // A workbook is measured by what it unpacks to, not by the file.
        let xlsx = dir.join("quiz.xlsx");
        let cells = format!(r#"<row r="1"><c r="A1" t="inlineStr"><is><t>Q</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>{}</t></is></c></row>"#, "x".repeat(20_000));
        workbook(&xlsx, &[("Sheet1", false, &cells)]);
        let size = fs::metadata(&xlsx).unwrap().len();
        assert!(size < 10_000, "{size}");
        assert!(read_tables_within(&xlsx, 10_000, 10).is_err_and(|e| matches!(e, QuizFileError::SpreadsheetTooLarge)));
        assert_eq!(read_tables_within(&xlsx, 100_000, 10).unwrap()[0].rows.len(), 2);

        // Too many columns.
        fs::write(&csv, format!("Q{}\n", ";x".repeat(MAX_SHEET_COLUMNS))).unwrap();
        assert!(matches!(read_tables(&csv), Err(QuizFileError::SpreadsheetTooLarge)));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_far_off_cell_is_refused_before_the_cells_in_between_take_memory() {
        let dir = temp_dir("far");
        let text = |cell: &str, row: u32| format!(r#"<row r="{row}"><c r="{cell}" t="inlineStr"><is><t>x</t></is></c></row>"#);
        let path = dir.join("quiz.xlsx");
        // Spread over every row and column Excel has, these two cells would be 17 billion.
        workbook(&path, &[("Sheet1", false, &(text("A1", 1) + &text("XFD1048576", 1_048_576)))]);
        assert!(matches!(read_tables(&path), Err(QuizFileError::SpreadsheetTooLarge)));
        workbook(&path, &[("Sheet1", false, &(text("A1", 1) + &text("A10001", 10_001)))]);
        assert!(matches!(read_tables(&path), Err(QuizFileError::SpreadsheetTooLarge)));
        // The rows are counted over all sheets as they are read.
        let rows = |n: u32| (1..=n).map(|row| text(&format!("A{row}"), row)).collect::<String>();
        workbook(&path, &[("One", false, &rows(6)), ("Two", false, &rows(6))]);
        assert!(matches!(read_tables_within(&path, MAX_SHEET_BYTES, 10), Err(QuizFileError::SpreadsheetTooLarge)));
        assert_eq!(read_tables_within(&path, MAX_SHEET_BYTES, 12).unwrap().len(), 2);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_reader_process_reports_tables_or_why_there_are_none() {
        let dir = temp_dir("report");
        let csv = dir.join("quiz.csv");
        fs::write(&csv, "Q\nOne?\n").unwrap();
        let read = SheetRead::of(&csv);
        let sent: SheetRead = serde_json::from_str(&serde_json::to_string(&read).unwrap()).unwrap();
        assert_eq!(sent.into_result().unwrap()[0].rows, [vec!["Q"], vec!["One?"]]);
        fs::write(dir.join("notes.xlsx"), b"not a zip").unwrap();
        let SheetRead::Unreadable(reason) = SheetRead::of(&dir.join("notes.xlsx")) else { panic!("read") };
        assert!(!reason.starts_with("the spreadsheet cannot be read"), "{reason}");
        assert!(matches!(SheetRead::TooLarge.into_result(), Err(QuizFileError::SpreadsheetTooLarge)));
        assert!(matches!(SheetRead::of(&dir.join("gone.csv")), SheetRead::Unreadable(_)));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_import_copies_the_media_its_cells_name_into_the_quiz() {
        let dir = temp_dir("import");
        let (sheets, root) = (dir.join("sheets"), dir.join("draft"));
        fs::create_dir_all(sheets.join("pictures")).unwrap();
        fs::create_dir_all(&root).unwrap();
        fs::write(sheets.join("pictures").join("flag.png"), b"png").unwrap();
        fs::write(sheets.join("anthem.mp3"), b"mp3").unwrap();
        let csv = "Q;P1;SOUND;BACKGROUND\n\
                   Flag?;pictures\\flag.png;anthem.mp3;\n\
                   Again?;pictures/flag.png;C:\\Users\\Ann\\anthem.mp3;stage.jpg\n";
        fs::write(sheets.join("quiz.csv"), csv).unwrap();

        let imported = import_sheet(&sheets.join("quiz.csv"), &read_tables(&sheets.join("quiz.csv")).unwrap(), &[], &root, 1).unwrap();
        assert_eq!(imported.slides.len(), 2);
        for slide in &imported.slides {
            assert_eq!(slide.picture.as_deref(), Some("media/flag.png"));
            assert_eq!(slide.sound, Some(SoundClip::new("media/anthem.mp3")), "found by its name next to the sheet");
        }
        assert_eq!(imported.slides[1].background, None);
        assert_eq!(fs::read(root.join("media/flag.png")).unwrap(), b"png");
        assert_eq!(fs::read_dir(root.join("media")).unwrap().count(), 2, "each file is copied once");
        assert_eq!(
            imported.notes,
            [ImportNote { issue: ImportIssue::FileNotFound { file: "stage.jpg".into() }, places: vec![Place { sheet: String::new(), row: 3 }] }]
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_import_notes_files_from_outside_the_sheets_folder_and_never_reads_a_network_path() {
        let dir = temp_dir("outside");
        let (sheets, root) = (dir.join("sheets"), dir.join("draft"));
        fs::create_dir_all(&sheets).unwrap();
        fs::create_dir_all(&root).unwrap();
        fs::write(dir.join("private.png"), b"private").unwrap();
        fs::write(sheets.join("near.png"), b"near").unwrap();
        let full = dir.join("private.png").display().to_string();
        // On Linux this names the same file, so the file not being taken shows it was not read.
        let share = format!("//{}", full.trim_start_matches('/'));
        let csv = format!("Q;P1\nUp?;../private.png\nFull?;{full}\nShare?;{share}\nNear?;\\\\server\\share\\near.png\nNt?;\\??\\UNC\\server\\near.png\n");
        fs::write(sheets.join("quiz.csv"), csv).unwrap();

        let imported = import_sheet(&sheets.join("quiz.csv"), &read_tables(&sheets.join("quiz.csv")).unwrap(), &[], &root, 1).unwrap();
        let pictures: Vec<Option<&str>> = imported.slides.iter().map(|slide| slide.picture.as_deref()).collect();
        assert_eq!(pictures, [Some("media/private.png"), Some("media/private.png"), None, Some("media/near.png"), Some("media/near.png")]);
        let note = |issue, row| ImportNote { issue, places: vec![Place { sheet: String::new(), row }] };
        assert_eq!(
            imported.notes,
            [
                note(ImportIssue::FileOutsideSheetFolder { file: "../private.png".into() }, 2),
                note(ImportIssue::FileOutsideSheetFolder { file: full }, 3),
                note(ImportIssue::NetworkPath { file: share }, 4),
            ]
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_export_writes_a_csv_file_with_the_media_beside_it_and_imports_again() {
        let dir = temp_dir("export");
        let (root, out) = (dir.join("draft"), dir.join("out"));
        fs::create_dir_all(root.join("media/round 2")).unwrap();
        fs::create_dir_all(&out).unwrap();
        fs::write(root.join("media/flag.png"), b"one").unwrap();
        fs::write(root.join("media/round 2/flag.png"), b"two").unwrap();
        fs::write(root.join("media/waves.mp4"), b"mp4").unwrap();
        // A file of the user's with the same name stays.
        fs::write(out.join("waves.mp4"), b"mine").unwrap();

        let mut first = Slide::new("a", SlideContent::Billboard { title: "Round 1".into(), text: String::new() });
        first.picture = Some("media/flag.png".into());
        first.background = Some(Background::new("media/waves.mp4"));
        let mut second = Slide::new("b", SlideContent::Billboard { title: "Round 2".into(), text: String::new() });
        second.picture = Some("media/round 2/flag.png".into());
        second.background = Some(Background::new("media/waves.mp4"));
        second.sound = Some(SoundClip::new("media/gone.mp3"));
        let quiz = Quiz::new("Quiz", vec![first, second]);

        let report = export_sheet(&quiz, &root, &out.join("quiz.csv"), b';').unwrap();
        assert_eq!(report, SheetExport { media: 3, missing: vec!["media/gone.mp3".into()] });
        let bytes = fs::read(out.join("quiz.csv")).unwrap();
        assert!(bytes.starts_with(UTF8_BOM));
        let text = String::from_utf8(bytes[UTF8_BOM.len()..].to_vec()).unwrap();
        assert!(text.starts_with("Q;A1;A2;"), "{text}");
        assert!(text.contains("{B}Round 2;;;;;;;flag-2.png;;;;;waves-2.mp4;"), "{text}");
        assert_eq!(fs::read(out.join("flag-2.png")).unwrap(), b"two");
        assert_eq!(fs::read(out.join("waves.mp4")).unwrap(), b"mine");

        let draft = dir.join("draft-2");
        let back = import_sheet(&out.join("quiz.csv"), &read_tables(&out.join("quiz.csv")).unwrap(), &[], &draft, 1).unwrap();
        assert!(back.notes.is_empty(), "{:?}", back.notes);
        assert_eq!(back.slides[1].picture.as_deref(), Some("media/flag-2.png"));
        assert_eq!(back.slides[0].background, Some(Background::new("media/waves-2.mp4")));
        assert_eq!(fs::read(draft.join("media/waves-2.mp4")).unwrap(), b"mp4");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A sheet of a test workbook: its name, whether it is hidden, its rows as XML.
    type SheetXml<'a> = (&'a str, bool, &'a str);

    /// A minimal Excel workbook with these sheets.
    fn workbook(path: &Path, sheets: &[SheetXml]) {
        const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
        const RELS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
        const DOC: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        let mut add = |name: &str, xml: String| {
            zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut zip, xml.as_bytes()).unwrap();
        };
        let numbered = |f: &dyn Fn(usize, &SheetXml) -> String| sheets.iter().enumerate().map(|(i, sheet)| f(i + 1, sheet)).collect::<String>();
        let overrides = numbered(&|n, _| {
            format!(r#"<Override PartName="/xl/worksheets/sheet{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#)
        });
        add(
            "[Content_Types].xml",
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>{overrides}</Types>"#
            ),
        );
        add("_rels/.rels", format!(r#"<Relationships xmlns="{RELS}"><Relationship Id="rId1" Type="{DOC}/officeDocument" Target="xl/workbook.xml"/></Relationships>"#));
        let entries = numbered(&|n, (name, hidden, _)| {
            let state = if *hidden { r#" state="hidden""# } else { "" };
            format!(r#"<sheet name="{name}" sheetId="{n}"{state} r:id="rId{n}"/>"#)
        });
        add("xl/workbook.xml", format!(r#"<workbook xmlns="{MAIN}" xmlns:r="{DOC}"><sheets>{entries}</sheets></workbook>"#));
        let links = numbered(&|n, _| format!(r#"<Relationship Id="rId{n}" Type="{DOC}/worksheet" Target="worksheets/sheet{n}.xml"/>"#));
        add("xl/_rels/workbook.xml.rels", format!(r#"<Relationships xmlns="{RELS}">{links}</Relationships>"#));
        for (n, (_, _, rows)) in sheets.iter().enumerate() {
            add(&format!("xl/worksheets/sheet{}.xml", n + 1), format!(r#"<worksheet xmlns="{MAIN}"><sheetData>{rows}</sheetData></worksheet>"#));
        }
        zip.finish().unwrap();
    }

    #[test]
    fn a_workbook_gives_a_table_per_visible_sheet() {
        let dir = temp_dir("workbook");
        let text = |cell: &str, value: &str| format!(r#"<c r="{cell}" t="inlineStr"><is><t>{value}</t></is></c>"#);
        let round = format!(
            r#"<row r="2">{}{}</row><row r="3">{}<c r="C3"><v>30</v></c><c r="D3"><v>1.5</v></c></row>"#,
            text("B2", "Q"),
            text("C2", "ANSWER-TIME"),
            text("B3", "Capital?")
        );
        let path = dir.join("quiz.xlsx");
        workbook(&path, &[("Round 1", false, &round), ("Answers", true, &format!(r#"<row r="1">{}</row>"#, text("A1", "Q")))]);
        let tables = read_tables(&path).unwrap();
        assert_eq!(tables.len(), 1, "the hidden sheet is left out");
        assert_eq!((tables[0].name.as_str(), tables[0].first_row), ("Round 1", 2));
        assert_eq!(tables[0].rows, [vec!["", "Q", "ANSWER-TIME", ""], vec!["", "Capital?", "30", "1.5"]]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_that_is_no_spreadsheet_is_refused() {
        let dir = temp_dir("refused");
        fs::write(dir.join("notes.xlsx"), b"not a zip").unwrap();
        assert!(matches!(read_tables(&dir.join("notes.xlsx")), Err(QuizFileError::Spreadsheet(_))));
        fs::write(dir.join("songs.csv"), "Title,Artist\nSong,Band\n").unwrap();
        let songs = dir.join("songs.csv");
        assert!(matches!(import_sheet(&songs, &read_tables(&songs).unwrap(), &[], &dir, 1), Err(QuizFileError::Sheet(_))));
        fs::remove_dir_all(&dir).unwrap();
    }
}
