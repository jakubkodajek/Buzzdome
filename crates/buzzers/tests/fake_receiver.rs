//! The fake 303S receiver as an end-to-end test drives it: over TCP, under
//! the real receiver thread and 303S protocol.

use buzzers::clock::ShowClock;
use buzzers::fake_receiver::FakeReceiver;
use buzzers::protocol::{self, ProtocolKind};
use buzzers::receiver::{spawn, ReceiverChoice, ReceiverConfig, ReceiverEvent, ReceiverHandle, ReceiverStatus};
use buzzers::{Press, RemoteKey};
use quiz_core::{BuzzerMode, Button, Light};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Lines, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;

/// The test's end: commands out, the receiver's lines in.
struct Test {
    commands: TcpStream,
    lines: Lines<BufReader<TcpStream>>,
}

impl Test {
    fn send(&mut self, command: Value) {
        writeln!(self.commands, "{command}").unwrap();
    }

    /// The next line from the receiver that `pred` takes.
    fn wait_for(&mut self, pred: impl Fn(&Value) -> bool) -> Value {
        loop {
            let line = self.lines.next().expect("connection open").expect("a line within the read timeout");
            let value: Value = serde_json::from_str(&line).unwrap();
            if pred(&value) {
                return value;
            }
        }
    }
}

/// A fake receiver connected to a test, chosen automatically by a receiver
/// thread that tries to connect every 50 ms.
fn start() -> (Test, ReceiverHandle, Receiver<ReceiverEvent>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let fake = FakeReceiver::connect(listener.local_addr().unwrap().to_string());
    let (stream, _) = listener.accept().unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let test = Test { commands: stream.try_clone().unwrap(), lines: BufReader::new(stream).lines() };
    let (tx, rx) = mpsc::channel();
    let resolver = Arc::new(|device: &_| protocol::for_device(&[], device));
    let config = ReceiverConfig { retry: Duration::from_millis(50), ..ReceiverConfig::new(ShowClock::start()) };
    let (handle, _join) = spawn(Arc::new(fake), resolver, tx, config);
    handle.select(ReceiverChoice::Automatic);
    (test, handle, rx)
}

fn wait_for(rx: &Receiver<ReceiverEvent>, pred: impl Fn(&ReceiverEvent) -> bool) -> ReceiverEvent {
    loop {
        let event = rx.recv_timeout(Duration::from_secs(5)).expect("timed out waiting for a receiver event");
        if pred(&event) {
            return event;
        }
    }
}

fn is_answered(event: &ReceiverEvent) -> bool {
    matches!(event, ReceiverEvent::Status(ReceiverStatus::Connected { protocol: ProtocolKind::Enjoy303s { answered: true }, .. }))
}

fn light_of(lights: &Value, buzzer: u64) -> Option<&Value> {
    lights["lights"].as_array()?.iter().find(|lit| lit["buzzer"] == buzzer).map(|lit| &lit["light"])
}

#[test]
fn a_test_plugs_the_receiver_in_presses_and_sees_the_lights() {
    let (mut test, handle, rx) = start();
    test.send(json!({ "plug": true }));
    wait_for(&rx, is_answered);

    handle.set_mode(BuzzerMode::FastestFinger);
    handle.set_lights(vec![(1, Light::Ready), (2, Light::Ready), (3, Light::Locked)]);
    // Ready for two goes to all at once, then buzzer 3 is locked out.
    test.wait_for(|v| light_of(v, 64).is_some_and(|l| l == "ready"));
    assert_eq!(test.wait_for(|_| true), json!({ "lights": [{ "buzzer": 3, "light": "locked" }] }));

    // The later press has the lower timer: the buzzers' order wins.
    test.send(json!({ "press": { "buzzer": 1, "button": "big", "timer": 300 } }));
    test.send(json!({ "press": { "buzzer": 2, "button": "big", "timer": 120 } }));
    test.send(json!({ "press": { "buzzer": 3, "button": "big", "timer": 50 } }));
    let presses: Vec<Press> = (0..2)
        .map(|_| match wait_for(&rx, |e| matches!(e, ReceiverEvent::Press { .. })) {
            ReceiverEvent::Press { press, .. } => press,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(presses, [Press::timed(2, Button::Big, 120), Press::timed(1, Button::Big, 300)], "buzzer 3 is locked out");

    test.send(json!({ "remote": "tick" }));
    assert!(matches!(wait_for(&rx, |e| matches!(e, ReceiverEvent::Remote { .. })), ReceiverEvent::Remote { key: RemoteKey::Tick, .. }));

    test.send(json!({ "plug": "yes" }));
    assert!(test.wait_for(|v| v.get("error").is_some())["error"].as_str().unwrap().contains("plug"));
}

#[test]
fn pulled_out_and_plugged_in_again_the_receiver_is_set_up_anew() {
    let (mut test, handle, rx) = start();
    test.send(json!({ "plug": true }));
    wait_for(&rx, is_answered);
    handle.set_lights(vec![(5, Light::Correct)]);
    test.wait_for(|v| light_of(v, 5).is_some_and(|l| l == "correct"));

    test.send(json!({ "plug": false }));
    wait_for(&rx, |e| matches!(e, ReceiverEvent::Status(ReceiverStatus::Error { .. } | ReceiverStatus::Looking)));
    test.send(json!({ "press": { "buzzer": 5, "button": "a" } }));
    test.send(json!({ "plug": true }));
    wait_for(&rx, is_answered);
    // The receiver forgot its colours: the app shows them again.
    test.wait_for(|v| light_of(v, 5).is_some_and(|l| l == "correct"));
    test.send(json!({ "press": { "buzzer": 5, "button": "a" } }));
    match wait_for(&rx, |e| matches!(e, ReceiverEvent::Press { .. })) {
        ReceiverEvent::Press { press, .. } => assert_eq!(press, Press::timed(5, Button::A, 0), "the press while pulled out reached no one"),
        _ => unreachable!(),
    }
}
