//! The AI panel's request from start to reply (docs/ai-assistants.md):
//! what is looked up on the internet (`research`), how the model's turn of
//! the conversation (`chat`) and the questions (`write_questions`) are
//! asked again of a model whose context the request did not fit, and how a
//! streamed reply is read (`read_stream`).
//! The network is the caller's: it passes in how to ask the model and how
//! to fetch a page, so all of this runs in tests without one.

use crate::local_model::{self, AttachmentContent, LocalModel, QuestionRequest, RequestError, StreamLine};
use crate::web_page::{self, Page, SearchResult, Source, Url};
use quiz_core::MediaKind;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use ts_rs::TS;

/// Most characters of a model's reply, reasoning included: far more than
/// `MAX_QUESTIONS` questions take, so a model that keeps writing is stopped.
pub const MAX_REPLY_CHARS: usize = 400_000;
/// How many more characters the model writes before the panel is told again.
const TELL_EVERY: usize = 200;

/// What the panel shows while a request is under way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(tag = "step", rename_all = "camelCase")]
pub enum LocalModelProgress {
    /// The model says what to look up on the internet.
    Planning,
    /// The app searches the internet for `query`.
    Searching { query: String },
    /// The app reads the page at `url`.
    Reading { url: String },
    /// The model reasons before it answers (a reasoning model): characters so far.
    Thinking { characters: usize },
    /// The model writes the questions: characters so far, questions begun.
    Writing { characters: usize, questions: usize },
    /// More of the model's turn of the conversation (`Reading::words`).
    Said { text: String },
}

/// How a streamed reply is read (`read_stream`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    /// Most characters a model may reason before it answers.
    pub max_thought: usize,
    /// Whether the panel is told the reply's words as they come (`LocalModelProgress::Said`).
    pub words: bool,
}

impl Reading {
    /// The questions: a reasoning model may reason as long as it needs.
    pub const QUESTIONS: Reading = Reading { max_thought: MAX_REPLY_CHARS, words: false };
    /// The model's turn of the conversation, shown as it is written.
    pub const CHAT: Reading = Reading { max_thought: MAX_REPLY_CHARS, words: true };
    /// The research plan: a few search queries, which need no long
    /// reasoning; a model that reasons on anyway (some cannot stop, whatever
    /// they are asked) is stopped, and the plan made without it.
    pub const PLAN: Reading = Reading { max_thought: PLAN_THOUGHT_CHARS, words: false };
}

/// Most characters a model may reason over the research plan [no source -
/// estimate: an 8B reasoning model thought 9,000 to 14,000 characters,
/// 97 to 157 s, over it in our test, which no plan needs].
pub const PLAN_THOUGHT_CHARS: usize = 3_000;

/// Why a request stopped, as this module finds it; the caller's error
/// type says it in its own words (`From<Halt>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Halt {
    Cancelled,
    /// A reasoning model reasoned up to its limit and wrote no answer.
    ThoughtTooLong,
    /// The server stopped the reply with an error (`StreamLine::Failed`).
    Failed { detail: String },
    /// A reply that is not a chat completion.
    NotQuestions,
}

/// What was found on the internet for a request (`research`).
#[derive(Debug, Default)]
pub struct Research {
    /// The searches made and what each found.
    pub results: Vec<(String, Vec<SearchResult>)>,
    /// The pages read, by the address each was read at.
    pub pages: Vec<(String, Page)>,
    pub sources: Vec<Source>,
    /// Whether a search or a page the request needed could not be read.
    pub failed: bool,
}

impl Research {
    /// What was found, as the model reads it in a request of `room` text
    /// characters (`local_model::MAX_TEXT_CHARS` or `SMALL_TEXT_CHARS`).
    pub fn text(&self, room: usize) -> Option<String> {
        if self.results.is_empty() && self.pages.is_empty() {
            return None;
        }
        Some(web_page::research_text(&self.results, &self.pages, web_page::MAX_RESEARCH_CHARS * room / local_model::MAX_TEXT_CHARS))
    }
}

/// Looks up on the internet what `asked` needs. `plan` asks `model` the
/// request it is given (`local_model::research_plan_request`), telling the
/// progress it is given, and the model says what to search for
/// (`local_model::research_plan`); a model that gives no plan (it reasoned
/// past its limit, say) has the request itself searched when the questions
/// are to be written (`for_quiz`), unless the failure is a cancel; in the conversation it then looks up nothing. The sites
/// the latest message names (all the quizmaster's messages, `for_quiz`) are read
/// with a few of their pages that say most about the company
/// (`web_page::links_to_follow`), then the searches' best results, at most
/// `web_page::MAX_PAGES` pages in all, none of them a legal page
/// (`web_page::says_nothing`) or on the local network unless the request
/// named it (`web_page::may_read`). `fetch` gets a page's address and
/// gives back the address it was read at and its HTML, or None. A search
/// or page that fails is passed over, and said so.
pub fn research<E: From<Halt> + PartialEq>(
    model: &LocalModel,
    asked: &QuestionRequest,
    stop: &AtomicBool,
    tell: &impl Fn(LocalModelProgress),
    plan: impl FnOnce(Value, &dyn Fn(LocalModelProgress)) -> Result<(String, bool), E>,
    fetch: impl Fn(&str) -> Option<(String, String)>,
    for_quiz: bool,
) -> Result<Research, E> {
    let written = if for_quiz { asked.request_text() } else { asked.prompt.clone() };
    tell(LocalModelProgress::Planning);
    let thinking = |step: LocalModelProgress| {
        if matches!(step, LocalModelProgress::Thinking { .. }) {
            tell(step);
        }
    };
    let queries = match plan(local_model::research_plan_request(model, asked), &thinking) {
        Ok((reply, _)) => local_model::research_plan(&reply).queries,
        Err(e) if e == Halt::Cancelled.into() => return Err(e),
        Err(_) if for_quiz => local_model::plain_search(&written).into_iter().collect(),
        Err(_) => Vec::new(),
    };
    let sites = web_page::addresses_in(&written);
    if queries.is_empty() && sites.is_empty() {
        return Ok(Research::default());
    }
    let mut found = Research::default();
    // Every page asked for and read at, so that one is read once however its address is written.
    let mut tried = HashSet::new();
    // The page at `url` added to the pages, unless there are enough, it was
    // tried or it may not be read; the address it was read at, or Err when
    // it could not be.
    let mut read = |url: &str, pages: &mut Vec<(String, Page)>| -> Result<Result<Option<Url>, ()>, E> {
        if stop.load(Ordering::SeqCst) {
            return Err(Halt::Cancelled.into());
        }
        let allowed = Url::parse(url).is_ok_and(|parsed| web_page::may_read(&parsed, &sites));
        if pages.len() >= web_page::MAX_PAGES || !allowed || web_page::says_nothing(url) || !tried.insert(web_page::page_key(url)) {
            return Ok(Ok(None));
        }
        tell(LocalModelProgress::Reading { url: url.to_string() });
        Ok(match fetch(url) {
            Some((at, html)) => {
                if !tried.insert(web_page::page_key(&at)) && web_page::page_key(&at) != web_page::page_key(url) {
                    // Redirected to a page read before.
                    return Ok(Ok(None));
                }
                let followed = Url::parse(&at).ok();
                pages.push((at, web_page::read_page(&html)));
                Ok(followed)
            }
            None => Err(()),
        })
    };
    for site in sites.iter().take(local_model::MAX_SEARCHES) {
        match read(site.as_str(), &mut found.pages)? {
            Ok(Some(at)) => {
                let links = found.pages.last().map(|(_, page)| page.links.clone()).unwrap_or_default();
                for link in web_page::links_to_follow(&at, &links, web_page::SITE_PAGES) {
                    // A page of a site that answered may be missing; that is no failure.
                    let _ = read(link.as_str(), &mut found.pages)?;
                }
            }
            Ok(None) => {}
            Err(()) => found.failed = true,
        }
    }
    for query in queries {
        if stop.load(Ordering::SeqCst) {
            return Err(Halt::Cancelled.into());
        }
        tell(LocalModelProgress::Searching { query: query.clone() });
        let results = fetch(&web_page::search_address(&query)).map(|(_, html)| web_page::read_results(&html)).unwrap_or_default();
        // None found is a search refused as often as one with no result.
        found.failed |= results.is_empty();
        for result in results.iter().take(web_page::RESULT_PAGES) {
            // The search's snippet stands for a result that cannot be read.
            let _ = read(&result.url, &mut found.pages)?;
        }
        found.results.push((query, results));
    }
    found.sources = found.pages.iter().map(|(url, page)| Source { url: url.clone(), title: if page.title.is_empty() { url.clone() } else { page.title.clone() } }).collect();
    Ok(found)
}

/// Asks `model` for the questions `asked` for, with what was `found` on the
/// internet, through `ask`. Asked once more, with the texts shortened
/// (`local_model::SMALL_TEXT_CHARS`) and the model asked not to reason
/// first (`local_model::thinking_off`), when the request does not fit
/// (`too_much`: it overflows the model's context, or the model reasoned to
/// its limit) or the reply was cut off while the texts could be shorter:
/// a model in Ollama has 4096 tokens unless the quizmaster gives it more,
/// for the request, the model's reasoning and its reply together. If that
/// fails too, its failure is given back.
pub fn write_questions<E: From<RequestError>>(
    model: &LocalModel,
    asked: &QuestionRequest,
    found: &Research,
    ask: impl FnMut(Value) -> Result<(String, bool), E>,
    too_much: impl Fn(&E) -> bool,
) -> Result<(String, bool), E> {
    ask_fitting(|room| local_model::request(model, asked, found.text(room).as_deref(), room), ask, too_much)
}

/// Asks `model` for its next turn of the conversation `asked` holds, with
/// what was `found` on the internet for it, as `write_questions` asks for
/// the questions.
pub fn chat<E: From<RequestError>>(
    model: &LocalModel,
    asked: &QuestionRequest,
    found: &Research,
    ask: impl FnMut(Value) -> Result<(String, bool), E>,
    too_much: impl Fn(&E) -> bool,
) -> Result<(String, bool), E> {
    ask_fitting(|room| local_model::chat_request(model, asked, found.text(room).as_deref(), room), ask, too_much)
}

/// Asks the request `build` makes with `MAX_TEXT_CHARS` of text, and once
/// more with `SMALL_TEXT_CHARS` and reasoning off when it does not fit
/// (`write_questions`).
fn ask_fitting<E: From<RequestError>>(
    build: impl Fn(usize) -> Result<Value, RequestError>,
    mut ask: impl FnMut(Value) -> Result<(String, bool), E>,
    too_much: impl Fn(&E) -> bool,
) -> Result<(String, bool), E> {
    let body = build(local_model::MAX_TEXT_CHARS)?;
    let shorter = build(local_model::SMALL_TEXT_CHARS)?;
    let shortened = shorter != body;
    match ask(body) {
        Err(e) if too_much(&e) => ask(local_model::thinking_off(shorter)),
        Ok((_, true)) if shortened => ask(local_model::thinking_off(shorter)),
        done => done,
    }
}

/// Reads a streamed chat completion line by line: the model's text and
/// whether it stopped at its length limit. `progress` hears when the model
/// begins a question, every few hundred characters, and while a reasoning
/// model only reasons, and, as `reading` says, each piece of text as it
/// comes; a model that reasons past `reading.max_thought` before it answers
/// is stopped (`Halt::ThoughtTooLong`). A server that sends the whole
/// reply at once is read too. `cancelled` is looked at after each line; a
/// line that cannot be read fails with what `read_failed` makes of it.
pub fn read_stream<E: From<Halt>>(mut reader: impl BufRead, cancelled: &AtomicBool, reading: Reading, progress: impl Fn(LocalModelProgress), read_failed: impl Fn(std::io::Error) -> E) -> Result<(String, bool), E> {
    let (mut content, mut cut_off) = (String::new(), false);
    // Characters written and questions begun, now and when last told; characters reasoned, now and when told.
    let (mut now, mut told) = ((0, 0), (0, 0));
    let (mut thought, mut told_thought) = (0, 0);
    // Bytes of the reply told as said (`LocalModelProgress::Said`).
    let mut told_words = 0;
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line).map_err(&read_failed)?;
        if cancelled.load(Ordering::SeqCst) {
            return Err(Halt::Cancelled.into());
        }
        if read == 0 {
            break;
        }
        match local_model::stream_line(&line) {
            StreamLine::Text { text, thought: more, cut_off: at_limit } => {
                thought += more;
                if content.is_empty() && thought >= told_thought + TELL_EVERY {
                    progress(LocalModelProgress::Thinking { characters: thought });
                    told_thought = thought;
                }
                if content.trim().is_empty() && thought > reading.max_thought {
                    return Err(Halt::ThoughtTooLong.into());
                }
                let had = content.len();
                content.push_str(&text);
                if reading.words {
                    // Thoughts in the reply's own text (`<think>`) are not told as said.
                    let shown = local_model::shown_so_far(&content);
                    if shown.len() > told_words {
                        progress(LocalModelProgress::Said { text: shown[told_words..].to_string() });
                        told_words = shown.len();
                    }
                }
                cut_off |= at_limit;
                now.0 += text.chars().count();
                now.1 += local_model::questions_begun(&content, had);
                if now.1 != told.1 || now.0 >= told.0 + TELL_EVERY {
                    progress(LocalModelProgress::Writing { characters: now.0, questions: now.1 });
                    told = now;
                }
                if now.0 + thought > MAX_REPLY_CHARS {
                    // A model that does not stop: what it wrote is read as cut off.
                    return if content.trim().is_empty() { Err(Halt::ThoughtTooLong.into()) } else { Ok((content, true)) };
                }
            }
            StreamLine::Failed { detail } => return Err(Halt::Failed { detail }.into()),
            StreamLine::Done => break,
            StreamLine::Nothing => {}
            StreamLine::NotStreamed => {
                let mut whole = line.clone();
                reader.read_to_string(&mut whole).map_err(&read_failed)?;
                let not_questions = || E::from(Halt::NotQuestions);
                let reply: Value = serde_json::from_str(&whole).map_err(|_| not_questions())?;
                let choice = reply.pointer("/choices/0").ok_or_else(not_questions)?;
                let text = choice.pointer("/message/content").and_then(Value::as_str).ok_or_else(not_questions)?;
                return Ok((text.to_string(), choice.get("finish_reason").and_then(Value::as_str) == Some("length")));
            }
        }
    }
    if cut_off && content.trim().is_empty() && thought > 0 {
        return Err(Halt::ThoughtTooLong.into());
    }
    Ok((content, cut_off))
}

/// The picture or video each of the `attachments` shows on its questions'
/// slides: its reference in the draft as `found` there, and whether it is
/// a video. A slide shows a file as what it is, whatever the attachment
/// says; a text, a recording or a file not in the draft shows none.
pub fn slide_media(attachments: &[local_model::Attachment], found: Vec<Option<String>>) -> Vec<Option<(String, bool)>> {
    attachments
        .iter()
        .zip(found)
        .map(|(attachment, found)| match (&attachment.content, found.as_deref().and_then(MediaKind::of)) {
            (AttachmentContent::Picture { .. } | AttachmentContent::Video { .. }, Some(kind @ (MediaKind::Image | MediaKind::Video))) => found.map(|reference| (reference, kind == MediaKind::Video)),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_model::{Attachment, ChatRole, ChatTurn, DEFAULT_URL};
    use quiz_core::QuizLanguage;
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Failure {
        Halt(Halt),
        Request(RequestError),
        ContextFull,
        Refused,
    }

    impl From<Halt> for Failure {
        fn from(halt: Halt) -> Self {
            Failure::Halt(halt)
        }
    }

    impl From<RequestError> for Failure {
        fn from(e: RequestError) -> Self {
            Failure::Request(e)
        }
    }

    fn model() -> LocalModel {
        LocalModel::new(DEFAULT_URL, "m")
    }

    fn asked(prompt: &str, internet: bool) -> QuestionRequest {
        QuestionRequest { prompt: prompt.into(), attachments: vec![], language: QuizLanguage::Cs, whole_quiz: false, internet, ..Default::default() }
    }

    /// A model's reply: its text and whether it stopped at its limit.
    type Reply = Result<(String, bool), Failure>;

    fn plan(queries: &[&str]) -> impl FnOnce(Value, &dyn Fn(LocalModelProgress)) -> Reply {
        let reply = serde_json::json!({ "queries": queries }).to_string();
        move |_, _| Ok((reply, false))
    }

    fn read(stream: &str) -> (Result<(String, bool), Failure>, Vec<LocalModelProgress>) {
        let steps = RefCell::new(Vec::new());
        let read = read_stream(std::io::Cursor::new(stream.to_string()), &AtomicBool::new(false), Reading::QUESTIONS, |step| steps.borrow_mut().push(step), |_| Failure::Refused);
        (read, steps.into_inner())
    }

    fn chunk(delta: Value, finish: Option<&str>) -> String {
        format!("data: {}\n\n", serde_json::json!({ "choices": [{ "delta": delta, "finish_reason": finish }] }))
    }

    #[test]
    fn a_site_the_request_names_is_read_with_its_telling_pages() {
        let fetched = RefCell::new(Vec::new());
        let fetch = |url: &str| {
            fetched.borrow_mut().push(url.to_string());
            match url {
                "https://pekarna.cz/firma" => Some((url.to_string(), r#"<title>Pekárna Novák</title><h1>Pečeme od 1921</h1><a href="/o-nas">O nás</a> <a href="/cenik.pdf">Ceník</a> <a href="/gdpr">GDPR</a>"#.to_string())),
                "https://pekarna.cz/o-nas" => Some((url.to_string(), "<title>O nás</title><p>Založil ji Jan Novák v Brně.</p>".to_string())),
                _ => None,
            }
        };
        let steps = RefCell::new(Vec::new());
        let found = research(&model(), &asked("Kvíz o firmě podle webu pekarna.cz/firma", true), &AtomicBool::new(false), &|step| steps.borrow_mut().push(step), plan(&[]), fetch, true).unwrap();
        assert_eq!(found.sources.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(), ["Pekárna Novák", "O nás"]);
        assert_eq!(fetched.into_inner(), ["https://pekarna.cz/firma", "https://pekarna.cz/o-nas"]);
        assert!(!found.failed);
        let text = found.text(local_model::MAX_TEXT_CHARS).unwrap();
        assert!(text.contains("Pečeme od 1921") && text.contains("Založil ji Jan Novák v Brně."), "{text}");
        assert_eq!(steps.borrow()[0], LocalModelProgress::Planning);
    }

    #[test]
    fn a_search_reads_its_best_results_once_and_none_on_the_local_network() {
        let results = r#"<a class="result__a" href="//duckduckgo.com/l/?uddg=http%3A%2F%2F192.168.1.1%2Fadmin&rut=x">Router</a>
            <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.bakerini.cz%2Fkontakt%2F&rut=x">Bakerini</a>
            <a class="result__a" href="//duckduckgo.com/l/?uddg=http%3A%2F%2Fbakerini.cz%2Fkontakt&rut=x">Bakerini</a>"#;
        let fetched = RefCell::new(Vec::new());
        let fetch = |url: &str| {
            fetched.borrow_mut().push(url.to_string());
            if url.contains("duckduckgo") {
                Some((url.to_string(), results.to_string()))
            } else {
                Some((url.to_string(), "<title>Kontakt</title><p>Hybešova 35</p>".to_string()))
            }
        };
        let found = research(&model(), &asked("Kvíz o pekárně Bakerini", true), &AtomicBool::new(false), &|_| {}, plan(&["Bakerini Brno"]), fetch, true).unwrap();
        let fetched = fetched.into_inner();
        assert_eq!(fetched.len(), 2, "{fetched:?}");
        assert_eq!(fetched[1], "https://www.bakerini.cz/kontakt/");
        assert_eq!(found.sources.len(), 1);
    }

    #[test]
    fn a_model_that_gives_no_plan_has_the_request_searched_and_a_cancel_stops() {
        let searched = RefCell::new(Vec::new());
        let fetch = |url: &str| {
            searched.borrow_mut().push(url.to_string());
            None
        };
        let thinking = |_: Value, _: &dyn Fn(LocalModelProgress)| Err(Failure::Halt(Halt::ThoughtTooLong));
        let found = research(&model(), &asked("Kvíz o firmě Bakchus aktivity", true), &AtomicBool::new(false), &|_| {}, thinking, fetch, true).unwrap();
        assert!(searched.into_inner()[0].contains("Bakchus+aktivity") || found.results[0].0.contains("Bakchus aktivity"));
        assert!(found.failed, "no result is a failed search");
        let cancelled = |_: Value, _: &dyn Fn(LocalModelProgress)| Err(Failure::Halt(Halt::Cancelled));
        let stopped = research(&model(), &asked("Kvíz", true), &AtomicBool::new(false), &|_| {}, cancelled, |_| None, true);
        assert_eq!(stopped.unwrap_err(), Failure::Halt(Halt::Cancelled));
    }

    #[test]
    fn a_request_that_needs_no_internet_reads_nothing() {
        let found = research(&model(), &asked("Kvíz o Praze", true), &AtomicBool::new(false), &|_| {}, plan(&[]), |_| panic!("nothing is fetched"), true).unwrap();
        assert!(found.text(local_model::MAX_TEXT_CHARS).is_none() && found.sources.is_empty() && !found.failed);
    }

    #[test]
    fn a_request_too_long_for_the_context_is_asked_again_shorter_once() {
        let text = Attachment { name: "a.txt".into(), media: None, content: AttachmentContent::Text { text: "slovo ".repeat(5_000) } };
        let asked = QuestionRequest { attachments: vec![text], ..asked("Kvíz", false) };
        let sent = RefCell::new(Vec::new());
        let ask = |answers: Vec<Result<(String, bool), Failure>>| {
            let mut answers = answers.into_iter();
            let sent = &sent;
            move |body: Value| {
                sent.borrow_mut().push(body.to_string().len());
                answers.next().unwrap()
            }
        };
        let full = |e: &Failure| matches!(e, Failure::ContextFull | Failure::Halt(Halt::ThoughtTooLong));
        assert!(write_questions(&model(), &asked, &Research::default(), ask(vec![Err(Failure::ContextFull), Ok(("{}".into(), false))]), full).is_ok());
        let sizes = sent.take();
        assert!(sizes[0] > local_model::MAX_TEXT_CHARS && sizes[1] < local_model::MAX_TEXT_CHARS, "{sizes:?}");
        assert_eq!(write_questions(&model(), &asked, &Research::default(), ask(vec![Err(Failure::ContextFull), Err(Failure::ContextFull)]), full), Err(Failure::ContextFull));
        assert_eq!(sent.take().len(), 2, "asked twice, not more");
        assert_eq!(write_questions(&model(), &asked, &Research::default(), ask(vec![Err(Failure::Refused)]), full), Err(Failure::Refused));
    }

    #[test]
    fn a_reply_cut_off_by_long_texts_or_long_reasoning_is_asked_for_again_shorter_and_without_reasoning() {
        let text = Attachment { name: "a.txt".into(), media: None, content: AttachmentContent::Text { text: "slovo ".repeat(5_000) } };
        let long = QuestionRequest { attachments: vec![text], ..asked("Kvíz", false) };
        let sent = RefCell::new(Vec::new());
        let ask = |answers: Vec<Result<(String, bool), Failure>>| {
            let mut answers = answers.into_iter();
            let sent = &sent;
            move |body: Value| {
                sent.borrow_mut().push(body);
                answers.next().unwrap()
            }
        };
        let full = |e: &Failure| matches!(e, Failure::ContextFull | Failure::Halt(Halt::ThoughtTooLong));
        let cut = || Ok(("{\"questions\": [".to_string(), true));
        let whole = || Ok(("{}".to_string(), false));
        assert_eq!(write_questions(&model(), &long, &Research::default(), ask(vec![cut(), whole()]), full), whole());
        let bodies = sent.take();
        assert!(bodies[1].to_string().len() < bodies[0].to_string().len());
        assert_eq!((bodies[0].get("reasoning_effort"), &bodies[1]["reasoning_effort"]), (None, &serde_json::json!("none")));
        // A short request cut off is not shortened: it is what the model wrote.
        assert_eq!(write_questions(&model(), &asked("Kvíz", false), &Research::default(), ask(vec![cut()]), full), cut());
        assert_eq!(sent.take().len(), 1);
        // A model that reasons to its limit is asked once more not to.
        let thought = || Err(Failure::Halt(Halt::ThoughtTooLong));
        assert_eq!(write_questions(&model(), &asked("Kvíz", false), &Research::default(), ask(vec![thought(), whole()]), full), whole());
        assert_eq!(sent.take()[1]["reasoning_effort"], "none");
    }

    #[test]
    fn the_research_plan_asks_a_model_not_to_reason() {
        assert_eq!(local_model::research_plan_request(&model(), &asked("Kvíz o firmě", true))["reasoning_effort"], "none");
    }

    #[test]
    fn a_streamed_reply_is_put_together_and_its_progress_told() {
        let stream = format!("{}{}data: [DONE]\n\n", chunk(serde_json::json!({ "content": "{\"questions\":[{\"question\"" }), None), chunk(serde_json::json!({ "content": ":\"Kde?\"}]}" }), Some("stop")));
        let (reply, steps) = read(&stream);
        assert_eq!(reply, Ok(("{\"questions\":[{\"question\":\"Kde?\"}]}".to_string(), false)));
        assert!(steps.contains(&LocalModelProgress::Writing { characters: 25, questions: 1 }), "{steps:?}");
    }

    #[test]
    fn a_model_that_only_reasons_is_shown_thinking_and_said_so_at_its_limit() {
        let thought = "Hmm. ".repeat(60);
        let (thinking, steps) = read(&format!("{}{}data: [DONE]\n\n", chunk(serde_json::json!({ "reasoning": thought }), None), chunk(serde_json::json!({ "content": "" }), Some("length"))));
        assert_eq!(thinking, Err(Failure::Halt(Halt::ThoughtTooLong)));
        assert_eq!(steps[0], LocalModelProgress::Thinking { characters: 300 });
        // Reasoning, then the answer: the answer is read.
        let (answered, _) = read(&format!("{}{}data: [DONE]\n\n", chunk(serde_json::json!({ "reasoning_content": "ok" }), None), chunk(serde_json::json!({ "content": "{}" }), Some("stop"))));
        assert_eq!(answered, Ok(("{}".to_string(), false)));
    }

    #[test]
    fn an_error_midway_a_whole_reply_and_a_model_that_does_not_stop_are_told() {
        let (failed, _) = read(&format!("{}data: {{\"error\":{{\"message\":\"out of memory\"}}}}\n\n", chunk(serde_json::json!({ "content": "{" }), None)));
        assert_eq!(failed, Err(Failure::Halt(Halt::Failed { detail: "out of memory".into() })));
        let whole = serde_json::json!({ "choices": [{ "message": { "content": "[]" }, "finish_reason": "length" }] }).to_string();
        assert_eq!(read(&whole).0, Ok(("[]".to_string(), true)));
        assert_eq!(read("not json at all").0, Err(Failure::Halt(Halt::NotQuestions)));
        let endless = chunk(serde_json::json!({ "content": "x".repeat(MAX_REPLY_CHARS / 4) }), None).repeat(5);
        let (stopped, _) = read(&endless);
        assert!(matches!(stopped, Ok((_, true))));
    }

    #[test]
    fn a_slide_shows_a_file_as_what_it_is() {
        let attachment = |content| Attachment { name: "x".into(), media: None, content };
        let attachments = [
            attachment(AttachmentContent::Picture { image: String::new() }),
            attachment(AttachmentContent::Video { frames: vec![], words: String::new() }),
            attachment(AttachmentContent::Picture { image: String::new() }),
            attachment(AttachmentContent::Recording { words: String::new() }),
        ];
        let found = vec![Some("media/a.mp4".into()), Some("media/b.jpg".into()), None, Some("media/c.mp3".into())];
        assert_eq!(slide_media(&attachments, found), [Some(("media/a.mp4".into(), true)), Some(("media/b.jpg".into(), false)), None, None]);
    }

    #[test]
    fn in_the_conversation_a_model_that_gives_no_plan_looks_up_nothing() {
        let thinking = |_: Value, _: &dyn Fn(LocalModelProgress)| Err(Failure::Halt(Halt::ThoughtTooLong));
        let found = research(&model(), &asked("A co jejich historie?", true), &AtomicBool::new(false), &|_| {}, thinking, |_| panic!("nothing is fetched"), false).unwrap();
        assert!(found.sources.is_empty() && found.results.is_empty() && !found.failed);
    }

    #[test]
    fn the_quiz_reads_the_sites_named_earlier_in_the_conversation_and_a_turn_only_its_own() {
        let earlier = vec![ChatTurn { role: ChatRole::User, text: "Kvíz o firmě podle pekarna.cz".into() }, ChatTurn { role: ChatRole::Assistant, text: "Navrhuji 3 kola.".into() }];
        let turn = QuestionRequest { conversation: earlier, ..asked("Přidej kolo o historii", true) };
        let read = RefCell::new(Vec::new());
        let fetch = |url: &str| {
            read.borrow_mut().push(url.to_string());
            None
        };
        research(&model(), &turn, &AtomicBool::new(false), &|_| {}, plan(&[]), fetch, false).unwrap();
        assert!(read.take().is_empty());
        research(&model(), &turn, &AtomicBool::new(false), &|_| {}, plan(&[]), fetch, true).unwrap();
        assert_eq!(read.take(), ["https://pekarna.cz/"]);
    }

    #[test]
    fn a_turn_of_the_conversation_is_told_as_it_comes_and_asked_again_shorter() {
        let (said, steps) = {
            let steps = RefCell::new(Vec::new());
            let stream = format!("{}{}data: [DONE]\n\n", chunk(serde_json::json!({ "content": "Navrhuji " }), None), chunk(serde_json::json!({ "content": "3 kola." }), Some("stop")));
            let said = read_stream(std::io::Cursor::new(stream), &AtomicBool::new(false), Reading::CHAT, |step| steps.borrow_mut().push(step), |_| Failure::Refused);
            (said, steps.into_inner())
        };
        assert_eq!(said, Ok(("Navrhuji 3 kola.".to_string(), false)));
        let words: Vec<_> = steps.iter().filter_map(|step| if let LocalModelProgress::Said { text } = step { Some(text.as_str()) } else { None }).collect();
        assert_eq!(words, ["Navrhuji ", "3 kola."]);
        let sent = RefCell::new(Vec::new());
        let mut answers = vec![Err(Failure::ContextFull), Ok(("Dobře.".to_string(), false))].into_iter();
        let reply = chat(&model(), &asked("Kvíz o Praze", false), &Research::default(), |body: Value| {
            sent.borrow_mut().push(body);
            answers.next().unwrap()
        }, |e| *e == Failure::ContextFull);
        assert_eq!(reply, Ok(("Dobře.".to_string(), false)));
        let sent = sent.into_inner();
        assert!(sent[0].get("response_format").is_none(), "a turn of the conversation is plain text");
        assert_eq!(sent[1]["reasoning_effort"], "none");
    }

    #[test]
    fn a_model_that_reasons_too_long_over_the_plan_is_stopped() {
        let thought = format!("{}data: [DONE]\n\n", chunk(serde_json::json!({ "reasoning": "Hmm. ".repeat(PLAN_THOUGHT_CHARS / 5 + 1) }), None));
        let read = |reading| read_stream(std::io::Cursor::new(thought.clone()), &AtomicBool::new(false), reading, |_| {}, |_| Failure::Refused);
        assert_eq!(read(Reading::PLAN), Err(Failure::Halt(Halt::ThoughtTooLong)));
        // Questions may be reasoned over as long as a model needs: it ends without an answer, and no limit is said.
        assert_eq!(read(Reading::QUESTIONS), Ok((String::new(), false)));
    }
}
