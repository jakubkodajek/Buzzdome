//! Receiver protocols: raw HID reports in, presses out; lights and the
//! buzzer mode in, reports out.

use crate::hid::{DeviceInfo, Report};
use crate::learned::{LearnedMapping, LearnedProtocol};
use crate::rf215::Rf215;
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
    /// Enjoy RF215 dome buzzers (see `rf215`). `answered` once the receiver
    /// answered the connect command.
    Rf215 { answered: bool },
}

pub trait Protocol: Send {
    fn kind(&self) -> ProtocolKind;

    /// Decodes one input report. Several presses in one report must be returned
    /// in the order the receiver registered them (earliest first), so the app
    /// never has to re-order fastest-finger presses.
    fn decode(&mut self, report: &[u8]) -> Vec<Press>;

    /// Reports that set the receiver up, sent once after the device is opened.
    fn init_reports(&self) -> Vec<Report> {
        Vec::new()
    }

    /// Whether the receiver takes commands after `init_reports`. Until it
    /// does, the receiver thread holds them back, for a while at most
    /// (`ReceiverConfig::ready_timeout`).
    fn ready(&self) -> bool {
        true
    }

    /// Reports that set the lights of these buzzers, whose lights changed.
    /// Empty when unsupported.
    fn encode_lights(&self, _lights: &[(BuzzerId, Light)]) -> Vec<Report> {
        Vec::new()
    }

    /// Reports that put the buzzers in `mode`, which changed: for receivers
    /// that run each round themselves.
    fn encode_mode(&self, _mode: BuzzerMode) -> Vec<Report> {
        Vec::new()
    }

    /// Reports that set the round in `mode` going again after the app
    /// ignored `press`: for receivers that lock the other buzzers by
    /// themselves when one presses, though the app took no buzz.
    fn rearm(&self, _press: Press, _mode: BuzzerMode) -> Vec<Report> {
        Vec::new()
    }

    fn has_lights(&self) -> bool {
        false
    }
}

/// Picks the protocol for a device. Called on every (re)connect, so changes such
/// as newly learned buttons apply without restarting.
pub type ProtocolResolver = dyn Fn(&DeviceInfo) -> Box<dyn Protocol> + Send + Sync;

/// The protocol for `device`: its vendor's when it is known, else the
/// buttons learned for its model, else raw reports only.
pub fn for_device(learned: &[LearnedMapping], device: &DeviceInfo) -> Box<dyn Protocol> {
    if Rf215::handles(device) {
        return Box::new(Rf215::default());
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
}

pub fn format_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}
