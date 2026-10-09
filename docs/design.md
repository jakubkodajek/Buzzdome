# Design

How the app is built, with the buzzer link in most detail. Requirements are in
[`specification.md`](specification.md). Last updated 2026-10-09.

## 1. Overview

```text
┌──────────────── Tauri app (one process) ────────────────┐
│  React UI                                               │
│  ├── Director window  (control, setup, Receiver tab,    │
│  │                     quiz editor)                     │
│  └── Host window      (projector, full screen)          │
│            ▲ events: game-state, clock-tick, ...        │
│            │ commands: invoke(...)                      │
│  Rust shell  app/src-tauri                              │
│  ├── lib.rs       AppState { game, session, driver }    │
│  │                execute(): apply → journal → effects  │
│  ├── session.rs   append-only journal per show          │
│  ├── editor.rs    the editor's crash-safe draft         │
│  └── hardware.rs  receiver setup, learn mode, raw log   │
│  Rust crates                                            │
│  ├── quiz-core    pure game state machine, scoring      │
│  └── buzzers      driver trait, HID receiver, protocols │
└─────────────────────────────────────────────────────────┘
```

`quiz-core` has no I/O: every change to a show is a timestamped `Command`
applied to `Game`, which returns `Effect`s (lights, sound cues). The shell
journals each command to disk before the new state is emitted to the windows,
and a crashed show is rebuilt by replaying the journal.

### 1.1 Between Rust and the UI

The game core is the only place that knows the rules. Everything a window
shows is either the `GameView` the core builds or a value the core computes
(the points a correct answer earns now, whether an answer is right), so the
two windows cannot disagree with each other or with the score.

- **Types.** Every type that crosses the boundary is defined once in Rust and
  exported to TypeScript by ts-rs (`#[derive(TS)] #[ts(export)]`) into
  `app/src/bindings` when `cargo test` runs. `app/src/types.ts` re-exports
  them. Constants the UI needs (`MAX_ANSWERS`, question defaults,
  `FINAL_LAST_STEP`) go to `bindings/Constants.ts` the same way, and media
  extensions to `bindings/MediaExtensions.ts`. CI fails when the committed
  bindings differ from the Rust types.
- **Commands** (UI to Rust) are the `api` object in `app/src/api.ts`, one
  entry per `#[tauri::command]`. Game actions all go through one command,
  `dispatch(Command)`, the same for buttons, keys, virtual and real buzzers.
- **Events** (Rust to UI) are typed in `BackendEvents` in the same file:

| Event | Payload | When |
| --- | --- | --- |
| `game-state` | `GameView` or null | after every journaled command |
| `show-changed` | none | a show starts, resumes or ends (reload the quiz) |
| `clock-tick` | `ClockView` | every 250 ms while a question clock runs |
| `sound-cue` | `Cue` | a game effect that has a sound |
| `receiver-status`, `receiver-raw`, `receiver-press`, `receiver-learn` | see `hardware.rs` | receiver setup and diagnostics |

- **Clock.** A ticker thread in the shell applies due timer commands (time
  up) and otherwise sends `clock-tick` with the core's `ClockView`. The UI
  never measures time itself; the gauge animates between ticks with CSS.
- **React side.** `app-context.tsx` gives each window its language, `t` and
  error reporting (`useRun`, `useDispatch`). `hooks.ts` wraps events and
  loads (`useBackendEvent`, `useGameView`, `useClock`, `useQuiz`,
  `useLastSession`). Small shared helpers live in `answers.ts`, `format.ts`,
  `quiz.ts` and `keyboard.ts`. ESLint with the React hooks rules runs in CI.
- **Sound.** The Director window plays the cues and the slides' sound
  clips (`slide-sound.ts`), because it stays open for the whole show and the
  projector window may not. Spec SN-5 moves playback into the Rust shell later.

### 1.2 Quiz files and media

Format: [`quiz-format.md`](quiz-format.md). `crates/quiz-file` opens a `.quiz`
file (or a plain `quiz.json` with `media/` next to it) by unpacking it into
`<app data>/quizzes/<hash>/`, where the hash covers every file name and byte.
Unpacking writes into a temporary folder, syncs each file, checks the quiz
(`Quiz::validate` plus "every media file is there") and only then renames the
folder into place, so a half-unpacked quiz is never used. Opening the same
file again reuses the folder.

The session's `meta.json` records that folder (`mediaRoot`), so a resumed
show finds its media even if the original file was moved. When a show
starts, every other unpacked quiz is deleted, because only the latest show can
be resumed. If the folder is gone on resume, the show still resumes and the
Director shows a warning.

The windows load media through Tauri's asset protocol. Its scope starts
empty and the shell allows exactly the current quiz folder
(`asset_protocol_scope().allow_directory`), so a page cannot read anything
else on the disk. `media_root` returns the folder with a trailing separator;
`media.ts` appends a reference to it and calls `convertFileSrc`. Media
references are checked in the core (`media.rs`) to stay inside `media/`.

### 1.3 Quiz editor

The editor is a screen of the Director window (`app/src/editor/`), laid out
like the reference's: ribbon with a File page, slide thumbnails, the slide
canvas with notes under it, the properties panel, and a status bar with a
table view toggle. It edits a quiz; it never touches a running show.

- **Draft.** `editor.rs` keeps the quiz being edited unpacked in
  `<app data>/editor/draft-*/` and `draft.json` points at the current
  folder, so a crash or a closed window loses nothing. A new or opened quiz
  gets its own folder and the pointer switches only once the folder is
  complete. Saving packs the draft into a `.quiz` file with `quiz-file`.
  Every change is written through one `DraftQueue` (`draft-queue.ts`):
  writes run in order, and writes still waiting collapse into the latest.
  The backend answers each write with the quiz's problems
  (`quiz_file::problems`: the core's checks plus missing media), shown on
  the thumbnails, in the status bar and on the File page.
- **State.** `history.ts` is a pure undo history of `Edit`s (set title,
  insert, update, remove, move a slide). Typing into one field of one slide
  merges into one undo step through a merge key (`<slide id>:<field>`).
  `editor-state.ts` reduces that history together with what the backend
  reported (loaded, saved, checked). The selection is a slide id with its
  index as a fallback once the slide is gone, plus the part of the slide
  (`elements.ts`: question, answer *n*, title, picture, video, sound, ...),
  which the canvas, the panel and the context menu share.
- **Slide changes.** `slides.ts` holds every change to a slide as a pure
  function that keeps the slide playable (an everyone-answers question
  always has 2 to 4 answers and a correct one, and so on). Nothing in the
  UI edits a slide object by hand. Tested in `slides.test.ts`.
- **Actions.** `actions.ts` defines an `Action` (label, icon, keys,
  disabled, checked, run). `EditorScreen.tsx` builds the one record of all
  actions, and the ribbon, context menus, the quick access bar, the status
  bar and the keyboard (`useActionKeys`) all read it, so a command, its
  shortcut and its enabled state are defined once.
- **Properties.** `properties.tsx` turns the selected slide into labelled
  rows (label, description, control). The properties panel, the ribbon's
  Question tab and the table view lay out the same rows, so a field is
  built and validated in one place. The Edit dialog (double click, Enter)
  edits a local copy and applies it as one undo step.
- **Rendering.** The canvas, the thumbnails (editor and Director) and the
  projector all draw a slide with `SlideScreen` inside a 16:9 `Stage`
  (`app/src/host/`). The editor and thumbnails feed it `previewView`, a
  `GameView` built from the slide alone. What a slide shows from the rest of
  the quiz comes in a `SlideContext` (`quiz.ts`): its question number and,
  for an explanation slide, the question it explains. `MediaRootContext` sits above
  every screen that resolves media, so the editor, the Director and their
  sound hooks agree on the folder.
- **Test run.** F5 (from the start) and Shift+F5 (from the selected slide)
  start a show of the draft with keyboard and on-screen buzzers. The
  Director shows a test run badge and "Back to the editor" ends the show
  and returns. A quiz with problems opens the File page instead.

## 2. Buzzer link

### 2.1 Layers

```text
USB HID device ──► HidBackend / HidDevice      crates/buzzers/src/hid.rs
                   (hidapi in production,      hidapi_backend.rs
                    a fake in tests)
                ──► receiver thread             receiver.rs
                    open, read, reconnect,
                    write lights
                ──► Protocol                    protocol.rs, learned.rs
                    report bytes → presses,
                    lights → report bytes
                ──► EdgeFilter                  edge.rs
                    held button = one press
                ──► ReceiverEvent channel
                ──► event pump (app)            app/src-tauri/src/hardware.rs
                    learn mode, raw log, UI events
                ──► execute(Command::Press)     app/src-tauri/src/lib.rs
                    journal, then game state, then lights
```

Each layer is replaceable on its own: the protocol does not know about USB,
the receiver thread does not know about the game, and the game does not know
where a press came from (keyboard, on-screen button or receiver are the same
`Command::Press`).

### 2.2 Device access

- `hidapi` 2.6 (`hidapi` crate). On Linux it uses the `hidraw` backend linked
  statically (needs `libudev` at run time); on macOS it opens devices
  non-exclusively; on Windows it uses the built-in HID driver. No vendor
  driver on any OS.
- A receiver is identified by a `DeviceSelector`: vendor ID, product ID and,
  when the device exposes several HID interfaces, its usage page and
  interface number. OS device paths change between ports and restarts, so
  they are never stored.
- `hidapi::HidDevice` may be used from one thread at a time, so a single
  thread owns the open device. Everyone else talks to it through a
  `ReceiverHandle` (a channel): select a device, set lights, reload the
  protocol, shut down.

### 2.3 Receiver thread

Loop, every iteration:

1. Drain control messages (select, lights, reload, shutdown).
2. If connected: `read_timeout(20 ms)`. A report goes through the protocol
   and the edge filter; the thread sends `Raw` (bytes, time, decoded presses)
   and then one `Press` event per press, in the order the protocol returned
   them. A read error closes the device and reports `Error` ("connection
   lost, reconnecting").
3. If not connected and a receiver is selected: every 2 seconds list devices,
   open the first match, send the protocol's init reports, then report
   `Connected` (init goes first so lights work as soon as the UI says
   "connected").

Statuses: `Off` (nothing selected), `Searching` (selected, not plugged in),
`Connected` (with protocol name and whether it drives lights), `Error`
(with an OS-specific hint: udev rule on Linux, Input Monitoring on macOS,
"another program has it" on Windows).

### 2.4 Protocols

```rust
trait Protocol: Send {
    fn name(&self) -> String;
    fn decode(&mut self, report: &[u8]) -> Vec<Press>;
    fn init_reports(&self) -> Vec<Vec<u8>>;                        // default: none
    fn encode_lights(&self, lights: &[(BuzzerId, Light)]) -> Vec<Vec<u8>>; // default: none
    fn has_lights(&self) -> bool;                                  // default: false
}
```

A resolver picks the protocol when a device opens, in this order:

1. **Vendor protocol** for the device's vendor and product ID. Empty slot
   today, marked in `hardware.rs`; filled once the SDK documentation arrives
   (dome and RGB get separate implementations, since the vendor says their
   protocols differ). `decode` is stateful so it can track sequence numbers
   or multi-report frames.
2. **Learned protocol** if the user taught buttons for this vendor and
   product ID.
3. **Raw only**: decodes nothing, but every report still appears in the raw
   log, which is how the real protocol gets worked out.

### 2.5 Learn mode

For when the protocol is not documented. The user picks a buzzer number and a
button, presses it at least twice, and saves.

- The recorder takes the first non-idle report after an idle report (all
  bytes after the report ID are zero) or after a 150 ms gap, so a held button
  gives one sample, not dozens.
- The signature is the bytes common to all samples; bytes that differ between
  samples (counters, timestamps) are masked out. At least two samples are
  needed for that, three are better.
- A new signature that matches an existing one for a different button (or the
  other way round) is refused, so two buttons never decode to the same press.
- Decoding picks the matching signature with the most fixed bytes.
- Stored per vendor and product ID in `receiver.json` in the app data folder,
  next to the chosen receiver. Written atomically (temp file, then rename).

Limits: learned mode cannot drive lights (the light commands are unknown) and
it assumes one press per report. If the receiver packs several presses into
one report, or encodes the buzzer ID in a way that is not a fixed byte
pattern, learn mode will show conflicts and the vendor protocol is needed.

### 2.6 From press to screen

The event pump thread in `hardware.rs` receives `Press` events in receiver
order and calls `execute_at(Command::Press { buzzer, button }, at_ms)` for
each, one at a time, under the app state lock. `at_ms` is when the receiver
thread read the report, not when the press is applied, so waiting for the
lock or for earlier presses' fsync costs a team no points. `execute_at`
applies the command to a copy of the game, appends it to the journal
(fsync'd file), swaps the new state in, sends light effects to the driver,
and only then emits the new state to the windows. If the journal write
fails, the press is rejected and the screen does not change.

Because a press can be applied a moment after it was received, the core
judges it by its `at_ms`: it counts only if it was received while the
question clock ran (or stood paused) and before time up. The ticker closes a
question `PRESS_GRACE_MS` (250 ms) after time up, so presses received in
time but still on their way are counted, and later ones are refused.

Fastest-finger order is therefore the receiver's order. When the vendor
protocol provides press times, `Command::Press` gets an optional
`pressed_at` from the receiver and the game ranks presses that arrive close
together by that time; the PC clock is never used for ranking.

While learn mode is active, presses are shown in the Receiver tab but not
sent to the game. So that it can never mute the buzzers in a show, learn
mode ends when a show starts, when the receiver disconnects and when the
Receiver tab closes, and the Director shows a banner while it is on.

### 2.7 Lights

`Game::apply` returns `Effect::Lights` with the light state of every affected
buzzer. The shell passes it to `BuzzerDriver::set_lights`; for the hardware
driver that is a message to the receiver thread, which encodes it with the
active protocol and writes the reports. Light failures are logged and never
block the show. With the learned or raw protocol, `has_lights()` is false and
the Director says "this receiver does not light the buzzers yet".

### 2.8 Diagnostics

- The app keeps the last 2000 raw reports in memory; the Receiver tab shows the
  latest ones and can export all of them as text
  (`unix-ms <tab> hex bytes <tab> decoded presses`).
- `cargo run -p buzzers --example hid-monitor` lists HID devices and, with a
  `VID:PID` argument, prints reports with timestamps, without the app.

### 2.9 Testing

`crates/buzzers/tests/receiver.rs` runs the real receiver thread against a
fake HID backend: connect and init, decode order, held-button repeats,
unknown reports, light commands, unplug and replug, open failure and retry,
deselect. Unit tests cover the edge filter, signatures, the recorder and
conflict detection. None of this proves the real receiver's format: see
[`hardware-test.md`](hardware-test.md).

## 3. OS specifics

| | Windows | macOS | Linux |
| --- | --- | --- | --- |
| Web view | WebView2 (installed by the NSIS installer if missing) | WKWebView | WebKitGTK 4.1 |
| HID access | no driver, no admin rights | may ask for Input Monitoring permission | udev rule (shown in the app) for `hidraw` access |
| Package | NSIS `.exe` | universal `.dmg`, ad-hoc signed | `.deb`, `.AppImage` |
| First start of an unsigned build | SmartScreen: "More info", "Run anyway" | Privacy & Security: "Open Anyway" | none |
| App data (journal, `receiver.json`) | `%APPDATA%\cz.kodajek.quizshow` | `~/Library/Application Support/cz.kodajek.quizshow` | `~/.local/share/cz.kodajek.quizshow` |

## 4. Build and release

`.github/workflows/build.yml`:

1. `check` (Ubuntu 22.04, every push): ESLint, type-check and build the UI,
   `cargo clippy -D warnings`, `cargo test --workspace` from an empty
   bindings folder, then fail if `app/src/bindings` changed.
2. `installers` (matrix, after `check`, on `main`, tags `v*` and manual
   runs, because macOS minutes are expensive): Windows NSIS, macOS universal DMG
   (`aarch64` and `x86_64` targets), Linux deb and AppImage on Ubuntu 22.04
   so the AppImage runs on older distributions. Each uploads its files as a
   run artifact (`quiz-show-windows`, `quiz-show-macos`, `quiz-show-linux`).

Signing and notarization need a Windows code-signing certificate and an Apple
Developer ID; until then builds are unsigned (macOS ad-hoc), see PL-5.

## 5. Extension points (for sections 5, 6 and 10 of the specification)

Not built yet; this is where they will attach, so current code does not block
them.

- **Themes and backgrounds.** The host screen reads colours and fonts from
  CSS variables and backgrounds from one resolver function
  (slide > round > quiz > theme > built-in). Theme packages unpack into
  `<app data>/themes/<id>/` and are served to the web view through Tauri's
  asset protocol, limited to that folder. Quiz files copy used backgrounds
  into their `media/` folder on save.
- **Plugins.** A plugin runs in an `<iframe sandbox="allow-scripts">` in the
  host window (screen plugins) or a hidden one in the Director (service
  plugins), loaded from `<app data>/plugins/<id>/` with a strict content
  security policy. A bridge in the React app relays `postMessage` traffic,
  checks permissions and turns requests into Tauri commands. Score and show
  changes become the same journaled `Command`s the Director uses; plugin
  `storage.set` becomes a new `Command::PluginState` so it is journaled and
  replayed. Events (`slide.shown`, `buzzer.pressed`, ...) are derived from the
  game state and the press stream the app already emits.
- **Light colours** move from the protocol into the theme
  (`buzzerColors`), passed to `encode_lights` for RGB buzzers.
- **Sounds** (spec section 10). Cues already come out of the core as effects.
  The shell maps each cue to a file (show setting, else theme, else built-in)
  and plays it with a Rust audio library chosen when this is built, so
  playback does not depend on a window.

## 6. Verification status

| What | How it was checked | Result |
| --- | --- | --- |
| Receiver thread, protocols, learn mode, edge filter | Rust unit and integration tests with a fake HID backend, repeated runs | pass |
| App builds and starts with the HID backend | Linux, under a virtual display; Receiver tab lists devices (none attached) | pass |
| Linux `.deb` | built locally | pass |
| UI screens (Director, projector) render without errors, incl. a slide picture | preview with a mocked backend, headless Chromium | pass |
| Editor: typing, undo and redo, copy and paste, delete, keys, Edit dialog, ribbon fields, layout gallery, drag reorder, table view, File page | `npm test` (history, draft queue, slide changes, layouts) and 22 flows in headless Chromium against the preview | pass |
| Quiz files: save, open, reuse, unsafe entries, size limit, missing media | `cargo test -p quiz-file` | pass |
| Media through the asset protocol, autoplay of video and sound | needs the installed app on each OS | **not tested** |
| Windows and macOS installers | CI | see the PR's checks |
| Real receiver: detection, raw reports, learn mode, presses in a show | needs the hardware | **not tested** |
| Lights, press times, battery | need the vendor protocol | **not built** |
