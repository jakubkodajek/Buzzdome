//! Enjoy 303S RGB buzzers (sold as the Galaxy RGB set), through the REC-A
//! receiver and its USB-HID cable. Not yet checked on a real receiver:
//! `docs/hardware-test.md` says what to check, and `docs/design.md` (2.4)
//! where this description comes from.
//!
//! The cable is a CP2110-type bridge (`cp2110`) to the receiver's serial
//! line, 750000 baud 8N1. A message either way is two header bytes, the
//! payload's length, the payload and the XOR of the payload's bytes.
//!
//! The receiver runs no round by itself: the app lights every buzzer in one
//! of 64 colours, and decides who buzzed first. Each press comes with the
//! colour its buzzer showed and the buzzer's own timer, which counts from
//! the moment its colour last changed (`Press::timer`, `timer_order`).

use crate::cp2110;
use crate::hid::{DeviceInfo, Report};
use crate::protocol::{Protocol, ProtocolKind};
use crate::Press;
use quiz_core::{BuzzerId, BuzzerMode, Button, Light};

pub const VENDOR_ID: u16 = 0x10c4;
pub const PRODUCT_ID: u16 = 0x1819;
const BAUD: u32 = 750_000;
/// Buzzers a receiver takes: 1 to 64 in the app, 0 to 63 on the radio.
pub const MAX_BUZZER: BuzzerId = 64;
const BUZZERS: usize = MAX_BUZZER as usize;

const TO_RECEIVER: [u8; 2] = [0x35, 0x96];
const FROM_RECEIVER: [u8; 2] = [0x27, 0xa5];

/// The first payload byte of what the receiver sends.
const CONNECTED: u8 = 0x10;
const OPENED: u8 = 0x12;
const PRESS: u8 = 0x32;

/// A colour byte: brightness, then red, green and blue at 0 (off) to 3.
/// The top bit, which the app never sets, stores a colour without showing it.
const fn colour(bright: bool, red: u8, green: u8, blue: u8) -> u8 {
    ((bright as u8) << 6) | (red << 4) | (green << 2) | blue
}
const STORED_ONLY: u8 = 0x80;

const OFF: u8 = colour(false, 0, 0, 0);
const READY: u8 = colour(true, 0, 0, 3);
const LOCKED: u8 = colour(false, 3, 0, 0);
const FIRST: u8 = colour(true, 3, 3, 3);
const CORRECT: u8 = colour(true, 0, 3, 0);
const WRONG: u8 = colour(true, 3, 0, 0);

fn colour_of(light: Light) -> u8 {
    match light {
        Light::Off => OFF,
        Light::Ready => READY,
        Light::Locked => LOCKED,
        Light::First => FIRST,
        Light::Correct => CORRECT,
        Light::Wrong => WRONG,
    }
}

/// Where a buzzer's colour is kept: none for a buzzer the receiver does not take.
fn slot(buzzer: BuzzerId) -> Option<usize> {
    (1..=MAX_BUZZER).contains(&buzzer).then(|| usize::from(buzzer - 1))
}

/// What the app tells the receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Order {
    Connect,
    /// Take buzzers 1 to `MAX_BUZZER`.
    TakeBuzzers,
    /// Start driving the buzzers' lights.
    Open,
    /// Take presses.
    Start,
    Stop,
    /// One buzzer's colour, by its slot.
    Paint(usize, u8),
    /// Every buzzer's colour.
    PaintAll(u8),
}

impl Order {
    fn reports(self) -> Vec<Report> {
        let payload = match self {
            Order::Connect => vec![0x10, 0x00],
            Order::TakeBuzzers => vec![0x14, MAX_BUZZER as u8 - 1],
            Order::Open => vec![0x12, 0x00],
            Order::Start => vec![0x85],
            Order::Stop => vec![0x87],
            Order::Paint(slot, colour) => vec![0x2d, slot as u8 % 4, slot as u8 / 4, colour],
            Order::PaintAll(colour) => vec![0x74, colour],
        };
        let mut message = TO_RECEIVER.to_vec();
        message.push(payload.len() as u8);
        message.extend(&payload);
        message.push(xor(&payload));
        cp2110::send(&message)
    }
}

fn xor(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |x, b| x ^ b)
}

/// What the bytes at the start of a stream are.
enum Parsed<'a> {
    /// A whole message `len` bytes long, with its payload.
    Message { payload: &'a [u8], len: usize },
    /// The start of a message whose rest has not arrived yet.
    Incomplete,
    /// Not a message: noise, or a message spoiled on the way.
    Noise,
}

fn parse(bytes: &[u8]) -> Parsed<'_> {
    if !FROM_RECEIVER.starts_with(&bytes[..bytes.len().min(2)]) {
        return Parsed::Noise;
    }
    let Some(&count) = bytes.get(2) else { return Parsed::Incomplete };
    let count = usize::from(count);
    if count == 0 {
        return Parsed::Noise;
    }
    let Some(&check) = bytes.get(3 + count) else { return Parsed::Incomplete };
    let payload = &bytes[3..3 + count];
    if check == xor(payload) { Parsed::Message { payload, len: count + 4 } } else { Parsed::Noise }
}

/// How far the receiver is set up since the cable connected.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Stage {
    /// Waiting for the answer to `Order::Connect`.
    #[default]
    Connecting,
    /// Waiting for the answer to `Order::Open`.
    Opening,
    Open,
}

#[derive(Debug)]
pub struct Enjoy303s {
    /// Bytes received and not yet read as a message.
    pending: Vec<u8>,
    stage: Stage,
    /// Reports for `follow_up`.
    follow: Vec<Report>,
    /// The colour each buzzer should show, by slot, as the app last set it.
    wanted: [Option<u8>; BUZZERS],
    /// The colour each buzzer was last told to show, by slot: none until told.
    shown: [Option<u8>; BUZZERS],
    mode: BuzzerMode,
    /// Whether the receiver was last told to take presses: none until told.
    taking: Option<bool>,
}

impl Default for Enjoy303s {
    fn default() -> Self {
        Enjoy303s {
            pending: Vec::new(),
            stage: Stage::default(),
            follow: Vec::new(),
            wanted: [None; BUZZERS],
            shown: [None; BUZZERS],
            mode: BuzzerMode::Off,
            taking: None,
        }
    }
}

impl Enjoy303s {
    pub fn handles(device: &DeviceInfo) -> bool {
        device.vendor_id == VENDOR_ID && device.product_id == PRODUCT_ID
    }

    /// Handles one message from the receiver: the press it is, if any.
    fn read(&mut self, payload: &[u8]) -> Option<Press> {
        match payload {
            [CONNECTED, ..] if self.stage == Stage::Connecting => {
                self.stage = Stage::Opening;
                self.follow.extend([Order::TakeBuzzers, Order::Open].into_iter().flat_map(Order::reports));
                None
            }
            [OPENED, 0x01, ..] if self.stage == Stage::Opening => {
                self.stage = Stage::Open;
                // What the buzzers should show waited for this.
                let shown = [self.paint(), self.start_or_stop()].concat();
                self.follow.extend(shown);
                None
            }
            [PRESS, ..] => self.press_in(payload),
            _ => None,
        }
    }

    /// The press in a press message: the buzzer's radio ID, the button in
    /// the high four bits, the colour the buzzer showed, its timer in three
    /// bytes, low byte first, and a time the app does not use. A message
    /// cut short gives what it holds.
    ///
    /// In fastest finger only a buzzer that showed ready buzzes: one pressed
    /// before its light turned ready, or while locked out, carries a timer
    /// from an older change of its light, which cannot be set against the
    /// others'.
    fn press_in(&self, payload: &[u8]) -> Option<Press> {
        let [_, radio_id, code, rest @ ..] = payload else { return None };
        let buzzer = BuzzerId::from(*radio_id) + 1;
        slot(buzzer)?;
        // Unverified: which code the big button sends. The others are A to D
        // (and E, F of the vendor's keypads), so any other code is taken for it.
        let button = match code >> 4 {
            1 => Button::A,
            2 => Button::B,
            3 => Button::C,
            4 => Button::D,
            _ => Button::Big,
        };
        let shown = rest.first().map(|c| c & !STORED_ONLY);
        if self.mode == BuzzerMode::FastestFinger && shown.is_some_and(|c| c != READY) {
            return None;
        }
        let timer = rest.get(1..4).map(|t| u32::from_le_bytes([t[0], t[1], t[2], 0]));
        Some(Press { buzzer, button, timer })
    }

    /// The orders that make every buzzer show the colour it should.
    /// Buzzers that turn ready together get it all at once, as their timers
    /// count from that moment and fastest-finger presses are ordered by them
    /// (`timer_order`): one by one, a write apart, the last one painted
    /// would gain that much. Any other colour goes to all at once when that
    /// takes fewer orders. Then each buzzer that should show another colour
    /// gets its own, though it shows the common one for a moment, which
    /// counts for nothing: it is out of the round. A buzzer the app never
    /// lit takes the common colour too.
    fn paint(&mut self) -> Vec<Report> {
        let changed: Vec<usize> = (0..BUZZERS).filter(|&i| self.wanted[i].is_some() && self.wanted[i] != self.shown[i]).collect();
        let wanted = self.wanted.iter().flatten();
        let others = |c: u8| wanted.clone().filter(|&&w| w != c).count();
        let turning_ready = changed.iter().filter(|&&i| self.wanted[i] == Some(READY)).count();
        let common = if turning_ready > 1 {
            Some(READY)
        } else {
            wanted.clone().copied().min_by_key(|&c| others(c)).filter(|&c| 1 + others(c) < changed.len())
        };
        let orders: Vec<Order> = match common {
            Some(common) => {
                let own = (0..BUZZERS).filter_map(|i| self.wanted[i].filter(|&w| w != common).map(|w| Order::Paint(i, w)));
                std::iter::once(Order::PaintAll(common)).chain(own).collect()
            }
            None => changed.into_iter().filter_map(|i| self.wanted[i].map(|w| Order::Paint(i, w))).collect(),
        };
        for order in &orders {
            match *order {
                Order::PaintAll(c) => self.shown = [Some(c); BUZZERS],
                Order::Paint(i, c) => self.shown[i] = Some(c),
                _ => {}
            }
        }
        orders.into_iter().flat_map(Order::reports).collect()
    }

    /// The order that has the receiver take presses while they can count,
    /// and stop between questions. While a buzz is judged or the results
    /// show it goes on as it was, so a round is not stopped and started
    /// again for nothing.
    fn start_or_stop(&mut self) -> Vec<Report> {
        let take = match self.mode {
            BuzzerMode::Off => false,
            BuzzerMode::Register | BuzzerMode::Answers | BuzzerMode::FastestFinger => true,
            BuzzerMode::Buzzed | BuzzerMode::Results => return Vec::new(),
        };
        if self.taking == Some(take) {
            return Vec::new();
        }
        self.taking = Some(take);
        if take { Order::Start.reports() } else { Order::Stop.reports() }
    }
}

impl Protocol for Enjoy303s {
    fn kind(&self) -> ProtocolKind {
        ProtocolKind::Enjoy303s { answered: self.stage == Stage::Open }
    }

    /// Reads the messages in `report`, with what earlier reports left of
    /// one. Noise is skipped a byte at a time until messages line up again;
    /// so is the start of a message that cannot be whole when a whole
    /// message follows it, so a spoiled header holds up no press.
    fn decode(&mut self, report: &[u8]) -> Vec<Press> {
        self.pending.extend_from_slice(cp2110::received(report));
        let pending = std::mem::take(&mut self.pending);
        let mut presses = Vec::new();
        let mut at = 0;
        while at < pending.len() {
            match parse(&pending[at..]) {
                Parsed::Message { payload, len } => {
                    presses.extend(self.read(payload));
                    at += len;
                }
                Parsed::Noise => at += 1,
                Parsed::Incomplete => match (at + 1..pending.len()).find(|&i| matches!(parse(&pending[i..]), Parsed::Message { .. })) {
                    Some(next) => at = next,
                    None => break,
                },
            }
        }
        self.pending = pending[at..].to_vec();
        presses
    }

    fn init_reports(&self) -> Vec<Report> {
        let mut reports = cp2110::open_uart(BAUD);
        reports.extend(Order::Connect.reports());
        reports
    }

    fn follow_up(&mut self) -> Vec<Report> {
        std::mem::take(&mut self.follow)
    }

    /// Kept until the receiver is open, then written.
    fn encode_lights(&mut self, lights: &[(BuzzerId, Light)]) -> Vec<Report> {
        for &(buzzer, light) in lights {
            if let Some(i) = slot(buzzer) {
                self.wanted[i] = Some(colour_of(light));
            }
        }
        if self.stage == Stage::Open { self.paint() } else { Vec::new() }
    }

    /// Kept until the receiver is open, then written.
    fn encode_mode(&mut self, mode: BuzzerMode) -> Vec<Report> {
        self.mode = mode;
        if self.stage == Stage::Open { self.start_or_stop() } else { Vec::new() }
    }

    fn has_lights(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The serial bytes of `reports`, one message per report.
    fn serial(reports: &[Report]) -> Vec<Vec<u8>> {
        reports
            .iter()
            .map(|r| match r {
                Report::Output(r) => cp2110::received(r).to_vec(),
                Report::Feature(r) => panic!("a feature report: {r:?}"),
            })
            .collect()
    }

    /// An input report carrying `bytes`.
    fn input(bytes: &[u8]) -> Vec<u8> {
        [&[bytes.len() as u8][..], bytes].concat()
    }

    /// A message from the receiver with `payload`.
    fn message(payload: &[u8]) -> Vec<u8> {
        [&FROM_RECEIVER[..], &[payload.len() as u8], payload, &[xor(payload)]].concat()
    }

    fn press_message(radio_id: u8, code: u8, shown: u8, timer: u32) -> Vec<u8> {
        let t = timer.to_le_bytes();
        message(&[PRESS, radio_id, code, shown, t[0], t[1], t[2], 0, 0, 0])
    }

    const CONNECT: [u8; 6] = [0x35, 0x96, 0x02, 0x10, 0x00, 0x10];
    const TAKE_64: [u8; 6] = [0x35, 0x96, 0x02, 0x14, 0x3f, 0x2b];
    const OPEN: [u8; 6] = [0x35, 0x96, 0x02, 0x12, 0x00, 0x12];
    const START: [u8; 5] = [0x35, 0x96, 0x01, 0x85, 0x85];
    const STOP: [u8; 5] = [0x35, 0x96, 0x01, 0x87, 0x87];

    /// A receiver that answered its set-up, and the orders it got for it.
    fn opened() -> (Enjoy303s, Vec<Vec<u8>>) {
        let mut p = Enjoy303s::default();
        p.decode(&input(&message(&[CONNECTED, 0x00])));
        let mut written = serial(&p.follow_up());
        p.decode(&input(&message(&[OPENED, 0x01])));
        written.extend(serial(&p.follow_up()));
        (p, written)
    }

    fn timed(buzzer: BuzzerId, button: Button, timer: u32) -> Press {
        Press { timer: Some(timer), ..Press::new(buzzer, button) }
    }

    #[test]
    fn opens_the_cable_at_750000_baud_and_connects() {
        let reports = Enjoy303s::default().init_reports();
        assert_eq!(reports[0], Report::Feature(vec![0x50, 0x00, 0x0b, 0x71, 0xb0, 0x00, 0x00, 0x03, 0x00]));
        assert_eq!(reports[..3], cp2110::open_uart(BAUD)[..]);
        assert_eq!(serial(&reports[3..]), [CONNECT]);
    }

    #[test]
    fn takes_all_buzzers_and_opens_once_connected_and_is_answered_once_open() {
        let mut p = Enjoy303s::default();
        assert_eq!(p.kind(), ProtocolKind::Enjoy303s { answered: false });
        assert!(p.decode(&input(&message(&[OPENED, 0x01]))).is_empty());
        assert!(p.follow_up().is_empty(), "nothing before the connect answer");

        p.decode(&input(&message(&[CONNECTED, 0x00])));
        assert_eq!(serial(&p.follow_up()), [TAKE_64.to_vec(), OPEN.to_vec()]);
        assert_eq!(p.kind(), ProtocolKind::Enjoy303s { answered: false });
        p.decode(&input(&message(&[OPENED, 0x00])));
        assert_eq!(p.kind(), ProtocolKind::Enjoy303s { answered: false }, "not opened");
        p.decode(&input(&message(&[OPENED, 0x01])));
        assert_eq!(p.kind(), ProtocolKind::Enjoy303s { answered: true });
        // No lights were set; between questions a round from before stops.
        assert_eq!(serial(&p.follow_up()), [STOP]);
        // A second answer starts nothing again.
        p.decode(&input(&message(&[CONNECTED, 0x00])));
        assert!(p.follow_up().is_empty());
    }

    #[test]
    fn lights_and_mode_set_before_the_receiver_opened_follow_its_answer() {
        let mut p = Enjoy303s::default();
        assert!(p.encode_lights(&[(1, Light::Ready), (2, Light::Ready), (3, Light::Wrong)]).is_empty());
        assert!(p.encode_mode(BuzzerMode::Answers).is_empty());
        p.decode(&input(&message(&[CONNECTED, 0x00])));
        p.follow_up();
        p.decode(&input(&message(&[OPENED, 0x01])));
        let ready_all = [0x35, 0x96, 0x02, 0x74, READY, 0x74 ^ READY];
        let wrong_3 = [0x35, 0x96, 0x04, 0x2d, 2, 0, WRONG, 0x2d ^ 2 ^ WRONG];
        assert_eq!(serial(&p.follow_up()), [ready_all.to_vec(), wrong_3.to_vec(), START.to_vec()]);
    }

    #[test]
    fn colours_one_buzzer_by_its_radio_id() {
        let (mut p, _) = opened();
        // Buzzer 6 is radio ID 5: 5 % 4 and 5 / 4.
        assert_eq!(serial(&p.encode_lights(&[(6, Light::First)])), [[0x35, 0x96, 0x04, 0x2d, 0x01, 0x01, 0x7f, 0x52]]);
        assert_eq!(serial(&p.encode_lights(&[(64, Light::Correct)])), [[0x35, 0x96, 0x04, 0x2d, 0x03, 0x0f, 0x4c, 0x2d ^ 0x03 ^ 0x0f ^ 0x4c]]);
        assert!(p.encode_lights(&[(0, Light::Ready), (65, Light::Ready)]).is_empty(), "no such buzzers");
        assert!(p.encode_lights(&[(6, Light::First)]).is_empty(), "shown already");
    }

    #[test]
    fn a_colour_most_buzzers_take_goes_to_all_at_once() {
        let (mut p, _) = opened();
        let paint = |i: u8, c: u8| vec![0x35, 0x96, 0x04, 0x2d, i % 4, i / 4, c, 0x2d ^ (i % 4) ^ (i / 4) ^ c];
        let all = |c: u8| vec![0x35, 0x96, 0x02, 0x74, c, 0x74 ^ c];
        // Two buzzers: one order each costs no more than one for all and one more.
        assert_eq!(serial(&p.encode_lights(&[(1, Light::Ready), (2, Light::Locked)])), [paint(0, READY), paint(1, LOCKED)]);
        // A round begins on four buzzers, one of them switched off.
        let round: Vec<_> = (1..=4).map(|b| (b, Light::Ready)).chain([(5, Light::Locked)]).collect();
        assert_eq!(serial(&p.encode_lights(&round)), [all(READY), paint(4, LOCKED)]);
        // Buzzer 3 buzzes.
        let buzzed = [(1, Light::Locked), (2, Light::Locked), (3, Light::First), (4, Light::Locked)];
        assert_eq!(serial(&p.encode_lights(&buzzed)), [all(LOCKED), paint(2, FIRST)]);
        // Judged wrong: the round goes on for the others.
        let on = [(1, Light::Ready), (2, Light::Ready), (3, Light::Wrong), (4, Light::Ready)];
        assert_eq!(serial(&p.encode_lights(&on)), [all(READY), paint(2, WRONG), paint(4, LOCKED)]);
        // One answer: just that buzzer.
        assert_eq!(serial(&p.encode_lights(&[(1, Light::Locked)])), [paint(0, LOCKED)]);
    }

    #[test]
    fn buzzers_turning_ready_together_turn_at_once() {
        let (mut p, _) = opened();
        let paint = |i: u8, c: u8| vec![0x35, 0x96, 0x04, 0x2d, i % 4, i / 4, c, 0x2d ^ (i % 4) ^ (i / 4) ^ c];
        let buzzed: Vec<_> = [(1, Light::First)].into_iter().chain((2..=6).map(|b| (b, Light::Locked))).collect();
        p.encode_lights(&buzzed);
        // Three out of the round: one order each would cost fewer orders but
        // start the timers of buzzers 2 and 3 a write apart.
        let on = [(1, Light::Wrong), (2, Light::Ready), (3, Light::Ready)];
        let all_ready = vec![0x35, 0x96, 0x02, 0x74, READY, 0x74 ^ READY];
        assert_eq!(serial(&p.encode_lights(&on)), [all_ready, paint(0, WRONG), paint(3, LOCKED), paint(4, LOCKED), paint(5, LOCKED)]);
        // Buzzer 2 buzzes and is judged wrong: buzzer 3 alone turns ready.
        assert_eq!(serial(&p.encode_lights(&[(2, Light::First), (3, Light::Locked)])), [paint(1, FIRST), paint(2, LOCKED)]);
        assert_eq!(serial(&p.encode_lights(&[(2, Light::Wrong), (3, Light::Ready)])), [paint(1, WRONG), paint(2, READY)]);
    }

    #[test]
    fn the_receiver_takes_presses_while_they_can_count() {
        let (mut p, written) = opened();
        assert_eq!(written, [TAKE_64.to_vec(), OPEN.to_vec(), STOP.to_vec()], "between questions on connecting");
        assert_eq!(serial(&p.encode_mode(BuzzerMode::Register)), [START]);
        assert!(p.encode_mode(BuzzerMode::Answers).is_empty(), "taking already");
        assert!(p.encode_mode(BuzzerMode::Results).is_empty());
        assert_eq!(serial(&p.encode_mode(BuzzerMode::Off)), [STOP]);
        assert_eq!(serial(&p.encode_mode(BuzzerMode::FastestFinger)), [START]);
        assert!(p.encode_mode(BuzzerMode::Buzzed).is_empty());
        assert!(p.encode_mode(BuzzerMode::FastestFinger).is_empty());
    }

    #[test]
    fn reads_presses_with_the_buzzers_timer() {
        let (mut p, _) = opened();
        // Pinned: buzzer 3 (radio ID 2) presses A, showing ready, 500 after its light changed.
        let frame = [0x27, 0xa5, 0x0a, 0x32, 0x02, 0x10, 0x43, 0xf4, 0x01, 0x00, 0x00, 0x00, 0x00, 0x96];
        assert_eq!(press_message(2, 0x10, READY, 500), frame);
        assert_eq!(p.decode(&input(&frame)), [timed(3, Button::A, 500)]);
        let codes = [(0x20, Button::B), (0x30, Button::C), (0x4f, Button::D), (0x50, Button::Big), (0x00, Button::Big)];
        for (code, button) in codes {
            assert_eq!(p.decode(&input(&press_message(63, code, OFF, 0x01_02_03))), [timed(64, button, 0x01_02_03)]);
        }
        assert!(p.decode(&input(&press_message(64, 0x10, OFF, 1))).is_empty(), "no buzzer 65");
        // A message cut short gives what it holds.
        assert_eq!(p.decode(&input(&message(&[PRESS, 0, 0x10]))), [Press::new(1, Button::A)]);
        // Quizmaster's remote and set-ID answers are not presses.
        assert!(p.decode(&input(&message(&[0x34, 0x05]))).is_empty());
        assert!(p.decode(&input(&message(&[0x17, 0x01]))).is_empty());
    }

    #[test]
    fn in_fastest_finger_only_a_buzzer_that_showed_ready_buzzes() {
        let (mut p, _) = opened();
        p.encode_mode(BuzzerMode::FastestFinger);
        assert_eq!(p.decode(&input(&press_message(0, 0x50, READY | STORED_ONLY, 300))), [timed(1, Button::Big, 300)]);
        assert!(p.decode(&input(&press_message(1, 0x50, LOCKED, 300))).is_empty());
        assert!(p.decode(&input(&press_message(2, 0x50, OFF, 90_000))).is_empty(), "pressed before its light turned ready");
        p.encode_mode(BuzzerMode::Answers);
        assert_eq!(p.decode(&input(&press_message(2, 0x10, OFF, 90_000))), [timed(3, Button::A, 90_000)]);
    }

    #[test]
    fn messages_split_over_reports_or_spoiled_are_read_once_whole() {
        let mut p = Enjoy303s::default();
        let a = press_message(0, 0x10, READY, 1);
        let b = press_message(1, 0x20, READY, 2);
        let stream = [&a[..], &[0x00, 0x27], &b[..]].concat();
        let (first, rest) = stream.split_at(5);
        assert!(p.decode(&input(first)).is_empty());
        assert_eq!(p.decode(&input(rest)), [timed(1, Button::A, 1), timed(2, Button::B, 2)]);

        // A wrong check byte drops the message.
        let mut spoiled = a.clone();
        *spoiled.last_mut().unwrap() ^= 0xff;
        assert!(p.decode(&input(&spoiled)).is_empty());
        // A header spoiled into a long length holds up no press behind it.
        let held_up = [&[0x27, 0xa5, 0x3c, 0x32][..], &b].concat();
        assert_eq!(p.decode(&input(&held_up)), [timed(2, Button::B, 2)]);
        assert_eq!(p.decode(&input(&a)), [timed(1, Button::A, 1)], "lined up again");
        // A report that is not data carries no bytes.
        assert!(p.decode(&[0x50, 0x27, 0xa5]).is_empty());
    }

    #[test]
    fn handles_only_its_own_cable() {
        let device = |vendor_id, product_id| DeviceInfo {
            path: "x".into(),
            vendor_id,
            product_id,
            manufacturer: None,
            product: None,
            serial_number: None,
            usage_page: 0,
            usage: 0,
            interface_number: 0,
        };
        assert!(Enjoy303s::handles(&device(0x10c4, 0x1819)));
        assert!(!Enjoy303s::handles(&device(0x10c4, 0xea60)));
        let chosen = crate::protocol::for_device(&[], &device(0x10c4, 0x1819));
        assert_eq!(chosen.kind(), ProtocolKind::Enjoy303s { answered: false });
        assert!(!chosen.learnable());
    }
}
