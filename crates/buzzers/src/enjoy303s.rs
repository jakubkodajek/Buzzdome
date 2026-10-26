//! Enjoy 303S RGB buzzers (sold as the Galaxy RGB set), through the REC-A
//! receiver and its USB-HID cable. First checked on a real receiver on
//! 2026-10-25 (Windows 11, buzzers 1 and 2; `docs/design.md` section 6):
//! `docs/hardware-test.md` says what that left to check, and
//! `docs/design.md` (2.4) where this description comes from.
//!
//! The cable is a CP2110-type bridge (`cp2110`) to the receiver's serial
//! line, 750000 baud 8N1. A message either way is two header bytes, the
//! payload's length, the payload and the XOR of the payload's bytes.
//!
//! The receiver runs no round by itself: the app lights every buzzer in one
//! of 64 colours, and decides who buzzed first. Each press comes with the
//! colour its buzzer showed and the buzzer's own timer, which counts from
//! the moment its colour last changed (`Press::timer`, `timer_order`).
//! What that first test did not check says so where it matters.

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

/// How long a set-up step waits for the receiver's answer before it is
/// asked again: the vendor's SDK waits 200 ms for one.
const ANSWER_WAIT_MS: u64 = 500;
/// How many times open goes unanswered before the set-up starts again from
/// connect: a receiver that answered connect and then lost its power, or
/// never opens, is asked to connect as after plugging it in.
const OPENS_BEFORE_CONNECT: u32 = 4;

/// A colour byte: brightness, then red, green and blue at 0 (off) to 3.
/// The top bit, which the app never sets, stores a colour without showing it.
const fn colour(bright: bool, red: u8, green: u8, blue: u8) -> u8 {
    assert!(red < 4 && green < 4 && blue < 4, "a colour level is 0 to 3");
    ((bright as u8) << 6) | (red << 4) | (green << 2) | blue
}

/// The red, green and blue of a colour byte, without its brightness or
/// the top bit.
const fn rgb(colour: u8) -> u8 {
    colour & 0x3f
}

const OFF: u8 = colour(false, 0, 0, 0);
const READY: u8 = colour(true, 0, 0, 3);
/// Dim white, which the red of a wrong answer next to it is told apart from.
const LOCKED: u8 = colour(false, 1, 1, 1);
const FIRST: u8 = colour(true, 3, 3, 3);
const CORRECT: u8 = colour(true, 0, 3, 0);
const WRONG: u8 = colour(true, 3, 0, 0);

/// The colours of a buzzer that is not ready: one pressed showing any of
/// them is out of the round (`Enjoy303s::press_in`).
const NOT_READY: [u8; 5] = [OFF, LOCKED, FIRST, CORRECT, WRONG];

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
    /// When the set-up step waiting for its answer was last asked, once
    /// `follow_up` saw it.
    asked_ms: Option<u64>,
    /// How many times open was asked since connect was answered.
    opens: u32,
    /// The colour each buzzer should show, by slot, as the app last set it.
    wanted: [Option<u8>; BUZZERS],
    /// The colour each buzzer was last told to show, by slot: none until told.
    shown: [Option<u8>; BUZZERS],
    mode: BuzzerMode,
    /// Presses read but not passed on, for `take_dropped`.
    dropped: Vec<Press>,
}

impl Default for Enjoy303s {
    fn default() -> Self {
        Enjoy303s {
            pending: Vec::new(),
            stage: Stage::default(),
            follow: Vec::new(),
            asked_ms: None,
            opens: 0,
            wanted: [None; BUZZERS],
            shown: [None; BUZZERS],
            mode: BuzzerMode::Off,
            dropped: Vec::new(),
        }
    }
}

impl Enjoy303s {
    pub fn handles(device: &DeviceInfo) -> bool {
        device.vendor_id == VENDOR_ID && device.product_id == PRODUCT_ID
    }

    /// The orders of the set-up step that waits for the receiver's answer:
    /// to connect, the cable's settings first, as after plugging it in.
    fn ask(&self) -> Vec<Report> {
        match self.stage {
            Stage::Connecting => [cp2110::open_uart(BAUD), Order::Connect.reports()].concat(),
            Stage::Opening => [Order::TakeBuzzers, Order::Open].into_iter().flat_map(Order::reports).collect(),
            Stage::Open => Vec::new(),
        }
    }

    /// Handles one message from the receiver: the press it is, if any.
    ///
    /// The answer to connect once open says the receiver started again
    /// (unverified that a receiver powered again says so): it has forgotten
    /// its set-up and what the buzzers were told, so both are sent again.
    /// The app never asks to connect while open. An answer to open counts
    /// only while open is asked: a late one, to an open asked again, says
    /// nothing new. A press before the set-up was answered says the
    /// receiver is open already: it stayed powered while the app started
    /// again, and may not answer connect then (unverified).
    fn read(&mut self, payload: &[u8]) -> Option<Press> {
        match payload {
            [CONNECTED, ..] if self.stage != Stage::Opening => {
                self.stage = Stage::Opening;
                self.asked_ms = None;
                self.opens = 1;
                self.follow.extend(self.ask());
                None
            }
            [OPENED, 0x01, ..] if self.stage == Stage::Opening => {
                self.opened();
                None
            }
            [PRESS, ..] => {
                if self.stage != Stage::Open {
                    self.opened();
                }
                self.press_in(payload)
            }
            _ => None,
        }
    }

    /// The receiver is open: it takes presses, and every buzzer goes dark
    /// and is then told what it should show, which waited for this
    /// (`repaint`). The receiver takes presses before any buzzer lights
    /// ready.
    fn opened(&mut self) {
        self.stage = Stage::Open;
        let shown = [Order::Start.reports(), self.repaint()].concat();
        self.follow.extend(shown);
    }

    /// The press in a press message: the buzzer's radio ID, the button in
    /// the high four bits, the colour the buzzer showed, its timer in three
    /// bytes, low byte first, and a time the app does not use. A message
    /// too short to hold all the app uses is no press.
    ///
    /// In fastest finger, and while its buzz is judged, a press counts only
    /// from a buzzer that was ready: one pressed while dark, locked out or
    /// showing a result must not count when it arrives just after the step
    /// changed, before the mode did, and its timer counts from another
    /// moment. Brightness aside, as the buzzer may report its own, and a
    /// colour the app never sets lets the press through, as what the
    /// buzzer reports is unverified; one that reads as dark does not, which
    /// the hardware test checks. Elsewhere the game decides, and any buzzer
    /// can register.
    fn press_in(&mut self, payload: &[u8]) -> Option<Press> {
        let [_, radio_id, code, shown, t0, t1, t2, ..] = *payload else { return None };
        let buzzer = BuzzerId::from(radio_id) + 1;
        slot(buzzer)?;
        // The big button sends 5 (hardware test, 2026-10-25), which the
        // guide gives for key E of the vendor's keypads; a buzzer has no
        // other key. Any other code is no press, so nothing unknown ever
        // buzzes.
        let button = match code >> 4 {
            1 => Button::A,
            2 => Button::B,
            3 => Button::C,
            4 => Button::D,
            5 => Button::Big,
            _ => return None,
        };
        let press = Press::timed(buzzer, button, u32::from_le_bytes([t0, t1, t2, 0]));
        let judged = matches!(self.mode, BuzzerMode::FastestFinger | BuzzerMode::Buzzed);
        if judged && NOT_READY.iter().any(|&c| rgb(c) == rgb(shown)) {
            self.dropped.push(press);
            return None;
        }
        Some(press)
    }

    /// The orders that make every buzzer show the colour it should.
    ///
    /// Buzzers that turn ready together get it all at once, as their timers
    /// count from that moment and fastest-finger presses are ordered by them
    /// (`timer_order`): one by one, a write apart, the last one painted
    /// would gain that much. Any other colour goes to all at once when that
    /// takes fewer orders, but never while a buzzer should show ready, which
    /// would then be lit again one by one. Each buzzer that should show
    /// another colour than the common one then gets its own, though it
    /// shows the common one for a moment, which counts for nothing: it is
    /// out of the round. A buzzer the app never lit takes the common colour
    /// too. No buzzer turns ready alone while others already count: a player
    /// switched on mid-round joins at the next one (`quiz_core::Game`).
    fn paint(&mut self) -> Vec<Report> {
        let changed: Vec<usize> = (0..BUZZERS).filter(|&i| self.wanted[i].is_some() && self.wanted[i] != self.shown[i]).collect();
        let wanted = self.wanted.iter().flatten();
        let others = |c: u8| wanted.clone().filter(|&&w| w != c).count();
        let any_ready = wanted.clone().any(|&c| c == READY);
        let turning_ready = changed.iter().filter(|&&i| self.wanted[i] == Some(READY)).count();
        let common = if turning_ready > 1 {
            Some(READY)
        } else {
            let candidates = wanted.clone().copied().filter(|&c| c == READY || !any_ready);
            candidates.min_by_key(|&c| others(c)).filter(|&c| 1 + others(c) < changed.len())
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

    /// Every buzzer dark, then each the colour it should show: what the
    /// buzzers show whatever they showed before, such as the colours
    /// another program left. A buzzer the app does not light stays dark,
    /// unless a colour goes to all at once (`paint`), as ready for two or
    /// more buzzers does: then it shows that colour too.
    fn repaint(&mut self) -> Vec<Report> {
        self.shown = [Some(OFF); BUZZERS];
        [Order::PaintAll(OFF).reports(), self.paint()].concat()
    }

    /// Notes the colours these buzzers should show.
    fn want(&mut self, lights: &[(BuzzerId, Light)]) {
        for &(buzzer, light) in lights {
            if let Some(i) = slot(buzzer) {
                self.wanted[i] = Some(colour_of(light));
            }
        }
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
        self.ask()
    }

    fn take_dropped(&mut self) -> Vec<Press> {
        std::mem::take(&mut self.dropped)
    }

    /// What the receiver's answers call for, and the set-up step again
    /// while its answer is overdue: one lost or spoiled on the way, or a
    /// receiver powered after the cable connected, delays the set-up only.
    /// Open unanswered `OPENS_BEFORE_CONNECT` times, the set-up starts again
    /// from connect.
    fn follow_up(&mut self, now_ms: u64) -> Vec<Report> {
        if self.stage != Stage::Open {
            let asked_ms = *self.asked_ms.get_or_insert(now_ms);
            if now_ms.saturating_sub(asked_ms) >= ANSWER_WAIT_MS {
                self.asked_ms = Some(now_ms);
                if self.stage == Stage::Opening {
                    if self.opens >= OPENS_BEFORE_CONNECT {
                        self.stage = Stage::Connecting;
                    } else {
                        self.opens += 1;
                    }
                }
                self.follow.extend(self.ask());
            }
        }
        std::mem::take(&mut self.follow)
    }

    /// Kept until the receiver is open, then written.
    fn encode_lights(&mut self, lights: &[(BuzzerId, Light)]) -> Vec<Report> {
        self.want(lights);
        if self.stage == Stage::Open { self.paint() } else { Vec::new() }
    }

    /// Kept until the receiver is open, then written, all buzzers dark
    /// first (`repaint`).
    fn reset_lights(&mut self, lights: &[(BuzzerId, Light)]) -> Vec<Report> {
        self.wanted = [None; BUZZERS];
        self.want(lights);
        if self.stage == Stage::Open { self.repaint() } else { Vec::new() }
    }

    /// Kept for telling presses that count from those that cannot: the
    /// receiver takes presses all the time once open, so a late team still
    /// registers between questions (unverified: that `85` once is enough).
    fn encode_mode(&mut self, mode: BuzzerMode) -> Vec<Report> {
        self.mode = mode;
        Vec::new()
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

    /// The order that colours the buzzer in slot `i`.
    fn paint(i: u8, c: u8) -> Vec<u8> {
        vec![0x35, 0x96, 0x04, 0x2d, i % 4, i / 4, c, 0x2d ^ (i % 4) ^ (i / 4) ^ c]
    }

    /// The order that colours every buzzer.
    fn all(c: u8) -> Vec<u8> {
        vec![0x35, 0x96, 0x02, 0x74, c, 0x74 ^ c]
    }

    /// A receiver that answered its set-up, and the orders it got for it.
    fn opened() -> (Enjoy303s, Vec<Vec<u8>>) {
        let mut p = Enjoy303s::default();
        p.decode(&input(&message(&[CONNECTED, 0x00])));
        let mut written = serial(&p.follow_up(0));
        p.decode(&input(&message(&[OPENED, 0x01])));
        written.extend(serial(&p.follow_up(0)));
        (p, written)
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
        assert!(p.follow_up(0).is_empty(), "nothing before the connect answer");

        p.decode(&input(&message(&[CONNECTED, 0x00])));
        assert_eq!(serial(&p.follow_up(0)), [TAKE_64.to_vec(), OPEN.to_vec()]);
        assert_eq!(p.kind(), ProtocolKind::Enjoy303s { answered: false });
        p.decode(&input(&message(&[OPENED, 0x00])));
        assert_eq!(p.kind(), ProtocolKind::Enjoy303s { answered: false }, "not opened");
        p.decode(&input(&message(&[OPENED, 0x01])));
        assert_eq!(p.kind(), ProtocolKind::Enjoy303s { answered: true });
        // No lights were set: it takes presses, every buzzer dark.
        assert_eq!(serial(&p.follow_up(0)), [START.to_vec(), all(OFF)]);
        p.decode(&input(&message(&[CONNECTED, 0x00])));
        assert_eq!(serial(&p.follow_up(0)), [TAKE_64.to_vec(), OPEN.to_vec()], "connect answered once open: started again");
        p.decode(&input(&message(&[CONNECTED, 0x00])));
        assert!(p.follow_up(0).is_empty(), "connect answered again while opening");
    }

    #[test]
    fn a_receiver_that_answers_its_set_up_again_once_open_is_set_up_again() {
        let (mut p, _) = opened();
        p.encode_lights(&[(1, Light::Ready), (2, Light::Ready), (3, Light::Wrong)]);
        // Started again: connect is answered while open.
        p.decode(&input(&message(&[CONNECTED, 0x00])));
        assert_eq!(p.kind(), ProtocolKind::Enjoy303s { answered: false });
        assert_eq!(serial(&p.follow_up(0)), [TAKE_64.to_vec(), OPEN.to_vec()]);
        assert!(p.encode_lights(&[(4, Light::Ready)]).is_empty(), "kept until open again");
        p.decode(&input(&message(&[OPENED, 0x01])));
        // Every light again, as the receiver forgot them, from dark: every
        // ready buzzer's timer starts again then.
        assert_eq!(serial(&p.follow_up(0)), [START.to_vec(), all(OFF), all(READY), paint(2, WRONG)]);
        // A late answer to an open asked again says nothing new: the ready
        // buzzers' timers keep counting from when they lit.
        p.decode(&input(&message(&[OPENED, 0x01])));
        assert!(p.follow_up(0).is_empty());
        assert_eq!(p.kind(), ProtocolKind::Enjoy303s { answered: true });
    }

    #[test]
    fn a_receiver_that_does_not_open_is_asked_to_connect_again() {
        let mut p = Enjoy303s::default();
        p.decode(&input(&message(&[CONNECTED, 0x00])));
        assert_eq!(serial(&p.follow_up(0)), [TAKE_64.to_vec(), OPEN.to_vec()]);
        for at in [500, 1_000, 1_500] {
            assert_eq!(serial(&p.follow_up(at)), [TAKE_64.to_vec(), OPEN.to_vec()], "{at}");
        }
        // Four opens unanswered: as after plugging it in.
        assert_eq!(p.follow_up(2_000), p.init_reports());
        p.decode(&input(&message(&[CONNECTED, 0x00])));
        assert_eq!(serial(&p.follow_up(2_100)), [TAKE_64.to_vec(), OPEN.to_vec()], "open asked again from the start");
        assert_eq!(serial(&p.follow_up(2_600)), [TAKE_64.to_vec(), OPEN.to_vec()]);
    }

    #[test]
    fn a_press_before_the_set_up_is_answered_says_the_receiver_is_open() {
        for opening in [false, true] {
            // The app started again while the receiver stayed open.
            let mut p = Enjoy303s::default();
            if opening {
                p.decode(&input(&message(&[CONNECTED, 0x00])));
                p.follow_up(0);
            }
            p.encode_lights(&[(1, Light::Ready), (2, Light::Ready), (3, Light::Wrong)]);
            assert_eq!(p.decode(&input(&press_message(4, 0x10, OFF, 300))), [Press::timed(5, Button::A, 300)], "{opening}");
            assert_eq!(p.kind(), ProtocolKind::Enjoy303s { answered: true });
            assert_eq!(serial(&p.follow_up(0)), [START.to_vec(), all(OFF), all(READY), paint(2, WRONG)], "{opening}");
            assert!(p.follow_up(60_000).is_empty(), "nothing more to ask once open");
        }
    }

    #[test]
    fn asks_again_while_the_answer_to_a_set_up_step_is_overdue() {
        let mut p = Enjoy303s::default();
        assert!(p.follow_up(1_000).is_empty(), "the connect order just went");
        assert!(p.follow_up(1_499).is_empty());
        assert_eq!(p.follow_up(1_500), p.init_reports(), "the cable's settings and the connect order again");
        assert!(p.follow_up(1_600).is_empty());
        p.decode(&input(&message(&[CONNECTED, 0x00])));
        assert_eq!(serial(&p.follow_up(1_700)), [TAKE_64.to_vec(), OPEN.to_vec()]);
        assert!(p.follow_up(2_199).is_empty());
        assert_eq!(serial(&p.follow_up(2_200)), [TAKE_64.to_vec(), OPEN.to_vec()]);
        p.decode(&input(&message(&[OPENED, 0x01])));
        assert_eq!(serial(&p.follow_up(2_300)), [START.to_vec(), all(OFF)]);
        assert!(p.follow_up(60_000).is_empty(), "nothing more to ask once open");
    }

    #[test]
    fn lights_and_mode_set_before_the_receiver_opened_follow_its_answer() {
        let mut p = Enjoy303s::default();
        assert!(p.encode_lights(&[(1, Light::Ready), (2, Light::Ready), (3, Light::Wrong)]).is_empty());
        assert!(p.encode_mode(BuzzerMode::Answers).is_empty());
        p.decode(&input(&message(&[CONNECTED, 0x00])));
        p.follow_up(0);
        p.decode(&input(&message(&[OPENED, 0x01])));
        // The receiver takes presses before the buzzers light ready.
        assert_eq!(serial(&p.follow_up(0)), [START.to_vec(), all(OFF), all(READY), paint(2, WRONG)]);
    }

    #[test]
    fn a_reset_lighting_two_buzzers_ready_lights_them_all_at_once() {
        let (mut p, _) = opened();
        p.encode_lights(&[(1, Light::Ready), (2, Light::Locked)]);
        // Their timers start together, so the order to all reaches the
        // buzzers not named too, as in any round.
        assert_eq!(serial(&p.reset_lights(&[(1, Light::Ready), (2, Light::Ready)])), [all(OFF), all(READY)]);
        assert_eq!(serial(&p.reset_lights(&[(1, Light::Ready), (2, Light::Ready), (3, Light::Locked)])), [all(OFF), all(READY), paint(2, LOCKED)]);
    }

    #[test]
    fn a_reset_darkens_every_buzzer_but_those_it_lights() {
        let (mut p, _) = opened();
        p.encode_lights(&[(1, Light::Ready), (2, Light::Ready), (3, Light::Locked)]);
        // Another show begins: buzzer 5 is a team in it, every other buzzer
        // goes dark, whatever it showed.
        assert_eq!(serial(&p.reset_lights(&[(5, Light::Ready)])), [all(OFF), paint(4, READY)]);
        assert!(p.encode_lights(&[(5, Light::Ready)]).is_empty(), "shown already");
        assert_eq!(serial(&p.reset_lights(&[])), [all(OFF)], "no show");

        // Before the receiver opened: kept, and the buzzers it no longer
        // lights are dark once it does.
        let mut p = Enjoy303s::default();
        p.encode_lights(&[(1, Light::Ready), (2, Light::Ready)]);
        assert!(p.reset_lights(&[(5, Light::Ready)]).is_empty());
        p.decode(&input(&message(&[CONNECTED, 0x00])));
        p.follow_up(0);
        p.decode(&input(&message(&[OPENED, 0x01])));
        assert_eq!(serial(&p.follow_up(0)), [START.to_vec(), all(OFF), paint(4, READY)]);
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
        // Two buzzers: one order each costs no more than one for all and one more.
        assert_eq!(serial(&p.encode_lights(&[(1, Light::Wrong), (2, Light::Locked)])), [paint(0, WRONG), paint(1, LOCKED)]);
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
        let buzzed: Vec<_> = [(1, Light::First)].into_iter().chain((2..=6).map(|b| (b, Light::Locked))).collect();
        p.encode_lights(&buzzed);
        // Three out of the round: one order each would cost fewer orders but
        // start the timers of buzzers 2 and 3 a write apart.
        let on = [(1, Light::Wrong), (2, Light::Ready), (3, Light::Ready)];
        assert_eq!(serial(&p.encode_lights(&on)), [all(READY), paint(0, WRONG), paint(3, LOCKED), paint(4, LOCKED), paint(5, LOCKED)]);
        // Buzzer 2 buzzes and is judged wrong: buzzer 3 alone turns ready.
        assert_eq!(serial(&p.encode_lights(&[(2, Light::First), (3, Light::Locked)])), [paint(1, FIRST), paint(2, LOCKED)]);
        assert_eq!(serial(&p.encode_lights(&[(2, Light::Wrong), (3, Light::Ready)])), [paint(1, WRONG), paint(2, READY)]);
    }

    #[test]
    fn no_colour_goes_to_all_while_a_buzzer_should_show_ready() {
        let (mut p, _) = opened();
        let round: Vec<_> = (1..=3).map(|b| (b, Light::Ready)).chain([(4, Light::Locked)]).collect();
        assert_eq!(serial(&p.encode_lights(&round)), [all(READY), paint(3, LOCKED)]);
        // Others locked out at once: an order to all would light the ready
        // buzzers again one by one.
        let locked: Vec<_> = (5..=9).map(|b| (b, Light::Locked)).collect();
        assert_eq!(serial(&p.encode_lights(&locked)), (4..=8).map(|i| paint(i, LOCKED)).collect::<Vec<_>>());
    }

    #[test]
    fn the_receiver_takes_presses_from_its_opening_on_whatever_the_mode() {
        let (mut p, written) = opened();
        assert_eq!(written, [TAKE_64.to_vec(), OPEN.to_vec(), START.to_vec(), all(OFF)]);
        let modes = [BuzzerMode::Register, BuzzerMode::Answers, BuzzerMode::Off, BuzzerMode::FastestFinger, BuzzerMode::Buzzed, BuzzerMode::Results];
        for mode in modes {
            assert!(p.encode_mode(mode).is_empty(), "{mode:?}");
        }
    }

    #[test]
    fn the_colours_differ_by_more_than_brightness() {
        let colours = [OFF, READY, LOCKED, FIRST, CORRECT, WRONG];
        for (i, a) in colours.iter().enumerate() {
            for b in &colours[i + 1..] {
                assert_ne!(rgb(*a), rgb(*b), "{a:#04x} and {b:#04x}");
            }
        }
        assert!(!NOT_READY.iter().any(|&c| rgb(c) == rgb(READY)));
    }

    #[test]
    fn reads_presses_with_the_buzzers_timer() {
        let (mut p, _) = opened();
        p.encode_mode(BuzzerMode::Answers);
        // Pinned: buzzer 3 (radio ID 2) presses A, showing ready, 500 after its light changed.
        let frame = [0x27, 0xa5, 0x0a, 0x32, 0x02, 0x10, 0x43, 0xf4, 0x01, 0x00, 0x00, 0x00, 0x00, 0x96];
        assert_eq!(press_message(2, 0x10, READY, 500), frame);
        assert_eq!(p.decode(&input(&frame)), [Press::timed(3, Button::A, 500)]);
        // The low four bits read `c` on every press in the hardware test.
        let codes = [(0x2c, Button::B), (0x3c, Button::C), (0x4f, Button::D), (0x50, Button::Big), (0x5c, Button::Big)];
        for (code, button) in codes {
            assert_eq!(p.decode(&input(&press_message(63, code, READY, 0x01_02_03))), [Press::timed(64, button, 0x01_02_03)]);
        }
        for code in [0x00, 0x60, 0x70, 0xf0] {
            assert!(p.decode(&input(&press_message(0, code, READY, 1))).is_empty(), "no button {code:#04x}");
        }
        assert!(p.decode(&input(&press_message(64, 0x10, READY, 1))).is_empty(), "no buzzer 65");
        // A message too short to hold the colour and the whole timer is no press.
        assert!(p.decode(&input(&message(&[PRESS, 0, 0x10, READY, 1, 0]))).is_empty());
        assert_eq!(p.decode(&input(&message(&[PRESS, 0, 0x10, READY, 1, 0, 0]))), [Press::timed(1, Button::A, 1)]);
        // Quizmaster's remote and set-ID answers are not presses.
        assert!(p.decode(&input(&message(&[0x34, 0x05]))).is_empty());
        assert!(p.decode(&input(&message(&[0x17, 0x01]))).is_empty());
    }

    #[test]
    fn in_fastest_finger_only_a_buzzer_that_was_ready_presses() {
        let (mut p, _) = opened();
        for mode in [BuzzerMode::FastestFinger, BuzzerMode::Buzzed] {
            p.encode_mode(mode);
            // Brightness and the top bit aside, and a colour the app never sets.
            for shown in [READY, READY | 0x80, rgb(READY), colour(true, 2, 2, 2)] {
                assert_eq!(p.decode(&input(&press_message(0, 0x50, shown, 300))), [Press::timed(1, Button::Big, 300)], "{mode:?} {shown:#04x}");
            }
            assert!(p.take_dropped().is_empty());
            for shown in [OFF, LOCKED, LOCKED | 0x40, FIRST, CORRECT, WRONG] {
                assert!(p.decode(&input(&press_message(1, 0x50, shown, 300))).is_empty(), "{mode:?} {shown:#04x}");
                assert_eq!(p.take_dropped(), [Press::timed(2, Button::Big, 300)], "logged");
            }
        }
        // Elsewhere the game decides: a switched-off player is ignored, a
        // new buzzer registers.
        for mode in [BuzzerMode::Register, BuzzerMode::Answers, BuzzerMode::Off, BuzzerMode::Results] {
            p.encode_mode(mode);
            assert_eq!(p.decode(&input(&press_message(2, 0x10, OFF, 90_000))), [Press::timed(3, Button::A, 90_000)], "{mode:?}");
            assert_eq!(p.decode(&input(&press_message(3, 0x10, LOCKED, 900))), [Press::timed(4, Button::A, 900)], "{mode:?}");
        }
        assert!(p.take_dropped().is_empty());
    }

    #[test]
    fn messages_split_over_reports_or_spoiled_are_read_once_whole() {
        let mut p = Enjoy303s::default();
        let a = press_message(0, 0x10, READY, 1);
        let b = press_message(1, 0x20, READY, 2);
        let stream = [&a[..], &[0x00, 0x27], &b[..]].concat();
        let (first, rest) = stream.split_at(5);
        assert!(p.decode(&input(first)).is_empty());
        assert_eq!(p.decode(&input(rest)), [Press::timed(1, Button::A, 1), Press::timed(2, Button::B, 2)]);

        // A wrong check byte drops the message.
        let mut spoiled = a.clone();
        *spoiled.last_mut().unwrap() ^= 0xff;
        assert!(p.decode(&input(&spoiled)).is_empty());
        // A header spoiled into a long length holds up no press behind it.
        let held_up = [&[0x27, 0xa5, 0x3c, 0x32][..], &b].concat();
        assert_eq!(p.decode(&input(&held_up)), [Press::timed(2, Button::B, 2)]);
        assert_eq!(p.decode(&input(&a)), [Press::timed(1, Button::A, 1)], "lined up again");
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
