//! Studio's AI panel (docs/ai-assistants.md): questions from a model running
//! on the quizmaster's computer, through a server that speaks the OpenAI
//! chat completions API (Ollama, LM Studio, llama.cpp's server). What is
//! sent and how the reply becomes slides is `quiz_ai::local_model`; here the
//! requests are sent, streamed back and cancelled.
//!
//! A request is refused during a show, and a show that begins cancels the
//! request under way: a model takes the computer's processor and memory for
//! minutes, which a live show must not compete with.

use crate::ai::SavedAiSettings;
use crate::editor;
use crate::error::{AppError, CmdResult};
use crate::{off_ui, refuse_during_show};
use quiz_ai::local_model::{self, AttachmentContent, LocalModel, LocalModelInfo, QuestionRequest, ReplyError, RequestError, StreamLine};
use quiz_core::{import_tables, MediaKind, Slide};
use serde::Serialize;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, PoisonError};
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;

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
/// Most characters of a model's reply: far more than `MAX_QUESTIONS`
/// questions take, so a model that keeps writing is stopped.
const MAX_REPLY_CHARS: usize = 400_000;

/// What the panel shows while the model writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelProgress {
    /// Characters the model has written.
    pub characters: usize,
    /// Questions it has begun.
    pub questions: usize,
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
#[tauri::command]
pub async fn editor_ask_local_model(app: AppHandle, draft: String, asked: QuestionRequest, progress: Channel<LocalModelProgress>) -> CmdResult<Vec<Slide>> {
    let state = app.state::<LocalModelState>();
    // Begun first, so a cancel from now on reaches this request.
    let (_asking, cancelled) = state.begin()?;
    refuse_during_show(&app)?;
    let model = app.state::<SavedAiSettings>().get().local_model.ok_or(AppError::LocalModelNotChosen)?;
    if !LocalModel::usable_url(&model.url) {
        return Err(AppError::LocalModelBadUrl { url: model.url });
    }
    let body = local_model::request(&model, &asked).map_err(|e| match e {
        RequestError::Empty => AppError::LocalModelNothingAsked,
        RequestError::TooManyFiles { max } => AppError::LocalModelTooManyFiles { max },
        RequestError::TooManyPictures { max } => AppError::LocalModelTooManyPictures { max },
        RequestError::NotAPicture => AppError::system("a picture for the model is not a data URL"),
    })?;
    let media = editor::draft_media(app.clone(), draft, asked.attachments.iter().map(|a| a.media.clone()).collect()).await?;
    // A slide shows a file as what it is, whatever the attachment says.
    let media: Vec<Option<(String, bool)>> = asked
        .attachments
        .iter()
        .zip(media)
        .map(|(attachment, found)| match (&attachment.content, found.as_deref().and_then(MediaKind::of)) {
            (AttachmentContent::Picture { .. } | AttachmentContent::Video { .. }, Some(kind @ (MediaKind::Image | MediaKind::Video))) => found.map(|reference| (reference, kind == MediaKind::Video)),
            _ => None,
        })
        .collect();
    if cancelled.load(Ordering::SeqCst) {
        return Err(AppError::LocalModelCancelled);
    }
    let (content, cut_off) = off_ui(move || {
        ask_cancellably(model, body, cancelled, move |done| {
            let _ = progress.send(done);
        })
    })
    .await?;
    let questions = local_model::read_questions(&content, cut_off).map_err(|e| match e {
        ReplyError::CutOff => AppError::LocalModelCutOff,
        ReplyError::NoQuestions => AppError::LocalModelNoQuestions,
        ReplyError::NotQuestions | ReplyError::NotACompletion => AppError::LocalModelNotQuestions,
    })?;
    let table = local_model::draft_table(&questions, &media, asked.whole_quiz);
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    let imported = import_tables(&[table], &[], seed).map_err(|_| AppError::LocalModelNoQuestions)?;
    Ok(imported.slides.into_iter().map(|(_, slide)| slide).collect())
}

/// Cancels the request under way; it then fails with `localModelCancelled`.
#[tauri::command]
pub fn editor_ask_local_model_cancel(state: State<'_, LocalModelState>) {
    state.cancel();
}

/// Sends `body` on a thread of its own and waits for its reply, which ends
/// at once when `cancelled` is set: a server that takes minutes before it
/// writes (loading the model, reading the pictures) does not keep the panel
/// waiting. The thread then closes the connection when the server next
/// writes, or at its time limit; `cancelled` stays set for it, as the next
/// request has its own.
fn ask_cancellably(
    model: LocalModel,
    body: Value,
    cancelled: Arc<AtomicBool>,
    progress: impl Fn(LocalModelProgress) + Send + 'static,
) -> CmdResult<(String, bool)> {
    let (sender, receiver) = mpsc::channel();
    let stop = cancelled.clone();
    std::thread::spawn(move || {
        let _ = sender.send(ask(&model, body, &stop, progress));
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
    read_stream(BufReader::new(reader), cancelled, progress)
}

/// Reads a streamed reply line by line, telling `progress` when the model
/// begins a question and every few hundred characters. A server that sends
/// the whole reply at once is read too.
fn read_stream(mut reader: impl BufRead, cancelled: &AtomicBool, progress: impl Fn(LocalModelProgress)) -> CmdResult<(String, bool)> {
    let (mut content, mut cut_off) = (String::new(), false);
    let (mut now, mut told) = (LocalModelProgress { characters: 0, questions: 0 }, LocalModelProgress { characters: 0, questions: 0 });
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line).map_err(stream_failed)?;
        if cancelled.load(Ordering::SeqCst) {
            return Err(AppError::LocalModelCancelled);
        }
        if read == 0 {
            break;
        }
        match local_model::stream_line(&line) {
            StreamLine::Text { text, cut_off: at_limit } => {
                let had = content.len();
                content.push_str(&text);
                cut_off |= at_limit;
                now.characters += text.chars().count();
                now.questions += local_model::questions_begun(&content, had);
                if now.questions != told.questions || now.characters >= told.characters + 200 {
                    progress(now);
                    told = now;
                }
                if now.characters > MAX_REPLY_CHARS {
                    // A model that does not stop: what it wrote is read as cut off.
                    return Ok((content, true));
                }
            }
            StreamLine::Failed { detail } => return Err(AppError::LocalModelFailed { detail }),
            StreamLine::Done => break,
            StreamLine::Nothing => {}
            StreamLine::NotStreamed => {
                let mut whole = line.clone();
                reader.read_to_string(&mut whole).map_err(stream_failed)?;
                let reply: Value = serde_json::from_str(&whole).map_err(|_| AppError::LocalModelNotQuestions)?;
                let choice = reply.pointer("/choices/0").ok_or(AppError::LocalModelNotQuestions)?;
                let text = choice.pointer("/message/content").and_then(Value::as_str).ok_or(AppError::LocalModelNotQuestions)?;
                return Ok((text.to_string(), choice.get("finish_reason").and_then(Value::as_str) == Some("length")));
            }
        }
    }
    Ok((content, cut_off))
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

/// The server refused the request; a model it does not have is said so.
fn refused(status: u16, detail: String, model: &str) -> AppError {
    if status == 404 && detail.to_lowercase().contains("not found") {
        AppError::LocalModelMissing { model: model.to_string() }
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
    use quiz_ai::local_model::{Attachment, DEFAULT_URL};
    use quiz_core::QuizLanguage;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::AtomicUsize;

    /// A server on this computer that answers each request with the next of
    /// `replies` (status and body) and keeps the requests' bodies.
    fn server(replies: Vec<(u16, &'static str, String)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/v1", listener.local_addr().unwrap().port());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let kept = seen.clone();
        std::thread::spawn(move || {
            for (status, kind, body) in replies {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap();
                    }
                    if line == "\r\n" {
                        break;
                    }
                }
                let mut request = vec![0; length];
                reader.read_exact(&mut request).unwrap();
                kept.lock().unwrap().push(String::from_utf8(request).unwrap());
                let head = format!("HTTP/1.1 {status} X\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
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
        QuestionRequest { prompt: "Kvíz".into(), attachments: vec![picture], language: QuizLanguage::Cs, whole_quiz: false }
    }

    fn ask_server(url: &str) -> (CmdResult<(String, bool)>, usize) {
        let model = LocalModel::new(url, "gemma3");
        let body = local_model::request(&model, &asked()).unwrap();
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
        let body = local_model::request(&model, &asked()).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancel = cancelled.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            cancel.store(true, Ordering::SeqCst);
        });
        assert_eq!(ask_cancellably(model, body, cancelled, |_| {}), Err(AppError::LocalModelCancelled));
        drop(listener);
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
    fn an_error_midway_and_a_model_that_does_not_stop_are_told() {
        let midway = "data: {\"choices\":[{\"delta\":{\"content\":\"{\\\"q\"}}]}\n\ndata: {\"error\":{\"message\":\"out of memory\"}}\n\n";
        let reply = read_stream(BufReader::new(midway.as_bytes()), &AtomicBool::new(false), |_| {});
        assert_eq!(reply, Err(AppError::LocalModelFailed { detail: "out of memory".into() }));
        let endless = sse(&vec!["{\"question\": \"ano\", "; MAX_REPLY_CHARS / 10]);
        let told = std::cell::Cell::new(LocalModelProgress { characters: 0, questions: 0 });
        let (content, cut_off) = read_stream(BufReader::new(endless.as_bytes()), &AtomicBool::new(false), |now| told.set(now)).unwrap();
        assert!(cut_off);
        assert!(content.chars().count() <= MAX_REPLY_CHARS + 30);
        // Counted as written, the same as counting the whole text.
        assert_eq!(told.get().questions, local_model::questions_begun(&content, 0));
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
        };
        let body = local_model::request(&model, &asked).unwrap();
        let told = Arc::new(AtomicUsize::new(0));
        let counted = told.clone();
        let started = std::time::Instant::now();
        let (content, cut_off) = ask_cancellably(model, body, Arc::new(AtomicBool::new(false)), move |progress| {
            counted.store(progress.questions, Ordering::SeqCst);
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

    #[test]
    fn the_default_address_is_this_computer() {
        assert!(is_this_computer(DEFAULT_URL));
    }
}
