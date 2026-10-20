//! Command-line probe for a buzzer receiver, no app needed.
//!
//!   cargo run -p buzzers --example hid-monitor                 # list HID devices
//!   cargo run -p buzzers --example hid-monitor -- 10c4:ea80    # print reports
//!   cargo run -p buzzers --example hid-monitor -- 10c4:1819 rf215       # Enjoy RF215, A to D
//!   cargo run -p buzzers --example hid-monitor -- 10c4:1819 rf215-rush  # Enjoy RF215, big button
//!
//! With a VID:PID it opens the first matching interface and prints every input
//! report in hex with a millisecond timestamp. Press each button on a buzzer and
//! save the output: it is what is needed to write the real protocol decoder.
//!
//! With `rf215` or `rf215-rush` it first sets the receiver up as the app
//! does and starts an everyone-answers or a fastest-finger round, then also
//! prints the presses it reads and whether the receiver answered.

use buzzers::hid::{DeviceSelector, HidBackend};
use buzzers::hidapi_backend::HidapiBackend;
use buzzers::protocol::{format_hex, Protocol};
use buzzers::rf215::Rf215;
use quiz_core::BuzzerMode;
use std::time::{Duration, Instant};

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
    let round = match std::env::args().nth(2).as_deref() {
        None => None,
        Some("rf215") => Some(BuzzerMode::Answers),
        Some("rf215-rush") => Some(BuzzerMode::FastestFinger { again: false }),
        Some(other) => return Err(format!("unknown mode {other}: expected rf215 or rf215-rush").into()),
    };
    let selector = DeviceSelector { vendor_id, product_id, usage_page: None, interface_number: None };
    let info = devices.iter().find(|d| selector.matches(d)).ok_or("no such device is connected")?;
    println!("Opening {} {} ({})", info.vid_pid(), info.product.as_deref().unwrap_or(""), info.path);
    let mut device = backend.open(&info.path)?;
    let start = Instant::now();
    let mut buf = [0u8; 256];
    let mut rf215 = round.map(|_| Rf215::default());
    if let (Some(rf), Some(round)) = (rf215.as_mut(), round) {
        // As the app's receiver thread: 10 ms between reports, and the round
        // once the receiver answered, or after 500 ms.
        for report in rf.init_reports() {
            report.send(device.as_mut())?;
            std::thread::sleep(Duration::from_millis(10));
        }
        let sent = Instant::now();
        while !rf.ready() && sent.elapsed() < Duration::from_millis(500) {
            let n = device.read_timeout(&mut buf, 50)?;
            if n > 0 {
                println!("{:>9.3}s  [{:>2}]  {}", start.elapsed().as_secs_f64(), n, format_hex(&buf[..n]));
                rf.decode(&buf[..n]);
            }
        }
        println!("Receiver {}.", if rf.ready() { "answered" } else { "did not answer within 500 ms" });
        for report in rf.encode_mode(round) {
            report.send(device.as_mut())?;
        }
        println!("Started the round ({round:?}).");
    }
    println!("Listening. Press buzzer buttons; Ctrl+C to stop.");
    loop {
        let n = device.read_timeout(&mut buf, 250)?;
        if n > 0 {
            println!("{:>9.3}s  [{:>2}]  {}", start.elapsed().as_secs_f64(), n, format_hex(&buf[..n]));
            if let Some(rf) = rf215.as_mut() {
                let presses = rf.decode(&buf[..n]);
                println!("{:>14}{:?} {presses:?}", "", rf.kind());
            }
        }
    }
}
