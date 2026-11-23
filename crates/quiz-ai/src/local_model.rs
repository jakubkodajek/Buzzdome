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
         Reply with JSON only, as the schema says: {{\"questions\": [...]}}.\n\
         Rules:\n\
         - Write every question, answer and explanation in {language}.\n\
         - Each question has 2 to 4 answers. The FIRST answer is the correct one; the app shuffles them. \
         Exactly one answer is correct, the others are wrong but believable.\n\
         - Keep a question under 150 characters and an answer under 40, so they read well on a projector.\n\
         - Ask about facts the attachments and the request give. Do not invent facts you are not sure of.\n\
         - When a question is about a picture or a video attachment, set \"file\" to that attachment's number so \
         the slide shows it, and ask about what it shows.\n\
         - \"explanation\" is one short sentence for the quizmaster on why the answer is right.\n\
         - When the request asks for rounds, give each question its round's name in \"round\"; questions of a round \
         come together.\n\
         - Unless the request says how many, write 10 questions."
    )
}

/// The JSON Schema the model's reply follows: small, so models of a few
/// billion parameters keep to it.
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
                        "round": { "type": "string" },
                        "question": { "type": "string" },
                        "answers": { "type": "array", "items": { "type": "string" }, "minItems": 2, "maxItems": MAX_EVERYONE_ANSWERS },
                        "explanation": { "type": "string" },
                        "file": { "type": "integer", "minimum": 0 }
                    },
                    "required": ["question", "answers"]
                }
            }
        },
        "required": ["questions"]
    })
}

/// The chat completion request for `asked` of `model`.
pub fn request(model: &LocalModel, asked: &QuestionRequest) -> Result<Value, RequestError> {
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
    let mut text_left = MAX_TEXT_CHARS;
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
        "temperature": 0.7,
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
    fn read(value: &Value) -> Option<DraftQuestion> {
        let field = |name: &str| value.get(name).map_or_else(String::new, text_of);
        let answers = match value.get("answers") {
            Some(Value::Array(answers)) => answers.iter().map(text_of).collect(),
            _ => Vec::new(),
        };
        let file = match value.get("file") {
            Some(Value::Number(n)) => n.as_u64().and_then(|n| usize::try_from(n).ok()).unwrap_or(0),
            Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
            _ => 0,
        };
        value.is_object().then(|| DraftQuestion { round: field("round"), question: field("question"), answers, explanation: field("explanation"), file })
    }

    /// Whether the question can be asked: a text, and a correct (first)
    /// answer with at least one other. A first answer left empty would make
    /// the next one correct, which the model did not mean.
    fn askable(&self) -> bool {
        let answers: Vec<String> = self.answers.iter().map(|a| answer_cell(a)).collect();
        !self.question.trim().is_empty() && answers.first().is_some_and(|a| !a.is_empty()) && answers.iter().filter(|a| !a.is_empty()).count() >= 2
    }
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
    let questions: Vec<DraftQuestion> = questions.into_iter().filter(DraftQuestion::askable).take(MAX_QUESTIONS).collect();
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
        QuestionRequest { prompt: prompt.into(), attachments, language: QuizLanguage::Cs, whole_quiz: false }
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
        let body = request(&model, &asked("Kvíz o Praze", files)).unwrap();
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
        assert_eq!(request(&model, &asked("  ", vec![])), Err(RequestError::Empty));
        let picture = || attachment("a.png", AttachmentContent::Picture { image: PICTURE.into() });
        assert_eq!(request(&model, &asked("x", vec![picture(); MAX_ATTACHMENTS + 1])), Err(RequestError::TooManyFiles { max: MAX_ATTACHMENTS }));
        let videos = vec![attachment("v.mp4", AttachmentContent::Video { frames: vec![PICTURE.into(); VIDEO_FRAMES] }); MAX_IMAGES / VIDEO_FRAMES + 1];
        assert_eq!(request(&model, &asked("x", videos)), Err(RequestError::TooManyPictures { max: MAX_IMAGES }));
        let not_a_picture = attachment("a.png", AttachmentContent::Picture { image: "file:///etc/passwd".into() });
        assert_eq!(request(&model, &asked("x", vec![not_a_picture])), Err(RequestError::NotAPicture));
        // Files alone are a request.
        assert!(request(&model, &asked("", vec![picture()])).is_ok());
    }

    #[test]
    fn long_texts_are_cut_to_what_a_request_takes() {
        let model = LocalModel::new(DEFAULT_URL, "m");
        let long = || attachment("a.txt", AttachmentContent::Text { text: "é".repeat(MAX_TEXT_CHARS) });
        let body = request(&model, &asked("x", vec![long(), long()])).unwrap();
        let sent: usize = body["messages"][1]["content"].as_array().unwrap().iter().filter_map(|p| p["text"].as_str()).map(|t| t.matches('é').count()).sum();
        assert_eq!(sent, MAX_TEXT_CHARS);
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
        assert_eq!(questions, vec![question("", "Rok?", &["1945", "1939", ""], 2), question("", "Záporný soubor?", &["a", "b"], 0)]);
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
        let body = request(&model, &asked("x", vec![])).unwrap();
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
