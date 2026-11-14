//! Questions as a table (spec QZ-8): a row per slide, in columns headed in
//! English or Czech, read into slides on import and written from them on
//! export. Pure: `quiz-file` reads and writes the files and
//! finds the media files the cells name, so here a slide's media are the
//! file names as the cells give them.

use crate::media::{
    Background, MediaKind, Replay, SoundClip, SoundStart, Span, VideoClip, VideoEffect, MAX_GAIN_DB, MAX_INTRO_PAUSE_MS, MAX_REPLAY_DELAY_MS,
    MAX_VIDEO_SPEED, MIN_GAIN_DB, MIN_VIDEO_SPEED, NEW_VIDEO_INTRO_PAUSE_MS,
};
use crate::category::{category_named, Category};
use crate::settings::QuizLanguage;
use crate::text::heading_key;
use crate::model::{AnswerMode, CorrectAnswer, Question, Quiz, Slide, SlideContent, DEFAULT_POINTS, DEFAULT_TIME_LIMIT_S, MAX_ANSWERS, MAX_POINTS, MAX_TIME_LIMIT_S};
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;
use ts_rs::TS;

/// One sheet of a workbook, or a CSV file: rows of cells, the first holding
/// the column headings. Serialized for the app's sheet reader process.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    /// The sheet's name; empty for a CSV file.
    pub name: String,
    /// The spreadsheet's number of the headings row (1 for the first row).
    pub first_row: u32,
    pub rows: Vec<Vec<String>>,
}

/// A row of a sheet, as the spreadsheet numbers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    /// Empty for a CSV file.
    pub sheet: String,
    pub row: u32,
}

/// Something an import could not take over as the sheet has it, and what it
/// did instead. The UI words it in the quizmaster's language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum ImportIssue {
    /// A sheet with values but no question column, such as one of instructions: skipped.
    NoQuestionColumn,
    /// A heading this app does not know, or a column without one (named by
    /// its letter): its values are left out.
    UnknownColumn { column: String },
    /// A second column with the same heading: left out.
    DuplicateColumn { column: String },
    /// A type this app does not know: the row is read as if it had none.
    UnknownType { value: String },
    /// Answers on a slide that is no question: left out.
    AnswersLeftOut,
    /// A category (the Category column) the quiz does not have: the slide keeps the theme's look.
    UnknownCategory { name: String },
    /// A picture and a video: the picture is left out, as a slide shows one.
    PictureAndVideo,
    /// A background picture and a background video: the picture is left out.
    TwoBackgrounds,
    /// A picture or video on an end of round or final slide, which shows the ranking: left out.
    MediaOnRanking,
    /// A file this column cannot use (a sound in VIDEO, or a type the app
    /// cannot play): left out.
    WrongMediaType { column: String, file: String },
    /// A value that is no number in range: the default is kept.
    BadValue { column: String, value: String },
    UnknownSetting { column: String, setting: String },
    /// A setting value that is wrong or out of range: the default is kept.
    BadSetting { column: String, setting: String, value: String },
    /// A setting that does not apply here, such as `intro-pause` on a
    /// background video, or a replay on a slide that is no question.
    SettingNotUsed { column: String, setting: String },
    /// A media file that is not there: left out.
    FileNotFound { file: String },
    /// A media file named by a full path, or a path out of the sheet's
    /// folder: copied in all the same, but it may be a file nobody meant
    /// to put in the quiz.
    FileOutsideSheetFolder { file: String },
    /// A media file on a network share or device path (`\\server\share\…`),
    /// which is not read, and not next to the sheet by its name: left out.
    NetworkPath { file: String },
}

/// An issue and the rows it came up in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ImportNote {
    pub issue: ImportIssue,
    pub places: Vec<Place>,
}

/// The notes of an import, one per issue, in the order they first came up.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Notes(Vec<ImportNote>);

impl Notes {
    pub fn add(&mut self, issue: ImportIssue, place: &Place) {
        match self.0.iter_mut().find(|note| note.issue == issue) {
            Some(note) if note.places.last() == Some(place) => {}
            Some(note) => note.places.push(place.clone()),
            None => self.0.push(ImportNote { issue, places: vec![place.clone()] }),
        }
    }

    pub fn into_vec(self) -> Vec<ImportNote> {
        self.0
    }
}

/// Slides read from a spreadsheet, each with the row it came from, and the notes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportedRows {
    pub slides: Vec<(Place, Slide)>,
    pub notes: Notes,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SheetError {
    #[error("no sheet has a question column")]
    NoQuestionColumn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Type,
    Text,
    Answer(usize),
    Picture,
    Notes,
    Sound,
    Video,
    Background,
    BackgroundVideo,
    AnswerTime,
    PointsStart,
    PointsEnd,
    Penalty,
    Category,
    ReadAs,
}

/// A column this app reads and writes, with its heading in English and in
/// Czech: an export writes it in the quizmaster's language, an import
/// knows a column by either, whatever their case, accents, spaces and
/// dashes (`heading_key`). A column that is not `basic` is exported only
/// when a slide has a value in it, so a sheet shows what its quiz uses.
struct Column {
    field: Field,
    english: &'static str,
    czech: &'static str,
    basic: bool,
}

const fn basic(field: Field, english: &'static str, czech: &'static str) -> Column {
    Column { field, english, czech, basic: true }
}

const fn extra(field: Field, english: &'static str, czech: &'static str) -> Column {
    Column { field, english, czech, basic: false }
}

/// The columns, in the order an export writes them.
const FIELDS: [Column; 20] = [
    basic(Field::Type, "Type", "Typ"),
    basic(Field::Text, "Question", "Otázka"),
    basic(Field::Answer(0), "Answer 1", "Odpověď 1"),
    basic(Field::Answer(1), "Answer 2", "Odpověď 2"),
    basic(Field::Answer(2), "Answer 3", "Odpověď 3"),
    basic(Field::Answer(3), "Answer 4", "Odpověď 4"),
    basic(Field::Answer(4), "Answer 5", "Odpověď 5"),
    basic(Field::Answer(5), "Answer 6", "Odpověď 6"),
    basic(Field::Notes, "Notes", "Poznámky"),
    basic(Field::AnswerTime, "Answer time", "Čas na odpověď"),
    basic(Field::PointsStart, "Points", "Body"),
    extra(Field::PointsEnd, "Points at end", "Body na konci"),
    extra(Field::Penalty, "Penalty", "Srážka za chybu"),
    extra(Field::Picture, "Picture", "Obrázek"),
    extra(Field::Sound, "Sound", "Zvuk"),
    extra(Field::Video, "Video", "Video"),
    extra(Field::Background, "Background", "Pozadí"),
    extra(Field::BackgroundVideo, "Background video", "Video na pozadí"),
    extra(Field::Category, "Category", "Kategorie"),
    // How the voice says a question.
    extra(Field::ReadAs, "Read as", "Jak přečíst"),
];

impl Column {
    fn heading(&self, language: QuizLanguage) -> &'static str {
        in_language(language, self.english, self.czech)
    }

    fn is_named(&self, key: &str) -> bool {
        [self.english, self.czech].iter().any(|name| heading_key(name) == key)
    }
}

fn in_language(language: QuizLanguage, english: &'static str, czech: &'static str) -> &'static str {
    match language {
        QuizLanguage::Cs => czech,
        QuizLanguage::En => english,
    }
}

impl Field {
    fn index(self) -> usize {
        FIELDS.iter().position(|column| column.field == self).expect("every field has a column")
    }

    /// The field's name for the UI (`sheet_words`).
    fn key(self) -> String {
        match self {
            Field::Type => "type".into(),
            Field::Text => "question".into(),
            Field::Answer(i) => format!("answer{}", i + 1),
            Field::Picture => "picture".into(),
            Field::Notes => "notes".into(),
            Field::Sound => "sound".into(),
            Field::Video => "video".into(),
            Field::Background => "background".into(),
            Field::BackgroundVideo => "backgroundVideo".into(),
            Field::AnswerTime => "answerTime".into(),
            Field::PointsStart => "points".into(),
            Field::PointsEnd => "pointsEnd".into(),
            Field::Penalty => "penalty".into(),
            Field::Category => "category".into(),
            Field::ReadAs => "readAs".into(),
        }
    }
}

/// The headings and types in each language, by name, for the UI to show
/// in its example of a sheet (`SHEET_WORDS` in Constants.ts), so it shows
/// the words the import reads.
pub fn sheet_words() -> serde_json::Value {
    let words = |language: QuizLanguage| {
        let columns: serde_json::Map<String, serde_json::Value> = FIELDS.iter().map(|column| (column.field.key(), column.heading(language).into())).collect();
        let types: serde_json::Map<String, serde_json::Value> = KINDS.iter().map(|&(kind, key, ..)| (key.to_string(), kind.name(language).into())).collect();
        serde_json::json!({ "columns": columns, "types": types })
    };
    serde_json::json!({ "cs": words(QuizLanguage::Cs), "en": words(QuizLanguage::En) })
}

/// What the Type column makes of a row; an empty cell is a question, or a
/// billboard for a row with only media.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Question,
    /// A question answered by the fastest finger also with two to four
    /// answers, which would otherwise be everyone's (`mode_for`).
    FastestFinger,
    /// A billboard: the first line is its title, the rest its text.
    Section,
    Explanation,
    EndOfRound,
    Results,
}

/// The types, by their name for the UI (`sheet_words`), in English and in
/// Czech, known as headings are (`heading_key`).
const KINDS: [(Kind, &str, &str, &str); 6] = [
    (Kind::Question, "question", "Question", "Otázka"),
    (Kind::FastestFinger, "fastestFinger", "Fastest finger", "Rychlá odpověď"),
    (Kind::Section, "section", "Section", "Sekce"),
    (Kind::Explanation, "explanation", "Explanation", "Vysvětlení"),
    (Kind::EndOfRound, "endOfRound", "End of round", "Konec kola"),
    (Kind::Results, "results", "Results", "Výsledky"),
];

impl Kind {
    fn name(self, language: QuizLanguage) -> &'static str {
        let &(_, _, english, czech) = KINDS.iter().find(|(kind, ..)| *kind == self).expect("every type has a name");
        in_language(language, english, czech)
    }

    /// The type a Type cell names; none for an empty cell, and for one this
    /// app does not know, with a note.
    fn of(row: &Row, place: &Place, notes: &mut Notes) -> Option<Kind> {
        let cell = row.get(Field::Type);
        let key = heading_key(cell);
        if key.is_empty() {
            return None;
        }
        let found = KINDS.iter().find(|(_, _, english, czech)| heading_key(english) == key || heading_key(czech) == key).map(|&(kind, ..)| kind);
        if found.is_none() {
            notes.add(ImportIssue::UnknownType { value: cell.to_string() }, place);
        }
        found
    }
}

// Media settings, in braces after the file name: `intro.mp4{loop=false}`.
const START_AT: &str = "start-at";
const STOP_AT: &str = "stop-at";
const LOOP: &str = "loop";
const SILENT: &str = "silent";
const VOLUME: &str = "volume";
const SPEED: &str = "speed";
const EFFECT: &str = "effect";
const INTRO_PAUSE: &str = "intro-pause";
const SYNC_WITH_QUIZ: &str = "sync-with-quiz";
// This app's own, for options the reference's settings do not cover.
const AUTOPLAY: &str = "autoplay";
const PLAYS: &str = "plays";
const GAIN: &str = "gain";
const REPLAY: &str = "replay";
const REPLAY_START_AT: &str = "replay-start-at";
const REPLAY_STOP_AT: &str = "replay-stop-at";
const REPLAY_DELAY: &str = "replay-delay";
const SETTINGS: [&str; 16] = [
    START_AT,
    STOP_AT,
    LOOP,
    SILENT,
    VOLUME,
    SPEED,
    EFFECT,
    INTRO_PAUSE,
    SYNC_WITH_QUIZ,
    AUTOPLAY,
    PLAYS,
    GAIN,
    REPLAY,
    REPLAY_START_AT,
    REPLAY_STOP_AT,
    REPLAY_DELAY,
];

/// A video from a sheet: the intro pause of a video an author adds.
fn sheet_video(file: impl Into<String>) -> VideoClip {
    VideoClip { intro_pause_ms: NEW_VIDEO_INTRO_PAUSE_MS, ..VideoClip::new(file) }
}

/// Reads the slides of `tables`, in order. With more than one sheet of
/// questions, each sheet is a round that a billboard with its name opens.
/// Answers whose correct one is not marked are shuffled with `seed`. A row
/// that names one of `categories` (Category) gets its id; the editor gives the
/// slide its look. Slides get the ids `import-1`, `import-2`... for the
/// editor to replace.
pub fn import_tables(tables: &[Table], categories: &[Category], seed: u64) -> Result<ImportedRows, SheetError> {
    let mut shuffler = Shuffler(seed);
    let mut notes = Notes::default();
    let sheets: Vec<(&Table, Vec<(Place, Slide)>)> =
        tables.iter().filter_map(|table| Some((table, import_table(table, categories, &mut shuffler, &mut notes)?))).collect();
    if sheets.is_empty() {
        return Err(SheetError::NoQuestionColumn);
    }
    let rounds = sheets.iter().filter(|(_, slides)| !slides.is_empty()).count() > 1;
    let mut slides = Vec::new();
    for (table, rows) in sheets {
        if rounds && !rows.is_empty() {
            let title = SlideContent::Billboard { title: table.name.clone(), text: String::new() };
            slides.push((Place { sheet: table.name.clone(), row: table.first_row }, Slide::new("", title)));
        }
        slides.extend(rows);
    }
    for (n, (_, slide)) in slides.iter_mut().enumerate() {
        slide.id = format!("import-{}", n + 1);
    }
    Ok(ImportedRows { slides, notes })
}

/// The slides of one sheet; none when it has no question column. An empty sheet
/// is skipped without a note: workbooks often have some.
fn import_table(table: &Table, categories: &[Category], shuffler: &mut Shuffler, notes: &mut Notes) -> Option<Vec<(Place, Slide)>> {
    let has_value = |row: &Vec<String>, column: usize| row.get(column).is_some_and(|cell| !cell.trim().is_empty());
    let (headings, data) = table.rows.split_first()?;
    let at_headings = Place { sheet: table.name.clone(), row: table.first_row };
    let mut columns: Vec<Option<Field>> = Vec::new();
    let mut left_out = Vec::new();
    for (column, heading) in headings.iter().enumerate() {
        let (name, key) = (heading.trim().to_string(), heading_key(heading));
        let field = FIELDS.iter().find(|known| !key.is_empty() && known.is_named(&key)).map(|known| known.field);
        columns.push(field.filter(|field| !columns.contains(&Some(*field))));
        let issue = match field {
            Some(_) if columns[column].is_none() => ImportIssue::DuplicateColumn { column: name },
            Some(_) => continue,
            None if key.is_empty() => ImportIssue::UnknownColumn { column: column_letters(column) },
            None => ImportIssue::UnknownColumn { column: name },
        };
        left_out.push((column, issue));
    }
    if !columns.contains(&Some(Field::Text)) {
        if table.rows.iter().any(|row| (0..row.len()).any(|column| has_value(row, column))) {
            notes.add(ImportIssue::NoQuestionColumn, &at_headings);
        }
        return None;
    }
    // Values beyond the headings have no column either.
    let widest = data.iter().map(Vec::len).max().unwrap_or(0);
    left_out.extend((headings.len()..widest).map(|column| (column, ImportIssue::UnknownColumn { column: column_letters(column) })));
    for (column, issue) in left_out {
        if data.iter().any(|row| has_value(row, column)) {
            notes.add(issue, &at_headings);
        }
    }
    let slides = data.iter().zip(table.first_row + 1..).filter_map(|(cells, row)| {
        let place = Place { sheet: table.name.clone(), row };
        let slide = import_row(&Row::new(&columns, headings, cells), categories, &place, shuffler, notes)?;
        Some((place, slide))
    });
    Some(slides.collect())
}

/// A column's letters as spreadsheets show them: A to Z, then AA.
fn column_letters(column: usize) -> String {
    let mut letters = Vec::new();
    let mut n = column + 1;
    while n > 0 {
        letters.push(b'A' + ((n - 1) % 26) as u8);
        n = (n - 1) / 26;
    }
    letters.iter().rev().map(|&b| b as char).collect()
}

/// A row's cells by field, trimmed and without the `'` an export put before
/// what looks like a formula; empty where the sheet has none. With them,
/// the headings of their columns as the sheet writes them, which notes name.
struct Row<'a> {
    cells: [&'a str; FIELDS.len()],
    headings: [&'a str; FIELDS.len()],
}

impl<'a> Row<'a> {
    fn new(columns: &[Option<Field>], headings: &'a [String], cells: &'a [String]) -> Row<'a> {
        let mut row = Row { cells: [""; FIELDS.len()], headings: [""; FIELDS.len()] };
        for ((field, heading), cell) in columns.iter().zip(headings).zip(cells.iter().map(String::as_str).chain(std::iter::repeat(""))) {
            if let Some(field) = field {
                // A text cell keeps a leading line break: an untitled
                // section is written as an empty first line, then its text
                // (`import_row` drops it for every other kind of row).
                row.cells[field.index()] = match field {
                    Field::Text => undefused(cell.trim_start_matches([' ', '\t']).trim_end()).trim_start_matches([' ', '\t']).trim_end(),
                    _ => undefused(cell.trim()).trim(),
                };
                row.headings[field.index()] = heading.trim();
            }
        }
        row
    }

    fn get(&self, field: Field) -> &'a str {
        self.cells[field.index()]
    }

    /// The heading of `field`'s column, to name it in a note.
    fn heading(&self, field: Field) -> String {
        self.headings[field.index()].to_string()
    }
}

/// Whether `text` is in brackets, which marks an answer cell correct.
fn bracketed(text: &str) -> bool {
    text.len() >= 2 && text.starts_with('[') && text.ends_with(']')
}

/// Whether an answer that is not correct would read as correct in its cell:
/// it is in brackets after the `'`s it starts with. Such an answer gets one
/// more `'` before it (`answer_cell`), which the import takes off (`answer`).
fn reads_as_correct(text: &str) -> bool {
    bracketed(text.trim_start_matches('\''))
}

/// The cell of an answer for export: correct answers in brackets.
fn answer_cell(text: &str, correct: bool) -> String {
    // The import trims every cell.
    let text = text.trim();
    if correct {
        format!("[{text}]")
    } else if reads_as_correct(text) {
        format!("'{text}")
    } else {
        text.to_string()
    }
}

/// An answer cell's text and whether brackets mark it correct; none when empty.
fn answer(cell: &str) -> Option<(String, bool)> {
    if cell.is_empty() {
        return None;
    }
    if let Some(text) = cell.strip_prefix('\'').filter(|rest| reads_as_correct(rest)) {
        return Some((text.to_string(), false));
    }
    let text = if bracketed(cell) { &cell[1..cell.len() - 1] } else { cell };
    Some((text.trim().to_string(), bracketed(cell)))
}

fn import_row(row: &Row, categories: &[Category], place: &Place, shuffler: &mut Shuffler, notes: &mut Notes) -> Option<Slide> {
    if row.cells.iter().all(|cell| cell.is_empty()) {
        return None;
    }
    let kind = Kind::of(row, place, notes);
    // Only a section reads a leading line break, as an empty title.
    let raw = row.get(Field::Text);
    let text = raw.trim_start().to_string();
    let answers: Vec<(String, bool)> = (0..MAX_ANSWERS).filter_map(|i| answer(row.get(Field::Answer(i)))).collect();
    let content = match kind {
        Some(Kind::Question) => SlideContent::Question(question(text, answers, None, row, place, shuffler, notes)),
        Some(Kind::FastestFinger) => SlideContent::Question(question(text, answers, Some(AnswerMode::FastestFinger), row, place, shuffler, notes)),
        None if !text.is_empty() || !answers.is_empty() => SlideContent::Question(question(text, answers, None, row, place, shuffler, notes)),
        // A row with only media, such as a video for a full-screen video slide.
        None => SlideContent::Billboard { title: text, text: String::new() },
        Some(other) => {
            if !answers.is_empty() {
                notes.add(ImportIssue::AnswersLeftOut, place);
            }
            match other {
                Kind::Section => billboard(raw),
                Kind::Explanation => SlideContent::Explanation { text },
                Kind::EndOfRound => SlideContent::EndOfRound { title: text },
                _ => SlideContent::Final { title: text },
            }
        }
    };
    let mut slide = Slide::new("", content);
    slide.notes = row.get(Field::Notes).to_string();
    let category = row.get(Field::Category);
    if !category.is_empty() {
        match category_named(categories, category) {
            Some(found) => slide.category = Some(found.id.clone()),
            None => notes.add(ImportIssue::UnknownCategory { name: category.to_string() }, place),
        }
    }
    import_media(row, &mut slide, place, notes);
    fit(&mut slide, row, place, notes);
    Some(slide)
}

/// A billboard's title is the first line, its text the rest.
fn billboard(text: &str) -> SlideContent {
    let (title, rest) = text.split_once('\n').unwrap_or((text, ""));
    SlideContent::Billboard { title: title.trim_end().to_string(), text: rest.to_string() }
}

/// A question; `asked` is the fastest finger when the Type column says so,
/// else the answers decide (`mode_for`).
fn question(
    text: String,
    answers: Vec<(String, bool)>,
    asked: Option<AnswerMode>,
    row: &Row,
    place: &Place,
    shuffler: &mut Shuffler,
    notes: &mut Notes,
) -> Question {
    // Every answer in brackets is correct.
    let mut correct: Vec<usize> = answers.iter().enumerate().filter(|(_, (_, correct))| *correct).map(|(i, _)| i).collect();
    let mut texts: Vec<String> = answers.into_iter().map(|(text, _)| text).collect();
    // Unmarked, the correct answer is A1 and the answers are shuffled.
    if correct.is_empty() && !texts.is_empty() {
        let order = shuffler.order(texts.len());
        correct = order.iter().position(|&i| i == 0).into_iter().collect();
        texts = order.iter().map(|&i| texts[i].clone()).collect();
    }
    let mode = asked.unwrap_or_else(|| mode_for(texts.len()));
    let mut question = Question {
        text,
        mode,
        answers: texts,
        correct: correct.into_iter().map(CorrectAnswer::full).collect(),
        time_limit_s: DEFAULT_TIME_LIMIT_S,
        points: DEFAULT_POINTS,
        points_end: None,
        penalty: 0,
        read_as: row.get(Field::ReadAs).to_string(),
    };
    let mut number = |field: Field, range: RangeInclusive<i64>| {
        let cell = row.get(field);
        let value = integer(cell).filter(|n| range.contains(n));
        if value.is_none() && !cell.is_empty() {
            notes.add(ImportIssue::BadValue { column: row.heading(field), value: cell.to_string() }, place);
        }
        value
    };
    let points = -i64::from(MAX_POINTS)..=i64::from(MAX_POINTS);
    // Every value is in its field's range, as `number` checked.
    if let Some(seconds) = number(Field::AnswerTime, 0..=i64::from(MAX_TIME_LIMIT_S)) {
        question.time_limit_s = seconds as u32;
    }
    if let Some(start) = number(Field::PointsStart, points.clone()) {
        question.points = start as i32;
    }
    question.points_end = number(Field::PointsEnd, points.clone()).map(|end| end as i32);
    // A penalty is points taken, whichever sign the sheet gives it.
    if let Some(penalty) = number(Field::Penalty, points) {
        question.penalty = penalty.abs() as i32;
    }
    question
}

/// How a question without the fastest finger in its Type column is
/// answered: by everyone on the A-D buttons when it has two to four
/// answers, else by the fastest finger, aloud when it has none or more
/// than four (`Button::buzzes_in`).
fn mode_for(answers: usize) -> AnswerMode {
    let everyone = AnswerMode::Everyone.answer_limits();
    if (everyone.fewest..=everyone.most).contains(&answers) { AnswerMode::Everyone } else { AnswerMode::FastestFinger }
}

/// A media cell: the file, and settings in braces.
struct MediaCell<'a> {
    file: &'a str,
    settings: Vec<(String, &'a str)>,
}

fn media_cell<'a>(row: &Row<'a>, field: Field, kinds: &[MediaKind], place: &Place, notes: &mut Notes) -> Option<MediaCell<'a>> {
    let mut file = row.get(field);
    let mut settings = Vec::new();
    if let Some((name, braced)) = file.strip_suffix('}').and_then(|body| body.rsplit_once('{')) {
        file = name.trim_end();
        for setting in braced.split(';').map(str::trim).filter(|setting| !setting.is_empty()) {
            let (key, value) = setting.split_once('=').unwrap_or((setting, ""));
            settings.push((key.trim().to_ascii_lowercase(), value.trim()));
        }
    }
    if file.is_empty() {
        return None;
    }
    if !MediaKind::of(file).is_some_and(|kind| kinds.contains(&kind)) {
        notes.add(ImportIssue::WrongMediaType { column: row.heading(field), file: file.to_string() }, place);
        return None;
    }
    Some(MediaCell { file, settings })
}

fn import_media(row: &Row, slide: &mut Slide, place: &Place, notes: &mut Notes) {
    use MediaKind::{Audio, Image, Video};
    let mut cell = |field: Field, kinds: &[MediaKind]| media_cell(row, field, kinds, place, notes);
    let picture = cell(Field::Picture, &[Image]);
    let sound = cell(Field::Sound, &[Audio]);
    let video = cell(Field::Video, &[Video]);
    let mut background = cell(Field::Background, &[Image, Video]).map(|cell| (Field::Background, cell));
    if let Some(moving) = cell(Field::BackgroundVideo, &[Video]) {
        if background.is_some() {
            notes.add(ImportIssue::TwoBackgrounds, place);
        }
        background = Some((Field::BackgroundVideo, moving));
    }

    if let Some(cell) = picture {
        apply_settings(&mut (), &cell, set_nothing, row.heading(Field::Picture), place, notes);
        slide.picture = Some(cell.file.to_string());
    }
    if let Some(cell) = sound {
        let mut sound = SoundClip::new(cell.file);
        apply_settings(&mut sound, &cell, set_sound, row.heading(Field::Sound), place, notes);
        keep_parts(&mut sound.part, &mut sound.replay, row.heading(Field::Sound), place, notes);
        slide.sound = Some(sound);
    }
    if let Some(cell) = video {
        let mut video = sheet_video(cell.file);
        apply_settings(&mut video, &cell, set_video, row.heading(Field::Video), place, notes);
        keep_parts(&mut video.part, &mut video.replay, row.heading(Field::Video), place, notes);
        slide.video = Some(video);
    }
    if let Some((field, cell)) = background {
        let mut background = Background::new(cell.file);
        apply_settings(&mut background, &cell, set_background, row.heading(field), place, notes);
        slide.background = Some(background);
    }
}

/// What became of one setting.
enum Setting {
    Set,
    Bad,
    NotUsed,
    Unknown,
}

impl Setting {
    /// Set when `value` was read (and given to `set`), bad otherwise.
    fn from<T>(value: Option<T>, set: impl FnOnce(T)) -> Setting {
        value.map_or(Setting::Bad, |value| {
            set(value);
            Setting::Set
        })
    }

    /// A key that `target` has no use for.
    fn other(key: &str) -> Setting {
        if SETTINGS.contains(&key) {
            Setting::NotUsed
        } else {
            Setting::Unknown
        }
    }
}

fn apply_settings<T>(target: &mut T, cell: &MediaCell, set: fn(&mut T, &str, &str) -> Setting, heading: String, place: &Place, notes: &mut Notes) {
    for (key, value) in &cell.settings {
        let (column, setting) = (heading.clone(), key.clone());
        let issue = match set(target, key, value) {
            Setting::Set => continue,
            Setting::Bad => ImportIssue::BadSetting { column, setting, value: value.to_string() },
            Setting::NotUsed => ImportIssue::SettingNotUsed { column, setting },
            Setting::Unknown => ImportIssue::UnknownSetting { column, setting },
        };
        notes.add(issue, place);
    }
}

/// Undoes a stop before the start: the whole file plays from the start.
fn keep_parts(part: &mut Span, replay: &mut Option<Replay>, heading: String, place: &Place, notes: &mut Notes) {
    let parts = [(Some(part), STOP_AT), (replay.as_mut().map(|replay| &mut replay.part), REPLAY_STOP_AT)];
    for (part, setting) in parts {
        let Some(part) = part.filter(|part| part.is_empty()) else { continue };
        let value = part.to_ms.map(clock).unwrap_or_default();
        notes.add(ImportIssue::BadSetting { column: heading.clone(), setting: setting.to_string(), value }, place);
        part.to_ms = None;
    }
}

/// What sounds and videos have in common (see `Playback`), to change.
struct PlaybackParts<'a> {
    repeat: &'a mut bool,
    volume: &'a mut u8,
    part: &'a mut Span,
    replay: &'a mut Option<Replay>,
}

/// Sets a setting that sounds and videos share; none for any other key.
fn set_playback(target: PlaybackParts, key: &str, value: &str) -> Option<Setting> {
    let replay = target.replay;
    let new_replay = || Replay { part: Span::default(), delay_ms: 0 };
    Some(match key {
        START_AT => Setting::from(clock_ms(value), |ms| target.part.from_ms = ms),
        STOP_AT => Setting::from(clock_ms(value), |ms| target.part.to_ms = Some(ms)),
        LOOP => Setting::from(flag(value), |on| *target.repeat = on),
        VOLUME => Setting::from(within(value, 0..=100), |volume| *target.volume = volume),
        REPLAY => Setting::from(flag(value), |on| *replay = on.then(|| replay.unwrap_or_else(new_replay))),
        REPLAY_START_AT => Setting::from(clock_ms(value), |ms| replay.get_or_insert_with(new_replay).part.from_ms = ms),
        REPLAY_STOP_AT => Setting::from(clock_ms(value), |ms| replay.get_or_insert_with(new_replay).part.to_ms = Some(ms)),
        REPLAY_DELAY => Setting::from(seconds_ms(value, MAX_REPLAY_DELAY_MS), |ms| replay.get_or_insert_with(new_replay).delay_ms = ms),
        _ => return None,
    })
}

fn set_nothing(_: &mut (), key: &str, _: &str) -> Setting {
    Setting::other(key)
}

fn set_sound(sound: &mut SoundClip, key: &str, value: &str) -> Setting {
    let parts = PlaybackParts { repeat: &mut sound.repeat, volume: &mut sound.volume, part: &mut sound.part, replay: &mut sound.replay };
    if let Some(done) = set_playback(parts, key, value) {
        return done;
    }
    match key {
        PLAYS => Setting::from(sound_start(value), |start| sound.start = start),
        GAIN => Setting::from(decimal(value).filter(|db| (f64::from(MIN_GAIN_DB)..=f64::from(MAX_GAIN_DB)).contains(db)), |db| {
            sound.gain_db = db as f32;
        }),
        _ => Setting::other(key),
    }
}

fn set_video(video: &mut VideoClip, key: &str, value: &str) -> Setting {
    let parts = PlaybackParts { repeat: &mut video.repeat, volume: &mut video.volume, part: &mut video.part, replay: &mut video.replay };
    if let Some(done) = set_playback(parts, key, value) {
        return done;
    }
    match key {
        SILENT => Setting::from(flag(value), |on| video.muted = on),
        SPEED => Setting::from(within(value, i64::from(MIN_VIDEO_SPEED)..=i64::from(MAX_VIDEO_SPEED)), |speed| video.speed = speed),
        EFFECT => Setting::from(video_effect(value), |effect| video.effect = effect),
        INTRO_PAUSE => Setting::from(seconds_ms(value, MAX_INTRO_PAUSE_MS), |ms| video.intro_pause_ms = ms),
        AUTOPLAY => Setting::from(flag(value), |on| video.autoplay = on),
        _ => Setting::other(key),
    }
}

fn set_background(background: &mut Background, key: &str, value: &str) -> Setting {
    match key {
        LOOP => Setting::from(flag(value), |on| background.repeat = on),
        SILENT => Setting::from(flag(value), |on| background.muted = on),
        SYNC_WITH_QUIZ => Setting::from(flag(value), |on| background.sync = on),
        _ => Setting::other(key),
    }
}

/// Drops what a slide of its type or its media cannot have, with a note.
fn fit(slide: &mut Slide, row: &Row, place: &Place, notes: &mut Notes) {
    if slide.picture.is_some() && slide.video.is_some() {
        notes.add(ImportIssue::PictureAndVideo, place);
        slide.picture = None;
    }
    let plays = slide.content.slide_type().plays();
    if !plays.picture_and_video && (slide.picture.is_some() || slide.video.is_some()) {
        notes.add(ImportIssue::MediaOnRanking, place);
        (slide.picture, slide.video) = (None, None);
    }
    if plays.clock_and_reveal {
        return;
    }
    let mut not_used = |field: Field, setting: &str| {
        notes.add(ImportIssue::SettingNotUsed { column: row.heading(field), setting: setting.to_string() }, place);
    };
    // Only a question has a clock and a reveal (`SlideType::plays`).
    if let Some(sound) = &mut slide.sound {
        if sound.start == SoundStart::WithClock {
            sound.start = SoundStart::OnSlide;
            not_used(Field::Sound, PLAYS);
        }
        if sound.replay.take().is_some() {
            not_used(Field::Sound, REPLAY);
        }
    }
    if let Some(video) = &mut slide.video {
        if video.replay.take().is_some() {
            not_used(Field::Video, REPLAY);
        }
        if video.effect.take().is_some() {
            not_used(Field::Video, EFFECT);
        }
    }
}

/// A whole number, also as Excel may write one (`10.0`, `10,0`).
fn integer(value: &str) -> Option<i64> {
    value.parse().ok().or_else(|| decimal(value).filter(|n| n.fract() == 0.0 && n.abs() < 1e15).map(|n| n as i64))
}

/// A whole number in `range`, as the type it is stored in.
fn within<T: TryFrom<i64>>(value: &str, range: RangeInclusive<i64>) -> Option<T> {
    integer(value).filter(|n| range.contains(n)).and_then(|n| T::try_from(n).ok())
}

/// A finite number, with a decimal point or comma.
fn decimal(value: &str) -> Option<f64> {
    value.replace(',', ".").parse::<f64>().ok().filter(|n| n.is_finite())
}

fn flag(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" => Some(true),
        "false" | "no" | "0" => Some(false),
        _ => None,
    }
}

/// Seconds (`1.5`) as milliseconds, up to `max_ms`.
fn seconds_ms(value: &str, max_ms: u32) -> Option<u32> {
    let ms = (decimal(value)? * 1000.0).round();
    (0.0..=f64::from(max_ms)).contains(&ms).then_some(ms as u32)
}

/// A time as `hh:mm:ss`, `mm:ss` or seconds, with a fraction (`00:01:15.5`), as milliseconds.
fn clock_ms(value: &str) -> Option<u32> {
    let parts: Vec<&str> = value.split(':').collect();
    let (seconds, whole) = parts.split_last()?;
    if whole.len() > 2 {
        return None;
    }
    let mut total = decimal(seconds).filter(|s| *s >= 0.0)?;
    for (part, unit) in whole.iter().rev().zip([60.0, 3600.0]) {
        total += f64::from(part.trim().parse::<u32>().ok()?) * unit;
    }
    let ms = (total * 1000.0).round();
    (ms <= f64::from(u32::MAX)).then_some(ms as u32)
}

fn video_effect(value: &str) -> Option<Option<VideoEffect>> {
    match value.to_ascii_lowercase().as_str() {
        "blur" => Some(Some(VideoEffect::Blur)),
        "mosaic" => Some(Some(VideoEffect::Mosaic)),
        "none" => Some(None),
        _ => None,
    }
}

const ON_SLIDE: &str = "on-slide";
const WITH_CLOCK: &str = "with-clock";
const ON_DEMAND: &str = "on-demand";

fn sound_start(value: &str) -> Option<SoundStart> {
    match value.to_ascii_lowercase().as_str() {
        ON_SLIDE => Some(SoundStart::OnSlide),
        WITH_CLOCK => Some(SoundStart::WithClock),
        ON_DEMAND => Some(SoundStart::OnDemand),
        _ => None,
    }
}

/// Milliseconds as `hh:mm:ss`, with the milliseconds when there are some.
fn clock(ms: u32) -> String {
    let (seconds, fraction) = (ms / 1000, ms % 1000);
    let time = format!("{:02}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60);
    if fraction == 0 {
        time
    } else {
        format!("{time}.{fraction:03}")
    }
}

/// Milliseconds as seconds (`1.5`).
fn seconds(ms: u32) -> String {
    if ms.is_multiple_of(1000) {
        (ms / 1000).to_string()
    } else {
        format!("{}.{:03}", ms / 1000, ms % 1000).trim_end_matches('0').to_string()
    }
}

/// The quiz's slides as a table: the headings in `language`, then a row
/// per slide, in the basic columns and the others a slide has a value in.
/// `file_name` gives the name a media reference has next to the table;
/// none leaves the file out.
pub fn export_table(quiz: &Quiz, language: QuizLanguage, file_name: impl Fn(&str) -> Option<String>) -> Vec<Vec<String>> {
    let rows: Vec<Vec<String>> = quiz.slides.iter().map(|slide| export_row(slide, &quiz.categories, language, &file_name)).collect();
    let kept: Vec<usize> = (0..FIELDS.len()).filter(|&i| FIELDS[i].basic || rows.iter().any(|row| !row[i].is_empty())).collect();
    let headings = kept.iter().map(|&i| FIELDS[i].heading(language).to_string()).collect();
    let rows = rows.into_iter().map(|row| kept.iter().map(|&i| defused(row[i].clone())).collect());
    std::iter::once(headings).chain(rows).collect()
}

/// Whether a spreadsheet program would take `cell` for a formula, which can
/// run commands when the sheet is opened: it starts with `=`, `@`, a tab, a
/// carriage return, or `+` or `-` not followed by just a number.
fn looks_like_formula(cell: &str) -> bool {
    let mut chars = cell.chars();
    match chars.next() {
        Some('=' | '@' | '\t' | '\r') => true,
        Some('+' | '-') => {
            let rest = chars.as_str();
            !rest.starts_with(|c: char| c.is_ascii_digit()) || !rest.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',')
        }
        _ => false,
    }
}

/// Whether an export puts a `'` before `cell`: it looks like a formula, or
/// would after the `'`s it starts with, so the import knows which to take off.
fn needs_quote(cell: &str) -> bool {
    looks_like_formula(cell.trim_start_matches('\''))
}

/// A cell for export: one that looks like a formula gets a leading `'`, so
/// a spreadsheet program shows it as text; the import takes it off again.
pub fn defused(cell: String) -> String {
    if needs_quote(&cell) { format!("'{cell}") } else { cell }
}

/// A cell as `defused` wrote it, without the `'`.
fn undefused(cell: &str) -> &str {
    match cell.strip_prefix('\'') {
        Some(rest) if needs_quote(rest) => rest,
        _ => cell,
    }
}

/// The Type cell of a question: empty where the import reads the row as
/// this question without one.
fn question_kind(question: &Question) -> Option<Kind> {
    if question.mode != mode_for(question.answers.len().min(MAX_ANSWERS)) {
        return Some(Kind::FastestFinger);
    }
    // A row with no text and no answers would be a billboard, or nothing.
    (question.text.trim().is_empty() && question.answers.is_empty()).then_some(Kind::Question)
}

fn export_row(slide: &Slide, categories: &[Category], language: QuizLanguage, file_name: &impl Fn(&str) -> Option<String>) -> Vec<String> {
    let mut row = vec![String::new(); FIELDS.len()];
    let mut set = |field: Field, value: String| row[field.index()] = value;
    let mut kind = |kind: Kind, text: String| {
        set(Field::Type, kind.name(language).to_string());
        set(Field::Text, text);
    };
    match &slide.content {
        SlideContent::Question(question) => {
            match question_kind(question) {
                Some(asked) => kind(asked, question.text.clone()),
                None => set(Field::Text, question.text.clone()),
            }
            for (i, answer) in question.answers.iter().enumerate().take(MAX_ANSWERS) {
                // The sheet has no share of the points: a partly correct answer is marked as correct.
                set(Field::Answer(i), answer_cell(answer, question.is_correct(i)));
            }
            // An empty cell is the default, which keeps the sheet short to read.
            let unless = |value: i64, default: i64| if value == default { String::new() } else { value.to_string() };
            set(Field::AnswerTime, unless(question.time_limit_s.into(), DEFAULT_TIME_LIMIT_S.into()));
            set(Field::PointsStart, unless(question.points.into(), DEFAULT_POINTS.into()));
            set(Field::PointsEnd, question.points_end.map(|end| end.to_string()).unwrap_or_default());
            set(Field::Penalty, unless(question.penalty.into(), 0));
            set(Field::ReadAs, question.read_as.clone());
        }
        SlideContent::Billboard { title, text } if text.is_empty() => kind(Kind::Section, title.clone()),
        SlideContent::Billboard { title, text } => kind(Kind::Section, format!("{title}\n{text}")),
        SlideContent::EndOfRound { title } => kind(Kind::EndOfRound, title.clone()),
        SlideContent::Explanation { text } => kind(Kind::Explanation, text.clone()),
        SlideContent::Final { title } => kind(Kind::Results, title.clone()),
    }
    set(Field::Notes, slide.notes.clone());
    if let Some(category) = categories.iter().find(|category| slide.category.as_ref() == Some(&category.id)) {
        set(Field::Category, category.name.clone());
    }
    let mut media = |field: Field, file: &str, settings: Vec<(&str, String)>| {
        let Some(name) = file_name(file) else { return };
        let settings: Vec<String> = settings.into_iter().map(|(key, value)| format!("{key}={value}")).collect();
        set(field, if settings.is_empty() { name } else { format!("{name}{{{}}}", settings.join(";")) });
    };
    if let Some(picture) = &slide.picture {
        media(Field::Picture, picture, Vec::new());
    }
    if let Some(sound) = &slide.sound {
        media(Field::Sound, &sound.file, sound_settings(sound));
    }
    if let Some(video) = &slide.video {
        media(Field::Video, &video.file, video_settings(video));
    }
    if let Some(background) = &slide.background {
        match MediaKind::of(&background.file) {
            Some(MediaKind::Video) => media(Field::BackgroundVideo, &background.file, background_settings(background)),
            _ => media(Field::Background, &background.file, Vec::new()),
        }
    }
    row
}

/// The settings shared by sounds and videos that differ from `default`'s.
fn playback_settings(settings: &mut Vec<(&'static str, String)>, it: (bool, u8, &Span, Option<&Replay>), default: (bool, u8)) {
    let (repeat, volume, part, replay) = it;
    if part.from_ms != 0 {
        settings.push((START_AT, clock(part.from_ms)));
    }
    if let Some(to) = part.to_ms {
        settings.push((STOP_AT, clock(to)));
    }
    if repeat != default.0 {
        settings.push((LOOP, repeat.to_string()));
    }
    if volume != default.1 {
        settings.push((VOLUME, volume.to_string()));
    }
    if let Some(replay) = replay {
        settings.push((REPLAY, true.to_string()));
        if replay.part.from_ms != 0 {
            settings.push((REPLAY_START_AT, clock(replay.part.from_ms)));
        }
        if let Some(to) = replay.part.to_ms {
            settings.push((REPLAY_STOP_AT, clock(to)));
        }
        if replay.delay_ms != 0 {
            settings.push((REPLAY_DELAY, seconds(replay.delay_ms)));
        }
    }
}

fn sound_settings(sound: &SoundClip) -> Vec<(&'static str, String)> {
    let default = SoundClip::new("");
    let mut settings = Vec::new();
    if sound.start != default.start {
        let start = match sound.start {
            SoundStart::OnSlide => ON_SLIDE,
            SoundStart::WithClock => WITH_CLOCK,
            SoundStart::OnDemand => ON_DEMAND,
        };
        settings.push((PLAYS, start.to_string()));
    }
    playback_settings(&mut settings, (sound.repeat, sound.volume, &sound.part, sound.replay.as_ref()), (default.repeat, default.volume));
    if sound.gain_db != default.gain_db {
        settings.push((GAIN, sound.gain_db.to_string()));
    }
    settings
}

fn video_settings(video: &VideoClip) -> Vec<(&'static str, String)> {
    let default = sheet_video("");
    let mut settings = Vec::new();
    playback_settings(&mut settings, (video.repeat, video.volume, &video.part, video.replay.as_ref()), (default.repeat, default.volume));
    if video.muted != default.muted {
        settings.push((SILENT, video.muted.to_string()));
    }
    if video.speed != default.speed {
        settings.push((SPEED, video.speed.to_string()));
    }
    if let Some(effect) = video.effect {
        let name = match effect {
            VideoEffect::Blur => "Blur",
            VideoEffect::Mosaic => "Mosaic",
        };
        settings.push((EFFECT, name.to_string()));
    }
    if video.intro_pause_ms != default.intro_pause_ms {
        settings.push((INTRO_PAUSE, seconds(video.intro_pause_ms)));
    }
    if video.autoplay != default.autoplay {
        settings.push((AUTOPLAY, video.autoplay.to_string()));
    }
    settings
}

fn background_settings(background: &Background) -> Vec<(&'static str, String)> {
    let default = Background::new("");
    let mut settings = Vec::new();
    if background.repeat != default.repeat {
        settings.push((LOOP, background.repeat.to_string()));
    }
    if background.muted != default.muted {
        settings.push((SILENT, background.muted.to_string()));
    }
    if background.sync != default.sync {
        settings.push((SYNC_WITH_QUIZ, background.sync.to_string()));
    }
    settings
}

/// SplitMix64 (Steele, Lea and Flood, "Fast Splittable Pseudorandom Number
/// Generators", OOPSLA 2014; constants from https://prng.di.unimi.it/splitmix64.c):
/// enough to shuffle answers, and seeded by the caller, so the core stays
/// free of I/O and its tests repeatable.
struct Shuffler(u64);

impl Shuffler {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A random order of `n` items (Fisher-Yates); the bias of the modulo is
    /// negligible for a few answers.
    fn order(&mut self, n: usize) -> Vec<usize> {
        let mut order: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            order.swap(i, j);
        }
        order
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(name: &str, rows: &[&[&str]]) -> Table {
        Table { name: name.into(), first_row: 1, rows: rows.iter().map(|row| row.iter().map(|cell| cell.to_string()).collect()).collect() }
    }

    fn import(rows: &[&[&str]]) -> ImportedRows {
        import_tables(&[table("", rows)], &[], 7).unwrap()
    }

    fn slides(imported: &ImportedRows) -> Vec<&Slide> {
        imported.slides.iter().map(|(_, slide)| slide).collect()
    }

    fn issues(imported: &ImportedRows) -> Vec<&ImportIssue> {
        imported.notes.0.iter().map(|note| &note.issue).collect()
    }

    fn question(slide: &Slide) -> &Question {
        match &slide.content {
            SlideContent::Question(question) => question,
            other => panic!("not a question: {other:?}"),
        }
    }

    /// The indexes of a question's correct answers.
    fn correct(question: &Question) -> Vec<usize> {
        question.correct.iter().map(|c| c.answer).collect()
    }

    /// The cell of an exported table's row in `field`'s column; empty when the export left the column out.
    fn cell(rows: &[Vec<String>], row: usize, field: Field) -> &str {
        let column = &FIELDS[field.index()];
        rows[0].iter().position(|heading| heading == column.english || heading == column.czech).map_or("", |i| rows[row][i].as_str())
    }

    fn at(row: u32) -> Place {
        Place { sheet: String::new(), row }
    }

    #[test]
    fn a_row_becomes_a_question() {
        let imported = import(&[
            &["Question", "Answer 1", "Answer 2", "Answer 3", "Answer 4", "Notes", "Picture", "Sound"],
            &["Capital of France?", "Paris", "Lyon", "Nice", "Lille", "Easy one", "france.png", "anthem.mp3"],
            &[],
            &["Name the song", "", "", "", "", "", "", "song.mp3"],
            &["Which is a fruit?", "Carrot", "[Apple]", "Leek"],
        ]);
        let found = slides(&imported);
        assert_eq!(found.len(), 3, "the empty row is skipped");
        let capital = question(found[0]);
        assert_eq!(capital.mode, AnswerMode::Everyone);
        assert_eq!(capital.answers.len(), 4);
        // The first is correct and the answers are shuffled; the correct one follows Paris.
        assert_eq!(correct(capital).iter().map(|&i| capital.answers[i].as_str()).collect::<Vec<_>>(), ["Paris"]);
        assert_eq!((found[0].notes.as_str(), found[0].picture.as_deref()), ("Easy one", Some("france.png")));
        assert_eq!(found[0].sound, Some(SoundClip::new("anthem.mp3")));
        assert_eq!((capital.time_limit_s, capital.points, capital.points_end, capital.penalty), (DEFAULT_TIME_LIMIT_S, DEFAULT_POINTS, None, 0));

        // No answers: an open question, answered aloud.
        let song = question(found[1]);
        assert_eq!((song.mode, song.answers.len(), correct(song)), (AnswerMode::FastestFinger, 0, vec![]));
        // A marked answer keeps the order.
        assert_eq!(question(found[2]).answers, ["Carrot", "Apple", "Leek"]);
        assert_eq!(correct(question(found[2])), [1]);
        assert_eq!(imported.slides.iter().map(|(place, _)| place.row).collect::<Vec<_>>(), [2, 4, 5]);
        // Every answer in brackets is correct, in its place.
        let several = slides(&import(&[&["Question", "Answer 1", "Answer 2", "Answer 3"], &["Which are fruits?", "[Apple]", "Leek", "[Pear]"]]))[0].clone();
        assert_eq!((question(&several).answers.as_slice(), correct(question(&several))), (&["Apple".to_string(), "Leek".into(), "Pear".into()][..], vec![0, 2]));
        assert_eq!(found.iter().map(|slide| slide.id.as_str()).collect::<Vec<_>>(), ["import-1", "import-2", "import-3"]);
        assert!(imported.notes.0.is_empty());
    }

    #[test]
    fn shuffled_answers_keep_the_correct_one_with_its_text() {
        let rows: &[&[&str]] = &[&["Question", "Answer 1", "Answer 2", "Answer 3", "Answer 4"], &["?", "right", "w1", "w2", "w3"]];
        let mut orders = std::collections::HashSet::new();
        for seed in 0..200 {
            let imported = import_tables(&[table("", rows)], &[], seed).unwrap();
            let question = question(&imported.slides[0].1);
            assert_eq!(correct(question).iter().map(|&i| question.answers[i].as_str()).collect::<Vec<_>>(), ["right"]);
            orders.insert(question.answers.clone());
        }
        assert_eq!(orders.len(), 24, "every order of four answers comes up");
    }

    #[test]
    fn headings_are_read_whatever_their_case_and_spaces() {
        let imported = import(&[&[" question ", "ANSWER-1", "answer2"], &["Yes or no?", "[Yes]", "No"]]);
        assert_eq!(question(slides(&imported)[0]).answers, ["Yes", "No"]);
    }

    #[test]
    fn only_a_section_reads_a_leading_line_break_as_an_empty_title() {
        let imported = import(&[
            &["Type", "Question", "Answer 1", "Answer 2"],
            &["Section", "\nText without a title"],
            &["", "\nCapital of France?", "[Paris]", "Lyon"],
            &["End of round", "\nEnd of round 1"],
        ]);
        let found = slides(&imported);
        assert_eq!(found[0].content, SlideContent::Billboard { title: String::new(), text: "Text without a title".into() });
        assert_eq!(question(found[1]).text, "Capital of France?");
        assert_eq!(found[2].content, SlideContent::EndOfRound { title: "End of round 1".into() });
    }

    #[test]
    fn the_type_column_gives_the_slide_type_in_either_language() {
        let imported = import(&[
            &["Type", "Question", "Answer 1", "Answer 2", "Answer 3", "Answer 4", "Answer 5"],
            &["Section", "Round 1\nTen questions about films"],
            &["sekce", "Kolo 2"],
            &["Fastest finger", "Who directed Alien?", "[Ridley Scott]", "James Cameron"],
            &["RYCHLA ODPOVED", "Name a director"],
            &["", "Pick five", "a", "b", "c", "d", "e"],
            &["Question", ""],
            &["End of round", "End of round 1", "stray"],
            &["Vysvětlení", "Ridley Scott also directed Gladiator."],
            &["Výsledky", "And the winner is"],
            &["Bonus", "{B}Old prefixes are text", "[a]", "b"],
        ]);
        let found = slides(&imported);
        let contents: Vec<&SlideContent> = found.iter().map(|slide| &slide.content).collect();
        assert_eq!(contents[0], &SlideContent::Billboard { title: "Round 1".into(), text: "Ten questions about films".into() });
        assert_eq!(contents[1], &SlideContent::Billboard { title: "Kolo 2".into(), text: String::new() });
        let ff = question(found[2]);
        assert_eq!((ff.mode, ff.answers.len(), correct(ff)), (AnswerMode::FastestFinger, 2, vec![0]));
        assert_eq!(question(found[3]).mode, AnswerMode::FastestFinger);
        assert_eq!(question(found[4]).mode, AnswerMode::FastestFinger, "five answers do not fit A-D");
        assert_eq!(question(found[5]).text, "", "a question with no text yet");
        assert_eq!(contents[6], &SlideContent::EndOfRound { title: "End of round 1".into() });
        assert_eq!(contents[7], &SlideContent::Explanation { text: "Ridley Scott also directed Gladiator.".into() });
        assert_eq!(contents[8], &SlideContent::Final { title: "And the winner is".into() });
        let unknown = question(found[9]);
        assert_eq!((unknown.text.as_str(), unknown.mode), ("{B}Old prefixes are text", AnswerMode::Everyone));
        assert_eq!(issues(&imported), [&ImportIssue::AnswersLeftOut, &ImportIssue::UnknownType { value: "Bonus".into() }]);
    }

    #[test]
    fn a_row_with_only_a_video_is_a_full_screen_video_slide() {
        let imported = import(&[&["Question", "Video"], &["", "intro.mp4"]]);
        let slide = slides(&imported)[0];
        assert_eq!(slide.content, SlideContent::Billboard { title: String::new(), text: String::new() });
        assert_eq!(slide.video, Some(sheet_video("intro.mp4")));
        assert_eq!(slide.video.as_ref().unwrap().intro_pause_ms, NEW_VIDEO_INTRO_PAUSE_MS);
    }

    #[test]
    fn the_old_short_headings_are_not_read() {
        let old: &[&[&str]] = &[&["Q", "A1", "A2", "NOTES"], &["{Q, FF}Capital?", "[Paris]", "Lyon", "Easy"]];
        assert_eq!(import_tables(&[table("", old)], &[], 1), Err(SheetError::NoQuestionColumn));
        let mixed = import(&[&["Question", "A1", "Answer 2"], &["Capital?", "[Paris]", "Lyon"]]);
        assert_eq!(question(slides(&mixed)[0]).answers, ["Lyon"]);
        assert_eq!(issues(&mixed), [&ImportIssue::UnknownColumn { column: "A1".into() }]);
    }

    #[test]
    fn scoring_columns_set_the_question_and_bad_values_keep_the_default() {
        let imported = import(&[
            &["Question", "Answer time", "Points", "Points at end", "Penalty"],
            &["A", "30", "100", "50.0", "-5"],
            &["B", "soon", "1000000", "", "5"],
        ]);
        let found = slides(&imported);
        let a = question(found[0]);
        assert_eq!((a.time_limit_s, a.points, a.points_end, a.penalty), (30, 100, Some(50), 5));
        let b = question(found[1]);
        assert_eq!((b.time_limit_s, b.points, b.points_end, b.penalty), (DEFAULT_TIME_LIMIT_S, DEFAULT_POINTS, None, 5));
        assert_eq!(
            issues(&imported),
            [
                &ImportIssue::BadValue { column: "Answer time".into(), value: "soon".into() },
                &ImportIssue::BadValue { column: "Points".into(), value: "1000000".into() },
            ]
        );
    }

    #[test]
    fn media_settings_in_braces_set_the_options() {
        let imported = import(&[
            &["Question", "Video", "Background video", "Sound"],
            &[
                "Watch",
                "clips/intro.mp4{loop=false; start-at=00:00:05;stop-at=0:20;silent=true;volume=40;speed=50;effect=Mosaic;intro-pause=2,5}",
                "waves.webm{sync-with-quiz=true;loop=false}",
                "hint.mp3{plays=with-clock;gain=-4.5;replay=true;replay-start-at=75;replay-delay=1.5}",
            ],
        ]);
        let slide = slides(&imported)[0];
        let video = slide.video.as_ref().unwrap();
        assert_eq!(video.file, "clips/intro.mp4");
        assert_eq!((video.repeat, video.muted, video.volume, video.speed, video.effect), (false, true, 40, 50, Some(VideoEffect::Mosaic)));
        assert_eq!((video.part, video.intro_pause_ms), (Span { from_ms: 5_000, to_ms: Some(20_000) }, 2_500));
        assert_eq!(slide.background, Some(Background { file: "waves.webm".into(), repeat: false, muted: true, sync: true }));
        let sound = slide.sound.as_ref().unwrap();
        assert_eq!((sound.start, sound.gain_db), (SoundStart::WithClock, -4.5));
        assert_eq!(sound.replay, Some(Replay { part: Span { from_ms: 75_000, to_ms: None }, delay_ms: 1_500 }));
        assert!(imported.notes.0.is_empty(), "{:?}", imported.notes);
    }

    #[test]
    fn settings_that_do_not_apply_or_are_wrong_are_noted() {
        let imported = import(&[
            &["Question", "Video", "Background video", "Sound"],
            &["", "a.mp4{speed=500;sync-with-quiz=true;colour=red;effect=Blur;stop-at=1:00;start-at=2:00}", "b.mp4{intro-pause=2}", "c.mp3{plays=with-clock}"],
        ]);
        let slide = slides(&imported)[0];
        let video = slide.video.as_ref().unwrap();
        assert_eq!((video.speed, video.effect, video.part), (100, None, Span { from_ms: 120_000, to_ms: None }));
        assert_eq!(slide.sound.as_ref().unwrap().start, SoundStart::OnSlide);
        let column = |name: &str| name.to_string();
        assert_eq!(
            issues(&imported),
            [
                &ImportIssue::BadSetting { column: column("Video"), setting: "speed".into(), value: "500".into() },
                &ImportIssue::SettingNotUsed { column: column("Video"), setting: "sync-with-quiz".into() },
                &ImportIssue::UnknownSetting { column: column("Video"), setting: "colour".into() },
                &ImportIssue::BadSetting { column: column("Video"), setting: "stop-at".into(), value: "00:01:00".into() },
                &ImportIssue::SettingNotUsed { column: column("Background video"), setting: "intro-pause".into() },
                // A billboard has no clock and no answer time.
                &ImportIssue::SettingNotUsed { column: column("Sound"), setting: "plays".into() },
                &ImportIssue::SettingNotUsed { column: column("Video"), setting: "effect".into() },
            ]
        );
    }

    #[test]
    fn media_a_slide_cannot_have_are_left_out() {
        let imported = import(&[
            &["Type", "Question", "Picture", "Video", "Background", "Background video", "Sound"],
            &["", "Both", "a.png", "b.mp4", "c.jpg", "d.mp4", "e.wmv"],
            &["Results", "Winners", "a.png", "", "c.jpg", "", "fanfare.mp3"],
        ]);
        let found = slides(&imported);
        assert_eq!((found[0].picture.as_deref(), found[0].background.as_ref().map(|b| b.file.as_str())), (None, Some("d.mp4")));
        assert_eq!(found[0].sound, None);
        assert_eq!((found[1].picture.as_deref(), found[1].background.as_ref().map(|b| b.file.as_str())), (None, Some("c.jpg")));
        assert_eq!(found[1].sound.as_ref().map(|s| s.file.as_str()), Some("fanfare.mp3"));
        assert_eq!(
            issues(&imported),
            [
                &ImportIssue::WrongMediaType { column: "Sound".into(), file: "e.wmv".into() },
                &ImportIssue::TwoBackgrounds,
                &ImportIssue::PictureAndVideo,
                &ImportIssue::MediaOnRanking,
            ]
        );
    }

    #[test]
    fn columns_without_a_place_here_are_noted_once_if_they_hold_values() {
        let imported = import(&[
            &["Question", "P2", "STYLE", "COLOUR", "", "Question", "BONUS-POINTS-1"],
            &["One", "b.png", "", "red", "x", "dup"],
            &["Two", "c.png", "", "", "", "", "", "beyond"],
        ]);
        assert_eq!(question(slides(&imported)[0]).text, "One");
        let headings = at(1);
        assert_eq!(
            imported.notes.0,
            [
                ImportNote { issue: ImportIssue::UnknownColumn { column: "P2".into() }, places: vec![headings.clone()] },
                ImportNote { issue: ImportIssue::UnknownColumn { column: "COLOUR".into() }, places: vec![headings.clone()] },
                ImportNote { issue: ImportIssue::UnknownColumn { column: "E".into() }, places: vec![headings.clone()] },
                ImportNote { issue: ImportIssue::DuplicateColumn { column: "Question".into() }, places: vec![headings.clone()] },
                ImportNote { issue: ImportIssue::UnknownColumn { column: "H".into() }, places: vec![headings] },
            ]
        );
    }

    #[test]
    fn a_row_takes_the_category_it_names_regardless_of_case_and_a_note_for_one_the_quiz_lacks() {
        let categories = [Category::new("c1", "Music"), Category::new("c2", "Film")];
        let rows: &[&[&str]] = &[&["Question", "Category"], &["One", " music "], &["Two", "Sport"], &["Three", ""]];
        let imported = import_tables(&[table("", rows)], &categories, 7).unwrap();
        let found: Vec<Option<&str>> = slides(&imported).iter().map(|slide| slide.category.as_deref()).collect();
        assert_eq!(found, [Some("c1"), None, None]);
        assert_eq!(imported.notes.0, [ImportNote { issue: ImportIssue::UnknownCategory { name: "Sport".into() }, places: vec![at(3)] }]);
    }

    #[test]
    fn notes_list_each_issue_once_with_its_rows() {
        let imported = import(&[&["Type", "Question", "Answer 1"], &["Bonus", "a"], &["Bonus", "b"], &["Joker", "c"], &["Section", "d", "x"]]);
        assert_eq!(
            imported.notes.0,
            [
                ImportNote { issue: ImportIssue::UnknownType { value: "Bonus".into() }, places: vec![at(2), at(3)] },
                ImportNote { issue: ImportIssue::UnknownType { value: "Joker".into() }, places: vec![at(4)] },
                ImportNote { issue: ImportIssue::AnswersLeftOut, places: vec![at(5)] },
            ]
        );
    }

    #[test]
    fn several_sheets_become_rounds_opened_by_their_names() {
        let tables = [
            table("Instructions", &[&["Fill in the questions below"]]),
            table("Round 1", &[&["Question", "Answer 1", "Answer 2"], &["One", "[a]", "b"]]),
            table("Sheet3", &[]),
            table("Round 2", &[&["Question"], &["Two"], &["Three"]]),
        ];
        let imported = import_tables(&tables, &[], 1).unwrap();
        let titles: Vec<String> = imported
            .slides
            .iter()
            .map(|(_, slide)| match &slide.content {
                SlideContent::Billboard { title, .. } => format!("billboard {title}"),
                SlideContent::Question(question) => question.text.clone(),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(titles, ["billboard Round 1", "One", "billboard Round 2", "Two", "Three"]);
        assert_eq!(imported.slides[4].0, Place { sheet: "Round 2".into(), row: 3 });
        assert_eq!(imported.notes.0, [ImportNote { issue: ImportIssue::NoQuestionColumn, places: vec![Place { sheet: "Instructions".into(), row: 1 }] }]);
        // One sheet of questions is no round.
        let one = import_tables(&tables[..2], &[], 1).unwrap();
        assert_eq!(one.slides.len(), 1);
    }

    #[test]
    fn a_sheet_without_a_question_column_is_refused() {
        assert_eq!(import_tables(&[table("", &[&["Title", "Artist"], &["Song", "Band"]])], &[], 1), Err(SheetError::NoQuestionColumn));
        assert_eq!(import_tables(&[], &[], 1), Err(SheetError::NoQuestionColumn));
    }

    #[test]
    fn times_read_as_the_reference_writes_them_and_as_people_type_them() {
        assert_eq!(clock_ms("00:00:05"), Some(5_000));
        assert_eq!(clock_ms("1:15"), Some(75_000));
        assert_eq!(clock_ms("0:01:15.5"), Some(75_500));
        assert_eq!(clock_ms("75,25"), Some(75_250));
        assert_eq!((clock_ms("1:2:3:4"), clock_ms("-5"), clock_ms("a:05"), clock_ms("")), (None, None, None, None));
        assert_eq!((clock(5_000), clock(3_725_500)), ("00:00:05".to_string(), "01:02:05.500".to_string()));
        assert_eq!((seconds(1_000), seconds(1_500), seconds(250)), ("1".to_string(), "1.5".to_string(), "0.25".to_string()));
        assert_eq!(column_letters(0) + &column_letters(25) + &column_letters(26) + &column_letters(701), "AZAAZZ");
    }

    /// A quiz with every slide type and every option a sheet carries.
    fn full_quiz() -> Quiz {
        let everyone = Question {
            text: "Capital of France?".into(),
            mode: AnswerMode::Everyone,
            answers: vec!["Lyon".into(), "Paris".into(), "Nice".into()],
            correct: vec![CorrectAnswer::full(1)],
            time_limit_s: 30,
            points: 100,
            points_end: Some(20),
            penalty: 5,
            read_as: "Capital of France, the country?".into(),
        };
        let open = Question { text: "Name the song".into(), mode: AnswerMode::FastestFinger, answers: vec![], correct: vec![], ..everyone.clone() };
        let mut asked = Slide::new("q1", SlideContent::Question(everyone));
        asked.notes = "Easy one\nRead slowly".into();
        asked.picture = Some("media/france.png".into());
        asked.background = Some(Background { sync: true, repeat: false, ..Background::new("media/waves.mp4") });
        asked.sound = Some(SoundClip {
            start: SoundStart::WithClock,
            repeat: true,
            volume: 70,
            part: Span { from_ms: 1_500, to_ms: Some(30_000) },
            gain_db: -3.5,
            replay: Some(Replay { part: Span { from_ms: 60_000, to_ms: None }, delay_ms: 500 }),
            ..SoundClip::new("media/anthem.mp3")
        });
        let mut song = Slide::new("q2", SlideContent::Question(open));
        song.video = Some(VideoClip {
            autoplay: false,
            intro_pause_ms: 0,
            muted: true,
            speed: 125,
            effect: Some(VideoEffect::Blur),
            replay: Some(Replay { part: Span::default(), delay_ms: 0 }),
            ..VideoClip::new("media/round 1/clip.webm")
        });
        let mut round = Slide::new("b1", SlideContent::Billboard { title: "Round 1".into(), text: "Films\nand music".into() });
        round.background = Some(Background::new("media/stage.jpg"));
        round.category = Some("c1".into());
        let mut winners = Slide::new("f1", SlideContent::Final { title: "Winners".into() });
        winners.sound = Some(SoundClip { start: SoundStart::OnDemand, ..SoundClip::new("media/anthem.mp3") });
        let mut quiz = Quiz::new(
            "Pub quiz",
            vec![
                round,
                asked,
                Slide::new("x1", SlideContent::Explanation { text: "Paris has been the capital since 987.".into() }),
                song,
                Slide::new("e1", SlideContent::EndOfRound { title: "End of round 1".into() }),
                Slide::new("t1", SlideContent::Billboard { title: "Just a title".into(), text: String::new() }),
                winners,
                Slide::new("u1", SlideContent::Billboard { title: String::new(), text: "Text without a title".into() }),
            ],
        );
        quiz.categories = vec![Category::new("c1", "Films")];
        quiz
    }

    #[test]
    fn an_exported_quiz_imports_as_it_was() {
        let quiz = full_quiz();
        assert!(quiz.validate().is_ok());
        let name = |reference: &str| reference.rsplit('/').next().map(str::to_string);
        let rows = export_table(&quiz, QuizLanguage::En, name);
        assert_eq!(rows[0], FIELDS.map(|column| column.english.to_string()));
        assert_eq!(cell(&rows, 2, Field::Text), "Capital of France?");
        assert_eq!(cell(&rows, 2, Field::Type), "", "three answers are A-C without a type");
        assert_eq!((cell(&rows, 1, Field::Type), cell(&rows, 1, Field::Text)), ("Section", "Round 1\nFilms\nand music"));
        assert_eq!((cell(&rows, 7, Field::Type), cell(&rows, 5, Field::Type)), ("Results", "End of round"));
        assert_eq!(cell(&rows, 2, Field::Answer(1)), "[Paris]");
        assert_eq!(cell(&rows, 2, Field::BackgroundVideo), "waves.mp4{loop=false;sync-with-quiz=true}");
        assert_eq!(cell(&rows, 1, Field::Background), "stage.jpg");
        assert_eq!((cell(&rows, 1, Field::Category), cell(&rows, 2, Field::Category)), ("Films", ""));
        assert_eq!(
            cell(&rows, 4, Field::Video),
            "clip.webm{replay=true;silent=true;speed=125;effect=Blur;intro-pause=0;autoplay=false}",
            "only settings that differ from the defaults"
        );

        let imported = import_tables(&[Table { name: String::new(), first_row: 1, rows }], &quiz.categories, 3).unwrap();
        assert!(imported.notes.0.is_empty(), "{:?}", imported.notes);
        let mut expected = quiz.clone();
        let renamed = quiz.media_files().into_iter().map(|file| (file.to_string(), name(file).unwrap())).collect();
        expected.rename_media(&renamed);
        let mut back: Vec<Slide> = imported.slides.into_iter().map(|(_, slide)| slide).collect();
        for (slide, original) in back.iter_mut().zip(&expected.slides) {
            slide.id.clone_from(&original.id);
        }
        assert_eq!(back, expected.slides);
    }

    #[test]
    fn cells_a_spreadsheet_would_run_as_formulas_export_as_text_and_import_as_they_were() {
        let mut slide = Slide::new("q", SlideContent::Question(Question {
            text: "=1+1".into(),
            mode: AnswerMode::FastestFinger,
            answers: vec!["=HYPERLINK(\"x\")".into(), "+420 777".into(), "-5".into(), "@home".into(), "'kept".into(), "'=x".into()],
            correct: vec![CorrectAnswer::full(2)],
            time_limit_s: 20,
            points: 10,
            points_end: None,
            penalty: 0,
            read_as: String::new(),
        }));
        slide.notes = "-2+3+cmd|' /C calc'!A0".into();
        let quiz = Quiz::new("Quiz", vec![slide]);
        let rows = export_table(&quiz, QuizLanguage::En, |reference| Some(reference.to_string()));
        let cells: Vec<&str> = (0..6).map(|i| cell(&rows, 1, Field::Answer(i))).collect();
        assert_eq!(cells, ["'=HYPERLINK(\"x\")", "'+420 777", "[-5]", "'@home", "'kept", "''=x"]);
        assert_eq!(cell(&rows, 1, Field::Notes), "'-2+3+cmd|' /C calc'!A0");
        assert_eq!(cell(&rows, 1, Field::Text), "'=1+1", "six answers are answered aloud without a prefix");

        let imported = import_tables(&[Table { name: String::new(), first_row: 1, rows }], &[], 3).unwrap();
        let back = &imported.slides[0].1;
        assert_eq!((question(back), &back.notes), (question(&quiz.slides[0]), &quiz.slides[0].notes));
    }

    #[test]
    fn a_wrong_answer_in_brackets_exports_with_a_quote_and_imports_wrong() {
        let quiz = Quiz::new("Quiz", vec![Slide::new("q", SlideContent::Question(Question {
            text: "Which is spelled as in the original?".into(),
            mode: AnswerMode::Everyone,
            answers: vec!["[Paris]".into(), "[sic]".into(), "'[sic]".into(), "[=x]".into()],
            correct: vec![CorrectAnswer::full(0)],
            time_limit_s: 20,
            points: 10,
            points_end: None,
            penalty: 0,
            read_as: String::new(),
        }))]);
        let rows = export_table(&quiz, QuizLanguage::En, |reference| Some(reference.to_string()));
        let cells: Vec<&str> = (0..4).map(|i| cell(&rows, 1, Field::Answer(i))).collect();
        assert_eq!(cells, ["[[Paris]]", "'[sic]", "''[sic]", "'[=x]"]);

        let imported = import_tables(&[Table { name: String::new(), first_row: 1, rows }], &[], 3).unwrap();
        assert_eq!(question(&imported.slides[0].1), question(&quiz.slides[0]));
    }

    #[test]
    fn media_left_out_of_an_export_leave_their_cells_empty() {
        let rows = export_table(&full_quiz(), QuizLanguage::En, |reference| (!reference.ends_with(".mp3")).then(|| reference.to_string()));
        assert_eq!(cell(&rows, 2, Field::Sound), "");
        assert_eq!(cell(&rows, 2, Field::Picture), "media/france.png");
    }

    #[test]
    fn czech_and_english_headings_name_the_same_columns() {
        let rows: [&[&str]; 3] = [
            &["Otázka", "Odpověď 1", "Odpověď 2", "Poznámky", "Čas na odpověď", "Body", "Srážka za chybu", "Video na pozadí"],
            &["otazka", "ODPOVED-1", "odpověď2", "poznamky", "cas na odpoved", "BODY", "srazka za chybu", "video na pozadi"],
            &["Question", "Answer 1", "Answer 2", "Notes", "Answer time", "Points", "Penalty", "Background video"],
        ];
        for headings in rows {
            let imported = import(&[headings, &["Capital?", "[Paris]", "Lyon", "Easy", "30", "50", "5", "waves.mp4"]]);
            assert!(imported.notes.0.is_empty(), "{headings:?}: {:?}", imported.notes);
            let slide = slides(&imported)[0];
            let asked = question(slide);
            assert_eq!((asked.text.as_str(), asked.answers.as_slice(), correct(asked)), ("Capital?", &["Paris".to_string(), "Lyon".into()][..], vec![0]));
            assert_eq!((asked.time_limit_s, asked.points, asked.penalty, slide.notes.as_str()), (30, 50, 5, "Easy"));
            assert_eq!(slide.background, Some(Background::new("waves.mp4")));
        }
        // A note names the column as the sheet does.
        let imported = import(&[&["Otázka", "Čas na odpověď", "Kategorie"], &["Capital?", "brzy"]]);
        assert_eq!(issues(&imported), [&ImportIssue::BadValue { column: "Čas na odpověď".into(), value: "brzy".into() }]);
    }

    #[test]
    fn a_sheet_of_just_questions_and_answers_takes_the_defaults() {
        let imported = import(&[
            &["Otázka", "Odpověď 1", "Odpověď 2", "Odpověď 3", "Odpověď 4", "Odpověď 5"],
            &["Capital of France?", "[Paris]", "Lyon", "Nice", "Lille"],
            &["Which is a fruit?", "Apple", "Leek"],
            &["Name a French river", "Seine", "Loire", "Rhône", "Garonne", "Dordogne"],
            &["How many people live in Paris?"],
        ]);
        assert!(imported.notes.0.is_empty(), "{:?}", imported.notes);
        let modes: Vec<(AnswerMode, usize)> = slides(&imported).iter().map(|slide| (question(slide).mode, question(slide).answers.len())).collect();
        // Up to four answers are the A-D buttons; more, or none, are answered aloud.
        assert_eq!(
            modes,
            [(AnswerMode::Everyone, 4), (AnswerMode::Everyone, 2), (AnswerMode::FastestFinger, 5), (AnswerMode::FastestFinger, 0)]
        );
        let first = question(slides(&imported)[0]);
        assert_eq!((first.time_limit_s, first.points, first.points_end, first.penalty), (DEFAULT_TIME_LIMIT_S, DEFAULT_POINTS, None, 0));
    }

    #[test]
    fn an_export_has_headings_in_the_quizmasters_language_and_only_the_columns_its_quiz_uses() {
        let rows = export_table(&full_quiz(), QuizLanguage::Cs, |reference| Some(reference.to_string()));
        assert_eq!(rows[0], FIELDS.map(|column| column.czech.to_string()), "the full quiz uses every column");
        assert_eq!((cell(&rows, 1, Field::Type), cell(&rows, 3, Field::Type)), ("Sekce", "Vysvětlení"));
        let table = |text: &str, mode: AnswerMode, answers: usize| {
            let answers = (0..answers).map(|i| i.to_string()).collect();
            let question = Question { text: text.into(), mode, answers, correct: vec![], time_limit_s: DEFAULT_TIME_LIMIT_S, points: DEFAULT_POINTS, points_end: None, penalty: 0, read_as: String::new() };
            export_table(&Quiz::new("Quiz", vec![Slide::new("q", SlideContent::Question(question))]), QuizLanguage::En, |_| None)
        };
        let rows = table("Capital?", AnswerMode::Everyone, 4);
        let basic = ["Type", "Question", "Answer 1", "Answer 2", "Answer 3", "Answer 4", "Answer 5", "Answer 6", "Notes", "Answer time", "Points"];
        assert_eq!(rows[0], basic, "only the basic columns");
        assert_eq!(rows[1][..3], ["", "Capital?", "0"]);
        assert_eq!([Field::AnswerTime, Field::PointsStart].map(|field| cell(&rows, 1, field)), ["", ""], "defaults are empty");
        // A type where the import would read the row otherwise without one.
        let typed = |rows: Vec<Vec<String>>| (cell(&rows, 1, Field::Type).to_string(), cell(&rows, 1, Field::Text).to_string());
        assert_eq!(typed(table("Who?", AnswerMode::FastestFinger, 2)), ("Fastest finger".into(), "Who?".into()));
        assert_eq!(typed(table("", AnswerMode::FastestFinger, 0)), ("Question".into(), "".into()));
        assert_eq!(typed(table("Name five", AnswerMode::FastestFinger, 5)), ("".into(), "Name five".into()));
        assert_eq!(typed(table("{B} is text", AnswerMode::FastestFinger, 0)), ("".into(), "{B} is text".into()));
    }
}
