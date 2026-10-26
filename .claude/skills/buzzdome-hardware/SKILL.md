---
name: buzzdome-hardware
description: Works on Buzzdome's wireless buzzers (Enjoy 303S receiver over the 10c4:1819 CP2110 cable), learn mode, the quizmaster's remote, or the stage lights (DMX, Art-Net, sACN, OSC, MIDI). Use before touching crates/buzzers, crates/lights or their shell modules.
---

# Buzzers, receiver and stage lights

The show must never fail because of hardware. Keyboard and on-screen
buzzers always work, and the lights are a view of the show that is never
journaled.

## Read first

- `replica/hardware.md`: what is known about the buzzers and the receiver.
- `docs/design.md` §2 (buzzer link: layers, receiver thread, 303S
  protocol, learn mode, press to screen, lights, remote) and §7 (stage
  lights).
- `docs/hardware-test.md`: the checklist for the real receiver, with
  what was verified on 2026-10-25 and what is still open.

## Rules

- **Clean room.** The 303S driver implements a description in our own
  words. No vendor SDK file or code goes into the repository, ever.
- **Order from the hardware.** Fastest-finger order comes from the buzzers'
  own timers (`timer_order.rs`) or the receiver's order, never from the PC.
- **Layers stay separate.** Protocols turn bytes into presses and know no
  USB; the receiver thread knows no game; the game does not know where a
  press came from (`Command::Press`). Keep each change inside one layer.
- **Document what the receiver reports** in `docs/design.md` §2.4 and the
  test plan, marked verified or not.

## Tools

- `cargo test -p buzzers` and `cargo test -p lights`: protocols and frames
  are tested byte for byte against fakes; no device needed.
- `cargo run -p buzzers --example hid-monitor [-- VID:PID]`: lists HID
  devices and prints receiver reports without the app (subcommands at the
  end of `docs/hardware-test.md`).
- E2E without hardware: a debug build started with
  `BUZZDOME_FAKE_RECEIVER=<host:port>` lists a fake 303S receiver
  (`crates/buzzers/src/fake_receiver.rs`) that tests plug in, press and
  read lights from over TCP. Release builds ignore the variable.
- Lights without fixtures: point an Art-Net output at `127.0.0.1` and
  listen on UDP 6454.

## Real hardware

Jakub tests on a Windows laptop with the receiver. Write the steps as a
section in `docs/hardware-test.md` (what to run, what to see, what to send
back), and keep findings in that file's results.
