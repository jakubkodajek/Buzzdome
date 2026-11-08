//! Reading questions aloud with the offline voice model (spec SN-10). The
//! model is downloaded once into the app's local data (`voice/`) and
//! checked against its hash (`voice::model`). Clips are rendered in the
//! voice process, the app started again with `--voice <folder>`, so that
//! the engine's native code ending badly ends only that process, never a
//! running show; a clip is kept in `voice-clips/` under a name made from
//! what it says (`voice::Line::clip_name`), so a show plays clips rendered
//! beforehand without waiting. The windows read the system's own voice
//! when a clip is not there.

use crate::error::{AppError, CmdResult};
use crate::refuse_during_show;
use crate::{app_log, off_ui};
use quiz_file::FileAction;
use serde::Serialize;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use ts_rs::TS;
use voice::server::{Reply, Request};
use voice::{Line, MODEL};

/// The argument that starts the app as the voice process.
const VOICE: &str = "--voice";
/// The model's folder, in the app's local data.
const MODEL_FOLDER: &str = "voice";
/// The rendered clips, in the app's local data.
const CLIPS_FOLDER: &str = "voice-clips";
/// How long the voice process may take to load the model.
const LOAD_TIMEOUT: Duration = Duration::from_secs(60);
/// How long one clip may take: a long question with six answers takes a few
/// seconds on a laptop's CPU [no source - measured in a 4-core container:
/// 10 s of speech in about 2 s], so this much and a little more for each
/// character, as the longest (`voice::MAX_TEXT_CHARS`) take minutes to say.
const CLIP_TIMEOUT: Duration = Duration::from_secs(20);
const CLIP_TIMEOUT_PER_CHAR: Duration = Duration::from_millis(60);
/// How much the voice process lowers its priority on macOS and Linux.
#[cfg(unix)]
const NICENESS: i32 = 10;
/// The most of one line of the voice process's output that is read: a
/// reply is a few bytes, and a longer line (a native library's) is not one.
const MAX_LINE_BYTES: u64 = 4096;
/// Redirects followed, each over HTTPS only (GitHub sends release files
/// on to its file host).
const MAX_REDIRECTS: u32 = 5;
/// How long the download may take in all.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// How often the download's progress goes to the windows.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

pub struct Voice {
    root: PathBuf,
    clips: PathBuf,
    /// Held while the model is downloaded or removed.
    installing: Mutex<()>,
    /// The voice process, started on the first clip it is asked for.
    process: Mutex<Option<VoiceProcess>>,
    /// Set when the voice process could not start or load the model: no
    /// other is started until the model is installed again or removed, so
    /// a broken one is not loaded again for every question.
    broken: AtomicBool,
    /// The quizmaster cancelled the download under way, which stops at its next progress report.
    cancelled: AtomicBool,
}

/// Whether the voice model is there, for the settings and the check before a show.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct VoiceModel {
    pub installed: bool,
    /// The download's size.
    pub bytes: u64,
    /// How many voices it has, numbered from 0.
    pub voices: u8,
}

/// A line's clip, when one is kept, and its words as they are said, which
/// the system's voice reads when there is no clip.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct VoiceClip {
    pub path: Option<String>,
    pub words: String,
}

/// How far the model's download is.
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct VoiceDownload {
    pub received: u64,
    pub total: u64,
}

impl Voice {
    pub fn new(app: &AppHandle) -> CmdResult<Voice> {
        Ok(Voice::at(crate::app_local_data(app, MODEL_FOLDER)?, crate::app_local_data(app, CLIPS_FOLDER)?))
    }

    fn at(root: PathBuf, clips: PathBuf) -> Voice {
        Voice { root, clips, installing: Mutex::new(()), process: Mutex::new(None), broken: AtomicBool::new(false), cancelled: AtomicBool::new(false) }
    }

    /// The folder the windows play clips from.
    pub fn clips_folder(&self) -> &Path {
        &self.clips
    }

    fn model(&self) -> VoiceModel {
        VoiceModel { installed: voice::model::installed(&self.root), bytes: MODEL.bytes, voices: MODEL.voices }
    }

    /// The clip of `line` when it is kept; rendered first when `render`.
    fn clip(&self, line: &Line, render: bool, exe: impl FnOnce() -> CmdResult<PathBuf>) -> CmdResult<Option<PathBuf>> {
        if line.is_empty() {
            return Ok(None);
        }
        let path = self.clips.join(line.clip_name());
        if path.is_file() || !render {
            return Ok(path.is_file().then_some(path));
        }
        if !voice::model::installed(&self.root) {
            return Err(AppError::VoiceMissing);
        }
        fs::create_dir_all(&self.clips).map_err(|e| AppError::file(FileAction::CreateFolder, &self.clips, e))?;
        let mut process = self.process.lock().unwrap();
        // Another call may have rendered it while this one waited.
        if path.is_file() {
            return Ok(Some(path));
        }
        if process.is_none() {
            if self.broken.load(Ordering::Relaxed) {
                return Err(AppError::VoiceFailed);
            }
            let started = exe().and_then(|exe| VoiceProcess::start(&exe, &self.root));
            match started {
                Ok(started) => *process = Some(started),
                Err(e) => {
                    self.broken.store(true, Ordering::Relaxed);
                    return Err(e);
                }
            }
        }
        let rendered = process.as_mut().map(|running| running.render(line, &path)).unwrap_or(Err(AppError::VoiceFailed));
        match rendered {
            Ok(true) => Ok(Some(path)),
            // The engine could not say this line (or the clip could not be
            // written): a failure, though the process goes on.
            Ok(false) => Err(AppError::VoiceFailed),
            Err(e) => {
                // A process that does not answer is ended; the next clip starts another.
                *process = None;
                Err(e)
            }
        }
    }

    /// Ends the voice process, as before the model it holds changes; the
    /// next clip may start one again.
    fn stop(&self) {
        *self.process.lock().unwrap() = None;
        self.broken.store(false, Ordering::Relaxed);
    }
}

/// The voice process and the replies it sends, read as they come.
struct VoiceProcess {
    child: Child,
    requests: ChildStdin,
    replies: Receiver<Reply>,
}

impl VoiceProcess {
    fn start(exe: &Path, root: &Path) -> CmdResult<VoiceProcess> {
        let mut command = Command::new(exe);
        command.arg(VOICE).arg(root).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut command, windows_sys::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS);
        let mut child = command.spawn().map_err(|_| AppError::VoiceFailed)?;
        let requests = child.stdin.take().expect("the voice process's input is piped");
        let output = child.stdout.take().expect("the voice process's output is piped");
        let (send, replies) = mpsc::channel();
        thread::spawn(move || {
            let mut output = BufReader::new(output);
            while let Some(line) = read_line(&mut output) {
                // A line that is not a reply (a native library writing to the
                // output) is passed over, so the replies after it still arrive.
                let Ok(reply) = serde_json::from_slice::<Reply>(&line) else {
                    app_log::note(&format!("voice: not a reply: {}", String::from_utf8_lossy(&line).chars().take(200).collect::<String>()));
                    continue;
                };
                if send.send(reply).is_err() {
                    break;
                }
            }
        });
        let process = VoiceProcess { child, requests, replies };
        match process.reply(LOAD_TIMEOUT)? {
            Reply::Ready => Ok(process),
            _ => Err(AppError::VoiceFailed),
        }
    }

    /// Renders `line` into `out`: whether the clip was written.
    fn render(&mut self, line: &Line, out: &Path) -> CmdResult<bool> {
        let request = Request { line: line.clone(), out: out.to_path_buf() };
        let mut text = serde_json::to_string(&request).map_err(|_| AppError::VoiceFailed)?;
        text.push('\n');
        self.requests.write_all(text.as_bytes()).and_then(|()| self.requests.flush()).map_err(|_| AppError::VoiceFailed)?;
        let started = Instant::now();
        let chars = line.words().chars().count();
        let reply = self.reply(CLIP_TIMEOUT + CLIP_TIMEOUT_PER_CHAR * chars as u32)?;
        app_log::note(&format!("voice: {chars} chars in {} ms, {reply:?}", started.elapsed().as_millis()));
        Ok(reply == Reply::Written)
    }

    fn reply(&self, timeout: Duration) -> CmdResult<Reply> {
        match self.replies.recv_timeout(timeout) {
            Ok(reply) => Ok(reply),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => Err(AppError::VoiceFailed),
        }
    }
}

impl Drop for VoiceProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Called first in `main`: when the app was started as the voice process,
/// serves clips until the app closes its input, then ends the process.
pub fn serve_if_asked() {
    let Some(mut args) = crate::started_as(VOICE) else { return };
    let Some(root) = args.next() else { std::process::exit(2) };
    // A crash here ends this process quietly: the app notices the voice is
    // gone, and no Windows error dialog waits on the quizmaster's screen.
    #[cfg(windows)]
    // SAFETY: SetErrorMode only changes how this process reports errors.
    unsafe {
        use windows_sys::Win32::System::Diagnostics::Debug::{SetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX};
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    }
    // Rendering gives way to the show (the Windows process starts below normal).
    #[cfg(unix)]
    // SAFETY: nice only changes this process's priority.
    unsafe {
        libc::nice(NICENESS);
    }
    let Ok(engine) = voice::engine::Engine::load(Path::new(&root)) else {
        std::process::exit(1);
    };
    let served = voice::server::serve(&engine, std::io::stdin().lock(), std::io::stdout().lock());
    std::process::exit(if served.is_ok() { 0 } else { 1 });
}

/// Whether the voice model is downloaded.
#[tauri::command]
pub async fn voice_model(app: AppHandle) -> CmdResult<VoiceModel> {
    off_ui(move || Ok(app.state::<Voice>().model())).await
}

/// Downloads the voice model, checks it and unpacks it, telling `progress`
/// how the download goes. A model already there is kept until the new one
/// is whole. Refused during a show, as the editor's large files are
/// (`refuse_during_show`): the journal shares the disk. A show that begins
/// meanwhile, or a cancel (`voice_install_cancel`), stops the download
/// within `PROGRESS_EVERY`, also while no data comes (`unstalled`).
#[tauri::command]
pub async fn voice_install(app: AppHandle, progress: Channel<VoiceDownload>) -> CmdResult<VoiceModel> {
    off_ui(move || {
        refuse_during_show(&app)?;
        let state = app.state::<Voice>();
        let _installing = state.installing.lock().unwrap();
        // A second request, given while the first one downloaded, finds it done.
        if voice::model::installed(&state.root) {
            return Ok(state.model());
        }
        fs::create_dir_all(&state.root).map_err(|e| AppError::file(FileAction::CreateFolder, &state.root, e))?;
        let archive = state.root.join(format!("{}.download", MODEL.id));
        state.cancelled.store(false, Ordering::SeqCst);
        let downloaded = download(
            &archive,
            |done| {
                let _ = progress.send(done);
            },
            || !state.cancelled.load(Ordering::SeqCst) && refuse_during_show(&app).is_ok(),
        )
        .map_err(|e| stopped_why(e, state.cancelled.load(Ordering::SeqCst)));
        // A show started after the last report: its journal keeps the disk,
        // and its clips the voice process. None starts while it unpacks.
        let installed = downloaded.and_then(|sha| {
            let _unpacking = Unpacking::begin(&app)?;
            state.stop();
            voice::model::unpack(&archive, &sha, &state.root).map_err(voice_error)
        });
        let _ = fs::remove_file(&archive);
        installed?;
        app_log::note(&format!("voice model {} installed", MODEL.id));
        Ok(state.model())
    })
    .await
}

/// A download that `stopped` (`VoiceDownloadStopped`) as the quizmaster
/// `cancelled` it fails with `VoiceDownloadCancelled`; else a show began.
fn stopped_why(e: AppError, cancelled: bool) -> AppError {
    match e {
        AppError::VoiceDownloadStopped if cancelled => AppError::VoiceDownloadCancelled,
        e => e,
    }
}

/// Cancels the voice model download under way, if one is: it stops within
/// `PROGRESS_EVERY`, also while the connection stalls, and fails with
/// `VoiceDownloadCancelled`.
#[tauri::command]
pub fn voice_install_cancel(app: AppHandle) {
    app.state::<Voice>().cancelled.store(true, Ordering::SeqCst);
}

/// Marks the show state while the voice model is put in place, so that no
/// show begins meanwhile (`Inner::voice_unpacking`); the mark goes when
/// this does, however unpacking ends.
struct Unpacking<'a>(&'a AppHandle);

impl<'a> Unpacking<'a> {
    /// Marks it, unless a show runs.
    fn begin(app: &'a AppHandle) -> CmdResult<Unpacking<'a>> {
        let mut inner = crate::lock_show(app);
        crate::refuse_show(&inner)?;
        inner.voice_unpacking = true;
        Ok(Unpacking(app))
    }
}

impl Drop for Unpacking<'_> {
    fn drop(&mut self) {
        crate::lock_show(self.0).voice_unpacking = false;
    }
}

/// Removes the voice model and the clips made with it. Refused during a show.
#[tauri::command]
pub async fn voice_remove(app: AppHandle) -> CmdResult<VoiceModel> {
    off_ui(move || {
        refuse_during_show(&app)?;
        let state = app.state::<Voice>();
        let _installing = state.installing.lock().unwrap();
        state.stop();
        voice::model::remove(&state.root).map_err(voice_error)?;
        match fs::remove_dir_all(&state.clips) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(AppError::file(FileAction::Delete, &state.clips, e)),
            _ => {}
        }
        Ok(state.model())
    })
    .await
}

/// The kept clip of `line`, as a path the windows can play; with `render`,
/// rendered first when it is not kept (which takes a few seconds). No path
/// when there is nothing to say, or it is not kept and not to be rendered.
#[tauri::command]
pub async fn voice_clip(app: AppHandle, line: Line, render: bool) -> CmdResult<VoiceClip> {
    off_ui(move || {
        let exe = || std::env::current_exe().map_err(|_| AppError::VoiceFailed);
        let line = line.checked();
        let clip = app.state::<Voice>().clip(&line, render, exe)?;
        Ok(VoiceClip { path: clip.map(|path| path.to_string_lossy().into_owned()), words: line.words() })
    })
    .await
}

/// Downloads the model into `archive`, telling `progress` how far it got
/// every `PROGRESS_EVERY`: the download's SHA-256. `go_on` is asked as
/// often, also while no data comes, and stops it by answering false, which
/// fails it with `VoiceDownloadStopped`.
fn download(archive: &Path, mut progress: impl FnMut(VoiceDownload), go_on: impl Fn() -> bool) -> CmdResult<String> {
    let failed = |detail: String| AppError::VoiceDownload { detail };
    let response = agent().get(MODEL.url).call().map_err(|e| failed(e.to_string()))?;
    let file = fs::File::create(archive).map_err(|e| AppError::file(FileAction::Write, archive, e))?;
    let body = response.into_body().into_with_config().limit(MODEL.bytes + 1).reader();
    let stopped = AtomicBool::new(false);
    let body = unstalled(body.take(MODEL.bytes + 1), PROGRESS_EVERY, || {
        let going = go_on();
        stopped.store(!going, Ordering::Relaxed);
        going
    })
    .map_err(|e| failed(e.to_string()))?;
    let mut last = Instant::now();
    let total = MODEL.bytes;
    let sha = voice::model::receive(body, std::io::BufWriter::new(&file), |received| {
        if last.elapsed() < PROGRESS_EVERY && received != total {
            return true;
        }
        last = Instant::now();
        progress(VoiceDownload { received, total });
        go_on()
    })
    // A read given up as `go_on` said stop is a stop, not a network failure.
    .map_err(|e| if stopped.load(Ordering::Relaxed) { AppError::VoiceDownloadStopped } else { voice_error(e) })?;
    file.sync_all().map_err(|e| AppError::file(FileAction::Write, archive, e))?;
    Ok(sha)
}

/// `from`, read on a thread of its own, so that a read that stalls (the
/// venue's network dropped) does not keep the download from stopping:
/// while no data comes, `go_on` is asked every `every`, and once it says
/// stop the read fails at once. The thread then ends with its read, at the
/// latest when the agent's timeout ends it. Fails when the system gives no
/// thread.
fn unstalled<'a>(mut from: impl Read + Send + 'static, every: Duration, go_on: impl FnMut() -> bool + 'a) -> std::io::Result<impl Read + 'a> {
    let (chunks, received) = mpsc::sync_channel::<std::io::Result<Vec<u8>>>(4);
    thread::Builder::new().name("voice-download".into()).spawn(move || {
        let mut buffer = vec![0u8; 1 << 16];
        loop {
            let read = from.read(&mut buffer).map(|n| buffer[..n].to_vec());
            let last = !matches!(&read, Ok(chunk) if !chunk.is_empty());
            // Nobody waits any more: the download stopped.
            if chunks.send(read).is_err() || last {
                return;
            }
        }
    })?;
    Ok(Unstalled { received, pending: std::io::Cursor::new(Vec::new()), every, go_on })
}

/// The reading end of `unstalled`.
struct Unstalled<F> {
    received: Receiver<std::io::Result<Vec<u8>>>,
    /// What came and is not read yet.
    pending: std::io::Cursor<Vec<u8>>,
    every: Duration,
    go_on: F,
}

impl<F: FnMut() -> bool> Read for Unstalled<F> {
    fn read(&mut self, into: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let n = self.pending.read(into)?;
            if n > 0 || into.is_empty() {
                return Ok(n);
            }
            match self.received.recv_timeout(self.every) {
                Ok(Ok(chunk)) if chunk.is_empty() => return Ok(0),
                Ok(Ok(chunk)) => self.pending = std::io::Cursor::new(chunk),
                Ok(Err(e)) => return Err(e),
                Err(RecvTimeoutError::Timeout) if (self.go_on)() => {}
                Err(RecvTimeoutError::Timeout) => return Err(std::io::Error::other("stopped")),
                Err(RecvTimeoutError::Disconnected) => return Err(std::io::ErrorKind::UnexpectedEof.into()),
            }
        }
    }
}

/// Downloads over HTTPS only, redirects included; what arrives is checked
/// by its hash, so the host it comes from does not matter.
fn agent() -> ureq::Agent {
    crate::https_agent(MAX_REDIRECTS, DOWNLOAD_TIMEOUT)
}

/// The next line of the voice process's output, without its end; at most
/// `MAX_LINE_BYTES` of it, the rest of a longer one passed over, so that
/// output with no line end cannot fill the memory. None when it ends.
fn read_line(output: &mut impl BufRead) -> Option<Vec<u8>> {
    let mut line = Vec::new();
    let read = output.by_ref().take(MAX_LINE_BYTES).read_until(b'\n', &mut line).ok()?;
    if read == 0 {
        return None;
    }
    if line.last() == Some(&b'\n') {
        line.pop();
    } else if read as u64 == MAX_LINE_BYTES {
        output.skip_until(b'\n').ok()?;
    }
    Some(line)
}

fn voice_error(e: voice::VoiceError) -> AppError {
    match e {
        voice::VoiceError::Download(detail) => AppError::VoiceDownload { detail },
        voice::VoiceError::Disk(detail) => AppError::VoiceDisk { detail },
        voice::VoiceError::Damaged => AppError::VoiceDamaged,
        // The app stops a download for a show that began or a cancel (`voice_install`, `stopped_why`).
        voice::VoiceError::Stopped => AppError::VoiceDownloadStopped,
        voice::VoiceError::Load | voice::VoiceError::Render => AppError::VoiceFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(name: &str) -> (Voice, PathBuf) {
        let dir = std::env::temp_dir().join(format!("voice-shell-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        (Voice::at(dir.join("voice"), dir.join("voice-clips")), dir)
    }

    fn line(text: &str) -> Line {
        Line { question: text.into(), answers: vec![], language: quiz_core::QuizLanguage::Cs, voice: 0, speed: quiz_core::DEFAULT_VOICE_SPEED }
    }

    #[test]
    fn a_long_output_line_is_cut_and_the_next_one_still_read() {
        let long = "x".repeat(3 * MAX_LINE_BYTES as usize);
        let output = format!("{long}\n\"ready\"\nlast");
        let mut output = std::io::Cursor::new(output.into_bytes());
        assert_eq!(read_line(&mut output).map(|line| line.len() as u64), Some(MAX_LINE_BYTES));
        assert_eq!(read_line(&mut output), Some(b"\"ready\"".to_vec()));
        assert_eq!(read_line(&mut output), Some(b"last".to_vec()));
        assert_eq!(read_line(&mut output), None);
    }

    #[test]
    fn a_stopped_download_says_whether_it_was_cancelled() {
        assert_eq!(stopped_why(AppError::VoiceDownloadStopped, true), AppError::VoiceDownloadCancelled);
        assert_eq!(stopped_why(AppError::VoiceDownloadStopped, false), AppError::VoiceDownloadStopped, "a show began");
        assert_eq!(stopped_why(AppError::VoiceDamaged, true), AppError::VoiceDamaged, "it failed before");
    }

    /// A connection that sends `first`, then nothing until the test ends.
    struct Stalls {
        first: Option<Vec<u8>>,
        hold: Receiver<()>,
    }

    impl Read for Stalls {
        fn read(&mut self, into: &mut [u8]) -> std::io::Result<usize> {
            if let Some(first) = self.first.take() {
                into[..first.len()].copy_from_slice(&first);
                return Ok(first.len());
            }
            let _ = self.hold.recv();
            Ok(0)
        }
    }

    #[test]
    fn a_stalled_download_stops_once_asked_without_waiting_for_data() {
        let (release, hold) = mpsc::channel();
        let asked = std::cell::Cell::new(0);
        let mut body = unstalled(Stalls { first: Some(b"abc".to_vec()), hold }, Duration::from_millis(10), || {
            asked.set(asked.get() + 1);
            asked.get() < 3
        })
        .unwrap();
        let mut got = [0u8; 8];
        assert_eq!(body.read(&mut got).unwrap(), 3);
        assert_eq!(&got[..3], b"abc");
        let began = Instant::now();
        assert!(body.read(&mut got).is_err(), "stopped while the connection stalls");
        assert_eq!(asked.get(), 3);
        assert!(began.elapsed() < Duration::from_secs(5));
        drop(release);
    }

    #[test]
    fn an_unstalled_read_passes_everything_on_to_the_end() {
        let data: Vec<u8> = (0..200_000u32).map(|i| i as u8).collect();
        let mut body = unstalled(std::io::Cursor::new(data.clone()), Duration::from_millis(10), || true).unwrap();
        let mut got = Vec::new();
        body.read_to_end(&mut got).unwrap();
        assert_eq!(got, data);
    }

    #[test]
    fn a_download_goes_over_https_only() {
        let agent = agent();
        assert!(agent.config().https_only());
        assert!(MODEL.url.starts_with("https://"));
    }

    #[test]
    fn a_kept_clip_is_found_and_nothing_is_rendered_without_the_model() {
        let (voice, dir) = voice("kept");
        let no_exe = || -> CmdResult<PathBuf> { panic!("no process is started") };
        assert_eq!(voice.clip(&line(" "), true, no_exe).unwrap(), None);
        assert_eq!(voice.clip(&line("Kolik?"), false, no_exe).unwrap(), None);
        assert_eq!(voice.clip(&line("Kolik?"), true, no_exe), Err(AppError::VoiceMissing));
        fs::create_dir_all(&voice.clips).unwrap();
        let kept = voice.clips.join(line("Kolik?").clip_name());
        fs::write(&kept, b"RIFF").unwrap();
        assert_eq!(voice.clip(&line("Kolik?"), true, no_exe).unwrap(), Some(kept));
        assert!(!voice.model().installed);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_voice_process_that_cannot_start_is_not_started_again_until_the_model_changes() {
        let (voice, dir) = voice("broken");
        let model = voice::model::model_dir(&voice.root);
        fs::create_dir_all(&model).unwrap();
        for file in voice::model::MODEL_FILES.iter().chain(["installed"].iter()) {
            fs::write(model.join(file), b"x").unwrap();
        }
        assert!(voice.model().installed);
        let fails = || -> CmdResult<PathBuf> { Err(AppError::VoiceFailed) };
        assert_eq!(voice.clip(&line("Kolik?"), true, fails), Err(AppError::VoiceFailed));
        let no_exe = || -> CmdResult<PathBuf> { panic!("not started again") };
        assert_eq!(voice.clip(&line("Kde?"), true, no_exe), Err(AppError::VoiceFailed));
        voice.stop();
        assert_eq!(voice.clip(&line("Kde?"), true, fails), Err(AppError::VoiceFailed));
        let _ = fs::remove_dir_all(&dir);
    }

    /// The real voice process, when `VOICE_MODEL_ROOT` names a folder the
    /// model is installed under and the app binary is built (its path in
    /// `BUZZDOME_APP`): a clip is rendered once, then kept.
    #[test]
    fn the_voice_process_renders_a_clip_once() {
        let (Some(root), Some(exe)) = (std::env::var_os("VOICE_MODEL_ROOT"), std::env::var_os("BUZZDOME_APP")) else {
            eprintln!("VOICE_MODEL_ROOT or BUZZDOME_APP not set: skipped");
            return;
        };
        let (_, dir) = voice("process");
        let voice = Voice::at(PathBuf::from(root), dir.join("voice-clips"));
        let exe = || Ok(PathBuf::from(&exe));
        let clip = voice.clip(&line("Kolik nohou má pavouk?"), true, exe).unwrap().unwrap();
        assert!(fs::metadata(&clip).unwrap().len() > 44 + 44_100);
        let no_exe = || -> CmdResult<PathBuf> { panic!("kept, so not rendered again") };
        assert_eq!(voice.clip(&line("Kolik nohou má pavouk?"), true, no_exe).unwrap(), Some(clip));
        let _ = fs::remove_dir_all(&dir);
    }
}
