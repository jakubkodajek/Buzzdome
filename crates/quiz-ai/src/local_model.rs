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
use std::collections::HashSet;

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
    /// A video, as frames taken from it, and what is said in it, written
    /// down on this computer (empty when nothing was, or no one speaks).
    Video {
        frames: Vec<String>,
        #[serde(default)]
        words: String,
    },
    /// A sound recording, as what is said in it, written down on this
    /// computer: the models of a local server read no sound.
    Recording { words: String },
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

/// Who said a turn of the panel's conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ChatRole {
    User,
    Assistant,
}

/// A turn of the conversation the quizmaster had with the model about the
/// quiz, which the next reply and the questions follow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ChatTurn {
    pub role: ChatRole,
    pub text: String,
}

/// Most characters of one turn of the conversation sent.
pub const MAX_TURN_CHARS: usize = 8_000;
/// Most characters kept of what was found on the internet during a
/// conversation (`QuestionRequest::notes`): the newest are kept.
pub const MAX_NOTES_CHARS: usize = 14_000;

/// What the quizmaster asks for: the latest message (`prompt`), the files,
/// and the conversation before it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, TS)]
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
    /// The conversation before `prompt`, oldest first; the newest turns
    /// are sent when it is long.
    #[serde(default)]
    pub conversation: Vec<ChatTurn>,
    /// What was found on the internet during the conversation, which the
    /// next replies and the questions are written from too.
    #[serde(default)]
    pub notes: Option<String>,
}

impl QuestionRequest {
    /// What the quizmaster wrote: their turns of the conversation and the
    /// latest message. The web addresses they named are taken from it, and
    /// it is searched when the model gives no plan.
    pub fn request_text(&self) -> String {
        let turns = self.conversation.iter().filter(|turn| turn.role == ChatRole::User).map(|turn| turn.text.as_str());
        turns.chain([self.prompt.as_str()]).map(str::trim).filter(|text| !text.is_empty()).collect::<Vec<_>>().join("\n")
    }
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
         - \"round\": the round's name when the request or the conversation asks for rounds, the same for every question of that round \
         (not numbered per question); empty otherwise.\n\
         - \"wrong\": 1 to 3 wrong but believable answers of the same kind as the right one (all colours when it \
         is a colour, all numbers when it is a number). None repeats the right answer or another wrong one, none \
         is a question. Each is false for this question by the attachments and what was found: never another \
         true answer (another product it also makes, another of its addresses).\n\
         Rules:\n\
         - Write everything in {language}, with correct spelling.\n\
         - Keep every answer under 40 characters.\n\
         - Ask about facts the attachments and the request give. Do not invent facts you are not sure of.\n\
         - Ask what is interesting to know (what it is, does or makes, its story, people, places, dates), not \
         identifiers such as registration, tax, phone or house numbers.\n\
         - Do not ask how many things a picture shows: counting in a picture is often wrong.\n\
         - A wrong answer differs clearly from the right one (not the same street with another number).\n\
         - Questions of a round come together.\n\
         - The questions agree with each other: one never says what another says is wrong.\n\
         - When a conversation with the quizmaster comes before, write the quiz agreed there: its rounds, topics, \
         number and kind of questions.\n\
         - Unless the request or the conversation says how many, write 10 questions."
    )
}

/// What the model is told when the quizmaster talks with it about the quiz
/// before it is written.
fn chat_instructions(language: QuizLanguage) -> String {
    let language = match language {
        QuizLanguage::Cs => "Czech",
        QuizLanguage::En => "English",
    };
    format!(
        "You help a quizmaster plan a quiz in a quiz app. It is played live in a room: questions on a projector, \
         every team answers A, B, C or D on a buzzer.\n\
         Talk with the quizmaster about the quiz: its topic, audience, rounds, how many questions and how hard, and \
         what the attached files and the facts found on the internet offer. Propose a concrete outline: the rounds, \
         each with its topic and number of questions, and a few examples of what you would ask. When something \
         important is unclear, ask about it, at most two questions at a time. Change the outline as the quizmaster \
         says.\n\
         Do not write out all the questions with their answers: when the quizmaster is happy, they press the button \
         that makes the questions, and the questions are then written from this conversation.\n\
         Use only facts you are sure of or that the files or the internet give. Reply in {language}, with correct \
         spelling, briefly, in plain text: short paragraphs and simple lists, no tables and no headings."
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

/// What to look up on the internet for a request: its searches. The web
/// addresses the request names are taken from it as written
/// (`web_page::addresses_in`), never from the model, which guesses them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResearchPlan {
    pub queries: Vec<String>,
}

/// The search made when the model gives no plan (it reasoned past its
/// length limit, say): the request itself, on one line, which a search
/// engine reads well enough [no source - tried by hand on DuckDuckGo].
pub fn plain_search(prompt: &str) -> Option<String> {
    let query: String = one_line(prompt).chars().take(150).collect();
    (!query.is_empty()).then_some(query)
}

/// The request that asks `model` what to look up for `asked`: only the
/// request and the files' names go, so it is quick.
pub fn research_plan_request(model: &LocalModel, asked: &QuestionRequest) -> Value {
    thinking_off(plan_request(model, asked))
}

fn plan_request(model: &LocalModel, asked: &QuestionRequest) -> Value {
    let files: Vec<String> = asked.attachments.iter().map(|a| a.name.replace(['\n', '\r'], " ")).collect();
    let task = format!(
        "{}The quizmaster's request:\n{}\n\nFiles given: {}.",
        conversation_before(asked),
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
                 history, its products), so that it finds that subject and nothing else of the same name. When the files and \
                 general knowledge are enough, give an empty list. Reply with JSON only, at once." },
            { "role": "user", "content": task }
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": { "name": "research", "schema": {
                "type": "object",
                "properties": {
                    "queries": { "type": "array", "items": { "type": "string" }, "maxItems": MAX_SEARCHES }
                },
                "required": ["queries"]
            } }
        },
        "temperature": 0.1,
        "stream": false
    })
}

/// The last turns of the conversation, for the research plan: what the
/// latest message is about may be named only before it.
fn conversation_before(asked: &QuestionRequest) -> String {
    const TURNS: usize = 4;
    const TURN_CHARS: usize = 600;
    let recent = &asked.conversation[asked.conversation.len().saturating_sub(TURNS)..];
    if recent.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = recent
        .iter()
        .map(|turn| {
            let who = if turn.role == ChatRole::User { "Quizmaster" } else { "Assistant" };
            format!("{who}: {}", one_line(&turn.text).chars().take(TURN_CHARS).collect::<String>())
        })
        .collect();
    format!("The conversation so far:\n{}\n\n", lines.join("\n"))
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
    ResearchPlan { queries: list("queries") }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The chat completion request for the questions `asked` of `model`, with
/// `research`, what was found on the internet for it
/// (`web_page::research_text`), and at most `max_text` characters of text
/// files and of the conversation each.
pub fn request(model: &LocalModel, asked: &QuestionRequest, research: Option<&str>, max_text: usize) -> Result<Value, RequestError> {
    let ask = if asked.conversation.is_empty() { "Write questions about the attachments." } else { "Write the questions of the quiz we agreed on above." };
    Ok(json!({
        "model": model.model,
        "messages": messages(instructions(asked.language), asked, research, max_text, ask)?,
        "response_format": {
            "type": "json_schema",
            "json_schema": { "name": "quiz_questions", "schema": reply_schema() }
        },
        // Low, for facts rather than invention.
        "temperature": 0.2,
        "stream": false
    }))
}

/// The chat completion request for the model's next turn of the
/// conversation about the quiz (`chat_instructions`): plain text.
pub fn chat_request(model: &LocalModel, asked: &QuestionRequest, research: Option<&str>, max_text: usize) -> Result<Value, RequestError> {
    Ok(json!({
        "model": model.model,
        "messages": messages(chat_instructions(asked.language), asked, research, max_text, "What quiz could we make from the attachments?")?,
        "temperature": 0.6,
        "stream": false
    }))
}

/// The messages of a request: the instructions `system`, the newest turns
/// of the conversation within `max_text` characters, and the latest
/// message (`ask` when it is empty) with the files and what was found on
/// the internet, before and now.
fn messages(system: String, asked: &QuestionRequest, research: Option<&str>, max_text: usize, ask: &str) -> Result<Vec<Value>, RequestError> {
    if asked.prompt.trim().is_empty() && asked.attachments.is_empty() && asked.conversation.is_empty() {
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
    let mut parts = vec![text_part(if prompt.is_empty() { ask.to_string() } else { prompt })];
    let mut text_left = max_text;
    for (number, attachment) in (1..).zip(&asked.attachments) {
        let name = attachment.name.replace(['\n', '\r'], " ");
        match &attachment.content {
            AttachmentContent::Picture { image } => {
                parts.push(text_part(format!("Attachment {number}: picture \"{name}\".")));
                parts.push(image_part(image)?);
            }
            AttachmentContent::Video { frames, words } => {
                parts.push(text_part(if frames.is_empty() {
                    format!("Attachment {number}: video \"{name}\"; its frames could not be read, so go by its name.")
                } else {
                    format!("Attachment {number}: video \"{name}\"; frames from its start to its end follow.")
                }));
                for frame in frames.iter().take(VIDEO_FRAMES) {
                    parts.push(image_part(frame)?);
                }
                let taken = take_text(words, &mut text_left);
                if !taken.is_empty() {
                    parts.push(text_part(format!("What is said in video \"{name}\" (written down from its sound):\n{taken}")));
                }
            }
            AttachmentContent::Recording { words } => {
                let taken = take_text(words, &mut text_left);
                parts.push(text_part(if taken.is_empty() {
                    format!("Attachment {number}: sound recording \"{name}\"; no speech was heard in it (music or sounds), so go by its name.")
                } else {
                    format!("Attachment {number}: sound recording \"{name}\"; what is said in it, written down from its sound:\n{taken}")
                }));
            }
            AttachmentContent::Text { text } => {
                let taken = take_text(text, &mut text_left);
                parts.push(text_part(format!("Attachment {number}: text \"{name}\":\n{taken}")));
            }

        }
    }
    let found: Vec<&str> = [asked.notes.as_deref(), research].into_iter().flatten().map(str::trim).filter(|text| !text.is_empty()).collect();
    if !found.is_empty() {
        let research = found.join("\n\n");
        // Taken out of what text files left, so the request stays its size.
        let taken: String = research.chars().take(text_left.max(max_text / 2)).collect();
        parts.push(text_part(format!(
            "Found on the internet for this request (search results and the pages read). Questions about the company, person \
             or subject asked for come from what this says; it is not an attachment, so \"attachment\" is 0 for them:\n{taken}"
        )));
    }
    let mut messages = vec![json!({ "role": "system", "content": system })];
    messages.extend(conversation(&asked.conversation, max_text));
    messages.push(json!({ "role": "user", "content": parts }));
    Ok(messages)
}

/// The newest turns of `turns` that fit in `max_chars` characters, each at
/// most `MAX_TURN_CHARS`, oldest first, as chat messages.
fn conversation(turns: &[ChatTurn], max_chars: usize) -> Vec<Value> {
    let mut left = max_chars;
    let mut kept: Vec<Value> = Vec::new();
    for turn in turns.iter().rev() {
        let text: String = turn.text.trim().chars().take(MAX_TURN_CHARS).collect();
        let length = text.chars().count();
        if text.is_empty() {
            continue;
        }
        if length > left {
            break;
        }
        left -= length;
        kept.push(json!({ "role": if turn.role == ChatRole::User { "user" } else { "assistant" }, "content": text }));
    }
    kept.reverse();
    kept
}

/// `notes` from before and what was `found` now, the newest
/// `MAX_NOTES_CHARS` characters, from a line's start.
pub fn notes(before: Option<&str>, found: Option<&str>) -> Option<String> {
    let all: Vec<&str> = [before, found].into_iter().flatten().map(str::trim).filter(|text| !text.is_empty()).collect();
    if all.is_empty() {
        return None;
    }
    let all = all.join("\n\n");
    let count = all.chars().count();
    if count <= MAX_NOTES_CHARS {
        return Some(all);
    }
    let cut: String = all.chars().skip(count - MAX_NOTES_CHARS).collect();
    Some(cut.split_once('\n').map_or(cut.clone(), |(_, rest)| rest.to_string()))
}

fn images(content: &AttachmentContent) -> &[String] {
    match content {
        AttachmentContent::Picture { image } => std::slice::from_ref(image),
        AttachmentContent::Video { frames, .. } => &frames[..frames.len().min(VIDEO_FRAMES)],
        AttachmentContent::Recording { .. } | AttachmentContent::Text { .. } => &[],
    }
}

/// `text`, trimmed, at most what is `left` of the request's text, which it takes.
fn take_text(text: &str, left: &mut usize) -> String {
    let taken: String = text.trim().chars().take(*left).collect();
    *left -= taken.chars().count();
    taken
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
    /// More of the model's text, how many characters it reasoned before
    /// it (a reasoning model's `reasoning` or `reasoning_content`, which
    /// Ollama, llama.cpp and LM Studio send apart from the text), and
    /// whether it stopped there at its length limit.
    Text { text: String, thought: usize, cut_off: bool },
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
    if line.starts_with('{') {
        return ollama_line(line);
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
    let thought = ["/delta/reasoning", "/delta/reasoning_content"].iter().filter_map(|at| choice.pointer(at).and_then(Value::as_str)).map(|t| t.chars().count()).sum();
    StreamLine::Text { text, thought, cut_off: choice.get("finish_reason").and_then(Value::as_str) == Some("length") }
}

/// A line of Ollama's own streamed chat (`/api/chat`, one JSON object a
/// line: `message.content`, `message.thinking`, `done`, `done_reason`), or
/// an OpenAI reply sent whole.
fn ollama_line(line: &str) -> StreamLine {
    let Ok(chunk) = serde_json::from_str::<Value>(line) else { return StreamLine::NotStreamed };
    if chunk.get("error").is_some_and(|e| !e.is_null()) {
        return StreamLine::Failed { detail: error_detail(line) };
    }
    if chunk.get("message").is_none() && chunk.get("done").is_none() {
        return StreamLine::NotStreamed;
    }
    let text = chunk.pointer("/message/content").and_then(Value::as_str).unwrap_or_default().to_string();
    let thought = chunk.pointer("/message/thinking").and_then(Value::as_str).map_or(0, |t| t.chars().count());
    StreamLine::Text { text, thought, cut_off: chunk.get("done_reason").and_then(Value::as_str) == Some("length") }
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

/// Whether a server's refusal `detail` says the request did not fit the
/// model's context, as Ollama ("the prompt is longer than the context
/// length"), llama.cpp ("exceeds the available context size"), LM Studio
/// ("when context the overflows") and vLLM ("maximum context length") say it.
pub fn context_full(detail: &str) -> bool {
    let detail = detail.to_lowercase();
    detail.contains("context") && ["length", "size", "overflow", "window", "n_ctx"].iter().any(|word| detail.contains(word))
}

/// Fields of a request that not every server takes: the reply's JSON
/// Schema, and `reasoning_effort` (`thinking_off`); in Ollama's own form
/// (`for_ollama`), `format` and `think`.
const EXTRAS: [&str; 4] = ["response_format", "reasoning_effort", "format", "think"];

/// Whether a server refused a request for one of its `EXTRAS`, which some
/// servers do not take: the request is then sent without them, and the
/// instructions alone ask for the JSON.
pub fn refuses_extras(detail: &str) -> bool {
    let detail = detail.to_lowercase();
    ["response_format", "json_schema", "structured output", "grammar", "reasoning_effort", "think"].iter().any(|word| detail.contains(word))
}

/// Whether `body` has any of the `EXTRAS`.
pub fn has_extras(body: &Value) -> bool {
    EXTRAS.iter().any(|field| body.get(field).is_some())
}

/// `body` without its `EXTRAS`.
pub fn without_extras(mut body: Value) -> Value {
    if let Some(object) = body.as_object_mut() {
        for field in EXTRAS {
            object.remove(field);
        }
    }
    body
}

/// `body` asking a reasoning model not to reason first. Ollama's
/// OpenAI-compatible API takes `reasoning_effort` and turns a model's
/// thinking off for "none" where the model can (Ollama docs, OpenAI
/// compatibility, https://docs.ollama.com/api/openai-compatibility); other
/// servers and models without reasoning keep their default. A reasoning
/// model's thoughts otherwise take up the context Ollama gives a model by
/// default, 4096 tokens, before it writes a word.
pub fn thinking_off(mut body: Value) -> Value {
    if let Some(object) = body.as_object_mut() {
        object.insert("reasoning_effort".into(), json!("none"));
    }
    body
}

/// The address of Ollama's own API for a server at `url`: `url` without
/// its `/v1`, where Ollama serves the OpenAI-compatible API.
pub fn ollama_root(url: &str) -> String {
    let url = LocalModel::new(url, "").url;
    url.strip_suffix("/v1").unwrap_or(&url).to_string()
}

/// Tokens of context a request to Ollama asks for (`num_ctx`), which
/// Ollama's OpenAI-compatible API cannot set (Ollama docs, OpenAI
/// compatibility, https://docs.ollama.com/api/openai-compatibility) and
/// which is 4096 by default on a computer with less than 24 GiB of
/// graphics memory (https://docs.ollama.com/context-length): too little for
/// a reasoning model, whose thoughts come before its reply.
pub const OLLAMA_CONTEXT: u32 = 16_384;
/// Tokens of context for a request too long for `OLLAMA_CONTEXT`.
pub const OLLAMA_LARGE_CONTEXT: u32 = 32_768;
/// Tokens kept for the model's reasoning and reply [no source - estimate:
/// a reasoning model thought 7,000 to 14,000 characters in our test].
const REPLY_TOKENS: usize = 8_192;
/// Tokens a picture takes [no source - estimate: vision models take some
/// hundreds to about a thousand for a 768 px picture].
const IMAGE_TOKENS: usize = 1_024;

/// The context to ask Ollama for `body` (as `for_ollama` sends it):
/// `OLLAMA_CONTEXT`, or `OLLAMA_LARGE_CONTEXT` when the request and room
/// for the reply need more; never more than the model's own `most`. The
/// same for most requests, as Ollama loads a model again for another.
pub fn ollama_context(body: &Value, most: Option<u32>) -> u32 {
    let messages = body.get("messages").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    let chars: usize = messages.iter().map(|m| m.get("content").and_then(Value::as_str).map_or(0, |t| t.chars().count())).sum();
    let images: usize = messages.iter().map(|m| m.get("images").and_then(Value::as_array).map_or(0, Vec::len)).sum();
    // About 3 characters a token [no source - estimate for Czech and English].
    let needed = chars / 3 + images * IMAGE_TOKENS + REPLY_TOKENS;
    let wanted = if needed <= OLLAMA_CONTEXT as usize { OLLAMA_CONTEXT } else { OLLAMA_LARGE_CONTEXT };
    most.filter(|most| *most >= 2_048).map_or(wanted, |most| wanted.min(most))
}

/// The model's longest context from Ollama's `/api/show` reply
/// (`model_info."<architecture>.context_length"`).
pub fn model_context(show: &Value) -> Option<u32> {
    let info = show.get("model_info")?.as_object()?;
    info.iter().find(|(key, _)| key.ends_with(".context_length")).and_then(|(_, value)| value.as_u64()).map(|n| n.min(u32::MAX as u64) as u32)
}

/// `body`, a chat completion request, in the form of Ollama's own chat
/// API (`/api/chat`), with `context` tokens of context: each message's
/// text as one string and its pictures as base64 (`images`), the JSON
/// Schema as `format`, `reasoning_effort: "none"` as `think: false`
/// (Ollama docs, Chat, https://docs.ollama.com/api/chat).
pub fn for_ollama(body: &Value, context: u32) -> Value {
    let messages: Vec<Value> = body.get("messages").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default().iter().map(ollama_message).collect();
    let mut options = json!({ "num_ctx": context });
    if let Some(temperature) = body.get("temperature") {
        options["temperature"] = temperature.clone();
    }
    let mut native = json!({
        "model": body.get("model").cloned().unwrap_or_default(),
        "messages": messages,
        "options": options,
        "stream": body.get("stream").and_then(Value::as_bool).unwrap_or(false)
    });
    if let Some(schema) = body.pointer("/response_format/json_schema/schema") {
        native["format"] = schema.clone();
    }
    if body.get("reasoning_effort").and_then(Value::as_str) == Some("none") {
        native["think"] = Value::Bool(false);
    }
    native
}

fn ollama_message(message: &Value) -> Value {
    let role = message.get("role").cloned().unwrap_or_else(|| json!("user"));
    let Some(parts) = message.get("content").and_then(Value::as_array) else {
        return json!({ "role": role, "content": message.get("content").and_then(Value::as_str).unwrap_or_default() });
    };
    let texts: Vec<&str> = parts.iter().filter_map(|part| part.get("text").and_then(Value::as_str)).collect();
    let images: Vec<&str> = parts.iter().filter_map(|part| part.pointer("/image_url/url").and_then(Value::as_str)).filter_map(|url| url.split_once(";base64,").map(|(_, data)| data)).collect();
    let mut native = json!({ "role": role, "content": texts.join("\n\n") });
    if !images.is_empty() {
        native["images"] = json!(images);
    }
    native
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

/// The model's turn of the conversation as the panel shows it: its text
/// without a reasoning model's thinking, trimmed.
pub fn said(content: &str) -> String {
    without_thinking(content).trim().to_string()
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
    let rounds = has_rounds(questions);
    for question in questions {
        // A question without a round's name among named rounds stays in the round before it.
        let name = Some(question.round.trim()).filter(|name| !name.is_empty()).or(round).unwrap_or_default();
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

/// Whether the model's `questions` make rounds: some name a round, and
/// rounds hold two questions on average, about. A model sometimes names a
/// round for nearly every question when it was asked for none ("První
/// kolo" … "Sedmé kolo" for 8 questions); such names are no rounds
/// [no source - our rule from a test with a 4B model].
fn has_rounds(questions: &[DraftQuestion]) -> bool {
    let names: HashSet<&str> = questions.iter().map(|q| q.round.trim()).filter(|name| !name.is_empty()).collect();
    !names.is_empty() && names.len() * 2 <= questions.len() + 1
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
        QuestionRequest { prompt: prompt.into(), attachments, language: QuizLanguage::Cs, ..Default::default() }
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
            attachment("klip.mp4", AttachmentContent::Video { frames: vec![PICTURE.into(); VIDEO_FRAMES + 2], words: "Vítejte v Praze.".into() }),
            attachment("rozhovor.mp3", AttachmentContent::Recording { words: "Karlův most stojí od roku 1357.".into() }),
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
        assert!(texts[3].starts_with("What is said in video \"klip.mp4\"") && texts[3].ends_with("Vítejte v Praze."));
        assert!(texts[4].starts_with("Attachment 3: sound recording \"rozhovor.mp3\"") && texts[4].ends_with("od roku 1357."));
        assert!(texts[5].starts_with("Attachment 4: text \"poznámky.txt\":\nVltava"));
        // A recording with no speech is said so, so the model goes by its name.
        let music = request(&model, &asked("x", vec![attachment("song.mp3", AttachmentContent::Recording { words: " ".into() })]), None, MAX_TEXT_CHARS).unwrap();
        assert!(music["messages"][1]["content"][1]["text"].as_str().unwrap().contains("no speech was heard"));
    }

    #[test]
    fn a_request_is_refused_when_it_cannot_be_sent() {
        let model = LocalModel::new(DEFAULT_URL, "m");
        assert_eq!(request(&model, &asked("  ", vec![]), None, MAX_TEXT_CHARS), Err(RequestError::Empty));
        let picture = || attachment("a.png", AttachmentContent::Picture { image: PICTURE.into() });
        assert_eq!(request(&model, &asked("x", vec![picture(); MAX_ATTACHMENTS + 1]), None, MAX_TEXT_CHARS), Err(RequestError::TooManyFiles { max: MAX_ATTACHMENTS }));
        let videos = vec![attachment("v.mp4", AttachmentContent::Video { frames: vec![PICTURE.into(); VIDEO_FRAMES], words: String::new() }); MAX_IMAGES / VIDEO_FRAMES + 1];
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
        assert_eq!(plan, ResearchPlan { queries: vec!["Pekárna Novák Brno".into(), "Novák historie".into(), "a".into()] });
        assert_eq!(plain_search(" Kvíz o firmě\n Bakchus  "), Some("Kvíz o firmě Bakchus".into()));
        assert_eq!(plain_search("  "), None);
        assert!(research_plan("Nevím.").queries.is_empty());
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
        // A question without a round's name stays in the round before it: no second end of round.
        let mut unnamed = questions.clone();
        unnamed.insert(2, question("", "X?", &["a", "b"], 0));
        let kinds_of = |questions: &[DraftQuestion]| -> Vec<bool> { import_tables(&[draft_table(questions, &[], true)], &[], 1).unwrap().slides.into_iter().map(|(_, slide)| matches!(slide.content, SlideContent::EndOfRound { .. })).collect() };
        assert_eq!(kinds_of(&unnamed).iter().filter(|end| **end).count(), 2);
        // Without rounds, a whole quiz still ends with the results.
        let plain = import_tables(&[draft_table(&questions[..1].iter().map(|q| DraftQuestion { round: String::new(), ..q.clone() }).collect::<Vec<_>>(), &[], true)], &[], 1).unwrap();
        assert!(matches!(plain.slides.last().unwrap().1.content, SlideContent::Final { .. }));
    }

    #[test]
    fn a_round_for_nearly_every_question_is_no_round() {
        let names = ["Úvodní kolo", "První kolo", "Druhé kolo", "Třetí kolo", "Čtvrté kolo", "Páté kolo", "Šesté kolo", "Sedmé kolo"];
        let mut questions: Vec<DraftQuestion> = names.iter().map(|name| question(name, "Q?", &["a", "b"], 0)).collect();
        questions[1].round = "Úvodní kolo".into();
        let imported = import_tables(&[draft_table(&questions, &[], false)], &[], 1).unwrap();
        assert!(imported.slides.iter().all(|(_, slide)| matches!(slide.content, SlideContent::Question(_))));
        // Two rounds of two are rounds.
        let four: Vec<DraftQuestion> = ["Logo", "Logo", "Text", "Text"].iter().map(|name| question(name, "Q?", &["a", "b"], 0)).collect();
        assert!(has_rounds(&four));
    }

    #[test]
    fn streamed_lines_give_the_text_and_its_end() {
        assert_eq!(stream_line(r#"data: {"choices":[{"delta":{"content":"{\"q"},"finish_reason":null}]}"#), StreamLine::Text { text: "{\"q".into(), thought: 0, cut_off: false });
        assert_eq!(stream_line(r#"data: {"choices":[{"delta":{"content":"","reasoning":"Hmm, Praha"},"finish_reason":null}]}"#), StreamLine::Text { text: String::new(), thought: 10, cut_off: false });
        assert_eq!(stream_line(r#"data: {"choices":[{"delta":{"reasoning_content":"ok"},"finish_reason":null}]}"#), StreamLine::Text { text: String::new(), thought: 2, cut_off: false });
        assert_eq!(stream_line(r#"data: {"choices":[{"delta":{},"finish_reason":"length"}]}"#), StreamLine::Text { text: String::new(), thought: 0, cut_off: true });
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
        assert!(refuses_extras("'response_format.type' must be 'json_schema' or 'text'"));
        assert!(refuses_extras("Unrecognized request argument supplied: reasoning_effort"));
        assert!(!refuses_extras("model 'x' not found"));
        let model = LocalModel::new(DEFAULT_URL, "m");
        let body = request(&model, &asked("x", vec![]), None, MAX_TEXT_CHARS).unwrap();
        assert!(without_extras(body.clone()).get("response_format").is_none());
        let plain = without_extras(thinking_off(body.clone()));
        assert!(!has_extras(&plain) && has_extras(&body) && has_extras(&thinking_off(plain.clone())));
        assert_eq!(streamed(body)["stream"], true);
    }

    #[test]
    fn model_lists_are_read_sorted() {
        let reply = r#"{"object":"list","data":[{"id":"qwen3-vl:8b"},{"id":"Gemma3:4b"},{"id":"qwen3-vl:8b"},{"x":1}]}"#;
        assert_eq!(read_models(reply), Some(vec!["Gemma3:4b".into(), "qwen3-vl:8b".into()]));
        assert_eq!(read_models("{}"), None);
    }

    fn turn(role: ChatRole, text: &str) -> ChatTurn {
        ChatTurn { role, text: text.into() }
    }

    #[test]
    fn the_conversation_comes_between_the_instructions_and_the_latest_message_newest_kept() {
        let model = LocalModel::new(DEFAULT_URL, "m");
        let old = turn(ChatRole::User, &"staré ".repeat(2_000));
        let asked = QuestionRequest { conversation: vec![old, turn(ChatRole::User, "Kvíz o Praze"), turn(ChatRole::Assistant, "Navrhuji 3 kola po 5 otázkách.")], ..asked("", vec![]) };
        let body = request(&model, &asked, None, 1_000).unwrap();
        let messages = body["messages"].as_array().unwrap();
        let roles: Vec<&str> = messages.iter().map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(roles, ["system", "user", "assistant", "user"], "the oldest turn does not fit");
        assert_eq!(messages[2]["content"], "Navrhuji 3 kola po 5 otázkách.");
        assert_eq!(messages[3]["content"][0]["text"], "Write the questions of the quiz we agreed on above.");
        let chat = chat_request(&model, &QuestionRequest { prompt: "Přidej kolo o mostech".into(), ..asked.clone() }, None, MAX_TEXT_CHARS).unwrap();
        assert!(chat.get("response_format").is_none());
        assert!(chat["messages"][0]["content"].as_str().unwrap().contains("Propose a concrete outline"));
        assert_eq!(request(&model, &QuestionRequest { conversation: vec![], ..asked }, None, MAX_TEXT_CHARS), Err(RequestError::Empty));
    }

    #[test]
    fn the_research_plan_reads_the_last_turns_and_the_request_text_the_quizmasters() {
        let asked = QuestionRequest { conversation: vec![turn(ChatRole::User, "Kvíz o Bakerini z Brna"), turn(ChatRole::Assistant, "Jaká kola?")], ..asked("Přidej kolo o historii", vec![]) };
        let plan = research_plan_request(&LocalModel::new(DEFAULT_URL, "m"), &asked);
        assert!(plan["messages"][1]["content"].as_str().unwrap().contains("Quizmaster: Kvíz o Bakerini z Brna"));
        assert_eq!(asked.request_text(), "Kvíz o Bakerini z Brna\nPřidej kolo o historii");
    }

    #[test]
    fn notes_keep_the_newest_from_a_line_start() {
        assert_eq!(notes(None, None), None);
        assert_eq!(notes(Some("a"), Some("b")).as_deref(), Some("a\n\nb"));
        let old = "x".repeat(MAX_NOTES_CHARS);
        let kept = notes(Some(&old), Some("nové\nřádky")).unwrap();
        assert!(kept.chars().count() <= MAX_NOTES_CHARS && kept.ends_with("nové\nřádky") && !kept.starts_with('x'), "{}", &kept[..20.min(kept.len())]);
    }

    #[test]
    fn a_request_goes_to_ollama_in_its_own_form() {
        let picture = Attachment { name: "a.jpg".into(), media: None, content: AttachmentContent::Picture { image: "data:image/jpeg;base64,AAAA".into() } };
        let body = thinking_off(request(&LocalModel::new(DEFAULT_URL, "gemma3:4b"), &asked("Kvíz", vec![picture]), None, MAX_TEXT_CHARS).unwrap());
        let native = for_ollama(&streamed(body), 16_384);
        assert_eq!(native["options"], json!({ "num_ctx": 16_384, "temperature": 0.2 }));
        assert_eq!((native["think"].clone(), native["stream"].clone()), (json!(false), json!(true)));
        assert_eq!(native["format"], reply_schema());
        let user = &native["messages"][1];
        assert_eq!(user["images"], json!(["AAAA"]));
        assert!(user["content"].as_str().unwrap().starts_with("Kvíz\n\nAttachment 1: picture"));
        assert!(native["messages"][0]["content"].is_string());
        assert!(!has_extras(&without_extras(native.clone())) && has_extras(&native));
        assert_eq!(ollama_root("http://localhost:11434/v1/"), "http://localhost:11434");
        assert_eq!(ollama_root("box:8080"), "http://box:8080");
    }

    #[test]
    fn ollama_is_given_a_context_that_fits_the_request_and_the_model() {
        let body = |chars: usize, images: usize| json!({ "messages": [{ "role": "user", "content": "a".repeat(chars), "images": vec!["x"; images] }] });
        assert_eq!(ollama_context(&body(100, 1), None), OLLAMA_CONTEXT);
        assert_eq!(ollama_context(&body(40_000, 2), None), OLLAMA_LARGE_CONTEXT);
        assert_eq!(ollama_context(&body(40_000, 2), Some(8_192)), 8_192);
        assert_eq!(ollama_context(&body(100, 0), Some(0)), OLLAMA_CONTEXT, "a context too small to be true is not taken");
        let show = json!({ "model_info": { "general.architecture": "gemma3", "gemma3.context_length": 131_072 } });
        assert_eq!(model_context(&show), Some(131_072));
        assert_eq!(model_context(&json!({})), None);
    }

    #[test]
    fn ollamas_own_stream_is_read() {
        assert_eq!(stream_line(r#"{"message":{"role":"assistant","content":"Ahoj","thinking":"Hm"},"done":false}"#), StreamLine::Text { text: "Ahoj".into(), thought: 2, cut_off: false });
        assert_eq!(stream_line(r#"{"message":{"role":"assistant","content":""},"done":true,"done_reason":"length"}"#), StreamLine::Text { text: String::new(), thought: 0, cut_off: true });
        assert_eq!(stream_line(r#"{"error":"model ran out of memory"}"#), StreamLine::Failed { detail: "model ran out of memory".into() });
        assert_eq!(stream_line(r#"{"choices":[{"message":{"content":"{}"}}]}"#), StreamLine::NotStreamed);
    }
}
