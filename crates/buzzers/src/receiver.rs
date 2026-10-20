//! One USB-HID receiver, driven from a dedicated I/O thread.
//!
//! The thread owns the open device (hidapi handles are not shared across
//! threads), reads input reports with a short timeout, decodes them in arrival
//! order and sends presses to the app. The buzzer mode and lights reach it
//! over a channel; it keeps them, and writes what changed between reads, one
//! report at a time with a gap, so a slow receiver is not flooded. When the
//! cable is unplugged the read fails, the device is dropped and the thread
//! retries until it is back, then sets the receiver up again with the mode
//! and lights the buzzers should show.

use crate::clock::ShowClock;
use crate::edge::EdgeFilter;
use crate::hid::{DeviceInfo, DeviceSelector, HidBackend, HidDevice, Report};
use crate::protocol::{Protocol, ProtocolKind, ProtocolResolver};
use crate::{BuzzerDriver, DriverError, Press};
use quiz_core::{BuzzerId, BuzzerMode, Light};
use serde::Serialize;
use ts_rs::TS;
use std::collections::{BTreeMap, VecDeque};
use std::sync::mpsc::{self, Receiver as ChannelRx, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

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
    Connected { device: DeviceInfo, protocol: ProtocolKind, has_lights: bool, learnable: bool },
    /// The receiver cannot be used now; the thread keeps retrying. `detail`
    /// is the system's own message.
    Error { problem: ReceiverProblem, detail: String },
}

/// Why the receiver cannot be used, for the Director (which words it and,
/// for `CannotOpen`, says what to check on the platform).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ReceiverProblem {
    CannotList,
    CannotOpen,
    ConnectionLost,
    WriteFailed,
}

/// One input report as received, with what the protocol made of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RawReport {
    /// When it arrived, on the show clock (`ReceiverConfig::clock`), as
    /// the presses decoded from it.
    pub at_ms: u64,
    pub bytes: Vec<u8>,
    pub decoded: Vec<Press>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiverEvent {
    Status(ReceiverStatus),
    /// Sent for every report before the presses decoded from it.
    Raw(RawReport),
    /// A press edge, in the order the receiver delivered it, with the time
    /// its report arrived on the show clock (`ReceiverConfig::clock`). The
    /// game scores the press by that time, however long the app takes to
    /// apply it.
    Press { press: Press, at_ms: u64 },
}

#[derive(Debug, Clone)]
pub struct ReceiverConfig {
    /// Wait between attempts to find or reopen the receiver.
    pub retry: Duration,
    /// Read timeout; also the longest a light command waits to be written.
    pub read_timeout_ms: i32,
    /// See [`EdgeFilter`].
    pub repeat_gap_ms: u64,
    /// What reports are stamped with: the app's show clock.
    pub clock: ShowClock,
    /// Least time between two reports written to the receiver.
    pub write_gap: Duration,
    /// Longest the mode and lights wait for a receiver that is not ready
    /// after being set up (`Protocol::ready`).
    pub ready_timeout: Duration,
}

impl ReceiverConfig {
    /// The usual settings, stamping reports with the app's show `clock`:
    /// there is no default clock, so presses and commands share one. The
    /// write gap and the wait for a ready receiver are those of the Enjoy
    /// SDK (`rf215`).
    pub fn new(clock: ShowClock) -> ReceiverConfig {
        ReceiverConfig {
            retry: Duration::from_secs(2),
            read_timeout_ms: 20,
            repeat_gap_ms: 150,
            clock,
            write_gap: Duration::from_millis(10),
            ready_timeout: Duration::from_millis(500),
        }
    }
}

enum Control {
    Select(Option<DeviceSelector>),
    Lights(Vec<(BuzzerId, Light)>),
    Mode(BuzzerMode),
    Ignored(Press),
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

    /// Sets these buzzers' lights, now or once a receiver connects.
    pub fn set_lights(&self, lights: Vec<(BuzzerId, Light)>) {
        let _ = self.tx.send(Control::Lights(lights));
    }

    /// Puts the buzzers in `mode`, now or once a receiver connects.
    pub fn set_mode(&self, mode: BuzzerMode) {
        let _ = self.tx.send(Control::Mode(mode));
    }

    /// Tells the receiver the app did not take `press` (`Protocol::rearm`).
    pub fn ignored(&self, press: Press) {
        let _ = self.tx.send(Control::Ignored(press));
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

/// `BuzzerDriver` for the game: the mode and lights go to the receiver
/// thread, which writes them when a receiver is connected and again after
/// it reconnects.
pub struct HardwareDriver(pub ReceiverHandle);

impl BuzzerDriver for HardwareDriver {
    fn name(&self) -> &'static str {
        "hid receiver"
    }

    fn has_lights(&self) -> bool {
        matches!(self.0.status(), ReceiverStatus::Connected { has_lights: true, .. })
    }

    fn set_lights(&mut self, lights: &[(BuzzerId, Light)]) -> Result<(), DriverError> {
        self.0.set_lights(lights.to_vec());
        Ok(())
    }

    fn set_mode(&mut self, mode: BuzzerMode) {
        self.0.set_mode(mode);
    }
}

/// Starts the receiver thread. Its events go to `events`, as `E`: the app
/// sends them into the queue of its show thread with everything else.
pub fn spawn<E: From<ReceiverEvent> + Send + 'static>(
    backend: Arc<dyn HidBackend>,
    resolver: Arc<ProtocolResolver>,
    events: Sender<E>,
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
        mode: BuzzerMode::Off,
        lights: BTreeMap::new(),
        outbox: VecDeque::new(),
        next_write: Instant::now(),
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
    /// The protocol's kind in the status last sent.
    kind: ProtocolKind,
    /// Until when writes wait for the receiver to be ready.
    ready_by: Instant,
}

impl Connection {
    fn new(info: DeviceInfo, device: Box<dyn HidDevice>, protocol: Box<dyn Protocol>) -> Connection {
        let kind = protocol.kind();
        Connection { info, device, protocol, kind, ready_by: Instant::now() }
    }

    /// Whether writes wait for the receiver at `now`.
    fn held(&self, now: Instant) -> bool {
        now < self.ready_by && !self.protocol.ready()
    }
}

struct Worker<E> {
    backend: Arc<dyn HidBackend>,
    resolver: Arc<ProtocolResolver>,
    events: Sender<E>,
    config: ReceiverConfig,
    status: Arc<Mutex<ReceiverStatus>>,
    selector: Option<DeviceSelector>,
    conn: Option<Connection>,
    next_try: Instant,
    edge: EdgeFilter,
    /// The mode and lights the buzzers should show, as the app last set them.
    mode: BuzzerMode,
    lights: BTreeMap<BuzzerId, Light>,
    /// Reports waiting to be written, in order.
    outbox: VecDeque<Report>,
    next_write: Instant,
}

impl<E: From<ReceiverEvent>> Worker<E> {
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
            self.write_due();
            let timeout_ms = self.read_timeout_ms();
            if let Some(conn) = self.conn.as_mut() {
                match conn.device.read_timeout(&mut buf, timeout_ms) {
                    Ok(0) => {}
                    Ok(n) => self.report(&buf[..n]),
                    Err(e) => self.lost(ReceiverProblem::ConnectionLost, &e),
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
                self.outbox.clear();
                self.edge.reset();
                self.selector = selector.clone();
                self.next_try = Instant::now();
                self.set_status(match selector {
                    Some(selector) => ReceiverStatus::Searching { selector },
                    None => ReceiverStatus::Off,
                });
            }
            Control::Lights(lights) => {
                let changed: Vec<_> = lights.into_iter().filter(|&(buzzer, light)| self.lights.insert(buzzer, light) != Some(light)).collect();
                if let Some(conn) = self.conn.as_mut() {
                    let reports = conn.protocol.encode_lights(&changed);
                    self.queue(reports);
                }
            }
            Control::Mode(mode) => {
                if mode == self.mode {
                    return;
                }
                self.mode = mode;
                if let Some(conn) = self.conn.as_mut() {
                    let reports = conn.protocol.encode_mode(mode);
                    self.queue(reports);
                }
            }
            Control::Ignored(press) => {
                if let Some(conn) = self.conn.as_mut() {
                    let reports = conn.protocol.rearm(press);
                    self.queue(reports);
                }
            }
            Control::ReloadProtocol => {
                if let Some(conn) = self.conn.take() {
                    let protocol = (self.resolver)(&conn.info);
                    self.set_up(Connection::new(conn.info, conn.device, protocol));
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
            Err(e) => return self.set_status(error_status(ReceiverProblem::CannotList, &e)),
        };
        let Some(info) = devices.into_iter().find(|d| selector.matches(d)) else {
            return self.set_status(ReceiverStatus::Searching { selector });
        };
        let device = match self.backend.open(&info.path) {
            Ok(d) => d,
            Err(e) => return self.set_status(error_status(ReceiverProblem::CannotOpen, &e)),
        };
        let protocol = (self.resolver)(&info);
        self.set_up(Connection::new(info, device, protocol));
    }

    /// Sets up the receiver on `conn` with its protocol's init reports,
    /// written one after another with the write gap before the connection
    /// is announced, so nothing is sent to an unready receiver. Then the
    /// lights and mode the buzzers should show wait to be written until
    /// the receiver is ready, in the order the app sends a change in.
    fn set_up(&mut self, mut conn: Connection) {
        self.outbox.clear();
        self.edge.reset();
        for (i, report) in conn.protocol.init_reports().iter().enumerate() {
            if i > 0 {
                std::thread::sleep(self.config.write_gap);
            }
            if let Err(e) = report.send(conn.device.as_mut()) {
                return self.lost(ReceiverProblem::WriteFailed, &e);
            }
        }
        let now = Instant::now();
        conn.ready_by = now + self.config.ready_timeout;
        self.next_write = now + self.config.write_gap;
        let lights: Vec<_> = self.lights.iter().map(|(&buzzer, &light)| (buzzer, light)).collect();
        let mut shown = conn.protocol.encode_lights(&lights);
        shown.extend(conn.protocol.encode_mode(self.mode));
        let status = connected_status(&conn);
        self.conn = Some(conn);
        self.queue(shown);
        self.set_status(status);
    }

    fn report(&mut self, bytes: &[u8]) {
        let Some(conn) = self.conn.as_mut() else { return };
        let at_ms = self.config.clock.now_ms();
        let decoded = conn.protocol.decode(bytes);
        // The report may have changed what the protocol knows of the receiver.
        let kind = conn.protocol.kind();
        let status = (kind != conn.kind).then(|| {
            conn.kind = kind;
            connected_status(conn)
        });
        let presses = self.edge.filter(decoded.clone(), at_ms);
        if let Some(status) = status {
            self.set_status(status);
        }
        let _ = self.events.send(ReceiverEvent::Raw(RawReport { at_ms, bytes: bytes.to_vec(), decoded }).into());
        for press in presses {
            let _ = self.events.send(ReceiverEvent::Press { press, at_ms }.into());
        }
    }

    /// Adds `reports` to those waiting to be written, but for one that
    /// repeats the report before it.
    fn queue(&mut self, reports: Vec<Report>) {
        for report in reports {
            if self.outbox.back() != Some(&report) {
                self.outbox.push_back(report);
            }
        }
    }

    /// Writes the next waiting report, once its time has come and the
    /// receiver is ready.
    fn write_due(&mut self) {
        let now = Instant::now();
        let Some(conn) = self.conn.as_mut() else { return };
        if now < self.next_write || conn.held(now) {
            return;
        }
        let Some(report) = self.outbox.pop_front() else { return };
        match report.send(conn.device.as_mut()) {
            Ok(()) => self.next_write = now + self.config.write_gap,
            Err(e) => self.lost(ReceiverProblem::WriteFailed, &e),
        }
    }

    /// How long the next read may wait: no longer than the read timeout,
    /// nor than until the next waiting report is due.
    fn read_timeout_ms(&self) -> i32 {
        let longest = self.config.read_timeout_ms;
        let Some(conn) = &self.conn else { return longest };
        if self.outbox.is_empty() {
            return longest;
        }
        let now = Instant::now();
        let due = if conn.held(now) { conn.ready_by } else { self.next_write };
        let wait_ms = due.saturating_duration_since(now).as_micros().div_ceil(1_000);
        i32::try_from(wait_ms).map_or(longest, |ms| ms.min(longest))
    }

    fn lost(&mut self, problem: ReceiverProblem, error: &DriverError) {
        self.conn = None;
        self.outbox.clear();
        self.edge.reset();
        self.next_try = Instant::now() + self.config.retry;
        self.set_status(error_status(problem, error));
    }

    fn set_status(&mut self, status: ReceiverStatus) {
        let mut current = self.status.lock().unwrap();
        if *current != status {
            *current = status.clone();
            let _ = self.events.send(ReceiverEvent::Status(status).into());
        }
    }
}

fn connected_status(conn: &Connection) -> ReceiverStatus {
    ReceiverStatus::Connected {
        device: conn.info.clone(),
        protocol: conn.kind.clone(),
        has_lights: conn.protocol.has_lights(),
        learnable: conn.protocol.learnable(),
    }
}

fn error_status(problem: ReceiverProblem, error: &DriverError) -> ReceiverStatus {
    ReceiverStatus::Error { problem, detail: error.to_string() }
}
