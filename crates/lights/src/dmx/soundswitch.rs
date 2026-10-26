//! The SoundSwitch Micro DMX Interface's protocol. SoundSwitch does not
//! publish it; it is taken from the independent, Apache-2.0 QLC+ plug-in
//! that drives the interface (qlcplus-soundswitch-integration,
//! <https://github.com/jloops412/qlcplus-soundswitch-integration>), in our
//! own words and unverified on the hardware here, so the Director calls the
//! output experimental. Our notes: `docs/design.md` section 7.5.
//!
//! The interface is a USB device with one bulk OUT endpoint. It takes two
//! 12-byte start packets, then for a while dark frames, then a frame
//! per universe every 25 ms: the tag "sTRt", four bytes that say "DMX
//! data", the port (0) and a zero, then the 512 channels.

use super::{Universe, UNIVERSE_SIZE};
use std::time::Duration;

/// Its USB vendor (inMusic) and product.
pub const VENDOR_ID: u16 = 0x15E4;
pub const PRODUCT_ID: u16 = 0x0053;
/// The bulk OUT endpoint the packets go to.
pub const ENDPOINT: u8 = 0x01;
/// The tag every packet starts with.
const TAG: &[u8; 4] = b"sTRt";
/// The bytes before the channels in a frame.
const FRAME_HEADER: usize = 10;
/// The start packets, in order.
pub const START_PACKETS: [[u8; 12]; 2] = [
    [b's', b'T', b'R', b't', 0x02, 0x00, 0x04, 0x00, 0x00, 0x00, 0x01, 0x00],
    [b's', b'T', b'R', b't', 0x02, 0x00, 0x04, 0x00, 0x01, 0x00, 0xFF, 0xFF],
];
/// How long it gets after the start packets.
pub const AFTER_START: Duration = Duration::from_millis(200);
/// How many dark frames it gets before the first frame of a look.
pub const WARM_UP_FRAMES: u32 = 50;
/// How many dark frames it gets before it is let go.
pub const DARK_FRAMES_AT_RELEASE: u32 = 3;
/// How often it takes a frame.
pub const FRAME_EVERY: Duration = Duration::from_millis(25);

/// The frame that sends `universe` to the interface's port 0.
pub fn frame(universe: &Universe) -> [u8; FRAME_HEADER + UNIVERSE_SIZE] {
    let mut frame = [0; FRAME_HEADER + UNIVERSE_SIZE];
    frame[..4].copy_from_slice(TAG);
    frame[4..8].copy_from_slice(&[0x01, 0x00, 0x02, 0x02]);
    frame[FRAME_HEADER..].copy_from_slice(universe.slots());
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_the_tag_its_kind_the_port_and_the_universe() {
        let mut universe = Universe::default();
        universe.set(1, 0x11);
        universe.set(512, 0x22);
        let frame = frame(&universe);
        assert_eq!(frame.len(), 522);
        assert_eq!(&frame[..11], b"sTRt\x01\x00\x02\x02\x00\x00\x11");
        assert_eq!(frame[521], 0x22);
        assert!(START_PACKETS.iter().all(|packet| packet.starts_with(TAG)));
    }
}
