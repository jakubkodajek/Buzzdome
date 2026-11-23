//! Studio's AI panel (docs/ai-assistants.md): questions from a model running
//! on the quizmaster's computer, through a server that speaks the OpenAI
//! chat completions API (Ollama, LM Studio, llama.cpp's server). What is
//! sent and how the reply becomes slides is `quiz_ai::local_model`; here the
//! requests are sent, streamed back and cancelled.
//!
//! With the quizmaster's leave (`QuestionRequest::internet`), the model
//! first says what to look up, and the app searches for it and reads the
//! pages (`research`, `quiz_ai::web_page`) before the model writes.
//!
//! A request is refused during a show, and a show that begins cancels the
//! request under way (`cancel_for_show`): a model takes the computer's
//! processor and memory for minutes, which a live show must not compete
//! with. The app stops waiting at once and says so (`LocalModelStoppedForShow`),
//! but the server hears of it only when the app drops the connection at the
//! model's next word: a model that is still reading a long request works on
//! until it writes, at most `FIRST_WORD_TIMEOUT`.

use crate::ai::SavedAiSettings;
use crate::editor;
use crate::error::{AppError, CmdResult};
use crate::{off_ui, refuse_during_show};
use quiz_ai::asking::{self, Halt, Research};
use quiz_ai::local_model::{self, LocalModel, LocalModelInfo, QuestionRequest, ReplyError, RequestError};
use quiz_ai::web_page::{self, Source, Url};
use quiz_core::{import_tables, Slide};
use serde::Serialize;
use serde_json::Value;
use std::io::BufReader;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, PoisonError};
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;

pub use quiz_ai::asking::LocalModelProgress;

/// How long a server may take to answer that it is there.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the list of models may take.
const LIST_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a model may take before it writes: loading a large model and
/// reading pictures on a processor without a graphics card takes minutes.
const FIRST_WORD_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// How long a whole reply may take.
const REPLY_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// Most bytes read of a reply or an error.
const MAX_REPLY_BYTES: u64 = 16 << 20;
/// How often a waiting request looks whether it was cancelled.
const CANCEL_CHECK: Duration = Duration::from_millis(100);

/// How long a web page or a search may take.
const WEB_TIMEOUT: Duration = Duration::from_secs(20);

/// The questions the model wrote, as slides, and what they come from.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AskedQuestions {
    pub slides: Vec<Slide>,
    /// The web pages read for them.
    pub sources: Vec<Source>,
    /// Whether a search or a page the request needed could not be read.
    pub web_failed: bool,
}

/// The request under way, one at a time, and how to cancel it.
#[derive(Default)]
pub struct LocalModelState {
    asking: AtomicBool,
    /// Set to cancel the latest request. Each request has its own, so a
    /// cancelled one whose thread still waits on the server is not asked
    /// on by the next.
    cancelled: Mutex<Arc<AtomicBool>>,
}

impl LocalModelState {
    /// Begins a request, unless one is under way: the mark goes when the
    /// first of the two given back does, and the second cancels it.
    fn begin(&self) -> CmdResult<(Asking<'_>, Arc<AtomicBool>)> {
        if self.asking.swap(true, Ordering::SeqCst) {
            return Err(AppError::LocalModelBusy);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        *self.cancelled.lock().unwrap_or_else(PoisonError::into_inner) = cancelled.clone();
        Ok((Asking(&self.asking), cancelled))
    }

    fn cancel(&self) {
        self.cancelled.lock().unwrap_or_else(PoisonError::into_inner).store(true, Ordering::SeqCst);
    }
}

/// A request under way (`LocalModelState::begin`).
struct Asking<'a>(&'a AtomicBool);

impl Drop for Asking<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Cancels a request to the model as a show begins.
pub(crate) fn cancel_for_show(app: &AppHandle) {
    if let Some(state) = app.try_state::<LocalModelState>() {
        state.cancel();
    }
}

/// `cancelled`, or `LocalModelStoppedForShow` when a show has begun, which
/// is what cancelled it: the quizmaster is told why the panel stopped.
pub(crate) fn stopped_for_show(app: &AppHandle, cancelled: AppError) -> AppError {
    if refuse_during_show(app).is_err() {
        AppError::LocalModelStoppedForShow
    } else {
        cancelled
    }
}

/// The model the AI panel asks (none until the quizmaster chose one) and
/// what a request may carry.
#[tauri::command]
pub fn ai_local_model(saved: State<'_, SavedAiSettings>) -> LocalModelInfo {
    LocalModelInfo::new(saved.get().local_model)
}

/// The models the server at `url` offers.
#[tauri::command]
pub async fn ai_local_models(url: String) -> CmdResult<Vec<String>> {
    off_ui(move || {
        if !LocalModel::usable_url(&url) {
            return Err(AppError::LocalModelBadUrl { url });
        }
        let address = LocalModel::models_url(&url);
        let mut response = agent(&address, LIST_TIMEOUT).get(&address).call().map_err(|e| unreachable(&url, e))?;
        let body = read_body(&mut response, &url)?;
        if !response.status().is_success() {
            return Err(AppError::LocalModelRefused { status: response.status().as_u16(), detail: local_model::error_detail(&body) });
        }
        local_model::read_models(&body).ok_or(AppError::LocalModelNoList { url })
    })
    .await
}

/// Keeps the model the AI panel asks.
#[tauri::command]
pub async fn ai_set_local_model(app: AppHandle, url: String, model: String) -> CmdResult<LocalModel> {
    off_ui(move || {
        let chosen = LocalModel::new(&url, &model);
        if chosen.url.is_empty() || chosen.model.is_empty() {
            return Err(AppError::LocalModelNotChosen);
        }
        if !LocalModel::usable_url(&chosen.url) {
            return Err(AppError::LocalModelBadUrl { url: chosen.url });
        }
        app.state::<SavedAiSettings>().change(|settings| settings.local_model = Some(chosen.clone()))?;
        Ok(chosen)
    })
    .await
}

/// Asks the chosen model for questions and makes them slides of the draft
/// `draft`, for the editor to insert as one change, as a sheet's
/// (`editor_import_sheet`). The pictures and videos the request names must
/// be the draft's already (`editor_add_dropped_media`); one that is not is
/// not shown. `progress` hears how far the model is. Cancelled by
/// `editor_ask_local_model_cancel` or a show that begins: the command
/// ends at once, and the connection closes as the server next writes, so
/// the server stops too.
/// The text of a text file added to the AI panel, sent as its first bytes
/// (at most four for each character a request takes), decoded as the
/// spreadsheet import decodes a CSV (`quiz_file::decode_text`).
#[tauri::command]
pub fn editor_read_text(request: tauri::ipc::Request<'_>) -> CmdResult<String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else { return Err(AppError::system("a text file comes as its bytes")) };
    Ok(quiz_file::decode_text(&bytes[..bytes.len().min(local_model::MAX_TEXT_CHARS * 4)]).into_owned())
}

#[tauri::command]
pub async fn editor_ask_local_model(app: AppHandle, draft: String, asked: QuestionRequest, progress: Channel<LocalModelProgress>) -> CmdResult<AskedQuestions> {
    let state = app.state::<LocalModelState>();
    // Begun first, so a cancel from now on reaches this request.
    let (_asking, cancelled) = state.begin()?;
    refuse_during_show(&app)?;
    let model = app.state::<SavedAiSettings>().get().local_model.ok_or(AppError::LocalModelNotChosen)?;
    if !LocalModel::usable_url(&model.url) {
        return Err(AppError::LocalModelBadUrl { url: model.url });
    }
    // Checked before anything is looked up.
    local_model::request(&model, &asked, None, local_model::MAX_TEXT_CHARS)?;
    let media = editor::draft_media(app.clone(), draft, asked.attachments.iter().map(|a| a.media.clone()).collect()).await?;
    let media = asking::slide_media(&asked.attachments, media);
    if cancelled.load(Ordering::SeqCst) {
        return Err(stopped_for_show(&app, AppError::LocalModelCancelled));
    }
    let whole_quiz = asked.whole_quiz;
    let (content, cut_off, found) = off_ui(move || {
        cancellably(cancelled, move |stop| {
            let tell = |step: LocalModelProgress| {
                let _ = progress.send(step);
            };
            find_and_write(&model, &asked, stop, tell)
        })
    })
    .await
    .map_err(|e| if e == AppError::LocalModelCancelled { stopped_for_show(&app, e) } else { e })?;
    let questions = local_model::read_questions(&content, cut_off).map_err(|e| match e {
        ReplyError::CutOff => AppError::LocalModelCutOff,
        ReplyError::NoQuestions => AppError::LocalModelNoQuestions,
        ReplyError::NotQuestions | ReplyError::NotACompletion => AppError::LocalModelNotQuestions,
    })?;
    let table = local_model::draft_table(&questions, &media, whole_quiz);
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    let imported = import_tables(&[table], &[], seed).map_err(|_| AppError::LocalModelNoQuestions)?;
    Ok(AskedQuestions { slides: imported.slides.into_iter().map(|(_, slide)| slide).collect(), sources: found.sources, web_failed: found.failed })
}

/// Looks up on the internet what `asked` needs when it may
/// (`asking::research`), then asks `model` for the questions
/// (`asking::write_questions`): the model's text, whether it stopped at its
/// length limit, and what was found.
fn find_and_write(model: &LocalModel, asked: &QuestionRequest, stop: &AtomicBool, tell: impl Fn(LocalModelProgress)) -> CmdResult<(String, bool, Research)> {
    let found = if asked.internet {
        // Pages on this computer or the local network only when the request named them, redirects included.
        let typed = web_page::addresses_in(&asked.prompt);
        let plan = |body: Value, thinking: &dyn Fn(LocalModelProgress)| {
            ask(model, body, stop, thinking).inspect_err(|e| {
                if *e != AppError::LocalModelCancelled {
                    crate::app_log::note(&format!("local model: no research plan ({e}); the request is searched as it is"));
                }
            })
        };
        asking::research(model, asked, stop, &tell, plan, |url| fetch_html(url, &typed), |e| *e == AppError::LocalModelCancelled)?
    } else {
        Research::default()
    };
    let (content, cut_off) = asking::write_questions(model, asked, &found, |body| ask(model, body, stop, &tell), |e| matches!(e, AppError::LocalModelContextFull { .. }))?;
    Ok((content, cut_off, found))
}

impl From<RequestError> for AppError {
    fn from(error: RequestError) -> Self {
        match error {
            RequestError::Empty => AppError::LocalModelNothingAsked,
            RequestError::TooManyFiles { max } => AppError::LocalModelTooManyFiles { max },
            RequestError::TooManyPictures { max } => AppError::LocalModelTooManyPictures { max },
            RequestError::NotAPicture => AppError::system("a picture for the model is not a data URL"),
        }
    }
}

impl From<Halt> for AppError {
    fn from(halt: Halt) -> Self {
        match halt {
            Halt::Cancelled => AppError::LocalModelCancelled,
            Halt::ThoughtTooLong => AppError::LocalModelThoughtTooLong,
            Halt::Failed { detail } => AppError::LocalModelFailed { detail },
            Halt::NotQuestions => AppError::LocalModelNotQuestions,
        }
    }
}

/// A client for web pages: a time limit, no redirects of its own
/// (`fetch_html` checks each), and the system's proxy unless `url` is on
/// this computer.
fn web_agent(url: &str) -> ureq::Agent {
    let config = ureq::Agent::config_builder().http_status_as_error(false).max_redirects(0).timeout_global(Some(WEB_TIMEOUT)).user_agent("Mozilla/5.0 (compatible; Buzzdome)");
    let config = if is_this_computer(url) { config.proxy(None) } else { config };
    config.build().into()
}

/// Redirects a page may take, as a browser follows them.
const MAX_REDIRECTS: usize = 5;

/// The page at `url` as HTML, with the address it was read at after its
/// redirects; None for one that is not there, is no web page, or redirects
/// to this computer or the local network when the request did not name that
/// address (`web_page::may_read` against `typed`).
fn fetch_html(url: &str, typed: &[Url]) -> Option<(String, String)> {
    let mut at = Url::parse(url).ok()?;
    for _ in 0..=MAX_REDIRECTS {
        if !web_page::may_read(&at, typed) {
            return None;
        }
        let mut response = web_agent(at.as_str()).get(at.as_str()).header("Accept", "text/html,application/xhtml+xml;q=0.9,*/*;q=0.5").header("Accept-Language", "cs,en;q=0.8").call().ok()?;
        if response.status().is_redirection() {
            let location = response.headers().get("location").and_then(|v| v.to_str().ok())?;
            at = at.join(location).ok().filter(|next| matches!(next.scheme(), "http" | "https"))?;
            continue;
        }
        let content_type = response.headers().get("content-type").and_then(|v| v.to_str().ok()).map(str::to_string);
        if !response.status().is_success() || content_type.as_deref().is_some_and(|t| !t.contains("html") && !t.starts_with("text/")) {
            return None;
        }
        let bytes = response.body_mut().with_config().limit(web_page::MAX_PAGE_BYTES).read_to_vec().ok()?;
        return Some((at.to_string(), web_page::page_html(&bytes, content_type.as_deref())));
    }
    None
}

/// Cancels the request under way; it then fails with `localModelCancelled`.
#[tauri::command]
pub fn editor_ask_local_model_cancel(state: State<'_, LocalModelState>) {
    state.cancel();
}

/// Runs `job` on a thread of its own and waits for it, which ends at once
/// when `cancelled` is set: a server that takes minutes before it writes
/// (loading the model, reading the pictures) does not keep the panel
/// waiting. The job sees `cancelled` too and stops at its next step, or
/// its connection closes when the server next writes or at its time limit;
/// `cancelled` stays set for it, as the next request has its own.
fn cancellably<T: Send + 'static>(cancelled: Arc<AtomicBool>, job: impl FnOnce(&AtomicBool) -> CmdResult<T> + Send + 'static) -> CmdResult<T> {
    let (sender, receiver) = mpsc::channel();
    let stop = cancelled.clone();
    std::thread::spawn(move || {
        let _ = sender.send(job(&stop));
    });
    loop {
        match receiver.recv_timeout(CANCEL_CHECK) {
            Ok(reply) => return reply,
            Err(mpsc::RecvTimeoutError::Timeout) if cancelled.load(Ordering::SeqCst) => return Err(AppError::LocalModelCancelled),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(AppError::system("the request to the model stopped")),
        }
    }
}

/// Sends `body` to `model`'s server as a streamed request and gives back
/// the model's text and whether it stopped at its length limit. A server
/// that refuses the response format is asked again without it.
fn ask(model: &LocalModel, body: Value, cancelled: &AtomicBool, progress: impl Fn(LocalModelProgress)) -> CmdResult<(String, bool)> {
    let address = model.chat_url();
    let agent = agent(&address, FIRST_WORD_TIMEOUT);
    let send = |body: &Value| agent.post(&address).header("Content-Type", "application/json").send(local_model::streamed(body.clone()).to_string()).map_err(|e| unreachable(&model.url, e));
    let mut response = send(&body)?;
    if !response.status().is_success() {
        let detail = local_model::error_detail(&read_body(&mut response, &model.url)?);
        if !local_model::refuses_format(&detail) || body.get("response_format").is_none() {
            return Err(refused(response.status().as_u16(), detail, &model.model));
        }
        response = send(&local_model::without_format(body))?;
        if !response.status().is_success() {
            let detail = local_model::error_detail(&read_body(&mut response, &model.url)?);
            return Err(refused(response.status().as_u16(), detail, &model.model));
        }
    }
    let reader = response.into_body().into_with_config().limit(MAX_REPLY_BYTES).reader();
    asking::read_stream(BufReader::new(reader), cancelled, progress, stream_failed)
}

/// A client for a model's server at `address`: plain HTTP is fine, as the
/// server is usually this computer; a proxy set for the system is not used
/// for this computer's own address.
fn agent(address: &str, first_word: Duration) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        // The address the quizmaster gave, and no other it sends on to.
        .max_redirects(0)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(first_word))
        .timeout_recv_body(Some(REPLY_TIMEOUT));
    let config = if is_this_computer(address) { config.proxy(None) } else { config };
    config.build().into()
}

/// Whether `address` names this computer.
fn is_this_computer(address: &str) -> bool {
    let Ok(uri) = address.parse::<ureq::http::Uri>() else { return false };
    let Some(host) = uri.host() else { return false };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

fn read_body(response: &mut ureq::http::Response<ureq::Body>, url: &str) -> CmdResult<String> {
    response.body_mut().with_config().limit(MAX_REPLY_BYTES).read_to_string().map_err(|e| unreachable(url, e))
}

/// A request that did not get a reply from the server at `url`.
fn unreachable(url: &str, error: ureq::Error) -> AppError {
    match error {
        ureq::Error::Timeout(_) => AppError::LocalModelTimedOut,
        ureq::Error::BadUri(_) | ureq::Error::Http(_) => AppError::LocalModelBadUrl { url: url.to_string() },
        other => AppError::LocalModelUnreachable { url: url.to_string(), detail: other.to_string() },
    }
}

/// The server refused the request; a model it does not have, and a request
/// too long for its context, are said so.
fn refused(status: u16, detail: String, model: &str) -> AppError {
    if status == 404 && detail.to_lowercase().contains("not found") {
        AppError::LocalModelMissing { model: model.to_string() }
    } else if local_model::context_full(&detail) {
        AppError::LocalModelContextFull { model: model.to_string() }
    } else {
        AppError::LocalModelRefused { status, detail }
    }
}

fn stream_failed(error: std::io::Error) -> AppError {
    if error.kind() == std::io::ErrorKind::TimedOut {
        AppError::LocalModelTimedOut
    } else {
        AppError::LocalModelInterrupted { detail: error.to_string() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiz_ai::local_model::{Attachment, AttachmentContent, DEFAULT_URL};
    use quiz_core::QuizLanguage;
    use std::io::{BufRead, Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::AtomicUsize;

    /// A server on this computer that answers each request with the next of
    /// `replies` (status and body) and keeps the requests' bodies.
    fn server(replies: Vec<(u16, &'static str, String)>) -> (String, Arc<Mutex<Vec<String>>>) {
        server_for(|_| replies)
    }

    /// `server`, with replies that may name its own address (`http://127.0.0.1:<port>`).
    fn server_for(replies: impl FnOnce(&str) -> Vec<(u16, &'static str, String)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let replies = replies(&base);
        let url = format!("{base}/v1");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let kept = seen.clone();
        std::thread::spawn(move || {
            for (status, kind, body) in replies {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut length = 0;
                let mut first = String::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if first.is_empty() {
                        first = line.clone();
                    }
                    if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap();
                    }
                    if line == "\r\n" {
                        break;
                    }
                }
                let mut request = vec![0; length];
                reader.read_exact(&mut request).unwrap();
                // A page asked for (no body) is kept as its request line.
                let request = String::from_utf8(request).unwrap();
                kept.lock().unwrap().push(if request.is_empty() { first.trim().to_string() } else { request });
                // A redirect's body is where it points.
                let location = if (300..400).contains(&status) { format!("Location: {body}\r\n") } else { String::new() };
                let head = format!("HTTP/1.1 {status} X\r\nContent-Type: {kind}\r\n{location}Content-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body.as_bytes());
            }
        });
        (url, seen)
    }

    fn sse(texts: &[&str]) -> String {
        let mut body: String = texts.iter().map(|text| format!("data: {}\n\n", serde_json::json!({ "choices": [{ "delta": { "content": text }, "finish_reason": null }] }))).collect();
        body.push_str("data: [DONE]\n\n");
        body
    }

    fn asked() -> QuestionRequest {
        let picture = Attachment { name: "a.jpg".into(), media: Some("media/a.jpg".into()), content: AttachmentContent::Picture { image: "data:image/jpeg;base64,AAAA".into() } };
        QuestionRequest { prompt: "Kvíz".into(), attachments: vec![picture], language: QuizLanguage::Cs, whole_quiz: false, internet: false }
    }

    fn ask_server(url: &str) -> (CmdResult<(String, bool)>, usize) {
        let model = LocalModel::new(url, "gemma3");
        let body = local_model::request(&model, &asked(), None, local_model::MAX_TEXT_CHARS).unwrap();
        let told = Arc::new(AtomicUsize::new(0));
        let counted = told.clone();
        let reply = ask(&model, body, &AtomicBool::new(false), move |_| {
            counted.fetch_add(1, Ordering::SeqCst);
        });
        (reply, told.load(Ordering::SeqCst))
    }

    #[test]
    fn a_streamed_reply_is_put_together_and_its_progress_told() {
        let json = r#"{"questions":[{"question":"Co je na obrázku?","answers":["Most","Řeka"],"file":1}]}"#;
        let (first, rest) = json.split_at(30);
        let (url, seen) = server(vec![(200, "text/event-stream", sse(&[first, rest]))]);
        let (reply, told) = ask_server(&url);
        assert_eq!(reply, Ok((json.to_string(), false)));
        assert!(told >= 1);
        let sent: Value = serde_json::from_str(&seen.lock().unwrap()[0]).unwrap();
        assert_eq!((sent["stream"].as_bool(), sent["model"].as_str()), (Some(true), Some("gemma3")));
        assert!(sent["response_format"].is_object());
    }

    #[test]
    fn a_server_that_refuses_the_format_is_asked_again_without_it() {
        let error = r#"{"error":{"message":"response_format json_schema is not supported"}}"#.to_string();
        let (url, seen) = server(vec![(400, "application/json", error), (200, "text/event-stream", sse(&["[]"]))]);
        assert_eq!(ask_server(&url).0, Ok(("[]".to_string(), false)));
        let sent: Value = serde_json::from_str(&seen.lock().unwrap()[1]).unwrap();
        assert!(sent.get("response_format").is_none());
    }

    #[test]
    fn a_reply_sent_whole_is_read_too() {
        let whole = serde_json::json!({ "choices": [{ "message": { "content": "[]" }, "finish_reason": "length" }] }).to_string();
        let (url, _) = server(vec![(200, "application/json", whole)]);
        assert_eq!(ask_server(&url).0, Ok(("[]".to_string(), true)));
    }

    #[test]
    fn a_missing_model_and_other_refusals_are_told_apart() {
        let missing = r#"{"error":{"message":"model \"gemma3\" not found, try pulling it first"}}"#.to_string();
        let (url, _) = server(vec![(404, "application/json", missing), (500, "text/plain", "out of memory".into())]);
        assert_eq!(ask_server(&url).0, Err(AppError::LocalModelMissing { model: "gemma3".into() }));
        assert_eq!(ask_server(&url).0, Err(AppError::LocalModelRefused { status: 500, detail: "out of memory".into() }));
    }

    #[test]
    fn no_server_is_said_so_with_its_address() {
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{port}/v1");
        assert!(matches!(ask_server(&url).0, Err(AppError::LocalModelUnreachable { url: said, .. }) if said == url));
    }

    #[test]
    fn a_cancel_ends_the_wait_at_once() {
        // A server that accepts but never answers.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/v1", listener.local_addr().unwrap().port());
        let model = LocalModel::new(&url, "m");
        let body = local_model::request(&model, &asked(), None, local_model::MAX_TEXT_CHARS).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancel = cancelled.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            cancel.store(true, Ordering::SeqCst);
        });
        assert_eq!(cancellably(cancelled, move |stop| ask(&model, body, stop, |_| {})), Err(AppError::LocalModelCancelled));
        drop(listener);
    }

    #[test]
    fn a_site_the_request_names_is_read_with_its_telling_pages_before_the_model_writes() {
        let (url, seen) = server_for(|base| {
            let plan = serde_json::json!({ "choices": [{ "message": { "content": "{\"queries\":[],\"sites\":[]}" }, "finish_reason": "stop" }] }).to_string();
            let home = format!(r#"<html><head><title>Pekárna Novák</title></head><body><h1>Pečeme od 1921</h1><a href="{base}/o-nas">O nás</a> <a href="/cenik.pdf">Ceník</a></body></html>"#);
            let about = "<html><title>O nás</title><p>Založil ji Jan Novák v Brně.</p></html>".to_string();
            vec![(200, "application/json", plan), (200, "text/html; charset=utf-8", home), (200, "text/html", about), (200, "text/event-stream", sse(&["[]"]))]
        });
        let base = url.trim_end_matches("/v1").to_string();
        let model = LocalModel::new(&url, "m");
        let asked = QuestionRequest { prompt: format!("Kvíz o firmě podle webu {base}/firma"), attachments: vec![], language: QuizLanguage::Cs, whole_quiz: false, internet: true };
        let steps = Mutex::new(Vec::new());
        let (_, _, found) = find_and_write(&model, &asked, &AtomicBool::new(false), |step| steps.lock().unwrap().push(step)).unwrap();
        assert_eq!(found.sources.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(), ["Pekárna Novák", "O nás"]);
        assert!(!found.failed);
        let text = found.text(local_model::MAX_TEXT_CHARS).unwrap();
        assert!(text.contains("Pečeme od 1921") && text.contains("Založil ji Jan Novák v Brně."), "{text}");
        assert_eq!(steps.lock().unwrap()[0], LocalModelProgress::Planning);
        // Then the questions are asked with what was found.
        let seen = seen.lock().unwrap();
        assert_eq!(seen[1], "GET /firma HTTP/1.1");
        assert_eq!(seen[2], "GET /o-nas HTTP/1.1");
        assert!(seen[3].contains("Založil ji Jan Novák"));
    }

    #[test]
    fn a_redirect_to_the_local_network_is_not_followed_unless_the_request_named_it() {
        let (url, seen) = server_for(|base| {
            let to_router = (302, "text/html", "http://192.168.1.1/admin".to_string());
            let to_itself = (301, "text/html", format!("{base}/novy"));
            vec![to_router, to_itself, (200, "text/html", "<html><title>Nový</title></html>".into())]
        });
        let base = Url::parse(url.trim_end_matches("/v1")).unwrap();
        assert_eq!(fetch_html(base.join("stary").unwrap().as_str(), std::slice::from_ref(&base)), None);
        let (at, html) = fetch_html(base.join("stary").unwrap().as_str(), std::slice::from_ref(&base)).unwrap();
        assert_eq!((at, html.contains("Nový")), (base.join("novy").unwrap().to_string(), true));
        // A page on this computer the request did not name is not read at all.
        assert_eq!(fetch_html(base.join("stary").unwrap().as_str(), &[]), None);
        assert_eq!(seen.lock().unwrap().len(), 3);
    }

    #[test]
    fn a_request_too_long_for_the_context_is_asked_again_shorter() {
        let full = r#"{"error":{"message":"the prompt is longer than the context length currently available to the model"}}"#.to_string();
        let reply = sse(&[r#"{"questions":[]}"#]);
        let (url, seen) = server(vec![(400, "application/json", full.clone()), (200, "text/event-stream", reply), (400, "application/json", full.clone()), (400, "application/json", full)]);
        let model = LocalModel::new(&url, "gemma3:4b");
        let text = Attachment { name: "a.txt".into(), media: None, content: AttachmentContent::Text { text: "slovo ".repeat(5_000) } };
        let asked = QuestionRequest { prompt: "Kvíz".into(), attachments: vec![text], language: QuizLanguage::Cs, whole_quiz: false, internet: false };
        assert!(find_and_write(&model, &asked, &AtomicBool::new(false), |_| {}).is_ok());
        {
            let seen = seen.lock().unwrap();
            assert!(seen[0].len() > local_model::MAX_TEXT_CHARS && seen[1].len() < local_model::MAX_TEXT_CHARS, "{} then {}", seen[0].len(), seen[1].len());
        }
        // Once shortened, a refusal is said so with the model's name.
        assert_eq!(find_and_write(&model, &asked, &AtomicBool::new(false), |_| {}).map(|_| ()), Err(AppError::LocalModelContextFull { model: "gemma3:4b".into() }));
    }

    #[test]
    fn a_cancelled_request_stays_cancelled_and_one_runs_at_a_time() {
        let state = LocalModelState::default();
        let (asking, first) = state.begin().unwrap();
        assert!(matches!(state.begin(), Err(AppError::LocalModelBusy)));
        state.cancel();
        drop(asking);
        // The next request has its own flag: the first, whose thread may
        // still wait on the server, is not asked on again.
        let (_asking, second) = state.begin().unwrap();
        assert!(first.load(Ordering::SeqCst));
        assert!(!second.load(Ordering::SeqCst));
        state.cancel();
        assert!(second.load(Ordering::SeqCst));
    }

    #[test]
    fn this_computer_is_known_by_its_names() {
        for address in ["http://localhost:11434/v1", "http://127.0.0.1:1234/v1", "http://[::1]:8080/v1", "http://LocalHost/v1"] {
            assert!(is_this_computer(address), "{address}");
        }
        for address in ["http://192.168.1.5:11434/v1", "https://example.com/v1", "http://localhost.example.com/v1", "http://localhost:80@evil.example/v1", "not an address"] {
            assert!(!is_this_computer(address), "{address}");
        }
    }

    /// `bytes` in base64 (RFC 4648), for the live test's picture.
    fn base64(bytes: &[u8]) -> String {
        const DIGITS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut text = String::new();
        for chunk in bytes.chunks(3) {
            let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
            for i in 0..4 {
                text.push(if i <= chunk.len() { DIGITS[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
            }
        }
        text
    }

    #[test]
    fn base64_is_as_the_rfc_says() {
        assert_eq!([base64(b""), base64(b"f"), base64(b"fo"), base64(b"foo"), base64(b"foob")], ["", "Zg==", "Zm8=", "Zm9v", "Zm9vYg=="]);
    }

    /// Asks a real model, when `BUZZDOME_LIVE_MODEL` names one (and
    /// `BUZZDOME_LIVE_URL` its server, Ollama's by default), about a
    /// picture and a text, and checks that its reply becomes slides that
    /// show the picture (docs/ai-assistants.md). Does nothing otherwise.
    #[test]
    fn live_model_writes_slides_about_a_picture_and_a_text() {
        let Ok(name) = std::env::var("BUZZDOME_LIVE_MODEL") else { return };
        let url = std::env::var("BUZZDOME_LIVE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());
        let model = LocalModel::new(&url, &name);
        let image = format!("data:image/png;base64,{}", base64(include_bytes!("../icons/128x128.png")));
        let asked = QuestionRequest {
            prompt: "4 otázky: 2 o obrázku (logo aplikace na kvízy) a 2 o textu, ve dvou kolech.".into(),
            attachments: vec![
                Attachment { name: "logo.png".into(), media: Some("media/logo.png".into()), content: AttachmentContent::Picture { image } },
                Attachment { name: "poznamky.txt".into(), media: None, content: AttachmentContent::Text { text: "Vltava je nejdelší řeka v Česku. Měří 430 km a teče Prahou.".into() } },
            ],
            language: QuizLanguage::Cs,
            whole_quiz: true,
            internet: false,
        };
        let told = Arc::new(AtomicUsize::new(0));
        let counted = told.clone();
        let started = std::time::Instant::now();
        let (content, cut_off) = cancellably(Arc::new(AtomicBool::new(false)), move |stop| {
            find_and_write(&model, &asked, stop, move |progress| {
                if let LocalModelProgress::Writing { questions, .. } = progress {
                    counted.store(questions, Ordering::SeqCst);
                }
            })
            .map(|(content, cut_off, _)| (content, cut_off))
        })
        .unwrap();
        println!("model wrote in {:?} ({} questions told):\n{content}", started.elapsed(), told.load(Ordering::SeqCst));
        let questions = local_model::read_questions(&content, cut_off).unwrap();
        let table = local_model::draft_table(&questions, &[Some(("media/logo.png".into(), false)), None], true);
        let imported = import_tables(&[table], &[], 1).unwrap();
        let slides: Vec<Slide> = imported.slides.into_iter().map(|(_, slide)| slide).collect();
        for slide in &slides {
            println!("{:?} picture={:?}", slide.content, slide.picture);
        }
        let questions = slides.iter().filter(|s| matches!(s.content, quiz_core::SlideContent::Question(_))).count();
        assert!(questions >= 2, "{questions} questions");
        assert!(matches!(slides.last().unwrap().content, quiz_core::SlideContent::Final { .. }));
        assert!(told.load(Ordering::SeqCst) >= 1, "the progress was told");
        println!("pictures shown: {}", slides.iter().filter(|s| s.picture.is_some()).count());
    }

    /// With `BUZZDOME_LIVE_MODEL`, `LISTENER_MODEL_ROOT` and
    /// `VOICE_MODEL_ROOT` set (folders the speech recognition model and the
    /// reading voice are installed under), the reading voice says a short
    /// talk, the listener writes it down as the panel does with a
    /// recording, and the real model writes questions about it and about a
    /// video's frames and speech. What was heard and written is printed.
    #[test]
    fn live_model_writes_questions_about_what_a_recording_says() {
        let (Ok(name), Some(heard_root), Some(voice_root)) = (std::env::var("BUZZDOME_LIVE_MODEL"), std::env::var_os("LISTENER_MODEL_ROOT"), std::env::var_os("VOICE_MODEL_ROOT")) else { return };
        let say = |text: &str| -> Vec<f32> {
            let line = voice::Line { question: text.into(), answers: vec![], language: QuizLanguage::Cs, voice: 1, speed: quiz_core::DEFAULT_VOICE_SPEED };
            let clip = voice::engine::Engine::load(std::path::Path::new(&voice_root)).unwrap().render(&line).unwrap();
            voice::listen::resampled(&clip.samples, clip.rate)
        };
        let listener = voice::listen::Listener::load(std::path::Path::new(&heard_root), QuizLanguage::Cs).unwrap();
        let started = std::time::Instant::now();
        let talk = listener.transcribe(&say("Hrad Karlštejn založil Karel Čtvrtý v roce 1348. Měl chránit korunovační klenoty. Leží asi třicet kilometrů od Prahy."), |_| {}).unwrap();
        let video_words = listener.transcribe(&say("Na videu vidíte logo aplikace Buzzdome, oranžové tlačítko v modrém kruhu."), |_| {}).unwrap();
        println!("heard in {:?}:\n  {talk}\n  {video_words}", started.elapsed());
        let url = std::env::var("BUZZDOME_LIVE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());
        let model = LocalModel::new(&url, &name);
        let image = format!("data:image/png;base64,{}", base64(include_bytes!("../icons/128x128.png")));
        let asked = QuestionRequest {
            prompt: "Udělej 4 otázky: 3 z nahrávky a 1 z videa.".into(),
            attachments: vec![
                Attachment { name: "pruvodce.mp3".into(), media: None, content: AttachmentContent::Recording { words: talk } },
                Attachment { name: "logo.mp4".into(), media: Some("media/logo.mp4".into()), content: AttachmentContent::Video { frames: vec![image.clone(), image], words: video_words } },
            ],
            language: QuizLanguage::Cs,
            whole_quiz: false,
            internet: false,
        };
        let started = std::time::Instant::now();
        let (content, cut_off) = cancellably(Arc::new(AtomicBool::new(false)), move |stop| find_and_write(&model, &asked, stop, |_| {}).map(|(content, cut_off, _)| (content, cut_off))).unwrap();
        println!("model wrote in {:?}:\n{content}", started.elapsed());
        let questions = local_model::read_questions(&content, cut_off).unwrap();
        for question in &questions {
            println!("{question:?}");
        }
        assert!(questions.len() >= 2, "{} questions", questions.len());
    }

    /// With `BUZZDOME_LIVE_MODEL` and `BUZZDOME_LIVE_WEB` set, asks a real
    /// model for a quiz about a company that only the internet knows about:
    /// what it looked up, the pages read and the questions are printed.
    #[test]
    fn live_model_looks_up_a_company_on_the_internet() {
        let (Ok(name), Ok(_)) = (std::env::var("BUZZDOME_LIVE_MODEL"), std::env::var("BUZZDOME_LIVE_WEB")) else { return };
        let url = std::env::var("BUZZDOME_LIVE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());
        let model = LocalModel::new(&url, &name);
        let prompt = std::env::var("BUZZDOME_LIVE_PROMPT").unwrap_or_else(|_| "Udělej 5 otázek o firmě Bakerini, pekárně v Brně, podle jejích webových stránek.".into());
        let asked = QuestionRequest { prompt, attachments: vec![], language: QuizLanguage::Cs, whole_quiz: false, internet: true };
        let started = std::time::Instant::now();
        let (content, cut_off, found) = cancellably(Arc::new(AtomicBool::new(false)), move |stop| {
            let tell = |step: LocalModelProgress| println!("{:?} {step:?}", std::time::Instant::now() - started);
            find_and_write(&model, &asked, stop, tell)
        })
        .unwrap();
        println!("in {:?}; pages read: {:?}; a search or page failed: {}", started.elapsed(), found.sources, found.failed);
        println!("found:\n{}\n\nmodel wrote:\n{content}", found.text(local_model::MAX_TEXT_CHARS).as_deref().unwrap_or("(nothing)"));
        let questions = local_model::read_questions(&content, cut_off).unwrap();
        assert!(!questions.is_empty());
        assert!(!found.sources.is_empty(), "nothing was read");
    }

    #[test]
    fn the_default_address_is_this_computer() {
        assert!(is_this_computer(DEFAULT_URL));
    }
}
