# Buzzdome

**A desktop quiz show for live events: questions on the projector, players
on wireless buzzers, the quizmaster in control from one laptop.**

Buzzdome runs a quiz night from start to finish. It covers writing the
quiz, registering the teams, the questions with their clock and sounds,
fastest-finger rounds, the scores and the final ranking. It works offline
on one laptop, with no server and no phones, and it is built so that a show
survives a crash: every step is saved before it is shown, and a restarted
app carries on where it stopped.

Windows first, with macOS and Linux too. Built with [Tauri 2](https://tauri.app)
(a Rust core and a React UI). The interface is in Czech and English.

> Buzzdome is an independent project. It is not affiliated with or endorsed
> by the makers of any other quiz or game-show software.

## Features

- **Two windows.** The **Director** on the laptop runs the show, edits
  scores and shows each buzzer's state, in a light or a dark look. The
  **projector** window shows the slides full screen on a second monitor.
  Before a show the Director checks the receiver, buzzers, projector,
  sound and disk space.
- **Results.** After the show, or during it, the Director lists the
  ranking with each round's points and every question's answers, exports
  them to CSV and prints them or saves a PDF.
- **Question types.** In *everyone answers* questions each team picks A-D
  against the clock. In *fastest finger* questions the first buzz answers
  aloud and the quizmaster judges it. Correct answers can carry shares of
  the points. There are also billboards, explanations, end-of-round scores
  and a final podium.
- **Wireless buzzers.** The app drives the Enjoy 303S RGB buzzers (up to 64
  per receiver) over their USB receiver. Fastest-finger order comes from
  each buzzer's own timer, and the buzzer lights show ready, locked, first,
  right and wrong in colour. The quizmaster's remote drives the Director.
  Keyboard and on-screen buzzers always work, with or without hardware.
- **Quiz editor (Studio).** Ribbon, slide thumbnails, a live slide canvas,
  a properties panel and a table view. Undo, a crash-safe draft and a test
  run are built in. Slides take pictures, video, sound with trimming and
  loudness normalization, categories, and per-slide styles.
- **Import and export.** Questions come in from Excel, OpenDocument and CSV
  sheets, and go out to CSV.
- **Themes.** Twelve built-in themes, five clock designs, and backgrounds
  per screen. A show can open on a welcome screen with the quiz's logo, an
  intro picture or video, and a footer.
- **Media.** A media player with a sound board for the quizmaster, and our
  own sound library with a choice of sound per moment of the show.
- **Stage lights.** DMX over USB interfaces, Art-Net and sACN, with looks
  that follow the show. OSC and MIDI messages go to other show software.
- **AI quiz writing.** An assistant can write a quiz for you. Claude
  Desktop connects to the app as an MCP server, and Claude, Gemini or
  ChatGPT can use a skill package the app saves. There is also a
  `buzzdome-quiz` command line (see [AI assistants](#ai-assistants)).
- **Crash-safe shows.** Every step is journaled to disk before it is
  shown. A crashed or closed show resumes from the start screen.

## Status

Under active development for a first live event in November 2026. The
303S driver passed its first test on real hardware on Windows 11
(2026-10-25). What remains is tracked in [`ROADMAP.md`](ROADMAP.md), and
the feature matrix is [`replica/features.csv`](replica/features.csv).

## Install

There are no public releases. CI builds installers on version tags `v*`
and on demand (GitHub › Actions › Build › Run workflow). Download them
from the run's artifacts:

| Artifact | Contents |
| --- | --- |
| `buzzdome-windows` | `.exe` installer (NSIS) |
| `buzzdome-macos` | universal `.dmg` |
| `buzzdome-linux` | `.deb` and `.AppImage` |

The builds are not code-signed yet:

- **Windows:** SmartScreen warns on first start. Choose *More info*, then
  *Run anyway*.
- **macOS:** open the app once, then go to System Settings › Privacy &
  Security › *Open Anyway*. Allow Input Monitoring if macOS asks, so the
  app can read the buzzer receiver.
- **Linux:** to open the receiver without root, install the udev rule that
  the Director's Receiver tab shows.

## Running a show

1. Open a quiz on the Director's start screen. You can pick a recent quiz
   file, *Open quiz…*, or the demo quiz.
2. Open the projector window. It goes full screen on the monitor chosen in
   Settings.
3. Plug in the buzzer receiver. The Receiver tab finds it by itself and
   lists each buzzer as it is pressed. Without one, use the keyboard or the
   on-screen buzzers.
4. Register the teams. Any press registers a buzzer in the lobby, and team
   names can be prepared beforehand.
5. Run the show with *Next* and the keys or the remote. The Director offers
   only the steps that apply at that moment.

The hardware checklist for the real receiver is
[`docs/hardware-test.md`](docs/hardware-test.md).

## Quiz files

A quiz is a single `.quiz` file: a ZIP with a versioned `quiz.json` and a
`media/` folder. The format is a stable, documented contract, described in
[`docs/quiz-format.md`](docs/quiz-format.md). A complete example is
[`samples/demo-quiz.json`](samples/demo-quiz.json).

## AI assistants

| Who | How |
| --- | --- |
| Quizmasters using **Claude Desktop** | In Buzzdome, File › AI assistants › *Connect Claude Desktop*. The app becomes Claude's MCP server, limited to one folder. |
| Quizmasters using **Claude, Gemini or ChatGPT** in a browser or their apps | File › AI assistants › *Save the AI skill*, then upload the ZIP to the assistant as a skill. |
| Scripts and coding assistants | `cargo run -p quiz-ai -- check \| pack \| unpack \| schema \| skill \| mcp` |
| AI coding assistants working **on this codebase** | Start with [`CLAUDE.md`](CLAUDE.md) (or [`AGENTS.md`](AGENTS.md)), then [`.claude/ARCHITECTURE.md`](.claude/ARCHITECTURE.md). Task checklists are in [`.claude/skills/`](.claude/skills). |

Details are in [`docs/ai-assistants.md`](docs/ai-assistants.md).

## Development

### Prerequisites

- **Rust** at the release pinned in [`rust-toolchain.toml`](rust-toolchain.toml).
  rustup fetches it by itself.
- **Node.js 22.**
- **Windows:** Git for Windows, Rust with rustup (MSVC toolchain), and the
  Microsoft C++ Build Tools with *Desktop development with C++*. WebView2
  ships with Windows 11, and with Windows 10 from version 1803. The
  receiver is a HID device and needs no driver.
- **macOS:** the Xcode Command Line Tools.
- **Linux:** Tauri's WebKitGTK packages plus `libudev-dev` (receiver) and
  `libasound2-dev` (MIDI). The exact list is in
  [`.github/actions/linux-deps/action.yml`](.github/actions/linux-deps/action.yml).

### Commands

```sh
cd app && npm ci                       # install UI dependencies as locked
cd app && npm run tauri dev            # run the app in development
cargo test --workspace                 # Rust tests; also regenerates app/src/bindings
cargo clippy --workspace --all-targets # lint Rust
cd app && npm run lint                 # lint the UI
cd app && npm run build                # type-check and build the UI
cd app && npm test                     # UI unit tests
cd app && npm run tauri build          # build an installer for this OS
```

| Also | Command |
| --- | --- |
| E2E tests of the real app (Linux; needs `tauri-driver`, `webkit2gtk-driver`, `xvfb`, `xdotool`) | `cd app && npm run tauri build -- --debug --no-bundle && xvfb-run -a npm run e2e` |
| UI preview in a browser with a mocked backend | `cd app && npm run dev`, then open `/preview.html?window=host&scene=running` |
| Random shows and crash drills | `SOAK_SHOWS=5000 cargo test --release -p show-sim` |
| Receiver probe without the app | `cargo run -p buzzers --example hid-monitor [-- VID:PID]` |

On Windows, `npm run tauri build` makes both an `.exe` (NSIS) and an `.msi`
installer. If the MSI step fails at `light.exe`, turn on VBSCRIPT under
Settings › Apps › Optional features, or build only the `.exe` with
`npm run tauri build -- --bundles nsis`.

### Project layout

```text
crates/quiz-core   game model, state machine, scoring, view (pure, no I/O)
crates/quiz-file   .quiz files, media, spreadsheet import and export
crates/buzzers     buzzer driver trait, USB-HID receiver, Enjoy 303S protocol
crates/lights      stage lights: DMX, Art-Net, sACN, OSC, MIDI
crates/quiz-ai     quiz JSON Schema, MCP server, AI skill, buzzdome-quiz CLI
crates/show-sim    random quizzes and shows for soak tests
app/src-tauri      Tauri shell: commands, windows, show thread, journal
app/src            React UI: director/, host/ (slides), editor/ (Studio)
app/e2e            end-to-end tests through WebDriver
docs/              specification, design, quiz format, AI assistants, hardware test
replica/           behaviour map and feature matrix of the reference app
```

The architecture in one page is [`.claude/ARCHITECTURE.md`](.claude/ARCHITECTURE.md),
and the full design is [`docs/design.md`](docs/design.md).

### CI

- [`build.yml`](.github/workflows/build.yml) lints and tests on every
  push, runs the Windows tests, and builds installers on tags and on
  demand.
- [`e2e.yml`](.github/workflows/e2e.yml) runs the E2E tests: Linux on
  every push, Windows on `main`.
- [`nightly.yml`](.github/workflows/nightly.yml) runs 30 000 random shows,
  10 000 crash drills and repeated E2E runs on every night `main` changed.

## Documentation

| Document | What it covers |
| --- | --- |
| [`docs/specification.md`](docs/specification.md) | requirements with IDs: buzzer link, themes, plugins, sounds, lights |
| [`docs/design.md`](docs/design.md) | architecture, threads, buzzer stack and 303S protocol, lights, build and release |
| [`docs/quiz-format.md`](docs/quiz-format.md) | the `.quiz` file format |
| [`docs/ai-assistants.md`](docs/ai-assistants.md) | quizzes written by AI assistants: MCP, skill, CLI |
| [`docs/hardware-test.md`](docs/hardware-test.md) | checklist and results for the real receiver |
| [`replica/hardware.md`](replica/hardware.md) | research on the buzzers, receiver and cable |
| [`replica/recon.md`](replica/recon.md) | screens, flows and scoring rules the app follows |
| [`ROADMAP.md`](ROADMAP.md) | milestones and progress |

## Ground rules

- **Clean room.** Behaviour is rebuilt from our own notes. Nothing of the
  reference app (code, text, screenshots, sounds, skins, icons, example
  quizzes) and no vendor SDK file goes into this repository. Third-party
  files are bundled only under their own licences (see Licence).
- **Live shows cannot fail.** State is persisted before it is shown, and
  the app stays usable without buzzer hardware.
- **English** for code, comments, commits and docs. The UI is Czech by
  default, with English too.

## Licence

This is a private project for the author's own non-profit events. No
open-source licence is granted. Bundled third-party files keep their own
licences:

- the theme fonts, under the SIL Open Font License (`OFL.txt` next to each
  font in `app/src-tauri/themes/*/fonts/`);
- the fixture data of the Open Fixture Library, under the MIT licence
  (`crates/lights/data/`, with `OPEN-FIXTURE-LIBRARY-LICENSE`).
