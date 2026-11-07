# Buzzdome

Canonical instructions for every AI coding agent working in this repository
(Claude Code, OpenAI Codex, GitHub Copilot, Cursor, Gemini, ...).
`CLAUDE.md` imports this file and adds only Claude-specific notes;
`.github/copilot-instructions.md` points here. Edit this file, not copies.

A desktop quiz and game-show app for Jakub's own non-profit events, played with
wireless hardware buzzers. It replaces Windows-only desktop game-show software;
QuizXpress 8.5 is the reference app. This project is independent and not
affiliated with it. Goal: behave like the reference app for the formats we use.

## Status

Scope changed on 2026-10-05 from a browser app with phones to a desktop app
with hardware buzzers. Target: first live event in
November 2026. Plan and progress: `ROADMAP.md`.

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
| checked voice libraries for a local Rust build (once; `crates/voice` links sherpa-onnx, `docs/releasing.md`) | `.github/scripts/sherpa-onnx-libs.sh .`, then export `SHERPA_ONNX_ARCHIVE_DIR` as the folder it names |
| lint Rust | `cargo clippy --workspace --all-targets` |
| lint UI (incl. React hooks rules) | `cd app && npm run lint` |
| type-check and build UI | `cd app && npm run build` |
| UI logic tests (editor history, draft queue, slide changes) | `cd app && npm test` |
| E2E tests of the real app (Linux; needs `tauri-driver`, `webkit2gtk-driver`, `xvfb`, `xdotool`. Windows only on CI: they use the real app data) | `cd app && npm run tauri build -- --debug --no-bundle && xvfb-run -a npm run e2e` |
| random shows / crash drills (more, or replay one) | `SOAK_SHOWS=5000 cargo test --release -p show-sim`; `SOAK_SEED=<n> cargo test -p buzzdome crash_drill` |
| UI preview in a browser (mocked backend) | `cd app && npm run dev`, then `/preview.html?window=host&scene=running` (scenes in `app/src/dev/preview.tsx`) |
| nightly soak and E2E | CI: `.github/workflows/nightly.yml` every night main changed, and on "Run workflow" |
| test installers (unsigned) | CI: `.github/workflows/build.yml` on "Run workflow"; artifacts `buzzdome-windows`, `buzzdome-macos`, `buzzdome-linux` |
| release (signed, offered to installed apps) | push an annotated tag `v<version>` of main: `.github/workflows/release.yml` (`docs/releasing.md`) |
| the app as an AI app's MCP server (no window; folder and instructions from `ai.json` in the app's data) | `target/debug/buzzdome --mcp` (`docs/ai-assistants.md`) |
| receiver probe (no app) | `cargo run -p buzzers --example hid-monitor [-- VID:PID]` |
| build installer | `cd app && npm run tauri build` (each OS builds its own installer) |
| agent files in sync (after editing `.claude/agents` or `.claude/skills`) | `python3 scripts/sync_agents.py` (`--check` in CI) |

`make setup`, `make lint`, `make test`, `make check`, `make e2e`, `make dev`,
`make preview`, `make soak` and `make layout` (`Makefile`) run the same
commands in the right order (the UI is built before Rust compiles, the voice
libraries are fetched and `SHERPA_ONNX_ARCHIVE_DIR` is set). `make check` is
what CI's `check` job runs, plus the agent-config check; show its output
before calling a change done.

Layout: `crates/quiz-core` (model, game state machine, scoring, view, results, welcome screen),
`crates/show-sim` (random quizzes and shows for the soak tests),
`crates/quiz-file` (`.quiz` files: ZIP with media, unpack cache, save;
`sheet.rs` spreadsheet import and CSV export),
`crates/quiz-ai` (quiz writing for AI assistants: the quiz JSON Schema, `workbench.rs` quiz folders,
`mcp.rs` the MCP server, `roots.rs` the folders it may use, `settings.rs` the folder and instructions the app keeps,
`assistants.rs` connecting AI apps),
`crates/buzzers` (driver trait, USB-HID receiver thread, protocols, light colours, learn mode, `clock.rs` show clock, `fake_receiver.rs` the E2E tests' fake receiver),
`crates/voice` (reading questions aloud offline: voice model download check, sherpa-onnx engine, the voice process's protocol),
`crates/lights` (stage lights: DMX outputs, looks, the lights thread, OSC and MIDI show messages),
`app/src-tauri` (Tauri shell, `session.rs` session journal, `crash_drill.rs` crash drills, `show_thread.rs` and `pump.rs` the show thread and its
event order, `app_windows.rs` the Director and projector windows, `remote.rs` what the quizmaster's remote keys do, `app_log.rs` the log file, `editor.rs` draft,
`dialogs.rs` file pickers, `recent_files.rs` recent quiz files, `sheet_reader.rs` spreadsheet reader process, `hardware.rs` receiver setup,
`teams.rs` prepared team names, `media_player.rs` media library, player, volume and sound board,
`themes.rs` built-in themes, `fonts.rs` installed fonts, `app_file.rs` settings files,
`app_icon.rs` the windows' icon colour, `stage_lights.rs` the lights setup and what the show tells them,
`fixture_library.rs` the fixture library in use and its download, `ai.rs` `--mcp` and File › AI assistants,
`updates.rs` checking for and installing app updates, `read_aloud.rs` the voice model and the voice process),
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
  from `useApp()`, and so do the quizmaster's messages: `notify` (green),
  `caution` (amber) and `fail` (red), all in the one plain shape of
  `app/src/notice.tsx` (tone dot, text, detail; the not-saved banner is a
  red bar without the dot). Icons are the SVG icons
  in `app/src/icons.tsx`, never typed glyphs. Rust never sends the UI an English sentence: errors are
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

- `docs/architecture.md`: the architecture in one page, its invariants
  and where to change what. Read it before any non-trivial change.
- `memory/`: durable project memory shared by people and agents
  (`memory/README.md`): `project-context.md` (goals, users, constraints,
  glossary), `features.json` (open work toward the next event),
  `decisions.md`, `lessons-learned.md`, `progress.md` (session log).
- `.claude/skills/buzzdome-*`: task checklists (verify, backend command,
  game rules, UI change, layout, hardware). `.claude/skills/{feature,review,bug-hunt,ux-audit,adr,handoff}`:
  the multi-agent workflow. `.claude/agents/`: the subagents
  (`docs/agent-workflow.md`). Agents that do not load skills read them as
  plain checklists.
- `ROADMAP.md`: phases, milestones and what is done.
- `docs/specification.md`: requirements with IDs (buzzer link, themes, plugins).
  `docs/design.md`: architecture, buzzer stack. `docs/quiz-format.md`: the quiz
  file format. `docs/ai-assistants.md`: quizzes written by AI assistants. `docs/hardware-test.md`:
  checklist for the real receiver. `docs/releasing.md`: how versions are
  released and how installed apps update. Jakub keeps a copy of the reference manual
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
- Never commit the reference manual copies or any vendor SDK file. The
  manual copies are every path `.gitignore` lists for it: the `docs/`
  subfolders except our `docs/adr/`, `docs/index.md`, `docs/solo.md`, `docs/tips-and-tricks.md`,
  `replica/sources/` and `replica/screens/`. They are not our docs; never
  quote them into ours (`.claude/hooks/protect_files.py` refuses edits there).
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
- Installed apps update themselves from GitHub releases of this repository, so
  only a release reaches them. To release: raise `version` in
  `app/src-tauri/tauri.conf.json`, merge to main, wait for `check` and
  `windows-tests`, then push an annotated tag of that commit,
  `git tag -a v<version> -m "<notes in Czech>" -m "[en]" -m "<notes in English>" origin/main && git push origin v<version>`,
  and Jakub approves the run (Review deployments, environment `release`).
  The tag's message is what the quizmaster reads before updating, in the
  app's language: Czech, then a line `[en]`, then English.
  `release.yml` builds, signs and publishes; never make the release or the
  tag on GitHub's Releases page, and never push a tag without Jakub's
  go-ahead (`docs/releasing.md`).

## Agent workflow

The full protocol is `docs/agent-workflow.md`. In short:

1. **Explore, plan, implement, verify, review.** Read the relevant code and
   `docs/architecture.md` first. For anything larger than a small fix, write
   a plan in `.work/<task-slug>/plan.md` (gitignored) with acceptance
   criteria that a test or command can check.
2. **One feature at a time.** Take one item from `memory/features.json` (or
   one `ROADMAP.md` line), finish and verify it before starting another.
3. **Evidence over assertion.** Done means `make check` (or the
   `buzzdome-verify` checks that apply) passed and the output was shown.
   Screen changes also need a look in the browser preview; layout or CSS
   changes the `buzzdome-layout` check.
4. **Never weaken tests.** Do not delete, skip, `#[ignore]` or loosen a test
   or assertion to get green, and never change a shipped journal line in
   `crates/quiz-core/tests/journal.rs`.
5. **Separation of duties.** The agent that wrote code does not approve it.
   Reviews run in a fresh context (`/review`), and an independent verifier
   filters false positives before fixes. Agents never merge, release or
   push a tag.
6. **Update memory before you stop** (`/handoff`): an entry in
   `memory/progress.md`, new decisions in `memory/decisions.md` (or an ADR
   in `docs/adr/`), repeated mistakes in `memory/lessons-learned.md`, and
   `ROADMAP.md` / `replica/features.csv` where a feature landed.
7. **Small commits** with Conventional Commit prefixes (`feat:`, `fix:`,
   `test:`, `docs:`, `refactor:`, `chore:`). PRs fill in
   `.github/pull_request_template.md`; reviewers follow `REVIEW.md`.
