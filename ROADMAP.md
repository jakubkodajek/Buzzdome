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
- [ ] Identify the colleague's buzzer model and the cable's USB VID/PID (photos, Device Manager)
- [ ] Buzzer protocol: official SDK from the manufacturer, or captures of the USB traffic
- [ ] Re-prioritise `features.csv` for the desktop scope (phone rows to skip, buzzer rows to must)

## Phase 2: Design

- [x] Stack: Tauri 2, Rust core, React + TypeScript UI
- [~] Quiz file format v1: JSON model done (`crates/quiz-core/src/model.rs`); ZIP with `media/` and `docs/quiz-format.md` to do
- [x] Game state machine and scoring rules, documented next to the code
- [x] Persistence and crash recovery: append-only command journal, replayed on resume
- [~] `BuzzerDriver` trait: keyboard and virtual done, hardware receiver to do
- [ ] Screen designs: host screen (question, fastest finger, judged, leaderboard, final), Director, editor

## Phase 3: Build

### M0: Skeleton (week 1)
- [x] Tauri app with Director window and a full-screen Host window on a chosen monitor
- [~] Pure Rust core crate with unit tests (done); CI build for Windows (to do)
- [x] Keyboard and on-screen virtual buzzers feeding the same event stream

### M1: Playable core (week 1-2)
- [~] Load a quiz file; billboard, multiple-choice (A-D) and end-of-round slides (first version, needs a click-through)
- [~] Everyone-answers questions: clock, answered count, reveal, points (first version)
- [~] Fastest-finger questions: first buzz, right/wrong from the Director, lockout, clock resume (first version)
- [~] Leaderboard and final scores with top-3 reveal (first version); round scores to do
- [~] Teams table: names per buzzer, edit scores, enable/disable (first version)
- [~] Crash recovery: resume the last show from the start screen (first version)

### M2: Real buzzers (as soon as the protocol is known)
- [ ] Hardware driver: presses with buzzer ID and order/time
- [ ] Buzzer lights: ready, locked out, first, correct, wrong (colours on RGB models)
- [ ] Buzzer registration screen and a buzzer test tool
- [ ] Fallback: vendor keystroke mode through the keyboard driver

### M3: Authoring (week 2-3)
- [ ] Editor: slide list (add, reorder, duplicate, delete), question form, answers, correct answer, time, points, picture, notes
- [ ] Excel import of questions
- [ ] Show settings: team names, sounds, welcome screen, end-of-question actions

### M4: Show polish (week 3-4)
- [ ] Sounds, countdown, transitions, branding of the welcome screen
- [ ] Czech and English UI
- [ ] Should-haves from `features.csv` that the first event needs

## Phase 4: Test

- [ ] Unit tests for scoring and the state machine
- [ ] Simulated show with 50 virtual buzzers
- [ ] Crash drill: kill the app mid-question, restart, continue
- [ ] Rehearsal on the event laptop with the projector and real buzzers
- [ ] Parity check against `features.csv`

## Phase 5: Run

- [ ] Windows installer and a portable build
- [ ] Event checklist: laptop power, display settings, receiver, spare batteries, backup quiz file
- [ ] First live event
- [ ] Retrospective: what to fix and what to build next
