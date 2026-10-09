//! The buzzer receiver inside the app: remembered settings, the receiver
//! thread, learn mode and the commands behind the Director's Receiver tab.
//!
//! Hardware presses take the same path as keyboard and on-screen presses
//! (`Command::Press` through `execute`), so they are journaled and replayed
//! like everything else.

use buzzers::hid::{udev_rule, DeviceInfo, DeviceSelector, HidBackend};
use buzzers::hidapi_backend::HidapiBackend;
use buzzers::learned::{LearnedMapping, LearnedProtocol, Recorder};
use buzzers::protocol::{format_hex, Protocol, RawOnly};
use buzzers::receiver::{self, RawReport, ReceiverConfig, ReceiverEvent, ReceiverHandle, ReceiverStatus};
use buzzers::Press;
use quiz_core::{BuzzerId, Button, Command};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
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
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ReceiverInfo {
    /// None when USB-HID could not be initialised on this system.
    status: Option<ReceiverStatus>,
    init_error: Option<String>,
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
    settings_path: PathBuf,
    learn: Mutex<Option<Recorder>>,
    raw_log: Mutex<VecDeque<RawReport>>,
}

type CmdResult<T> = Result<T, String>;

impl Hardware {
    /// Starts the receiver thread and reconnects to the remembered receiver.
    pub fn start(app: &AppHandle) -> Hardware {
        let settings_path = app.path().app_data_dir().map(|d| d.join(SETTINGS_FILE)).unwrap_or_else(|_| SETTINGS_FILE.into());
        let settings: ReceiverSettings = std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let remembered = settings.receiver.clone();
        let settings = Arc::new(Mutex::new(settings));

        let backend = match HidapiBackend::new() {
            Ok(b) => Arc::new(b),
            Err(e) => {
                return Hardware {
                    backend: None,
                    handle: None,
                    init_error: Some(format!("USB-HID is not available: {e}")),
                    settings,
                    settings_path,
                    learn: Mutex::new(None),
                    raw_log: Mutex::new(VecDeque::new()),
                }
            }
        };

        let resolver_settings = settings.clone();
        let resolver = Arc::new(move |device: &DeviceInfo| -> Box<dyn Protocol> {
            // Vendor protocols go first here once the receiver protocol is documented.
            let settings = resolver_settings.lock().unwrap();
            match settings.learned.iter().find(|m| m.for_device(device) && !m.signatures.is_empty()) {
                Some(mapping) => Box::new(LearnedProtocol::new(mapping.clone())),
                None => Box::new(RawOnly),
            }
        });
        let (tx, rx) = mpsc::channel();
        let (handle, _join) = receiver::spawn(backend.clone(), resolver, tx, ReceiverConfig::default());
        handle.select(remembered);
        spawn_event_pump(app.clone(), rx);

        Hardware {
            backend: Some(backend),
            handle: Some(handle),
            init_error: None,
            settings,
            settings_path,
            learn: Mutex::new(None),
            raw_log: Mutex::new(VecDeque::new()),
        }
    }

    pub fn handle(&self) -> Option<ReceiverHandle> {
        self.handle.clone()
    }

    fn save_settings(&self) -> CmdResult<()> {
        let json = serde_json::to_string_pretty(&*self.settings.lock().unwrap()).map_err(|e| e.to_string())?;
        if let Some(dir) = self.settings_path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = self.settings_path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.settings_path).map_err(|e| e.to_string())
    }

    fn learn_progress(&self) -> LearnProgress {
        match self.learn.lock().unwrap().as_ref() {
            Some(r) => LearnProgress { active: true, buzzer: Some(r.buzzer), button: Some(r.button), samples: r.sample_count() },
            None => LearnProgress { active: false, buzzer: None, button: None, samples: 0 },
        }
    }

    fn connected_device(&self) -> Option<DeviceInfo> {
        match self.handle.as_ref()?.status() {
            ReceiverStatus::Connected { device, .. } => Some(device),
            _ => None,
        }
    }
}

/// Moves receiver events into the game and out to the windows.
fn spawn_event_pump(app: AppHandle, rx: mpsc::Receiver<ReceiverEvent>) {
    std::thread::Builder::new()
        .name("buzzer-events".into())
        .spawn(move || {
            for event in rx {
                let hw = app.state::<Hardware>();
                match event {
                    ReceiverEvent::Status(status) => {
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
                        let used = !learning && crate::press_from_hardware(&app, command, at_ms);
                        let _ = app.emit("receiver-press", PressEvent { press, used });
                    }
                }
            }
        })
        .expect("spawn buzzer event thread");
}

#[tauri::command]
pub fn receiver_info(hw: tauri::State<Hardware>) -> ReceiverInfo {
    let log = hw.raw_log.lock().unwrap();
    let skip = log.len().saturating_sub(RAW_LOG_SNAPSHOT);
    ReceiverInfo {
        status: hw.handle.as_ref().map(ReceiverHandle::status),
        init_error: hw.init_error.clone(),
        learned_buttons: hw.settings.lock().unwrap().learned.iter().map(|m| m.signatures.len()).sum(),
        learn: hw.learn_progress(),
        recent: log.iter().skip(skip).cloned().collect(),
        platform: std::env::consts::OS,
    }
}

#[tauri::command]
pub fn receiver_devices(hw: tauri::State<Hardware>) -> CmdResult<Vec<DeviceInfo>> {
    let backend = hw.backend.as_ref().ok_or("USB-HID is not available")?;
    let mut devices = backend.list().map_err(|e| e.to_string())?;
    devices.sort_by_key(|d| (d.is_keyboard_or_mouse(), d.vendor_id, d.product_id, d.interface_number));
    Ok(devices)
}

/// Uses this receiver from now on (also after restarts), or none.
#[tauri::command]
pub fn receiver_select(hw: tauri::State<Hardware>, device: Option<DeviceInfo>) -> CmdResult<()> {
    let selector = device.as_ref().map(DeviceSelector::for_device);
    hw.settings.lock().unwrap().receiver = selector.clone();
    hw.save_settings()?;
    hw.handle.as_ref().ok_or("USB-HID is not available")?.select(selector);
    Ok(())
}

#[tauri::command]
pub fn receiver_learn_start(app: AppHandle, hw: tauri::State<Hardware>, buzzer: BuzzerId, button: Button) -> CmdResult<()> {
    if hw.connected_device().is_none() {
        return Err("connect the receiver first".into());
    }
    *hw.learn.lock().unwrap() = Some(Recorder::new(buzzer, button));
    let _ = app.emit("receiver-learn", hw.learn_progress());
    Ok(())
}

#[tauri::command]
pub fn receiver_learn_cancel(app: AppHandle, hw: tauri::State<Hardware>) {
    *hw.learn.lock().unwrap() = None;
    let _ = app.emit("receiver-learn", hw.learn_progress());
}

/// Saves the recorded presses as the signature of the button being learned.
#[tauri::command]
pub fn receiver_learn_finish(app: AppHandle, hw: tauri::State<Hardware>) -> CmdResult<String> {
    let device = hw.connected_device().ok_or("the receiver is not connected")?;
    let recorder = hw.learn.lock().unwrap().clone().ok_or("learning was not started")?;
    if recorder.sample_count() < 2 {
        return Err("press the button at least twice (three times is best)".into());
    }
    {
        let mut settings = hw.settings.lock().unwrap();
        let index = match settings.learned.iter().position(|m| m.for_device(&device)) {
            Some(i) => i,
            None => {
                settings.learned.push(LearnedMapping { vendor_id: device.vendor_id, product_id: device.product_id, signatures: vec![] });
                settings.learned.len() - 1
            }
        };
        let outcome = recorder.finish(&settings.learned[index].signatures)?;
        if let Some(c) = outcome.conflicts.first() {
            return Err(format!(
                "these presses look the same as buzzer {} button {:?}; try again or re-learn that button",
                c.buzzer, c.button
            ));
        }
        settings.learned[index].upsert(outcome.signature);
    }
    hw.save_settings()?;
    *hw.learn.lock().unwrap() = None;
    if let Some(handle) = hw.handle.as_ref() {
        handle.reload_protocol();
    }
    let _ = app.emit("receiver-learn", hw.learn_progress());
    Ok(format!("Learned buzzer {} button {:?}.", recorder.buzzer, recorder.button))
}

#[tauri::command]
pub fn receiver_forget_learned(hw: tauri::State<Hardware>) -> CmdResult<()> {
    hw.settings.lock().unwrap().learned.clear();
    hw.save_settings()?;
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
    std::fs::write(path, out).map_err(|e| e.to_string())
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
