//! Receiver protocols: raw HID reports in, presses out; lights in, reports out.

use crate::hid::DeviceInfo;
use crate::Press;
use quiz_core::{BuzzerId, Light};
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
}

pub trait Protocol: Send {
    fn kind(&self) -> ProtocolKind;

    /// Decodes one input report. Several presses in one report must be returned
    /// in the order the receiver registered them (earliest first), so the app
    /// never has to re-order fastest-finger presses.
    fn decode(&mut self, report: &[u8]) -> Vec<Press>;

    /// Output reports to send once after the device is opened.
    fn init_reports(&self) -> Vec<Vec<u8>> {
        Vec::new()
    }

    /// Output reports that set buzzer lights. Empty when unsupported.
    fn encode_lights(&self, _lights: &[(BuzzerId, Light)]) -> Vec<Vec<u8>> {
        Vec::new()
    }

    fn has_lights(&self) -> bool {
        false
    }
}

/// Picks the protocol for a device. Called on every (re)connect, so changes such
/// as newly learned buttons apply without restarting.
pub type ProtocolResolver = dyn Fn(&DeviceInfo) -> Box<dyn Protocol> + Send + Sync;

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
