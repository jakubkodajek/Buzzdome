//! The buzzer receiver inside the app: remembered settings, the receiver
//! thread, learn mode and the commands behind the Director's Receiver tab.
//!
//! Hardware presses take the same path as keyboard and on-screen presses
//! (`Command::Press` through `execute_at`), so they are journaled and replayed
//! like everything else.

use buzzers::hid::{udev_rule, DeviceInfo, DeviceSelector, HidBackend};
use buzzers::hidapi_backend::HidapiBackend;
use crate::app_file::{AppFile, SettingsLost};
use crate::error::{AppError, CmdResult};
use buzzers::learned::{self, LearnedMapping, Recorder};
use buzzers::protocol::{format_hex, Protocol};
use buzzers::receiver::{self, RawReport, ReceiverConfig, ReceiverEvent, ReceiverHandle, ReceiverStatus};
use buzzers::Press;
use quiz_core::{BuzzerId, Button, Command};
use quiz_file::FileAction;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{AppHandle, Emitter, Manager};

const SETTINGS_FILE: &str = "receiver.json";
const RAW_LOG_LEN: usize = 2000;
/// How many recent reports the Receiver tab gets when it opens.
const RAW_LOG_SNAPSHOT: usize = 50;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReceiverSettings {
    #[serde(default)]
    receiver: Option<DeviceSelector>,
    #[serde(default)]
    learned: Vec<LearnedMapping>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LearnProgress {
    active: bool,
    buzzer: Option<BuzzerId>,
    button: Option<Button>,
    samples: usize,
    /// Enough presses are recorded to save the button.
    can_save: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ReceiverInfo {
    /// None when USB-HID could not be initialised on this system.
    status: Option<ReceiverStatus>,
    /// Why USB-HID could not be initialised: the system's message.
    init_error: Option<String>,
    settings_error: Option<SettingsLost>,
    learned_buttons: usize,
    learn: LearnProgress,
    recent: Vec<RawReport>,
    platform: &'static str,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
struct PressEvent {
    press: Press,
    /// False when the press was only recorded (learn mode) or no show is running.
    used: bool,
}

pub struct Hardware {
    backend: Option<Arc<HidapiBackend>>,
    handle: Option<ReceiverHandle>,
    init_error: Option<String>,
    settings: Arc<Mutex<ReceiverSettings>>,
    /// Where they are kept, and why they were not read, if so.
    settings_file: AppFile,
    learn: Mutex<Option<Recorder>>,
    raw_log: Mutex<VecDeque<RawReport>>,
}

/// Receiver events waiting for `spawn_event_pump`.
pub type ReceiverEvents = mpsc::Receiver<ReceiverEvent>;

impl Hardware {
    /// Starts the receiver thread and reconnects to the remembered receiver.
    /// Its events queue up until `spawn_event_pump` runs, which must be after
    /// the returned `Hardware` is managed by the app (the pump reads it).
    pub fn start(app: &AppHandle) -> (Hardware, Option<ReceiverEvents>) {
        let settings_path = app.path().app_data_dir().map(|d| d.join(SETTINGS_FILE)).unwrap_or_else(|_| SETTINGS_FILE.into());
        // Unreadable settings are reported in the Receiver tab: learned buttons are gone until learned again.
        let (settings_file, settings) = AppFile::load::<ReceiverSettings>(settings_path);
        let remembered = settings.receiver.clone();
        let settings = Arc::new(Mutex::new(settings));

        let backend = match HidapiBackend::new() {
            Ok(b) => Arc::new(b),
            Err(e) => {
                let hardware = Hardware {
                    backend: None,
                    handle: None,
                    init_error: Some(e.to_string()),
                    settings,
                    settings_file,
                    learn: Mutex::new(None),
                    raw_log: Mutex::new(VecDeque::new()),
                };
                return (hardware, None);
            }
        };

        let resolver_settings = settings.clone();
        let resolver = Arc::new(move |device: &DeviceInfo| -> Box<dyn Protocol> {
            learned::protocol_for(&resolver_settings.lock().unwrap().learned, device)
        });
        let (tx, rx) = mpsc::channel();
        let (handle, _join) = receiver::spawn(backend.clone(), resolver, tx, ReceiverConfig::default());
        handle.select(remembered);

        let hardware = Hardware {
            backend: Some(backend),
            handle: Some(handle),
            init_error: None,
            settings,
            settings_file,
            learn: Mutex::new(None),
            raw_log: Mutex::new(VecDeque::new()),
        };
        (hardware, Some(rx))
    }

    pub fn handle(&self) -> Option<ReceiverHandle> {
        self.handle.clone()
    }

    /// A copy of the settings to change and pass to `store_settings`.
    fn settings(&self) -> ReceiverSettings {
        self.settings.lock().unwrap().clone()
    }

    /// Writes `settings` to disk and only then uses them, so the app never
    /// works with settings that a restart would lose.
    fn store_settings(&self, settings: ReceiverSettings) -> CmdResult<()> {
        self.settings_file.store(&settings)?;
        *self.settings.lock().unwrap() = settings;
        Ok(())
    }

    /// Ends learn mode, if on, so presses play again.
    fn end_learning(&self, app: &AppHandle) {
        stop_learning(&mut self.learn.lock().unwrap(), app);
    }

    fn learn_progress(&self) -> LearnProgress {
        progress(self.learn.lock().unwrap().as_ref())
    }

    fn connected_device(&self) -> Option<DeviceInfo> {
        match self.handle.as_ref()?.status() {
            ReceiverStatus::Connected { device, .. } => Some(device),
            _ => None,
        }
    }
}

/// What the Receiver tab shows of learn mode.
fn progress(recorder: Option<&Recorder>) -> LearnProgress {
    match recorder {
        Some(r) => LearnProgress {
            active: true,
            buzzer: Some(r.buzzer),
            button: Some(r.button),
            samples: r.sample_count(),
            can_save: r.can_save(),
        },
        None => LearnProgress { active: false, buzzer: None, button: None, samples: 0, can_save: false },
    }
}

/// Ends the learn mode in `learn`, if on, and tells the windows.
fn stop_learning(learn: &mut Option<Recorder>, app: &AppHandle) {
    if learn.take().is_some() {
        let _ = app.emit("receiver-learn", progress(None));
    }
}

/// Starts learning `button` of `buzzer` in `learn`, unless a show runs: in
/// a show every press must play, and learn mode drops them (A14).
fn start_learning(learn: &mut Option<Recorder>, show_running: bool, buzzer: BuzzerId, button: Button) -> CmdResult<()> {
    if show_running {
        return Err(AppError::ShowRunning);
    }
    *learn = Some(Recorder::new(buzzer, button));
    Ok(())
}

/// Ends learn mode for a new show, as during a show every press must play.
/// Gives the learn lock, to keep until the show state is locked, so learn
/// mode cannot start in between: `receiver_learn_start` too takes the learn
/// lock first, then the show's.
pub fn end_learning(app: &AppHandle) -> Option<MutexGuard<'_, Option<Recorder>>> {
    let hw = app.try_state::<Hardware>()?.inner();
    let mut learn = hw.learn.lock().unwrap();
    stop_learning(&mut learn, app);
    Some(learn)
}

/// Moves receiver events into the game and out to the windows.
pub fn handle_event(app: &AppHandle, event: ReceiverEvent) {
    let hw = app.state::<Hardware>();
    match event {
        ReceiverEvent::Status(status) => {
            // Nothing more can be learned without the receiver.
            if !matches!(status, ReceiverStatus::Connected { .. }) {
                hw.end_learning(app);
            }
            let _ = app.emit("receiver-status", status);
        }
        ReceiverEvent::Raw(report) => {
            let sampled = hw.learn.lock().unwrap().as_mut().is_some_and(|r| r.feed(&report.bytes, report.at_ms));
            {
                let mut log = hw.raw_log.lock().unwrap();
                if log.len() == RAW_LOG_LEN {
                    log.pop_front();
                }
                log.push_back(report.clone());
            }
            let _ = app.emit("receiver-raw", report);
            if sampled {
                let _ = app.emit("receiver-learn", hw.learn_progress());
            }
        }
        ReceiverEvent::Press { press, at_ms } => {
            // While learning, presses only teach; they must not play.
            let learning = hw.learn.lock().unwrap().is_some();
            let command = Command::Press { buzzer: press.buzzer, button: press.button };
            let used = !learning && crate::press_from_hardware(app, command, at_ms);
            let _ = app.emit("receiver-press", PressEvent { press, used });
        }
    }
}

#[tauri::command]
pub fn receiver_info(hw: tauri::State<Hardware>) -> ReceiverInfo {
    let log = hw.raw_log.lock().unwrap();
    let skip = log.len().saturating_sub(RAW_LOG_SNAPSHOT);
    ReceiverInfo {
        status: hw.handle.as_ref().map(ReceiverHandle::status),
        init_error: hw.init_error.clone(),
        settings_error: hw.settings_file.lost().cloned(),
        learned_buttons: hw.settings.lock().unwrap().learned.iter().map(|m| m.signatures.len()).sum(),
        learn: hw.learn_progress(),
        recent: log.iter().skip(skip).cloned().collect(),
        platform: std::env::consts::OS,
    }
}

#[tauri::command]
pub fn receiver_devices(hw: tauri::State<Hardware>) -> CmdResult<Vec<DeviceInfo>> {
    let backend = hw.backend.as_ref().ok_or(AppError::UsbUnavailable)?;
    let mut devices = backend.list().map_err(|e| AppError::UsbListFailed { detail: e.to_string() })?;
    devices.sort_by_key(|d| (d.is_keyboard_or_mouse(), d.vendor_id, d.product_id, d.interface_number));
    Ok(devices)
}

/// Uses this receiver from now on (also after restarts), or none.
#[tauri::command]
pub fn receiver_select(hw: tauri::State<Hardware>, device: Option<DeviceInfo>) -> CmdResult<()> {
    let selector = device.as_ref().map(DeviceSelector::for_device);
    hw.store_settings(ReceiverSettings { receiver: selector.clone(), ..hw.settings() })?;
    hw.handle.as_ref().ok_or(AppError::UsbUnavailable)?.select(selector);
    Ok(())
}

#[tauri::command]
pub fn receiver_learn_start(app: AppHandle, hw: tauri::State<Hardware>, buzzer: BuzzerId, button: Button) -> CmdResult<()> {
    if hw.connected_device().is_none() {
        return Err(AppError::ReceiverNotConnected);
    }
    // The learn lock first, then the show's, as for a new show (`end_learning`).
    start_learning(&mut hw.learn.lock().unwrap(), crate::show_running(&app), buzzer, button)?;
    let _ = app.emit("receiver-learn", hw.learn_progress());
    Ok(())
}

#[tauri::command]
pub fn receiver_learn_cancel(app: AppHandle, hw: tauri::State<Hardware>) {
    hw.end_learning(&app);
}

/// Saves the recorded presses as the signature of the button being learned,
/// and returns that button.
#[tauri::command]
pub fn receiver_learn_finish(app: AppHandle, hw: tauri::State<Hardware>) -> CmdResult<Press> {
    let device = hw.connected_device().ok_or(AppError::ReceiverNotConnected)?;
    let recorder = hw.learn.lock().unwrap().clone().ok_or(AppError::LearnNotStarted)?;
    let mut settings = hw.settings();
    // Too few presses or a clash with another button: learn mode stays on to fix it.
    recorder.save_into(&mut settings.learned, &device)?;
    let stored = hw.store_settings(settings);
    // Stored or not, learn mode ends, so presses play again.
    hw.end_learning(&app);
    stored?;
    if let Some(handle) = hw.handle.as_ref() {
        handle.reload_protocol();
    }
    Ok(Press { buzzer: recorder.buzzer, button: recorder.button })
}

#[tauri::command]
pub fn receiver_forget_learned(hw: tauri::State<Hardware>) -> CmdResult<()> {
    hw.store_settings(ReceiverSettings { learned: Vec::new(), ..hw.settings() })?;
    if let Some(handle) = hw.handle.as_ref() {
        handle.reload_protocol();
    }
    Ok(())
}

/// Writes the recent raw reports to a text file, e.g. to attach to a bug
/// report or to work out the receiver's protocol.
#[tauri::command]
pub fn receiver_export_log(hw: tauri::State<Hardware>, path: String) -> CmdResult<()> {
    let status = hw.handle.as_ref().map(ReceiverHandle::status);
    let mut out = format!(
        "# Receiver log, app {} on {}\n# status: {}\n# unix-ms\tbytes (hex)\tdecoded\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        serde_json::to_string(&status).unwrap_or_default()
    );
    for r in hw.raw_log.lock().unwrap().iter() {
        let decoded: Vec<String> = r.decoded.iter().map(|p| format!("{}{:?}", p.buzzer, p.button)).collect();
        out.push_str(&format!("{}\t{}\t{}\n", r.at_ms, format_hex(&r.bytes), decoded.join(",")));
    }
    std::fs::write(&path, out).map_err(|e| AppError::file(FileAction::Write, Path::new(&path), e))
}

#[tauri::command]
pub fn receiver_udev_rule(hw: tauri::State<Hardware>) -> String {
    // The rule matters most when opening fails, so fall back to the chosen receiver.
    let ids = hw
        .connected_device()
        .map(|d| (d.vendor_id, d.product_id))
        .or_else(|| hw.settings.lock().unwrap().receiver.as_ref().map(|s| (s.vendor_id, s.product_id)));
    udev_rule(ids.map(|i| i.0), ids.map(|i| i.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learn_mode_does_not_start_during_a_show() {
        let mut learn = None;
        assert_eq!(start_learning(&mut learn, true, 3, Button::A), Err(AppError::ShowRunning));
        assert!(learn.is_none());
        start_learning(&mut learn, false, 3, Button::A).unwrap();
        assert_eq!(learn.map(|r| (r.buzzer, r.button)), Some((3, Button::A)));
    }

    fn hardware(settings_path: std::path::PathBuf) -> Hardware {
        let (settings_file, settings) = AppFile::load(settings_path);
        Hardware {
            backend: None,
            handle: None,
            init_error: None,
            settings: Arc::new(Mutex::new(settings)),
            settings_file,
            learn: Mutex::new(None),
            raw_log: Mutex::new(VecDeque::new()),
        }
    }

    fn receiver() -> Option<DeviceSelector> {
        Some(DeviceSelector { vendor_id: 1, product_id: 2, usage_page: None, interface_number: None })
    }

    #[test]
    fn settings_are_used_only_once_they_are_on_disk() {
        let dir = std::env::temp_dir().join(format!("receiver-settings-{}", std::process::id()));
        // A folder where the file should be: writing it fails.
        let blocked = hardware(dir.join("blocked"));
        std::fs::create_dir_all(dir.join("blocked")).unwrap();
        assert!(blocked.store_settings(ReceiverSettings { receiver: receiver(), ..blocked.settings() }).is_err());
        assert!(blocked.settings().receiver.is_none());

        let fine = hardware(dir.join(SETTINGS_FILE));
        fine.store_settings(ReceiverSettings { receiver: receiver(), ..fine.settings() }).unwrap();
        assert_eq!(fine.settings().receiver, receiver());
        assert_eq!(hardware(dir.join(SETTINGS_FILE)).settings().receiver, receiver());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
