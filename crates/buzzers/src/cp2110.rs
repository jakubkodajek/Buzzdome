//! HID-to-UART bridges of the Silicon Labs CP2110 kind: the USB side of a
//! cable that carries a receiver's serial line as HID reports, so no driver
//! is needed. Report layout from Silicon Labs AN434, "CP2110/4 Interface
//! Specification".
//!
//! The UART is set up with feature reports. Data goes both ways in reports
//! whose ID is the number of data bytes they carry (1 to 63), so a message
//! can arrive split over reports or several in one: a protocol on top reads
//! the bytes as a stream.

use crate::hid::Report;

/// Most data bytes in one report: report IDs 0x01 to 0x3F.
pub const MAX_DATA: usize = 0x3f;
/// Every report is this long, the ID included (AN434: 0 to 63).
const REPORT_LEN: usize = 64;

const SET_UART_ENABLE: u8 = 0x41;
const PURGE_FIFOS: u8 = 0x43;
const PURGE_BOTH: u8 = 0x03;
const UART_CONFIG: u8 = 0x50;
const PARITY_NONE: u8 = 0x00;
const FLOW_CONTROL_NONE: u8 = 0x00;
const DATA_BITS_8: u8 = 0x03;
const STOP_BITS_ONE: u8 = 0x00;

/// The reports that open the UART at `baud`, 8 data bits, no parity, one
/// stop bit and no flow control, with nothing left over from before. The
/// bridge forgets its settings when it loses power, so they are sent on
/// every connect.
///
/// The FIFOs are purged here only: purging before each message, as some
/// programs do, would also drop presses the receiver just sent.
pub fn open_uart(baud: u32) -> Vec<Report> {
    let mut config = vec![UART_CONFIG];
    config.extend(baud.to_be_bytes());
    config.extend([PARITY_NONE, FLOW_CONTROL_NONE, DATA_BITS_8, STOP_BITS_ONE]);
    vec![Report::Feature(config), Report::Feature(vec![PURGE_FIFOS, PURGE_BOTH]), Report::Feature(vec![SET_UART_ENABLE, 0x01])]
}

/// The output reports that send `bytes` down the serial line.
pub fn send(bytes: &[u8]) -> Vec<Report> {
    bytes.chunks(MAX_DATA).map(|chunk| Report::Output(data_report(chunk))).collect()
}

/// The report that carries `chunk`, at most `MAX_DATA` bytes, either way.
pub(crate) fn data_report(chunk: &[u8]) -> Vec<u8> {
    let mut report = vec![0; REPORT_LEN];
    report[0] = chunk.len() as u8;
    report[1..=chunk.len()].copy_from_slice(chunk);
    report
}

/// Whether the feature `report` purges the bridge's buffers (`open_uart`).
pub(crate) fn purges(report: &[u8]) -> bool {
    report.first() == Some(&PURGE_FIFOS)
}

/// The serial bytes an input report carries: none for a report that is not
/// data, and no more than the report holds.
pub fn received(report: &[u8]) -> &[u8] {
    match report.split_first() {
        Some((&count, data)) if (1..=MAX_DATA).contains(&usize::from(count)) => &data[..data.len().min(usize::from(count))],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_the_uart_at_115200_8n1() {
        assert_eq!(
            open_uart(115_200),
            [
                Report::Feature(vec![0x50, 0x00, 0x01, 0xc2, 0x00, 0x00, 0x00, 0x03, 0x00]),
                Report::Feature(vec![0x43, 0x03]),
                Report::Feature(vec![0x41, 0x01]),
            ]
        );
    }

    #[test]
    fn sends_bytes_in_reports_named_by_their_length() {
        let reports = send(&[0x01, 0xe4, 0xe5]);
        let mut expected = vec![0; 64];
        expected[..4].copy_from_slice(&[0x03, 0x01, 0xe4, 0xe5]);
        assert_eq!(reports, [Report::Output(expected)]);

        let long: Vec<u8> = (0..70).collect();
        let reports = send(&long);
        assert_eq!(reports.len(), 2);
        let Report::Output(second) = &reports[1] else { panic!("{reports:?}") };
        assert_eq!(second[0], 7);
        assert_eq!(&second[1..8], &long[63..]);
        assert!(send(&[]).is_empty());
    }

    #[test]
    fn reads_only_the_data_bytes_a_report_names() {
        assert_eq!(received(&[0x02, 0xaa, 0xbb, 0x00, 0x00]), [0xaa, 0xbb]);
        // A short read gives what arrived, never more.
        assert_eq!(received(&[0x05, 0xaa]), [0xaa]);
        assert!(received(&[0x00, 0xaa]).is_empty());
        assert!(received(&[0x50, 0xaa]).is_empty());
        assert!(received(&[]).is_empty());
    }
}
