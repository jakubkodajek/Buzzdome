//! Command-line probe for a buzzer receiver, no app needed.
//!
//!   cargo run -p buzzers --example hid-monitor                 # list HID devices
//!   cargo run -p buzzers --example hid-monitor -- 10c4:ea80    # print reports
//!
//! With a VID:PID it opens the first matching interface and prints every input
//! report in hex with a millisecond timestamp. Press each button on a buzzer and
//! save the output: it is what is needed to write the real protocol decoder.

use buzzers::hid::{DeviceSelector, HidBackend};
use buzzers::hidapi_backend::HidapiBackend;
use buzzers::protocol::format_hex;
use std::time::Instant;

fn parse_vid_pid(s: &str) -> Option<(u16, u16)> {
    let (v, p) = s.split_once(':')?;
    Some((u16::from_str_radix(v, 16).ok()?, u16::from_str_radix(p, 16).ok()?))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = HidapiBackend::new()?;
    let devices = backend.list()?;
    let Some(arg) = std::env::args().nth(1) else {
        println!("{:<10} {:<6} {:<6} {:<4} product / manufacturer", "VID:PID", "page", "usage", "if");
        for d in &devices {
            println!(
                "{:<10} {:<6} {:<6} {:<4} {} / {}{}",
                d.vid_pid(),
                format!("{:04x}", d.usage_page),
                format!("{:04x}", d.usage),
                d.interface_number,
                d.product.as_deref().unwrap_or("-"),
                d.manufacturer.as_deref().unwrap_or("-"),
                if d.is_keyboard_or_mouse() { "  (keyboard/mouse: owned by the OS)" } else { "" },
            );
        }
        println!("\nRun again with a VID:PID to print its reports.");
        return Ok(());
    };
    let (vendor_id, product_id) = parse_vid_pid(&arg).ok_or("expected VID:PID in hex, e.g. 10c4:ea80")?;
    let selector = DeviceSelector { vendor_id, product_id, usage_page: None, interface_number: None };
    let info = devices.iter().find(|d| selector.matches(d)).ok_or("no such device is connected")?;
    println!("Opening {} {} ({})", info.vid_pid(), info.product.as_deref().unwrap_or(""), info.path);
    let mut device = backend.open(&info.path)?;
    println!("Listening. Press buzzer buttons; Ctrl+C to stop.");
    let start = Instant::now();
    let mut buf = [0u8; 256];
    loop {
        let n = device.read_timeout(&mut buf, 250)?;
        if n > 0 {
            println!("{:>9.3}s  [{:>2}]  {}", start.elapsed().as_secs_f64(), n, format_hex(&buf[..n]));
        }
    }
}
