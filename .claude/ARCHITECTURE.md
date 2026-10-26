# Buzzdome architecture (for AI coding assistants)

A map of the codebase for an assistant that has never seen it. It says what
each part owns, which rules hold everywhere, and where to look next. The
full design, with the reasons behind each choice, is
[`docs/design.md`](../docs/design.md); requirements with IDs are in
[`docs/specification.md`](../docs/specification.md). When this file and
those disagree, the docs and the code win: fix this file.

## What the app is

A desktop quiz show for live events. The quizmaster runs it on one laptop
with two windows:

- **Director**: the control window on the laptop screen. Runs the show,
  edits scores, shows the buzzers, and hosts the quiz editor and settings.
- **Projector** (host screen): full screen on the projector. It only shows
  the game state and is never its source.

Players answer only with wireless buzzers (Enjoy 303S, A-D plus a big
button, through a USB-HID receiver), or with keyboard and on-screen
buzzers when there is no hardware. There is no network, no server and no
phone client. Stage lights (DMX, Art-Net, sACN) and show messages (OSC,
MIDI) follow the show.

## One process, three layers

```text
React UI (app/src)              Director and projector windows
   │  api.ts: invoke(command)        ▲ BackendEvents: game-state, clock-tick, ...
   ▼                                 │
Tauri shell (app/src-tauri)     owns all state, I/O, threads, the journal
   │  Command                        ▲ GameView, Effects
   ▼                                 │
quiz-core (crates/quiz-core)    pure game state machine, rules, scoring, view
```

- **`quiz-core` decides, everything else carries.** Every change to a show
  is a timestamped `Command` applied to `Game` (`game.rs`), which returns
  `Effect`s (sound cues, lights). `Game::view` builds the `GameView` both
  windows draw. The crate has no UI or I/O dependencies.
- **The shell persists before it shows.** `app/src-tauri/src/lib.rs`
  (`execute_at`) applies a command, appends it to the session journal
  (`session.rs`, `sessions/<start ms>/` in the OS app-data folder), and only
  then emits the new view. After a crash the show is rebuilt by replaying
  the journal.
- **The UI only asks.** Components call the backend through `api` and
  listen through `BackendEvents` (both in `app/src/api.ts`), wrapped by the
  hooks in `app/src/hooks.ts`. Game actions from buttons, keys, on-screen
  and real buzzers all become the same `dispatch(Command)`.

## Crates and folders

| Path | Owns | Notes |
| --- | --- | --- |
| `crates/quiz-core` | quiz model (`model.rs`, `SCHEMA_VERSION`), game state machine (`game.rs`, `RULES_VERSION`), view, scoring, themes, styles, spreadsheet rows, the stage (the show's moment the lights follow, `stage.rs`), sound board | Pure. Writes `app/src/bindings` (ts-rs) when tested. |
| `crates/quiz-file` | `.quiz` files: ZIP with `quiz.json` and `media/`, unpack cache, atomic save, spreadsheet import and CSV export | Format: `docs/quiz-format.md`. |
| `crates/buzzers` | `BuzzerDriver` trait, USB-HID receiver thread, protocols (`enjoy303s.rs` over `cp2110.rs`, `learned.rs`), edge filter, fastest-finger timer order, show clock, fake receiver for E2E | Read `replica/hardware.md` and `docs/design.md` §2 first. |
| `crates/lights` | stage lights: setup checks, looks (`look.rs`), desk priority and fades, DMX frame formats, outputs, OSC and MIDI, the lights thread | A view of the show, never journaled (`docs/design.md` §7). |
| `crates/quiz-ai` | quiz writing for AI assistants: JSON Schema, quiz folders, MCP server, skill package, the `buzzdome-quiz` CLI | `docs/ai-assistants.md`. |
| `crates/show-sim` | random quizzes and shows for soak tests | Used by CI nightly. |
| `app/src-tauri` | Tauri shell: commands, windows, show thread and event order (`show_thread.rs`, `pump.rs`), journal, editor draft, receiver setup, media player, themes, fonts, lights setup, log, crash drills | Module list in `CLAUDE.md`. |
| `app/src` | React UI: `director/`, `host/` (slides), `editor/` (Studio), `theme/`, `audio/`, shared helpers | Conventions in `CLAUDE.md`. |
| `app/e2e` | E2E tests of the real app through WebDriver | Linux locally, Windows on CI only. |
| `app/src-tauri/themes/<id>/` | built-in theme packages (`theme.json`, fonts, pictures) | |
| `samples/demo-quiz.json` | the demo quiz the app ships | |
| `docs/` | our own specification, design, quiz format, AI assistants, hardware test | The reference manual copy there is gitignored. |
| `replica/` | recon of the reference app (screens, flows, scoring), feature matrix, buzzer research | `replica/sources/` and `replica/screens/` are gitignored. |

## Threads

- **Show thread** (`show_thread.rs`): the only thread that changes a running
  show. Receiver events and the windows' show commands reach it through one
  queue, in arrival order. It applies due timer commands and sends
  `clock-tick`. Windows never wait on a journal write.
- **Receiver thread** (`crates/buzzers/src/receiver.rs`): owns the open HID
  device, decodes presses with the active protocol, writes modes and
  lights. Others talk to it through `ReceiverHandle`.
- **Lights thread** (`crates/lights/src/engine.rs`): sends a frame every
  25 ms; the show thread only pushes to its channel.
- Async Tauri commands that read or start a show, write files or list USB
  devices run off the windows' thread. Lock order and the reasons are in
  `docs/design.md` §1.1 (Threads).

## Invariants (do not break these)

1. **Persist before show.** No state change reaches a window before it is
   in the journal. A show resumes after a crash or restart.
2. **Rules live in `quiz-core`.** Points, correctness, which step commands
   apply now (`Game::controls` → `GameView.controls`), where Next goes
   (`Game::following`). The UI offers only what `controls` allows.
3. **Replays stay faithful.** A rule change that alters how a journal
   replays bumps `RULES_VERSION` and gates the old behaviour on
   `Game.rules_version`, so older shows replay as they were played.
4. **The quiz format is a contract.** Quizzes are written by a colleague
   and by AI assistants. Change it only in a versioned way
   (`SCHEMA_VERSION`, `docs/quiz-format.md`, the JSON Schema in
   `crates/quiz-ai`).
5. **Fastest-finger order comes from the hardware** (the buzzers' own
   timers or the receiver's order), never re-ordered by the PC.
6. **Usable without hardware.** Keyboard and on-screen buzzers always work.
7. **One source for shared types.** Rust types cross to TypeScript only
   through ts-rs into `app/src/bindings`; never edit the bindings by hand.
   UI constants come from `bindings/Constants.ts`.
8. **No English sentences from Rust.** Errors are `AppError` codes with
   parameters, quiz problems are `Problem` codes; the UI words them in
   Czech or English (`app/src/messages.ts`, `app/src/i18n.ts`).
9. **Each window gets only its commands.** A new Tauri command is listed in
   `build.rs`, `generate_handler!` and the window capabilities; a test
   keeps them in step.

## Where to change what

| Task | Start here | Skill |
| --- | --- | --- |
| Check a change before a PR | the commands table in `CLAUDE.md` | `buzzdome-verify` |
| Add or change a backend command | `app/src-tauri/build.rs`, `src/lib.rs`, `capabilities/`, `app/src/api.ts` | `buzzdome-backend-command` |
| Change a game rule or the quiz format | `crates/quiz-core/src/game.rs`, `model.rs`, `docs/quiz-format.md` | `buzzdome-game-rules` |
| Change a screen, the editor or a text | `app/src`, `i18n.ts`, `editor/slides.ts`, `host/SlideScreen.tsx` | `buzzdome-ui-change` |
| Buzzers, receiver or stage lights | `crates/buzzers`, `crates/lights`, `docs/hardware-test.md` | `buzzdome-hardware` |
| Write or check a quiz file | `crates/quiz-ai`, `docs/quiz-format.md` | `buzzdome-quiz-authoring` |

The skills are in `.claude/skills/<name>/SKILL.md`. Assistants that do not
load skills can read them as plain checklists.

## Testing layers

- `cargo test --workspace`: Rust unit and integration tests, including a
  game-flow suite, 40 random shows (`show-sim`) and 40 crash drills through
  the real journal. Also regenerates the bindings.
- `npm test` in `app/`: vitest for UI logic (editor history, draft queue,
  slide changes, keys, colours, audio maths).
- `npm run e2e` in `app/`: the real app driven through `tauri-driver`
  (Linux; Windows on CI only, because it wipes the real app data).
- UI preview in a browser with a mocked backend: `cd app && npm run dev`,
  then open `/preview.html?window=<director|host>&scene=<name>`
  (scenes in `app/src/dev/preview.tsx`).
- CI: `.github/workflows/build.yml` (check, Windows tests, installers),
  `e2e.yml`, `nightly.yml` (30 000 random shows, 10 000 crash drills, E2E
  repeated).

## Boundaries

- **Clean room.** Behaviour is rebuilt from `replica/recon.md` and the
  reference manuals. No code, text, screenshots, sounds, skins, icons or
  example quizzes of the reference app, and no vendor SDK file, go into the
  repository. Third-party files are bundled only under their own licences
  (OFL fonts in the themes, the Open Fixture Library data in
  `crates/lights/data/`). The reference manual copies are gitignored and
  must stay so: every path `.gitignore` lists for the manual (`docs/`
  subfolders, `docs/index.md`, `docs/solo.md`, `docs/tips-and-tricks.md`,
  `replica/sources/`, `replica/screens/`). Never quote them into our docs.
- **No trademarked names** in the product or UI (see `CLAUDE.md`).
- **Languages.** UI in Czech (default) and English; all code, comments,
  commits and docs in English.
