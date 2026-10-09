//! Buzzer drivers. Presses flow into the app over a channel in hardware order;
//! lights flow out through `BuzzerDriver::set_lights`.
//!
//! The hardware path is layered so each part can be tested without a device:
//!
//! - [`hid`]: a minimal USB-HID backend trait, implemented with `hidapi` in
//!   production and with fakes in tests.
//! - [`protocol`]: turns raw input reports into presses and light commands into
//!   output reports. Vendor protocols plug in here; until the receiver's protocol
//!   is documented, [`learned`] maps reports recorded from real presses.
//! - [`receiver`]: owns one receiver on an I/O thread: connect, read, decode,
//!   write lights, reconnect after unplugging.
//!
//! See `docs/design.md` and `replica/hardware.md`.

pub mod edge;
pub mod hid;
#[cfg(feature = "hidapi")]
pub mod hidapi_backend;
pub mod learned;
pub mod protocol;
pub mod receiver;

use quiz_core::{BuzzerId, Button, Light};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Press {
    pub buzzer: BuzzerId,
    pub button: Button,
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
}
