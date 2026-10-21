//! Receiver thread against a fake HID backend: connect, decode, mode and
//! lights, fastest-finger order, unplug and replug.

use buzzers::clock::ShowClock;
use buzzers::cp2110;
use buzzers::hid::{DeviceInfo, DeviceSelector, HidBackend, HidDevice, Report};
use buzzers::protocol::{Protocol, ProtocolKind};
use buzzers::enjoy303s::Enjoy303s;
use buzzers::receiver::{spawn, ReceiverConfig, ReceiverEvent, ReceiverHandle, ReceiverProblem, ReceiverStatus};
use buzzers::{DriverError, Press};
use quiz_core::{BuzzerId, BuzzerMode, Button, Light};
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Wire {
    plugged: bool,
    fail_open: bool,
    inbox: VecDeque<Vec<u8>>,
    written: Vec<Vec<u8>>,
    /// When each output report was written.
    written_at: Vec<Instant>,
    features: Vec<Vec<u8>>,
    opens: usize,
    /// Makes the next read fail, as when the cable is pulled.
    break_link: bool,
}

#[derive(Clone, Default)]
struct FakeBackend(Arc<Mutex<Wire>>);

fn info() -> DeviceInfo {
    DeviceInfo {
        path: "fake0".into(),
        vendor_id: 0x1234,
        product_id: 0x5678,
        manufacturer: None,
        product: Some("Fake receiver".into()),
        serial_number: None,
        usage_page: 0xff00,
        usage: 1,
        interface_number: 0,
    }
}

impl HidBackend for FakeBackend {
    fn list(&self) -> Result<Vec<DeviceInfo>, DriverError> {
        Ok(if self.0.lock().unwrap().plugged { vec![info()] } else { vec![] })
    }

    fn open(&self, _path: &str) -> Result<Box<dyn HidDevice>, DriverError> {
        let mut w = self.0.lock().unwrap();
        if w.fail_open {
            return Err(DriverError::Device("access denied".into()));
        }
        w.opens += 1;
        Ok(Box::new(FakeDevice(self.0.clone())))
    }
}

struct FakeDevice(Arc<Mutex<Wire>>);

impl HidDevice for FakeDevice {
    fn read_timeout(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, DriverError> {
        let next = {
            let mut w = self.0.lock().unwrap();
            if w.break_link {
                w.break_link = false;
                return Err(DriverError::Device("device disconnected".into()));
            }
            w.inbox.pop_front()
        };
        match next {
            Some(r) => {
                buf[..r.len()].copy_from_slice(&r);
                Ok(r.len())
            }
            None => {
                std::thread::sleep(Duration::from_millis(timeout_ms as u64));
                Ok(0)
            }
        }
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, DriverError> {
        let mut w = self.0.lock().unwrap();
        w.written.push(data.to_vec());
        w.written_at.push(Instant::now());
        Ok(data.len())
    }

    fn send_feature_report(&mut self, data: &[u8]) -> Result<(), DriverError> {
        self.0.lock().unwrap().features.push(data.to_vec());
        Ok(())
    }
}

/// Test protocol: [0x01, buzzer, button 0..=4] is a press and [0x7e] the
/// receiver's answer, which calls for [0x06, 0x01]. Set up with feature
/// [0x30] and output [0x03, 0x01]; lights -> [0x02, buzzer, light], a
/// mode -> [0x04, mode].
struct TestProtocol {
    answered: bool,
    follow: Vec<Report>,
}

fn mode_byte(mode: BuzzerMode) -> u8 {
    match mode {
        BuzzerMode::Off => 0,
        BuzzerMode::Register => 1,
        BuzzerMode::Answers => 2,
        BuzzerMode::FastestFinger => 3,
        BuzzerMode::Buzzed => 5,
        BuzzerMode::Results => 6,
    }
}

impl Protocol for TestProtocol {
    fn kind(&self) -> ProtocolKind {
        ProtocolKind::Enjoy303s { answered: self.answered }
    }

    fn decode(&mut self, r: &[u8]) -> Vec<Press> {
        if r == [0x7e] {
            self.answered = true;
            self.follow.push(Report::Output(vec![0x06, 0x01]));
        }
        if r.len() < 3 || r[0] != 0x01 {
            return vec![];
        }
        let button = match r[2] {
            0 => Button::A,
            1 => Button::B,
            2 => Button::C,
            3 => Button::D,
            4 => Button::Big,
            _ => return vec![],
        };
        vec![Press::new(r[1] as BuzzerId, button)]
    }

    fn init_reports(&self) -> Vec<Report> {
        vec![Report::Feature(vec![0x30]), Report::Output(vec![0x03, 0x01])]
    }

    fn follow_up(&mut self) -> Vec<Report> {
        std::mem::take(&mut self.follow)
    }

    fn encode_lights(&mut self, lights: &[(BuzzerId, Light)]) -> Vec<Report> {
        lights.iter().map(|(b, l)| Report::Output(vec![0x02, *b as u8, *l as u8])).collect()
    }

    fn encode_mode(&mut self, mode: BuzzerMode) -> Vec<Report> {
        vec![Report::Output(vec![0x04, mode_byte(mode)])]
    }

    fn has_lights(&self) -> bool {
        true
    }
}

fn config() -> ReceiverConfig {
    ReceiverConfig {
        retry: Duration::from_millis(30),
        read_timeout_ms: 5,
        repeat_gap_ms: 150,
        write_gap: Duration::from_millis(1),
        ..ReceiverConfig::new(ShowClock::start())
    }
}

fn selector() -> DeviceSelector {
    DeviceSelector::for_device(&info())
}

fn wait_for(rx: &Receiver<ReceiverEvent>, pred: impl Fn(&ReceiverEvent) -> bool) -> ReceiverEvent {
    loop {
        let ev = rx.recv_timeout(Duration::from_secs(3)).expect("timed out waiting for a receiver event");
        if pred(&ev) {
            return ev;
        }
    }
}

fn is_connected(ev: &ReceiverEvent) -> bool {
    matches!(ev, ReceiverEvent::Status(ReceiverStatus::Connected { .. }))
}

fn start(backend: &FakeBackend) -> (ReceiverHandle, Receiver<ReceiverEvent>) {
    start_with(backend, config(), true)
}

/// Starts the receiver thread with `config`; its receiver has answered at
/// once when `answered`, else once it sends [0x7e].
fn start_with(backend: &FakeBackend, config: ReceiverConfig, answered: bool) -> (ReceiverHandle, Receiver<ReceiverEvent>) {
    let (tx, rx) = mpsc::channel();
    let resolver = Arc::new(move |_: &DeviceInfo| Box::new(TestProtocol { answered, follow: Vec::new() }) as Box<dyn Protocol>);
    let (handle, _join) = spawn(Arc::new(backend.clone()), resolver, tx, config);
    (handle, rx)
}

/// The output reports written once there are `count`, without the init report.
fn written_after_init(backend: &FakeBackend, count: usize) -> Vec<Vec<u8>> {
    for _ in 0..300 {
        let written = backend.0.lock().unwrap().written.clone();
        if written.len() > count {
            return written[1..].to_vec();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let written = backend.0.lock().unwrap().written.clone();
    panic!("expected {count} reports after the init report, got {written:?}");
}

#[test]
fn connects_sends_init_and_decodes_presses_in_order() {
    let backend = FakeBackend::default();
    backend.0.lock().unwrap().plugged = true;
    let (handle, rx) = start(&backend);
    handle.select(Some(selector()));
    wait_for(&rx, is_connected);
    assert_eq!(backend.0.lock().unwrap().features, vec![vec![0x30]]);
    // The mode the buzzers are in from the start.
    assert_eq!(written_after_init(&backend, 1), [vec![0x04, 0]]);

    {
        let mut w = backend.0.lock().unwrap();
        w.inbox.push_back(vec![0x01, 7, 4]);
        w.inbox.push_back(vec![0x01, 3, 4]);
        w.inbox.push_back(vec![0x01, 3, 4]); // repeat while held
        w.inbox.push_back(vec![0x09, 9, 9]); // unknown report
    }
    let mut presses = Vec::new();
    let mut raws = Vec::new();
    while raws.len() < 4 {
        match wait_for(&rx, |e| !matches!(e, ReceiverEvent::Status(_))) {
            ReceiverEvent::Raw(report) => raws.push(report),
            ReceiverEvent::Press { press, at_ms } => presses.push((press, at_ms)),
            ReceiverEvent::Status(_) => {}
        }
    }
    // Drain presses that follow the last raw report.
    while let Ok(ReceiverEvent::Press { press, at_ms }) = rx.recv_timeout(Duration::from_millis(50)) {
        presses.push((press, at_ms));
    }
    // Each press carries the arrival time of the report it came from.
    let expected = [(Press::new(7, Button::Big), raws[0].at_ms), (Press::new(3, Button::Big), raws[1].at_ms)];
    assert_eq!(presses, expected);
    handle.shutdown();
}

#[test]
fn writes_light_commands() {
    let backend = FakeBackend::default();
    backend.0.lock().unwrap().plugged = true;
    let (handle, rx) = start(&backend);
    handle.select(Some(selector()));
    wait_for(&rx, is_connected);
    handle.set_lights(vec![(4, Light::Ready), (5, Light::First)]);
    // Only what changed is written again.
    handle.set_lights(vec![(4, Light::Ready), (5, Light::Locked)]);
    handle.set_mode(BuzzerMode::Off);
    handle.set_mode(BuzzerMode::Answers);
    handle.set_mode(BuzzerMode::Answers);
    assert_eq!(
        written_after_init(&backend, 5),
        [
            vec![0x04, 0],
            vec![0x02, 4, Light::Ready as u8],
            vec![0x02, 5, Light::First as u8],
            vec![0x02, 5, Light::Locked as u8],
            vec![0x04, 2],
        ]
    );
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(backend.0.lock().unwrap().written.len(), 6, "nothing more is written");
    handle.shutdown();
}

#[test]
fn shows_the_lights_and_mode_set_before_the_receiver_connected_and_again_after_replugging() {
    let backend = FakeBackend::default();
    let (handle, rx) = start(&backend);
    handle.set_mode(BuzzerMode::FastestFinger);
    handle.set_lights(vec![(2, Light::Wrong), (3, Light::Ready)]);
    handle.select(Some(selector()));
    wait_for(&rx, |e| matches!(e, ReceiverEvent::Status(ReceiverStatus::Searching { .. })));
    backend.0.lock().unwrap().plugged = true;
    wait_for(&rx, is_connected);
    let shown = [vec![0x02, 2, Light::Wrong as u8], vec![0x02, 3, Light::Ready as u8], vec![0x04, 3]];
    assert_eq!(written_after_init(&backend, 3), shown);

    {
        let mut w = backend.0.lock().unwrap();
        w.break_link = true;
        w.written.clear();
    }
    wait_for(&rx, |e| matches!(e, ReceiverEvent::Status(ReceiverStatus::Error { .. })));
    wait_for(&rx, is_connected);
    assert_eq!(written_after_init(&backend, 3), shown);
    assert_eq!(backend.0.lock().unwrap().features.len(), 2, "set up again");
    handle.shutdown();
}

#[test]
fn writes_what_the_receivers_answer_calls_for_and_reports_that_it_answered() {
    let backend = FakeBackend::default();
    backend.0.lock().unwrap().plugged = true;
    let (handle, rx) = start_with(&backend, config(), false);
    handle.select(Some(selector()));
    match wait_for(&rx, is_connected) {
        ReceiverEvent::Status(ReceiverStatus::Connected { protocol, .. }) => assert_eq!(protocol, ProtocolKind::Enjoy303s { answered: false }),
        _ => unreachable!(),
    }
    assert_eq!(written_after_init(&backend, 1), [vec![0x04, 0]]);

    backend.0.lock().unwrap().inbox.push_back(vec![0x7e]);
    match wait_for(&rx, is_connected) {
        ReceiverEvent::Status(ReceiverStatus::Connected { protocol, .. }) => assert_eq!(protocol, ProtocolKind::Enjoy303s { answered: true }),
        _ => unreachable!(),
    }
    assert_eq!(written_after_init(&backend, 2), [vec![0x04, 0], vec![0x06, 0x01]]);
    handle.shutdown();
}

#[test]
fn spaces_writes_by_the_write_gap() {
    let backend = FakeBackend::default();
    backend.0.lock().unwrap().plugged = true;
    let config = ReceiverConfig { write_gap: Duration::from_millis(25), ..config() };
    let (handle, rx) = start_with(&backend, config, true);
    handle.select(Some(selector()));
    wait_for(&rx, is_connected);
    handle.set_lights((1..=4).map(|b| (b, Light::Correct)).collect());
    written_after_init(&backend, 5);
    let at = backend.0.lock().unwrap().written_at.clone();
    for pair in at.windows(2) {
        assert!(pair[1] - pair[0] >= Duration::from_millis(25), "{:?}", pair[1] - pair[0]);
    }
    handle.shutdown();
}

/// An input report from an Enjoy 303S receiver carrying the message with
/// `payload`.
fn from_303s(payload: &[u8]) -> Vec<u8> {
    let check = payload.iter().fold(0, |x, b| x ^ b);
    let message = [&[0x27, 0xa5, payload.len() as u8][..], payload, &[check]].concat();
    [&[message.len() as u8][..], &message].concat()
}

/// A press of `buzzer`'s big button, showing ready, `timer` after its light changed.
fn buzz_303s(buzzer: u8, timer: u8) -> Vec<u8> {
    from_303s(&[0x32, buzzer - 1, 0x50, 0x43, timer, 0, 0, 0, 0, 0])
}

/// The receiver thread on an Enjoy 303S receiver that answered its set-up.
fn start_303s(backend: &FakeBackend, config: ReceiverConfig) -> (ReceiverHandle, Receiver<ReceiverEvent>) {
    backend.0.lock().unwrap().plugged = true;
    let (tx, rx) = mpsc::channel();
    let resolver = Arc::new(|_: &DeviceInfo| Box::new(Enjoy303s::default()) as Box<dyn Protocol>);
    let (handle, _join) = spawn(Arc::new(backend.clone()), resolver, tx, config);
    handle.select(Some(selector()));
    wait_for(&rx, is_connected);
    backend.0.lock().unwrap().inbox.extend([from_303s(&[0x10, 0x00]), from_303s(&[0x12, 0x01])]);
    wait_for(&rx, |e| matches!(e, ReceiverEvent::Status(ReceiverStatus::Connected { protocol: ProtocolKind::Enjoy303s { answered: true }, .. })));
    (handle, rx)
}

#[test]
fn an_enjoy_303s_shows_what_waited_once_it_answered_its_set_up() {
    let backend = FakeBackend::default();
    let (tx, rx) = mpsc::channel();
    let resolver = Arc::new(|_: &DeviceInfo| Box::new(Enjoy303s::default()) as Box<dyn Protocol>);
    let (handle, _join) = spawn(Arc::new(backend.clone()), resolver, tx, config());
    handle.set_lights((1..=3).map(|b| (b, Light::Ready)).collect());
    handle.set_mode(BuzzerMode::Answers);
    backend.0.lock().unwrap().plugged = true;
    handle.select(Some(selector()));
    wait_for(&rx, is_connected);
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(backend.0.lock().unwrap().written.len(), 1, "only the connect order until the receiver answers");

    backend.0.lock().unwrap().inbox.push_back(from_303s(&[0x10, 0x00]));
    backend.0.lock().unwrap().inbox.push_back(from_303s(&[0x12, 0x01]));
    let serial: Vec<Vec<u8>> = written_after_init(&backend, 4).iter().map(|r| cp2110::received(r).to_vec()).collect();
    let take_64 = vec![0x35, 0x96, 0x02, 0x14, 0x3f, 0x2b];
    let open = vec![0x35, 0x96, 0x02, 0x12, 0x00, 0x12];
    let ready_all = vec![0x35, 0x96, 0x02, 0x74, 0x43, 0x37];
    let start = vec![0x35, 0x96, 0x01, 0x85, 0x85];
    assert_eq!(serial, [take_64, open, ready_all, start]);
    handle.shutdown();
}

#[test]
fn orders_a_fastest_finger_buzz_by_the_buzzers_timers() {
    let backend = FakeBackend::default();
    let config = ReceiverConfig { timer_window: Duration::from_millis(60), ..config() };
    let (handle, rx) = start_303s(&backend, config);
    handle.set_mode(BuzzerMode::FastestFinger);
    // Buzzer 1's press arrives first, though buzzer 2 pressed sooner after its light.
    backend.0.lock().unwrap().inbox.extend([buzz_303s(1, 200), buzz_303s(2, 120)]);
    let ReceiverEvent::Raw(first) = wait_for(&rx, |e| matches!(e, ReceiverEvent::Raw(r) if !r.decoded.is_empty())) else { unreachable!() };
    assert_eq!(first.decoded[0].buzzer, 1);
    let mut presses = Vec::new();
    while presses.len() < 2 {
        if let ReceiverEvent::Press { press, at_ms } = wait_for(&rx, |e| matches!(e, ReceiverEvent::Press { .. })) {
            presses.push((press.buzzer, press.timer, at_ms));
        }
    }
    assert_eq!(presses, [(2, Some(120), first.at_ms), (1, Some(200), first.at_ms)]);

    // Outside fastest finger a press goes on at once.
    handle.set_mode(BuzzerMode::Answers);
    let sent = Instant::now();
    backend.0.lock().unwrap().inbox.push_back(buzz_303s(3, 50));
    wait_for(&rx, |e| matches!(e, ReceiverEvent::Press { press, .. } if press.buzzer == 3));
    assert!(sent.elapsed() < Duration::from_millis(60), "{:?}", sent.elapsed());
    handle.shutdown();
}

#[test]
fn a_fastest_finger_buzz_waiting_for_the_timers_goes_on_when_the_round_ends() {
    let backend = FakeBackend::default();
    let config = ReceiverConfig { timer_window: Duration::from_secs(30), ..config() };
    let (handle, rx) = start_303s(&backend, config);
    handle.set_mode(BuzzerMode::FastestFinger);
    backend.0.lock().unwrap().inbox.push_back(buzz_303s(4, 90));
    wait_for(&rx, |e| matches!(e, ReceiverEvent::Raw(r) if !r.decoded.is_empty()));
    handle.set_mode(BuzzerMode::Off);
    wait_for(&rx, |e| matches!(e, ReceiverEvent::Press { press, .. } if press.buzzer == 4));
    handle.shutdown();
}

#[test]
fn waits_for_the_receiver_and_reconnects_after_unplugging() {
    let backend = FakeBackend::default();
    let (handle, rx) = start(&backend);
    handle.select(Some(selector()));
    wait_for(&rx, |e| matches!(e, ReceiverEvent::Status(ReceiverStatus::Searching { .. })));

    backend.0.lock().unwrap().plugged = true;
    wait_for(&rx, is_connected);

    {
        let mut w = backend.0.lock().unwrap();
        w.break_link = true;
        w.plugged = false;
    }
    wait_for(&rx, |e| matches!(e, ReceiverEvent::Status(ReceiverStatus::Error { .. })));
    backend.0.lock().unwrap().plugged = true;
    wait_for(&rx, is_connected);
    assert_eq!(backend.0.lock().unwrap().opens, 2);
    handle.shutdown();
}

#[test]
fn reports_open_failures_and_retries() {
    let backend = FakeBackend::default();
    {
        let mut w = backend.0.lock().unwrap();
        w.plugged = true;
        w.fail_open = true;
    }
    let (handle, rx) = start(&backend);
    handle.select(Some(selector()));
    match wait_for(&rx, |e| matches!(e, ReceiverEvent::Status(ReceiverStatus::Error { .. }))) {
        ReceiverEvent::Status(ReceiverStatus::Error { problem, detail }) => {
            assert_eq!(problem, ReceiverProblem::CannotOpen);
            assert!(detail.contains("access denied"), "{detail}");
        }
        _ => unreachable!(),
    }
    backend.0.lock().unwrap().fail_open = false;
    wait_for(&rx, is_connected);
    handle.shutdown();
}

#[test]
fn deselecting_closes_the_receiver() {
    let backend = FakeBackend::default();
    backend.0.lock().unwrap().plugged = true;
    let (handle, rx) = start(&backend);
    handle.select(Some(selector()));
    wait_for(&rx, is_connected);
    handle.select(None);
    wait_for(&rx, |e| matches!(e, ReceiverEvent::Status(ReceiverStatus::Off)));
    assert_eq!(handle.status(), ReceiverStatus::Off);
    handle.shutdown();
}
