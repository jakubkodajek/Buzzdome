//! One USB-HID receiver, driven from a dedicated I/O thread.
//!
//! The thread owns the open device (hidapi handles are not shared across
//! threads), reads input reports with a short timeout, decodes them in arrival
//! order and sends presses to the app. Light commands reach it over a channel
//! and are written between reads. When the cable is unplugged the read fails,
//! the device is dropped and the thread retries until it is back.

use crate::edge::EdgeFilter;
use crate::hid::{DeviceInfo, DeviceSelector, HidBackend, HidDevice};
use crate::protocol::{Protocol, ProtocolResolver};
use crate::{BuzzerDriver, DriverError, Press};
use quiz_core::{BuzzerId, Light};
use serde::Serialize;
use ts_rs::TS;
use std::sync::mpsc::{self, Receiver as ChannelRx, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReceiverStatus {
    /// No receiver chosen.
    Off,
    /// Waiting for the chosen receiver to be plugged in.
    #[serde(rename_all = "camelCase")]
    Searching { selector: DeviceSelector },
    #[serde(rename_all = "camelCase")]
    Connected { device: DeviceInfo, protocol: String, has_lights: bool },
    #[serde(rename_all = "camelCase")]
    Error { message: String },
}

/// One input report as received, with what the protocol made of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RawReport {
    /// Wall-clock time, ms since the Unix epoch.
    pub at_ms: u64,
    pub bytes: Vec<u8>,
    pub decoded: Vec<Press>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiverEvent {
    Status(ReceiverStatus),
    /// Sent for every report before the presses decoded from it.
    Raw(RawReport),
    /// A press edge, in the order the receiver delivered it.
    Press(Press),
}

#[derive(Debug, Clone)]
pub struct ReceiverConfig {
    /// Wait between attempts to find or reopen the receiver.
    pub retry: Duration,
    /// Read timeout; also the longest a light command waits to be written.
    pub read_timeout_ms: i32,
    /// See [`EdgeFilter`].
    pub repeat_gap_ms: u64,
}

impl Default for ReceiverConfig {
    fn default() -> Self {
        ReceiverConfig { retry: Duration::from_secs(2), read_timeout_ms: 20, repeat_gap_ms: 150 }
    }
}

enum Control {
    Select(Option<DeviceSelector>),
    Lights(Vec<(BuzzerId, Light)>),
    ReloadProtocol,
    Shutdown,
}

/// Cheap, cloneable handle to the receiver thread.
#[derive(Clone)]
pub struct ReceiverHandle {
    tx: Sender<Control>,
    status: Arc<Mutex<ReceiverStatus>>,
}

impl ReceiverHandle {
    /// Chooses the receiver to use (and remember), or `None` to close it.
    pub fn select(&self, selector: Option<DeviceSelector>) {
        let _ = self.tx.send(Control::Select(selector));
    }

    pub fn set_lights(&self, lights: Vec<(BuzzerId, Light)>) {
        let _ = self.tx.send(Control::Lights(lights));
    }

    /// Re-resolves the protocol of the open device, e.g. after learning buttons.
    pub fn reload_protocol(&self) {
        let _ = self.tx.send(Control::ReloadProtocol);
    }

    pub fn status(&self) -> ReceiverStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(Control::Shutdown);
    }
}

/// `BuzzerDriver` for the game: lights go to the receiver when one is connected.
pub struct HardwareDriver(pub ReceiverHandle);

impl BuzzerDriver for HardwareDriver {
    fn name(&self) -> &'static str {
        "hid receiver"
    }

    fn has_lights(&self) -> bool {
        matches!(self.0.status(), ReceiverStatus::Connected { has_lights: true, .. })
    }

    fn set_lights(&mut self, lights: &[(BuzzerId, Light)]) -> Result<(), DriverError> {
        if self.has_lights() {
            self.0.set_lights(lights.to_vec());
        }
        Ok(())
    }
}

pub fn spawn(
    backend: Arc<dyn HidBackend>,
    resolver: Arc<ProtocolResolver>,
    events: Sender<ReceiverEvent>,
    config: ReceiverConfig,
) -> (ReceiverHandle, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let status = Arc::new(Mutex::new(ReceiverStatus::Off));
    let handle = ReceiverHandle { tx, status: status.clone() };
    let worker = Worker {
        backend,
        resolver,
        events,
        config,
        status,
        selector: None,
        conn: None,
        next_try: Instant::now(),
        edge: EdgeFilter::default(),
    };
    let join = std::thread::Builder::new()
        .name("buzzer-receiver".into())
        .spawn(move || worker.run(rx))
        .expect("spawn receiver thread");
    (handle, join)
}

struct Connection {
    info: DeviceInfo,
    device: Box<dyn HidDevice>,
    protocol: Box<dyn Protocol>,
}

struct Worker {
    backend: Arc<dyn HidBackend>,
    resolver: Arc<ProtocolResolver>,
    events: Sender<ReceiverEvent>,
    config: ReceiverConfig,
    status: Arc<Mutex<ReceiverStatus>>,
    selector: Option<DeviceSelector>,
    conn: Option<Connection>,
    next_try: Instant,
    edge: EdgeFilter,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

impl Worker {
    fn run(mut self, rx: ChannelRx<Control>) {
        let mut buf = [0u8; 256];
        loop {
            loop {
                match rx.try_recv() {
                    Ok(Control::Shutdown) | Err(TryRecvError::Disconnected) => return,
                    Ok(c) => self.control(c),
                    Err(TryRecvError::Empty) => break,
                }
            }
            if let Some(conn) = self.conn.as_mut() {
                match conn.device.read_timeout(&mut buf, self.config.read_timeout_ms) {
                    Ok(0) => {}
                    Ok(n) => self.report(&buf[..n]),
                    Err(e) => self.lost(format!("Receiver connection lost ({e}). Reconnecting…")),
                }
                continue;
            }
            if self.selector.is_some() && Instant::now() >= self.next_try {
                self.try_connect();
                continue;
            }
            // Idle: wait for a control message or the next retry.
            let wait = self.next_try.saturating_duration_since(Instant::now()).clamp(Duration::from_millis(10), Duration::from_millis(200));
            match rx.recv_timeout(wait) {
                Ok(Control::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
                Ok(c) => self.control(c),
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }

    fn control(&mut self, c: Control) {
        match c {
            Control::Select(selector) => {
                self.conn = None;
                self.edge.reset();
                self.selector = selector.clone();
                self.next_try = Instant::now();
                self.set_status(match selector {
                    Some(selector) => ReceiverStatus::Searching { selector },
                    None => ReceiverStatus::Off,
                });
            }
            Control::Lights(lights) => {
                let reports = match &self.conn {
                    Some(conn) => conn.protocol.encode_lights(&lights),
                    None => return,
                };
                self.write_all(&reports);
            }
            Control::ReloadProtocol => {
                if let Some(conn) = self.conn.as_mut() {
                    conn.protocol = (self.resolver)(&conn.info);
                    let status = connected_status(conn);
                    self.set_status(status);
                }
            }
            Control::Shutdown => {}
        }
    }

    fn try_connect(&mut self) {
        let Some(selector) = self.selector.clone() else { return };
        self.next_try = Instant::now() + self.config.retry;
        let devices = match self.backend.list() {
            Ok(d) => d,
            Err(e) => return self.set_status(ReceiverStatus::Error { message: format!("Cannot list USB devices: {e}") }),
        };
        let Some(info) = devices.into_iter().find(|d| selector.matches(d)) else {
            return self.set_status(ReceiverStatus::Searching { selector });
        };
        let device = match self.backend.open(&info.path) {
            Ok(d) => d,
            Err(e) => return self.set_status(ReceiverStatus::Error { message: open_error_hint(&e) }),
        };
        let protocol = (self.resolver)(&info);
        let init = protocol.init_reports();
        let conn = Connection { info, device, protocol };
        let status = connected_status(&conn);
        self.conn = Some(conn);
        self.edge.reset();
        // Initialise before announcing the connection, so nothing is sent to an unready receiver.
        self.write_all(&init);
        if self.conn.is_some() {
            self.set_status(status);
        }
    }

    fn report(&mut self, bytes: &[u8]) {
        let Some(conn) = self.conn.as_mut() else { return };
        let at_ms = now_ms();
        let decoded = conn.protocol.decode(bytes);
        let presses = self.edge.filter(decoded.clone(), at_ms);
        let _ = self.events.send(ReceiverEvent::Raw(RawReport { at_ms, bytes: bytes.to_vec(), decoded }));
        for press in presses {
            let _ = self.events.send(ReceiverEvent::Press(press));
        }
    }

    fn write_all(&mut self, reports: &[Vec<u8>]) {
        for report in reports {
            let result = match self.conn.as_mut() {
                Some(conn) => conn.device.write(report),
                None => return,
            };
            if let Err(e) = result {
                return self.lost(format!("Writing to the receiver failed ({e}). Reconnecting…"));
            }
        }
    }

    fn lost(&mut self, message: String) {
        self.conn = None;
        self.edge.reset();
        self.next_try = Instant::now() + self.config.retry;
        self.set_status(ReceiverStatus::Error { message });
    }

    fn set_status(&mut self, status: ReceiverStatus) {
        let mut current = self.status.lock().unwrap();
        if *current != status {
            *current = status.clone();
            let _ = self.events.send(ReceiverEvent::Status(status));
        }
    }
}

fn connected_status(conn: &Connection) -> ReceiverStatus {
    ReceiverStatus::Connected {
        device: conn.info.clone(),
        protocol: conn.protocol.name(),
        has_lights: conn.protocol.has_lights(),
    }
}

fn open_error_hint(e: &DriverError) -> String {
    let hint = if cfg!(target_os = "linux") {
        "On Linux, install the udev rule shown on the Receiver tab so your user may open the device."
    } else if cfg!(target_os = "macos") {
        "On macOS, allow the app under System Settings > Privacy & Security > Input Monitoring if asked."
    } else {
        "Close other programs that may be using the receiver (for example the vendor's quiz software)."
    };
    format!("Cannot open the receiver: {e}. {hint}")
}
