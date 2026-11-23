//! Writes down what a recording dropped into the AI panel says, offline,
//! so that the panel's model, which reads no sound, gets its words as text
//! (spec QZ-14). The speech recognition model (`voice::LISTENER`) is
//! downloaded once into the app's local data (`listener/`) and checked
//! against its hash, as the voice model is (`read_aloud`). A recording is
//! written down in the listening process, the app started again with
//! `--listen <folder> <language>`, which ends with it: the engine's
//! native code ending badly ends only that process, and it holds the
//! model's gigabyte of memory only while it works.

use crate::error::{AppError, CmdResult};
use crate::read_aloud::{download, Unpacking, VoiceDownload};
use crate::media_player::header;
use crate::{app_log, off_ui, refuse_during_show};
use quiz_core::QuizLanguage;
use quiz_file::FileAction;
use serde::Serialize;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::ipc::{Channel, InvokeBody, Request};
use tauri::{AppHandle, Manager};
use ts_rs::TS;
use voice::listen::{MAX_SECONDS, RATE};
use voice::LISTENER;

/// The argument that starts the app as the listening process.
const LISTEN: &str = "--listen";
/// The model's folder, in the app's local data.
const MODEL_FOLDER: &str = "listener";
/// The headers of `editor_transcribe`, whose body is the recording's samples.
const NAME_HEADER: &str = "name";
const LANGUAGE_HEADER: &str = "language";
/// How long the process may take to load the model.
const LOAD_TIME: Duration = Duration::from_secs(90);
/// How long it may take for each second heard: about 0.4 s on a 4-core
/// container's CPU [no source - measured: 5.2 s heard in 2.0 s], so this
/// leaves room for a slower laptop.
const TIME_PER_SECOND: Duration = Duration::from_millis(2500);
/// How often a cancel and the process are looked at.
const POLL_EVERY: Duration = Duration::from_millis(100);
/// The most of the process's output read: the words of `MAX_SECONDS`.
const MAX_TEXT_BYTES: u64 = 1 << 20;

pub struct Listening {
    root: PathBuf,
    /// Held while the model is downloaded or removed.
    installing: Mutex<()>,
    /// The quizmaster cancelled the download under way.
    download_cancelled: AtomicBool,
    /// Set to stop the recording being written down, one at a time.
    stop: Mutex<Option<Arc<AtomicBool>>>,
}

/// Whether the speech recognition model is there, for the AI panel.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ListenerModel {
    pub installed: bool,
    /// The download's size.
    pub bytes: u64,
    /// The samples a second a recording is sent at (`editor_transcribe`).
    pub rate: u32,
    /// The most seconds of a recording that are written down.
    pub max_seconds: u32,
}

impl Listening {
    pub fn new(app: &AppHandle) -> CmdResult<Listening> {
        Ok(Listening { root: crate::app_local_data(app, MODEL_FOLDER)?, installing: Mutex::new(()), download_cancelled: AtomicBool::new(false), stop: Mutex::new(None) })
    }

    fn model(&self) -> ListenerModel {
        ListenerModel { installed: LISTENER.installed(&self.root), bytes: LISTENER.bytes, rate: RATE as u32, max_seconds: MAX_SECONDS as u32 }
    }

    /// Stops the recording being written down, if one is.
    fn cancel(&self) {
        if let Some(stop) = self.stop.lock().unwrap().as_ref() {
            stop.store(true, Ordering::SeqCst);
        }
    }
}

/// A show that begins stops writing a recording down, so the process gives
/// the show its CPU.
pub(crate) fn cancel_for_show(app: &AppHandle) {
    if let Some(listening) = app.try_state::<Listening>() {
        listening.cancel();
    }
}

/// Whether the speech recognition model is downloaded.
#[tauri::command]
pub async fn listener_model(app: AppHandle) -> CmdResult<ListenerModel> {
    off_ui(move || Ok(app.state::<Listening>().model())).await
}

/// Downloads the speech recognition model, checks it and unpacks it,
/// telling `progress` how the download goes; as `read_aloud::voice_install`
/// does for the voice, refused during a show, stopped by one that begins
/// or a cancel (`listener_install_cancel`).
#[tauri::command]
pub async fn listener_install(app: AppHandle, progress: Channel<VoiceDownload>) -> CmdResult<ListenerModel> {
    off_ui(move || {
        refuse_during_show(&app)?;
        let state = app.state::<Listening>();
        let _installing = state.installing.lock().unwrap();
        if LISTENER.installed(&state.root) {
            return Ok(state.model());
        }
        fs::create_dir_all(&state.root).map_err(|e| AppError::file(FileAction::CreateFolder, &state.root, e))?;
        let archive = state.root.join(format!("{}.download", LISTENER.id));
        state.download_cancelled.store(false, Ordering::SeqCst);
        let cancelled = || state.download_cancelled.load(Ordering::SeqCst);
        let downloaded = download(
            &LISTENER,
            &archive,
            |done| {
                let _ = progress.send(done);
            },
            || !cancelled() && refuse_during_show(&app).is_ok(),
            listener_error,
        )
        .map_err(|e| match e {
            AppError::ListenerDownloadStopped if cancelled() => AppError::ListenerDownloadCancelled,
            e => e,
        });
        let installed = downloaded.and_then(|sha| {
            let _unpacking = Unpacking::begin(&app)?;
            LISTENER.unpack(&archive, &sha, &state.root).map_err(listener_error)
        });
        let _ = fs::remove_file(&archive);
        installed?;
        app_log::note(&format!("speech recognition model {} installed", LISTENER.id));
        Ok(state.model())
    })
    .await
}

/// Cancels the model's download under way, which then fails with `ListenerDownloadCancelled`.
#[tauri::command]
pub fn listener_install_cancel(app: AppHandle) {
    app.state::<Listening>().download_cancelled.store(true, Ordering::SeqCst);
}

/// Removes the speech recognition model. Refused during a show.
#[tauri::command]
pub async fn listener_remove(app: AppHandle) -> CmdResult<ListenerModel> {
    off_ui(move || {
        refuse_during_show(&app)?;
        let state = app.state::<Listening>();
        let _installing = state.installing.lock().unwrap();
        LISTENER.remove(&state.root).map_err(listener_error)?;
        Ok(state.model())
    })
    .await
}

/// What the recording sent as its samples (`voice::listen::RATE` a
/// second, mono, 32-bit floats, little-endian) says, in the language of
/// `LANGUAGE_HEADER` (`cs` or `en`); `NAME_HEADER` names it for a failure.
/// At most `MAX_SECONDS` of it are heard. Refused during a show; a show
/// that begins or `editor_transcribe_cancel` stops it.
#[tauri::command]
pub async fn editor_transcribe(app: AppHandle, request: Request<'_>) -> CmdResult<String> {
    let InvokeBody::Raw(bytes) = request.body() else { return Err(AppError::system("a recording comes as its samples")) };
    let name = header(&request, NAME_HEADER);
    let language = QuizLanguage::of_code(&header(&request, LANGUAGE_HEADER)).unwrap_or_default();
    let samples = bytes[..bytes.len().min(MAX_SECONDS * RATE * 4) / 4 * 4].to_vec();
    off_ui(move || {
        let state = app.state::<Listening>();
        // Set before the show is checked, so a show that begins after the check stops it (`cancel_for_show`).
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut current = state.stop.lock().unwrap();
            if let Some(earlier) = current.replace(stop.clone()) {
                earlier.store(true, Ordering::SeqCst);
            }
        }
        refuse_during_show(&app)?;
        if !LISTENER.installed(&state.root) {
            return Err(AppError::ListenerMissing);
        }
        let exe = std::env::current_exe().map_err(|_| AppError::ListenerFailed { name: name.clone() })?;
        let started = Instant::now();
        let heard = transcribe(&exe, &state.root, language, samples, &stop);
        app_log::note(&format!("listener: {name} in {} ms: {}", started.elapsed().as_millis(), heard.as_ref().map_or("failed", |_| "done")));
        match heard {
            Some(text) => Ok(text),
            None if stop.load(Ordering::SeqCst) => Err(crate::local_model::stopped_for_show(&app, AppError::ListenerCancelled)),
            None => Err(AppError::ListenerFailed { name }),
        }
    })
    .await
}

/// Stops the recording being written down, which then fails with `ListenerCancelled`.
#[tauri::command]
pub fn editor_transcribe_cancel(app: AppHandle) {
    app.state::<Listening>().cancel();
}

/// Starts the listening process for `samples` (as bytes) and waits for its
/// words; None when it fails, takes too long or `stop` is set, which ends it.
fn transcribe(exe: &std::path::Path, root: &std::path::Path, language: QuizLanguage, samples: Vec<u8>, stop: &AtomicBool) -> Option<String> {
    let seconds = samples.len() / 4 / RATE;
    let mut command = Command::new(exe);
    command.arg(LISTEN).arg(root).arg(language.code()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, windows_sys::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS);
    let mut child = Ended(command.spawn().ok()?);
    let mut input = child.0.stdin.take()?;
    let output = child.0.stdout.take()?;
    // Written and read on threads of their own, so a process that stops reading or writing cannot hold this one.
    thread::spawn(move || {
        let _ = input.write_all(&samples);
    });
    let (send, words) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut text = String::new();
        let read = output.take(MAX_TEXT_BYTES).read_to_string(&mut text);
        let _ = send.send(read.ok().map(|_| text));
    });
    let deadline = Instant::now() + LOAD_TIME + TIME_PER_SECOND * seconds as u32;
    loop {
        if stop.load(Ordering::SeqCst) || Instant::now() > deadline {
            return None;
        }
        if let Ok(Some(status)) = child.0.try_wait() {
            let text = words.recv_timeout(Duration::from_secs(5)).ok().flatten()?;
            return status.success().then(|| text.trim().to_string());
        }
        thread::sleep(POLL_EVERY);
    }
}

/// A child process that is ended when this is dropped.
struct Ended(Child);

impl Drop for Ended {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Called first in `main`: when the app was started as the listening
/// process, writes down the samples on its input and ends.
pub fn serve_if_asked() {
    let Some(mut args) = crate::started_as(LISTEN) else { return };
    let (Some(root), Some(language)) = (args.next(), args.next()) else { std::process::exit(2) };
    // A crash here ends this process quietly, as the voice process's does.
    #[cfg(windows)]
    // SAFETY: SetErrorMode only changes how this process reports errors.
    unsafe {
        use windows_sys::Win32::System::Diagnostics::Debug::{SetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX};
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    }
    #[cfg(unix)]
    // SAFETY: nice only changes this process's priority.
    unsafe {
        libc::nice(10);
    }
    let language = language.to_str().and_then(QuizLanguage::of_code).unwrap_or_default();
    let mut bytes = Vec::new();
    if std::io::stdin().lock().take((MAX_SECONDS * RATE * 4) as u64).read_to_end(&mut bytes).is_err() {
        std::process::exit(3);
    }
    let samples: Vec<f32> = bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).map(|s| if s.is_finite() { s.clamp(-1.0, 1.0) } else { 0.0 }).collect();
    let Ok(listener) = voice::listen::Listener::load(std::path::Path::new(&root), language) else { std::process::exit(1) };
    let Ok(text) = listener.transcribe(&samples, |_| {}) else { std::process::exit(1) };
    let mut out = std::io::stdout().lock();
    let written = out.write_all(text.as_bytes()).and_then(|()| out.flush());
    std::process::exit(if written.is_ok() { 0 } else { 1 });
}

fn listener_error(e: voice::VoiceError) -> AppError {
    match e {
        voice::VoiceError::Download(detail) => AppError::ListenerDownload { detail },
        voice::VoiceError::Disk(detail) => AppError::ListenerDisk { detail },
        voice::VoiceError::Damaged => AppError::ListenerDamaged,
        voice::VoiceError::Stopped => AppError::ListenerDownloadStopped,
        voice::VoiceError::Load | voice::VoiceError::Render => AppError::ListenerFailed { name: String::new() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_download_is_said_as_the_listener_s() {
        assert_eq!(listener_error(voice::VoiceError::Stopped), AppError::ListenerDownloadStopped);
        assert_eq!(listener_error(voice::VoiceError::Damaged), AppError::ListenerDamaged);
        assert_eq!(listener_error(voice::VoiceError::Download("x".into())), AppError::ListenerDownload { detail: "x".into() });
    }

    #[test]
    fn a_process_that_does_not_start_or_is_stopped_gives_no_words() {
        let root = std::env::temp_dir();
        assert_eq!(transcribe(std::path::Path::new("/no/such/program"), &root, QuizLanguage::Cs, vec![0; 64], &AtomicBool::new(false)), None);
        // This test program, given arguments it does not know, ends at once without words; stopped first, it gives none either.
        let exe = std::env::current_exe().unwrap();
        assert_eq!(transcribe(&exe, &root, QuizLanguage::Cs, vec![0; 64], &AtomicBool::new(true)), None);
    }
}
