//! The SoundSwitch Micro DMX Interface over libusb (`soundswitch`).
//! SoundSwitch's own software must be closed: only one program can hold
//! the interface.

use super::soundswitch::{frame, AFTER_START, DARK_FRAMES_AT_RELEASE, ENDPOINT, FRAME_EVERY, PRODUCT_ID, START_PACKETS, VENDOR_ID, WARM_UP_FRAMES};
use super::{DmxOutput, FoundOutput, OutputError, Universe};
use crate::setup::OutputChoice;
use rusb::{DeviceHandle, GlobalContext};
use std::time::Duration;

/// The longest a bulk write may take before the interface counts as lost.
const WRITE_TIMEOUT: Duration = Duration::from_millis(500);
/// The interface its endpoint is on.
const INTERFACE: u8 = 0;

fn not_available(e: impl ToString) -> OutputError {
    OutputError::NotAvailable { detail: e.to_string() }
}

pub struct SoundSwitchMicro {
    handle: DeviceHandle<GlobalContext>,
}

impl SoundSwitchMicro {
    pub fn open() -> Result<SoundSwitchMicro, OutputError> {
        // Not found, or SoundSwitch holds it: libusb does not say which.
        let handle = rusb::open_device_with_vid_pid(VENDOR_ID, PRODUCT_ID).ok_or(OutputError::NotFound)?;
        // Where the system has a driver of its own on it (not on Windows), it lets go.
        let _ = handle.set_auto_detach_kernel_driver(true);
        handle.claim_interface(INTERFACE).map_err(not_available)?;
        let mut micro = SoundSwitchMicro { handle };
        for packet in START_PACKETS {
            micro.write(&packet)?;
        }
        std::thread::sleep(AFTER_START);
        Ok(micro)
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), OutputError> {
        let written = self.handle.write_bulk(ENDPOINT, bytes, WRITE_TIMEOUT).map_err(|e| OutputError::SendFailed { detail: e.to_string() })?;
        if written == bytes.len() {
            Ok(())
        } else {
            Err(OutputError::SentPart { written, of: bytes.len() })
        }
    }
}

impl DmxOutput for SoundSwitchMicro {
    fn name(&self) -> String {
        "SoundSwitch Micro DMX".to_owned()
    }

    fn frame_every(&self) -> Duration {
        FRAME_EVERY
    }

    fn warm_up_frames(&self) -> u32 {
        WARM_UP_FRAMES
    }

    /// Leaves the lights dark, as SoundSwitch does when it lets go.
    fn dark_frames_at_release(&self) -> u32 {
        DARK_FRAMES_AT_RELEASE
    }

    fn send(&mut self, universe: &Universe) -> Result<(), OutputError> {
        self.write(&frame(universe))
    }
}

impl Drop for SoundSwitchMicro {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(INTERFACE);
    }
}

/// The SoundSwitch Micro DMX Interface, when one is plugged in.
pub fn find() -> Vec<FoundOutput> {
    let plugged = rusb::devices().is_ok_and(|devices| {
        devices.iter().any(|device| device.device_descriptor().is_ok_and(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID))
    });
    if plugged {
        vec![FoundOutput { choice: OutputChoice::SoundSwitch, label: "SoundSwitch Micro DMX Interface".to_owned() }]
    } else {
        Vec::new()
    }
}
