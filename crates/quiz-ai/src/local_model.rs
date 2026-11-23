//! Questions from a model running on the quizmaster's computer (Studio's AI
//! panel, docs/ai-assistants.md): Ollama, LM Studio, llama.cpp's server or
//! any other server that speaks the OpenAI chat completions API. No key and
//! no account: the quizmaster gives the server's address and picks a model.
//!
//! Pure: the app sends the requests. Here are what is sent (`request`), what
//! is made of the reply (`read_reply`) and how its questions become slides:
//! as rows of a sheet with English headings (`draft_table`), which
//! `quiz_core::import_tables` reads as it reads a spreadsheet, so a question
//! from a model gets the same defaults and checks as one from a sheet.

use quiz_core::{QuizLanguage, Table, MAX_EVERYONE_ANSWERS};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

/// Where Ollama, the most common such server, listens.
pub const DEFAULT_URL: &str = "http://localhost:11434/v1";
/// Longest server address kept, in characters.
pub const MAX_URL_CHARS: usize = 500;
/// Longest model name kept, in characters.
pub const MAX_MODEL_CHARS: usize = 200;
/// Longest request the quizmaster may type, in characters.
pub const MAX_PROMPT_CHARS: usize = 4000;
/// Most files a request may carry: a small model loses track of more.
pub const MAX_ATTACHMENTS: usize = 12;
// What a request carries is kept small: a model reads it all within its
// context, which Ollama sets to 4k tokens on a computer with less than 24 GiB
// of graphics memory (https://docs.ollama.com/context-length), and a
// picture takes some hundreds of tokens.
/// Most pictures a request may send, a video's frames included.
pub const MAX_IMAGES: usize = 16;
/// Frames the panel takes from each video for the model to look at.
pub const VIDEO_FRAMES: usize = 3;
/// Longest side of a picture or frame sent, in pixels.
pub const IMAGE_SIDE_PX: u32 = 768;
/// Most bytes of one picture sent, as its data URL.
pub const MAX_IMAGE_BYTES: usize = 3 << 20;
/// Most characters of all text files together.
pub const MAX_TEXT_CHARS: usize = 20_000;
/// Most characters of text files and what was found on the internet
/// together, asked again of a model whose context the request did not fit:
/// Ollama gives a model 4096 tokens on a computer with less than 24 GB of
/// graphics memory (docs.ollama.com/context-length), and this much text
/// leaves room there for the instructions, a few pictures and the reply
/// [no source - estimate: about 3 to 4 characters a token in Czech].
pub const SMALL_TEXT_CHARS: usize = 4_000;
/// Most questions taken from one reply.
pub const MAX_QUESTIONS: usize = 100;

/// What the AI panel needs to know before it asks: the model chosen and
/// what a request may carry, so the panel prepares no more than is sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelInfo {
    /// None until the quizmaster chose a model.
    pub chosen: Option<LocalModel>,
    pub default_url: String,
    pub max_prompt_chars: usize,
    pub max_attachments: usize,
    pub max_images: usize,
    pub video_frames: usize,
    pub image_side_px: u32,
    pub max_image_bytes: usize,
    pub max_text_chars: usize,
}

impl LocalModelInfo {
    pub fn new(chosen: Option<LocalModel>) -> LocalModelInfo {
        LocalModelInfo {
            chosen,
            default_url: DEFAULT_URL.into(),
            max_prompt_chars: MAX_PROMPT_CHARS,
            max_attachments: MAX_ATTACHMENTS,
            max_images: MAX_IMAGES,
            video_frames: VIDEO_FRAMES,
            image_side_px: IMAGE_SIDE_PX,
            max_image_bytes: MAX_IMAGE_BYTES,
            max_text_chars: MAX_TEXT_CHARS,
        }
    }
}

/// The model the quizmaster chose, kept with the other AI settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    /// The server's API address, ending before `/chat/completions`.
    pub url: String,
    /// The model's name as the server lists it.
    pub model: String,
}

impl LocalModel {
    /// The settings as given, trimmed and cut to their longest length; the
    /// address without a trailing slash, and with `http://` when it has no
    /// scheme, as people type `localhost:11434/v1`.
    pub fn new(url: &str, model: &str) -> LocalModel {
        let url: String = url.trim().trim_end_matches('/').chars().take(MAX_URL_CHARS).collect();
        let url = if url.is_empty() || url.contains("://") { url } else { format!("http://{url}") };
        LocalModel { url, model: model.trim().chars().take(MAX_MODEL_CHARS).collect() }
    }

    /// Whether `url` (as `new` tidies it) is an address the app sends to:
    /// `http` or `https`, a host, and no user name or password, which would
    /// be kept in the settings file as plain text.
    pub fn usable_url(url: &str) -> bool {
        let url = LocalModel::new(url, "").url;
        let Some((scheme, rest)) = url.split_once("://") else { return false };
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        // A port, when there is one, is a number: `[::1]:8080`, `box:1234`.
        let port = authority.rsplit_once(']').map_or(authority, |(_, after)| after).split_once(':').map(|(_, port)| port);
        matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https")
            && !authority.is_empty()
            && !authority.contains('@')
            && !authority.starts_with(':')
            && port.is_none_or(|port| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()))
    }

    pub fn models_url(url: &str) -> String {
        format!("{}/models", LocalModel::new(url, "").url)
    }

    pub fn chat_url(&self) -> String {
        format!("{}/chat/completions", self.url)
    }
}

/// What a file the quizmaster gave is to the model.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AttachmentContent {
    /// A picture, as a data URL the panel made smaller.
    Picture { image: String },
    /// A video, as frames taken from it.
    Video { frames: Vec<String> },
    /// A text file's text.
    Text { text: String },
}

/// A file for the model to make questions from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    /// The file's name, which the model is told.
    pub name: String,
    /// The draft's reference to the picture or video, which a question about
    /// it shows; none for a text file.
    pub media: Option<String>,
    pub content: AttachmentContent,
}

/// What the quizmaster asks for.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct QuestionRequest {
    pub prompt: String,
    pub attachments: Vec<Attachment>,
    /// The quiz's language, which the questions are written in.
    pub language: QuizLanguage,
    /// Whether the questions make a whole quiz (an empty one): its rounds
    /// then end with their scores and the quiz with the results.
    pub whole_quiz: bool,
    /// Whether the app may look up on the internet what the request needs
    /// (`research_plan`).
    #[serde(default)]
    pub internet: bool,
}

/// Why a request was not sent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    #[error("nothing asked")]
    Empty,
    #[error("more than {max} files")]
    TooManyFiles { max: usize },
    #[error("more than {max} pictures")]
    TooManyPictures { max: usize },
    #[error("a picture is not a data URL")]
    NotAPicture,
}

/// Why a reply gave no questions.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReplyError {
    /// The server's reply is not a chat completion.
    #[error("not a chat completion")]
    NotACompletion,
    /// The model wrote something that is not the questions asked for.
    #[error("the model's answer is not questions")]
    NotQuestions,
    /// The model stopped at its length limit before the questions ended.
    #[error("the model's answer was cut off")]
    CutOff,
    #[error("the model wrote no questions")]
    NoQuestions,
}

/// What the model is told to do, whatever it is asked.
fn instructions(language: QuizLanguage) -> String {
    let language = match language {
        QuizLanguage::Cs => "Czech",
        QuizLanguage::En => "English",
    };
    format!(
        "You write questions for a live quiz played in a room with buzzers. The questions are shown on a projector and \
         every team answers by pressing A, B, C or D.\n\
         Reply with JSON only, as the schema says: {{\"questions\": [...]}}. For each question, in this order:\n\
         - \"attachment\": the number of the picture or video attachment the question is about, so the slide shows \
         it; 0 when it is about none.\n\
         - \"question\": the question, under 150 characters so it reads well on a projector.\n\
         - \"reason\": one short sentence with the fact that answers it, taken from the attachment or the request \
         (for a picture: what is really seen in it). The quizmaster reads it.\n\
         - \"right\": the one correct answer. It must agree with the reason and with the attachments: what a text \
         attachment says is true.\n\
         - \"round\": the round's name when the request asks for rounds, the same for every question of that round \
         (not numbered per question); empty otherwise.\n\
         - \"wrong\": 1 to 3 wrong but believable answers of the same kind as the right one (all colours when it \
         is a colour, all numbers when it is a number). None repeats the right answer or another wrong one, none \
         is a question.\n\
         Rules:\n\
         - Write everything in {language}, with correct spelling.\n\
         - Keep every answer under 40 characters.\n\
         - Ask about facts the attachments and the request give. Do not invent facts you are not sure of.\n\
         - Ask what is interesting to know (what it is, does or makes, its story, people, places, dates), not \
         identifiers such as registration, tax, phone or house numbers.\n\
         - Do not ask how many things a picture shows: counting in a picture is often wrong.\n\
         - A wrong answer differs clearly from the right one (not the same street with another number).\n\
         - Questions of a round come together.\n\
         - Unless the request says how many, write 10 questions."
    )
}

/// The JSON Schema the model's reply follows: small, so models of a few
/// billion parameters keep to it. A model writes the fields in the order
/// the schema gives them, which is the alphabetical order of their names
/// (`serde_json` sorts them): so the fields are named for a model to write
/// what the question is about, the question, the fact that answers it and
/// only then the right answer (small models mark a wrong answer right far
/// less often when they state the fact first, as in a chain of thought).
pub fn reply_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "questions": {
                "type": "array",
                "minItems": 1,
                "maxItems": MAX_QUESTIONS,
                "items": {
                    "type": "object",
                    "properties": {
                        "attachment": { "type": "integer", "minimum": 0 },
                        "question": { "type": "string" },
                        "reason": { "type": "string" },
                        "right": { "type": "string" },
                        "round": { "type": "string" },
                        "wrong": { "type": "array", "items": { "type": "string" }, "minItems": 1, "maxItems": MAX_EVERYONE_ANSWERS - 1 }
                    },
                    "required": ["attachment", "question", "reason", "right", "wrong"]
                }
            }
        },
        "required": ["questions"]
    })
}

/// Most searches a request leads to.
pub const MAX_SEARCHES: usize = 3;

/// What to look up on the internet for a request: searches and the web
/// addresses it names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResearchPlan {
    pub queries: Vec<String>,
    pub sites: Vec<String>,
}

impl ResearchPlan {
    pub fn is_empty(&self) -> bool {
        self.queries.is_empty() && self.sites.is_empty()
    }
}

/// The request that asks `model` what to look up for `asked`: only the
/// request and the files' names go, so it is quick.
pub fn research_plan_request(model: &LocalModel, asked: &QuestionRequest) -> Value {
    let files: Vec<String> = asked.attachments.iter().map(|a| a.name.replace(['\n', '\r'], " ")).collect();
    let task = format!(
        "The quizmaster's request:\n{}\n\nFiles given: {}.",
        asked.prompt.trim().chars().take(MAX_PROMPT_CHARS).collect::<String>(),
        if files.is_empty() { "none".to_string() } else { files.join(", ") }
    );
    json!({
        "model": model.model,
        "messages": [
            { "role": "system", "content":
                "You prepare the research for a quiz. Read the quizmaster's request. When writing it needs facts from the \
                 internet (a company, an organisation, a person, a place or an event it names, or recent facts), give at most \
                 3 short web search queries in the request's language in \"queries\". Every query names the subject in full, \
                 as the request does (a company's whole name and its town), and adds what to find out (what it does, its \
                 history, its products), so that it finds that subject and nothing else of the same name. Give the web \
                 addresses the request names in \"sites\". When the files and general knowledge are enough, give two empty \
                 lists. Reply with JSON only." },
            { "role": "user", "content": task }
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": { "name": "research", "schema": {
                "type": "object",
                "properties": {
                    "queries": { "type": "array", "items": { "type": "string" }, "maxItems": MAX_SEARCHES },
                    "sites": { "type": "array", "items": { "type": "string" }, "maxItems": MAX_SEARCHES }
                },
                "required": ["queries", "sites"]
            } }
        },
        "temperature": 0.1,
        "stream": false
    })
}

/// What the model said to look up, from its reply `content`: at most
/// `MAX_SEARCHES` of each, without blanks and repeats. A reply that is not
/// the plan is an empty one.
pub fn research_plan(content: &str) -> ResearchPlan {
    let text = without_thinking(content);
    let value = text
        .match_indices('{')
        .take(JSON_STARTS)
        .find_map(|(start, _)| serde_json::Deserializer::from_str(&text[start..]).into_iter::<Value>().next()?.ok().filter(Value::is_object));
    let list = |name: &str| -> Vec<String> {
        let mut items: Vec<String> = Vec::new();
        if let Some(Value::Array(values)) = value.as_ref().and_then(|v| v.get(name)) {
            for item in values.iter().map(text_of).map(|t| one_line(&t)) {
                if !item.is_empty() && items.len() < MAX_SEARCHES && !items.iter().any(|had| had.eq_ignore_ascii_case(&item)) {
                    items.push(item.chars().take(200).collect());
                }
            }
        }
        items
    };
    ResearchPlan { queries: list("queries"), sites: list("sites") }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The chat completion request for `asked` of `model`, with `research`,
/// what was found on the internet for it (`web_page::research_text`).
pub fn request(model: &LocalModel, asked: &QuestionRequest, research: Option<&str>, max_text: usize) -> Result<Value, RequestError> {
    if asked.prompt.trim().is_empty() && asked.attachments.is_empty() {
        return Err(RequestError::Empty);
    }
    if asked.attachments.len() > MAX_ATTACHMENTS {
        return Err(RequestError::TooManyFiles { max: MAX_ATTACHMENTS });
    }
    let images = asked.attachments.iter().map(|a| images(&a.content).len()).sum::<usize>();
    if images > MAX_IMAGES {
        return Err(RequestError::TooManyPictures { max: MAX_IMAGES });
    }
    let prompt: String = asked.prompt.trim().chars().take(MAX_PROMPT_CHARS).collect();
    let mut parts = vec![text_part(if prompt.is_empty() { "Write questions about the attachments.".to_string() } else { prompt })];
    let mut text_left = max_text;
    for (number, attachment) in (1..).zip(&asked.attachments) {
        let name = attachment.name.replace(['\n', '\r'], " ");
        match &attachment.content {
            AttachmentContent::Picture { image } => {
                parts.push(text_part(format!("Attachment {number}: picture \"{name}\".")));
                parts.push(image_part(image)?);
            }
            AttachmentContent::Video { frames } => {
                parts.push(text_part(if frames.is_empty() {
                    format!("Attachment {number}: video \"{name}\"; its frames could not be read, so go by its name.")
                } else {
                    format!("Attachment {number}: video \"{name}\"; frames from its start to its end follow.")
                }));
                for frame in frames.iter().take(VIDEO_FRAMES) {
                    parts.push(image_part(frame)?);
                }
            }
            AttachmentContent::Text { text } => {
                let taken: String = text.chars().take(text_left).collect();
                text_left -= taken.chars().count();
                parts.push(text_part(format!("Attachment {number}: text \"{name}\":\n{taken}")));
            }

        }
    }
    if let Some(research) = research.filter(|r| !r.trim().is_empty()) {
        // Taken out of what text files left, so the request stays its size.
        let taken: String = research.chars().take(text_left.max(max_text / 2)).collect();
        parts.push(text_part(format!(
            "Found on the internet for this request (search results and the pages read). Questions about the company, person \
             or subject asked for come from what this says; it is not an attachment, so \"attachment\" is 0 for them:\n{taken}"
        )));
    }
    Ok(json!({
        "model": model.model,
        "messages": [
            { "role": "system", "content": instructions(asked.language) },
            { "role": "user", "content": parts }
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": { "name": "quiz_questions", "schema": reply_schema() }
        },
        // Low, for facts rather than invention.
        "temperature": 0.2,
        "stream": false
    }))
}

fn images(content: &AttachmentContent) -> &[String] {
    match content {
        AttachmentContent::Picture { image } => std::slice::from_ref(image),
        AttachmentContent::Video { frames } => &frames[..frames.len().min(VIDEO_FRAMES)],
        AttachmentContent::Text { .. } => &[],
    }
}

fn text_part(text: String) -> Value {
    json!({ "type": "text", "text": text })
}

/// A picture for the model, which servers take only as a base64 data URL.
fn image_part(image: &str) -> Result<Value, RequestError> {
    if !image.starts_with("data:image/") || !image.contains(";base64,") || image.len() > MAX_IMAGE_BYTES {
        return Err(RequestError::NotAPicture);
    }
    Ok(json!({ "type": "image_url", "image_url": { "url": image } }))
}

/// A question as the model writes it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DraftQuestion {
    pub round: String,
    pub question: String,
    pub answers: Vec<String>,
    pub explanation: String,
    /// The attachment it is about, counted from 1; 0 for none.
    pub file: usize,
}

impl DraftQuestion {
    /// The question in `value`, read field by field so that one field a
    /// small model wrote oddly (`null`, a number for a text, `"2"` for a
    /// number) loses no more than itself; None for what is not an object.
    /// The schema's fields (`reply_schema`) are read, and also the form a
    /// model that does not keep to it often writes (`answers`, the first
    /// correct; `explanation`; `file`).
    fn read(value: &Value) -> Option<DraftQuestion> {
        let field = |names: &[&str]| names.iter().find_map(|name| value.get(*name).filter(|v| !v.is_null())).map_or_else(String::new, text_of);
        let list = |name: &str| match value.get(name) {
            Some(Value::Array(items)) => items.iter().map(text_of).collect(),
            Some(Value::String(one)) => vec![one.clone()],
            _ => Vec::new(),
        };
        let answers = match value.get("right").filter(|v| !v.is_null()) {
            Some(right) => std::iter::once(text_of(right)).chain(list("wrong")).collect(),
            None => list("answers"),
        };
        let file = match value.get("attachment").or_else(|| value.get("file")) {
            Some(Value::Number(n)) => n.as_u64().and_then(|n| usize::try_from(n).ok()).unwrap_or(0),
            Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
            _ => 0,
        };
        value.is_object().then(|| DraftQuestion { round: round_name(&field(&["round"])), question: field(&["question"]), answers, explanation: field(&["reason", "explanation"]), file })
    }

    /// The question with the answers a quiz can use: none empty, none twice
    /// (whatever the case and spaces), none that is itself a question. None
    /// when it cannot be asked then: no text, its correct (first) answer is
    /// one of those, or no other answer is left.
    fn askable(mut self) -> Option<DraftQuestion> {
        let mut seen: Vec<String> = Vec::new();
        let usable = |answer: &String, seen: &mut Vec<String>| {
            let cell = answer_cell(answer);
            let key = cell.to_lowercase();
            let ok = !cell.is_empty() && !cell.ends_with('?') && !seen.contains(&key);
            seen.push(key);
            ok
        };
        let first_ok = self.answers.first().is_some_and(|first| usable(first, &mut seen));
        let wrong: Vec<String> = self.answers.iter().skip(1).filter(|a| usable(a, &mut seen)).cloned().collect();
        if self.question.trim().is_empty() || !first_ok || wrong.is_empty() {
            return None;
        }
        self.answers.truncate(1);
        self.answers.extend(wrong);
        Some(self)
    }
}

/// A round's name as the model wrote it, without the numbering some
/// models add to each question's ("Geography (2/3)", "Geography 2/3"):
/// a round's questions then share one name, so they make one round.
fn round_name(name: &str) -> String {
    let name = name.trim();
    let in_brackets = name.strip_suffix(')').and_then(|n| n.rsplit_once('(')).filter(|(_, inside)| numbered(inside)).map(|(before, _)| before);
    let at_end = || {
        let start = name.trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == '/').len();
        numbered(&name[start..]).then(|| &name[..start])
    };
    let without = in_brackets.or_else(at_end).unwrap_or(name).trim_end_matches([' ', '-', '–', ':', ',']);
    if without.is_empty() { name } else { without }.to_string()
}

/// Whether `text` is a numbering such as `2/3` or `2 / 3`.
fn numbered(text: &str) -> bool {
    text.split_once('/').is_some_and(|(a, b)| [a, b].iter().all(|n| !n.trim().is_empty() && n.trim().bytes().all(|b| b.is_ascii_digit())))
}

/// A text or a number or a truth as the text it says; anything else as none.
fn text_of(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

/// The questions in the server's `reply` to a chat completion request that
/// was not streamed.
pub fn read_reply(reply: &str) -> Result<Vec<DraftQuestion>, ReplyError> {
    let reply: Value = serde_json::from_str(reply).map_err(|_| ReplyError::NotACompletion)?;
    let choice = reply.get("choices").and_then(|choices| choices.get(0)).ok_or(ReplyError::NotACompletion)?;
    let content = choice.pointer("/message/content").and_then(Value::as_str).ok_or(ReplyError::NotACompletion)?;
    read_questions(content, choice.get("finish_reason").and_then(Value::as_str) == Some("length"))
}

/// The questions the model wrote as `content`; `cut_off` when it stopped
/// at its length limit. Models that do not keep to the schema often wrap
/// the JSON in a code block or a sentence, or give a bare list: those are
/// read too, and a reasoning model's thinking before them is skipped.
/// Questions that cannot be asked (`DraftQuestion::askable`) are left out.
pub fn read_questions(content: &str, cut_off: bool) -> Result<Vec<DraftQuestion>, ReplyError> {
    let questions = match questions_in(content) {
        Some(questions) => questions,
        None if cut_off => return Err(ReplyError::CutOff),
        None => return Err(ReplyError::NotQuestions),
    };
    let questions: Vec<DraftQuestion> = questions.into_iter().filter_map(DraftQuestion::askable).take(MAX_QUESTIONS).collect();
    if questions.is_empty() {
        return Err(ReplyError::NoQuestions);
    }
    Ok(questions)
}

/// What marks the start of a question in the model's text.
const QUESTION_KEY: &str = "\"question\"";

/// How many questions the model began in `content` after its first `from`
/// bytes, which were counted before: the panel counts as the model writes,
/// without reading the whole text again each time.
pub fn questions_begun(content: &str, from: usize) -> usize {
    // A key that the last piece cut in two is counted now.
    let mut start = from.saturating_sub(QUESTION_KEY.len() - 1).min(content.len());
    while !content.is_char_boundary(start) {
        start -= 1;
    }
    content[start..].matches(QUESTION_KEY).count()
}

/// A line of a streamed reply (server-sent events).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamLine {
    /// More of the model's text, and whether it stopped there at its length limit.
    Text { text: String, cut_off: bool },
    /// The end of the reply.
    Done,
    /// A line with nothing for the reply: a comment, a blank, a keep-alive.
    Nothing,
    /// A line that is not a server-sent event: the server did not stream.
    NotStreamed,
    /// The server stopped the reply with an error (`error_detail`), as
    /// Ollama and LM Studio do when the model runs out of memory midway.
    Failed { detail: String },
}

/// Reads a line of a streamed chat completion.
pub fn stream_line(line: &str) -> StreamLine {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') || line.starts_with("event:") || line.starts_with("id:") {
        return StreamLine::Nothing;
    }
    let Some(data) = line.strip_prefix("data:").map(str::trim) else { return StreamLine::NotStreamed };
    if data == "[DONE]" {
        return StreamLine::Done;
    }
    let Ok(chunk) = serde_json::from_str::<Value>(data) else { return StreamLine::Nothing };
    if chunk.get("error").is_some_and(|e| !e.is_null()) {
        return StreamLine::Failed { detail: error_detail(data) };
    }
    let Some(choice) = chunk.get("choices").and_then(|choices| choices.get(0)) else { return StreamLine::Nothing };
    let text = choice.pointer("/delta/content").and_then(Value::as_str).unwrap_or_default().to_string();
    StreamLine::Text { text, cut_off: choice.get("finish_reason").and_then(Value::as_str) == Some("length") }
}

/// The message in a server's error reply, in OpenAI's form
/// (`{"error": {"message": ...}}`) or Ollama's (`{"error": ...}`), or its
/// first line as it is; at most 300 characters.
pub fn error_detail(body: &str) -> String {
    let value: Option<Value> = serde_json::from_str(body).ok();
    let error = value.as_ref().and_then(|v| v.get("error"));
    let message = error.and_then(|e| e.get("message").and_then(Value::as_str).or(e.as_str()));
    let detail = message.unwrap_or_else(|| body.lines().next().unwrap_or_default());
    detail.trim().chars().take(300).collect()
}

/// Whether a server refused a request for its `response_format`, which
/// some servers do not take: the request is then sent without it, and the
/// instructions alone ask for the JSON.
/// Whether a server's refusal `detail` says the request did not fit the
/// model's context, as Ollama ("the prompt is longer than the context
/// length"), llama.cpp ("exceeds the available context size"), LM Studio
/// ("when context the overflows") and vLLM ("maximum context length") say it.
pub fn context_full(detail: &str) -> bool {
    let detail = detail.to_lowercase();
    detail.contains("context") && ["length", "size", "overflow", "window", "n_ctx"].iter().any(|word| detail.contains(word))
}

pub fn refuses_format(detail: &str) -> bool {
    let detail = detail.to_lowercase();
    ["response_format", "json_schema", "structured output", "grammar"].iter().any(|word| detail.contains(word))
}

/// `body` without its `response_format`.
pub fn without_format(mut body: Value) -> Value {
    if let Some(object) = body.as_object_mut() {
        object.remove("response_format");
    }
    body
}

/// `body` as a streamed request.
pub fn streamed(mut body: Value) -> Value {
    body["stream"] = Value::Bool(true);
    body
}

/// Most places in a reply where the questions' JSON is looked for: prose
/// before it rarely has more braces than this.
const JSON_STARTS: usize = 64;

fn questions_in(content: &str) -> Option<Vec<DraftQuestion>> {
    let questions = |value: Value| -> Option<Vec<DraftQuestion>> {
        let list = match value {
            Value::Array(list) => list,
            Value::Object(mut object) => match object.remove("questions")? {
                Value::Array(list) => list,
                _ => return None,
            },
            _ => return None,
        };
        let read: Vec<DraftQuestion> = list.iter().filter_map(DraftQuestion::read).collect();
        (!read.is_empty()).then_some(read)
    };
    let text = without_thinking(content);
    // The first JSON value that holds questions, from where a `{` or `[`
    // begins: so a code block's fences, a sentence around it or a brace in
    // the prose before it does not hide it.
    text.match_indices(['{', '['])
        .take(JSON_STARTS)
        .find_map(|(start, _)| serde_json::Deserializer::from_str(&text[start..]).into_iter::<Value>().next()?.ok().and_then(questions))
}

/// `content` after a reasoning model's thinking (`<think>…</think>`), which
/// some servers leave in the text before the answer.
fn without_thinking(content: &str) -> &str {
    match content.rfind("</think>") {
        Some(end) => &content[end + "</think>".len()..],
        None => content,
    }
}

/// The questions as a sheet that `quiz_core::import_tables` reads: a
/// section opens each named round and, in a whole quiz, an end of round
/// closes it and the results end the quiz. A question shows the picture or
/// video of the attachment it names in `media` (by its number, from 1);
/// another number is left out. Answers are left unmarked, so the import
/// takes the first as the correct one and shuffles them.
pub fn draft_table(questions: &[DraftQuestion], media: &[Option<(String, bool)>], whole_quiz: bool) -> Table {
    const HEADINGS: [&str; 9] = ["Type", "Question", "Answer 1", "Answer 2", "Answer 3", "Answer 4", "Notes", "Picture", "Video"];
    let row = |kind: &str, text: &str| {
        let mut row = vec![String::new(); HEADINGS.len()];
        row[0] = kind.to_string();
        row[1] = cell(text);
        row
    };
    let mut rows = vec![HEADINGS.iter().map(|heading| heading.to_string()).collect::<Vec<_>>()];
    let mut round: Option<&str> = None;
    let rounds = questions.iter().any(|q| !q.round.trim().is_empty());
    for question in questions {
        let name = question.round.trim();
        if rounds && round != Some(name) {
            if whole_quiz && round.is_some() {
                rows.push(row("End of round", ""));
            }
            if !name.is_empty() {
                rows.push(row("Section", name));
            }
            round = Some(name);
        }
        let mut cells = row("Question", question.question.trim());
        let answers = question.answers.iter().map(|a| answer_cell(a)).filter(|a| !a.is_empty());
        for (i, answer) in answers.take(MAX_EVERYONE_ANSWERS).enumerate() {
            cells[2 + i] = answer;
        }
        cells[6] = cell(question.explanation.trim());
        if let Some(Some((reference, video))) = question.file.checked_sub(1).and_then(|i| media.get(i)) {
            cells[if *video { 8 } else { 7 }] = reference.clone();
        }
        rows.push(cells);
    }
    if whole_quiz {
        if rounds {
            rows.push(row("End of round", ""));
        }
        rows.push(row("Results", ""));
    }
    Table { name: String::new(), first_row: 1, rows }
}

/// A cell of model text, which an import reads as it is: on one line (a
/// line break would split a question's title from its text), and with a
/// `'` before what a sheet would read as a formula (`quiz_core::defused`).
fn cell(text: &str) -> String {
    quiz_core::defused(text.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// An answer's cell. Brackets around a cell mark an answer correct in a
/// sheet, so the model's own brackets are taken off.
fn answer_cell(text: &str) -> String {
    let text = text.trim();
    let text = text.strip_prefix('[').and_then(|t| t.strip_suffix(']')).unwrap_or(text);
    cell(text)
}

/// The model names a server lists in its reply to `GET /models`, sorted.
pub fn read_models(reply: &str) -> Option<Vec<String>> {
    let reply: Value = serde_json::from_str(reply).ok()?;
    let mut models: Vec<String> = reply.get("data")?.as_array()?.iter().filter_map(|m| m.get("id")?.as_str().map(str::to_string)).collect();
    models.sort_by_key(|name| name.to_lowercase());
    models.dedup();
    Some(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiz_core::{import_tables, AnswerMode, SlideContent};

    const PICTURE: &str = "data:image/jpeg;base64,/9j/4AAQ";

    fn asked(prompt: &str, attachments: Vec<Attachment>) -> QuestionRequest {
        QuestionRequest { prompt: prompt.into(), attachments, language: QuizLanguage::Cs, whole_quiz: false, internet: false }
    }

    fn attachment(name: &str, content: AttachmentContent) -> Attachment {
        Attachment { name: name.into(), media: None, content }
    }

    fn completion(content: &str, finish: &str) -> String {
        json!({ "choices": [{ "message": { "role": "assistant", "content": content }, "finish_reason": finish }] }).to_string()
    }

    fn question(round: &str, text: &str, answers: &[&str], file: usize) -> DraftQuestion {
        DraftQuestion { round: round.into(), question: text.into(), answers: answers.iter().map(|a| a.to_string()).collect(), explanation: String::new(), file }
    }

    #[test]
    fn settings_are_tidied_as_people_type_them() {
        let model = LocalModel::new("  localhost:11434/v1/ ", " qwen3-vl:8b ");
        assert_eq!(model, LocalModel { url: "http://localhost:11434/v1".into(), model: "qwen3-vl:8b".into() });
        assert_eq!(model.chat_url(), "http://localhost:11434/v1/chat/completions");
        assert_eq!(LocalModel::models_url("https://box:1234/v1"), "https://box:1234/v1/models");
        assert_eq!(LocalModel::new(&"x".repeat(MAX_URL_CHARS + 50), "").url.chars().count(), MAX_URL_CHARS + "http://".len());
    }

    #[test]
    fn a_request_carries_the_schema_the_files_and_the_language() {
        let files = vec![
            attachment("Praha.jpg", AttachmentContent::Picture { image: PICTURE.into() }),
            attachment("klip.mp4", AttachmentContent::Video { frames: vec![PICTURE.into(); VIDEO_FRAMES + 2] }),
            attachment("poznámky.txt", AttachmentContent::Text { text: "Vltava teče Prahou.".into() }),
        ];
        let model = LocalModel::new(DEFAULT_URL, "gemma3");
        let body = request(&model, &asked("Kvíz o Praze", files), None, MAX_TEXT_CHARS).unwrap();
        assert_eq!(body["model"], "gemma3");
        assert_eq!(body["response_format"]["json_schema"]["schema"], reply_schema());
        assert!(body["messages"][0]["content"].as_str().unwrap().contains("in Czech"));
        let parts = body["messages"][1]["content"].as_array().unwrap();
        let images = parts.iter().filter(|p| p["type"] == "image_url").count();
        // The picture and at most VIDEO_FRAMES frames.
        assert_eq!(images, 1 + VIDEO_FRAMES);
        let texts: Vec<&str> = parts.iter().filter_map(|p| p["text"].as_str()).collect();
        assert_eq!(texts[0], "Kvíz o Praze");
        assert!(texts[1].starts_with("Attachment 1: picture \"Praha.jpg\""));
        assert!(texts[2].starts_with("Attachment 2: video \"klip.mp4\""));
        assert!(texts[3].starts_with("Attachment 3: text \"poznámky.txt\":\nVltava"));
    }

    #[test]
    fn a_request_is_refused_when_it_cannot_be_sent() {
        let model = LocalModel::new(DEFAULT_URL, "m");
        assert_eq!(request(&model, &asked("  ", vec![]), None, MAX_TEXT_CHARS), Err(RequestError::Empty));
        let picture = || attachment("a.png", AttachmentContent::Picture { image: PICTURE.into() });
        assert_eq!(request(&model, &asked("x", vec![picture(); MAX_ATTACHMENTS + 1]), None, MAX_TEXT_CHARS), Err(RequestError::TooManyFiles { max: MAX_ATTACHMENTS }));
        let videos = vec![attachment("v.mp4", AttachmentContent::Video { frames: vec![PICTURE.into(); VIDEO_FRAMES] }); MAX_IMAGES / VIDEO_FRAMES + 1];
        assert_eq!(request(&model, &asked("x", videos), None, MAX_TEXT_CHARS), Err(RequestError::TooManyPictures { max: MAX_IMAGES }));
        let not_a_picture = attachment("a.png", AttachmentContent::Picture { image: "file:///etc/passwd".into() });
        assert_eq!(request(&model, &asked("x", vec![not_a_picture]), None, MAX_TEXT_CHARS), Err(RequestError::NotAPicture));
        // Files alone are a request.
        assert!(request(&model, &asked("", vec![picture()]), None, MAX_TEXT_CHARS).is_ok());
    }

    #[test]
    fn long_texts_are_cut_to_what_a_request_takes() {
        let model = LocalModel::new(DEFAULT_URL, "m");
        let long = || attachment("a.txt", AttachmentContent::Text { text: "é".repeat(MAX_TEXT_CHARS) });
        let body = request(&model, &asked("x", vec![long(), long()]), None, MAX_TEXT_CHARS).unwrap();
        let sent: usize = body["messages"][1]["content"].as_array().unwrap().iter().filter_map(|p| p["text"].as_str()).map(|t| t.matches('é').count()).sum();
        assert_eq!(sent, MAX_TEXT_CHARS);
        // Asked again of a model with a small context, files and what was found share less.
        let found = "ü".repeat(MAX_TEXT_CHARS);
        let body = request(&model, &asked("x", vec![long()]), Some(&found), SMALL_TEXT_CHARS).unwrap();
        let count = |c: char| -> usize { body["messages"][1]["content"].as_array().unwrap().iter().filter_map(|p| p["text"].as_str()).map(|t| t.matches(c).count()).sum() };
        assert_eq!((count('é'), count('ü')), (SMALL_TEXT_CHARS, SMALL_TEXT_CHARS / 2));
    }

    #[test]
    fn a_request_too_long_for_the_context_is_known_by_what_servers_say() {
        for said in [
            "the prompt is longer than the context length currently available to the model; shorten the prompt, adjust the context length in settings, or use a model with a longer context length",
            "the request exceeds the available context size, try increasing it",
            "Trying to keep the first 5000 tokens when context the overflows. However, the model is loaded with context length of only 4096 tokens",
            "This model's maximum context length is 8192 tokens. However, you requested 9000 tokens",
        ] {
            assert!(context_full(said), "{said}");
        }
        assert!(!context_full("model 'x' not found") && !context_full("out of memory"));
    }

    #[test]
    fn replies_are_read_also_when_the_model_wraps_its_json() {
        let json = r#"{"questions":[{"question":"Kde je Praha?","answers":["Čechy","Morava"],"file":1}]}"#;
        let expected = vec![question("", "Kde je Praha?", &["Čechy", "Morava"], 1)];
        assert_eq!(read_reply(&completion(json, "stop")), Ok(expected.clone()));
        assert_eq!(read_reply(&completion(&format!("Tady jsou otázky:\n```json\n{json}\n```"), "stop")), Ok(expected.clone()));
        let list = r#"[{"question":"Kde je Praha?","answers":["Čechy","Morava"],"file":1}]"#;
        assert_eq!(read_reply(&completion(list, "stop")), Ok(expected));
        // Blank questions are skipped; none left is no questions.
        assert_eq!(read_reply(&completion(r#"{"questions":[{"question":" ","answers":["a","b"]}]}"#, "stop")), Err(ReplyError::NoQuestions));
        assert_eq!(read_reply(&completion("Nevím.", "stop")), Err(ReplyError::NotQuestions));
        assert_eq!(read_reply(&completion(r#"{"questions":[{"question":"Kde"#, "length")), Err(ReplyError::CutOff));
        assert_eq!(read_reply("<html>"), Err(ReplyError::NotACompletion));
        assert_eq!(read_reply(r#"{"error":"model not found"}"#), Err(ReplyError::NotACompletion));
    }

    #[test]
    fn a_field_written_oddly_loses_only_itself() {
        let reply = r#"{"questions":[
            {"question":"Rok?","answers":[1945, "1939", null],"explanation":null,"round":null,"file":"2"},
            {"question":"Bez odpovědi?","answers":"a"},
            {"question":"Prázdná správná?","answers":["", "Brno", "Praha"]},
            {"question":"Jen jedna?","answers":["Praha", " "]},
            "not a question",
            {"question":"Záporný soubor?","answers":["a","b"],"file":-1}
        ]}"#;
        let questions = read_questions(reply, false).unwrap();
        assert_eq!(questions, vec![question("", "Rok?", &["1945", "1939"], 2), question("", "Záporný soubor?", &["a", "b"], 0)]);
    }

    #[test]
    fn the_schemas_fields_are_read_with_the_reason_before_the_right_answer() {
        // serde_json keeps object keys sorted: the order a model writes them in.
        let item = &reply_schema()["properties"]["questions"]["items"]["properties"];
        let order: Vec<&String> = item.as_object().unwrap().keys().collect();
        assert_eq!(order, ["attachment", "question", "reason", "right", "round", "wrong"]);
        let reply = r#"{"questions":[{"attachment":1,"question":"Jaká barva převládá?","reason":"Tlačítko na obrázku je oranžové.","right":"Oranžová","round":"Logo (1/2)","wrong":["Modrá","modrá ","Oranžová","Co je to?","Zelená"]},
            {"attachment":0,"question":"Kolik měří Vltava?","reason":"Text říká 430 km.","right":"430 km","round":"Logo 2/2","wrong":"300 km"}]}"#;
        let questions = read_questions(reply, false).unwrap();
        assert_eq!(questions[0].answers, ["Oranžová", "Modrá", "Zelená"]);
        assert_eq!((questions[0].file, questions[0].explanation.as_str()), (1, "Tlačítko na obrázku je oranžové."));
        assert_eq!(questions[1].answers, ["430 km", "300 km"]);
        // Both are of one round, whatever numbering the model gave each.
        assert_eq!([questions[0].round.as_str(), questions[1].round.as_str()], ["Logo", "Logo"]);
        // A right answer that is a question, or with no wrong one left, is no question.
        let unusable = r#"{"questions":[{"question":"A?","right":"Proč?","wrong":["b"]},{"question":"B?","right":"a","wrong":["A"]}]}"#;
        assert_eq!(read_questions(unusable, false), Err(ReplyError::NoQuestions));
    }

    #[test]
    fn the_model_says_what_to_look_up_and_what_was_found_goes_with_the_request() {
        let model = LocalModel::new(DEFAULT_URL, "m");
        let plan_body = research_plan_request(&model, &asked("Kvíz o firmě Pekárna Novák z Brna", vec![attachment("logo.png", AttachmentContent::Picture { image: PICTURE.into() })]));
        let task = plan_body["messages"][1]["content"].as_str().unwrap();
        assert!(task.contains("Pekárna Novák") && task.contains("logo.png") && !task.contains("base64"));
        let plan = research_plan(r#"<think>{maybe}</think>{"queries":["Pekárna Novák Brno"," pekárna  novák brno ","","Novák historie","a","b"],"sites":["pekarna-novak.cz"]}"#);
        assert_eq!(plan, ResearchPlan { queries: vec!["Pekárna Novák Brno".into(), "Novák historie".into(), "a".into()], sites: vec!["pekarna-novak.cz".into()] });
        assert!(research_plan("Nevím.").is_empty());
        let body = request(&model, &asked("Kvíz o firmě", vec![]), Some("Page https://pekarna.cz/ — Úvod\nPečeme od 1921."), MAX_TEXT_CHARS).unwrap();
        let parts = body["messages"][1]["content"].as_array().unwrap();
        assert!(parts.last().unwrap()["text"].as_str().unwrap().ends_with("Pečeme od 1921."));
        assert_eq!(request(&model, &asked("Kvíz", vec![]), Some("  "), MAX_TEXT_CHARS).unwrap()["messages"][1]["content"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn round_names_lose_only_their_numbering() {
        for (written, name) in [("Zeměpis (2/3)", "Zeměpis"), ("Zeměpis 2/3", "Zeměpis"), ("Kolo – 1 / 4", "Kolo"), ("Rok 1945", "Rok 1945"), ("1/2", "1/2"), ("Hudba (pop)", "Hudba (pop)"), ("", "")] {
            assert_eq!(round_name(written), name, "{written}");
        }
    }

    #[test]
    fn a_reasoning_models_thinking_and_braces_in_prose_do_not_hide_the_questions() {
        let json = r#"{"questions":[{"question":"A?","answers":["a","b"]}]}"#;
        let thinking = format!("<think>The user wants {{a quiz}}. Maybe {{\"x\": 1}}.</think>\n{json}");
        assert_eq!(read_questions(&thinking, false).unwrap().len(), 1);
        let prose = format!("Here is a set {{of questions}} as asked: {json} Enjoy {{!}}");
        assert_eq!(read_questions(&prose, false).unwrap().len(), 1);
        assert_eq!(read_questions(r#"Answers ["a","b"] then {"questions":[{"question":"A?","answers":["a","b"]}]}"#, false).unwrap().len(), 1);
    }

    #[test]
    fn only_plain_web_addresses_are_used() {
        for good in ["localhost:11434/v1", "http://127.0.0.1:1234/v1", "https://box.local/v1", "http://[::1]:8080/v1"] {
            assert!(LocalModel::usable_url(good), "{good}");
        }
        for bad in ["", "ftp://box/v1", "file:///etc", "http://user:pass@box/v1", "http://localhost:80@evil.example/v1", "http:///v1", "javascript:alert(1)"] {
            assert!(!LocalModel::usable_url(bad), "{bad}");
        }
    }

    #[test]
    fn questions_become_slides_with_the_first_answer_correct() {
        let questions = vec![
            question("", "Které město leží na Vltavě?", &["Praha", "Brno", "Ostrava", "Plzeň", "Olomouc"], 1),
            question("", "Co je na videu?", &["[Most]", "Řeka"], 2),
            question("", "=1+1", &["2", "3"], 7),
        ];
        let media = vec![Some(("media/praha.jpg".to_string(), false)), Some(("media/klip.mp4".to_string(), true))];
        let slides = import_tables(&[draft_table(&questions, &media, false)], &[], 7).unwrap();
        assert!(slides.notes.into_vec().is_empty());
        let slides: Vec<_> = slides.slides.into_iter().map(|(_, slide)| slide).collect();
        assert_eq!(slides.len(), 3);
        let SlideContent::Question(first) = &slides[0].content else { panic!("{:?}", slides[0]) };
        assert_eq!(first.mode, AnswerMode::Everyone);
        // At most four answers, the first one correct wherever the shuffle put it.
        assert_eq!(first.answers.len(), MAX_EVERYONE_ANSWERS);
        assert_eq!(first.correct.len(), 1);
        assert_eq!(first.answers[first.correct[0].answer], "Praha");
        assert_eq!(slides[0].picture.as_deref(), Some("media/praha.jpg"));
        let SlideContent::Question(second) = &slides[1].content else { panic!() };
        assert_eq!(second.answers[second.correct[0].answer], "Most");
        assert_eq!(slides[1].video.as_ref().map(|v| v.file.as_str()), Some("media/klip.mp4"));
        // A formula stays text, and a file the request did not carry is left out.
        let SlideContent::Question(third) = &slides[2].content else { panic!() };
        assert_eq!(third.text, "=1+1");
        assert_eq!((slides[2].picture.as_ref(), slides[2].video.as_ref()), (None, None));
    }

    #[test]
    fn rounds_open_with_a_section_and_a_whole_quiz_ends_with_the_results() {
        let questions = vec![question("Zeměpis", "A?", &["a", "b"], 0), question("Zeměpis", "B?", &["a", "b"], 0), question("Historie", "C?", &["a", "b"], 0)];
        let kinds = |whole| -> Vec<String> {
            let imported = import_tables(&[draft_table(&questions, &[], whole)], &[], 1).unwrap();
            imported
                .slides
                .into_iter()
                .map(|(_, slide)| match slide.content {
                    SlideContent::Billboard { title, .. } => format!("section {title}"),
                    SlideContent::Question(q) => q.text,
                    SlideContent::EndOfRound { .. } => "end".into(),
                    SlideContent::Final { .. } => "results".into(),
                    other => format!("{other:?}"),
                })
                .collect()
        };
        assert_eq!(kinds(false), ["section Zeměpis", "A?", "B?", "section Historie", "C?"]);
        assert_eq!(kinds(true), ["section Zeměpis", "A?", "B?", "end", "section Historie", "C?", "end", "results"]);
        // Without rounds, a whole quiz still ends with the results.
        let plain = import_tables(&[draft_table(&questions[..1].iter().map(|q| DraftQuestion { round: String::new(), ..q.clone() }).collect::<Vec<_>>(), &[], true)], &[], 1).unwrap();
        assert!(matches!(plain.slides.last().unwrap().1.content, SlideContent::Final { .. }));
    }

    #[test]
    fn streamed_lines_give_the_text_and_its_end() {
        assert_eq!(stream_line(r#"data: {"choices":[{"delta":{"content":"{\"q"},"finish_reason":null}]}"#), StreamLine::Text { text: "{\"q".into(), cut_off: false });
        assert_eq!(stream_line(r#"data: {"choices":[{"delta":{},"finish_reason":"length"}]}"#), StreamLine::Text { text: String::new(), cut_off: true });
        assert_eq!(stream_line("data: [DONE]"), StreamLine::Done);
        assert_eq!(stream_line(": keep-alive"), StreamLine::Nothing);
        assert_eq!(stream_line(""), StreamLine::Nothing);
        assert_eq!(stream_line(r#"{"choices":[]}"#), StreamLine::NotStreamed);
        assert_eq!(questions_begun(r#"{"questions":[{"question":"a"},{"question":"#, 0), 2);
        // Counted piece by piece, a key cut between two pieces is counted once.
        let whole = r#"{"questions":[{"question":"a"},{"question":"b"}]}"#;
        let (mut counted, mut from) = (0, 0);
        for end in [20, 33, 37, whole.len()] {
            counted += questions_begun(&whole[..end], from);
            from = end;
        }
        assert_eq!(counted, 2);
        assert_eq!(stream_line(r#"data: {"error":{"message":"model ran out of memory"}}"#), StreamLine::Failed { detail: "model ran out of memory".into() });
        assert_eq!(read_questions(r#"{"questions":[{"question":"A?","answers":["a","b"]}]}"#, false).unwrap().len(), 1);
    }

    #[test]
    fn errors_are_read_in_either_form_and_a_refused_format_is_known() {
        assert_eq!(error_detail(r#"{"error":{"message":"model 'x' not found","type":"invalid_request_error"}}"#), "model 'x' not found");
        assert_eq!(error_detail(r#"{"error":"json_schema is not supported"}"#), "json_schema is not supported");
        assert_eq!(error_detail("Bad Gateway\nmore"), "Bad Gateway");
        assert!(refuses_format("'response_format.type' must be 'json_schema' or 'text'"));
        assert!(!refuses_format("model 'x' not found"));
        let model = LocalModel::new(DEFAULT_URL, "m");
        let body = request(&model, &asked("x", vec![]), None, MAX_TEXT_CHARS).unwrap();
        assert!(without_format(body.clone()).get("response_format").is_none());
        assert_eq!(streamed(body)["stream"], true);
    }

    #[test]
    fn model_lists_are_read_sorted() {
        let reply = r#"{"object":"list","data":[{"id":"qwen3-vl:8b"},{"id":"Gemma3:4b"},{"id":"qwen3-vl:8b"},{"x":1}]}"#;
        assert_eq!(read_models(reply), Some(vec!["Gemma3:4b".into(), "qwen3-vl:8b".into()]));
        assert_eq!(read_models("{}"), None);
    }
}
