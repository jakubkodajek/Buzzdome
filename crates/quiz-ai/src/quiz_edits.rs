//! Changes a model on this computer makes to the quiz open in Studio, for
//! the AI panel (docs/ai-assistants.md): whatever the editor can change. The
//! model reads the quiz as `quiz.json` writes it, its slides numbered from 1
//! (`quiz_view`), and answers with changes (`edits_schema`): slides added,
//! edited, deleted or moved, and the quiz's own fields (title, theme,
//! settings, sounds, welcome screen, categories). They are read leniently
//! (`read_edits`) and made into `QuizChange`s against the slides' ids
//! (`resolve`), which the editor applies as one undo step: each slide and
//! value is read by the quiz model itself, so what the model writes is what
//! a quiz file may hold, and a change it cannot read is left out.

use quiz_core::{AnswerMode, Category, Quiz, QuizLanguage, QuizSettings, QuizSounds, Slide, SlideStyle, Welcome, MAX_ANSWERS};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use ts_rs::TS;

/// Most changes one reply makes.
pub const MAX_CHANGES: usize = 200;

/// Characters of a slide's JSON shown in full while the view has room;
/// the rest are shown in short (`short_slide`).
const SHORT_SLIDE_CHARS: usize = 120;

/// What the model is told about how a quiz is written: the slide types and
/// their fields, and the fields of the quiz's settings, sounds, welcome
/// screen, categories and slide styles as the quiz model describes them.
pub fn format_guide() -> String {
    use quiz_core::text_limits::{MAX_ANSWER_CHARS, MAX_NOTES_CHARS, MAX_QUESTION_CHARS, MAX_QUIZ_TITLE_CHARS, MAX_SLIDE_TEXT_CHARS, MAX_TITLE_CHARS};
    let everyone = AnswerMode::Everyone.answer_limits();
    format!(
        "Slides, as quiz.json writes them:\n\
         - Question: {{\"type\":\"question\",\"text\":\"...\",\"mode\":\"everyone\" or \"fastestFinger\",\"answers\":[\"...\"],\
         \"correct\":[{{\"answer\":0}}],\"timeLimitS\":30,\"points\":100,\"pointsEnd\":null,\"penalty\":0,\"readAs\":\"\"}}. \
         \"everyone\": every team presses A to D, {}-{} answers. \"fastestFinger\": the first team to buzz answers aloud and \
         the quizmaster judges, 0-{} answers shown at the reveal. \"correct\" lists the right answers by their index from 0 \
         (\"share\": percent of the points, 100 when left out). \"timeLimitS\": seconds, 0 for no clock. \"points\" fall to \
         \"pointsEnd\" as the clock runs when it is set. \"penalty\": points taken for a wrong answer. \"readAs\": how the voice \
         reads the question, empty for its text. For a new question you may write \"reason\" (the fact that answers it, kept \
         in the notes), then \"right\" (the correct answer) and \"wrong\" (the wrong answers) in place of \"answers\" and \"correct\".\n\
         - Billboard: {{\"type\":\"billboard\",\"title\":\"...\",\"text\":\"...\"}}: a round's title or an announcement.\n\
         - Explanation: {{\"type\":\"explanation\",\"text\":\"...\"}}: explains the answer of the question before it.\n\
         - End of round: {{\"type\":\"endOfRound\",\"title\":\"...\"}}: ends a round and shows the scores.\n\
         - Final: {{\"type\":\"final\",\"title\":\"...\"}}: the final ranking, at the end of the quiz.\n\
         Every slide may also have \"notes\" (only the quizmaster sees them), \"picture\": file, \"video\": {{\"file\": file}}, \
         \"sound\": {{\"file\": file}}, \"background\": {{\"file\": file}}, \"category\": a category's id, and \"style\" (below). \
         A file is \"attachment:N\" for the quizmaster's attachment N, or a file the quiz already uses (\"media/...\"). A slide \
         has a picture or a video, not both; an end of round and the final have neither.\n\
         Longest texts: quiz title {MAX_QUIZ_TITLE_CHARS}, question {MAX_QUESTION_CHARS}, answer {MAX_ANSWER_CHARS}, \
         slide title {MAX_TITLE_CHARS}, slide text {MAX_SLIDE_TEXT_CHARS}, notes {MAX_NOTES_CHARS} characters.\n\
         Fields of the quiz and of a slide's style (a field left out keeps its default; colours are \"#rrggbb\"):\n{}",
        everyone.fewest,
        everyone.most,
        MAX_ANSWERS,
        [
            ("settings", schema_of::<QuizSettings>()),
            ("sounds", schema_of::<QuizSounds>()),
            ("welcome", schema_of::<Welcome>()),
            ("categories[]", schema_of::<Category>()),
            ("style", schema_of::<SlideStyle>()),
        ]
        .iter()
        .map(|(name, schema)| format!("- {name}: {}", outline(schema, schema, 0)))
        .collect::<Vec<_>>()
        .join("\n")
    )
}

fn schema_of<T: schemars::JsonSchema>() -> Value {
    serde_json::to_value(schemars::schema_for!(T)).expect("a JSON Schema serializes: it is plain JSON values")
}

/// How deep `outline` goes into objects within objects.
const OUTLINE_DEPTH: usize = 3;

/// A short text of what `schema` (within `root`, for its `$defs`) holds:
/// an object's fields with what each takes, the allowed values of an enum.
fn outline(schema: &Value, root: &Value, depth: usize) -> String {
    let schema = resolved(schema, root);
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        return values.iter().map(Value::to_string).collect::<Vec<_>>().join("|");
    }
    if let Some(value) = schema.get("const") {
        return value.to_string();
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(options) = schema.get(key).and_then(Value::as_array) {
            let named: Vec<String> = options.iter().map(|option| outline(option, root, depth)).filter(|text| text != "null").collect();
            return named.join(" or ");
        }
    }
    let kind = match schema.get("type") {
        Some(Value::Array(kinds)) => kinds.iter().filter_map(Value::as_str).find(|kind| *kind != "null").unwrap_or("value").to_string(),
        Some(Value::String(kind)) => kind.clone(),
        _ => "value".to_string(),
    };
    match kind.as_str() {
        "object" if depth < OUTLINE_DEPTH => match schema.get("properties").and_then(Value::as_object) {
            Some(fields) => {
                let fields: Vec<String> = fields.iter().map(|(name, field)| format!("{name}: {}", outline(field, root, depth + 1))).collect();
                format!("{{{}}}", fields.join(", "))
            }
            None => "object".to_string(),
        },
        "array" => format!("[{}]", schema.get("items").map_or_else(|| "value".to_string(), |items| outline(items, root, depth))),
        "integer" | "number" => match (schema.get("minimum"), schema.get("maximum")) {
            (Some(min), Some(max)) => format!("{kind} {min}-{max}"),
            _ => kind,
        },
        _ => kind,
    }
}

/// `schema`, or what its `$ref` points at in `root`.
fn resolved<'a>(schema: &'a Value, root: &'a Value) -> &'a Value {
    match schema.get("$ref").and_then(Value::as_str).and_then(|path| path.strip_prefix("#/")) {
        Some(path) => root.pointer(&format!("/{path}")).unwrap_or(schema),
        None => schema,
    }
}

/// The quiz as the model reads it: its own fields as `quiz.json` writes
/// them, then each slide on a line of its own after its number, from 1,
/// without its id, in at most `max_chars` characters (the slides that do
/// not fit in full are shown in short), and the slide the quizmaster has
/// selected and the themes the quiz can take.
pub fn quiz_view(quiz: &Quiz, selected: Option<usize>, themes: &[(String, String)], max_chars: usize) -> String {
    let mut own = serde_json::to_value(quiz).expect("a quiz serializes");
    if let Some(fields) = own.as_object_mut() {
        fields.remove("slides");
        fields.remove("schemaVersion");
    }
    let mut lines = vec![format!("The quiz now: {own}")];
    let mut left = max_chars.saturating_sub(lines[0].len());
    if quiz.slides.is_empty() {
        lines.push("It has no slides yet.".to_string());
    }
    for (number, slide) in (1..).zip(&quiz.slides) {
        let full = slide_json(slide).to_string();
        let shown = if full.len() <= left { full } else { short_slide(slide) };
        left = left.saturating_sub(shown.len());
        lines.push(format!("{number}. {shown}"));
    }
    if let Some(index) = selected.filter(|index| *index < quiz.slides.len()) {
        lines.push(format!("The quizmaster has slide {} selected.", index + 1));
    }
    if !themes.is_empty() {
        let names: Vec<String> = themes.iter().map(|(id, name)| format!("\"{id}\" ({name})")).collect();
        lines.push(format!("Themes the quiz can take (\"theme\"): {}.", names.join(", ")));
    }
    lines.join("\n")
}

/// A slide as `quiz.json` writes it, without its id.
fn slide_json(slide: &Slide) -> Value {
    let mut value = serde_json::to_value(slide).expect("a slide serializes");
    if let Some(fields) = value.as_object_mut() {
        fields.remove("id");
    }
    value
}

/// A slide in short: its type and the start of its text.
fn short_slide(slide: &Slide) -> String {
    let value = slide_json(slide);
    let text: String = ["text", "title"].iter().find_map(|key| value.get(*key).and_then(Value::as_str)).unwrap_or_default().chars().take(SHORT_SLIDE_CHARS).collect();
    json!({ "type": value["type"], "text": text, "shortened": true }).to_string()
}

/// The JSON Schema of the model's reply: the changes and a summary for the
/// quizmaster. Small, so models of a few billion parameters keep to it; a
/// slide and the fields to set are objects of the quiz's format, which
/// `format_guide` describes.
pub fn edits_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "changes": {
                "type": "array",
                "maxItems": MAX_CHANGES,
                "items": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["add", "edit", "delete", "move", "quiz"] },
                        "after": { "type": "integer", "minimum": 0 },
                        "set": { "type": "object" },
                        "slide": { "type": "integer", "minimum": 1 },
                        "slides": { "type": "array", "items": { "type": "object" } }
                    },
                    "required": ["action"]
                }
            },
            "summary": { "type": "string" }
        },
        "required": ["changes", "summary"]
    })
}

/// What a change does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Add,
    Edit,
    Delete,
    Move,
    Quiz,
}

/// A change as the model wrote it, slides by their number from 1.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelChange {
    pub action: Option<Action>,
    pub slide: Option<usize>,
    /// Where added slides go, or a slide moves to: after this slide, 0 for the start.
    pub after: Option<usize>,
    pub set: Map<String, Value>,
    pub slides: Vec<Map<String, Value>>,
}

/// The changes and the summary the model wrote.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModelEdits {
    pub changes: Vec<ModelChange>,
    pub summary: String,
}

impl ModelChange {
    /// The change in `value`, read field by field: a number may be written
    /// as text, a slide or the fields to set as the only one of a list.
    fn read(value: &Value) -> Option<ModelChange> {
        let object = value.as_object()?;
        let number = |key: &str| match object.get(key) {
            Some(Value::Number(n)) => n.as_u64().and_then(|n| usize::try_from(n).ok()),
            Some(Value::String(s)) => s.trim().parse().ok(),
            _ => None,
        };
        let action = match object.get("action").and_then(Value::as_str).map(|a| a.trim().to_ascii_lowercase()).as_deref() {
            Some("add" | "insert") => Some(Action::Add),
            Some("edit" | "change" | "update") => Some(Action::Edit),
            Some("delete" | "remove") => Some(Action::Delete),
            Some("move") => Some(Action::Move),
            Some("quiz") => Some(Action::Quiz),
            _ => None,
        };
        let objects = |key: &str| match object.get(key) {
            Some(Value::Array(items)) => items.iter().filter_map(Value::as_object).cloned().collect(),
            Some(Value::Object(one)) => vec![one.clone()],
            _ => Vec::new(),
        };
        let set = match object.get("set") {
            Some(Value::Object(set)) => set.clone(),
            Some(Value::Array(items)) => items.iter().find_map(Value::as_object).cloned().unwrap_or_default(),
            _ => Map::new(),
        };
        Some(ModelChange { action, slide: number("slide"), after: number("after"), set, slides: objects("slides") })
    }
}

/// Most places in a reply where the changes' JSON is looked for: prose
/// before it rarely has more braces than this.
pub(crate) const JSON_STARTS: usize = 64;

/// The changes the model wrote as `content` (without a reasoning model's
/// thinking): the first JSON value from where a `{` or `[` begins that
/// holds changes, so a code block's fences or a sentence around it does
/// not hide it. None when there is none.
pub fn read_edits(content: &str) -> Option<ModelEdits> {
    let edits = |value: Value| -> Option<ModelEdits> {
        let (list, summary) = match value {
            Value::Array(list) => (list, String::new()),
            Value::Object(mut object) => match object.remove("changes")? {
                Value::Array(list) => (list, object.get("summary").and_then(Value::as_str).unwrap_or_default().trim().to_string()),
                _ => return None,
            },
            _ => return None,
        };
        let changes: Vec<ModelChange> = list.iter().filter_map(ModelChange::read).take(MAX_CHANGES).collect();
        Some(ModelEdits { changes, summary })
    };
    content
        .match_indices(['{', '['])
        .take(JSON_STARTS)
        .find_map(|(start, _)| serde_json::Deserializer::from_str(&content[start..]).into_iter::<Value>().next()?.ok().and_then(edits))
}

/// What marks the start of a change in the model's text.
const CHANGE_KEY: &str = "\"action\"";

/// How many changes the model began in `content` after its first `from`
/// bytes, which were counted before: the panel counts as the model writes,
/// without reading the whole text again each time.
pub fn changes_begun(content: &str, from: usize) -> usize {
    // A key that the last piece cut in two is counted now.
    let mut start = from.saturating_sub(CHANGE_KEY.len() - 1).min(content.len());
    while !content.is_char_boundary(start) {
        start -= 1;
    }
    content[start..].matches(CHANGE_KEY).count()
}

/// A change to the quiz, as the editor applies it (`history.ts`), by the
/// slides' ids as they were when the model was asked.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum QuizChange {
    /// New slides, without ids yet, after the slide `after` (none: at the start).
    #[serde(rename_all = "camelCase")]
    InsertSlides { after: Option<String>, slides: Vec<Slide> },
    /// The slide with this id, changed.
    #[serde(rename_all = "camelCase")]
    ReplaceSlide { slide: Box<Slide> },
    #[serde(rename_all = "camelCase")]
    RemoveSlide { id: String },
    /// The slide `id` after the slide `after` (none: at the start).
    #[serde(rename_all = "camelCase")]
    MoveSlide { id: String, after: Option<String> },
    #[serde(rename_all = "camelCase")]
    SetTitle { title: String },
    #[serde(rename_all = "camelCase")]
    SetTheme { theme: String },
    #[serde(rename_all = "camelCase")]
    SetSettings { settings: QuizSettings },
    #[serde(rename_all = "camelCase")]
    SetSounds { sounds: QuizSounds },
    #[serde(rename_all = "camelCase")]
    SetWelcome { welcome: Welcome },
    /// The quiz's categories as they are to be: the editor adds, changes
    /// and removes them by id.
    #[serde(rename_all = "camelCase")]
    SetCategories { categories: Vec<Category> },
}

/// The changes the editor applies, and how many of the model's it left out.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Resolved {
    pub changes: Vec<QuizChange>,
    pub left_out: usize,
}

/// An attachment as a slide can show it: its reference in the draft, and
/// whether it is a video, a picture or a sound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachedFile {
    pub reference: String,
    pub kind: quiz_core::MediaKind,
}

/// What `edits` change in `quiz`, as `QuizChange`s by the slides' ids:
/// slides numbered as `quiz_view` showed them, attachments (`attachment:N`
/// in a file field) as `attachments` has them in the draft, a theme only
/// one of `themes`. A slide or value the quiz model cannot read, or a
/// change of a slide that is not there, is left out. Added slides go after
/// the selected slide when the model says nowhere. `seed` shuffles the
/// answers of a question written with `right` and `wrong`.
pub fn resolve(quiz: &Quiz, edits: &ModelEdits, attachments: &[Option<AttachedFile>], selected: Option<usize>, themes: &[String], seed: u64) -> Resolved {
    let mut resolved = Resolved::default();
    let mut shuffle = Shuffler(seed | 1);
    let id_of = |number: usize| number.checked_sub(1).and_then(|i| quiz.slides.get(i)).map(|slide| slide.id.clone());
    // After slide `number`: 0 is the start, past the end is the end.
    let after = |number: usize| if number == 0 { None } else { id_of(number.min(quiz.slides.len())) };
    let mut gone: HashSet<String> = HashSet::new();
    for change in &edits.changes {
        let done = match change.action {
            Some(Action::Add) => {
                let slides: Vec<Slide> = change.slides.iter().filter_map(|written| new_slide(written, attachments, &mut shuffle)).collect();
                let place = change.after.or(change.slide).unwrap_or_else(|| selected.map_or(quiz.slides.len(), |index| index + 1));
                (!slides.is_empty()).then(|| QuizChange::InsertSlides { after: after(place), slides })
            }
            Some(Action::Edit) => change.slide.and_then(|number| number.checked_sub(1)).and_then(|i| quiz.slides.get(i)).and_then(|slide| {
                let edited = edited_slide(slide, &change.set, attachments, &mut shuffle)?;
                (edited != *slide).then(|| QuizChange::ReplaceSlide { slide: Box::new(edited) })
            }),
            Some(Action::Delete) => change.slide.and_then(id_of).filter(|id| gone.insert(id.clone())).map(|id| QuizChange::RemoveSlide { id }),
            Some(Action::Move) => match (change.slide.and_then(id_of), change.after) {
                (Some(id), Some(to)) if after(to).as_ref() != Some(&id) => Some(QuizChange::MoveSlide { id, after: after(to) }),
                _ => None,
            },
            Some(Action::Quiz) => {
                let before = resolved.changes.len();
                quiz_changes(quiz, &change.set, themes, &mut resolved.changes);
                if resolved.changes.len() == before {
                    resolved.left_out += 1;
                }
                continue;
            }
            None => None,
        };
        match done {
            Some(done) => resolved.changes.push(done),
            None => resolved.left_out += 1,
        }
    }
    resolved
}

/// `quiz` with `changes` made, as the editor makes them (`history.ts`),
/// for an assistant that changes a quiz folder (`mcp.rs` `change_quiz`).
/// Added slides get ids no slide has; a change of a slide that is not
/// there does nothing.
pub fn apply(quiz: &Quiz, changes: &[QuizChange]) -> Quiz {
    let mut quiz = quiz.clone();
    let mut used: HashSet<String> = quiz.slides.iter().map(|slide| slide.id.clone()).collect();
    let mut next = used.len() + 1;
    let place = |slides: &[Slide], after: &Option<String>| after.as_ref().map_or(Some(0), |id| slides.iter().position(|s| s.id == *id).map(|i| i + 1));
    for change in changes {
        match change {
            QuizChange::InsertSlides { after, slides } => {
                let at = place(&quiz.slides, after).unwrap_or(quiz.slides.len());
                let fresh: Vec<Slide> = slides
                    .iter()
                    .map(|slide| {
                        let mut slide = slide.clone();
                        while used.contains(&format!("slide-{next}")) {
                            next += 1;
                        }
                        slide.id = format!("slide-{next}");
                        used.insert(slide.id.clone());
                        slide
                    })
                    .collect();
                quiz.slides.splice(at..at, fresh);
            }
            QuizChange::ReplaceSlide { slide } => {
                if let Some(had) = quiz.slides.iter_mut().find(|had| had.id == slide.id) {
                    *had = Slide::clone(slide);
                }
            }
            QuizChange::RemoveSlide { id } => quiz.slides.retain(|slide| slide.id != *id),
            QuizChange::MoveSlide { id, after } => {
                if let Some(from) = quiz.slides.iter().position(|slide| slide.id == *id) {
                    let moved = quiz.slides.remove(from);
                    let at = place(&quiz.slides, after).unwrap_or(quiz.slides.len());
                    quiz.slides.insert(at, moved);
                }
            }
            QuizChange::SetTitle { title } => quiz.title = title.clone(),
            QuizChange::SetTheme { theme } => quiz.theme = Some(theme.clone()),
            QuizChange::SetSettings { settings } => quiz.settings = *settings,
            QuizChange::SetSounds { sounds } => quiz.sounds = sounds.clone(),
            QuizChange::SetWelcome { welcome } => quiz.welcome = welcome.clone(),
            QuizChange::SetCategories { categories } => quiz.categories = categories.clone(),
        }
    }
    quiz
}

/// A new slide from what the model wrote; none when the quiz model cannot read it.
fn new_slide(written: &Map<String, Value>, attachments: &[Option<AttachedFile>], shuffle: &mut Shuffler) -> Option<Slide> {
    let mut value = Value::Object(written.clone());
    let fields = value.as_object_mut()?;
    fields.insert("id".into(), json!(""));
    plain_fields(fields, attachments, shuffle, None);
    serde_json::from_value(value).ok()
}

/// `slide` with the fields `set` changes (a merge patch: null removes a
/// field); none when the quiz model cannot read the result.
fn edited_slide(slide: &Slide, set: &Map<String, Value>, attachments: &[Option<AttachedFile>], shuffle: &mut Shuffler) -> Option<Slide> {
    let mut patch = set.clone();
    patch.remove("id");
    let mut value = serde_json::to_value(slide).ok()?;
    plain_fields(&mut patch, attachments, shuffle, Some(&value));
    merge(&mut value, &Value::Object(patch));
    let mut edited: Slide = serde_json::from_value(value).ok()?;
    edited.id = slide.id.clone();
    Some(edited)
}

/// RFC 7386 JSON merge patch: `patch`'s fields replace `target`'s, objects
/// are merged, and null removes a field.
fn merge(target: &mut Value, patch: &Value) {
    let Value::Object(patch) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    let target = target.as_object_mut().expect("made an object above");
    for (key, value) in patch {
        if value.is_null() {
            target.remove(key);
        } else {
            merge(target.entry(key.clone()).or_insert(Value::Null), value);
        }
    }
}

/// The fields of a slide the model wrote as `quiz.json` writes them: the
/// type names it uses for the others, `question` for the text, `right` and
/// `wrong` as answers in shuffled order with the right one correct,
/// numbers for correct answers, `fastest` for the fastest finger, a file
/// written as `attachment:N` as the attachment's reference, a picture in
/// place of a video and back. `before` is the slide being edited.
fn plain_fields(fields: &mut Map<String, Value>, attachments: &[Option<AttachedFile>], shuffle: &mut Shuffler, before: Option<&Value>) {
    if let Some(kind) = fields.get("type").and_then(Value::as_str).map(|kind| kind.trim().to_ascii_lowercase().replace(['_', '-', ' '], "")) {
        let (kind, mode) = match kind.as_str() {
            "fastestfinger" | "fastest" | "buzz" => ("question", Some("fastestFinger")),
            "billboard" | "info" | "section" | "round" | "title" | "announcement" => ("billboard", None),
            "endofround" | "end" | "scores" => ("endOfRound", None),
            "final" | "results" | "ranking" => ("final", None),
            "explanation" => ("explanation", None),
            _ => ("question", None),
        };
        fields.insert("type".into(), json!(kind));
        if let Some(mode) = mode {
            fields.insert("mode".into(), json!(mode));
        }
    }
    let kind = fields.get("type").or_else(|| before.and_then(|b| b.get("type"))).and_then(Value::as_str).unwrap_or("question").to_string();
    if kind == "question" {
        if let Some(text) = fields.remove("question") {
            fields.entry("text").or_insert(text);
        }
    }
    if let Some(seconds) = fields.remove("time").or_else(|| fields.remove("timeLimit")) {
        fields.entry("timeLimitS").or_insert(seconds);
    }
    if let Some(Value::String(mode)) = fields.get("mode") {
        let mode = if mode.to_ascii_lowercase().starts_with("fast") || mode.eq_ignore_ascii_case("buzz") { "fastestFinger" } else { "everyone" };
        fields.insert("mode".into(), json!(mode));
    }
    let reason = fields.remove("reason").and_then(|reason| reason.as_str().map(str::trim).map(str::to_string)).filter(|reason| !reason.is_empty());
    if fields.contains_key("right") {
        let right = fields.remove("right").map(|right| text_of(&right)).unwrap_or_default();
        let wrong: Vec<String> = match fields.remove("wrong") {
            Some(Value::Array(items)) => items.iter().map(text_of).collect(),
            Some(one) => vec![text_of(&one)],
            None => Vec::new(),
        };
        let answers = usable_answers(&right, wrong);
        let order = shuffle.order(answers.len());
        fields.insert("answers".into(), json!(order.iter().map(|&i| &answers[i]).collect::<Vec<_>>()));
        let right_at = order.iter().position(|&i| i == 0).unwrap_or(0);
        fields.insert("correct".into(), json!([{ "answer": right_at }]));
    }
    if let Some(Value::Array(correct)) = fields.get("correct") {
        let answers: Vec<String> = fields.get("answers").and_then(Value::as_array).map(|a| a.iter().map(text_of).collect()).unwrap_or_default();
        let read: Vec<Value> = correct.iter().filter_map(|c| correct_answer(c, &answers)).collect();
        fields.insert("correct".into(), Value::Array(read));
    }
    if kind == "question" && before.is_none() {
        // A new question answered as its answers allow when the model gave no mode.
        if !fields.contains_key("mode") {
            let count = fields.get("answers").and_then(Value::as_array).map_or(0, Vec::len);
            let everyone = AnswerMode::Everyone.answer_limits();
            fields.insert("mode".into(), json!(if (everyone.fewest..=everyone.most).contains(&count) { "everyone" } else { "fastestFinger" }));
        }
    }
    if let Some(reason) = reason {
        let notes = fields.get("notes").or_else(|| before.and_then(|b| b.get("notes"))).and_then(Value::as_str).unwrap_or_default().trim().to_string();
        fields.insert("notes".into(), json!(if notes.is_empty() { reason } else { format!("{notes}\n{reason}") }));
    }
    files(fields, attachments);
}

/// The files of a slide's fields: `attachment:N` as the attachment's
/// reference, a clip written as its file alone as `{"file": ...}`, and a
/// slide given a picture loses its video and back.
fn files(fields: &mut Map<String, Value>, attachments: &[Option<AttachedFile>]) {
    let file = |value: &Value| -> Option<String> {
        let text = value.as_str()?.trim();
        let number = text.strip_prefix("attachment").map(|rest| rest.trim_start_matches([':', ' ', '#']));
        match number.and_then(|n| n.parse::<usize>().ok()) {
            Some(n) => n.checked_sub(1).and_then(|i| attachments.get(i)).cloned().flatten().map(|attached| attached.reference),
            None => (!text.is_empty()).then(|| text.to_string()),
        }
    };
    if let Some(n) = fields.remove("attachment") {
        let number = match &n {
            Value::Number(n) => n.as_u64().and_then(|n| usize::try_from(n).ok()),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        };
        if let Some(attached) = number.and_then(|n| n.checked_sub(1)).and_then(|i| attachments.get(i)).cloned().flatten() {
            match attached.kind {
                quiz_core::MediaKind::Video => fields.insert("video".into(), json!({ "file": attached.reference })),
                quiz_core::MediaKind::Audio => fields.insert("sound".into(), json!({ "file": attached.reference })),
                quiz_core::MediaKind::Image => fields.insert("picture".into(), json!(attached.reference)),
            };
        }
    }
    if let Some(picture) = fields.get("picture").filter(|v| !v.is_null()) {
        match file(picture) {
            Some(reference) => {
                fields.insert("picture".into(), json!(reference));
                fields.entry("video").or_insert(Value::Null);
            }
            None => {
                fields.remove("picture");
            }
        }
    }
    for clip in ["video", "sound", "background"] {
        let Some(value) = fields.get(clip).filter(|v| !v.is_null()).cloned() else { continue };
        let written = if value.is_object() { value.get("file").cloned().unwrap_or(Value::Null) } else { value.clone() };
        match file(&written) {
            Some(reference) => {
                let mut object = value.as_object().cloned().unwrap_or_default();
                object.insert("file".into(), json!(reference));
                fields.insert(clip.into(), Value::Object(object));
                if clip == "video" {
                    fields.entry("picture").or_insert(Value::Null);
                }
            }
            None => {
                fields.remove(clip);
            }
        }
    }
}

/// A correct answer as `quiz.json` writes it, from an index, an object, or
/// the answer's text; none for one that is not among `answers`.
fn correct_answer(written: &Value, answers: &[String]) -> Option<Value> {
    let index = |value: &Value| match value {
        Value::Number(n) => n.as_u64().and_then(|n| usize::try_from(n).ok()),
        Value::String(s) => s.trim().parse().ok().or_else(|| answers.iter().position(|a| a.trim().eq_ignore_ascii_case(s.trim()))),
        _ => None,
    };
    let (answer, share) = match written {
        Value::Object(object) => (object.get("answer").and_then(index)?, object.get("share").cloned()),
        other => (index(other)?, None),
    };
    (answers.is_empty() || answer < answers.len()).then(|| match share {
        Some(share) => json!({ "answer": answer, "share": share }),
        None => json!({ "answer": answer }),
    })
}

/// The right answer first, then the wrong ones a quiz can use: none empty,
/// none twice (whatever the case and spaces), none that is itself a question.
fn usable_answers(right: &str, wrong: Vec<String>) -> Vec<String> {
    let clean = |text: &str| {
        let text = text.trim();
        // Brackets mark a correct answer in a sheet; a model copies them.
        text.strip_prefix('[').and_then(|t| t.strip_suffix(']')).unwrap_or(text).split_whitespace().collect::<Vec<_>>().join(" ")
    };
    let mut seen: Vec<String> = Vec::new();
    let mut answers = Vec::new();
    for answer in std::iter::once(right.to_string()).chain(wrong) {
        let answer = clean(&answer);
        let key = answer.to_lowercase();
        if answer.is_empty() || answer.ends_with('?') || seen.contains(&key) {
            continue;
        }
        seen.push(key);
        answers.push(answer);
    }
    answers.truncate(MAX_ANSWERS);
    answers
}

/// A text or a number as the text it says; anything else as none.
pub(crate) fn text_of(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

/// The quiz's own fields `set` changes, as `QuizChange`s: each value read
/// by the quiz model, the theme only one of `themes`, a category without an
/// id given one.
fn quiz_changes(quiz: &Quiz, set: &Map<String, Value>, themes: &[String], changes: &mut Vec<QuizChange>) {
    let mut own = serde_json::to_value(quiz).expect("a quiz serializes");
    own["slides"] = json!([]);
    let mut patch = set.clone();
    for kept in ["slides", "schemaVersion"] {
        patch.remove(kept);
    }
    if let Some(Value::Array(categories)) = patch.get_mut("categories") {
        let mut taken: HashSet<String> = quiz.categories.iter().map(|c| c.id.clone()).collect();
        for (n, category) in categories.iter_mut().enumerate() {
            if let Some(fields) = category.as_object_mut() {
                if fields.get("id").and_then(Value::as_str).is_none_or(str::is_empty) {
                    let id = (n + 1..).map(|k| format!("ai-category-{k}")).find(|id| !taken.contains(id)).expect("an id is free");
                    taken.insert(id.clone());
                    fields.insert("id".into(), json!(id));
                }
            }
        }
    }
    // Each field is read on its own, so one the quiz model cannot read loses no other.
    for (key, value) in patch {
        let mut one = own.clone();
        merge(&mut one, &json!({ key.clone(): value }));
        let Ok(changed) = serde_json::from_value::<Quiz>(one) else { continue };
        match key.as_str() {
            "title" if changed.title != quiz.title => changes.push(QuizChange::SetTitle { title: changed.title }),
            "theme" => {
                if let Some(theme) = changed.theme.filter(|theme| themes.contains(theme) && quiz.theme.as_ref() != Some(theme)) {
                    changes.push(QuizChange::SetTheme { theme });
                }
            }
            "settings" if changed.settings != quiz.settings => changes.push(QuizChange::SetSettings { settings: changed.settings }),
            "sounds" if changed.sounds != quiz.sounds => changes.push(QuizChange::SetSounds { sounds: changed.sounds }),
            "welcome" if changed.welcome != quiz.welcome => changes.push(QuizChange::SetWelcome { welcome: changed.welcome }),
            "categories" if changed.categories != quiz.categories => changes.push(QuizChange::SetCategories { categories: changed.categories }),
            _ => {}
        }
    }
}

/// A small seeded shuffle (xorshift), so the right answer is not always first.
struct Shuffler(u64);

impl Shuffler {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// The indexes 0..n in a shuffled order.
    fn order(&mut self, n: usize) -> Vec<usize> {
        let mut order: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            order.swap(i, j);
        }
        order
    }
}

/// The quiz's language as the instructions name it.
pub fn language_name(language: QuizLanguage) -> &'static str {
    match language {
        QuizLanguage::Cs => "Czech",
        QuizLanguage::En => "English",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiz_core::{MediaKind, SlideContent};

    fn slide(value: Value) -> Slide {
        serde_json::from_value(value).unwrap()
    }

    /// Round 1: a billboard, a question with a picture and notes, an end of round.
    fn quiz() -> Quiz {
        Quiz::new(
            "Hlavní města",
            vec![
                slide(json!({ "id": "r1", "type": "billboard", "title": "Kolo 1", "text": "" })),
                slide(json!({ "id": "q1", "type": "question", "mode": "everyone", "text": "Hlavní město Moravy?", "answers": ["Brno", "Olomouc"],
                              "correct": [{ "answer": 0 }], "picture": "media/brno.jpg", "notes": "Zemské hlavní město." })),
                slide(json!({ "id": "end", "type": "endOfRound", "title": "Konec kola" })),
            ],
        )
    }

    fn edits(changes: Value) -> ModelEdits {
        read_edits(&json!({ "changes": changes, "summary": "ok" }).to_string()).unwrap()
    }

    fn resolved(changes: Value) -> Resolved {
        resolve(&quiz(), &edits(changes), &[], None, &["builtin.classic".into(), "builtin.night".into()], 7)
    }

    fn ids(quiz: &Quiz) -> Vec<&str> {
        quiz.slides.iter().map(|slide| slide.id.as_str()).collect()
    }

    fn question(slide: &Slide) -> &quiz_core::Question {
        match &slide.content {
            SlideContent::Question(question) => question,
            other => panic!("not a question: {other:?}"),
        }
    }

    #[test]
    fn changes_are_read_whatever_words_and_forms_the_model_uses() {
        let read = read_edits(r#"Here: {"changes":[{"action":"Remove","slide":"2"},{"action":"insert","after":1,"slides":{"type":"billboard","title":"A"}},{"action":"update","slide":1,"set":[{"title":"B"}]},{"action":"dance"}],"summary":" Hotovo. "}"#).unwrap();
        assert_eq!(read.summary, "Hotovo.");
        assert_eq!(read.changes.iter().map(|c| c.action).collect::<Vec<_>>(), [Some(Action::Delete), Some(Action::Add), Some(Action::Edit), None]);
        assert_eq!((read.changes[0].slide, read.changes[1].after, read.changes[1].slides.len()), (Some(2), Some(1), 1));
        assert_eq!(read.changes[2].set.get("title"), Some(&json!("B")));
        // A bare list of changes is read too; prose without them is not.
        assert_eq!(read_edits(r#"[{"action":"delete","slide":1}]"#).unwrap().changes.len(), 1);
        assert_eq!(read_edits("Nevím {co} dělat."), None);
        let many: Vec<Value> = (0..MAX_CHANGES + 5).map(|_| json!({ "action": "delete", "slide": 1 })).collect();
        assert_eq!(edits(json!(many)).changes.len(), MAX_CHANGES);
    }

    #[test]
    fn a_new_question_written_with_right_and_wrong_has_the_right_answer_correct() {
        let added = resolved(json!([{ "action": "add", "after": 2, "slides": [
            { "type": "question", "question": "Hlavní město Slovenska?", "reason": "Bratislava je hlavní město.", "right": "Bratislava", "wrong": ["Košice", "košice ", "Nitra", "Co?", "Bratislava"], "time": 20 }
        ] }]));
        assert_eq!(added.left_out, 0);
        let QuizChange::InsertSlides { after, slides } = &added.changes[0] else { panic!("{added:?}") };
        assert_eq!(after.as_deref(), Some("q1"));
        let new = question(&slides[0]);
        assert_eq!(new.mode, AnswerMode::Everyone);
        assert_eq!(new.answers.len(), 3, "repeats and questions are no answers: {:?}", new.answers);
        assert_eq!(new.answers[new.correct[0].answer], "Bratislava");
        assert_eq!(new.time_limit_s, 20);
        assert_eq!(slides[0].notes, "Bratislava je hlavní město.");
        // More answers than everyone can press: the fastest finger answers.
        let many = resolved(json!([{ "action": "add", "slides": [{ "type": "question", "text": "?", "answers": ["a", "b", "c", "d", "e"], "correct": ["c"] }] }]));
        let QuizChange::InsertSlides { slides, .. } = &many.changes[0] else { panic!() };
        assert_eq!((question(&slides[0]).mode, question(&slides[0]).correct[0].answer), (AnswerMode::FastestFinger, 2));
    }

    #[test]
    fn an_edit_changes_only_the_fields_it_sets_and_null_removes_one() {
        let edited = resolved(json!([{ "action": "edit", "slide": 2, "set": { "text": "Hlavní město Moravy dnes?", "notes": null, "points": 50 } }]));
        let QuizChange::ReplaceSlide { slide } = &edited.changes[0] else { panic!("{edited:?}") };
        assert_eq!(slide.id, "q1");
        let changed = question(slide);
        assert_eq!((changed.text.as_str(), changed.points, changed.answers.len()), ("Hlavní město Moravy dnes?", 50, 2));
        assert_eq!((slide.notes.as_str(), slide.picture.as_deref()), ("", Some("media/brno.jpg")));
        // A slide changes type with the new type's fields.
        let billboard = resolved(json!([{ "action": "edit", "slide": 1, "set": { "type": "explanation", "text": "Vysvětlení" } }]));
        let QuizChange::ReplaceSlide { slide } = &billboard.changes[0] else { panic!("{billboard:?}") };
        assert!(matches!(&slide.content, SlideContent::Explanation { text } if text == "Vysvětlení"));
        // An edit that changes nothing, of a slide that is not there, or that the model cannot read, is left out.
        let none = resolved(json!([
            { "action": "edit", "slide": 1, "set": { "title": "Kolo 1" } },
            { "action": "edit", "slide": 9, "set": { "title": "X" } },
            { "action": "edit", "slide": 2, "set": { "points": "hodně" } },
        ]));
        assert_eq!((none.changes.len(), none.left_out), (0, 3));
    }

    #[test]
    fn slides_are_deleted_once_and_moved_by_their_numbers_before_the_changes() {
        let done = resolved(json!([
            { "action": "move", "slide": 3, "after": 0 },
            { "action": "delete", "slide": 1 },
            { "action": "delete", "slide": 1 },
            { "action": "move", "slide": 2, "after": 2 },
        ]));
        assert_eq!(
            done.changes,
            [QuizChange::MoveSlide { id: "end".into(), after: None }, QuizChange::RemoveSlide { id: "r1".into() }]
        );
        assert_eq!(done.left_out, 2);
        assert_eq!(ids(&apply(&quiz(), &done.changes)), ["end", "q1"]);
    }

    #[test]
    fn added_slides_go_after_the_selected_slide_when_the_model_says_nowhere_and_get_new_ids() {
        let edits = edits(json!([{ "action": "add", "slides": [{ "type": "billboard", "title": "Kolo 2" }, { "type": "final", "title": "Výsledky" }] }]));
        let done = resolve(&quiz(), &edits, &[], Some(0), &[], 1);
        let QuizChange::InsertSlides { after, .. } = &done.changes[0] else { panic!() };
        assert_eq!(after.as_deref(), Some("r1"));
        let changed = apply(&quiz(), &done.changes);
        assert_eq!(changed.slides.len(), 5);
        assert_eq!(ids(&changed)[0], "r1");
        let new: HashSet<&str> = ids(&changed).into_iter().collect();
        assert_eq!(new.len(), 5, "every id is its own: {:?}", ids(&changed));
        // Without a selected slide they go at the end; after 0, at the start.
        let at_end = resolve(&quiz(), &edits, &[], None, &[], 1);
        assert!(matches!(&at_end.changes[0], QuizChange::InsertSlides { after: Some(id), .. } if id == "end"));
        let at_start = resolve(&quiz(), &self::edits(json!([{ "action": "add", "after": 0, "slides": [{ "type": "billboard", "title": "A" }] }])), &[], Some(1), &[], 1);
        assert!(matches!(&at_start.changes[0], QuizChange::InsertSlides { after: None, .. }));
        // A slide the quiz model cannot read is not added.
        let bad = resolved(json!([{ "action": "add", "slides": [{ "type": "question", "text": "?", "answers": "x", "points": [] }] }]));
        assert_eq!((bad.changes.len(), bad.left_out), (0, 1));
    }

    #[test]
    fn attachments_become_the_drafts_files_and_a_picture_replaces_a_video() {
        let files = [
            Some(AttachedFile { reference: "media/klip.mp4".into(), kind: MediaKind::Video }),
            None,
            Some(AttachedFile { reference: "media/zvuk.mp3".into(), kind: MediaKind::Audio }),
        ];
        let edits = edits(json!([
            { "action": "edit", "slide": 2, "set": { "attachment": 1 } },
            { "action": "add", "slides": [
                { "type": "billboard", "title": "A", "picture": "attachment:2" },
                { "type": "billboard", "title": "B", "sound": "attachment:3", "picture": "media/brno.jpg" }
            ] }
        ]));
        let done = resolve(&quiz(), &edits, &files, None, &[], 1);
        let QuizChange::ReplaceSlide { slide } = &done.changes[0] else { panic!("{done:?}") };
        assert_eq!((slide.picture.as_deref(), slide.video.as_ref().map(|v| v.file.as_str())), (None, Some("media/klip.mp4")));
        let QuizChange::InsertSlides { slides, .. } = &done.changes[1] else { panic!("{done:?}") };
        // An attachment that is in no slide's draft is no file.
        assert_eq!(slides[0].picture, None);
        assert_eq!((slides[1].sound.as_ref().map(|s| s.file.as_str()), slides[1].picture.as_deref()), (Some("media/zvuk.mp3"), Some("media/brno.jpg")));
    }

    #[test]
    fn the_quizs_own_fields_change_each_on_its_own() {
        let done = resolved(json!([
            { "action": "quiz", "set": { "title": "Města", "theme": "builtin.night", "settings": { "startOnNext": true }, "categories": [{ "name": "Zeměpis" }] } },
            { "action": "quiz", "set": { "theme": "builtin.unknown", "settings": { "allAnsweredDelayS": "x" } } },
        ]));
        assert_eq!(done.left_out, 1, "{done:?}");
        let changed = apply(&quiz(), &done.changes);
        assert_eq!((changed.title.as_str(), changed.theme.as_deref()), ("Města", Some("builtin.night")));
        assert!(changed.settings.start_on_next);
        assert_eq!(changed.settings.all_answered_delay_s, quiz().settings.all_answered_delay_s, "the rest of the settings stay");
        assert_eq!((changed.categories[0].id.as_str(), changed.categories[0].name.as_str()), ("ai-category-1", "Zeměpis"));
        assert_eq!(ids(&changed), ids(&quiz()));
    }

    #[test]
    fn the_model_sees_the_quiz_numbered_and_in_short_when_it_is_long() {
        let view = quiz_view(&quiz(), Some(1), &[("classic".into(), "Klasický".into())], 10_000);
        assert!(view.contains("\"title\":\"Hlavní města\"") && !view.contains("schemaVersion"), "{view}");
        assert!(view.contains("\n1. {") && view.contains("\n2. {") && view.contains("\n3. {"));
        assert!(!view.contains("\"id\""), "the model numbers slides, ids are the app's: {view}");
        assert!(view.contains("slide 2 selected") && view.contains("\"classic\" (Klasický)"));
        let short = quiz_view(&quiz(), None, &[], 100);
        assert!(short.contains("\"shortened\":true") && !short.contains("Zemské"), "{short}");
        assert!(quiz_view(&Quiz::new("", Vec::new()), None, &[], 1_000).contains("no slides yet"));
    }

    #[test]
    fn the_guide_names_the_fields_of_the_quiz_and_the_slides() {
        let guide = format_guide();
        for field in ["startOnNext", "timeLimitS", "fastestFinger", "endOfRound", "attachment:N", "categories[]", "style"] {
            assert!(guide.contains(field), "{field} is missing");
        }
        assert_eq!(edits_schema()["properties"]["changes"]["items"]["properties"]["action"]["enum"], json!(["add", "edit", "delete", "move", "quiz"]));
    }
}
