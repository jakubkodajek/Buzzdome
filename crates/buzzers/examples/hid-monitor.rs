//! Command-line probe for a buzzer receiver, no app needed.
//!
//!   cargo run -p buzzers --example hid-monitor                      # list HID devices
//!   cargo run -p buzzers --example hid-monitor -- 10c4:ea80         # print reports
//!   cargo run -p buzzers --example hid-monitor -- 10c4:1819 rush    # fastest finger
//!   cargo run -p buzzers --example hid-monitor -- 10c4:1819 lights  # every light in turn
//!   cargo run -p buzzers --example hid-monitor -- 10c4:1819 start   # no light at all
//!
//! With a VID:PID it runs the app's own receiver thread on the first matching
//! interface, with the protocol the app picks for it, and prints the status,
//! every input report in hex with a millisecond timestamp, and the presses
//! decoded from it, with the buzzer's own timer when it sends one, or
//! dropped as from a buzzer that was not ready. Press each
//! button on a buzzer and save the output: for a receiver the app does not
//! know, it is what the real protocol decoder is written from.
//!
//! The buzzers show ready and take answers as in a question; with `rush` as
//! in a fastest-finger round, whose buzzes go in the order of the buzzers'
//! timers. With `lights` they show each light of a show in turn, two
//! seconds each, and a second into each, buzzers 2 and 6 alone take the
//! next one: one order lights all buzzers, then one order each lights
//! those two. Every press is taken then, whatever the buzzer shows. With
//! `start` the receiver is set up and started, but no buzzer is told a
//! colour: what start alone does, which a show's first colours hide.

use buzzers::clock::ShowClock;
use buzzers::enjoy303s::MAX_BUZZER;
use buzzers::hid::{DeviceInfo, DeviceSelector, HidBackend};
use buzzers::hidapi_backend::HidapiBackend;
use buzzers::protocol::{self, format_hex};
use buzzers::receiver::{self, ReceiverChoice, ReceiverConfig, ReceiverEvent, ReceiverHandle};
use quiz_core::{BuzzerMode, Light};
use std::sync::{mpsc, Arc};
use std::time::Duration;

/// Lights every buzzer a receiver takes.
fn light_all(handle: &ReceiverHandle, light: Light) {
    handle.set_lights((1..=MAX_BUZZER).map(|b| (b, light)).collect());
}

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
    let (mode, cycle, lit) = match std::env::args().nth(2).as_deref() {
        None => (BuzzerMode::Answers, false, true),
        Some("rush") => (BuzzerMode::FastestFinger, false, true),
        Some("lights") => (BuzzerMode::Register, true, true),
        Some("start") => (BuzzerMode::Register, false, false),
        Some(other) => return Err(format!("unknown option {other}: expected rush, lights or start").into()),
    };
    let resolver = Arc::new(|device: &DeviceInfo| protocol::for_device(&[], device));
    let (events, rx) = mpsc::channel();
    let (handle, _join) = receiver::spawn(Arc::new(backend), resolver, events, ReceiverConfig::new(ShowClock::start()));
    if lit {
        light_all(&handle, Light::Ready);
    }
    handle.set_mode(mode);
    handle.select(ReceiverChoice::Device(DeviceSelector { vendor_id, product_id, usage_page: None, interface_number: None }));
    if cycle {
        let handle = handle.clone();
        std::thread::spawn(move || {
            let lights = Light::ALL;
            for (light, next) in lights.into_iter().zip(lights.into_iter().cycle().skip(1)).cycle() {
                println!("{:>14}all buzzers: {light:?}", "");
                light_all(&handle, light);
                std::thread::sleep(Duration::from_secs(1));
                println!("{:>14}buzzers 2 and 6: {next:?}", "");
                handle.set_lights(vec![(2, next), (6, next)]);
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    }
    println!("Waiting for {arg}. Press buzzer buttons; Ctrl+C to stop.");
    for event in rx {
        match event {
            ReceiverEvent::Status(status) => println!("{status:?}"),
            ReceiverEvent::Raw(raw) => {
                println!("{:>9.3}s  [{:>2}]  {}", raw.at_ms as f64 / 1000.0, raw.bytes.len(), format_hex(&raw.bytes));
                for press in raw.dropped {
                    println!("{:>14}buzzer {}, {:?}: dropped, the buzzer was not ready", "", press.buzzer, press.button);
                }
            }
            ReceiverEvent::Press { press, .. } => {
                let timer = press.timer.map(|t| format!(", timer {t}")).unwrap_or_default();
                println!("{:>14}buzzer {}, {:?}{timer}", "", press.buzzer, press.button);
            }
            ReceiverEvent::Remote { key, .. } => println!("{:>14}remote: {key:?}", ""),
        }
    }
    Ok(())
}
