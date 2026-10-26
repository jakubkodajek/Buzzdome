//! The two Enttec USB-DMX protocols, which most other USB-DMX interfaces
//! copy (Enttec, DMX USB Pro API and Open DMX USB,
//! <https://www.enttec.com/product/dmx-usb-interfaces/open-dmx-usb/>).
//!
//! - DMX USB Pro: the interface times DMX itself; the computer sends each
//!   universe as one message over the USB serial port.
//! - Open DMX USB: a bare FTDI chip; the computer makes the DMX signal: a
//!   break, then the start code and the channels at 250 kbit/s.

use super::{Universe, UNIVERSE_SIZE};

/// Starts and ends every DMX USB Pro message.
const START: u8 = 0x7E;
const END: u8 = 0xE7;
/// The DMX USB Pro message that sends a universe.
const SEND_DMX: u8 = 6;
/// The start code of a frame of channel values.
pub const START_CODE: u8 = 0;

/// The DMX USB Pro message that sends `universe`.
pub fn pro_message(universe: &Universe) -> Vec<u8> {
    let length = (UNIVERSE_SIZE + 1) as u16;
    let mut message = Vec::with_capacity(UNIVERSE_SIZE + 6);
    message.extend([START, SEND_DMX]);
    message.extend(length.to_le_bytes());
    message.push(START_CODE);
    message.extend(universe.slots());
    message.push(END);
    message
}

/// What an Open DMX USB sends after the break: the start code and the channels.
pub fn open_dmx_frame(universe: &Universe) -> Vec<u8> {
    let mut frame = Vec::with_capacity(UNIVERSE_SIZE + 1);
    frame.push(START_CODE);
    frame.extend(universe.slots());
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pro_message_wraps_the_start_code_and_the_universe() {
        let mut universe = Universe::default();
        universe.set(1, 1);
        universe.set(512, 2);
        let message = pro_message(&universe);
        assert_eq!(message.len(), 518);
        assert_eq!(&message[..6], &[0x7E, 6, 0x01, 0x02, 0, 1]);
        assert_eq!(&message[516..], &[2, 0xE7]);
    }

    #[test]
    fn an_open_dmx_frame_is_the_start_code_and_the_channels() {
        let mut universe = Universe::default();
        universe.set(512, 3);
        let frame = open_dmx_frame(&universe);
        assert_eq!((frame.len(), frame[0], frame[512]), (513, 0, 3));
    }
}
