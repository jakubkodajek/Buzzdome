//! The buzzer receiver inside the app: remembered settings, the receiver
//! thread, learn mode and the commands behind the Director's Receiver tab.
//!
//! Hardware presses take the same path as keyboard and on-screen presses
//! (`Command::Press` through `execute_at`), so they are journaled and replayed
//! like everything else.

use buzzers::clock::ShowClock;
use buzzers::hid::{udev_rule, DeviceInfo, DeviceSelector, HidBackend};
use buzzers::hidapi_backend::HidapiBackend;
use crate::app_file::{AppFile, SettingsLost};
use crate::dialogs::{self, Picker};
use crate::error::{AppError, CmdResult};
use crate::off_ui;
use crate::show_thread::ShowEvent;
use buzzers::learned::{LearnedMapping, Recorder};
use buzzers::protocol::{self, format_hex, Protocol};
use buzzers::receiver::{self, RawReport, ReceiverChoice, ReceiverConfig, ReceiverEvent, ReceiverHandle, ReceiverStatus};
use buzzers::Press;
use quiz_core::{BuzzerId, Button};
use quiz_file::FileAction;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{AppHandle, Emitter, Manager, Window};

const SETTINGS_FILE: &str = "receiver.json";
const RAW_LOG_LEN: usize = 2000;
/// How many recent reports the Receiver tab gets when it opens.
const RAW_LOG_SNAPSHOT: usize = 50;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReceiverSettings {
    /// The receiver chosen in the Receiver tab; with none, the first one a
    /// built-in protocol knows (`ReceiverChoice::Automatic`).
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

/// A buzzer the receiver heard since it connected: its last press, counted
/// or not, and when that came on the show clock (Unix ms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct BuzzerSeen {
    buzzer: BuzzerId,
    button: Button,
    at_ms: u64,
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
    /// The receiver chosen in the Receiver tab; none when it is found by
    /// itself.
    chosen: Option<DeviceSelector>,
    learned_buttons: usize,
    learn: LearnProgress,
    recent: Vec<RawReport>,
    /// The buzzers heard since the receiver connected, by number.
    buzzers: Vec<BuzzerSeen>,
    platform: &'static str,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
struct PressEvent {
    press: Press,
    /// False when the press did not play: it was only recorded (learn
    /// mode), no show is running, or it changed nothing (`Game::ignores`).
    used: bool,
}

pub struct Hardware {
    backend: Option<Arc<HidapiBackend>>,
    handle: Option<ReceiverHandle>,
    init_error: Option<String>,
    settings: Arc<Mutex<ReceiverSettings>>,
    /// Where they are kept, and why they were not read, if so.
    settings_file: AppFile,
    /// Held while a command changes the settings (`SettingsChange`).
    changing: Mutex<()>,
    learn: Mutex<Option<Recorder>>,
    raw_log: Mutex<VecDeque<RawReport>>,
    /// The buzzers heard since the receiver connected.
    seen: Mutex<BTreeMap<BuzzerId, BuzzerSeen>>,
}

impl Hardware {
    /// Starts the receiver thread and reconnects to the remembered receiver,
    /// or looks for one a built-in protocol knows.
    /// Its events, stamped by `clock`, go to the show thread's queue `events`
    /// (`show_thread`), and wait there until that thread starts.
    pub fn start(app: &AppHandle, events: Sender<ShowEvent>, clock: ShowClock) -> Hardware {
        let settings_path = app.path().app_data_dir().map(|d| d.join(SETTINGS_FILE)).unwrap_or_else(|_| SETTINGS_FILE.into());
        // Unreadable settings are reported in the Receiver tab: learned buttons are gone until learned again.
        let (settings_file, settings) = AppFile::load::<ReceiverSettings>(settings_path);
        let remembered = choice(settings.receiver.clone());
        let settings = Arc::new(Mutex::new(settings));

        let backend = match HidapiBackend::new() {
            Ok(b) => Arc::new(b),
            Err(e) => return Hardware::without_usb(settings_file, settings, Some(e.to_string())),
        };

        let resolver_settings = settings.clone();
        let resolver = Arc::new(move |device: &DeviceInfo| -> Box<dyn Protocol> {
            protocol::for_device(&resolver_settings.lock().unwrap().learned, device)
        });
        let (handle, _join) = receiver::spawn(backend.clone(), resolver, events, ReceiverConfig::new(clock));
        handle.select(remembered);
        Hardware { backend: Some(backend), handle: Some(handle), ..Hardware::without_usb(settings_file, settings, None) }
    }

    /// The receiver's settings with no USB-HID, which failed with
    /// `init_error` if it did.
    fn without_usb(settings_file: AppFile, settings: Arc<Mutex<ReceiverSettings>>, init_error: Option<String>) -> Hardware {
        Hardware {
            backend: None,
            handle: None,
            init_error,
            settings,
            settings_file,
            changing: Mutex::new(()),
            learn: Mutex::new(None),
            raw_log: Mutex::new(VecDeque::new()),
            seen: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn handle(&self) -> Option<ReceiverHandle> {
        self.handle.clone()
    }

    /// Begins a change of the settings, once no other command is changing
    /// them. Only off the windows' thread (`off_ui`), and never under the
    /// show's lock: the file is in the app's data folder with the show's
    /// journal, so a disk that does not answer holds both.
    fn change_settings(&self) -> SettingsChange<'_> {
        SettingsChange { hw: self, _changing: self.changing.lock().unwrap() }
    }

    /// Ends learn mode, if on, so presses play again.
    fn end_learning(&self, app: &AppHandle) {
        stop_learning(&mut self.learn.lock().unwrap(), app);
    }

    fn learn_progress(&self) -> LearnProgress {
        progress(self.learn.lock().unwrap().as_ref())
    }

    /// Notes the buzzers `report` holds presses of, counted or not, and
    /// returns them as they now stand.
    fn hear(&self, report: &RawReport) -> Vec<BuzzerSeen> {
        let heard: BTreeMap<BuzzerId, BuzzerSeen> = report
            .decoded
            .iter()
            .chain(&report.dropped)
            .map(|p| (p.buzzer, BuzzerSeen { buzzer: p.buzzer, button: p.button, at_ms: report.at_ms }))
            .collect();
        self.seen.lock().unwrap().extend(&heard);
        heard.into_values().collect()
    }

    fn connected_device(&self) -> Option<DeviceInfo> {
        match self.handle.as_ref()?.status() {
            ReceiverStatus::Connected { device, .. } => Some(device),
            _ => None,
        }
    }
}

/// One command's change of the settings: read, changed and stored while no
/// other command changes them, so two quick clicks cannot lose either
/// change. The receiver thread reads the settings meanwhile, never waiting
/// for the disk.
struct SettingsChange<'a> {
    hw: &'a Hardware,
    _changing: MutexGuard<'a, ()>,
}

impl SettingsChange<'_> {
    /// A copy of the settings to change and pass to `store`.
    fn settings(&self) -> ReceiverSettings {
        self.hw.settings.lock().unwrap().clone()
    }

    /// Writes `settings` to disk and only then uses them, so the app never
    /// works with settings that a restart would lose.
    fn store(&self, settings: ReceiverSettings) -> CmdResult<()> {
        self.hw.settings_file.store(&settings)?;
        *self.hw.settings.lock().unwrap() = settings;
        Ok(())
    }
}

/// What the receiver thread uses for the receiver chosen in the settings.
fn choice(chosen: Option<DeviceSelector>) -> ReceiverChoice {
    chosen.map_or(ReceiverChoice::Automatic, ReceiverChoice::Device)
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
/// a show every press must play, and learn mode drops them.
fn start_learning(learn: &mut Option<Recorder>, show_running: bool, buzzer: BuzzerId, button: Button) -> CmdResult<()> {
    if show_running {
        return Err(AppError::ShowRunning);
    }
    *learn = Some(Recorder::new(buzzer, button));
    Ok(())
}

/// Ends learn mode for a new show, as during a show every press must play.
pub fn end_learning(app: &AppHandle) {
    if let Some(hw) = app.try_state::<Hardware>() {
        hw.end_learning(app);
    }
}

/// Moves receiver events into the game and out to the windows.
pub fn handle_event(app: &AppHandle, event: ReceiverEvent) {
    let hw = app.state::<Hardware>();
    match event {
        ReceiverEvent::Status(status) => {
            // Nothing more can be learned without the receiver, and the
            // buzzers it heard are to be heard again once it is back.
            if !matches!(status, ReceiverStatus::Connected { .. }) {
                hw.end_learning(app);
                hw.seen.lock().unwrap().clear();
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
            for buzzer in hw.hear(&report) {
                let _ = app.emit("receiver-buzzer", buzzer);
            }
            let _ = app.emit("receiver-raw", report);
            if sampled {
                let _ = app.emit("receiver-learn", hw.learn_progress());
            }
        }
        ReceiverEvent::Press { press, at_ms } => {
            // While learning, presses only teach; they must not play.
            let learning = hw.learn.lock().unwrap().is_some();
            let used = !learning && crate::play_press(app, press, at_ms);
            let _ = app.emit("receiver-press", PressEvent { press, used });
        }
    }
}

#[tauri::command]
pub fn receiver_info(hw: tauri::State<Hardware>) -> ReceiverInfo {
    // One lock at a time.
    let settings = hw.settings.lock().unwrap().clone();
    let buzzers = hw.seen.lock().unwrap().values().copied().collect();
    let log = hw.raw_log.lock().unwrap();
    let skip = log.len().saturating_sub(RAW_LOG_SNAPSHOT);
    ReceiverInfo {
        status: hw.handle.as_ref().map(ReceiverHandle::status),
        init_error: hw.init_error.clone(),
        settings_error: hw.settings_file.lost().cloned(),
        chosen: settings.receiver.clone(),
        learned_buttons: settings.learned.iter().map(|m| m.signatures.len()).sum(),
        learn: hw.learn_progress(),
        recent: log.iter().skip(skip).cloned().collect(),
        buzzers,
        platform: std::env::consts::OS,
    }
}

/// The USB-HID devices plugged in, receivers first; listed off the windows'
/// thread, as listing can take a while (on Windows it goes through SetupAPI).
#[tauri::command]
pub async fn receiver_devices(app: AppHandle) -> CmdResult<Vec<DeviceInfo>> {
    off_ui(move || {
        let backend = app.state::<Hardware>().backend.clone().ok_or(AppError::UsbUnavailable)?;
        let mut devices = backend.list().map_err(|e| AppError::UsbListFailed { detail: e.to_string() })?;
        devices.sort_by_key(|d| (d.is_keyboard_or_mouse(), d.vendor_id, d.product_id, d.interface_number));
        Ok(devices)
    })
    .await
}

/// Uses this receiver from now on (also after restarts), or with none the
/// first one a built-in protocol knows, whenever one is plugged in. The
/// receiver in use is opened anew, as a replug would.
#[tauri::command]
pub async fn receiver_select(app: AppHandle, device: Option<DeviceInfo>) -> CmdResult<()> {
    off_ui(move || {
        let hw = app.state::<Hardware>();
        let selector = device.as_ref().map(DeviceSelector::for_device);
        // Selected in the change too, so the receiver used is the one stored.
        let change = hw.change_settings();
        change.store(ReceiverSettings { receiver: selector.clone(), ..change.settings() })?;
        hw.handle.as_ref().ok_or(AppError::UsbUnavailable)?.select(choice(selector));
        Ok(())
    })
    .await
}

/// Async, as it takes the show state, which a journal write can hold (`off_ui`).
#[tauri::command]
pub async fn receiver_learn_start(app: AppHandle, buzzer: BuzzerId, button: Button) -> CmdResult<()> {
    off_ui(move || {
        let hw = app.state::<Hardware>();
        match hw.handle.as_ref().map(ReceiverHandle::status) {
            Some(ReceiverStatus::Connected { learnable: true, .. }) => {}
            Some(ReceiverStatus::Connected { .. }) => return Err(AppError::ReceiverNotLearnable),
            _ => return Err(AppError::ReceiverNotConnected),
        }
        // Held until learn mode is on, so no show starts in between; the
        // show's lock comes before the learn lock (`lock_show`).
        let show = crate::lock_show(&app);
        start_learning(&mut hw.learn.lock().unwrap(), show.game.is_some(), buzzer, button)?;
        drop(show);
        let _ = app.emit("receiver-learn", hw.learn_progress());
        Ok(())
    })
    .await
}

#[tauri::command]
pub fn receiver_learn_cancel(app: AppHandle, hw: tauri::State<Hardware>) {
    hw.end_learning(&app);
}

/// Saves the recorded presses as the signature of the button being learned,
/// and returns that button.
#[tauri::command]
pub async fn receiver_learn_finish(app: AppHandle) -> CmdResult<Press> {
    off_ui(move || {
        let hw = app.state::<Hardware>();
        let device = hw.connected_device().ok_or(AppError::ReceiverNotConnected)?;
        let recorder = hw.learn.lock().unwrap().clone().ok_or(AppError::LearnNotStarted)?;
        let change = hw.change_settings();
        let mut settings = change.settings();
        // Too few presses or a clash with another button: learn mode stays on to fix it.
        recorder.save_into(&mut settings.learned, &device)?;
        let stored = change.store(settings);
        // Stored or not, learn mode ends, so presses play again.
        hw.end_learning(&app);
        stored?;
        if let Some(handle) = hw.handle.as_ref() {
            handle.reload_protocol();
        }
        Ok(Press::new(recorder.buzzer, recorder.button))
    })
    .await
}

#[tauri::command]
pub async fn receiver_forget_learned(app: AppHandle) -> CmdResult<()> {
    off_ui(move || {
        let hw = app.state::<Hardware>();
        let change = hw.change_settings();
        change.store(ReceiverSettings { learned: Vec::new(), ..change.settings() })?;
        if let Some(handle) = hw.handle.as_ref() {
            handle.reload_protocol();
        }
        Ok(())
    })
    .await
}

/// Writes the recent raw reports to a text file the quizmaster picks, e.g.
/// to attach to a bug report or to work out the receiver's protocol; false
/// when they cancel.
#[tauri::command]
pub async fn receiver_export_log(app: AppHandle, window: Window, picker: Picker) -> CmdResult<bool> {
    let Some(path) = dialogs::pick_save(window, picker, "txt", None).await? else { return Ok(false) };
    let version = app.package_info().version.to_string();
    off_ui(move || write_log(&app.state::<Hardware>(), &version, &path).map(|()| true)).await
}

/// Writes the recent raw reports to `path`, under a header naming the app's `version`.
fn write_log(hw: &Hardware, version: &str, path: &Path) -> CmdResult<()> {
    let status = hw.handle.as_ref().map(ReceiverHandle::status);
    let mut out = format!(
        "# Receiver log, app {} on {}\n# status: {}\n# unix-ms\tbytes (hex)\tdecoded (buzzer, button, @ the buzzer's timer)\tdropped as not ready\n",
        version,
        std::env::consts::OS,
        serde_json::to_string(&status).unwrap_or_default()
    );
    let presses = |presses: &[Press]| -> String {
        let presses: Vec<String> =
            presses.iter().map(|p| format!("{}{:?}{}", p.buzzer, p.button, p.timer.map(|t| format!("@{t}")).unwrap_or_default())).collect();
        presses.join(",")
    };
    for r in hw.raw_log.lock().unwrap().iter() {
        out.push_str(&format!("{}\t{}\t{}\t{}\n", r.at_ms, format_hex(&r.bytes), presses(&r.decoded), presses(&r.dropped)));
    }
    std::fs::write(path, out).map_err(|e| AppError::file(FileAction::Write, path, e))
}

/// Off the windows' thread: it may list the devices (`receiver_devices`).
#[tauri::command]
pub async fn receiver_udev_rule(app: AppHandle) -> CmdResult<String> {
    off_ui(move || {
        let hw = app.state::<Hardware>();
        // The rule matters most when opening fails, so fall back to the chosen
        // receiver, or with none to the known one plugged in.
        let ids = hw
            .connected_device()
            .map(|d| (d.vendor_id, d.product_id))
            .or_else(|| hw.settings.lock().unwrap().receiver.as_ref().map(|s| (s.vendor_id, s.product_id)))
            .or_else(|| hw.backend.as_ref()?.list().ok()?.into_iter().find(protocol::is_known).map(|d| (d.vendor_id, d.product_id)));
        Ok(udev_rule(ids.map(|i| i.0), ids.map(|i| i.1)))
    })
    .await
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
        Hardware::without_usb(settings_file, Arc::new(Mutex::new(settings)), None)
    }

    fn stored(hw: &Hardware, settings: ReceiverSettings) -> CmdResult<()> {
        let change = hw.change_settings();
        change.store(settings)
    }

    fn receiver() -> Option<DeviceSelector> {
        Some(DeviceSelector { vendor_id: 1, product_id: 2, usage_page: None, interface_number: None })
    }

    #[test]
    fn every_buzzer_heard_shows_with_its_last_press() {
        let hw = hardware(std::env::temp_dir().join("receiver-heard").join(SETTINGS_FILE));
        let report = |at_ms, decoded, dropped| RawReport { at_ms, bytes: Vec::new(), decoded, dropped };
        let seen = |buzzer, button, at_ms| BuzzerSeen { buzzer, button, at_ms };
        let heard = hw.hear(&report(100, vec![Press::new(5, Button::A), Press::new(3, Button::Big), Press::new(5, Button::B)], vec![Press::timed(7, Button::C, 40)]));
        assert_eq!(heard, [seen(3, Button::Big, 100), seen(5, Button::B, 100), seen(7, Button::C, 100)], "a press not counted shows too");
        assert!(hw.hear(&report(150, Vec::new(), Vec::new())).is_empty());
        assert_eq!(hw.hear(&report(200, vec![Press::new(3, Button::D)], Vec::new())), [seen(3, Button::D, 200)]);
        let all: Vec<_> = hw.seen.lock().unwrap().values().copied().collect();
        assert_eq!(all, [seen(3, Button::D, 200), seen(5, Button::B, 100), seen(7, Button::C, 100)]);
    }

    #[test]
    fn settings_are_used_only_once_they_are_on_disk() {
        let dir = std::env::temp_dir().join(format!("receiver-settings-{}", std::process::id()));
        // A folder where the file should be: writing it fails.
        let blocked = hardware(dir.join("blocked"));
        std::fs::create_dir_all(dir.join("blocked")).unwrap();
        assert!(stored(&blocked, ReceiverSettings { receiver: receiver(), ..Default::default() }).is_err());
        assert!(blocked.change_settings().settings().receiver.is_none());

        let fine = hardware(dir.join(SETTINGS_FILE));
        stored(&fine, ReceiverSettings { receiver: receiver(), ..Default::default() }).unwrap();
        assert_eq!(fine.change_settings().settings().receiver, receiver());
        assert_eq!(hardware(dir.join(SETTINGS_FILE)).change_settings().settings().receiver, receiver());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn settings_changed_at_once_keep_every_change() {
        let dir = std::env::temp_dir().join(format!("receiver-settings-at-once-{}", std::process::id()));
        let hw = hardware(dir.join(SETTINGS_FILE));
        std::thread::scope(|scope| {
            for vendor_id in 0..8 {
                let hw = &hw;
                scope.spawn(move || {
                    let change = hw.change_settings();
                    let mut settings = change.settings();
                    std::thread::yield_now();
                    settings.learned.push(LearnedMapping { vendor_id, product_id: 1, signatures: Vec::new() });
                    change.store(settings).unwrap();
                });
            }
        });
        let mut kept: Vec<u16> = hardware(dir.join(SETTINGS_FILE)).change_settings().settings().learned.iter().map(|m| m.vendor_id).collect();
        kept.sort_unstable();
        assert_eq!(kept, (0..8).collect::<Vec<_>>());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
