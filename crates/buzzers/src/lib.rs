//! Buzzer drivers. Presses flow into the app over a channel in hardware order;
//! lights flow out through `BuzzerDriver::set_lights`.
//!
//! The hardware path is layered so each part can be tested without a device:
//!
//! - [`hid`]: a minimal USB-HID backend trait, implemented with `hidapi` in
//!   production and with fakes in tests.
//! - [`protocol`]: turns raw input reports into presses, and the buzzer mode
//!   and lights into reports. Vendor protocols plug in here ([`enjoy303s`],
//!   over the [`cp2110`] bridge); for other receivers, [`learned`] maps
//!   reports recorded from real presses.
//! - [`receiver`]: owns one receiver on an I/O thread: connect, read, decode,
//!   order fastest-finger presses by the buzzers' own timers ([`timer_order`]),
//!   write the mode and lights, reconnect after unplugging.
//!
//! See `docs/design.md` and `replica/hardware.md`.

pub mod clock;
pub mod cp2110;
pub mod edge;
pub mod enjoy303s;
pub mod hid;
#[cfg(feature = "hidapi")]
pub mod hidapi_backend;
pub mod learned;
pub mod protocol;
pub mod receiver;
pub mod timer_order;

use quiz_core::{BuzzerId, BuzzerMode, Button, Light};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Press {
    pub buzzer: BuzzerId,
    pub button: Button,
    /// The buzzer's own count from the moment its light last changed to the
    /// press, for receivers that report one (`enjoy303s`), in the vendor's
    /// unit. Fastest-finger presses are ordered by it (`timer_order`).
    #[serde(default)]
    pub timer: Option<u32>,
}

impl Press {
    /// A press with no timer of the buzzer's own.
    pub fn new(buzzer: BuzzerId, button: Button) -> Press {
        Press { buzzer, button, timer: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DriverError {
    #[error("receiver not connected")]
    NotConnected,
    #[error("receiver error: {0}")]
    Device(String),
}

pub trait BuzzerDriver: Send {
    fn name(&self) -> &'static str;

    /// Whether the driver can show `Light` states on the buzzers.
    fn has_lights(&self) -> bool;

    fn set_lights(&mut self, lights: &[(BuzzerId, Light)]) -> Result<(), DriverError>;

    /// Tells the receiver what the buzzers take now: when presses count,
    /// and when they are ordered by the buzzers' own timers.
    fn set_mode(&mut self, mode: BuzzerMode);
}

/// Keyboard and on-screen buzzers. Presses are injected by the UI; there are
/// no lights to set.
#[derive(Debug, Default)]
pub struct VirtualDriver;

impl BuzzerDriver for VirtualDriver {
    fn name(&self) -> &'static str {
        "virtual"
    }

    fn has_lights(&self) -> bool {
        false
    }

    fn set_lights(&mut self, _lights: &[(BuzzerId, Light)]) -> Result<(), DriverError> {
        Ok(())
    }

    fn set_mode(&mut self, _mode: BuzzerMode) {}
}
