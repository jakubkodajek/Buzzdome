# Buzzdome

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

The app is called Buzzdome (decided 2026-10-10). Its logo is a top view of a
dome buzzer in six colours, orange by default (`app/src-tauri/icons`). The
bundle identifier `cz.kodajek.quizshow` and the `quiz-show.*` preference keys
keep the working title, so saved settings and data stay where they are.

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
| install UI deps (as locked; `npm install <pkg>` only to add or upgrade one) | `cd app && npm ci` |
| run the app (dev) | `cd app && npm run tauri dev` |
| core + driver tests (also regenerates `app/src/bindings`) | `cargo test --workspace` |
| lint Rust | `cargo clippy --workspace --all-targets` |
| lint UI (incl. React hooks rules) | `cd app && npm run lint` |
| type-check and build UI | `cd app && npm run build` |
| UI logic tests (editor history, draft queue, slide changes) | `cd app && npm test` |
| E2E tests of the real app (Linux; needs `tauri-driver`, `webkit2gtk-driver`, `xvfb`, `xdotool`. Windows only on CI: they use the real app data) | `cd app && npm run tauri build -- --debug --no-bundle && xvfb-run -a npm run e2e` |
| random shows / crash drills (more, or replay one) | `SOAK_SHOWS=5000 cargo test --release -p show-sim`; `SOAK_SEED=<n> cargo test -p buzzdome crash_drill` |
| UI preview in a browser (mocked backend) | `cd app && npm run dev`, then `/preview.html?window=host&scene=running` (scenes in `app/src/dev/preview.tsx`) |
| nightly soak and E2E | CI: `.github/workflows/nightly.yml` every night main changed, and on "Run workflow" |
| installers | CI: `.github/workflows/build.yml` on tags `v*` and "Run workflow"; artifacts `buzzdome-windows`, `buzzdome-macos`, `buzzdome-linux` |
| quiz files for scripts and AI assistants (check, pack, unpack, schema, skill, MCP) | `cargo run -p quiz-ai -- <command>` (`docs/ai-assistants.md`) |
| receiver probe (no app) | `cargo run -p buzzers --example hid-monitor [-- VID:PID]` |
| build installer | `cd app && npm run tauri build` (each OS builds its own installer) |

Layout: `crates/quiz-core` (model, game state machine, scoring, view),
`crates/show-sim` (random quizzes and shows for the soak tests),
`crates/quiz-file` (`.quiz` files: ZIP with media, unpack cache, save;
`sheet.rs` spreadsheet import and CSV export),
`crates/quiz-ai` (quiz writing for AI assistants: the quiz JSON Schema, `workbench.rs` quiz folders,
`mcp.rs` the MCP server, `roots.rs` the folders it may use, `skill.rs` the skill package, `claude_desktop.rs`,
the `buzzdome-quiz` command line),
`crates/buzzers` (driver trait, USB-HID receiver thread, protocols, learn mode, `clock.rs` show clock, `fake_receiver.rs` the E2E tests' fake receiver),
`crates/lights` (stage lights: DMX outputs, looks, the lights thread, OSC and MIDI show messages),
`app/src-tauri` (Tauri shell, session journal, `show_thread.rs` and `pump.rs` the show thread and its
event order, `app_windows.rs` the Director and projector windows, `remote.rs` what the quizmaster's remote keys do, `app_log.rs` the log file, `editor.rs` draft, `sheet_reader.rs` spreadsheet reader process, `hardware.rs` receiver setup,
`teams.rs` prepared team names, `media_player.rs` media library, player, volume and sound board,
`themes.rs` built-in themes, `fonts.rs` installed fonts, `app_file.rs` settings files,
`app_icon.rs` the windows' icon colour, `stage_lights.rs` the lights setup and what the show tells them,
`fixture_library.rs` the fixture library in use and its download, `ai.rs` `--mcp` and File › AI assistants),
`app/src` (React UI: `director/`, `host/`, `editor/`; `browser-guard.ts` and `error-boundary.tsx`
keep the windows from acting as a browser or going blank), `app/e2e` (E2E tests through WebDriver),
`samples/demo-quiz.json`.
Show sessions are journaled under the OS app-data dir in `sessions/<start ms>/`.

UI conventions (keep one way of doing each thing):
- Types shared with Rust are generated by ts-rs into `app/src/bindings` and
  re-exported from `app/src/types.ts`. Change the Rust type, never the bindings.
  Core constants the UI needs (answer limits, question defaults and limits,
  final steps, style bounds) are written to `bindings/Constants.ts` by a quiz-core test;
  never copy one.
  Game rules and derived values (points, correctness) live in `quiz-core`.
- Backend calls go through `api` and events through `BackendEvents` in
  `app/src/api.ts`; components use the hooks in `app/src/hooks.ts`.
- Errors from backend calls go through `useRun()` / `useReport()`, game commands
  through `useDispatch()` (`app/src/app-context.tsx`); language and `t` come
  from `useApp()`. Rust never sends the UI an English sentence: errors are
  `AppError` codes with parameters (`app/src-tauri/src/error.rs`), quiz
  problems `Problem` codes, worded in `app/src/messages.ts` from `i18n.ts`.
- Which step commands apply now comes from `GameView.controls` (`Game::controls`);
  buttons and keys offer only those.
- Shared helpers: `answers.ts` (buttons, letters), `format.ts` (times, scores),
  `quiz.ts` (slide titles, question numbers), `keyboard.ts` (shortcuts, key
  hints), `messages.ts` (backend codes in words), `store.ts` (a value several
  components share outside React state), `theme/color.ts` (reading, writing
  and converting the quiz file's colours).
- Slides are drawn only by `host/SlideScreen.tsx` in a `host/Stage.tsx`
  (projector, editor canvas, thumbnails via `host/SlideThumbnail.tsx`).
  Their look comes from the quiz's theme (a `Theme` that Rust resolves,
  `list_themes`, given by `ThemeContext`; `theme/themes.ts` turns it into
  CSS): stage CSS takes colours and fonts only from `--theme-*` variables,
  never a written colour that a theme should decide. The one exception is a
  slide's own style: `theme/slide-style.ts` turns it into `--style-*`
  variables that `host/slide.css` reads over the theme's look, so no theme
  value is written twice. Built-in themes are
  packages in `app/src-tauri/themes/<id>/` (`theme.json` and files).
- Editor: a slide changes only through the pure functions in
  `editor/slides.ts`; editor commands are `Action`s in the one record built in
  `editor/EditorScreen.tsx` (ribbon, menus and keys read it); slide fields are
  rows from `editor/properties.tsx` and, for a slide's look,
  `editor/style-properties.tsx` (panel, ribbon and table view share them),
  each built with the builder for its kind in `editor/row-builders.tsx`;
  `editor/property-pages.ts` decides which rows a selection shows. A
  category is edited as its sample slide (`editor/categories.ts`), so it
  uses the slide's rows too.

## Where things live

- `ROADMAP.md`: phases, milestones and what is done.
- `docs/specification.md`: requirements with IDs (buzzer link, themes, plugins).
  `docs/design.md`: architecture, buzzer stack. `docs/quiz-format.md`: the quiz
  file format. `docs/ai-assistants.md`: quizzes written by AI assistants. `docs/hardware-test.md`:
  checklist for the real receiver. Jakub keeps a copy of the reference manual
  in `docs/` locally; it is gitignored and must never be committed.
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
  buzzer SDK comes from the manufacturer or not at all. One exception, Jakub's
  decision of 2026-10-20 for this private, non-distributed app: Enjoy's
  protocol may be read from an unofficial copy of its SDK too. The Enjoy 303S
  driver implements a description written in our own words from Enjoy's
  official 303S kit (Jakub's, 2026-10-21) and that copy (`docs/design.md`
  2.4). No file or code from either goes into the repository.
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
