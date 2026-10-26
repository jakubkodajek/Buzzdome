//! Command-line probe for a buzzer receiver, no app needed.
//!
//!   cargo run -p buzzers --example hid-monitor                    # list HID devices
//!   cargo run -p buzzers --example hid-monitor -- 10c4:ea80       # print reports
//!   cargo run -p buzzers --example hid-monitor -- 10c4:1819 rush  # fastest finger
//!
//! With a VID:PID it runs the app's own receiver thread on the first matching
//! interface, with the protocol the app picks for it, and prints the status,
//! every input report in hex with a millisecond timestamp, and the presses
//! decoded from it. Press each button on a buzzer and save the output: for a
//! receiver the app does not know, it is what the real protocol decoder is
//! written from. A receiver that runs its rounds itself (the Enjoy RF215) is
//! set up as in the app and takes answers A to D, or with `rush` a
//! fastest-finger round, which goes on when its first buzz is ignored.

use buzzers::clock::ShowClock;
use buzzers::hid::{DeviceInfo, DeviceSelector, HidBackend};
use buzzers::hidapi_backend::HidapiBackend;
use buzzers::protocol::{self, format_hex};
use buzzers::receiver::{self, ReceiverConfig, ReceiverEvent};
use quiz_core::BuzzerMode;
use std::sync::{mpsc, Arc};

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
    let mode = match std::env::args().nth(2).as_deref() {
        None => BuzzerMode::Answers,
        Some("rush") => BuzzerMode::FastestFinger,
        Some(other) => return Err(format!("unknown round {other}: expected rush").into()),
    };
    let resolver = Arc::new(|device: &DeviceInfo| protocol::for_device(&[], device));
    let (events, rx) = mpsc::channel();
    let (handle, _join) = receiver::spawn(Arc::new(backend), resolver, events, ReceiverConfig::new(ShowClock::start()));
    handle.set_mode(mode);
    handle.select(Some(DeviceSelector { vendor_id, product_id, usage_page: None, interface_number: None }));
    println!("Waiting for {arg}. Press buzzer buttons; Ctrl+C to stop.");
    for event in rx {
        match event {
            ReceiverEvent::Status(status) => println!("{status:?}"),
            ReceiverEvent::Raw(raw) => println!("{:>9.3}s  [{:>2}]  {}", raw.at_ms as f64 / 1000.0, raw.bytes.len(), format_hex(&raw.bytes)),
            ReceiverEvent::Press { press, .. } => {
                println!("{:>14}buzzer {}, {:?}", "", press.buzzer, press.button);
                // As the app does with a press it does not take, so a
                // fastest-finger round goes on.
                if mode == BuzzerMode::FastestFinger {
                    handle.ignored(press);
                }
            }
        }
    }
    Ok(())
}
