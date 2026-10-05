# Quiz show app (working title)

A desktop quiz and game-show app for Jakub's own non-profit events, played with
wireless hardware buzzers. It replaces Windows-only desktop game-show software;
QuizXpress 8.5 is the reference app. This project is independent and not
affiliated with it. Goal: behave like the reference app for the formats we use.

## Status

Scope changed on 2026-10-05 from a browser app with phones to a desktop app
with hardware buzzers. Target: first live event in about four weeks
(early November 2026). Plan and progress: `ROADMAP.md`.

## Product shape

One desktop app, Windows first (the event laptop runs Windows), macOS for
development. Two windows on one machine:

- **Director**: the quizmaster's control window on the laptop screen. Runs the
  show, edits scores, shows buzzer status. Also hosts the quiz editor (Studio)
  and show settings.
- **Host screen**: full-screen window on the projector. A view of the game
  state, never the source of truth.

Players answer only with wireless buzzers (no phones, no network, no server).
Up to ~50 buzzers. Quizzes are written by a colleague, so the quiz file format
is a stable, versioned contract.

## Stack

- Tauri 2. Rust owns all state and logic; the UI is TypeScript + React (Vite).
- Rust crates: game model, scoring and session state machine in a pure crate
  with no UI or I/O dependencies; buzzer drivers behind a `BuzzerDriver` trait
  (keyboard, on-screen virtual buzzers, and the hardware receiver).
- Quiz file: one ZIP file with `quiz.json` (versioned schema) plus `media/`.
  Excel import of questions.

## Commands

Rust comes from Homebrew's keg-only `rustup`: put `/opt/homebrew/opt/rustup/bin`
on `PATH` first (macOS dev machine).

| task | command (from the repo root unless noted) |
| --- | --- |
| install UI deps | `cd app && npm install` |
| run the app (dev) | `cd app && npm run tauri dev` |
| core + driver tests | `cargo test --workspace` |
| lint Rust | `cargo clippy --workspace --all-targets` |
| type-check UI | `cd app && npx tsc --noEmit` |
| UI preview in a browser (mocked backend) | `cd app && npm run dev`, then `/preview.html?window=host&scene=running` (scenes in `app/src/dev/preview.tsx`) |
| Windows installer | CI: `.github/workflows/windows-build.yml` on every push, artifact `quiz-show-windows` |
| build installer | `cd app && npm run tauri build` (Windows build must run on Windows) |

Layout: `crates/quiz-core` (model, game state machine, scoring, view),
`crates/buzzers` (driver trait), `app/src-tauri` (Tauri shell, session journal),
`app/src` (React UI: `director/`, `host/`), `samples/demo-quiz.json`.
Show sessions are journaled under the OS app-data dir in `sessions/<start ms>/`.

## Where things live

- `ROADMAP.md`: phases, milestones and what is done.
- `replica/recon.md`: screens (S01..S42), flows (F01..F10), components, data model and
  scoring rules of the reference app. Read it before designing or building a feature.
  Its phone, PIN/QR and cloud parts are out of scope now.
- `replica/features.csv`: feature matrix with must / should / could. Set the `clone`
  column to `yes` or `partial` as features land.
- `replica/hardware.md`: what is known about the buzzers and receiver, and the plan
  to get their protocol. Read it before touching buzzer code.
- `replica/sources/manual-v8.5/` and `manual-v6.5/`: the reference manuals as Markdown,
  one file per section, with `index.md`. Page markers equal printed page numbers.
  Gitignored. Search these instead of the PDFs.
- `replica/screens/`: manual page renders of key screens, by screen ID. Gitignored.
- `.claude/skills/replica-*`: the original web-oriented build workflow. Use the
  parts that still fit (design, test, diff); the stack defaults there do not apply.

## Rules

- Clean room. Rebuild behaviour from the recon map and manuals. Never copy the
  reference app's code, text, screenshots, sounds, skins, icons or example quizzes.
  Never use decompiled code or vendor SDKs obtained from unofficial reposts; the
  buzzer SDK comes from the manufacturer or not at all.
- No trademarked names in the product or UI: not QuizXpress, Jeopardy!, Who Wants to
  Be a Millionaire?, Family Feud or Wheel of Fortune. Use generic names
  ("topic board", "ladder round", "survey round").
- Live events cannot fail mid-show: every state change is persisted to disk before
  it is shown, and the app resumes the show after a crash or restart.
- Fastest-finger order comes from the buzzer hardware (receiver order or press
  timestamps), never re-ordered by the PC. Document what the receiver reports.
- The app must stay usable without buzzer hardware: keyboard and on-screen
  virtual buzzers are always available.
- UI in Czech (default) and English. Text-answer matching must handle diacritics.
- English for all code, comments, commit messages, docs and identifiers.
