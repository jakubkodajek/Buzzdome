//! A fake Enjoy 303S receiver on its USB-HID cable, which an end-to-end
//! test drives over TCP. The app's debug builds use it in place of USB-HID
//! when `BUZZDOME_FAKE_RECEIVER` gives the test's address (`hardware.rs` in
//! the app); release builds never do. Everything above the HID backend runs
//! as with real buzzers: the receiver thread, `protocol::for_device`, the
//! 303S protocol and the cable's reports (`cp2110`). The receiver itself is
//! `enjoy303s::SimulatedReceiver`.
//!
//! The app connects to the test, which listens, and connects again whenever
//! the connection is lost; until then the cable is pulled out. Each line
//! either way is one JSON object. From the test:
//!
//! - `{"plug": true}` or `{"plug": false}`: the cable is plugged in or
//!   pulled out. Plugged in again, the receiver has forgotten everything.
//! - `{"press": {"buzzer": 7, "button": "a", "timer": 120}}`: a press, as
//!   `Press` (buttons `a` to `d` and `big`; `timer` is 0 when left out),
//!   sent with the colour the buzzer shows, as the receiver does.
//! - `{"remote": "tick"}`: a key of the quizmaster's remote, as `RemoteKey`
//!   (`back`, `forward`, `power`, `grid`, `target`, `playPause`, `centre`,
//!   `question`, `trophy`, `colourBars`, `cross`, `tick`).
//! - `{"raw": "27 a5 01"}`: bytes the receiver sends as they are, in hex,
//!   such as a spoiled message.
//!
//! To the test:
//!
//! - `{"lights": [{"buzzer": 7, "light": "locked"}]}`: buzzers the app
//!   coloured anew with one write, with the `Light` each shows now (null
//!   for a colour the app never sets). A colour sent to all at once lists
//!   all 64 buzzers.
//! - `{"error": "…"}`: a line that was not understood.

use crate::cp2110;
use crate::enjoy303s::{self, SimulatedReceiver};
use crate::hid::{DeviceInfo, HidBackend, HidDevice};
use crate::{DriverError, Press, RemoteKey};
use quiz_core::{BuzzerId, Light};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

/// The variable that gives the test's address, as `host:port`.
pub const ENV: &str = "BUZZDOME_FAKE_RECEIVER";
const PATH: &str = "fake-303s";
/// Wait before connecting to the test again.
const RECONNECT: Duration = Duration::from_millis(100);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Command {
    Plug(bool),
    Press(Press),
    Remote(RemoteKey),
    Raw(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum Event {
    Lights(Vec<Lit>),
    Error(String),
}

#[derive(Debug, Serialize)]
struct Lit {
    buzzer: BuzzerId,
    light: Option<Light>,
}

#[derive(Default)]
struct Cable {
    plugged: bool,
    /// Counts the cable plugged in and pulled out: a device opened before
    /// is gone.
    plugs: u64,
    receiver: SimulatedReceiver,
    /// The connection to the test, for what the receiver tells it.
    test: Option<TcpStream>,
}

impl Cable {
    fn plug(&mut self, plugged: bool) {
        if self.plugged != plugged {
            self.plugged = plugged;
            self.plugs += 1;
            self.receiver = SimulatedReceiver::default();
        }
    }

    /// Tells the test `event`. A connection that fails is noticed where it is read.
    fn tell(&mut self, event: &Event) {
        if let Some(test) = self.test.as_mut() {
            let line = serde_json::to_string(event).expect("events serialize") + "\n";
            let _ = test.write_all(line.as_bytes());
        }
    }
}

#[derive(Default)]
struct Shared {
    cable: Mutex<Cable>,
    /// Signalled when the receiver has more to send, or the cable is pulled out.
    changed: Condvar,
}

impl Shared {
    /// Takes the test's commands from `stream` until it closes, then pulls
    /// the cable out.
    fn serve(&self, stream: TcpStream) {
        let Ok(test) = stream.try_clone() else { return };
        // A test that stops reading holds up the receiver thread no longer than this.
        let _ = test.set_write_timeout(Some(Duration::from_secs(1)));
        self.cable.lock().unwrap().test = Some(test);
        for line in BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            let mut cable = self.cable.lock().unwrap();
            match serde_json::from_str(&line) {
                Ok(command) => apply(&mut cable, command),
                Err(e) => cable.tell(&Event::Error(format!("{line}: {e}"))),
            }
            self.changed.notify_all();
        }
        let mut cable = self.cable.lock().unwrap();
        cable.test = None;
        cable.plug(false);
        self.changed.notify_all();
    }
}

fn apply(cable: &mut Cable, command: Command) {
    match command {
        Command::Plug(plugged) => cable.plug(plugged),
        // Pressed with the cable pulled out, a buzzer or the remote reaches no one.
        Command::Press(press) if cable.plugged => cable.receiver.press(press),
        Command::Remote(key) if cable.plugged => cable.receiver.remote(key),
        Command::Raw(hex) => match bytes_of(&hex) {
            Some(bytes) if cable.plugged => cable.receiver.send_raw(&bytes),
            Some(_) => {}
            None => cable.tell(&Event::Error(format!("not hex: {hex}"))),
        },
        Command::Press(_) | Command::Remote(_) => {}
    }
}

/// The bytes `hex` spells, spaces allowed between them.
fn bytes_of(hex: &str) -> Option<Vec<u8>> {
    let digits: String = hex.split_whitespace().collect();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    (0..digits.len()).step_by(2).map(|i| u8::from_str_radix(digits.get(i..i + 2)?, 16).ok()).collect()
}

/// The fake receiver's cable as the HID backend lists it: the only device.
pub struct FakeReceiver(Arc<Shared>);

impl FakeReceiver {
    /// A fake receiver whose test listens at `address`, connected to from
    /// a thread of its own for as long as the app runs.
    pub fn connect(address: String) -> FakeReceiver {
        let shared = Arc::new(Shared::default());
        let link = shared.clone();
        std::thread::Builder::new()
            .name("fake-receiver".into())
            .spawn(move || loop {
                if let Ok(stream) = TcpStream::connect(&address) {
                    link.serve(stream);
                }
                std::thread::sleep(RECONNECT);
            })
            .expect("spawn fake receiver thread");
        FakeReceiver(shared)
    }
}

fn info() -> DeviceInfo {
    DeviceInfo {
        path: PATH.into(),
        vendor_id: enjoy303s::VENDOR_ID,
        product_id: enjoy303s::PRODUCT_ID,
        manufacturer: None,
        product: Some("Fake 303S receiver".into()),
        serial_number: None,
        usage_page: 0xff00,
        usage: 1,
        interface_number: 0,
    }
}

impl HidBackend for FakeReceiver {
    fn list(&self) -> Result<Vec<DeviceInfo>, DriverError> {
        Ok(self.0.cable.lock().unwrap().plugged.then(info).into_iter().collect())
    }

    fn open(&self, path: &str) -> Result<Box<dyn HidDevice>, DriverError> {
        let cable = self.0.cable.lock().unwrap();
        if !cable.plugged || path != PATH {
            return Err(DriverError::Device("no such device".into()));
        }
        Ok(Box::new(OpenCable { shared: self.0.clone(), plugs: cable.plugs }))
    }
}

/// The cable as one open of it sees it: gone once pulled out.
struct OpenCable {
    shared: Arc<Shared>,
    plugs: u64,
}

impl OpenCable {
    fn is_this(&self, cable: &Cable) -> bool {
        cable.plugged && cable.plugs == self.plugs
    }

    /// The cable, while it is still the one opened.
    fn cable(&self) -> Result<MutexGuard<'_, Cable>, DriverError> {
        let cable = self.shared.cable.lock().unwrap();
        if self.is_this(&cable) { Ok(cable) } else { Err(DriverError::Device("device disconnected".into())) }
    }
}

impl HidDevice for OpenCable {
    fn read_timeout(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, DriverError> {
        let cable = self.cable()?;
        let wait = Duration::from_millis(u64::try_from(timeout_ms).unwrap_or(0));
        let (mut cable, _) = self.shared.changed.wait_timeout_while(cable, wait, |c| self.is_this(c) && !c.receiver.has_sent()).unwrap();
        if !self.is_this(&cable) {
            return Err(DriverError::Device("device disconnected".into()));
        }
        let bytes = cable.receiver.take_sent(cp2110::MAX_DATA.min(buf.len().saturating_sub(1)));
        if bytes.is_empty() {
            return Ok(0);
        }
        let report = cp2110::data_report(&bytes);
        let n = report.len().min(buf.len());
        buf[..n].copy_from_slice(&report[..n]);
        Ok(n)
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, DriverError> {
        let mut cable = self.cable()?;
        let lit: Vec<Lit> = cable.receiver.hear(cp2110::received(data)).into_iter().map(|(buzzer, light)| Lit { buzzer, light }).collect();
        if !lit.is_empty() {
            cable.tell(&Event::Lights(lit));
        }
        Ok(data.len())
    }

    fn send_feature_report(&mut self, data: &[u8]) -> Result<(), DriverError> {
        let mut cable = self.cable()?;
        if cp2110::purges(data) {
            cable.receiver.purge();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_hex_with_or_without_spaces() {
        assert_eq!(bytes_of("27a5 01"), Some(vec![0x27, 0xa5, 0x01]));
        assert_eq!(bytes_of(""), Some(vec![]));
        assert_eq!(bytes_of("2"), None);
        assert_eq!(bytes_of("zz"), None);
        assert_eq!(bytes_of("é1"), None);
    }

    #[test]
    fn commands_and_events_read_as_documented() {
        let press: Command = serde_json::from_str(r#"{"press": {"buzzer": 7, "button": "big", "timer": 120}}"#).unwrap();
        assert!(matches!(press, Command::Press(p) if p == Press::timed(7, quiz_core::Button::Big, 120)));
        assert!(matches!(serde_json::from_str(r#"{"plug": false}"#).unwrap(), Command::Plug(false)));
        assert!(matches!(serde_json::from_str(r#"{"remote": "playPause"}"#).unwrap(), Command::Remote(RemoteKey::PlayPause)));
        let lights = Event::Lights(vec![Lit { buzzer: 7, light: Some(Light::Locked) }, Lit { buzzer: 8, light: None }]);
        assert_eq!(serde_json::to_string(&lights).unwrap(), r#"{"lights":[{"buzzer":7,"light":"locked"},{"buzzer":8,"light":null}]}"#);
    }
}
