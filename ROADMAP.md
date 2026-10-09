# Roadmap

Target: first live event in about four weeks (early November 2026) on a
Windows laptop with a projector and wireless buzzers. Phases follow
analysis -> design -> build -> test -> run, but build and test run in short
loops: every milestone ends with something that can be played.

Status keys: `[x]` done, `[~]` in progress, `[ ]` not started.

## Phase 1: Analysis

- [x] Recon of the reference app: screens, flows, data model, scoring (`replica/recon.md`)
- [x] Feature matrix (`replica/features.csv`)
- [x] Manuals converted to searchable Markdown (`replica/sources/`, gitignored)
- [x] Scope decision: desktop app, Windows, hardware buzzers, Czech + English
- [x] Hardware research: OEM, receiver, cable, SDK situation (`replica/hardware.md`)
- [ ] Identify the colleague's buzzer model and the cable's USB VID/PID (steps in `docs/hardware-test.md`)
- [~] Buzzer protocol: Enjoy says the SDK is free and covers it (dome and RGB differ); SDK and its terms requested. Raw report capture is built into the app
- [ ] Re-prioritise `features.csv` for the desktop scope (phone rows to skip, buzzer rows to must)

## Phase 2: Design

- [x] Stack: Tauri 2, Rust core, React + TypeScript UI
- [x] Specification and design: `docs/specification.md`, `docs/design.md` (incl. theme and plugin formats)
- [x] Quiz file format v1: `.quiz` ZIP with `quiz.json` and `media/` (`docs/quiz-format.md`, `crates/quiz-file`)
- [x] Game state machine and scoring rules, documented next to the code
- [x] Persistence and crash recovery: append-only command journal, replayed on resume
- [x] `BuzzerDriver` trait: keyboard, virtual and USB-HID receiver (`crates/buzzers`)
- [x] Screen designs: host screen, Director and quiz editor follow the reference layout

## Phase 3: Build

### M0: Skeleton (week 1)
- [x] Tauri app with Director window and a full-screen Host window on a chosen monitor
- [x] Pure Rust core crate with unit tests; CI tests and builds installers for Windows, macOS and Linux (`.github/workflows/build.yml`)
- [x] Keyboard and on-screen virtual buzzers feeding the same event stream

### M1: Playable core (week 1-2)
- [~] Load a quiz file; billboard, multiple-choice (A-D) and end-of-round slides (first version, needs a click-through)
- [~] Everyone-answers questions: clock, answered count, reveal, points (first version)
- [~] Fastest-finger questions: first buzz, right/wrong from the Director, lockout, clock resume (first version)
- [~] Leaderboard and final scores with top-3 reveal (first version); round scores to do
- [~] Teams table: names per buzzer, edit scores, enable/disable (first version)
- [~] Crash recovery: resume the last show from the start screen (first version)

### M2: Real buzzers (as soon as the protocol is known)
- [x] Receiver connection: pick the USB device, remember it, reconnect after unplugging, OS hints (udev rule, Input Monitoring)
- [x] Presses in receiver order through the journal, held buttons count once
- [x] Learn mode: teach each button when the protocol is unknown (no lights)
- [x] Receiver tab with live presses, raw reports and log export; `hid-monitor` CLI
- [ ] First test with the real receiver (`docs/hardware-test.md`)
- [ ] Vendor protocol from the SDK: buzzer ID, press time, lights, battery (dome and RGB)
- [~] Buzzer lights: ready, locked out, first, correct, wrong; game sends them, encoding waits for the protocol
- [~] Buzzer registration (any press registers in the lobby) and a buzzer test grid (list view done)
- [ ] Fallback: vendor keystroke mode through the keyboard driver

### M3: Authoring (week 2-3)
- [x] Editor in the reference's layout: ribbon, thumbnails, slide canvas, properties panel, Edit question form, table view, File page; undo, crash-safe draft, test run
- [x] Pictures, videos and backgrounds per slide on the projector; slide sounds from the Director (on slide, with the clock, on demand)
- [ ] Excel import of questions
- [ ] Show settings: team names, sounds, welcome screen, end-of-question actions

### M4: Show polish (week 3-4)
- [ ] Sounds, countdown, transitions, branding of the welcome screen
- [x] Design templates: five built-in themes (colours, fonts, a background per screen) chosen on the editor's Design tab with live preview
- [x] Czech and English UI (switchable in Settings)
- [ ] Should-haves from `features.csv` that the first event needs

### M5: Extensions (after the first event)
- [ ] Theme packages, background gallery, round and quiz backgrounds (`docs/specification.md` section 5)
- [ ] Plugins: mini games and integrations (`docs/specification.md` section 6)

## Phase 4: Test

- [ ] Unit tests for scoring and the state machine
- [ ] Simulated show with 50 virtual buzzers
- [ ] Crash drill: kill the app mid-question, restart, continue
- [ ] Rehearsal on the event laptop with the projector and real buzzers
- [ ] Parity check against `features.csv`

## Phase 5: Run

- [~] Installers for Windows, macOS and Linux from CI (unsigned); code signing and a portable build to do
- [ ] Event checklist: laptop power, display settings, receiver, spare batteries, backup quiz file
- [ ] First live event
- [ ] Retrospective: what to fix and what to build next
