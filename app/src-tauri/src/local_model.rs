//! Studio's AI panel (docs/ai-assistants.md): questions from a model running
//! on the quizmaster's computer, through a server that speaks the OpenAI
//! chat completions API (Ollama, LM Studio, llama.cpp's server). What is
//! sent and how the reply becomes slides is `quiz_ai::local_model`; here the
//! requests are sent, streamed back and cancelled.
//!
//! A request is refused during a show: a model takes the computer's
//! processor and memory for minutes, which a live show must not compete
//! with.

use crate::ai::SavedAiSettings;
use crate::editor;
use crate::error::{AppError, CmdResult};
use crate::{off_ui, refuse_during_show};
use quiz_ai::local_model::{self, AttachmentContent, LocalModel, LocalModelInfo, QuestionRequest, ReplyError, RequestError, StreamLine};
use quiz_core::{import_tables, Slide};
use serde::Serialize;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
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

/// The request under way, one at a time, and whether it was cancelled.
#[derive(Default)]
pub struct LocalModelState {
    asking: Mutex<()>,
    cancelled: Arc<AtomicBool>,
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
/// `editor_ask_local_model_cancel`, which closes the connection, so the
/// server stops too.
#[tauri::command]
pub async fn editor_ask_local_model(app: AppHandle, draft: String, asked: QuestionRequest, progress: Channel<LocalModelProgress>) -> CmdResult<Vec<Slide>> {
    refuse_during_show(&app)?;
    let model = app.state::<SavedAiSettings>().get().local_model.ok_or(AppError::LocalModelNotChosen)?;
    let body = local_model::request(&model, &asked).map_err(|e| match e {
        RequestError::Empty => AppError::LocalModelNothingAsked,
        RequestError::TooManyFiles { max } => AppError::LocalModelTooManyFiles { max },
        RequestError::TooManyPictures { max } => AppError::LocalModelTooManyPictures { max },
        RequestError::NotAPicture => AppError::system("a picture for the model is not a data URL"),
    })?;
    let media = editor::draft_media(app.clone(), draft, asked.attachments.iter().map(|a| a.media.clone()).collect()).await?;
    let media: Vec<Option<(String, bool)>> =
        asked.attachments.iter().zip(media).map(|(attachment, found)| found.map(|reference| (reference, matches!(attachment.content, AttachmentContent::Video { .. })))).collect();
    let (content, cut_off) = off_ui(move || {
        let state = app.state::<LocalModelState>();
        let _one_at_a_time = state.asking.try_lock().map_err(|_| AppError::LocalModelBusy)?;
        state.cancelled.store(false, Ordering::SeqCst);
        ask_cancellably(model, body, state.cancelled.clone(), move |done| {
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
    state.cancelled.store(true, Ordering::SeqCst);
}

/// Sends `body` on a thread of its own and waits for its reply, which ends
/// at once when `cancelled` is set: a server that takes minutes before it
/// writes does not keep the panel waiting. The thread then closes the
/// connection when the server next writes, or at its time limit.
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
    let (mut content, mut cut_off, mut told) = (String::new(), false, LocalModelProgress { characters: 0, questions: 0 });
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
                content.push_str(&text);
                cut_off |= at_limit;
                let now = LocalModelProgress { characters: content.chars().count(), questions: local_model::questions_begun(&content) };
                if now.questions != told.questions || now.characters >= told.characters + 200 {
                    progress(now);
                    told = now;
                }
            }
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
        .max_redirects(3)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(first_word))
        .timeout_recv_body(Some(REPLY_TIMEOUT));
    let config = if is_this_computer(address) { config.proxy(None) } else { config };
    config.build().into()
}

/// Whether `address` names this computer.
fn is_this_computer(address: &str) -> bool {
    let host = address.split_once("://").map_or(address, |(_, rest)| rest);
    let host = host.split(['/', '?']).next().unwrap_or_default();
    let host = if host.starts_with('[') { host.split(']').next().unwrap_or_default().trim_start_matches('[') } else { host.split(':').next().unwrap_or_default() };
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
    fn this_computer_is_known_by_its_names() {
        for address in ["http://localhost:11434/v1", "http://127.0.0.1:1234/v1", "http://[::1]:8080/v1", "localhost"] {
            assert!(is_this_computer(address), "{address}");
        }
        for address in ["http://192.168.1.5:11434/v1", "https://example.com/v1", "http://localhost.example.com/v1"] {
            assert!(!is_this_computer(address), "{address}");
        }
    }

    #[test]
    fn the_default_address_is_this_computer() {
        assert!(is_this_computer(DEFAULT_URL));
    }
}
