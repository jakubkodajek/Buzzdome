//! Enjoy RF215 dome buzzers, through the REC-A receiver and its USB-HID
//! cable. Not yet checked on a real receiver: `docs/hardware-test.md` says
//! what to check, and `docs/design.md` (2.4) where this description comes
//! from.
//!
//! The cable is a CP2110-type bridge (`cp2110`) to the receiver's serial
//! line, 115200 baud 8N1. Every message, either way, is three bytes: two
//! bytes and their XOR. The receiver sends presses by itself.
//!
//! The receiver runs each round itself: in fastest finger it locks the
//! other buzzers out and lights the first one. So the app tells it the
//! round (`BuzzerMode`) and the results, right or wrong, rather than each
//! buzzer's light.

use crate::cp2110;
use crate::hid::{DeviceInfo, Report};
use crate::protocol::{Protocol, ProtocolKind};
use crate::Press;
use quiz_core::{BuzzerId, BuzzerMode, Button, Light};

pub const VENDOR_ID: u16 = 0x10c4;
pub const PRODUCT_ID: u16 = 0x1819;
const BAUD: u32 = 115_200;
/// The highest buzzer ID a receiver takes, which it is told on connect.
const MAX_BUZZER: u8 = 100;
/// The first byte of the receiver's answer to the connect command.
const CONNECTED: u8 = 0x7e;

/// What the app tells the receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Order {
    /// Take buzzers 1 to `MAX_BUZZER`.
    Connect,
    /// End the round: the buzzers go dark.
    Stop,
    /// A new fastest-finger round with the big button.
    Rush,
    /// The fastest-finger round goes on: the buzzers that buzzed stay out.
    RushOn,
    /// Every buzzer answers A to D.
    AllAnswer,
    Right(BuzzerId),
    Wrong(BuzzerId),
}

impl Order {
    /// The two bytes of the order; none for a buzzer the order cannot name.
    fn bytes(self) -> Option<[u8; 2]> {
        let buzzer = |code: u8, buzzer: BuzzerId| u8::try_from(buzzer).ok().filter(|b| *b < 0x80).map(|b| [code, 0x80 | b]);
        match self {
            Order::Connect => Some([0x01, 0x80 | MAX_BUZZER]),
            Order::Stop => Some([0x03, 0x85]),
            Order::Rush => Some([0x03, 0x83]),
            Order::RushOn => Some([0x03, 0x86]),
            Order::AllAnswer => Some([0x03, 0x8a]),
            Order::Right(b) => buzzer(0x06, b),
            Order::Wrong(b) => buzzer(0x07, b),
        }
    }

    fn reports(self) -> Vec<Report> {
        match self.bytes() {
            Some([a, b]) => cp2110::send(&[a, b, a ^ b]),
            None => Vec::new(),
        }
    }
}

/// The press a message from the receiver stands for, if any. The two
/// forms of a press are those the vendor's SDK reads.
fn press_in(d0: u8, d1: u8) -> Option<Press> {
    let kind = (d0 >> 5) & 0x03;
    let high = d0 & 0x1f;
    let code = (d1 >> 4) & 0x07;
    let low = d1 & 0x0f;
    match kind {
        // The older form: the button in the first byte, a buzzer ID up to
        // 15 in the second. 0 is a key of the quizmaster's remote.
        0 => {
            let button = match high {
                1 => Button::Big,
                2 => Button::A,
                3 => Button::B,
                4 => Button::C,
                5 => Button::D,
                _ => return None,
            };
            Some(Press { buzzer: BuzzerId::from(low), button })
        }
        // The button in the second byte; 4 and 5 are E and F, which a dome
        // buzzer does not have.
        1 => {
            let button = match code {
                0 => Button::A,
                1 => Button::B,
                2 => Button::C,
                3 => Button::D,
                4 | 5 => return None,
                _ => Button::Big,
            };
            Some(Press { buzzer: buzzer_id(high, low), button })
        }
        _ => None,
    }
}

/// The buzzer ID in a press of the second form. Unverified: the SDK keeps
/// only `low` (IDs up to 15), though a receiver takes up to 100 buzzers and
/// the reply to setting an ID gives it as `high * 16 + low`, so that is
/// taken here. A receiver that numbers otherwise shows in the Receiver tab
/// as presses of the wrong buzzers above 15 (`docs/hardware-test.md`).
fn buzzer_id(high: u8, low: u8) -> BuzzerId {
    BuzzerId::from(high) * 16 + BuzzerId::from(low)
}

#[derive(Debug, Default)]
pub struct Rf215 {
    /// Bytes received and not yet read as a message.
    pending: Vec<u8>,
    /// The receiver answered the connect command.
    answered: bool,
}

impl Rf215 {
    pub fn handles(device: &DeviceInfo) -> bool {
        device.vendor_id == VENDOR_ID && device.product_id == PRODUCT_ID
    }
}

impl Protocol for Rf215 {
    fn kind(&self) -> ProtocolKind {
        ProtocolKind::Rf215 { answered: self.answered }
    }

    /// Reads the messages in `report`, with what earlier reports left of
    /// one. A message is valid when its first byte's top bit is clear and
    /// its third byte is the XOR of the other two; anything else is skipped
    /// a byte at a time until messages line up again.
    fn decode(&mut self, report: &[u8]) -> Vec<Press> {
        self.pending.extend_from_slice(cp2110::received(report));
        let mut presses = Vec::new();
        let mut at = 0;
        while let [d0, d1, d2, ..] = self.pending[at..] {
            if !self.answered && d0 == CONNECTED {
                self.answered = true;
                at += 3;
            } else if d0 & 0x80 == 0 && d2 == d0 ^ d1 {
                presses.extend(press_in(d0, d1));
                at += 3;
            } else {
                at += 1;
            }
        }
        self.pending.drain(..at);
        presses
    }

    fn init_reports(&self) -> Vec<Report> {
        let mut reports = cp2110::open_uart(BAUD);
        reports.extend(Order::Connect.reports());
        reports
    }

    fn ready(&self) -> bool {
        self.answered
    }

    /// Right and wrong. The other lights follow from the round.
    fn encode_lights(&self, lights: &[(BuzzerId, Light)]) -> Vec<Report> {
        lights
            .iter()
            .filter_map(|&(buzzer, light)| match light {
                Light::Correct => Some(Order::Right(buzzer)),
                Light::Wrong => Some(Order::Wrong(buzzer)),
                Light::Off | Light::Ready | Light::Locked | Light::First => None,
            })
            .flat_map(Order::reports)
            .collect()
    }

    /// A buzz and the results go on in the round the receiver runs, and
    /// the results come as lights.
    fn encode_mode(&self, mode: BuzzerMode) -> Vec<Report> {
        let order = match mode {
            BuzzerMode::Off => Order::Stop,
            BuzzerMode::Register | BuzzerMode::Answers => Order::AllAnswer,
            BuzzerMode::FastestFinger { again: false } => Order::Rush,
            BuzzerMode::FastestFinger { again: true } => Order::RushOn,
            BuzzerMode::Buzzed | BuzzerMode::Results => return Vec::new(),
        };
        order.reports()
    }

    /// A buzz the app did not take (from a player switched off, or just
    /// after time up) locked the other buzzers all the same, so the round
    /// goes on.
    fn rearm(&self, press: Press, mode: BuzzerMode) -> Vec<Report> {
        match (press.button, mode) {
            (Button::Big, BuzzerMode::FastestFinger { .. }) => Order::RushOn.reports(),
            _ => Vec::new(),
        }
    }

    fn has_lights(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The serial bytes the reports carry.
    fn serial(reports: &[Report]) -> Vec<Vec<u8>> {
        reports
            .iter()
            .map(|report| match report {
                Report::Output(data) => cp2110::received(data).to_vec(),
                Report::Feature(data) => panic!("feature report {data:?}"),
            })
            .collect()
    }

    /// An input report carrying `bytes`.
    fn input(bytes: &[u8]) -> Vec<u8> {
        let mut report = vec![bytes.len() as u8];
        report.extend_from_slice(bytes);
        report
    }

    fn press(buzzer: BuzzerId, button: Button) -> Press {
        Press { buzzer, button }
    }

    /// A press message of the second form: buzzer 0..=127, button code 0..=7.
    fn message(buzzer: u8, code: u8) -> [u8; 3] {
        let d0 = 0x20 | (buzzer >> 4);
        let d1 = (code << 4) | (buzzer & 0x0f);
        [d0, d1, d0 ^ d1]
    }

    #[test]
    fn opens_the_cable_then_asks_for_buzzers_1_to_100() {
        let init = Rf215::default().init_reports();
        assert_eq!(init[..3], cp2110::open_uart(115_200)[..]);
        assert_eq!(serial(&init[3..]), [vec![0x01, 0xe4, 0xe5]]);
    }

    #[test]
    fn is_ready_once_the_receiver_answers_the_connect() {
        let mut rf = Rf215::default();
        assert!(!rf.ready());
        assert_eq!(rf.kind(), ProtocolKind::Rf215 { answered: false });
        assert!(rf.decode(&input(&[0x7e, 0x12, 0x34])).is_empty());
        assert!(rf.ready());
        assert_eq!(rf.kind(), ProtocolKind::Rf215 { answered: true });
    }

    #[test]
    fn rounds_and_results_are_the_vendor_orders() {
        let rf = Rf215::default();
        let mode = |mode| serial(&rf.encode_mode(mode));
        assert_eq!(mode(BuzzerMode::Off), [vec![0x03, 0x85, 0x86]]);
        assert_eq!(mode(BuzzerMode::Register), [vec![0x03, 0x8a, 0x89]]);
        assert_eq!(mode(BuzzerMode::Answers), [vec![0x03, 0x8a, 0x89]]);
        assert_eq!(mode(BuzzerMode::FastestFinger { again: false }), [vec![0x03, 0x83, 0x80]]);
        assert_eq!(mode(BuzzerMode::FastestFinger { again: true }), [vec![0x03, 0x86, 0x85]]);
        assert!(mode(BuzzerMode::Buzzed).is_empty());
        assert!(mode(BuzzerMode::Results).is_empty());

        let lights = [(3, Light::Correct), (4, Light::Wrong), (5, Light::Ready), (6, Light::First), (200, Light::Correct)];
        assert_eq!(serial(&rf.encode_lights(&lights)), [vec![0x06, 0x83, 0x85], vec![0x07, 0x84, 0x83]]);
    }

    #[test]
    fn only_a_big_button_in_fastest_finger_sets_the_round_going_again() {
        let rf = Rf215::default();
        let ff = BuzzerMode::FastestFinger { again: false };
        assert_eq!(serial(&rf.rearm(press(2, Button::Big), ff)), [vec![0x03, 0x86, 0x85]]);
        assert!(rf.rearm(press(2, Button::A), ff).is_empty());
        assert!(rf.rearm(press(2, Button::Big), BuzzerMode::Buzzed).is_empty());
        assert!(rf.rearm(press(2, Button::Big), BuzzerMode::Answers).is_empty());
    }

    #[test]
    fn reads_both_forms_of_a_press() {
        let mut rf = Rf215::default();
        let mut bytes = Vec::new();
        for code in [0, 1, 2, 3, 6, 7] {
            bytes.extend(message(7, code));
        }
        let buttons = [Button::A, Button::B, Button::C, Button::D, Button::Big, Button::Big];
        let expected: Vec<Press> = buttons.iter().map(|b| press(7, *b)).collect();
        assert_eq!(rf.decode(&input(&bytes)), expected);

        // The older form: button in the first byte, buzzer in the second.
        let older = |button: u8, buzzer: u8| [button, buzzer, button ^ buzzer];
        let mut bytes = Vec::new();
        for button in 1..=5 {
            bytes.extend(older(button, 9));
        }
        bytes.extend(older(0, 2)); // a key of the remote
        let buttons = [Button::Big, Button::A, Button::B, Button::C, Button::D];
        let expected: Vec<Press> = buttons.iter().map(|b| press(9, *b)).collect();
        assert_eq!(rf.decode(&input(&bytes)), expected);
    }

    #[test]
    fn buzzers_above_15_carry_the_high_part_of_their_id_in_the_first_byte() {
        let mut rf = Rf215::default();
        let bytes: Vec<u8> = [message(15, 6), message(16, 6), message(100, 0)].concat();
        assert_eq!(rf.decode(&input(&bytes)), [press(15, Button::Big), press(16, Button::Big), press(100, Button::A)]);
    }

    #[test]
    fn e_and_f_and_other_messages_are_no_press() {
        let mut rf = Rf215::default();
        let set_id = [0x40 | 1, 0x04, 0x45];
        let bytes: Vec<u8> = [message(3, 4), message(3, 5), set_id].concat();
        assert!(rf.decode(&input(&bytes)).is_empty());
    }

    #[test]
    fn messages_split_over_reports_or_after_noise_are_read_in_order() {
        let mut rf = Rf215::default();
        let [a0, a1, a2] = message(1, 6);
        let [b0, b1, b2] = message(2, 6);
        assert!(rf.decode(&input(&[a0])).is_empty());
        assert_eq!(rf.decode(&input(&[a1, a2, b0, b1])), [press(1, Button::Big)]);
        assert_eq!(rf.decode(&input(&[b2])), [press(2, Button::Big)]);

        // A byte with the top bit set, then a broken checksum: skipped.
        let mut bytes = vec![0x95, a0, a1, a2 ^ 0x01];
        bytes.extend(message(4, 0));
        assert_eq!(rf.decode(&input(&bytes)), [press(4, Button::A)]);
        // Nothing is left over to grow without end.
        assert!(rf.pending.len() < 3);
    }

    #[test]
    fn handles_only_its_own_cable() {
        let device = |vendor_id, product_id| DeviceInfo {
            path: "p".into(),
            vendor_id,
            product_id,
            manufacturer: None,
            product: None,
            serial_number: None,
            usage_page: 0xff00,
            usage: 1,
            interface_number: 0,
        };
        assert!(Rf215::handles(&device(0x10c4, 0x1819)));
        assert!(!Rf215::handles(&device(0x10c4, 0xea80)));
    }
}
