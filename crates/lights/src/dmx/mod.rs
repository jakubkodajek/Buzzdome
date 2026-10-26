//! DMX512 outputs. A universe is 512 channels of 0 to 255; each output
//! sends a whole universe as a frame, and the lights thread sends one at a
//! steady rate (`DmxOutput::frame_every`), as fixtures expect.
//!
//! Every frame format is built by a pure function and tested byte for byte;
//! the outputs only move those bytes.

pub mod artnet;
pub mod enttec;
pub mod sacn;
#[cfg(feature = "devices")]
pub mod serial;
pub mod soundswitch;
#[cfg(feature = "devices")]
pub mod usb;

use crate::setup::OutputChoice;
use serde::Serialize;
use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;
use ts_rs::TS;

/// Channels in a universe.
pub const UNIVERSE_SIZE: usize = 512;

/// How often most outputs send a frame: 40 frames a second.
pub const FRAME_EVERY: Duration = Duration::from_millis(25);

/// The 512 channel values of a DMX universe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Universe([u8; UNIVERSE_SIZE]);

impl Default for Universe {
    fn default() -> Universe {
        Universe([0; UNIVERSE_SIZE])
    }
}

impl Universe {
    /// The value of the channel at `address`, from 1; 0 outside the universe.
    pub fn get(&self, address: usize) -> u8 {
        address.checked_sub(1).and_then(|i| self.0.get(i)).copied().unwrap_or(0)
    }

    /// Sets the channel at `address`, from 1. One outside the universe is
    /// left out: setups are checked, so this never happens in a show.
    pub fn set(&mut self, address: usize, value: u8) {
        if let Some(slot) = address.checked_sub(1).and_then(|i| self.0.get_mut(i)) {
            *slot = value;
        }
    }

    pub fn slots(&self) -> &[u8; UNIVERSE_SIZE] {
        &self.0
    }

    /// Each channel `progress` (0 to 1) of the way from `self` to `to`.
    pub fn blend(&self, to: &Universe, progress: f32) -> Universe {
        let progress = progress.clamp(0.0, 1.0);
        let mut blended = Universe::default();
        for (slot, (&from, &to)) in blended.0.iter_mut().zip(self.0.iter().zip(to.0.iter())) {
            *slot = (f32::from(from) + (f32::from(to) - f32::from(from)) * progress).round() as u8;
        }
        blended
    }
}

/// Why an output could not be opened or send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[ts(export)]
#[serde(tag = "problem", rename_all = "camelCase")]
pub enum OutputError {
    /// The interface is not plugged in, or another program holds it.
    #[error("not found or in use: {detail}")]
    NotAvailable { detail: String },
    /// The host name does not resolve.
    #[error("unknown host {host}")]
    UnknownHost { host: String },
    /// Writing to the interface or the network failed.
    #[error("send failed: {detail}")]
    SendFailed { detail: String },
    /// This build of the app cannot drive it.
    #[error("not supported in this build")]
    NotSupported,
}

/// A way to send universes to the lights.
pub trait DmxOutput: Send {
    /// The interface's name, for the Director.
    fn name(&self) -> String;

    /// How often it wants a frame.
    fn frame_every(&self) -> Duration {
        FRAME_EVERY
    }

    /// Sends one frame. An error means the output is lost; the lights thread
    /// opens it again a while later.
    fn send(&mut self, universe: &Universe) -> Result<(), OutputError>;
}

/// Opens the output `choice` names; none for `OutputChoice::None`.
pub fn open(choice: &OutputChoice) -> Result<Option<Box<dyn DmxOutput>>, OutputError> {
    Ok(Some(match choice {
        OutputChoice::None => return Ok(None),
        OutputChoice::ArtNet { host, universe } => Box::new(artnet::ArtNet::open(host, *universe)?),
        OutputChoice::Sacn { universe, host } => Box::new(sacn::Sacn::open(*universe, host.as_deref())?),
        #[cfg(feature = "devices")]
        OutputChoice::EnttecPro { port } => Box::new(serial::EnttecProPort::open(port)?),
        #[cfg(feature = "devices")]
        OutputChoice::OpenDmx { port } => Box::new(serial::OpenDmxPort::open(port)?),
        #[cfg(feature = "devices")]
        OutputChoice::SoundSwitch => Box::new(usb::SoundSwitchMicro::open()?),
        #[cfg(not(feature = "devices"))]
        OutputChoice::EnttecPro { .. } | OutputChoice::OpenDmx { .. } | OutputChoice::SoundSwitch => return Err(OutputError::NotSupported),
    }))
}

/// A UDP socket that sends to `host` (a name or an address) at `port`.
/// Broadcast is allowed, for Art-Net's broadcast addresses.
pub(crate) fn udp_to(host: &str, port: u16) -> Result<(UdpSocket, SocketAddr), OutputError> {
    let unknown = || OutputError::UnknownHost { host: host.to_owned() };
    let to = (host.trim(), port).to_socket_addrs().map_err(|_| unknown())?.find(SocketAddr::is_ipv4).ok_or_else(unknown)?;
    let failed = |e: std::io::Error| OutputError::NotAvailable { detail: e.to_string() };
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(failed)?;
    socket.set_broadcast(true).map_err(failed)?;
    Ok((socket, to))
}

/// Sends one datagram to `to`.
pub(crate) fn send_to(socket: &UdpSocket, packet: &[u8], to: SocketAddr) -> Result<(), OutputError> {
    socket.send_to(packet, to).map(drop).map_err(|e| OutputError::SendFailed { detail: e.to_string() })
}

/// An interface found on this computer, as the Director offers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FoundOutput {
    /// The output to choose for it.
    pub choice: OutputChoice,
    /// What the system calls it, such as a USB product name.
    pub label: String,
}

/// The USB-DMX interfaces plugged in now, and the serial ports one could
/// be on. A port whose USB product is known is offered with its protocol;
/// any other is offered as both, for the quizmaster to pick.
pub fn find() -> Vec<FoundOutput> {
    #[cfg(feature = "devices")]
    {
        let mut found = serial::find();
        found.extend(usb::find());
        found
    }
    #[cfg(not(feature = "devices"))]
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_count_from_one_and_stay_in_the_universe() {
        let mut universe = Universe::default();
        universe.set(1, 10);
        universe.set(512, 20);
        universe.set(0, 30);
        universe.set(513, 40);
        assert_eq!((universe.get(1), universe.get(512), universe.get(0), universe.get(513)), (10, 20, 0, 0));
        assert_eq!(universe.slots()[0], 10);
    }

    #[test]
    fn a_blend_goes_each_channel_part_of_the_way() {
        let (mut from, mut to) = (Universe::default(), Universe::default());
        from.set(1, 200);
        to.set(2, 100);
        let half = from.blend(&to, 0.5);
        assert_eq!((half.get(1), half.get(2)), (100, 50));
        assert_eq!(from.blend(&to, 2.0), to);
        assert_eq!(from.blend(&to, -1.0), from);
    }
}
