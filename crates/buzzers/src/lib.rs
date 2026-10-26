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
//! - [`fake_receiver`]: a 303S receiver that end-to-end tests plug in and
//!   press over TCP, in the app's debug builds.
//!
//! See `docs/design.md` and `replica/hardware.md`.

pub mod clock;
pub mod cp2110;
pub mod edge;
pub mod enjoy303s;
pub mod fake_receiver;
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

    /// A press with the buzzer's own `timer`.
    pub fn timed(buzzer: BuzzerId, button: Button, timer: u32) -> Press {
        Press { buzzer, button, timer: Some(timer) }
    }
}

/// A key of the quizmaster's remote, named after what is printed on it.
/// The Enjoy 303S receiver hears the Galaxy set's 12-key remote
/// (`enjoy303s`); what each key does in a show is the app's choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum RemoteKey {
    /// The arrow pointing back.
    Back,
    /// The arrow pointing forward.
    Forward,
    Power,
    Grid,
    /// The quiz key, a target.
    Target,
    PlayPause,
    /// The forward arrow in the middle.
    Centre,
    /// The question mark.
    Question,
    Trophy,
    ColourBars,
    /// The red cross.
    Cross,
    /// The green tick.
    Tick,
}

impl RemoteKey {
    pub const ALL: [RemoteKey; 12] = [
        RemoteKey::Back,
        RemoteKey::Forward,
        RemoteKey::Power,
        RemoteKey::Grid,
        RemoteKey::Target,
        RemoteKey::PlayPause,
        RemoteKey::Centre,
        RemoteKey::Question,
        RemoteKey::Trophy,
        RemoteKey::ColourBars,
        RemoteKey::Cross,
        RemoteKey::Tick,
    ];
}

/// Stops compiling when a key is added: add it to `RemoteKey::ALL` too, then here.
const _: fn(RemoteKey) = |key| match key {
    RemoteKey::Back
    | RemoteKey::Forward
    | RemoteKey::Power
    | RemoteKey::Grid
    | RemoteKey::Target
    | RemoteKey::PlayPause
    | RemoteKey::Centre
    | RemoteKey::Question
    | RemoteKey::Trophy
    | RemoteKey::ColourBars
    | RemoteKey::Cross
    | RemoteKey::Tick => {}
};

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

    /// Darkens every buzzer, whatever it showed before (another show's
    /// lights, or those another program left), and sets these buzzers'
    /// lights. A light then sent to all at once, such as ready for two or
    /// more, reaches the buzzers not named too. By default it only sets
    /// these lights, so a driver with lights must override it.
    fn reset_lights(&mut self, lights: &[(BuzzerId, Light)]) -> Result<(), DriverError> {
        self.set_lights(lights)
    }

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
