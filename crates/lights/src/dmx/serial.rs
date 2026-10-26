//! USB-DMX interfaces on a serial port: Enttec DMX USB Pro and Open DMX
//! USB, and the many interfaces that copy them (`enttec`).

use super::enttec::{open_dmx_frame, pro_message};
use super::{DmxOutput, FoundOutput, OutputError, Universe};
use crate::setup::OutputChoice;
use serialport::{DataBits, Parity, SerialPort, SerialPortType, StopBits};
use std::time::Duration;

/// DMX's bit rate.
const DMX_BAUD: u32 = 250_000;
/// The break before a frame; DMX asks for at least 88 µs, and a system
/// sleep is rarely shorter than a millisecond anyway.
const BREAK: Duration = Duration::from_micros(150);
/// The mark after the break; at least 8 µs.
const MARK_AFTER_BREAK: Duration = Duration::from_micros(20);
/// An Open DMX frame takes about 23 ms on the wire, so it is sent less often.
const OPEN_DMX_EVERY: Duration = Duration::from_millis(33);
/// The longest a write may take before the interface counts as lost.
const WRITE_TIMEOUT: Duration = Duration::from_millis(500);
/// FTDI's USB vendor id, which Enttec's interfaces and their clones use.
const FTDI: u16 = 0x0403;

fn not_available(e: impl ToString) -> OutputError {
    OutputError::NotAvailable { detail: e.to_string() }
}

fn send_failed(e: impl ToString) -> OutputError {
    OutputError::SendFailed { detail: e.to_string() }
}

fn open_port(port: &str) -> Result<Box<dyn SerialPort>, OutputError> {
    serialport::new(port, DMX_BAUD)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::Two)
        .timeout(WRITE_TIMEOUT)
        .open()
        .map_err(not_available)
}

/// An Enttec DMX USB Pro, or an interface that speaks its protocol.
pub struct EnttecProPort {
    port: Box<dyn SerialPort>,
    name: String,
}

impl EnttecProPort {
    pub fn open(port: &str) -> Result<EnttecProPort, OutputError> {
        Ok(EnttecProPort { port: open_port(port)?, name: format!("DMX USB Pro ({port})") })
    }
}

impl DmxOutput for EnttecProPort {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn send(&mut self, universe: &Universe) -> Result<(), OutputError> {
        self.port.write_all(&pro_message(universe)).map_err(send_failed)
    }
}

/// An Enttec Open DMX USB or a clone: the computer makes the DMX signal.
pub struct OpenDmxPort {
    port: Box<dyn SerialPort>,
    name: String,
}

impl OpenDmxPort {
    pub fn open(port: &str) -> Result<OpenDmxPort, OutputError> {
        Ok(OpenDmxPort { port: open_port(port)?, name: format!("Open DMX USB ({port})") })
    }
}

impl DmxOutput for OpenDmxPort {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn frame_every(&self) -> Duration {
        OPEN_DMX_EVERY
    }

    fn send(&mut self, universe: &Universe) -> Result<(), OutputError> {
        // The frame before must be on the wire before the break.
        self.port.flush().map_err(send_failed)?;
        self.port.set_break().map_err(send_failed)?;
        std::thread::sleep(BREAK);
        self.port.clear_break().map_err(send_failed)?;
        std::thread::sleep(MARK_AFTER_BREAK);
        self.port.write_all(&open_dmx_frame(universe)).map_err(send_failed)
    }
}

/// The serial ports, with the protocol their USB product names when it
/// names one, else as both Enttec outputs.
pub fn find() -> Vec<FoundOutput> {
    let Ok(ports) = serialport::available_ports() else { return Vec::new() };
    let mut found = Vec::new();
    for port in ports {
        let SerialPortType::UsbPort(usb) = &port.port_type else { continue };
        let product = usb.product.clone().unwrap_or_default();
        let label = [usb.manufacturer.as_deref().unwrap_or(""), &product, &port.port_name]
            .iter()
            .filter(|part| !part.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" · ");
        let pro = product.to_ascii_uppercase().contains("PRO");
        let name = port.port_name.clone();
        if pro || usb.vid != FTDI {
            found.push(FoundOutput { choice: OutputChoice::EnttecPro { port: name.clone() }, label: label.clone() });
        }
        if !pro {
            found.push(FoundOutput { choice: OutputChoice::OpenDmx { port: name }, label });
        }
    }
    found
}
