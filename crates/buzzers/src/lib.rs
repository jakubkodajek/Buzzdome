//! Buzzer drivers. Presses flow into the app over a channel in hardware order;
//! lights flow out through `BuzzerDriver::set_lights`.
//!
//! Hardware receivers are not implemented yet: see `replica/hardware.md`.

use quiz_core::{BuzzerId, Button, Light};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Press {
    pub buzzer: BuzzerId,
    pub button: Button,
}

#[derive(Debug, thiserror::Error)]
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
