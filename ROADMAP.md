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
- [x] First test with the real receiver: Enjoy 303S on Windows 11, buzzers 1 and 2, 2026-10-25 (`docs/hardware-test.md`, `docs/design.md` section 6)
- [~] Vendor protocol: Enjoy 303S (RGB, Jakub's set) from Enjoy's SDK in `crates/buzzers/src/enjoy303s.rs`: presses with each buzzer's timer, fastest finger in the timers' order; passed the first test, still to test with every buzzer in a room, high buzzer numbers and on macOS. Its SDK has no battery level, and no message in the test carried one. The RF215 (dome) driver was dropped, as Jakub's set is the 303S
- [~] Buzzer lights: ready, locked out, first, correct, wrong, in colour on the 303S; seen in the first test, still to read from the back of a room
- [~] Buzzer registration (any press registers in the lobby) and a buzzer test grid (list view done)
- [ ] Fallback: vendor keystroke mode through the keyboard driver

### M3: Authoring (week 2-3)
- [x] Editor in the reference's layout: ribbon, thumbnails, slide canvas, properties panel, Edit question form, table view, File page; undo, crash-safe draft, test run
- [x] Pictures, videos and backgrounds per slide on the projector; slide sounds from the Director (on slide, with the clock, on demand)
- [x] Sound trim on a waveform with a replay part after the reveal (SN-8), loudness normalization to a LUFS target (SN-9), background video options and preview (BG-4); quiz format v3
- [x] Video options and Play video (QZ-4), the Director's media player with a sound board and search (SN-7): docs gap items 4, 8, 9
- [x] Questions from Excel, OpenDocument and CSV sheets, and export to CSV (QZ-8): docs gap item 5
- [x] Properties panel features and several selected slides at once (QZ-1): docs gap item 7
- [x] Colour, gradient and font pickers with per-slide styles (QZ-9): docs gap item 7
- [x] Several right answers per question, each with a share of the points: quiz format v5
- [x] Categories with a background video, applied from the Design tab and named by a spreadsheet's `CAT` (QZ-10): docs gap item 6
- [x] Quizzes written by AI assistants: MCP server for Claude Desktop, skill package for chat assistants, `buzzdome-quiz` command line (QZ-12)
- [ ] Show settings: team names, sounds, welcome screen, end-of-question actions

### M4: Show polish (week 3-4)
- [ ] Sounds, countdown, transitions, branding of the welcome screen
- [x] Design templates: eleven built-in themes (colours, fonts, corners, heading case, a background per screen) chosen on the editor's Design tab with live preview
- [x] Clock designs: ring, bar, digital, hourglass or stopwatch, chosen per question
- [x] Czech and English UI (switchable in Settings)
- [ ] Should-haves from `features.csv` that the first event needs

### M5: Extensions (after the first event)
- [ ] Theme packages, background gallery, round and quiz backgrounds (`docs/specification.md` section 5)
- [ ] Plugins: mini games and integrations (`docs/specification.md` section 6)
- [x] Stage lights over DMX (USB interfaces, Art-Net, sACN) and show messages over OSC and MIDI (`docs/specification.md` section 12); to try with real fixtures

## Phase 4: Test

- [ ] Unit tests for scoring and the state machine
- [x] Simulated show with 50 virtual buzzers (`crates/show-sim`: random quizzes and shows checked against the game rules; 30 000 more every night in `.github/workflows/nightly.yml`)
- [~] Crash drill: kill the app mid-question, restart, continue (automated as run, crash and resume tests in `app/src-tauri/src/lib.rs`, random crash drills with torn journal lines in `crash_drill.rs`, and the real app killed and resumed in `app/e2e/resume.e2e.ts`; the drill on the event laptop to do)
- [x] End-to-end tests of the real app (`app/e2e`): a whole show, every Director and editor button, on every push (Linux)
- [ ] Rehearsal on the event laptop with the projector and real buzzers
- [ ] Parity check against `features.csv`

## Phase 5: Run

- [~] Installers for Windows, macOS and Linux from CI (unsigned); code signing and a portable build to do
- [ ] Event checklist: laptop power, display settings, receiver, spare batteries, backup quiz file
- [ ] First live event
- [ ] Retrospective: what to fix and what to build next
