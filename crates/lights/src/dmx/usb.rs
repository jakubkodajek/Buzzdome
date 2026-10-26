//! The SoundSwitch Micro DMX Interface over libusb (`soundswitch`).
//! SoundSwitch's own software must be closed: only one program can hold
//! the interface.

use super::soundswitch::{frame, AFTER_START, ENDPOINT, FRAME_EVERY, PRODUCT_ID, START_PACKETS, VENDOR_ID, WARM_UP_FRAMES};
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
    /// Frames of zeros still to send before the channels.
    warming_up: u32,
}

impl SoundSwitchMicro {
    pub fn open() -> Result<SoundSwitchMicro, OutputError> {
        let handle = rusb::open_device_with_vid_pid(VENDOR_ID, PRODUCT_ID)
            .ok_or_else(|| not_available("SoundSwitch Micro DMX Interface not found, or SoundSwitch holds it"))?;
        // Where the system has a driver of its own on it (not on Windows), it lets go.
        let _ = handle.set_auto_detach_kernel_driver(true);
        handle.claim_interface(INTERFACE).map_err(not_available)?;
        let mut micro = SoundSwitchMicro { handle, warming_up: WARM_UP_FRAMES };
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
            Err(OutputError::SendFailed { detail: format!("wrote {written} of {} bytes", bytes.len()) })
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

    fn send(&mut self, universe: &Universe) -> Result<(), OutputError> {
        if self.warming_up > 0 {
            self.warming_up -= 1;
            return self.write(&frame(&Universe::default()));
        }
        self.write(&frame(universe))
    }
}

impl Drop for SoundSwitchMicro {
    /// Leaves the lights dark, as SoundSwitch does when it lets go.
    fn drop(&mut self) {
        let dark = frame(&Universe::default());
        for _ in 0..3 {
            let _ = self.write(&dark);
        }
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
