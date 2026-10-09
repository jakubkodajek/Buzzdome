//! Receiver thread against a fake HID backend: connect, decode, lights,
//! unplug and replug.

use buzzers::hid::{DeviceInfo, DeviceSelector, HidBackend, HidDevice};
use buzzers::protocol::Protocol;
use buzzers::receiver::{spawn, ReceiverConfig, ReceiverEvent, ReceiverHandle, ReceiverStatus};
use buzzers::{DriverError, Press};
use quiz_core::{BuzzerId, Button, Light};
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct Wire {
    plugged: bool,
    fail_open: bool,
    inbox: VecDeque<Vec<u8>>,
    written: Vec<Vec<u8>>,
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
        self.0.lock().unwrap().written.push(data.to_vec());
        Ok(data.len())
    }
}

/// Test protocol: [0x01, buzzer, button 0..=4]; lights -> [0x02, buzzer, light].
struct TestProtocol;

impl Protocol for TestProtocol {
    fn name(&self) -> String {
        "test".into()
    }

    fn decode(&mut self, r: &[u8]) -> Vec<Press> {
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
        vec![Press { buzzer: r[1] as BuzzerId, button }]
    }

    fn init_reports(&self) -> Vec<Vec<u8>> {
        vec![vec![0x03, 0x01]]
    }

    fn encode_lights(&self, lights: &[(BuzzerId, Light)]) -> Vec<Vec<u8>> {
        lights.iter().map(|(b, l)| vec![0x02, *b as u8, *l as u8]).collect()
    }

    fn has_lights(&self) -> bool {
        true
    }
}

fn config() -> ReceiverConfig {
    ReceiverConfig { retry: Duration::from_millis(30), read_timeout_ms: 5, repeat_gap_ms: 150 }
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
    let (tx, rx) = mpsc::channel();
    let resolver = Arc::new(|_: &DeviceInfo| Box::new(TestProtocol) as Box<dyn Protocol>);
    let (handle, _join) = spawn(Arc::new(backend.clone()), resolver, tx, config());
    (handle, rx)
}

#[test]
fn connects_sends_init_and_decodes_presses_in_order() {
    let backend = FakeBackend::default();
    backend.0.lock().unwrap().plugged = true;
    let (handle, rx) = start(&backend);
    handle.select(Some(selector()));
    wait_for(&rx, is_connected);
    assert_eq!(backend.0.lock().unwrap().written, vec![vec![0x03, 0x01]]);

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
    let expected = [(Press { buzzer: 7, button: Button::Big }, raws[0].at_ms), (Press { buzzer: 3, button: Button::Big }, raws[1].at_ms)];
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
    for _ in 0..200 {
        if backend.0.lock().unwrap().written.len() >= 3 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        backend.0.lock().unwrap().written[1..],
        [vec![0x02, 4, Light::Ready as u8], vec![0x02, 5, Light::First as u8]]
    );
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
        ReceiverEvent::Status(ReceiverStatus::Error { message }) => assert!(message.contains("access denied")),
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
