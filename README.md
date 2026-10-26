# Buzzdome

A desktop quiz and game-show app for live quiz nights at non-profit events:
questions on the projector, players answering with wireless buzzers, the
quizmaster in control from the laptop. Runs on Windows, macOS and Linux,
built with Tauri (Rust core, React UI).

Inspired by Windows-only game-show software. This is an independent project,
not affiliated with or endorsed by QuizXpress or its makers.

## Status

First playable version: demo quiz, everyone-answers and fastest-finger
questions, scores, leaderboard, final scores, crash recovery, keyboard and
on-screen buzzers. The wireless buzzer receiver connects through its USB-HID
cable (Director > Receiver tab). Jakub's Enjoy 303S (RGB) set is driven with
its own protocol, presses ordered by each buzzer's timer and lights in
colour, though not yet checked on the hardware. For other receivers, buttons
are taught once in learn mode and the buzzer lights stay off. Plan and progress:
[`ROADMAP.md`](ROADMAP.md).

What the app must do: [`docs/specification.md`](docs/specification.md).
How it is built: [`docs/design.md`](docs/design.md). Testing with the real
buzzers: [`docs/hardware-test.md`](docs/hardware-test.md).

## Install

CI builds installers on `main`, on version tags and on demand (GitHub >
Actions > Build > Run workflow, pick the branch). Download them from the
run's Artifacts: `buzzdome-windows` (`.exe` installer), `buzzdome-macos`
(universal `.dmg`) and `buzzdome-linux` (`.deb` and `.AppImage`). They are
not code-signed yet:

- Windows: SmartScreen warns on first start; choose "More info", then
  "Run anyway".
- macOS: open the app once, then System Settings > Privacy & Security >
  "Open Anyway". Allow Input Monitoring if macOS asks, so the app can read
  the buzzer receiver.
- Linux: to open the receiver without root, install the udev rule that the
  Receiver tab shows.

## Development

Needs Rust (the release in `rust-toolchain.toml`, which rustup fetches by
itself) and Node.js 22.

On Windows, install first: Git for Windows, Rust with rustup (the MSVC
toolchain), Node.js 22 and the Microsoft C++ Build Tools with "Desktop
development with C++". WebView2 comes with Windows 11 and with Windows 10
from version 1803. The buzzer receiver's USB cable is a HID device and
needs no driver.
`npm run tauri build` makes both an `.exe` (NSIS) and an `.msi` installer;
if the MSI step fails with `light.exe`, turn on VBSCRIPT under Settings >
Apps > Optional features, or build only the `.exe` with
`npm run tauri build -- --bundles nsis`.

```sh
cd app
npm ci
npm run tauri dev      # run the app
cd .. && cargo test --workspace
cargo run -p buzzers --example hid-monitor   # list HID devices, print receiver reports
```

On Linux, building also needs `libudev-dev` (for the receiver) besides
Tauri's usual WebKitGTK packages; the exact list is in
`.github/workflows/build.yml`.

Project instructions for Claude Code are in `CLAUDE.md`. What the app should do
is mapped in [`replica/recon.md`](replica/recon.md) and prioritised in
[`replica/features.csv`](replica/features.csv).
