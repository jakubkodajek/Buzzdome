//! Art-Net: DMX over UDP, as most lighting desks, nodes and programs take
//! it. Each frame is one ArtDmx packet (Art-Net 4 specification, Artistic
//! Licence, <https://art-net.org.uk>).

use super::{send_to, udp_to, DmxOutput, OutputError, Universe, UNIVERSE_SIZE};
use std::net::{SocketAddr, UdpSocket};

/// Art-Net's UDP port.
pub const PORT: u16 = 6454;
/// The protocol version the packets carry.
const PROTOCOL_VERSION: u16 = 14;
/// The ArtDmx operation code, sent low byte first.
const OP_DMX: u16 = 0x5000;
/// Bytes before the channel data.
const HEADER: usize = 18;

/// The ArtDmx packet for `universe` at `port_address` (net, sub-net and
/// universe in 15 bits). `sequence` counts frames from 1 to 255, so a
/// node can put frames that arrive out of order back in order.
pub fn packet(port_address: u16, sequence: u8, universe: &Universe) -> [u8; HEADER + UNIVERSE_SIZE] {
    let mut packet = [0; HEADER + UNIVERSE_SIZE];
    packet[..8].copy_from_slice(b"Art-Net\0");
    packet[8..10].copy_from_slice(&OP_DMX.to_le_bytes());
    packet[10..12].copy_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    packet[12] = sequence;
    // Physical input port: informational only.
    packet[13] = 0;
    // SubUni (sub-net and universe), then Net.
    packet[14..16].copy_from_slice(&(port_address & 0x7FFF).to_le_bytes());
    packet[16..18].copy_from_slice(&(UNIVERSE_SIZE as u16).to_be_bytes());
    packet[HEADER..].copy_from_slice(universe.slots());
    packet
}

/// Art-Net to one host or a broadcast address.
pub struct ArtNet {
    socket: UdpSocket,
    to: SocketAddr,
    port_address: u16,
    sequence: u8,
}

impl ArtNet {
    pub fn open(host: &str, port_address: u16) -> Result<ArtNet, OutputError> {
        let (socket, to) = udp_to(host, PORT)?;
        Ok(ArtNet { socket, to, port_address, sequence: 0 })
    }
}

impl DmxOutput for ArtNet {
    fn name(&self) -> String {
        format!("Art-Net {} ({})", self.port_address, self.to.ip())
    }

    fn send(&mut self, universe: &Universe) -> Result<(), OutputError> {
        // 0 means "not in order", so the count runs 1 to 255.
        self.sequence = self.sequence % 255 + 1;
        send_to(&self.socket, &packet(self.port_address, self.sequence, universe), self.to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_artdmx_packet_carries_its_port_address_and_the_whole_universe() {
        let mut universe = Universe::default();
        universe.set(1, 0xAA);
        universe.set(512, 0x55);
        let packet = packet(0x1234, 7, &universe);
        assert_eq!(&packet[..18], b"Art-Net\0\x00\x50\x00\x0e\x07\x00\x34\x12\x02\x00");
        assert_eq!((packet[18], packet[529]), (0xAA, 0x55));
        // Only 15 bits address a port.
        assert_eq!(&super::packet(0xFFFF, 1, &universe)[14..16], &[0xFF, 0x7F]);
    }

    #[test]
    fn frames_reach_the_host_with_a_sequence_that_skips_zero() {
        let node = UdpSocket::bind("127.0.0.1:0").unwrap();
        let (socket, _) = udp_to("127.0.0.1", 0).unwrap();
        let mut output = ArtNet { socket, to: node.local_addr().unwrap(), port_address: 3, sequence: 254 };
        let mut buffer = [0; 600];
        let mut sequences = Vec::new();
        for _ in 0..2 {
            output.send(&Universe::default()).unwrap();
            let (len, _) = node.recv_from(&mut buffer).unwrap();
            assert_eq!(len, 530);
            sequences.push(buffer[12]);
        }
        assert_eq!(sequences, [255, 1]);
    }
}
