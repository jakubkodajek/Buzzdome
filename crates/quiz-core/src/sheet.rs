//! Questions as a table (spec QZ-8): a row per slide, in the columns of the
//! reference app's Excel template, read into slides on import and written
//! from them on export. Pure: `quiz-file` reads and writes the files and
//! finds the media files the cells name, so here a slide's media are the
//! file names as the cells give them.

use crate::media::{
    Background, MediaKind, Replay, SoundClip, SoundStart, Span, VideoClip, VideoEffect, MAX_GAIN_DB, MAX_INTRO_PAUSE_MS, MAX_REPLAY_DELAY_MS,
    MAX_VIDEO_SPEED, MIN_GAIN_DB, MIN_VIDEO_SPEED,
};
use crate::model::{
    AnswerMode, Question, Quiz, Slide, SlideContent, DEFAULT_POINTS, DEFAULT_TIME_LIMIT_S, MAX_ANSWERS, MAX_EVERYONE_ANSWERS, MAX_POINTS,
    MAX_TIME_LIMIT_S, MIN_EVERYONE_ANSWERS,
};
use serde::Serialize;
use std::ops::RangeInclusive;
use ts_rs::TS;

/// One sheet of a workbook, or a CSV file: rows of cells, the first holding
/// the column headings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
    /// A sheet with values but no `Q` column, such as one of instructions: skipped.
    NoQuestionColumn,
    /// A heading this app does not know, or a column without one (named by
    /// its letter): its values are left out.
    UnknownColumn { column: String },
    /// A column of the reference app this one has no place for: its values are left out.
    UnsupportedColumn { column: String },
    /// A second column with the same heading: left out.
    DuplicateColumn { column: String },
    /// Text after `||` (a second language): left out.
    SecondLanguage,
    /// A file marked `{L}` to be linked: copied into the quiz like any
    /// other, as a quiz file always carries its media.
    LinkedFile,
    /// A slide type this app does not have: imported as a billboard with the text.
    SlideTypeNotSupported { prefix: String },
    /// A test question (`{T}`): imported as a question that gives no points.
    TestQuestion,
    /// A qualifier in a `Q` prefix this app does not have: left out.
    QualifierNotSupported { qualifier: String },
    /// Everyone answers (`EA`) with an answer count the A-D buttons cannot
    /// offer: imported as a fastest finger question.
    NotEveryone { answers: usize },
    /// More than one answer in brackets: the first is the correct one.
    SeveralCorrect,
    /// Answers on a slide that is no question: left out.
    AnswersLeftOut,
    /// A picture and a video: the picture is left out, as a slide shows one.
    PictureAndVideo,
    /// A background picture and a background video: the picture is left out.
    TwoBackgrounds,
    /// A picture or video on a final slide, which shows the ranking: left out.
    MediaOnFinal,
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
    #[error("no sheet has a Q column")]
    NoQuestionColumn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
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
}

/// The columns this app reads and writes, in the order an export writes
/// them, under the reference template's headings.
const FIELDS: [(&str, Field); 17] = [
    ("Q", Field::Text),
    ("A1", Field::Answer(0)),
    ("A2", Field::Answer(1)),
    ("A3", Field::Answer(2)),
    ("A4", Field::Answer(3)),
    ("A5", Field::Answer(4)),
    ("A6", Field::Answer(5)),
    ("P1", Field::Picture),
    ("NOTES", Field::Notes),
    ("SOUND", Field::Sound),
    ("VIDEO", Field::Video),
    ("BACKGROUND", Field::Background),
    ("BACKGROUND-VIDEO", Field::BackgroundVideo),
    ("ANSWER-TIME", Field::AnswerTime),
    ("POINTS-START", Field::PointsStart),
    ("POINTS-END", Field::PointsEnd),
    ("PENALTY-POINTS-START", Field::Penalty),
];

/// Columns of the reference template this app has no place for.
const UNSUPPORTED: [&str; 21] = [
    "CAT",
    "P2",
    "P3",
    "P4",
    "P5",
    "P6",
    "A7",
    "A8",
    "PENALTY-POINTS-END",
    "NO-ANSWER-POINTS",
    "BONUS-POINTS-1",
    "BONUS-POINTS-2",
    "BONUS-POINTS-3",
    "STYLE",
    "TRANSITION",
    "SHUFFLEMODE",
    "EVALUATION-MODE",
    "NEAREST-WINS-WINNERS",
    "TEXT-EVALUATION",
    "TOLERANCE",
    "END-OF-ROUND-ACTION",
];

impl Field {
    fn index(self) -> usize {
        FIELDS.iter().position(|&(_, field)| field == self).expect("every field has a column")
    }

    fn heading(self) -> String {
        FIELDS[self.index()].0.to_string()
    }
}

// Slide types of the `Q` prefix (`{B}Round 1`). X and F are this app's
// own, for its explanation and final slides.
const QUESTION: &str = "Q";
const TEST: &str = "T";
const BILLBOARD: &str = "B";
const END_OF_ROUND: &str = "E";
const EXPLANATION: &str = "X";
const FINAL: &str = "F";
/// The reference app's other slide types, which this one does not have.
const OTHER_TYPES: [&str; 9] = ["W", "A", "D", "L", "M", "J", "TL", "TF", "SR"];
// Question qualifiers (`{Q, EA}`).
const EVERYONE: &str = "EA";
const FASTEST_FINGER: &str = "FF";

/// Marks a media file to be linked rather than copied into the quiz.
const LINKED: &str = "{L}";

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

/// A video from a sheet: the reference's default intro pause of one
/// second, as the editor gives a new video.
fn sheet_video(file: impl Into<String>) -> VideoClip {
    VideoClip { intro_pause_ms: 1_000, ..VideoClip::new(file) }
}

/// Reads the slides of `tables`, in order. With more than one sheet of
/// questions, each sheet is a round that a billboard with its name opens.
/// Answers whose correct one is not marked are shuffled with `seed`.
/// Slides get the ids `import-1`, `import-2`... for the editor to replace.
pub fn import_tables(tables: &[Table], seed: u64) -> Result<ImportedRows, SheetError> {
    let mut shuffler = Shuffler(seed);
    let mut notes = Notes::default();
    let sheets: Vec<(&Table, Vec<(Place, Slide)>)> =
        tables.iter().filter_map(|table| Some((table, import_table(table, &mut shuffler, &mut notes)?))).collect();
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

/// The slides of one sheet; none when it has no `Q` column. An empty sheet
/// is skipped without a note: workbooks often have some.
fn import_table(table: &Table, shuffler: &mut Shuffler, notes: &mut Notes) -> Option<Vec<(Place, Slide)>> {
    let has_value = |row: &Vec<String>, column: usize| row.get(column).is_some_and(|cell| !cell.trim().is_empty());
    let (headings, data) = table.rows.split_first()?;
    let at_headings = Place { sheet: table.name.clone(), row: table.first_row };
    let mut columns: Vec<Option<Field>> = Vec::new();
    let mut left_out = Vec::new();
    for (column, heading) in headings.iter().enumerate() {
        let name = heading.trim().to_ascii_uppercase();
        let field = FIELDS.iter().find(|(known, _)| *known == name).map(|&(_, field)| field);
        columns.push(field.filter(|field| !columns.contains(&Some(*field))));
        let issue = match field {
            Some(_) if columns[column].is_none() => ImportIssue::DuplicateColumn { column: name },
            Some(_) => continue,
            None if name.is_empty() => ImportIssue::UnknownColumn { column: column_letters(column) },
            None if UNSUPPORTED.contains(&name.as_str()) => ImportIssue::UnsupportedColumn { column: name },
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
        let slide = import_row(&Row::new(&columns, cells), &place, shuffler, notes)?;
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

/// A row's cells by field, trimmed; empty where the sheet has none.
struct Row<'a>([&'a str; FIELDS.len()]);

impl<'a> Row<'a> {
    fn new(columns: &[Option<Field>], cells: &'a [String]) -> Row<'a> {
        let mut row = [""; FIELDS.len()];
        for (field, cell) in columns.iter().zip(cells) {
            if let Some(field) = field {
                row[field.index()] = cell.trim();
            }
        }
        Row(row)
    }

    fn get(&self, field: Field) -> &'a str {
        self.0[field.index()]
    }
}

/// What a row's `Q` prefix makes of it.
enum Kind {
    /// No prefix: a question, or a billboard for a row with only media.
    Plain,
    Question { mode: Option<AnswerMode>, test: bool },
    Billboard,
    EndOfRound,
    Explanation,
    Final,
    /// A slide type of the reference app's, as written (`{W}`).
    Other(String),
}

/// The slide type a `Q` cell's prefix gives, and the text after it. A
/// prefix with an unknown type is no prefix but part of the text.
fn prefix<'a>(cell: &'a str, place: &Place, notes: &mut Notes) -> (Kind, &'a str) {
    let Some((inner, text)) = cell.strip_prefix('{').and_then(|rest| rest.split_once('}')) else { return (Kind::Plain, cell) };
    let mut tokens = inner.split(',').map(|token| token.trim().to_ascii_uppercase());
    let first = tokens.next().unwrap_or_default();
    let mut kind = match first.as_str() {
        QUESTION | TEST => Kind::Question { mode: None, test: first == TEST },
        BILLBOARD => Kind::Billboard,
        END_OF_ROUND => Kind::EndOfRound,
        EXPLANATION => Kind::Explanation,
        FINAL => Kind::Final,
        other if OTHER_TYPES.contains(&other) => Kind::Other(format!("{{{other}}}")),
        _ => return (Kind::Plain, cell),
    };
    for qualifier in tokens {
        match (&mut kind, qualifier.as_str()) {
            (Kind::Question { mode, .. }, EVERYONE) => *mode = Some(AnswerMode::Everyone),
            (Kind::Question { mode, .. }, FASTEST_FINGER) => *mode = Some(AnswerMode::FastestFinger),
            _ => notes.add(ImportIssue::QualifierNotSupported { qualifier }, place),
        }
    }
    // Spaces after the prefix go, a line break stays: it starts a billboard's text.
    (kind, text.trim_start_matches([' ', '\t']))
}

/// The text before `||`; what follows is a second language this app does not have.
fn first_language<'a>(cell: &'a str, place: &Place, notes: &mut Notes) -> &'a str {
    match cell.split_once("||") {
        Some((first, _)) => {
            notes.add(ImportIssue::SecondLanguage, place);
            first.trim()
        }
        None => cell,
    }
}

/// An answer cell's text and whether brackets mark it correct; none when empty.
fn answer(cell: &str, place: &Place, notes: &mut Notes) -> Option<(String, bool)> {
    let bracketed = |text: &str| text.len() >= 2 && text.starts_with('[') && text.ends_with(']');
    if cell.is_empty() {
        return None;
    }
    let text = first_language(cell, place, notes);
    // `[Paris||Parijs]` marks the answer as `[Paris]||[Parijs]` does.
    let correct = bracketed(cell) || bracketed(text);
    let text = if correct { text.strip_prefix('[').unwrap_or(text) } else { text };
    let text = if correct { text.strip_suffix(']').unwrap_or(text) } else { text };
    Some((text.trim().to_string(), correct))
}

fn import_row(row: &Row, place: &Place, shuffler: &mut Shuffler, notes: &mut Notes) -> Option<Slide> {
    if row.0.iter().all(|cell| cell.is_empty()) {
        return None;
    }
    let (kind, text) = prefix(row.get(Field::Text), place, notes);
    let text = first_language(text, place, notes).to_string();
    let answers: Vec<(String, bool)> = (0..MAX_ANSWERS).filter_map(|i| answer(row.get(Field::Answer(i)), place, notes)).collect();
    let is_question = match kind {
        Kind::Question { .. } => true,
        Kind::Plain => !text.is_empty() || !answers.is_empty(),
        _ => false,
    };
    if !is_question && !answers.is_empty() {
        notes.add(ImportIssue::AnswersLeftOut, place);
    }
    let content = match kind {
        Kind::Question { mode, test } => SlideContent::Question(question(text, answers, mode, test, row, place, shuffler, notes)),
        Kind::Plain if is_question => SlideContent::Question(question(text, answers, None, false, row, place, shuffler, notes)),
        // A row with only media, such as a video for a full-screen video slide.
        Kind::Plain => SlideContent::Billboard { title: text, text: String::new() },
        Kind::Billboard => billboard(&text),
        Kind::Other(prefix) => {
            notes.add(ImportIssue::SlideTypeNotSupported { prefix }, place);
            billboard(&text)
        }
        Kind::EndOfRound => SlideContent::EndOfRound { title: text },
        Kind::Explanation => SlideContent::Explanation { text },
        Kind::Final => SlideContent::Final { title: text },
    };
    let mut slide = Slide::new("", content);
    slide.notes = row.get(Field::Notes).to_string();
    import_media(row, &mut slide, place, notes);
    fit(&mut slide, place, notes);
    Some(slide)
}

/// A billboard's title is the first line, its text the rest.
fn billboard(text: &str) -> SlideContent {
    let (title, rest) = text.split_once('\n').unwrap_or((text, ""));
    SlideContent::Billboard { title: title.trim_end().to_string(), text: rest.to_string() }
}

#[allow(clippy::too_many_arguments)]
fn question(
    text: String,
    answers: Vec<(String, bool)>,
    asked: Option<AnswerMode>,
    test: bool,
    row: &Row,
    place: &Place,
    shuffler: &mut Shuffler,
    notes: &mut Notes,
) -> Question {
    let marked: Vec<usize> = answers.iter().enumerate().filter(|(_, (_, correct))| *correct).map(|(i, _)| i).collect();
    if marked.len() > 1 {
        notes.add(ImportIssue::SeveralCorrect, place);
    }
    let mut texts: Vec<String> = answers.into_iter().map(|(text, _)| text).collect();
    // Unmarked, the correct answer is A1 and the answers are shuffled.
    let mut correct = marked.first().copied().or((!texts.is_empty()).then_some(0));
    if marked.is_empty() && texts.len() > 1 {
        let order = shuffler.order(texts.len());
        correct = order.iter().position(|&i| i == 0);
        texts = order.iter().map(|&i| texts[i].clone()).collect();
    }
    let fits = (MIN_EVERYONE_ANSWERS..=MAX_EVERYONE_ANSWERS).contains(&texts.len());
    let mode = match asked {
        Some(AnswerMode::Everyone) if !fits => {
            notes.add(ImportIssue::NotEveryone { answers: texts.len() }, place);
            AnswerMode::FastestFinger
        }
        Some(mode) => mode,
        None if fits => AnswerMode::Everyone,
        None => AnswerMode::FastestFinger,
    };
    let mut question = Question {
        text,
        mode,
        answers: texts,
        correct,
        time_limit_s: DEFAULT_TIME_LIMIT_S,
        points: DEFAULT_POINTS,
        points_end: None,
        penalty: 0,
    };
    if test {
        notes.add(ImportIssue::TestQuestion, place);
        question.points = 0;
        return question;
    }
    let mut number = |field: Field, range: RangeInclusive<i64>| {
        let cell = row.get(field);
        let value = integer(cell).filter(|n| range.contains(n));
        if value.is_none() && !cell.is_empty() {
            notes.add(ImportIssue::BadValue { column: field.heading(), value: cell.to_string() }, place);
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

/// A media cell: `{L}` (linked), the file, and settings in braces.
struct MediaCell<'a> {
    file: &'a str,
    settings: Vec<(String, &'a str)>,
}

fn media_cell<'a>(field: Field, cell: &'a str, kinds: &[MediaKind], place: &Place, notes: &mut Notes) -> Option<MediaCell<'a>> {
    let mut file = cell;
    if file.get(..LINKED.len()).is_some_and(|start| start.eq_ignore_ascii_case(LINKED)) {
        notes.add(ImportIssue::LinkedFile, place);
        file = file[LINKED.len()..].trim_start();
    }
    let mut settings = Vec::new();
    if let Some((name, braced)) = file.strip_suffix('}').and_then(|body| body.rsplit_once('{')) {
        file = name.trim_end();
        for setting in braced.split(';').map(str::trim).filter(|setting| !setting.is_empty()) {
            let (key, value) = setting.split_once('=').unwrap_or((setting, ""));
            settings.push((key.trim().to_ascii_lowercase(), value.trim()));
        }
    }
    // Brackets mark the correct answer's picture in the reference; here P1 is the slide's picture.
    if field == Field::Picture {
        file = file.strip_prefix('[').and_then(|inner| inner.strip_suffix(']')).unwrap_or(file);
    }
    if file.is_empty() {
        return None;
    }
    if !MediaKind::of(file).is_some_and(|kind| kinds.contains(&kind)) {
        notes.add(ImportIssue::WrongMediaType { column: field.heading(), file: file.to_string() }, place);
        return None;
    }
    Some(MediaCell { file, settings })
}

fn import_media(row: &Row, slide: &mut Slide, place: &Place, notes: &mut Notes) {
    use MediaKind::{Audio, Image, Video};
    let mut cell = |field: Field, kinds: &[MediaKind]| media_cell(field, row.get(field), kinds, place, notes);
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
        apply_settings(&mut (), &cell, set_nothing, Field::Picture, place, notes);
        slide.picture = Some(cell.file.to_string());
    }
    if let Some(cell) = sound {
        let mut sound = SoundClip::new(cell.file);
        apply_settings(&mut sound, &cell, set_sound, Field::Sound, place, notes);
        keep_parts(&mut sound.part, &mut sound.replay, Field::Sound, place, notes);
        slide.sound = Some(sound);
    }
    if let Some(cell) = video {
        let mut video = sheet_video(cell.file);
        apply_settings(&mut video, &cell, set_video, Field::Video, place, notes);
        keep_parts(&mut video.part, &mut video.replay, Field::Video, place, notes);
        slide.video = Some(video);
    }
    if let Some((field, cell)) = background {
        let mut background = Background::new(cell.file);
        apply_settings(&mut background, &cell, set_background, field, place, notes);
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

fn apply_settings<T>(target: &mut T, cell: &MediaCell, set: fn(&mut T, &str, &str) -> Setting, field: Field, place: &Place, notes: &mut Notes) {
    for (key, value) in &cell.settings {
        let (column, setting) = (field.heading(), key.clone());
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
fn keep_parts(part: &mut Span, replay: &mut Option<Replay>, field: Field, place: &Place, notes: &mut Notes) {
    let parts = [(Some(part), STOP_AT), (replay.as_mut().map(|replay| &mut replay.part), REPLAY_STOP_AT)];
    for (part, setting) in parts {
        let Some(part) = part.filter(|part| part.is_empty()) else { continue };
        let value = part.to_ms.map(clock).unwrap_or_default();
        notes.add(ImportIssue::BadSetting { column: field.heading(), setting: setting.to_string(), value }, place);
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
fn fit(slide: &mut Slide, place: &Place, notes: &mut Notes) {
    if slide.picture.is_some() && slide.video.is_some() {
        notes.add(ImportIssue::PictureAndVideo, place);
        slide.picture = None;
    }
    if matches!(slide.content, SlideContent::Final { .. }) && (slide.picture.is_some() || slide.video.is_some()) {
        notes.add(ImportIssue::MediaOnFinal, place);
        (slide.picture, slide.video) = (None, None);
    }
    if matches!(slide.content, SlideContent::Question(_)) {
        return;
    }
    let mut not_used = |field: Field, setting: &str| {
        notes.add(ImportIssue::SettingNotUsed { column: field.heading(), setting: setting.to_string() }, place);
    };
    // A clock, a reveal and an answer time are a question's.
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

/// The quiz's slides as a table: the headings, then a row per slide.
/// `file_name` gives the name a media reference has next to the table;
/// none leaves the file out.
pub fn export_table(quiz: &Quiz, file_name: impl Fn(&str) -> Option<String>) -> Vec<Vec<String>> {
    let headings = FIELDS.iter().map(|(heading, _)| heading.to_string()).collect();
    std::iter::once(headings).chain(quiz.slides.iter().map(|slide| export_row(slide, &file_name))).collect()
}

fn export_row(slide: &Slide, file_name: &impl Fn(&str) -> Option<String>) -> Vec<String> {
    let mut row = vec![String::new(); FIELDS.len()];
    let mut set = |field: Field, value: String| row[field.index()] = value;
    let prefixed = |kind: &str, text: &str| format!("{{{kind}}}{text}");
    match &slide.content {
        SlideContent::Question(question) => {
            let mode = match question.mode {
                AnswerMode::Everyone => EVERYONE,
                AnswerMode::FastestFinger => FASTEST_FINGER,
            };
            set(Field::Text, format!("{{{QUESTION}, {mode}}}{}", question.text));
            for (i, answer) in question.answers.iter().enumerate().take(MAX_ANSWERS) {
                set(Field::Answer(i), if question.correct == Some(i) { format!("[{answer}]") } else { answer.clone() });
            }
            set(Field::AnswerTime, question.time_limit_s.to_string());
            set(Field::PointsStart, question.points.to_string());
            set(Field::PointsEnd, question.points_end.map(|end| end.to_string()).unwrap_or_default());
            set(Field::Penalty, question.penalty.to_string());
        }
        SlideContent::Billboard { title, text } if text.is_empty() => set(Field::Text, prefixed(BILLBOARD, title)),
        SlideContent::Billboard { title, text } => set(Field::Text, prefixed(BILLBOARD, &format!("{title}\n{text}"))),
        SlideContent::EndOfRound { title } => set(Field::Text, prefixed(END_OF_ROUND, title)),
        SlideContent::Explanation { text } => set(Field::Text, prefixed(EXPLANATION, text)),
        SlideContent::Final { title } => set(Field::Text, prefixed(FINAL, title)),
    }
    set(Field::Notes, slide.notes.clone());
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
        import_tables(&[table("", rows)], 7).unwrap()
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

    fn at(row: u32) -> Place {
        Place { sheet: String::new(), row }
    }

    #[test]
    fn a_row_of_the_reference_template_becomes_a_question() {
        let imported = import(&[
            &["Q", "A1", "A2", "A3", "A4", "NOTES", "P1", "SOUND"],
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
        // A1 is correct and the answers are shuffled; the correct one follows Paris.
        assert_eq!(capital.answers[capital.correct.unwrap()], "Paris");
        assert_eq!((found[0].notes.as_str(), found[0].picture.as_deref()), ("Easy one", Some("france.png")));
        assert_eq!(found[0].sound, Some(SoundClip::new("anthem.mp3")));
        assert_eq!((capital.time_limit_s, capital.points, capital.points_end, capital.penalty), (DEFAULT_TIME_LIMIT_S, DEFAULT_POINTS, None, 0));

        // No answers: an open question, answered aloud.
        let song = question(found[1]);
        assert_eq!((song.mode, song.answers.len(), song.correct), (AnswerMode::FastestFinger, 0, None));
        // A marked answer keeps the order.
        assert_eq!(question(found[2]).answers, ["Carrot", "Apple", "Leek"]);
        assert_eq!(question(found[2]).correct, Some(1));
        assert_eq!(imported.slides.iter().map(|(place, _)| place.row).collect::<Vec<_>>(), [2, 4, 5]);
        assert_eq!(found.iter().map(|slide| slide.id.as_str()).collect::<Vec<_>>(), ["import-1", "import-2", "import-3"]);
        assert!(imported.notes.0.is_empty());
    }

    #[test]
    fn shuffled_answers_keep_the_correct_one_with_its_text() {
        let rows: &[&[&str]] = &[&["Q", "A1", "A2", "A3", "A4"], &["?", "right", "w1", "w2", "w3"]];
        let mut orders = std::collections::HashSet::new();
        for seed in 0..200 {
            let imported = import_tables(&[table("", rows)], seed).unwrap();
            let question = question(&imported.slides[0].1);
            assert_eq!(question.answers[question.correct.unwrap()], "right");
            orders.insert(question.answers.clone());
        }
        assert_eq!(orders.len(), 24, "every order of four answers comes up");
    }

    #[test]
    fn headings_are_read_whatever_their_case_and_spaces() {
        let imported = import(&[&[" q ", "a1", "A2"], &["Yes or no?", "[Yes]", "No"]]);
        assert_eq!(question(slides(&imported)[0]).answers, ["Yes", "No"]);
    }

    #[test]
    fn a_prefix_gives_the_slide_type() {
        let imported = import(&[
            &["Q", "A1", "A2", "A3", "A4", "A5"],
            &["{B}Round 1\nTen questions about films"],
            &["{Q, FF}Who directed Alien?", "Ridley Scott"],
            &["{Q, EA, CM}Pick five", "a", "b", "c", "d", "e"],
            &["{T}Warm-up", "[x]", "y"],
            &["{W}How much do you bet?", "20%", "50%"],
            &["{E}End of round 1"],
            &["{X}Ridley Scott also directed Gladiator."],
            &["{F}And the winner is"],
            &["{Hint} is part of the text", "[a]", "b"],
        ]);
        let found = slides(&imported);
        let contents: Vec<&SlideContent> = found.iter().map(|slide| &slide.content).collect();
        assert_eq!(contents[0], &SlideContent::Billboard { title: "Round 1".into(), text: "Ten questions about films".into() });
        let ff = question(found[1]);
        assert_eq!((ff.mode, ff.answers.as_slice(), ff.correct), (AnswerMode::FastestFinger, &["Ridley Scott".to_string()][..], Some(0)));
        assert_eq!(question(found[2]).mode, AnswerMode::FastestFinger, "five answers do not fit A-D");
        assert_eq!(question(found[3]).points, 0);
        assert_eq!(contents[4], &SlideContent::Billboard { title: "How much do you bet?".into(), text: String::new() });
        assert_eq!(contents[5], &SlideContent::EndOfRound { title: "End of round 1".into() });
        assert_eq!(contents[6], &SlideContent::Explanation { text: "Ridley Scott also directed Gladiator.".into() });
        assert_eq!(contents[7], &SlideContent::Final { title: "And the winner is".into() });
        assert_eq!(question(found[8]).text, "{Hint} is part of the text");
        assert_eq!(
            issues(&imported),
            [
                &ImportIssue::QualifierNotSupported { qualifier: "CM".into() },
                &ImportIssue::NotEveryone { answers: 5 },
                &ImportIssue::TestQuestion,
                &ImportIssue::AnswersLeftOut,
                &ImportIssue::SlideTypeNotSupported { prefix: "{W}".into() },
            ]
        );
    }

    #[test]
    fn a_row_with_only_a_video_is_a_full_screen_video_slide() {
        let imported = import(&[&["Q", "VIDEO"], &["", "intro.mp4"]]);
        let slide = slides(&imported)[0];
        assert_eq!(slide.content, SlideContent::Billboard { title: String::new(), text: String::new() });
        assert_eq!(slide.video, Some(sheet_video("intro.mp4")));
        assert_eq!(slide.video.as_ref().unwrap().intro_pause_ms, 1_000, "the reference's default intro pause");
    }

    #[test]
    fn a_second_language_is_left_out() {
        let imported = import(&[&["Q", "A1", "A2"], &["Capital?||Hoofdstad?", "[Paris||Parijs]", "Lyon || Lyon"]]);
        let question = question(slides(&imported)[0]);
        assert_eq!((question.text.as_str(), question.answers.as_slice()), ("Capital?", &["Paris".to_string(), "Lyon".to_string()][..]));
        assert_eq!(question.correct, Some(0));
        assert_eq!(imported.notes.0, [ImportNote { issue: ImportIssue::SecondLanguage, places: vec![at(2)] }]);
    }

    #[test]
    fn scoring_columns_set_the_question_and_bad_values_keep_the_default() {
        let imported = import(&[
            &["Q", "ANSWER-TIME", "POINTS-START", "POINTS-END", "PENALTY-POINTS-START"],
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
                &ImportIssue::BadValue { column: "ANSWER-TIME".into(), value: "soon".into() },
                &ImportIssue::BadValue { column: "POINTS-START".into(), value: "1000000".into() },
            ]
        );
    }

    #[test]
    fn media_settings_in_braces_set_the_options() {
        let imported = import(&[
            &["Q", "VIDEO", "BACKGROUND-VIDEO", "SOUND"],
            &[
                "Watch",
                "{L} clips/intro.mp4{loop=false; start-at=00:00:05;stop-at=0:20;silent=true;volume=40;speed=50;effect=Mosaic;intro-pause=2,5}",
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
        assert_eq!(issues(&imported), [&ImportIssue::LinkedFile]);
    }

    #[test]
    fn settings_that_do_not_apply_or_are_wrong_are_noted() {
        let imported = import(&[
            &["Q", "VIDEO", "BACKGROUND-VIDEO", "SOUND"],
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
                &ImportIssue::BadSetting { column: column("VIDEO"), setting: "speed".into(), value: "500".into() },
                &ImportIssue::SettingNotUsed { column: column("VIDEO"), setting: "sync-with-quiz".into() },
                &ImportIssue::UnknownSetting { column: column("VIDEO"), setting: "colour".into() },
                &ImportIssue::BadSetting { column: column("VIDEO"), setting: "stop-at".into(), value: "00:01:00".into() },
                &ImportIssue::SettingNotUsed { column: column("BACKGROUND-VIDEO"), setting: "intro-pause".into() },
                // A billboard has no clock and no answer time.
                &ImportIssue::SettingNotUsed { column: column("SOUND"), setting: "plays".into() },
                &ImportIssue::SettingNotUsed { column: column("VIDEO"), setting: "effect".into() },
            ]
        );
    }

    #[test]
    fn media_a_slide_cannot_have_are_left_out() {
        let imported = import(&[
            &["Q", "P1", "VIDEO", "BACKGROUND", "BACKGROUND-VIDEO", "SOUND"],
            &["Both", "[a.png]", "b.mp4", "c.jpg", "d.mp4", "e.wmv"],
            &["{F}Winners", "a.png", "", "c.jpg", "", "fanfare.mp3"],
        ]);
        let found = slides(&imported);
        assert_eq!((found[0].picture.as_deref(), found[0].background.as_ref().map(|b| b.file.as_str())), (None, Some("d.mp4")));
        assert_eq!(found[0].sound, None);
        assert_eq!((found[1].picture.as_deref(), found[1].background.as_ref().map(|b| b.file.as_str())), (None, Some("c.jpg")));
        assert_eq!(found[1].sound.as_ref().map(|s| s.file.as_str()), Some("fanfare.mp3"));
        assert_eq!(
            issues(&imported),
            [
                &ImportIssue::WrongMediaType { column: "SOUND".into(), file: "e.wmv".into() },
                &ImportIssue::TwoBackgrounds,
                &ImportIssue::PictureAndVideo,
                &ImportIssue::MediaOnFinal,
            ]
        );
    }

    #[test]
    fn columns_without_a_place_here_are_noted_once_if_they_hold_values() {
        let imported = import(&[
            &["Q", "CAT", "STYLE", "COLOUR", "", "Q", "BONUS-POINTS-1"],
            &["One", "Music", "", "red", "x", "dup"],
            &["Two", "Music", "", "", "", "", "", "beyond"],
        ]);
        assert_eq!(question(slides(&imported)[0]).text, "One");
        let headings = at(1);
        assert_eq!(
            imported.notes.0,
            [
                ImportNote { issue: ImportIssue::UnsupportedColumn { column: "CAT".into() }, places: vec![headings.clone()] },
                ImportNote { issue: ImportIssue::UnknownColumn { column: "COLOUR".into() }, places: vec![headings.clone()] },
                ImportNote { issue: ImportIssue::UnknownColumn { column: "E".into() }, places: vec![headings.clone()] },
                ImportNote { issue: ImportIssue::DuplicateColumn { column: "Q".into() }, places: vec![headings.clone()] },
                ImportNote { issue: ImportIssue::UnknownColumn { column: "H".into() }, places: vec![headings] },
            ]
        );
    }

    #[test]
    fn notes_list_each_issue_once_with_its_rows() {
        let imported = import(&[&["Q", "A1", "A2"], &["{W}a"], &["{W}b"], &["{D}c", "x"]]);
        assert_eq!(
            imported.notes.0,
            [
                ImportNote { issue: ImportIssue::SlideTypeNotSupported { prefix: "{W}".into() }, places: vec![at(2), at(3)] },
                ImportNote { issue: ImportIssue::AnswersLeftOut, places: vec![at(4)] },
                ImportNote { issue: ImportIssue::SlideTypeNotSupported { prefix: "{D}".into() }, places: vec![at(4)] },
            ]
        );
    }

    #[test]
    fn several_sheets_become_rounds_opened_by_their_names() {
        let tables = [
            table("Instructions", &[&["Fill in the questions below"]]),
            table("Round 1", &[&["Q", "A1", "A2"], &["One", "[a]", "b"]]),
            table("Sheet3", &[]),
            table("Round 2", &[&["Q"], &["Two"], &["Three"]]),
        ];
        let imported = import_tables(&tables, 1).unwrap();
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
        let one = import_tables(&tables[..2], 1).unwrap();
        assert_eq!(one.slides.len(), 1);
    }

    #[test]
    fn a_sheet_without_a_q_column_is_refused() {
        assert_eq!(import_tables(&[table("", &[&["Title", "Artist"], &["Song", "Band"]])], 1), Err(SheetError::NoQuestionColumn));
        assert_eq!(import_tables(&[], 1), Err(SheetError::NoQuestionColumn));
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
            correct: Some(1),
            time_limit_s: 30,
            points: 100,
            points_end: Some(20),
            penalty: 5,
        };
        let open = Question { text: "Name the song".into(), mode: AnswerMode::FastestFinger, answers: vec![], correct: None, ..everyone.clone() };
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
        let mut winners = Slide::new("f1", SlideContent::Final { title: "Winners".into() });
        winners.sound = Some(SoundClip { start: SoundStart::OnDemand, ..SoundClip::new("media/anthem.mp3") });
        Quiz::new(
            "Pub quiz",
            vec![
                round,
                asked,
                Slide::new("x1", SlideContent::Explanation { text: "Paris has been the capital since 987.".into() }),
                song,
                Slide::new("e1", SlideContent::EndOfRound { title: "End of round 1".into() }),
                Slide::new("t1", SlideContent::Billboard { title: "Just a title".into(), text: String::new() }),
                winners,
            ],
        )
    }

    #[test]
    fn an_exported_quiz_imports_as_it_was() {
        let quiz = full_quiz();
        assert!(quiz.validate().is_ok());
        let name = |reference: &str| reference.rsplit('/').next().map(str::to_string);
        let rows = export_table(&quiz, name);
        assert_eq!(rows[0], FIELDS.map(|(heading, _)| heading.to_string()));
        assert_eq!(rows[2][Field::Text.index()], "{Q, EA}Capital of France?");
        assert_eq!(rows[2][Field::Answer(1).index()], "[Paris]");
        assert_eq!(rows[2][Field::BackgroundVideo.index()], "waves.mp4{loop=false;sync-with-quiz=true}");
        assert_eq!(rows[1][Field::Background.index()], "stage.jpg");
        assert_eq!(
            rows[4][Field::Video.index()],
            "clip.webm{replay=true;silent=true;speed=125;effect=Blur;intro-pause=0;autoplay=false}",
            "only settings that differ from the defaults"
        );

        let imported = import_tables(&[Table { name: String::new(), first_row: 1, rows }], 3).unwrap();
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
    fn media_left_out_of_an_export_leave_their_cells_empty() {
        let rows = export_table(&full_quiz(), |reference| (!reference.ends_with(".mp3")).then(|| reference.to_string()));
        assert_eq!(rows[2][Field::Sound.index()], "");
        assert_eq!(rows[2][Field::Picture.index()], "media/france.png");
    }
}
