//! sACN, Streaming ACN (ANSI E1.31): DMX over UDP, by multicast to a
//! group per universe or to one host. Each frame is one E1.31 data packet
//! of a root, a framing and a DMP layer.

use super::{send_to, udp_to, DmxOutput, OutputError, Universe, UNIVERSE_SIZE};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::net::{SocketAddr, UdpSocket};

/// sACN's UDP port.
pub const PORT: u16 = 5568;
/// The bytes of a packet with a whole universe.
pub const PACKET_LEN: usize = 638;
/// The priority receivers use to choose between sources (default 100).
const PRIORITY: u8 = 100;
/// The name receivers show for this source.
const SOURCE_NAME: &str = "Buzzdome";

/// A layer's flags and length: the length from the layer's start to the
/// packet's end, with the flags 0x7 in the high bits.
fn flags_and_length(from: usize) -> [u8; 2] {
    (0x7000 | (PACKET_LEN - from) as u16).to_be_bytes()
}

/// The E1.31 data packet for `universe` from the source `cid`, with its
/// `sequence` number.
pub fn packet(cid: &[u8; 16], universe_number: u16, sequence: u8, universe: &Universe) -> [u8; PACKET_LEN] {
    let mut p = [0; PACKET_LEN];
    // Root layer.
    p[0..2].copy_from_slice(&0x0010u16.to_be_bytes());
    p[4..16].copy_from_slice(b"ASC-E1.17\0\0\0");
    p[16..18].copy_from_slice(&flags_and_length(16));
    p[18..22].copy_from_slice(&0x0000_0004u32.to_be_bytes());
    p[22..38].copy_from_slice(cid);
    // Framing layer.
    p[38..40].copy_from_slice(&flags_and_length(38));
    p[40..44].copy_from_slice(&0x0000_0002u32.to_be_bytes());
    p[44..44 + SOURCE_NAME.len()].copy_from_slice(SOURCE_NAME.as_bytes());
    p[108] = PRIORITY;
    p[111] = sequence;
    p[113..115].copy_from_slice(&universe_number.to_be_bytes());
    // DMP layer: one property, the start code and the channels.
    p[115..117].copy_from_slice(&flags_and_length(115));
    p[117] = 0x02;
    p[118] = 0xA1;
    p[121..123].copy_from_slice(&1u16.to_be_bytes());
    p[123..125].copy_from_slice(&(UNIVERSE_SIZE as u16 + 1).to_be_bytes());
    p[126..].copy_from_slice(universe.slots());
    p
}

/// The multicast group of `universe`: 239.255 and its two bytes.
pub fn multicast_group(universe: u16) -> String {
    let [high, low] = universe.to_be_bytes();
    format!("239.255.{high}.{low}")
}

/// This computer's source id: the same on every start, so receivers see one
/// source, and unlike another computer's.
pub fn source_id() -> [u8; 16] {
    let computer = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_default();
    let half = |part: u8| {
        let mut hasher = DefaultHasher::new();
        (SOURCE_NAME, computer.as_str(), part).hash(&mut hasher);
        hasher.finish().to_be_bytes()
    };
    let mut cid = [0; 16];
    cid[..8].copy_from_slice(&half(0));
    cid[8..].copy_from_slice(&half(1));
    cid
}

pub struct Sacn {
    socket: UdpSocket,
    to: SocketAddr,
    universe: u16,
    cid: [u8; 16],
    sequence: u8,
}

impl Sacn {
    /// sACN for `universe`, to `host` or else by multicast.
    pub fn open(universe: u16, host: Option<&str>) -> Result<Sacn, OutputError> {
        let host = host.map_or_else(|| multicast_group(universe), str::to_owned);
        let (socket, to) = udp_to(&host, PORT)?;
        Ok(Sacn { socket, to, universe, cid: source_id(), sequence: 0 })
    }
}

impl DmxOutput for Sacn {
    fn name(&self) -> String {
        format!("sACN {} ({})", self.universe, self.to.ip())
    }

    fn send(&mut self, universe: &Universe) -> Result<(), OutputError> {
        self.sequence = self.sequence.wrapping_add(1);
        send_to(&self.socket, &packet(&self.cid, self.universe, self.sequence, universe), self.to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_e131_packet_has_its_three_layers_and_the_universe() {
        let mut universe = Universe::default();
        universe.set(1, 9);
        universe.set(512, 8);
        let cid = [7; 16];
        let p = packet(&cid, 0x0102, 42, &universe);
        assert_eq!(&p[..22], b"\x00\x10\x00\x00ASC-E1.17\x00\x00\x00\x72\x6e\x00\x00\x00\x04");
        assert_eq!(&p[22..38], &cid);
        assert_eq!(&p[38..44], b"\x72\x58\x00\x00\x00\x02");
        assert_eq!(&p[44..53], b"Buzzdome\0");
        assert_eq!((p[108], p[111], p[113], p[114]), (100, 42, 1, 2));
        assert_eq!(&p[115..126], b"\x72\x0b\x02\xa1\x00\x00\x00\x01\x02\x01\x00");
        assert_eq!((p[126], p[637]), (9, 8));
    }

    #[test]
    fn each_universe_has_its_multicast_group_and_the_source_id_stays() {
        assert_eq!(multicast_group(1), "239.255.0.1");
        assert_eq!(multicast_group(63_999), "239.255.249.255");
        assert_eq!(source_id(), source_id());
    }
}
