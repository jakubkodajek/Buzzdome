//! Receiver protocols: raw HID reports in, presses out; lights and the
//! buzzer mode in, reports out.

use crate::enjoy303s::Enjoy303s;
use crate::hid::{DeviceInfo, Report};
use crate::learned::{LearnedMapping, LearnedProtocol};
use crate::Press;
use quiz_core::{BuzzerId, BuzzerMode, Light};
use serde::Serialize;
use ts_rs::TS;

/// Which protocol reads the receiver, for the Director (which words it).
/// Vendor protocols add a variant each.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ProtocolKind {
    /// Reports are shown but not decoded.
    RawOnly,
    /// Buttons learned from recorded presses (see `learned`).
    Learned { buttons: usize },
    /// Enjoy 303S RGB buzzers (see `enjoy303s`). `answered` once the
    /// receiver answered its set-up.
    Enjoy303s { answered: bool },
}

pub trait Protocol: Send {
    fn kind(&self) -> ProtocolKind;

    /// Decodes one input report. Several presses in one report must be returned
    /// in the order the receiver registered them (earliest first), so the app
    /// never re-orders presses by its own clock; presses carrying the
    /// buzzer's own timer are ordered by it in fastest finger (`timer_order`).
    fn decode(&mut self, report: &[u8]) -> Vec<Press>;

    /// The presses `decode` read but did not pass on, as ones that cannot
    /// count now, since this was last asked: for the raw log only.
    fn take_dropped(&mut self) -> Vec<Press> {
        Vec::new()
    }

    /// Reports that set the receiver up, sent once after the device is opened.
    fn init_reports(&self) -> Vec<Report> {
        Vec::new()
    }

    /// Reports to write now, taken after every read, whether a report came
    /// or the read timed out, at `now_ms` on the show clock: what the
    /// receiver's messages call for, as the next step of its set-up and the
    /// lights and mode that waited for it, or a step again whose answer is
    /// overdue. Written after the reports already waiting.
    fn follow_up(&mut self, _now_ms: u64) -> Vec<Report> {
        Vec::new()
    }

    /// Reports that set the lights of these buzzers, whose lights changed.
    /// Empty when unsupported.
    fn encode_lights(&mut self, _lights: &[(BuzzerId, Light)]) -> Vec<Report> {
        Vec::new()
    }

    /// Reports that light these buzzers and every other one dark, whatever
    /// they showed before (`BuzzerDriver::reset_lights`). These buzzers'
    /// lights only, by default.
    fn reset_lights(&mut self, lights: &[(BuzzerId, Light)]) -> Vec<Report> {
        self.encode_lights(lights)
    }

    /// Reports that tell the receiver `mode`, which changed: for receivers
    /// that take presses only while told to.
    fn encode_mode(&mut self, _mode: BuzzerMode) -> Vec<Report> {
        Vec::new()
    }

    fn has_lights(&self) -> bool {
        false
    }

    /// Whether buttons can be taught for this receiver (`learned`): only
    /// while the app does not know its vendor's protocol.
    fn learnable(&self) -> bool {
        false
    }
}

/// Picks the protocol for a device. Called on every (re)connect, so changes such
/// as newly learned buttons apply without restarting.
pub type ProtocolResolver = dyn Fn(&DeviceInfo) -> Box<dyn Protocol> + Send + Sync;

/// The protocol for `device`: its vendor's when it is known, else the
/// buttons learned for its model, else raw reports only.
pub fn for_device(learned: &[LearnedMapping], device: &DeviceInfo) -> Box<dyn Protocol> {
    if Enjoy303s::handles(device) {
        return Box::new(Enjoy303s::default());
    }
    match learned.iter().find(|m| m.for_device(device) && !m.signatures.is_empty()) {
        Some(mapping) => Box::new(LearnedProtocol::new(mapping.clone())),
        None => Box::new(RawOnly),
    }
}

/// Used when no protocol knows the device: decodes nothing, so the raw reports
/// can still be inspected and learned.
#[derive(Debug, Default)]
pub struct RawOnly;

impl Protocol for RawOnly {
    fn kind(&self) -> ProtocolKind {
        ProtocolKind::RawOnly
    }

    fn decode(&mut self, _report: &[u8]) -> Vec<Press> {
        Vec::new()
    }

    fn learnable(&self) -> bool {
        true
    }
}

pub fn format_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}
