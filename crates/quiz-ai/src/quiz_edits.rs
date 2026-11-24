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
/// Characters a slide's line takes besides the slide: its number and its end.
const LINE_START: usize = 6;

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
         The quiz's own fields: \"title\", \"theme\" (one of the themes listed), \"settings\", \"sounds\", \"welcome\", \"categories\" (named looks: a slide with \"category\" takes on its style and background).\n\
         Fields of the quiz, of a slide's files and of its style (a field left out keeps its default; colours are \"#rrggbb\"; a \"font\" is one installed on the computer, else the theme's is used):\n{}",
        everyone.fewest,
        everyone.most,
        MAX_ANSWERS,
        [
            ("settings", schema_of::<QuizSettings>()),
            ("sounds", schema_of::<QuizSounds>()),
            ("welcome", schema_of::<Welcome>()),
            ("categories[]", json!({ "type": "object", "properties": { "id": { "type": "string" }, "name": { "type": "string" }, "background": { "const": "as a slide's" }, "style": { "const": "as a slide's" } } })),
            ("style", schema_of::<SlideStyle>()),
            ("video", schema_of::<quiz_core::VideoClip>()),
            ("sound", schema_of::<quiz_core::SoundClip>()),
            ("background", schema_of::<quiz_core::Background>()),
        ]
        .iter()
        .map(|(name, schema)| format!("- {name}: {}", outline(schema, schema, 0)))
        .chain(std::iter::once(sound_moments()))
        .collect::<Vec<_>>()
        .join("\n")
    )
}

/// What each moment of the show (`sounds.slots`) can play: nothing, a
/// sound file of the quiz, or one of the app's own sounds by its id: the
/// effects at the effects' moments, the music at the music's.
fn sound_moments() -> String {
    let name = |slot: quiz_core::SoundSlot| serde_json::to_value(slot).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    let kind = |music: bool| {
        let slots: Vec<quiz_core::SoundSlot> = quiz_core::SoundSlot::ALL.into_iter().filter(|slot| slot.is_music() == music).collect();
        let ids = slots.first().map(|slot| slot.playable().join("|")).unwrap_or_default();
        format!("{} take {ids}", slots.into_iter().map(name).collect::<Vec<_>>().join(", "))
    };
    format!("- sounds.slots.<moment>: \"none\", {{\"file\": file}} or {{\"builtIn\": id}}: {}; {}", kind(false), kind(true))
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

/// The quiz as the model reads it, in at most about `max_chars`
/// characters: its own fields as `quiz.json` writes them, then each slide
/// on a line of its own after its number, from 1, without its id. The
/// selected slide and those nearest it are shown in full while there is
/// room, then in short (`short_slide`); the slides farthest from it are
/// left out when even that does not fit, said by their numbers. Then the
/// slide the quizmaster has selected and the themes the quiz can take.
pub fn quiz_view(quiz: &Quiz, selected: Option<usize>, themes: &[(String, String)], max_chars: usize) -> String {
    let mut own = serde_json::to_value(quiz).expect("a quiz serializes");
    if let Some(fields) = own.as_object_mut() {
        fields.remove("slides");
        fields.remove("schemaVersion");
    }
    let head = format!("The quiz now: {own}");
    let mut left = max_chars.saturating_sub(head.len());
    // Nearest the selected slide first (the start when none is), so what the quizmaster works on is seen best.
    let centre = selected.filter(|index| *index < quiz.slides.len()).unwrap_or(0);
    let mut order: Vec<usize> = (0..quiz.slides.len()).collect();
    order.sort_by_key(|index| index.abs_diff(centre));
    let mut shown: Vec<Option<String>> = vec![None; quiz.slides.len()];
    for index in order {
        let full = slide_json(&quiz.slides[index]).to_string();
        let short = short_slide(&quiz.slides[index]);
        // Its number and the line's end take a few characters too.
        let line = if full.len() + LINE_START <= left { full } else if short.len() + LINE_START <= left { short } else { continue };
        left -= line.len() + LINE_START;
        shown[index] = Some(line);
    }
    let mut lines = vec![head];
    if quiz.slides.is_empty() {
        lines.push("It has no slides yet.".to_string());
    }
    let mut missing: Option<(usize, usize)> = None;
    let tell_missing = |lines: &mut Vec<String>, missing: &mut Option<(usize, usize)>| {
        if let Some((from, to)) = missing.take() {
            lines.push(if from == to { format!("{from}. (not shown: the quiz is too long to show whole)") } else { format!("{from}-{to}. (not shown: the quiz is too long to show whole)") });
        }
    };
    for (number, line) in (1..).zip(&shown) {
        match line {
            Some(line) => {
                tell_missing(&mut lines, &mut missing);
                lines.push(format!("{number}. {line}"));
            }
            None => missing = Some((missing.map_or(number, |(from, _)| from), number)),
        }
    }
    tell_missing(&mut lines, &mut missing);
    if let Some(index) = selected.filter(|index| *index < quiz.slides.len()) {
        lines.push(format!("The quizmaster has slide {} selected.", index + 1));
    }
    if !themes.is_empty() {
        let names: Vec<String> = themes.iter().map(|(id, name)| format!("\"{id}\" ({name})")).collect();
        lines.push(format!("Themes the quiz can take (\"theme\"): {}.", names.join(", ")));
    }
    lines.join("\n")
}

/// Most characters of the installed fonts' names the model is told.
const MAX_FONT_CHARS: usize = 3_000;

/// The fonts installed on this computer, which a style's `font` can name,
/// in at most `MAX_FONT_CHARS` characters; none when there are none.
pub fn fonts_line(fonts: &[String]) -> Option<String> {
    let mut names = String::new();
    for font in fonts {
        if names.len() + font.len() + 2 > MAX_FONT_CHARS {
            break;
        }
        if !names.is_empty() {
            names.push_str(", ");
        }
        names.push_str(font);
    }
    (!names.is_empty()).then(|| format!("Fonts installed on this computer (a style's \"font\"): {names}."))
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
            // A bare list is the changes only when it holds one: a list in prose (slides [1, 2]) is not.
            Value::Array(list) if list.iter().any(|item| item.get("action").is_some()) => (list, String::new()),
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
    // The quiz as the changes so far leave it: a second edit of a slide or
    // of the settings builds on the first.
    let mut working = quiz.clone();
    for change in &edits.changes {
        let changes = &mut resolved.changes;
        let done = match change.action {
            Some(Action::Add) => {
                let slides: Vec<Slide> = change.slides.iter().filter_map(|written| new_slide(written, attachments, &mut shuffle)).collect();
                let place = change.after.or(change.slide).unwrap_or_else(|| selected.map_or(quiz.slides.len(), |index| index + 1));
                let anchor = after(place);
                // Slides added where earlier ones were go after them, in the order the model wrote them.
                let earlier = changes.iter_mut().find_map(|done| match done {
                    QuizChange::InsertSlides { after, slides } if *after == anchor => Some(slides),
                    _ => None,
                });
                match (slides.is_empty(), earlier) {
                    (true, _) => false,
                    (false, Some(earlier)) => {
                        earlier.extend(slides);
                        true
                    }
                    (false, None) => {
                        changes.push(QuizChange::InsertSlides { after: anchor, slides });
                        true
                    }
                }
            }
            Some(Action::Edit) => {
                let index = change.slide.and_then(id_of).filter(|id| !gone.contains(id)).and_then(|id| working.slides.iter().position(|slide| slide.id == id));
                match index.and_then(|i| edited_slide(&working.slides[i], &change.set, attachments, &mut shuffle).map(|edited| (i, edited))) {
                    Some((i, edited)) if edited != working.slides[i] => {
                        working.slides[i] = edited.clone();
                        let replaced = QuizChange::ReplaceSlide { slide: Box::new(edited) };
                        // One change for each slide, however often it is edited.
                        match changes.iter_mut().find(|done| matches!(done, QuizChange::ReplaceSlide { slide } if slide.id == working.slides[i].id)) {
                            Some(earlier) => *earlier = replaced,
                            None => changes.push(replaced),
                        }
                        true
                    }
                    _ => false,
                }
            }
            Some(Action::Delete) => match change.slide.and_then(id_of).filter(|id| gone.insert(id.clone())) {
                Some(id) => {
                    changes.push(QuizChange::RemoveSlide { id });
                    true
                }
                None => false,
            },
            Some(Action::Move) => match (change.slide.and_then(id_of).filter(|id| !gone.contains(id)), change.after) {
                (Some(id), Some(to)) if after(to).as_ref() != Some(&id) => {
                    changes.push(QuizChange::MoveSlide { id, after: after(to) });
                    true
                }
                _ => false,
            },
            Some(Action::Quiz) => quiz_changes(&mut working, &change.set, themes, changes),
            None => false,
        };
        if !done {
            resolved.left_out += 1;
        }
    }
    // A place after a slide deleted is after the nearest slide before it that stays.
    let stays = |id: &str| -> Option<String> {
        let at = quiz.slides.iter().position(|slide| slide.id == id)?;
        quiz.slides[..at].iter().rev().find(|slide| !gone.contains(&slide.id)).map(|slide| slide.id.clone())
    };
    // A slide keeps only a category the quiz has once the changes are made.
    let categories: HashSet<&str> = working.categories.iter().map(|category| category.id.as_str()).collect();
    let known = |slide: &mut Slide| {
        if slide.category.as_deref().is_some_and(|id| !categories.contains(id)) {
            slide.category = None;
        }
    };
    for change in &mut resolved.changes {
        match change {
            QuizChange::InsertSlides { after, slides } => {
                if after.as_ref().is_some_and(|id| gone.contains(id)) {
                    *after = after.as_deref().and_then(stays);
                }
                slides.iter_mut().for_each(known);
            }
            QuizChange::MoveSlide { after, .. } if after.as_ref().is_some_and(|id| gone.contains(id)) => *after = after.as_deref().and_then(stays),
            QuizChange::ReplaceSlide { slide } => known(slide),
            _ => {}
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
    // The categories change first, as in the editor (`ai-changes.ts`), so a slide added finds its new category.
    let categories_first = changes.iter().filter(|c| matches!(c, QuizChange::SetCategories { .. })).chain(changes.iter().filter(|c| !matches!(c, QuizChange::SetCategories { .. })));
    for change in categories_first {
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
                // A new slide takes on the look of the category it names; a background of its own stays.
                let fresh: Vec<Slide> = fresh
                    .into_iter()
                    .map(|slide| match quiz.categories.iter().find(|c| Some(&c.id) == slide.category.as_ref()) {
                        Some(category) => {
                            let background = slide.background.clone();
                            let mut formatted = format_like(slide, category);
                            formatted.background = background.or(formatted.background);
                            formatted
                        }
                        None => slide,
                    })
                    .collect();
                quiz.slides.splice(at..at, fresh);
            }
            QuizChange::ReplaceSlide { slide } => {
                if let Some(had) = quiz.slides.iter_mut().find(|had| had.id == slide.id) {
                    // A slide that takes on another category takes on its look, as in the editor.
                    let category = quiz.categories.iter().find(|c| Some(&c.id) == slide.category.as_ref()).filter(|_| had.category != slide.category);
                    *had = match category {
                        Some(category) => format_like(Slide::clone(slide), category),
                        None => Slide::clone(slide),
                    };
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
            QuizChange::SetCategories { categories } => {
                quiz.categories = categories.clone();
                // A slide whose category is gone forgets it and keeps its look, as `history.ts` `removeCategory` does.
                let kept: HashSet<&str> = categories.iter().map(|category| category.id.as_str()).collect();
                for slide in &mut quiz.slides {
                    if slide.category.as_deref().is_some_and(|id| !kept.contains(id)) {
                        slide.category = None;
                    }
                }
            }
        }
    }
    quiz
}

/// `slide` with the look of `category` (the editor's `formatLike`, spec
/// QZ-10): the category's fill and text looks, a question's clock, and its
/// background when it has one. The slide's picture, video and sound stay.
fn format_like(mut slide: Slide, category: &Category) -> Slide {
    let look = &category.style;
    slide.style.fill = look.fill.clone();
    slide.style.headline = look.headline.clone();
    slide.style.text = look.text.clone();
    slide.style.answers = look.answers.clone();
    if slide.content.slide_type().plays().clock_and_reveal {
        slide.style.timer = look.timer;
    }
    slide.background = category.background.clone().or(slide.background);
    slide.category = Some(category.id.clone());
    slide
}

/// A new slide from what the model wrote; none when the quiz model cannot read it.
fn new_slide(written: &Map<String, Value>, attachments: &[Option<AttachedFile>], shuffle: &mut Shuffler) -> Option<Slide> {
    let mut value = Value::Object(written.clone());
    let fields = value.as_object_mut()?;
    fields.insert("id".into(), json!(""));
    plain_fields(fields, attachments, shuffle, None);
    serde_json::from_value(value).ok().map(playable)
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
    Some(playable(edited))
}

/// `slide` with only the files it can play: each in the quiz's media
/// folder (`is_safe_reference`), a picture or a video but not both, and
/// neither on an end of round or the final.
fn playable(mut slide: Slide) -> Slide {
    let safe = |file: &str| quiz_core::is_safe_reference(file);
    if slide.picture.as_deref().is_some_and(|file| !safe(file)) {
        slide.picture = None;
    }
    if slide.video.as_ref().is_some_and(|clip| !safe(&clip.file)) {
        slide.video = None;
    }
    if slide.sound.as_ref().is_some_and(|clip| !safe(&clip.file)) {
        slide.sound = None;
    }
    if slide.background.as_ref().is_some_and(|background| !safe(&background.file)) {
        slide.background = None;
    }
    if slide.picture.is_some() {
        slide.video = None;
    }
    if !slide.content.slide_type().plays().picture_and_video {
        slide.picture = None;
        slide.video = None;
    }
    slide
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
        // The answers as they will be: those the change sets, or else the slide's own.
        let answers: Vec<String> = fields.get("answers").or_else(|| before.and_then(|b| b.get("answers"))).and_then(Value::as_array).map(|a| a.iter().map(text_of).collect()).unwrap_or_default();
        let read: Vec<Value> = correct.iter().filter_map(|c| correct_answer(c, &answers)).collect();
        if read.is_empty() && !correct.is_empty() {
            // None of them is an answer: the correct answers stay as they are.
            fields.remove("correct");
        } else {
            fields.insert("correct".into(), Value::Array(read));
        }
    }
    let was_question = before.and_then(|b| b.get("type")).and_then(Value::as_str) == Some("question");
    if kind == "question" && !was_question {
        // A new question, or a slide made one, answered as its answers allow when the model gave no mode.
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

/// The quiz's own fields `set` changes, as `QuizChange`s, made in
/// `quiz` too, so a later change builds on them: each value read by the
/// quiz model, the theme only one of `themes`, a category without an id
/// given one and one with an id given before left out. A field changed
/// again replaces its earlier change. Whether anything changed.
fn quiz_changes(quiz: &mut Quiz, set: &Map<String, Value>, themes: &[String], changes: &mut Vec<QuizChange>) -> bool {
    let mut own = serde_json::to_value(&*quiz).expect("a quiz serializes");
    own["slides"] = json!([]);
    let mut patch = set.clone();
    for kept in ["slides", "schemaVersion"] {
        patch.remove(kept);
    }
    if let Some(Value::Array(categories)) = patch.get_mut("categories") {
        let mut taken: HashSet<String> = quiz.categories.iter().map(|c| c.id.clone()).collect();
        let mut listed: HashSet<String> = HashSet::new();
        categories.retain_mut(|category| {
            let Some(fields) = category.as_object_mut() else { return false };
            if fields.get("id").and_then(Value::as_str).is_none_or(str::is_empty) {
                let id = (1..).map(|k| format!("ai-category-{k}")).find(|id| !taken.contains(id)).expect("an id is free");
                taken.insert(id.clone());
                fields.insert("id".into(), json!(id));
            }
            // Slides name their category by id: two with one id would be one.
            listed.insert(fields["id"].as_str().unwrap_or_default().to_string())
        });
    }
    let mut changed = false;
    // Each field is read on its own, so one the quiz model cannot read loses no other.
    for (key, value) in patch {
        let mut one = own.clone();
        merge(&mut one, &json!({ key.clone(): value }));
        let Ok(read) = serde_json::from_value::<Quiz>(one) else { continue };
        let change = match key.as_str() {
            "title" if read.title != quiz.title => QuizChange::SetTitle { title: read.title },
            "theme" => match read.theme.filter(|theme| themes.contains(theme) && quiz.theme.as_ref() != Some(theme)) {
                Some(theme) => QuizChange::SetTheme { theme },
                None => continue,
            },
            "settings" if read.settings != quiz.settings => QuizChange::SetSettings { settings: read.settings },
            "sounds" if read.sounds != quiz.sounds => QuizChange::SetSounds { sounds: read.sounds },
            "welcome" if read.welcome != quiz.welcome => QuizChange::SetWelcome { welcome: read.welcome },
            "categories" if read.categories != quiz.categories => QuizChange::SetCategories { categories: read.categories },
            _ => continue,
        };
        match &change {
            QuizChange::SetTitle { title } => quiz.title = title.clone(),
            QuizChange::SetTheme { theme } => quiz.theme = Some(theme.clone()),
            QuizChange::SetSettings { settings } => quiz.settings = *settings,
            QuizChange::SetSounds { sounds } => quiz.sounds = sounds.clone(),
            QuizChange::SetWelcome { welcome } => quiz.welcome = welcome.clone(),
            QuizChange::SetCategories { categories } => quiz.categories = categories.clone(),
            _ => {}
        }
        match changes.iter_mut().find(|done| std::mem::discriminant(*done) == std::mem::discriminant(&change)) {
            Some(earlier) => *earlier = change,
            None => changes.push(change),
        }
        changed = true;
    }
    changed
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
        let short = quiz_view(&quiz(), None, &[], 330);
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

    #[test]
    fn a_second_edit_of_a_slide_or_the_settings_builds_on_the_first() {
        let done = resolved(json!([
            { "action": "edit", "slide": 2, "set": { "text": "Nový text?" } },
            { "action": "edit", "slide": 2, "set": { "points": 50 } },
            { "action": "quiz", "set": { "settings": { "startOnNext": true } } },
            { "action": "quiz", "set": { "settings": { "nextAfterReveal": true } } },
        ]));
        assert_eq!((done.changes.len(), done.left_out), (2, 0), "one change for the slide, one for the settings: {done:?}");
        let changed = apply(&quiz(), &done.changes);
        let edited = question(&changed.slides[1]);
        assert_eq!((edited.text.as_str(), edited.points), ("Nový text?", 50));
        assert!(changed.settings.start_on_next && changed.settings.next_after_reveal);
    }

    #[test]
    fn slides_added_at_one_place_keep_the_order_written_and_a_deleted_place_is_the_slide_before() {
        let one_by_one = resolved(json!([
            { "action": "add", "after": 2, "slides": [{ "type": "billboard", "title": "První" }] },
            { "action": "add", "after": 2, "slides": [{ "type": "billboard", "title": "Druhý" }] },
        ]));
        let titles = |q: &Quiz| -> Vec<String> { q.slides.iter().map(|s| match &s.content { SlideContent::Billboard { title, .. } => title.clone(), _ => s.id.clone() }).collect() };
        assert_eq!(titles(&apply(&quiz(), &one_by_one.changes)), ["Kolo 1", "q1", "První", "Druhý", "end"]);
        // Slide 2 replaced: deleted, and a new one added after it, in either order.
        for changes in [
            json!([{ "action": "delete", "slide": 2 }, { "action": "add", "after": 2, "slides": [{ "type": "billboard", "title": "Nový" }] }]),
            json!([{ "action": "add", "after": 2, "slides": [{ "type": "billboard", "title": "Nový" }] }, { "action": "delete", "slide": 2 }]),
        ] {
            assert_eq!(titles(&apply(&quiz(), &resolved(changes).changes)), ["Kolo 1", "Nový", "end"]);
        }
    }

    #[test]
    fn correct_answers_named_in_an_edit_are_the_slides_own() {
        let by_text = resolved(json!([{ "action": "edit", "slide": 2, "set": { "correct": ["Olomouc"] } }]));
        let QuizChange::ReplaceSlide { slide } = &by_text.changes[0] else { panic!("{by_text:?}") };
        assert_eq!(question(slide).correct[0].answer, 1);
        // An answer the question does not have changes nothing.
        let unknown = resolved(json!([{ "action": "edit", "slide": 2, "set": { "correct": [{ "answer": 7 }] } }]));
        assert_eq!((unknown.changes.len(), unknown.left_out), (0, 1));
    }

    #[test]
    fn a_slide_made_a_question_without_a_mode_is_answered_as_its_answers_allow() {
        let done = resolved(json!([{ "action": "edit", "slide": 1, "set": { "type": "question", "text": "Kde?", "answers": ["Brno", "Praha"], "correct": [1] } }]));
        let QuizChange::ReplaceSlide { slide } = &done.changes[0] else { panic!("{done:?}") };
        assert_eq!((question(slide).mode, question(slide).correct[0].answer), (AnswerMode::Everyone, 1));
    }

    #[test]
    fn a_slide_keeps_only_files_it_can_play_and_categories_the_quiz_has() {
        let done = resolved(json!([
            { "action": "add", "slides": [
                { "type": "billboard", "title": "A", "picture": "media/a.jpg", "video": { "file": "media/b.mp4" }, "category": "nope" },
                { "type": "billboard", "title": "B", "picture": "https://x.y/a.jpg", "sound": { "file": "../../secret.mp3" } },
            ] },
            { "action": "edit", "slide": 2, "set": { "type": "endOfRound", "title": "Konec" } },
        ]));
        let QuizChange::InsertSlides { slides, .. } = &done.changes[0] else { panic!("{done:?}") };
        assert_eq!((slides[0].picture.as_deref(), slides[0].video.is_some(), slides[0].category.as_deref()), (Some("media/a.jpg"), false, None));
        assert_eq!((slides[1].picture.as_deref(), slides[1].sound.is_some()), (None, false));
        let QuizChange::ReplaceSlide { slide } = &done.changes[1] else { panic!("{done:?}") };
        assert_eq!(slide.picture, None, "an end of round shows no picture");
        // A category the same reply makes is kept.
        let made = resolved(json!([
            { "action": "add", "slides": [{ "type": "billboard", "title": "A", "category": "ai-category-1" }] },
            { "action": "quiz", "set": { "categories": [{ "name": "Filmy" }, { "id": "ai-category-1", "name": "Znovu" }] } },
        ]));
        let changed = apply(&quiz(), &made.changes);
        assert_eq!(changed.categories.len(), 1, "one id is one category");
        assert_eq!(changed.slides[3].category.as_deref(), Some("ai-category-1"));
        // Removing it makes its slides forget it, as the editor does.
        let removed = apply(&changed, &resolve(&changed, &edits(json!([{ "action": "quiz", "set": { "categories": [] } }])), &[], None, &[], 1).changes);
        assert_eq!(removed.slides[3].category, None);
    }

    #[test]
    fn a_long_quiz_is_shown_within_its_budget_nearest_the_selected_slide() {
        let slides = (0..200).map(|i| slide(json!({ "id": format!("q{i}"), "type": "question", "mode": "everyone", "text": format!("Otázka číslo {i} o něčem dost dlouhém, aby se nevešla?"), "answers": ["a", "b"], "correct": [{ "answer": 0 }] }))).collect();
        let long = Quiz::new("Dlouhý", slides);
        for budget in [6_000, 20_000] {
            let view = quiz_view(&long, Some(150), &[], budget);
            assert!(view.len() <= budget + 200, "{} characters for {budget}", view.len());
            assert!(view.contains("\n151. {") && view.contains("Otázka číslo 150 "), "the selected slide is shown in full");
            assert!(view.contains("not shown"), "what does not fit is said");
        }
    }

    #[test]
    fn a_list_in_prose_is_not_the_changes() {
        let read = read_edits(r#"I looked at slides [1, 2]. {"changes":[{"action":"delete","slide":1}],"summary":"x"}"#).unwrap();
        assert_eq!(read.changes.len(), 1);
    }

    /// Every field a quiz file can hold (the quiz model's JSON Schema, all
    /// its definitions) is named in the guide, so the model can change
    /// whatever the editor can: a field added to the format and not to the
    /// guide fails here.
    #[test]
    fn the_guide_names_every_field_of_the_quiz_format() {
        fn names(schema: &Value, into: &mut HashSet<String>) {
            match schema {
                Value::Object(fields) => {
                    if let Some(Value::Object(properties)) = fields.get("properties") {
                        into.extend(properties.keys().cloned());
                    }
                    fields.values().for_each(|value| names(value, into));
                }
                Value::Array(items) => items.iter().for_each(|value| names(value, into)),
                _ => {}
            }
        }
        let mut fields = HashSet::new();
        names(&crate::quiz_schema(), &mut fields);
        // The quiz's own bookkeeping, which the model never sets.
        for own in ["schemaVersion", "slides", "id"] {
            fields.remove(own);
        }
        let guide = format_guide();
        let mut missing: Vec<&String> = fields.iter().filter(|field| !guide.contains(&format!("{field}:")) && !guide.contains(&format!("\"{field}\""))).collect();
        missing.sort();
        assert!(missing.is_empty(), "the guide does not name {missing:?}");
    }

    #[test]
    fn a_slide_given_a_category_takes_on_its_look_as_in_the_editor() {
        let mut with_category = quiz();
        with_category.categories = serde_json::from_value(json!([{ "id": "c-1", "name": "Filmy", "background": { "file": "media/film.jpg" },
            "style": { "fill": { "from": "#000000", "to": "#222222", "kind": "vertical" }, "headline": { "color": "#FFCC00", "font": "Segoe UI", "bold": true }, "timer": "bar" } }])).unwrap();
        let done = resolve(&with_category, &edits(json!([
            { "action": "edit", "slide": 2, "set": { "category": "c-1" } },
            { "action": "add", "after": 0, "slides": [{ "type": "billboard", "title": "Film", "category": "c-1" }] },
        ])), &[], None, &[], 1);
        let changed = apply(&with_category, &done.changes);
        for slide in [&changed.slides[0], &changed.slides[2]] {
            assert_eq!(serde_json::to_value(&slide.style.headline.font).unwrap(), json!("Segoe UI"), "{slide:?}");
            assert_eq!(slide.background.as_ref().map(|b| b.file.as_str()), Some("media/film.jpg"));
        }
        assert!(changed.slides[2].style.timer.is_some(), "a question takes on the clock");
    }
}
