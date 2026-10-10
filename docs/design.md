# Design

How the app is built, with the buzzer link in most detail. Requirements are in
[`specification.md`](specification.md). Last updated 2026-10-10.

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
│  ├── lib.rs       AppState { game, session, teams, .. } │
│  │                execute_at(): apply → journal → emit  │
│  ├── session.rs   append-only journal per show          │
│  ├── app_windows.rs  the two windows, closing, pages    │
│  ├── app_log.rs   log file, panics                      │
│  ├── editor.rs    the editor's crash-safe draft         │
│  ├── teams.rs     team names kept between shows         │
│  ├── media_player.rs  media library, sound board        │
│  ├── themes.rs    built-in themes, list_themes          │
│  ├── app_file.rs  small JSON files in app data          │
│  └── hardware.rs  receiver setup, learn mode, raw log   │
│  Rust crates                                            │
│  ├── quiz-core    pure game state machine, scoring,     │
│  │                themes, spreadsheet rows              │
│  ├── quiz-file    .quiz files, media, spreadsheets      │
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
  them. Constants the UI needs (`MAX_ANSWERS`, question defaults and limits,
  `FINAL_LAST_STEP`, `DEFAULT_THEME_ID`) go to `bindings/Constants.ts` the
  same way, media extensions to `bindings/MediaExtensions.ts` and the
  built-in themes to `bindings/BuiltinThemes.json` (1.4). CI fails when the committed
  bindings differ from the Rust types.
- **Errors and messages.** No Rust code sends the UI an English sentence.
  A failed command returns an `AppError` (`app/src-tauri/src/error.rs`): a
  code with parameters, such as `{ code: "file", action: "read", path,
  detail }`. Quiz problems are `Problem`s (`quiz-core`), receiver failures
  `ReceiverProblem`s and protocols `ProtocolKind`s (`buzzers`).
  `app/src/messages.ts` words them in the quizmaster's language from
  `i18n.ts`; `detail` fields carry the system's own message as it is.
  `Display` on these types is for logs and tests only.
- **Step controls.** `GameView.controls` says which step commands (next,
  judge, pause, resume, end question, go to a slide or the final) the
  current step takes; `Game::controls` is checked against `apply` by a test
  over a whole show. The Director shows only those buttons and
  `keyboard.ts` sends only those keys, so no key produces "not now" in a
  show. A question in play (its clock runs or stands paused, or a buzz
  waits for judgement) cannot be jumped away from, as that would throw its
  score changes away; the Director jumps only with its Go to buttons, and
  asks before Go to the end and End show.
- **One step per press.** A key held down sends one command, Enter held
  on a focused button presses it once, and a key a focused control already
  took (Space on a team chip) is not the show's. `useDispatch` drops a step
  command (`step-guard.ts`) while the last one is unanswered and for
  `STEP_PAUSE_MS` (300 ms) after its answer, less than a person takes to
  see the new step, so a double press never runs the step after the one on
  screen, however long the journal write took. Presses and score changes are never dropped.
  Show keys count as typing only in text fields: after the quizmaster
  touched a slider, checkbox or list, the keys still work, and a slider
  lets go of the focus when released.
- **Where Next goes.** `Game::following` decides it: the next slide, or
  after the last slide the final ranking, unless the quiz has a final slide
  of its own (`Quiz::final_slide`), which then is the ranking and the show
  ends with the last slide. `controls` and `GameView.next_slide_title` use
  the same function.
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
| `host-window` | bool | the projector window opened or closed (also from its title bar) |
| `host-problem` | string | the projector met a problem, worded for the Director to show |
| `close-requested` | none | the quizmaster closed the Director or quit during a show (the Director asks, then `quit_app` or `close_declined`) |
| `show-not-saved` | `AppError` or null | the running show's journal refused a write, or works again |
| `teams-changed` | string list | the prepared team names changed, or a show started or ended |
| `media-library`, `sound-board` | `MediaLibrary`, `SoundBoard` | the media player's library or pads changed |
| `media-playing`, `media-volume` | `MediaNow`, number | the media player started, paused or stopped (with the show clock's time then); its volume changed |
| `app-icon` | `IconColor` | the windows' icon took another colour |

- **Clock.** Every command and press is stamped by one show clock
  (`buzzers::clock::ShowClock`, shared with the receiver thread): it reads
  the wall clock once when the app starts and counts on from a monotonic
  clock, so the system setting its clock mid-show (say, on joining the
  venue's network) neither freezes the countdown nor ends a question early.
  The show thread (`show_thread.rs`) applies due timer commands (time up)
  and otherwise sends `clock-tick` with the core's `ClockView`. The UI never
  measures time itself; the gauge animates between ticks with CSS.
- **Threads.** One show thread changes a running show: the receiver's
  events and the windows' commands that change or end the show
  (`dispatch`, team edits, `end_show`) reach it through one queue, so they
  are handled in the order they came, and the windows never wait on a
  journal write. A step given just before the show ends still counts in
  it, and none given after reaches the show. The commands that read the
  show or start one are async and take the show state on a worker thread
  (`off_ui`); Tauri runs synchronous commands on the windows' thread, so
  they must not wait for it. The app's locks are taken in one order: the
  editor's, the show state, then learn mode's; so nothing holding learn
  mode's lock, as the Receiver tab's commands do, waits for a journal write.
- **Saving problems.** When the journal refuses a write (the disk is full,
  say), the change is not made, and the Director shows why in a banner until
  a write works again (`show-not-saved`, `show_not_saved`). A due command
  that failed is tried again a second later. A write that does not finish
  (the disk does not answer) holds the show thread up: after 3 s
  (`STUCK_AFTER`) a watchdog shows that in the banner (`saveHangs`), and a
  command the thread has not begun by then is given up (`showBusy`) and
  never made, so the quizmaster can give it again without it happening
  twice. Presses wait in the queue and count by their time once the write
  finishes.
- **Windows.** `app_windows.rs` keeps both windows through a show. Closing
  the Director during a show asks first (`close-requested`), and the app
  ends with the Director, so a full-screen projector is never left alone.
  The question is asked above the Director's screen (`App.tsx`), so it
  comes even when the screen failed to render, and once: a second close
  while it is open adds no dialog. A Director that does not answer within
  10 s (its page hangs) does not keep the app open: the next close goes
  through. On macOS the app menu has a Quit of its own (`app_menu`), as
  the system's ends the app at once: Cmd+Q asks like the close button, and
  only Quit from the Dock ends a show without asking (it stays saved).
  Starting the app again brings the Director forward, or builds it again
  if it is gone. The projector opens on the
  monitor chosen in Settings while it is connected, else on one the
  Director is not on, and full screen over the Director only when chosen
  there. The monitors are listed when it opens, so one plugged in late
  counts; a monitor is known by its name and position (`MonitorInfo.id`).
  M and Esc close the projector from itself.
- **Pages.** The windows stay on the app's own pages: a navigation guard
  (`app_windows::navigation_guard`) refuses every other URL, and
  `browser-guard.ts` takes the web view's own keys (reload, back, find,
  print; Option+arrows still move by words in a text field), its context
  menu outside text fields, and a drop that no drop zone takes. It listens
  as events come down to their target, so a part that stops one on its
  way back up, as a popup menu does, cannot skip it. A part that fails to
  render shows the error in its place (`ErrorBoundary`, worded by
  `RenderFailed`): the Director's window, the editor and each Director tab
  offer to try again, the window also to close the app and the editor to
  go back to the show; a slide drawn in a list, a preview or the editor
  says it failed in its own box (`SlideBoundary`) and is drawn again once
  it changes; a projector slide leaves the stage as it is and tells the
  Director; a projector whose whole screen fails goes black, tells the
  Director to reopen it, and still closes with M or Esc.
- **What a page may do.** Each window gets only the commands it needs
  (`capabilities/director.json`, `capabilities/projector.json`; `build.rs`
  lists the app's commands so Tauri checks them, and a test in
  `app_windows.rs` keeps the three lists in step). The projector can read
  the show and its media and dispatch, but not open, save or pick files.
  A content security policy (`tauri.conf.json`) lets the pages load only
  the app's own scripts and styles and media through the asset protocol.
  Files the quizmaster opens, saves, imports or exports are picked by the
  backend (`dialogs.rs`, worded by the UI's `Picker`), never named by a
  page, and the pages confirm through the dialog plugin's `ask`
  (`confirm.ts`, with Yes and No in the UI's language); ESLint refuses `alert`, `confirm`, `prompt` and the
  plugin's file pickers.
- **Log.** `app_log.rs` writes `buzzdome.log` in the app's log folder
  (section 3): the start, a panic with its backtrace, and problems that
  have no window to show them. Each line carries the show clock's time, as
  the journal does. A full log is moved to `buzzdome.old.log`.
- **React side.** `app-context.tsx` gives each window its language, `t` and
  error reporting (`useReport`, `useRun`, `useDispatch`). `hooks.ts` wraps events and
  loads (`useBackendEvent`, `useGameView`, `useClock`, `useQuiz`,
  `useLastSession`). Small shared helpers live in `answers.ts`, `format.ts`,
  `quiz.ts`, `keyboard.ts` and `messages.ts`. ESLint with the React hooks rules runs in CI.
- **Sound.** The Director window plays the cues and the slides' sound
  clips, because it stays open for the whole show and the projector window
  may not. Spec SN-5 moves playback into the Rust shell later. The code is in
  `app/src/audio/`: `context.ts` (one shared `AudioContext`), `cues.ts` (game
  effects), `clip-player.ts` (`PartPlayer` plays a part of any media
  element, looping or stopping at its end; `ClipPlayer` adds a sound file
  at volume × loudness gain), `slide-sound.ts` (when a slide's sound plays),
  `replay.ts` (the replay part after the reveal, for a slide's sound and
  its video alike), `loudness.ts` (ITU-R BS.1770-4 measurement and the gain
  that reaches a target), `decode.ts` and `measure.ts`.
  - A sound is decoded whole only up to `MAX_DECODED_MINUTES` (10), read
    from its metadata first: decoded stereo takes about 23 MB a minute, in
    the window that may run the show. One that tells no length is read only
    up to `MAX_UNTIMED_BYTES` (what 10 minutes at 64 kbit/s take), checked
    on the response's length and again on the bytes. A longer one cannot be
    trimmed or normalized, and the windows say so.
  - Measuring runs in a Worker (`loudness.worker.ts`), since a long part
    takes over a second; decoding stays in the window, which alone has an
    `OfflineAudioContext`. The samples move to the Worker, not copied,
    except where the window still shows them: the trim window sends a copy
    of the new part only (`measureCopy`).
  - A gain above 1 goes through a Web Audio `GainNode` (an element's
    `volume` stops at 1). That needs the media response to allow the
    window's origin, which Tauri's asset protocol does; a gain at or below 1
    uses `volume`, so most sounds do not depend on it.
  - Files are decoded for measuring with an `OfflineAudioContext` at 48 kHz,
    because `decodeAudioData` resamples to its context's rate (W3C Web Audio
    API) and BS.1770-4 gives the K-weighting filter for 48 kHz.
  - The editor's Trim windows (`TrimDialog.tsx` with the waveform maths in
    `waveform.ts`, and `VideoTrimDialog.tsx`, sharing the part state, keys
    and controls in `part-editing.tsx`) and Normalize volume window
    (`LoudnessDialog.tsx`, `normalize.ts`) only change `part`, `replay` and
    `gainDb`; files are never rewritten.
- **Background video.** `StageBackground` (`slide-media.tsx`) plays it with
  its options. Sync
  with quiz reads `GameView.onHold` from the core (a player answering, the
  clock paused, the question closed or revealed), so the rule lives in one
  place.
- **Slide video.** `ShowVideo` (`slide-video.tsx`) plays a slide's video on
  the projector with a `PartPlayer`: it starts after its intro pause, or
  each time `GameView.videoPlays` grows. That counter is the core's answer
  to the journaled `PlayVideo` command, so a Play video given before the
  projector window opened, or before a restart, still reaches it. Once
  started it follows `onHold` like a synced background, and after the
  reveal it plays its replay part (`useReplay`, after the hold, since a
  reveal can end a hold in the same view). The effect's strength is `timeLeft`
  (`answer-time.ts`, also the clock gauge's fill), read between the
  250 ms clock ticks from the time since the last tick, every frame; blur
  is a CSS filter, the mosaic draws the frame at a few blocks per row on a
  canvas over the video (`video-effect.ts` holds the maths).

### 1.2 Quiz files and media

Format: [`quiz-format.md`](quiz-format.md). `crates/quiz-file` opens a `.quiz`
file (or a plain `quiz.json` with `media/` next to it) by unpacking it into
`<local data>/quizzes/<hash>/` (section 3), where the hash covers every file name and byte.
Unpacking writes into a temporary folder, syncs each file, checks the quiz
(`Quiz::validate` plus "every media file is there") and only then renames the
folder into place, so a half-unpacked quiz is never used. Opening the same
file again reuses the folder. A folder left broken (by a crash, or a `prune`
that could not finish) is first renamed aside into `.trash-*` and then
deleted: renaming works where deleting may not (a read-only file), and
`prune` deletes what is left. On Windows a folder with a file still open in
it (a previous show's video) can be neither renamed nor emptied; then the
package goes to `<hash>-1` (`-2`, ...), and that folder is reused next time.

The editor opens a file with `open_into`, which unpacks the same way but
does not check the quiz: a file an older version wrote may break rules added
since (quiz-format.md, Versioning), and the author fixes it in the editor.
Only safe names inside `media/` are ever read or written, checked or not.
A `.quiz` entry with a name that is not portable is stored under the
closest portable one (`quiz_core::portable_reference`, numbered if taken)
and the quiz's references are renamed with it (the draft then counts as
unsaved); a plain `quiz.json` reads
only safe names from the disk (no device names on Windows), and the show
path keeps such a file refused.
A `quiz.json` over 16 MB (`MAX_QUIZ_JSON_BYTES`) is refused before it is
parsed, whether read from a package, from the disk or about to be saved, so
a small packed file cannot unpack into a huge quiz.

A slide's `style` (`quiz_core::style`) is typed: `Color`, `FontName`,
`TextSize` and `Transparency` check their values whenever they are read,
from a file or from the UI, and each style field reads through `lenient`,
so a wrong value reads as missing (the theme's look) instead of refusing
the quiz. A font name cannot hold quotes, so the UI can put it in a style
sheet as it is.

Reading a quiz (`Quiz::read_json`) also says what it left out
(`ReadQuiz::dropped`): style values that did not read
(`SlideStyle::left_out`, by comparing the file's `style` with the style
read) and what a slide's type cannot play (`drop_unplayable`), each as the
`Problem` it would be. Opening a file in the editor writes the draft
without them and marks it unsaved; `draft.json` keeps the list until the
draft is saved, and File › Info shows it, so a save never removes
something without the author having been told.

The session's `meta.json` records that folder (`mediaRoot`), so a resumed
show finds its media even if the original file was moved. When a show
starts, every other unpacked quiz is deleted, because only the latest show can
be resumed. If the folder is gone on resume, the show still resumes and the
Director shows a warning. `meta.json` also records `RULES_VERSION`, which
quiz-core bumps whenever the same journal would replay into another show;
a show begun under another version still resumes, rebuilt with today's
rules, and the Director says its scores may differ. What would put the
replay a step off is kept: a journal begun before version 2 replays its
jumps out of a question in play, as the app then restarted a question
after a crash with a `GoTo` to it (`Game::replay`).

The windows load media through Tauri's asset protocol. Its scope starts
empty and the shell allows exactly the current quiz folder, the editor's
draft folder, the built-in theme packages and the media player's library
(`<app data>/library`, spec SN-7) (`asset_protocol_scope().allow_directory`),
so a page cannot read anything else on the disk. A quiz folder is allowed
only inside the app's own folders (the unpacked quizzes and the editor's
drafts): a session whose `meta.json` names another folder resumes without
media, with a warning (`mediaFolderRefused`). `media_root` returns the folder with a trailing separator;
`media.ts` appends a reference to it and calls `convertFileSrc`. Media
references are checked in the core (`media.rs`) to stay inside `media/`.

### 1.3 Quiz editor

The editor is a screen of the Director window (`app/src/editor/`), laid out
like the reference's: ribbon with a File page, slide thumbnails, the slide
canvas with notes under it, the properties panel, and a status bar with a
table view toggle. It edits a quiz; it never touches a running show.

- **Draft.** `editor.rs` keeps the quiz being edited unpacked in
  `<local data>/editor/draft-*/` and `draft.json` points at the current
  folder, so a crash or a closed window loses nothing. A new or opened quiz
  gets its own folder and the pointer switches only once the folder is
  complete. Saving packs the draft into a `.quiz` file with `quiz-file`.
  Every change is written through one `DraftQueue` (`draft-queue.ts`):
  writes run in order, and writes still waiting collapse into the latest.
  The backend answers each write with the quiz's problems
  (`quiz_file::problems`: the core's checks plus missing media), shown on
  the thumbnails, in the status bar and on the File page. A file that opens
  with problems opens on that list.
- **One draft at a time.** The draft folder's name is its id
  (`EditorDraft.id`): every command on a draft names it, and the backend
  refuses one for a draft that another has replaced since
  (`draftChanged`). `draft-session.ts` runs one file operation at a time
  (new, open, close, save, media, import, export, test run): another one
  given meanwhile, such as Ctrl+S while a big quiz opens, does not run, and
  while another quiz takes the open one's place the editor takes no edits.
  A write that fails is tried again every 2 s until it or a later one
  works, with a banner meanwhile; one for a replaced draft is let go. The
  slide clipboard and a copied property value belong to their draft, so
  a slide never brings a media name into another quiz. A test run holds
  the draft folder it plays from: switching drafts keeps it.
- **A draft that does not read.** A `draft.json` or `quiz.json` that cannot
  be read (a newer schema after going back to an older version, say) is
  not taken for "no draft": the editor says why and offers to try again,
  to discard the draft (after asking) or to go back, and nothing deletes
  it meanwhile. The backend refuses a new or opened quiz while `draft.json`
  does not read, so its folder is not pruned unseen.
- **State.** `history.ts` is a pure undo history of `Edit`s (set title,
  set theme, insert, update, remove, move a slide; insert, update, remove
  and apply a category). Typing into one field of one slide
  merges into one undo step through a merge key (`<slide id>:<field>`).
  `editor-state.ts` reduces that history together with what the backend
  reported (loaded, saved, checked). The selection (`selection.ts`) is the
  slide on the canvas, by id with its index as a fallback once the slide is
  gone, the part of it (`elements.ts`: question, answer *n*, title,
  picture, video, sound, ...), which the canvas, the panel and the context
  menu share, and the other slides selected with it (Ctrl and Shift click,
  Ctrl+A). Cut, copy, paste, duplicate, delete, moving and dragging act on
  every selected slide as one `batch` edit; `selection.ts` builds those
  edits and the click rules, tested in `selection.test.ts`.
- **Slide changes.** `slides.ts` holds every change to a slide as a pure
  function that keeps the slide playable (an everyone-answers question
  always has 2 to 4 answers and at least one right one, a fastest finger question at
  most 6, and so on; `answerLimits` reads the core's `ANSWER_LIMITS`). What
  a slide type can play (a picture or video; a clock and a reveal) is one
  rule, `SlideType::plays` in quiz-core: `validate`, opening a file, the
  spreadsheet import and the editor (`plays` in `quiz.ts`, over the
  generated `SLIDE_PLAYS`) all read it. Nothing in the
  UI edits a slide object by hand. Tested in `slides.test.ts`.
- **Actions.** `actions.ts` defines an `Action` (label, icon, keys,
  disabled, checked, run). `EditorScreen.tsx` builds the one record of all
  actions, and the ribbon, context menus, the quick access bar, the status
  bar and the keyboard (`useActionKeys`) all read it, so a command, its
  shortcut and its enabled state are defined once. The record is put
  together from hooks that hold what their commands need: `useSlideList`
  (the selection, the clipboard, moving and going through the slides),
  `useSlideMedia` (a slide's picture, video, sound and background, and
  playing a background video on the canvas), and, with a window of their
  own, `useSoundTools` (trimming a sound or video, evening out loudness)
  and `useSheetActions` (spreadsheet import, sample sheet and export, read
  by File › Import and Export); `actions.ts` lists the ids by hook.
  `menus.ts` says what a right click offers on a thumbnail, the list, a part
  of the slide or a category.
- **Properties.** `properties.tsx` turns the selected slide into labelled
  rows (id, label, description, control, and for most rows a typed value:
  number with its range, flag, choice or media). The properties panel, the
  ribbon's Question tab and the table view lay out the same rows, so a field is
  built and validated in one place. The Edit dialog (double click, Enter)
  edits a local copy and applies it as one undo step.
  `property-pages.ts` sorts the rows into the panel's pages and sections.
  With several slides selected each slide gets its own rows, and the panel
  shows only the rows with a value that every slide has (`commonRows`);
  a change runs the same slide change on each of them as one undo step,
  and values that differ show as Mixed (`MixedContext` in `controls.tsx`).
  What the panel does with a row's value (search with accents ignored,
  dragging a number's name, Reset, Copy and Paste between rows, the
  alphabetical list) is pure in `property-rows.ts`, tested in
  `property-rows.test.ts`. `row-builders.tsx` makes a row of each kind
  with its control, so the slide's rows (`properties.tsx`) and its look
  (`style-properties.tsx`) are built alike.
- **Look.** A slide's style (1.4) is set on the design page: a gradient
  behind the slide and, for each of its texts, font, colour, alignment,
  shadow, fill and transparency. Only what differs from the theme is
  stored (`setTextStyle` drops what is unset), so Reset goes back to the
  theme's. A gradient row is a grouped row: its preview sums it up and its
  From, To and Type are `parts`, folded open by clicking its name. The
  colour picker (`ColorPicker.tsx`) offers the theme's colour, a palette of
  shades, standard colours, the colours used since the app started
  (`palette.ts`, kept in memory) and a custom colour; its pipette is the
  web view's `EyeDropper`, which only Chromium has (WebView2 on Windows),
  so on macOS and Linux the button is not shown. Gradient presets and up to
  five own gradients (`gradients.ts`, kept per computer) are in
  `GradientPicker.tsx`; the font list (`FontPicker.tsx`) asks the backend
  for the installed fonts (`fonts.rs`, fontdb, read once per run, only
  names a quiz can store) and offers common fonts where they cannot be
  read. Ctrl+C and Ctrl+V on a row copy and paste its value instead of
  slides: copying puts the value on the clipboard as text (a colour as
  `#1E4A6B`), and pasting takes text from another program where the row
  can read it (`pasteText`), else the value copied in the panel. A hex box
  is a text field, so pasting into it works on every platform.
- **Clock.** A question's clock has one of five designs (`TimerDesign` in
  the slide's style; `host/Timer.tsx` draws each, `timer.css` colours what
  runs out with the theme's accent). The `timer` row (`style-properties.tsx`)
  is on the design page, alone when the clock is clicked on the canvas, and
  the Question tab shows the same row as a gallery (`TimerGallery.tsx`)
  that draws each design in the quiz's theme, measured and scaled to its
  tile. The default design is stored as no value (`DEFAULT_TIMER`), so a
  quiz without a choice reads in builds before the designs.
- **Categories.** A category (spec QZ-10, `quiz_core::category`) is a
  background and a style that applying copies onto slides (`formatLike` in
  `slides.ts`), so drawing a slide never looks one up and the rule of what
  a category gives lives in one place; the core only checks categories and
  resolves a spreadsheet's `CAT` name to an id. The Categories view
  (`useCategories.tsx`, `CategoryView.tsx`) edits a category as its sample
  slide (`sampleSlide` in `categories.ts`): the panel's background and look
  rows run on that slide and `withLook` takes its background and style
  back into the category, so they are the slide's own rows. In that view
  the keys act only on categories and quiz-wide commands, never on slides
  out of sight. Applying from the Design tab or the Category row, and
  Apply again, are `applyCategory` edits, which take the category as it is
  when the edit lands.
- **Rendering.** The canvas, the thumbnails (editor and Director) and the
  projector all draw a slide with `SlideScreen` inside a 16:9 `Stage`
  (`app/src/host/`). The editor and thumbnails feed it `previewView`, a
  `GameView` built from the slide alone. What a slide shows from the rest of
  the quiz comes in a `SlideContext` (`quiz.ts`): its question number and,
  for an explanation slide, the question it explains. `MediaRootContext` sits above
  every screen that resolves media, so the editor, the Director and their
  sound hooks agree on the folder.
- **Spreadsheets.** File › Import and Export (spec QZ-8, columns in
  quiz-format.md). `quiz_file::read_tables` reads a workbook (calamine,
  visible worksheets) or a CSV file (encoding and separator detected) into
  `Table`s, within 50 MB, 10,000 rows, 200 columns and 16 MB of text
  (counted per cell, so cells naming one shared string cannot multiply it);
  a sheet without cells is left out. Excel sheets are read cell by cell, so
  a far-off cell is refused before it takes memory; the parser reads `.xls`
  and `.ods` sheets whole, expanding repeated rows and cells first.
  It runs in a process of its own, the app started again with
  `--read-sheet` (`sheet_reader.rs`), which reports the tables as JSON
  (`SheetRead`, at most `MAX_SHEET_REPORT_BYTES`, derived from those
  limits): a damaged or crafted file that makes the parser panic ends the
  reader, never a running show. The app checks the reader every 10 ms and
  stops it once it holds over 1 GB (resident and swapped memory on Linux,
  the physical footprint on macOS, committed memory on Windows), refusing
  the sheet as too large, or once it takes over a minute. `quiz_core::import_tables` turns them into slides and notes
  (pure; a seed from the shell shuffles unmarked answers); `import_sheet`
  then finds each media file, copies it in with `import_media` and drops a
  missing one with a note. The editor gives the slides fresh ids
  (`withFreshIds`), formats a row that named a category like it
  (`formatImported`), inserts them after the current slide as one batch,
  selects them as a paste does and shows the notes
  (`ImportNotesDialog`, worded by `messages.ts`). Export is the reverse:
  `export_table` builds the rows and `export_sheet` places each media file
  next to the CSV (`place_file`, shared with media imports: the same name
  with the same bytes is reused, another gets a number) and writes the file
  atomically, skipping a media reference that would leave the quiz
  (`SheetExport.refused`). `quiz-file` holds the extensions
  (`SHEET_EXTENSIONS`, `CSV_EXTENSION`); `sheet.ts` holds the separator per
  language and the sample sheet.
- **Test run.** F5 (from the start) and Shift+F5 (from the selected slide)
  start a show of the draft with keyboard and on-screen buzzers. The
  Director shows a test run badge and "Back to the editor" ends the show
  and returns. A quiz with problems opens the File page instead.

### 1.4 Themes

A theme is the look of the stage: colours, two fonts, round or square
corners, titles as written or in capitals, and a background per screen
(spec 5.2). The quiz stores only its id (`Quiz.theme`). Themes are
data in the package layout of spec 5.2, so the built-in ones and the theme
packages users will install are read the same way.

- **Packages.** The built-in themes are folders in
  `app/src-tauri/themes/<id>/`: `theme.json`, `backgrounds/` and `fonts/`
  (with each font's licence). They ship as resources (`bundle.resources`)
  and the shell allows that folder in the asset protocol scope. Their
  `theme.json` files are also compiled into the shell (`themes.rs`), so the
  app always has its complete default theme; a test checks that every file
  they name is in the package.
- **Resolving.** `quiz_core::theme` reads `theme.json`
  (`ThemeManifest::from_json`): it refuses a file over 64 KB, an unknown
  `format` (TH-1), colours that are not `#rrggbb` or `#rrggbbaa`, corners
  and headings that are none of their values (TH-4) and file paths that leave the package or have the wrong type (TH-3), drops
  keys set to `"default"` (TH-2) and returns unknown keys, at the top level
  or in a section, for a warning instead of failing on them (TH-5). `resolve_themes` fills in every value from the parents, ending at
  `builtin.classic`, which must set them all; a missing parent or a loop
  ends there too, so a look never stops a quiz from opening or playing. A
  screen without a background takes the theme's own background of the
  screen it falls back to before any parent's. The app's own themes must
  all resolve, as they ship with it; every other theme stands alone, and
  one whose id is under `builtin.` or already taken is left out and
  reported (`ResolvedThemes::skipped`), so it cannot take the others down.
  Each file comes out as a
  `ThemeFile` (package folder and path), because one resolved theme can use
  files of several packages. The keys of each section (`ThemeColors`,
  `ThemeFonts`, `ThemeBackgrounds`, `BuzzerColors`) are written once, in a
  macro that makes both the key enum and the filled-in struct. Colour keys:
  `text`, `shadow`, `glow`, `panel`, `bar`, `barEdge`, `tile`, `accent`,
  `answerA`..`answerF`, `correct`, `wrong`. `corners` (`Corners`) and
  `headings` (`HeadingCase`) are single keys at the top level, inherited
  the same way.
- **In the windows.** `list_themes` returns the resolved themes, read once at
  start. `ThemesProvider` (`theme/context.tsx`) loads them once per window
  and `useResolvedTheme(id)` picks the quiz's (`pickTheme`: the default
  theme for an id that is not installed). Until they are loaded a stage has
  no theme and draws only the slide.
- **Drawing.** `ThemeContext` carries the resolved theme: the projector, the
  Director and the editor provide the quiz's. `Stage` turns it into
  `--theme-*` CSS variables (`themeStyle`) and draws the background. Which
  one is decided in one place: `resolveBackground(slide, theme, screen,
  builtIn)` (`theme/background.ts`, tested) lists the levels that can give
  the screen its background, the most specific first (BG-6: the slide's own,
  then the theme's for the screen, `screenOf(view)`, TH-5, and under another
  theme the built-in default's; round and quiz levels slot in between once
  the format has them). A slide style's gradient is a level of its own
  under the slide's file: it never fails, and the theme's background shows
  through where its colours are transparent. `StageBackground`
  (`slide-media.tsx`) draws the theme's picture or video, the gradient, and
  over them the first more specific level that loads; one that fails gives way to the next
  and is reported once per media folder (BG-9), so the Director warns about
  a broken file. The projector has nowhere to show a warning: it passes its
  own to the Director (`report_host_problem`, event `host-problem`), which
  shows it as "Projector: …". A theme's video loops silently. Every stage (projector,
  editor canvas, thumbnails) goes through `Stage`, and an overlay over it
  (the leaderboard) through `StageOverlay`, so none places a background
  itself. `slide.css` takes colours and fonts only from those
  variables, apart from a slide's own style: `textStyleCss`
  (`theme/slide-style.ts`, tested) turns a text's style into inline
  `--style-*` variables for only what the style changes, a size as
  `--text-scale` (which the text's `font-size` multiplies) and an answer's
  fill as `--answer-fill` under the theme's sheen, so a revealed or dimmed
  answer keeps its look. `slide.css` reads each variable with the theme's
  value as the fallback, so a font, a shadow or a corner is written once,
  in CSS. Every rounded corner in `slide.css` and `timer.css` is multiplied
  by `--theme-roundness` (1, or 0 for square corners), and the text in the
  heading font takes `--theme-heading-case` as its `text-transform`; a test
  checks that the texts the editor offers bold and with a
  shadow are the ones `slide.css` draws so. A title and a question share
  the headline's style.
  Text on a filled area (bars, answers, rows) gets white or dark
  text from `readableText`, which keeps at least the 3:1 contrast WCAG 2.2
  asks for large text (SC 1.4.3); a test checks every built-in colour. A
  slide's own fill without a text colour of its own gets the same, judged
  on the worse end of the gradient, unless the fill mostly shows through.
- **Fonts.** A theme font is a file. `registerFonts` (`theme/fonts.ts`) adds
  each file to `document.fonts` once (CSS Font Loading API), under a family
  named after the file, for every weight from 1 to 1000, so a variable font
  gets its real weights; the stage's font stack ends with system fonts for
  letters the file lacks. The built-in fonts (Inter, Nunito, Oswald, Exo 2,
  Montserrat, Outfit, Baloo 2, Caveat, Cinzel, Fira Sans Condensed) are
  WOFF2 files cut to Latin (Basic, Latin-1, Extended-A and B, so Czech) and
  common punctuation, 38 to 100 kB each; the newer six were cut from the
  Google Fonts TTFs with fontTools' `pyftsubset`. All but Fira Sans
  Condensed (SemiBold, a static font) are variable fonts. All are under the SIL Open Font
  License 1.1, which allows bundling as long as the licence goes with the
  font: each package has it as `fonts/OFL.txt`, and it ships with the
  installers. A cut font is a Modified Version under the licence, which may
  not keep a Reserved Font Name, so the built-in fonts are ones without one.
  Bakchus follows the pages of the Bakchus Aktivity website: black text on
  white, its orange `#E8520E` for scores and edges, bars and score rows in
  the dark `#1A1F28` of its photo captions, its light grey `#EDEDED` footer
  band under an orange line, square corners and Fira Sans Condensed titles
  in capitals. The site's body font, Raleway, has a Reserved Font Name, so
  Bakchus takes its parent Daylight's Montserrat for body text. Bakchus
  Contours is Bakchus over thin orange contour lines, all of one weight, so
  no line stands out from the rest; it sets only its background.
- **Choosing.** The editor's Design tab shows each installed theme on a
  sample question (`ThemeGallery.tsx`); when the ribbon is too narrow for
  all of them, the row pages with arrows at its ends (`GalleryStrip.tsx`)
  and the groups after it stay in sight. The theme under the pointer or focus
  is shown on the slide canvas only, the thumbnails keep the quiz's; a click
  is a `setTheme` edit, so it is undone like any other change.
  `useThemeChoice.ts` holds both themes and the gallery's choice. A quiz
  whose theme is not installed is drawn in the default one and the gallery
  and the Director's status bar say so (`isThemeMissing`); it never stops a
  show. A `theme` value that is not a theme id
  reads as none.
- **Preview.** A test of the shell writes the built-in themes as
  `list_themes` returns them to `app/src/bindings/BuiltinThemes.json`, with
  folders that Vite serves; the browser preview and the UI tests use it.

### 1.5 Teams and registration

- **Team names.** The game keeps the team names prepared for the show
  (`Game::teams`), set as a whole by `SetTeams`: `team_names` trims them,
  drops blanks and repeats (ignoring case), cuts them to `MAX_NAME_CHARS`
  and refuses more than `MAX_TEAMS` as soon as it sees one more, so a wrong
  file of thousands of lines does not hold the show state. Being commands,
  they are journaled and come back after a crash like everything else.
- **Kept between shows.** The shell (`teams.rs`) keeps the names in
  `<app data>/teams.json`, written atomically (`app_file.rs`, which the
  receiver settings use too), so they can be typed before a show and
  outlive it: ending a show, opening another quiz or a test run keeps them.
  A show starts with them as its first journaled `SetTeams`. The Director
  changes them with `add_teams` and `remove_team`, which work on the list
  the shell holds, so two quick edits cannot undo each other; in a show
  the change is journaled first and then saved. A `SetTeams` sent through
  `dispatch` (a window or a plugin) takes the same way, so the saved names
  never differ from the show's. The rules work on `Inner` and its teams
  file (`teams::begin`, `teams::change`) and are tested there.
  `teams-changed` tells the windows. A `teams.json` that does not hold
  names is renamed aside (`teams.json.broken`, then `.broken-2` and on, so
  an earlier one stays) so the next save does not overwrite it, and the
  Teams tab says so. One that cannot be read at all (another program holds
  it) is left as it is: its `AppFile` refuses to write it until the app
  starts again, so a passing lock never costs the saved names, learned
  buttons or sound board.
- **Pairing.** A team is paired with the buzzer whose player has its name,
  so the player's name is the only record; `GameView.teams` (`TeamView`)
  derives the pairs. `PairTeam` gives a buzzer a name and takes it from any
  other player; renaming a player in the table pairs or unpairs the same way.
- **Director.** `TeamList.tsx` sits beside the players table: buzzers
  without a team, the team names with their buzzers, and adding, pasting or
  loading names (`team-names.ts`: lines, or a CSV file's first column, in
  UTF-8 or Windows-1250). A buzzer number (`BuzzerChip`) is dragged onto a
  name with HTML drag and drop, or clicked and then the name clicked, which
  also works on a touch screen.
- **Projector.** The registration grid shows a paired team's name under its
  number.

### 1.6 Media player and sound board

- **Library.** The shell (`media_player.rs`) keeps the media player's
  files in `<app data>/library/media/`, copied in with
  `quiz_file::import_media` like the editor's media (same name and bytes
  reused, another file numbered; written as `.part` and renamed, so a
  failed copy leaves nothing). A file dropped into the window has no path
  in a web view, so `media_add_dropped` takes its bytes as a raw IPC body
  with its name in a header; the Director refuses drops over 256 MB, which
  Add copies from their path instead. The list is the folder's media files,
  read again after every change. Copies are made holding only the import
  lock, so the board and player answer while a big video comes in.
- **Board.** `quiz_core::SoundBoard` holds every place of the 8 × 8 grid
  and shows the first `rows` × `columns`, so pads keep their places when the
  grid changes size; `BoardChange` is every edit, checked there (a shown
  pad, a library file). Until the quizmaster changes it (`arranged`), the
  board takes new files by itself and holds each once. A file that leaves
  the library leaves its pads. It is saved as `library/soundboard.json`
  (`app_file.rs`, atomically); a broken file is kept aside.
- **Playing.** `quiz_core::MediaPlayer` keeps what plays (`NowPlaying`:
  the file, where it stood and since when it plays) and what a click on a
  pad does. It is no show state, so it is not journaled; `media-playing`
  (and `media_now` for a window that opens) tells the windows, with the
  show clock's time when it was sent (`MediaNow`). `since` is on the show
  clock, never the wall clock, which can jump: a window counts on from
  that time by its own monotonic clock (`showClockAt`). The Director plays sounds (`useMediaPlayer`, a
  `ClipPlayer` per start); the projector shows a picture or video over the
  stage (`MediaOverlay`) and seeks a video to where the player stands, so
  a projector opened mid-file joins in. The Director reads each file's
  length from its metadata, one file at a time (`media-durations.ts`), and
  sends `ended` when a sound or video reaches its end, so a video ends
  even with the projector closed; `ended` names the start (`id`), so the
  end of an earlier file stops nothing. The slide's sound and video pads
  play through the show (`useSlideSound`, `PlayVideo`). The one rule the
  UI repeats instead of asking the core is where a file stands
  (`positionAt` in `media-player.ts`, `NowPlaying::position_at`): the
  Director needs it every frame for the pads' countdowns. Both are tested
  with the same cases (`crates/quiz-core/tests/data/position-at.json`).
- **Volume.** The player's volume (0 to `MAX_PLAYER_VOLUME` percent) is the
  level everything the show plays sounds at: each window multiplies its
  sounds, videos and cues by it (`playerLevel`, `ClipPlayer::setGain`), and
  `media-volume` tells them at once, so what plays changes as the slider
  moves. It is kept in `library/player.json` (`app_file.rs`); a broken file
  is kept aside and the volume starts full. A drag sends every value it passes,
  faster than a disk writes, so `media_set_volume` (async, off the main
  thread) only changes the volume and tells it; the first step since the
  file was written writes it a quarter second later, with whatever volume
  the drag has come to by then, and the steps meanwhile write nothing.

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
                ──► show queue (with commands)  app/src-tauri/src/show_thread.rs
                ──► show thread (app)           show_thread.rs, pump.rs, hardware.rs
                    learn mode, raw log, UI events, time up
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
   them. A read error closes the device and reports `Error`
   (`ConnectionLost`; the thread reconnects).
3. If not connected and a receiver is selected: every 2 seconds list devices,
   open the first match, send the protocol's init reports, then report
   `Connected` (init goes first so lights work as soon as the UI says
   "connected").

Statuses: `Off` (nothing selected), `Searching` (selected, not plugged in),
`Connected` (with the `ProtocolKind` and whether it drives lights), `Error`
(a `ReceiverProblem` and the system's message; for `CannotOpen` the
Director adds what to check: the udev rule on Linux, Input Monitoring on
macOS, another program holding the receiver on Windows).

### 2.4 Protocols

```rust
trait Protocol: Send {
    fn kind(&self) -> ProtocolKind;
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
  next to the chosen receiver. Written atomically (temp file, then rename),
  and used only once written: if the write fails, the receiver keeps its
  previous settings, the Receiver tab says why, and learn mode ends all the
  same, so presses play again.

Limits: learned mode cannot drive lights (the light commands are unknown) and
it assumes one press per report. If the receiver packs several presses into
one report, or encodes the buzzer ID in a way that is not a fixed byte
pattern, learn mode will show conflicts and the vendor protocol is needed.

### 2.6 From press to screen

The show thread receives `Press` events in receiver order and
(`hardware::handle_event`) calls `execute_at(Command::Press { buzzer, button }, at_ms)` for
each, one at a time, under the app state lock. `at_ms` is when the receiver
thread read the report, not when the press is applied, so waiting for the
lock or for earlier presses' fsync costs a team no points. A press that
changes nothing (a second answer, a press after time up; `Game::ignores`)
stops there: it is neither journaled nor shown, so a burst of them costs no
disk writes. `execute_at`
applies the command to a copy of the game, appends it to the journal
(fsync'd file), swaps the new state in, sends light effects to the driver,
and only then emits the new state to the windows. If the journal write
fails, the press is rejected and the screen does not change. A new show's
session folder, and the journal's entry in it, are synced to disk when the
show starts, so a power cut cannot lose the journal itself. Commands from
the windows (`dispatch`: keyboard and on-screen buzzers, the quizmaster's
buttons) are timed the same way: `show_thread::run` reads the time before
the command joins the show queue, so a keyboard press made during a burst
of buzzer presses counts as on time.

Resuming (`resume_show`) reads the latest session folder: the one with the
highest number, as a folder is named by its show's start in milliseconds,
or a number past the last folder's when the clock went back, so two shows
never share one. The journal is read as bytes (a byte order mark and CRLF
line ends, which an editor may add, are taken as they come). Each line is
synced before the next is written, so only the last line can be one the app
never accepted: one without its newline, which was being written when the
app stopped, or a whole one that is not JSON at all or holds a NUL, which a
power cut leaves when the disk kept the newline but not the bytes before
it. Resume cuts such a line off, after keeping the journal as it was in
`torn/journal-<n>.jsonl` (a copy per cut; when the copy fails, as on a full
disk, the log says so and the show resumes all the same). Any other line
that does not read as a command (one from a newer app, or one the disk
damaged before the end) refuses the resume with `journalDamaged` and leaves
the journal as it is, since replaying without it would rebuild another show.
A show that stopped mid-question has its question restarted
(`Game::recovery_command` gives `RestartQuestion`, which takes the
question's score changes back, so a wrong answer before the crash is not
charged twice), and that command is journaled before `show-changed`
reaches the windows, so no window shows the stopped question. The
quizmaster can restart a question in play too (Restart question, after a
confirmation): it is the way to leave a question without scoring it, since
a jump is refused then. Score changes made by hand (`AdjustScore`) are not
the question's and stay.
The Director keeps a resume problem on screen, under the Resume button,
until another show loads. When a failed journal write cannot be taken back,
the session refuses every later command (`journalBroken`) until the show is
resumed, which cuts the torn line off. If the line was written whole and
only syncing it failed, it is not torn: resuming replays it, so the show
then has the change that was refused.

Because a press can be applied a moment after it was received, the core
judges it by its `at_ms`: it counts only if it was received while the
question clock ran (or stood paused) and before time up. The clock keeps the
time presses count from (`Clock::presses_from_ms`): when the question
started, or when it went on after a wrong answer. So a press made while
another player answered never counts, and one made during the quizmaster's
pause counts even when it is applied after the resume. The same thread
closes a question `PRESS_GRACE_MS` (250 ms) after time up, and only after it
has handled every event received by that moment (`pump::Pump::once`), so a
press that came in on time counts however far behind the thread is; the
grace covers presses the receiver thread has read but not yet passed on.
Later presses are refused. Each round, the thread reads the clock once,
handles the events received up to then and ticks at that moment; events
that come in meanwhile wait for the next round, so a burst of presses never
holds the clock or time up back.

Fastest-finger order is therefore the receiver's order. When the vendor
protocol provides press times, `Command::Press` gets an optional
`pressed_at` from the receiver and the game ranks presses that arrive close
together by that time; the PC clock is never used for ranking. A keyboard
or on-screen buzzer press competes with the buzzers by the order the
presses are applied in, so one received a moment before a buzzer press but
applied after it loses (`at_ms` only decides whether a press is on time).
Ranking across sources by `at_ms` would mean holding every buzz open for a
while and re-ordering presses on the PC, which BZ-5 rules out; keyboard and
on-screen buzzers are for shows without the hardware and for testing.

While learn mode is active, presses are shown in the Receiver tab but not
sent to the game. So that it can never mute the buzzers in a show, learn
mode cannot start while a show runs (`showRunning`), ends when a show
starts, when the receiver disconnects and when the Receiver tab closes, and
the Director shows a banner while it is on. The learn lock is taken before
the show state's both when a show starts and when learning starts, so
neither can slip in between the other's check and its start.

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
| App data (journal, `receiver.json`, settings) | `%APPDATA%\cz.kodajek.quizshow` | `~/Library/Application Support/cz.kodajek.quizshow` | `~/.local/share/cz.kodajek.quizshow` |
| This computer's data (unpacked quizzes, editor drafts) | `%LOCALAPPDATA%\cz.kodajek.quizshow` | as app data | as app data |
| Log (`buzzdome.log`) | `%LOCALAPPDATA%\cz.kodajek.quizshow\logs` | `~/Library/Logs/cz.kodajek.quizshow` | `~/.local/share/cz.kodajek.quizshow/logs` |

## 4. Build and release

`.github/workflows/build.yml`:

1. `check` (Ubuntu 22.04, every push): ESLint, type-check and build the UI,
   `cargo clippy -D warnings`, `cargo test --workspace` from an empty
   bindings folder, then fail if `app/src/bindings` changed.
2. `installers` (matrix, after `check`, on `main`, tags `v*` and manual
   runs, because macOS minutes are expensive): Windows NSIS, macOS universal DMG
   (`aarch64` and `x86_64` targets), Linux deb and AppImage on Ubuntu 22.04
   so the AppImage runs on older distributions. Each uploads its files as a
   run artifact (`buzzdome-windows`, `buzzdome-macos`, `buzzdome-linux`).

Signing and notarization need a Windows code-signing certificate and an Apple
Developer ID; until then builds are unsigned (macOS ad-hoc), see PL-5.

## 5. Extension points (for sections 5, 6 and 10 of the specification)

Not built yet; this is where they will attach, so current code does not block
them.

- **Theme packages and backgrounds.** Built-in themes are done (1.4), as
  packages that `quiz_core::theme` reads and resolves. Round and quiz
  backgrounds join the slide background in front of the theme's (slide >
  round > quiz > theme > built-in). Installed theme packages unpack into
  `<app data>/themes/<id>/`, go through the same `ThemeManifest::from_json`
  and `resolve_themes` as the built-in ones, and are served to the web view
  through Tauri's asset protocol, limited to that folder. Quiz files copy
  used backgrounds into their `media/` folder on save.
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
| Properties panel and several selected slides: search, More options, drag a name, Reset, Copy and Paste, Mixed values, dark panel, alphabetical list; Ctrl and Shift click, Ctrl+A, delete, move | `npm test` (`property-rows`, `selection`) and 38 checks in headless Chromium against the preview | pass |
| Quiz files: save, open, reuse, unsafe entries, size limit, missing media | `cargo test -p quiz-file` | pass |
| Themes: `theme.json` rules, inheritance, built-in packages complete with their files and font licences; gallery, fonts, a broken theme background warns once | `cargo test` (`quiz_core::theme`, shell `themes`), `npm test`, headless Chromium against the preview | pass |
| Teams: names, pairing, replay, names kept between shows; drag, click, paste and CSV import (Windows-1250), preparing names before a show | `cargo test` (`game_flow`, shell `teams`, `app_file`), `npm test` (`team-names`), headless Chromium against the preview | pass |
| Media player and sound board: library list and order, board rules (filling, placing, swapping, resizing, mending a stored board), playing, pausing, pad clicks, ends; Director tab (search, pads lit and counting down, menus, drag and drop, layout) and the projector's picture and video | `cargo test` (`quiz_core::sound_board`, `media_player`, shell `media_player`, `quiz-file` imports), `npm test` (`media-player`), headless Chromium against the preview with generated sounds | pass |
| Spreadsheets: columns, prefixes, settings, notes, shuffling, rounds per sheet, CSV encodings and separators, workbooks, media found and copied, export and re-import; File › Import and Export, notes dialog, one undo step, sample sheet | `cargo test` (`quiz_core::sheet`, `quiz-file` sheet), `npm test` (`sheet`, `slides`, `messages`), headless Chromium against the preview | pass |
| Loudness measurement | `npm test`: a full-scale 997 Hz sine in one channel measures -3.01 LUFS as BS.1770-4 states; gating, channel sum, part, peak limit | pass |
| Trim and Normalize volume windows, video trim window, background video menu and options, slide video (intro pause, Play video, speed, volume, part, holding with the show, blur and mosaic, replay) | headless Chromium against the preview with generated test media (a song and a test-pattern video, not in the repo) | pass |
| Media through the asset protocol, autoplay of video and sound | needs the installed app on each OS | **not tested** |
| Windows and macOS installers | CI | see the PR's checks |
| Real receiver: detection, raw reports, learn mode, presses in a show | needs the hardware | **not tested** |
| Lights, press times, battery | need the vendor protocol | **not built** |
