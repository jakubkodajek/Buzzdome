//! Questions from a spreadsheet and to one (spec QZ-8). An import reads an
//! Excel or OpenDocument workbook or a CSV file into tables, makes slides of
//! them (`quiz_core::import_tables`) and copies the media files their cells
//! name into the quiz. An export writes the slides as a CSV file and the
//! media next to it, under their own names.

use crate::{place_file, write_atomic, Context, FileAction, MediaSource, QuizFileError, Result};
use calamine::{open_workbook_auto, Data, Reader, SheetType, SheetVisible};
use quiz_core::{export_table, import_tables, ImportIssue, ImportNote, Quiz, Slide, Table};
use serde::Serialize;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use ts_rs::TS;

const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";

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

/// Reads the spreadsheet at `path` into slides and copies the media they
/// use into the package folder `root`. Answers whose correct one is not
/// marked are shuffled with `seed`. A media file is looked for where its
/// cell says, relative to the spreadsheet, and else next to it by its name,
/// so a sheet made on another computer works with its media beside it.
pub fn import_sheet(path: &Path, root: &Path, seed: u64) -> Result<SheetImport> {
    let imported = import_tables(&read_tables(path)?, seed)?;
    let folder = path.parent().unwrap_or(Path::new(""));
    let mut notes = imported.notes;
    // The reference each file got; none for one that is not there.
    let mut copied: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut slides = Vec::new();
    for (place, mut slide) in imported.slides {
        let files: Vec<String> = slide.media().into_iter().map(|(file, _)| file.to_string()).collect();
        for file in files {
            if !copied.contains_key(&file) {
                let reference = match find_media(folder, &file) {
                    Some(source) => Some(crate::import_media(root, MediaSource::File(&source))?),
                    None => None,
                };
                copied.insert(file.clone(), reference);
            }
            if copied[&file].is_none() {
                notes.add(ImportIssue::FileNotFound { file: file.clone() }, &place);
                slide.remove_media(&file);
            }
        }
        slides.push(slide);
    }
    let renamed: BTreeMap<String, String> = copied.into_iter().filter_map(|(file, reference)| Some((file, reference?))).collect();
    for slide in &mut slides {
        slide.rename_media(&renamed);
    }
    Ok(SheetImport { slides, notes: notes.into_vec() })
}

/// The file a media cell names: where it says, then by its name next to the sheet.
fn find_media(folder: &Path, file: &str) -> Option<PathBuf> {
    // A sheet made on Windows may name subfolders with backslashes.
    let portable = if cfg!(windows) { file.to_string() } else { file.replace('\\', "/") };
    // A path from another computer, in either system's form, may still name a file next to the sheet.
    let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
    [folder.join(&portable), folder.join(name)].into_iter().find(|path| path.is_file())
}

/// The tables of a workbook's visible worksheets, or of a CSV file.
pub fn read_tables(path: &Path) -> Result<Vec<Table>> {
    let extension = path.extension().map(|extension| extension.to_string_lossy().to_ascii_lowercase());
    if extension.as_deref() == Some("csv") {
        let bytes = fs::read(path).context(FileAction::Read, path)?;
        return Ok(vec![csv_table(&bytes)?]);
    }
    let unreadable = |e: calamine::Error| QuizFileError::Spreadsheet(e.to_string());
    let mut workbook = open_workbook_auto(path).map_err(unreadable)?;
    let sheets = workbook.sheets_metadata().to_vec();
    let mut tables = Vec::new();
    for sheet in sheets.into_iter().filter(|sheet| sheet.typ == SheetType::WorkSheet && sheet.visible == SheetVisible::Visible) {
        let range = workbook.worksheet_range(&sheet.name).map_err(unreadable)?;
        let (first_row, first_column) = range.start().unwrap_or((0, 0));
        // Cells keep their columns, so a note can name a column by its letters.
        let lead = std::iter::repeat_n(String::new(), first_column as usize);
        let rows = range.rows().map(|row| lead.clone().chain(row.iter().map(cell_text)).collect()).collect();
        tables.push(Table { name: sheet.name, first_row: first_row + 1, rows });
    }
    Ok(tables)
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
fn csv_table(bytes: &[u8]) -> Result<Table> {
    let text = decode(bytes);
    let mut reader = csv::ReaderBuilder::new().has_headers(false).flexible(true).delimiter(separator(&text)).from_reader(text.as_bytes());
    let rows = reader
        .records()
        .map(|record| record.map(|record| record.iter().map(|cell| cell.replace("\r\n", "\n")).collect()))
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| QuizFileError::Spreadsheet(e.to_string()))?;
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
        let table = csv_table(semicolons).unwrap();
        assert_eq!(table.rows, [vec!["Q", "A1", "A2"], vec!["Příliš; \"žluťoučký\"", "[ano]", "ne"]]);

        let utf8 = "\u{feff}Q,NOTES\r\n\"Two\r\nlines\",ok\r\n".as_bytes();
        assert_eq!(csv_table(utf8).unwrap().rows[1], ["Two\nlines", "ok"]);

        let utf16: Vec<u8> = [0xff, 0xfe].into_iter().chain("Q\tA1\nŽ\tx\n".encode_utf16().flat_map(u16::to_le_bytes)).collect();
        assert_eq!(csv_table(&utf16).unwrap().rows[1], ["Ž", "x"]);
        assert_eq!((separator("Q"), separator("\"a;b\",c")), (b',', b','));
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

        let imported = import_sheet(&sheets.join("quiz.csv"), &root, 1).unwrap();
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
        let back = import_sheet(&out.join("quiz.csv"), &draft, 1).unwrap();
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
        assert!(matches!(import_sheet(&dir.join("songs.csv"), &dir, 1), Err(QuizFileError::Sheet(_))));
        fs::remove_dir_all(&dir).unwrap();
    }
}
