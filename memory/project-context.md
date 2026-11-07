# Project context

Loaded into every Claude session (`CLAUDE.md`). Keep it under ~60 lines; the
details live in `AGENTS.md`, `docs/architecture.md` and `docs/design.md`.

## Product
- **One-liner:** a desktop quiz show for live events: questions on the
  projector, players on wireless buzzers, the quizmaster in control from one
  laptop.
- **Problem it solves:** Jakub runs non-profit quiz nights and needs his own
  replacement for Windows-only game-show software, with his Enjoy 303S RGB
  buzzers, that cannot fail in front of a room.
- **People:** the quizmaster (Jakub) at the Director window under show
  pressure, often in a dark hall; a colleague who writes quizzes (in the
  editor, in spreadsheets, or with an AI app through the MCP server); up to
  ~50 buzzers (one per team) whose players see only the projector and their buzzer's light.

## Scope
- **In:** writing a quiz (Studio), registering teams, everyone-answers and
  fastest-finger questions, clock, sounds, media, themes, scores, results,
  stage lights, reading questions aloud offline, AI quiz writing, in-app
  updates from signed GitHub releases.
- **Out (now):** phones, PIN/QR joining, cloud, any network or server during
  a show; plugins and theme packages wait until after the first event.
- **Open work:** `memory/features.json` (next event) and `ROADMAP.md`; the
  full feature matrix is `replica/features.csv`.

## Constraints
- **Deadline:** first live event in November 2026, on a Windows laptop with a
  projector and the 303S receiver. Stability beats new features.
- **Stack:** Tauri 2; Rust (pinned in `rust-toolchain.toml`) owns all state;
  React + TypeScript (Vite) UI; Node.js 22.
- **Platforms:** Windows first (the event laptop), macOS for development,
  Linux for CI and E2E.
- **Live show safety:** every state change is journaled before it is shown;
  a crash resumes the show. Usable with keyboard and on-screen buzzers only.
- **Contracts:** the `.quiz` format (`SCHEMA_VERSION`) and journal replay
  (`RULES_VERSION`) stay backward compatible.
- **Clean room and names:** nothing from the reference app or a vendor SDK
  enters the repository; no trademarked show names in the UI.
- **CI budget:** GitHub Actions minutes are limited; be green locally first.
- **Releases:** only Jakub's go-ahead pushes a `v*` tag.

## Glossary (use these words)
- **Director**: the quizmaster's control window. **Projector** (host screen):
  the full-screen view; never the source of truth.
- **Studio**: the quiz editor inside the Director.
- **Show**: one run of a quiz; its **journal** is `sessions/<start ms>/` in
  the app-data folder. **Show thread**: the only thread that changes a show.
- **Command / Effect / GameView**: what changes a show, what it causes
  (sounds, lights), what the windows draw (`quiz-core`).
- **Everyone-answers** (A-D against the clock) and **fastest-finger** (first
  buzz answers aloud, the quizmaster judges) questions; **buzz queue** (BZ-23).
- **Light** states of a buzzer (ready, answered, waiting, locked, first,
  correct, wrong); **light colours** are the quizmaster's per computer.
- **Look**: a stage-lights state that follows the show (`crates/lights`).
- **Receiver**: the 303S USB receiver on the `10c4:1819` CP2110 cable;
  **remote**: the quizmaster's 12-key remote.
- Czech UI terms: the table in `.claude/skills/buzzdome-ui-change/SKILL.md`.
