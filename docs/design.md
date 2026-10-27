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
shows of the show is either the `GameView` the core builds or a value the
core computes (the points a correct answer earns now, whether an answer is
right), so the two windows cannot disagree with each other or with the
score. What follows from the quiz alone, such as a question's number, is
worked out from the quiz in `quiz.ts` (1.3, Rendering), as the editor
draws slides without a show.

- **Types.** Every type that crosses the boundary is defined once in Rust and
  exported to TypeScript by ts-rs (`#[derive(TS)] #[ts(export)]`) into
  `app/src/bindings` when `cargo test` runs. `app/src/types.ts` re-exports
  them. Constants the UI needs (`MAX_ANSWERS`, question defaults and limits,
  `FINAL_LAST_STEP`, `DEFAULT_THEME_ID`) go to `bindings/Constants.ts` the
  same way, media extensions to `bindings/MediaExtensions.ts` and the
  built-in themes to `bindings/BuiltinThemes.ts` (1.4). CI fails when the committed
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
  screen, however long the journal write took. Presses and score changes are never dropped,
  nor is Restart question, which the quizmaster confirms first.
  A step command also carries the step the window shows (`GameView.stepNo`,
  `Game::step_no`, counted in `Game::apply` by `same_step`), and `dispatch`
  drops it, as the guard would, when the show has taken another step since:
  a press made as time ran out, a player buzzed in, the show took a step
  by itself or another window took one, before this window showed it, is
  not run on the step after. Pausing or resuming the clock and the
  leaderboard are no steps; another buzzer buzzing in is. A dropped
  command also sends that window the show as it stands, so a window that
  missed a view is not left sending stale steps; and a window loads its
  first view only once it listens for the next (`useReceivedValue`), so a
  step taken as it opens is not missed.
  Show keys count as typing only in text fields: after the quizmaster
  touched a slider, checkbox or list, the keys still work, and a slider
  lets go of the focus when released.
- **Where Next goes.** `Game::following` decides it: the next slide, or
  after the last slide the final ranking, unless the quiz has a final slide
  of its own (`Quiz::final_slide`), which then is the ranking and the show
  ends with the last slide. `controls` and `GameView.next_slide` use the
  same function.
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
  The show thread (`show_thread.rs`) applies due timer commands (time up,
  and the step the quiz's settings take once everyone answered) and
  otherwise sends `clock-tick` with the core's `ClockView`. The UI never
  reads the wall clock: what plays counts on from the show clock it heard,
  by one monotonic clock (`showClockNow` and `msSinceHeard` in
  `media-player.ts`; ESLint refuses other clock reads anywhere in the UI,
  but for a few marked ones that are not the show's), and the gauge
  animates between ticks with CSS.
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
  The close request's lock is taken under the show state when a show begins
  or ends (an unanswered request is forgotten then), and holds no other.
  Every command that writes a file, or lists the USB devices (slow on
  Windows), runs off the windows' thread too: the app's files are in its
  data folder with the journal, so a disk that does not answer holds both.
  A lock that a command on the windows' thread takes (the media library,
  the icon colour) is never held while the disk works: their files (the
  sound board, the volume, the icon) each have a lock of their own, held
  while written, so the last change is the one kept.
  `receiver.json` changes one at a time (`SettingsChange`, whose lock is
  never taken under the show state), so two quick clicks cannot lose either
  change; and every atomic write has a temporary file of its own
  (`quiz_file::write_atomic`). The editor's commands that open, save or
  copy large files, or read a sheet, are refused during a show.
- **Saving problems.** When the journal refuses a write (the disk is full,
  say), the change is not made, and the Director shows why in a banner until
  a write works again (`show-not-saved`, `show_not_saved`). A due command
  that failed is tried again a second later. A write that does not finish
  (the disk does not answer) holds the show thread up: once one thing has
  held it for 3 s (`STUCK_AFTER`), a watchdog shows that in the banner
  (`saveHangs`), and a command the thread has not begun is given up
  (`showBusy`) and never made, so the quizmaster can give it again without
  it happening twice. A command behind a burst of quick writes is only
  waited for. The step being saved is made once the write finishes, which
  the banner says, so it is not given twice. Presses, from the buzzers,
  keys and on-screen buzzers alike, are never given up: they wait in the
  queue and count by their time once the write finishes.
- **Windows.** `app_windows.rs` keeps both windows through a show. Closing
  the Director during a show asks first (`close-requested`), and the app
  ends with the Director, so a full-screen projector is never left alone.
  The question is asked above the Director's screen (`App.tsx`), so it
  comes even when the screen failed to render. A second close in the next
  10 s adds no dialog. A Director that does not answer within 10 s (its
  page hangs) does not keep the app open: a close in the rest of that
  minute goes through, whether or not the dialog is still open. A close
  after that minute, or after a show began or ended (`replace_show`), asks
  again, so a request whose answer was lost does not let an accidental
  close through; a dialog still open by then gets a second one. On macOS the app menu has a Quit of its own
  (`app_menu`, worded in the UI's language by `set_menu_words`), as the
  system's ends the app at once: Cmd+Q asks like the close button, and
  only Quit from the Dock ends a show without asking (it stays saved).
  Problems a window has nowhere to show (the close dialog not opening)
  go in the log (`log_problem`), as do the projector's.
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
  the show and its media, but not open, save or pick files, and `dispatch`
  takes from it only what its keys stand for (`may_give` in `lib.rs`: the
  `PROJECTOR_COMMANDS`, written to `bindings/ProjectorCommands.ts`, where a
  test in `keyboard.test.ts` checks that every command a key gives is in
  it, and presses of the big button of buzzers 1 to 10 only); a window the
  app does not know may give none.
  A content security policy (`tauri.conf.json`) lets the pages load only
  the app's own scripts and styles and media through the asset protocol.
  Tauri sets it only on the pages a release build serves, not on the dev
  server's, so the release smoke check in `docs/hardware-test.md` runs
  under it; what it blocks goes in the log (`csp-report.ts`). Files the
  quizmaster opens, saves, imports or exports are picked by the backend
  (`dialogs.rs`, worded by the UI's `Picker`, whose offered name, a quiz
  title, the backend turns into a portable file name), never named by a
  page: the recent quiz files, which the editor opens and the Director
  plays, are kept by the backend (`recent_files.rs`), and the page opens
  one by its path, which has to be in that list; a file leaves the list
  once it is gone from its folder. The
  pages confirm through the dialog plugin's `ask`
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
- **Director look (SF-10).** `director/look.ts` keeps the choice (light,
  dark or as the system) in the window's preferences, and the Director's
  root takes the class `dark` while it shows dark. Every colour of
  `director.css` is a custom property of `.director`, which `.director.dark`
  sets again, so the two looks cannot drift apart; the command panel,
  toolbars and sound board are dark in both. `.director.dark` applies on
  screen only, so what the Director prints (the results, SF-11) is light. The editor's popups the
  Director borrows (menus, dialogs, colour, gradient and font pickers)
  read `--popup-*` properties with their light colours as fallbacks, which
  `.director.dark` sets, as the editor's dark properties panel does; the
  light switches get their dark colours in `director.css`.
  While the dark look is on screen the window's title bar is set dark
  (`setTheme`, which draws it on Windows), and left to the system again
  otherwise, so the light look changes nothing there.
- **Check before the show (SF-2).** `director/launch-check.ts` decides
  how each part stands from what the Director has anyway (the receiver's
  info, the view's players and teams, the volumes) and from two reads the
  backend does not push: `projector_window` (whether the projector window
  is full screen and on the Director's screen) and `show_disk_space` (free
  space where the journals are written, through `fs4`). `LaunchCheck.tsx`
  shows them below the tabs while the show is in the lobby, and reads those
  two again every 2 and 10 seconds while it is on screen. Hide puts it away
  until another show starts; Settings turns it off for the computer
  (`preferences.ts`). Only a person can tell that sound reaches the hall,
  so the check plays the app's own right-answer sound at the show's
  volume and asks.
- **Sound.** The Director window plays the show's sounds and the slides'
  sound clips, because it stays open for the whole show and the projector
  window may not. Spec SN-5 moves playback into the Rust shell later. The
  code is in `app/src/audio/`: `context.ts` (one shared `AudioContext`),
  `library.ts` (the app's sounds and what each moment plays),
  `show-audio.ts` and `show-sounds.ts` (playing them), `clip-player.ts` (`PartPlayer` plays a part of any media
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
  - The show's sounds (spec SN-1 to SN-4) are in the quiz's `sounds`
    (`quiz_core::QuizSounds`, docs/quiz-format.md): a choice per moment
    (`SoundSlot`), kept only where it is not the default, and two volumes.
    The core says when: an effect comes as `Effect::Sound` with its `Cue`
    (the `sound-cue` event), and the music as `GameView.music`, the moment
    the show is at (`Game::music`), so a resumed show plays what it played.
    The countdown is the one effect the clock plays: `useCountdown` starts
    it from the clock's latest reading so that it ends as the time is up.
  - The app's sounds (`app/src/audio/library/`) are read whole and play
    from `AudioBuffer`s, to the sample; the last 20 read stay read (a piece
    of music takes up to 18 MB once decoded, an effect about 1 MB). A piece of
    music loops by starting each round `loopMs` after the one before, set
    on the audio clock up to 2 s ahead, while the file's tail (a reverb, a
    last chord) rings on under the next round: no seam and no gap. A quiz's
    own files may be long, so they stream through `ClipPlayer` into the
    same nodes; a file an effect plays stays open, so it starts at once.
  - Music fades in (0.4 s), out (1.2 s) when the moment changes, and away
    (0.3 s) while other sound plays (`otherSoundPlays`: the slide's sound,
    a slide video with sound, a sound or video in the media player). Volumes: the quiz's
    effects or music volume times the player volume.
  - The library is made by `make_sounds.py` with NumPy alone, from
    oscillators, noise and filters, at 48 kHz, each sound at -16 LUFS
    (BS.1770-4, as SN-9), lowered where its decoded peak would pass
    -1 dBFS, as MP3 by FFmpeg's LAME encoder.
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
unsaved). So is an entry whose name is one file with an entry before it on
Windows or macOS (`quiz_core::file_key`: the names differ only in letter
case, which Windows compares in upper case and macOS in lower case, or in
how accents are encoded, which macOS ignores too), which would take its
place there; a show refuses a quiz that uses such a name
(`MediaNameClash`), whether or not it uses the entry before it, as it does
a name it would have to rename. Media added in the editor, and media
an export copies beside a sheet, take no name that is one file with a name
in the folder, also on Linux. A plain `quiz.json` reads
only safe names from the disk (no device names on Windows), nor any
through a link that leads out of its folder, and the show path keeps such
a file refused.
A `.quiz` file of more than 10,000 entries (`MAX_ENTRIES`) is refused. Its
ZIP64 end records are checked before the archive is opened (`zip_end.rs`),
as opening reads every entry header the record it takes counts, and a file
of more than 64 end records (`MAX_END_RECORDS`) is refused as not a quiz
file, as the reader would read the entry headers again for each one.
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
(`ReadQuiz::dropped`): a `theme` that is no theme id (`ThemeUnread`; the
quiz then has the default look), style values that did not read
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
quiz-core bumps whenever the same journal would replay into another show.
What would put the replay a step off is kept from the show's own version,
and the game keeps that version to the show's end (`Game::replay`), so
what is journaled after a resume replays the same way at the next one: a
show begun before version 2 jumps out of a question in play, as the app
then restarted a question after a crash with a `GoTo` to it, one begun
before version 3 switches a player off without closing the question,
one begun before version 4 restarts a closed question, and one begun
before version 6 asks a question it goes back to again. Version 3 also
changed what is not kept so (ties, accents in names), so a show begun
before it, or under a newer app's rules, resumes with a warning that its
scores may differ (`RulesChanged`); from version 3 on
(`REPLAYS_AS_PLAYED_FROM`) a show replays as it was played and resumes
without one.

The windows load media through Tauri's asset protocol. Its scope is empty
in `tauri.conf.json`; at startup the shell allows the folder of unpacked
quizzes, the editor's drafts folder, the built-in theme packages and the
media player's library (`<app data>/library`, spec SN-7)
(`asset_protocol_scope().allow_directory`), and nothing changes it later,
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
  or the draft's `quiz.json` does not read, so its folder is not pruned
  unseen.
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
  for an explanation slide, the question it explains. It follows from the
  quiz alone, not from the show, so `quiz.ts` is its one place, for the
  editor, which draws slides without a show, and for the show's windows
  alike; so is the line a slide is listed by (`slideTitle`), which the
  Director shows for the next slide (`GameView.next_slide`). `MediaRootContext` sits above
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
  backend begins it at the selected slide in the same call
  (`editor_test_run`), as no window has seen the new show yet to send the
  jump for (one step per press). The Director shows a test run badge and
  "Back to the editor" ends the show and returns. A quiz with problems
  opens the File page instead.

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
  `format` (TH-1), colours that are not `#rrggbb` or `#rrggbbaa` (TH-4)
  and file paths that leave the package or have the wrong type (TH-3),
  drops keys set to `"default"` (TH-2) and returns unknown keys, at the
  top level or in a section, and corners or headings that are none of
  their values (a newer version's, say; the parent's value is used), for a
  warning instead of failing on them (TH-5). `resolve_themes` fills in every value from the parents, ending at
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
  `list_themes` returns them to `app/src/bindings/BuiltinThemes.ts`, with
  folders that Vite serves; the browser preview and the UI tests use it.
  It is TypeScript typed as `Theme`, so the compiler checks the named
  values that JSON would type as any string.

### 1.5 Teams and registration

- **Team names.** The game keeps the team names prepared for the show
  (`Game::teams`), set as a whole by `SetTeams`: `team_names` trims them,
  drops blanks and repeats, cuts them to `MAX_NAME_CHARS`
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
- **Names.** Player and team names are kept with their accents encoded
  as single characters (NFC). They and category names are compared
  ignoring case and that encoding (`text::name_key`), so "é" typed and
  "é" pasted from a macOS file name, which encodes it as "e" and an
  accent, are the same name.
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
- **Scores.** A right everyone-answers answer scores the points the clock
  shows when it was given (`Question::points_at`, falling or rising from
  `points` to `points_end` over the time limit), times its share; a wrong
  one costs the penalty, and no answer costs nothing. A fastest-finger buzz judged right scores the points at
  the moment of the buzz, one judged wrong costs the penalty. An untimed
  question always scores `points`.
- **Who counts.** When an everyone-answers question closes, the players
  switched on then are the ones it counts (`Game::counted`): its scores,
  the result bars, the answer counts and the result lights go by them,
  whoever is switched on or off afterwards. A question closes on time up,
  on Next, or when nobody switched on is left to answer: everyone answered,
  or in fastest finger everyone answered wrong. Switching a player on or
  off is checked the same way, so switching off the last player who had
  not answered closes the question. A player switched off cannot answer;
  one switched on mid-question can, unless they already answered or were
  locked out.
- **Everyone answered (QZ-11).** A question that closed for having nobody
  left to answer notes when (`Game::auto_step_from`), and the quiz's
  `settings.allAnswered` decides what follows: wait for Next, as before,
  or after `allAnsweredDelayS` the step Next would take (the reveal) or the
  jump to the slide Next would go on to (`GoTo`, or `GoToFinal` past the
  last). `Game::pending_step` says which and when; `due_command` issues it
  through the show thread like time up, so it is journaled and a replay
  ends where the show did, and `GameView::auto_step` shows it coming in the
  Director with a bar that runs out in CSS. The quizmaster's Next leaves
  the closed step first, and `PauseClock` holds the step (the Director
  calls it Hold): the question then waits for Next. Opening the
  leaderboard and playing the slide's video again hold it as well, since
  the quizmaster has taken over, and a show resumed after a crash holds it
  (`recovery_command`), so the quizmaster first sees where it stands. A question closed by
  time up or by the quizmaster has no `auto_step_from` and always waits.
  With `settings.nextAfterReveal` the reveal starts the same count
  (`Game::reveal`, also for a fastest finger judged right): after
  `nextAfterRevealDelayS` the show takes the next slide's step the same
  way, held the same way. A question shown again with its answer on a
  revisit (rules version 6) does not count, as the quizmaster went there.
  The settings come with the quiz the session keeps, so a resumed show
  goes on with them; no rules version is needed, as a quiz without them
  plays as before. Space pressed as the reveal comes is meant for the step
  before and is dropped (one step per press, above), so it does not also
  leave the slide.
- **Other quiz settings (QZ-11).** `runClockOut` keeps a timed question
  running once nobody is left to answer (`Game::closes_as_answered`); its
  time up then closes it as everyone answered, so `allAnswered` applies.
  `changeAnswer` lets a press of another answer button replace the
  player's answer and its time while an everyone-answers question runs.
  `oneBuzz` makes everyone sit out a fastest-finger question once a buzz
  in it was judged wrong (`Game::sits_out`), which leaves nobody to
  answer. `noAnswerPenalty` gives the players an everyone-answers question
  counts and who did not answer the question's penalty, as a wrong
  answer. `hideAnswer`, `hideBuzzPoints` and `answerChart` change only
  what the projector shows (`GameView::settings`); the Director and the
  scores stay the same. The answer chart's bars come from the view's
  `answer_counts` over the players the question counted (`results`), so
  they show what the counts on the answers already say, as shares.
  None needs a rules version: the settings travel with the quiz in the
  session, and a quiz without them plays as before.
- **Jokers (SF-8).** `Game::jokers` keeps the questions players play
  their joker on, as (slide, buzzer) pairs: the ones that ended and the
  current one, so a player's jokers left are `settings.jokers` less their
  pairs, and a joker is spent when it is played. `SetJoker` plays or takes
  back the current question's while it is ready or running and has given
  the player nothing (`Game::can_set_joker`, no mark of theirs in
  `asking`), so a joker always covers all or nothing of what the question
  gives. `credit` doubles the points, gain or penalty, and the player's
  mark keeps the doubled amount, so `RestartQuestion` takes back what was
  given and the joker stays on the question asked again. Leaving a question before it ended
  (`enter_slide` to another slide) drops its pairs, as the question was
  never asked. The command is journaled like any other and only exists in
  shows played since, so no rules version is needed.
- **Reset scores and start over (SF-9).** `ResetScores` sets every
  player's `score`, `total_answer_ms` and `adjusted` to zero and empties
  the results (`asked` and `asking`, SF-11), so `RestartQuestion` has
  nothing left to take back; the slide, the jokers
  played and `closed_questions` stay, so a question that ended is still
  not asked twice. `StartOver` does the same, empties `jokers` and
  `closed_questions` and enters the lobby on slide 0 (`Game::enter`, which
  `enter_slide` calls with the slide's own phase): the players, their
  names, who is switched on and the team names stay, and the rest is as in
  a new show. Both are refused while a question is in play, as the jumps
  are (`Controls::go_to`), so no question's score changes are split by
  them. They only exist in shows played since, so no rules version is
  needed. The Director's buttons for them ask first.
- **Results (SF-11).** A question as asked (`Asked`) keeps what it gave
  each player it scored as a `Mark`: their answer or buzz and its time,
  whether it was right, the points (doubled by a joker) and whether a
  joker was played. `credit` is the one place a question changes a score,
  and it writes the mark, so the history cannot drift from the scores.
  The current question's marks are `asking`, made when the question first
  credits a player or ends; entering another slide moves them to `asked`,
  and `RestartQuestion` takes its points and answering time back from
  them. `AdjustScore` also adds to `Player::adjusted`, so a score is
  always the sum of the player's marks and `adjusted`: show-sim checks
  this, and that `total_answer_ms` is the sum of the right answers' times,
  after every command of its shows (unless a score reached the `i32`
  limits). `Game::show_results` reads the history: the questions in the
  order they were asked, each with its round (an end-of-round slide closes
  one), and every player in the ranking's order (`Game::standings`, which
  the view's ranking uses too) with what each round gave them. The history
  is game state, rebuilt by replaying the journal like the rest, so it
  survives a crash and needs no rules version: no command changes what it
  does to a show.
  The Director's Results tab loads the results again whenever the view
  changes, one load at a time. Export to CSV writes the table that
  `director/results.ts` lays out through `quiz_file::export_results`:
  UTF-8 with a BOM, `;` and a decimal comma in Czech, `,` in English, and
  a cell that would start a formula gets a leading apostrophe, as in the
  question export. Print opens the system's print dialog for the
  Director's window (`Webview::print`), which can also save a PDF; the
  print style leaves the results alone on the page, under the quiz's
  title, on as many pages as they take.
- **Ranking.** `Game::ranking` orders the players switched on by score,
  then by the time their right answers took together (`total_answer_ms`,
  less first); wrong answers add no time, so not answering never helps.
  Players equal in both share a place and the places after them are
  skipped (1, 1, 3). On the final podium (`host/podium.ts`) players who
  share a place stand on its block (more than three: two named, the rest
  counted), and a place the game skips leaves its block empty.
  The original breaks ties on total answering time (`replica/recon.md`);
  counting only right answers is a choice made on purpose, with its price:
  at equal score, a player whose extra right answers were cancelled by
  penalties has spent more time and ranks below one with fewer right
  answers. The rule came with rules version 3 and has no older variant, so
  in a show begun before it, which resumes with the Director's warning
  that scores may differ (`RulesChanged`), tied players may swap places.

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

### 1.7 AI assistants

How it is used: [`ai-assistants.md`](ai-assistants.md) (spec QZ-12).
`crates/quiz-ai` gives assistants the quiz format and the app's checks,
and has no rules of its own: what it accepts is what `quiz_file` and
`Quiz::validate` accept.

- **Schema.** `quiz_schema()` is `schemars::schema_for!(Quiz)`: the quiz
  model's types derive `JsonSchema` beside `Serialize` and `TS`, so the
  schema follows every change to them, with their doc comments as
  descriptions. A field read through a `with` module (lenient styles, the
  old background form) says which type it takes (`#[schemars(with)]`);
  ranges are given where the type enforces them (`TextSize`,
  `Transparency`, `schemaVersion`). Rules that relate fields stay in
  `Quiz::validate` and `quiz-format.md`.
- **Workbench.** `workbench` is what an assistant does with a quiz folder
  (`quiz.json` and `media/`, which the app opens as it is): `write_quiz`
  reads the JSON as a file would be read (`Quiz::read_json`, so what does
  not fit is left out and said), `add_media` copies through
  `quiz_file::import_media`, `check` lists `quiz_file::problems` (for a
  `.quiz` file `quiz_file::inspect`, which reads `quiz.json` and the
  names of the files without unpacking the media), `pack` is
  `quiz_file::save`, and `unpack` is `quiz_file::open_into`. A quiz is
  written only into a new folder, a quiz folder or one of media, and a
  `.quiz` file is replaced only when asked. The MCP tools and the
  `buzzdome-quiz` command line are thin layers over it.
- **Roots.** The MCP tools work only in the folders given with
  `--root` (`roots::Roots`), as the reference filesystem server does. A
  path counts by its real path (`canonicalize` of the nearest part that is
  there, so links and junctions are followed), must be absolute and has
  no `..`. Inside a quiz folder, the workbench also refuses a
  `quiz.json`, `media` folder or media file that leads out of the folder
  (`stays_inside`, with `quiz_file::leads_inside`) before it reads, packs
  or writes there. A root that holds or is inside the app's data folders
  (`roots::app_data_dirs`: Tauri's data, local data and, on macOS, log
  folders for the bundle identifier) is refused, so no tool reaches the
  show's journal, the drafts or the settings. The app passes its own
  identifier; the command line uses `roots::APP_IDENTIFIER`, which a test
  keeps equal to `tauri.conf.json`.
- **MCP server.** `mcp::serve` reads JSON-RPC 2.0 messages, one per line,
  from standard input and answers on standard output: `initialize` (the
  client's protocol version when it is one of `PROTOCOL_VERSIONS`), `ping`,
  `tools/list` and `tools/call`. Written by hand rather than with an SDK:
  nine tools, no notifications of its own and no async runtime. A tool's
  failure is a result with `isError` for the assistant to act on; only an
  unknown method or tool is a protocol error. Every path goes through
  `Roots::resolve`, and the instructions name the roots. The
  app serves it when started with `--mcp`: `ai::serve_mcp_if_asked` runs
  first in `run`, before the single-instance plugin and any window, so it
  neither shows the running app nor touches its show, and ends the process
  when the assistant closes its input. On Windows the release build is a
  GUI program; standard input and output work all the same when the parent
  passes pipes, as the sheet reader (`sheet_reader.rs`) relies on too.
- **Claude Desktop.** `claude_desktop::connect` adds
  `mcpServers.buzzdome = { command: <the app>, args: ["--mcp", "--root", <folder>] }` to
  Claude Desktop's `claude_desktop_config.json` and keeps the rest (keys
  may come out in another order). It refuses a file that is not a JSON
  object, so nothing of the user's is lost, and makes no folder: without
  Claude's folder, Claude Desktop is not installed. The folder is the
  `Buzzdome` folder of the quizmaster's documents, which the page makes,
  or one they pick (`ai_connect_claude`).
- **Skill.** `skill::write` zips `SKILL.md` and `make_quiz.py` (both in
  `crates/quiz-ai/src/skill/`), `docs/quiz-format.md` and the schema into
  one folder, as Claude takes a skill. The script is for the assistants'
  sandboxes, which cannot run this app: standard library only, a JSON
  Schema checker for the parts the schema uses, media present, and the
  ZIP written as `quiz_file::save` writes it. Media are every text that
  starts with `media/`, so a new media field needs no change there, and
  only `media/<portable file name>` is packed. The schema gives media
  fields the pattern `^media/` (`MEDIA_PATTERN`), which the script checks,
  so a reference outside `media/` is reported too. `SKILL.md` gets the core's
  answer limits and media types from `skill_md()`, never written by hand;
  `.gitattributes` (`* text=auto eol=lf`) keeps it and the format guide LF
  on every system. Its tests run it with Python
  and open what it wrote with `workbench::check`.

## 2. Buzzer link

### 2.1 Layers

```text
USB HID device ──► HidBackend / HidDevice      crates/buzzers/src/hid.rs
                   (hidapi in production,      hidapi_backend.rs
                    a fake in tests)
                ──► receiver thread             receiver.rs
                    open, read, reconnect,
                    write the mode and lights
                ──► Protocol                    protocol.rs, learned.rs,
                    report bytes → presses,     enjoy303s.rs (over cp2110.rs)
                    mode and lights → reports
                ──► EdgeFilter                  edge.rs
                    held button = one press
                ──► TimerOrder                  timer_order.rs
                    fastest finger in the
                    buzzers' own timer order
                ──► show queue (with commands)  app/src-tauri/src/show_thread.rs
                ──► show thread (app)           show_thread.rs, pump.rs, hardware.rs
                    learn mode, raw log, UI events, time up
                ──► execute(Command::Press)     app/src-tauri/src/lib.rs
                    journal, then game state, then lights and mode
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
  `ReceiverHandle` (a channel): select a device, set the buzzer mode and
  lights, reload the protocol, shut down.
- Besides input and output reports, a receiver may need feature reports to
  be set up (the Enjoy receivers' cable is a USB-to-serial bridge set up
  that way, 2.4). `HidDevice::send_feature_report` sends them, and each report a
  protocol gives says which kind it is (`hid::Report`).

### 2.3 Receiver thread

Loop, every iteration:

1. Drain control messages (select, mode, lights, reset lights, reload,
   shutdown). The thread keeps the buzzer mode and each buzzer's light as
   the app last set them; only what changed is encoded with the active
   protocol and queued for writing. A reset (`BuzzerDriver::reset_lights`,
   as a show begins or ends, 2.7) replaces every light: the protocol
   darkens every buzzer and then sends the lights it names
   (`Protocol::reset_lights`, which a protocol with lights overrides), as
   nothing says what the buzzers show before it, such as colours another
   program left. A buzzer it does not name stays dark unless a light then
   goes to all at once (2.4).
2. If connected: write the next queued report if it is due, then
   `read_timeout` (20 ms, or less when the next report or a held buzz is
   due sooner). A report goes through the protocol and the edge filter; the
   thread sends `Raw` (bytes, time, decoded presses, and those the protocol
   dropped as unable to count, `Protocol::take_dropped`) and then one `Press`
   event per press, in the order the protocol returned them, except that in
   a fastest-finger round presses with the buzzer's own timer wait for the
   timer order (2.6). The edge filter starts afresh as a fastest-finger
   round begins, so a press made just before it cannot hide the first one
   in it as a held button's repeat. A press that carries its buzzer's
   timer and repeats that buzzer's last one, timer and all, within two
   seconds is dropped as well, also after such a fresh start: it is the
   receiver sending it again, which the 303S did 180 and 541 ms later,
   twice in about 90 presses, in the hardware test of 2026-10-25 (a
   buzzer's timer moves on every millisecond). After every read, a report or a timeout, it queues
   what the protocol has to write now (`Protocol::follow_up`): what the
   receiver's messages call for, or a step again whose answer is overdue.
   A read or write error
   closes the device and reports `Error` (`ConnectionLost` or
   `WriteFailed`; the thread reconnects).
3. If not connected and the receiver is not closed: every 2 seconds list
   devices, open the first match and set it up: write the protocol's init reports,
   queue the mode and, as a reset, the lights the buzzers should show, then report
   `Connected` (init goes first so the receiver is set up as soon as the UI
   says "connected"). So a receiver unplugged mid-show gets the round and
   the lights it missed once it is back.

Writes go out one at a time, at least `write_gap` (10 ms) apart, as the
vendor's SDK paces them, so a slow receiver is not flooded; a report the
same as the one queued just before it is not queued twice. A protocol that
must hear from the receiver before it takes orders (the 303S, 2.4) keeps
the lights and mode it is given until then, and gives them as follow-up
reports with the answer, so however slowly the receiver answers, nothing
is sent too soon or lost; a set-up step left unanswered is asked again
until the receiver answers (2.4). A lost connection drops the queue; the
next set-up queues what is current.

What matches is the `ReceiverChoice`: a device chosen in the Receiver tab
(`Device`, by vendor and product ID, usage page and interface), or with
none chosen the first device a built-in protocol knows (`Automatic`,
`protocol::is_known`: today the 303S's cable), so the 303S needs no click
and a receiver plugged in later, also mid-show, is taken by the next
retry. A raw or learned receiver is used only once chosen, as any HID
device could be it. The app keeps the choice in `receiver.json` (none:
automatic); choosing again, also the same, opens the receiver anew, which
is the Receiver tab's Reconnect.

Statuses: `Off` (closed), `Looking` (automatic, nothing known plugged
in), `Searching` (chosen, not plugged in),
`Connected` (with the `ProtocolKind`, which for the 303S says whether the
receiver answered its set-up, and whether it drives lights), `Error`
(a `ReceiverProblem` and the system's message; for `CannotOpen` the
Director adds what to check: the udev rule on Linux, Input Monitoring on
macOS, another program holding the receiver on Windows).

### 2.4 Protocols

```rust
trait Protocol: Send {
    fn kind(&self) -> ProtocolKind;
    fn decode(&mut self, report: &[u8]) -> Vec<Press>;
    fn take_dropped(&mut self) -> Vec<Press>;                             // default: none
    fn init_reports(&self) -> Vec<Report>;                                // default: none
    fn follow_up(&mut self, now_ms: u64) -> Vec<Report>;                  // default: none
    fn encode_lights(&mut self, lights: &[(BuzzerId, Light)]) -> Vec<Report>; // default: none
    fn reset_lights(&mut self, lights: &[(BuzzerId, Light)]) -> Vec<Report>;  // default: encode_lights
    fn encode_mode(&mut self, mode: BuzzerMode) -> Vec<Report>;              // default: none
    fn has_lights(&self) -> bool;                                             // default: false
    fn learnable(&self) -> bool;                                              // default: false
}
```

The encoders take `&mut self`, so a protocol can keep what it told the
receiver (the 303S keeps each buzzer's colour, below). `follow_up`, taken
after every read or read timeout, gives what the reports decoded last call
for: the 303S's next set-up step, then the lights it kept until the
receiver opened, or a step again whose answer is overdue. `take_dropped`
gives the presses `decode` read but did not pass on, for the raw log.

`protocol::for_device` picks the protocol when a device opens, in this
order:

1. **Vendor protocol** for the device's vendor and product ID: the Enjoy
   303S (below). `decode` is stateful, so it can read messages split over
   reports.
2. **Learned protocol** if the user taught buttons for this vendor and
   product ID. Only the learned and the raw protocol are `learnable`: the
   Receiver tab offers learn mode, and `receiver_learn_start` starts it,
   only for those (`receiverNotLearnable` otherwise), as buttons taught
   for a receiver with a vendor protocol would never be used.
3. **Raw only**: decodes nothing, but every report still appears in the raw
   log, which is how the real protocol gets worked out.

#### Enjoy 303S

Jakub's set is the Enjoy 303S (RGB buzzers, sold as the Galaxy RGB set) with
the half-dome REC-A receiver. Enjoy does not publish its SDK. On 2026-10-21
Jakub got Enjoy's official SDK kit for the 303S (a C# demo, a usage guide
and prebuilt Windows libraries, no source); a project thread read it with
the unofficial copy of the SDK Jakub approved on 2026-10-20, whose RF219
driver carries the 303S's commands one to one as the guide lists them, and
wrote the protocol down in its own words. `enjoy303s.rs`, `cp2110.rs` and
this description come from that write-up. No file or code from the kit or
the copy is in the repository. The first test on a real receiver, on
Windows 11 with buzzers 1 and 2 on 2026-10-25
([`hardware-test.md`](hardware-test.md)), confirmed the cable, the
set-up, the buttons, the colours and the timer as written below; what it
could not check is marked **unverified**.

The older Enjoy RF215 dome buzzers use the same cable with another
protocol. Their driver was dropped once Jakub's set turned out to be the
303S: the cable cannot tell which buzzers are behind it, and a guess
between the two could go wrong mid-show.

- The USB-HID cable (VID:PID `10C4:1819`) is a Silicon Labs CP2110-type
  HID-to-UART bridge to the receiver's serial line (`cp2110.rs`, report
  layout from Silicon Labs AN434). Three feature reports open the UART:
  `50` with the settings (750000 baud as a big-endian u32, no parity, no
  flow control, 8 data bits, one stop bit), `43 03` to purge both FIFOs,
  `41 01` to enable it. Serial bytes go both ways in 64-byte reports whose
  ID is the number of bytes they carry (1 to 63), so a message can be split
  over reports and is read from a byte stream. The vendor's SDK purges the
  FIFOs before every command; the app purges them only on connect, as a
  purge also drops presses the receiver has just sent.
- A message is two header bytes, the payload's length, the payload and the
  XOR of the payload's bytes: `35 96` from the PC, `27 A5` from the
  receiver. A stream that does not line up, or a message whose check fails,
  is read on a byte at a time; the start of a message that cannot be whole
  is skipped when a whole one follows it, so a spoiled length byte holds up
  no press.
- Set-up: connect `10 00` (the answer starts `10`; it was `10 82`), take
  buzzers 1 to 64 (`14 3F`: radio IDs 0 to 63, answer `14 01`), open the
  receiver `12 00` (answer `12 01`). Each step waits for the answer to the
  one before (`follow_up`), and the lights wait for the last; in the test
  the whole set-up was answered within about 50 ms. A step not answered within 500 ms (the SDK
  waits 200 ms) is sent again, connect with the cable's settings before it
  as after plugging in, so a lost or spoiled answer, or a receiver powered
  after the cable connected, only delays the set-up. Open unanswered four
  times, the set-up starts again from connect, as a receiver that answered
  connect and then lost its power needs. Until the receiver answers, the
  Receiver tab shows it as still being looked for (blue, as the search)
  and says to check the receiver's power adapter and RJ45 cable, and else
  to unplug its power for a moment. The guide says fewer
  buzzers make the lights quicker; the app takes all 64, so any buzzer's ID
  works (the lag is to check on the hardware, `hardware-test.md` 6).
- An answer to connect while the receiver is open says it started again
  (**unverified** that a receiver powered again says so): it is set up
  again and every buzzer's light sent again, as it forgot them; the app
  never asks to connect while open. An answer to open counts only while
  open is asked: a late one, to an open asked again, would otherwise light
  the ready buzzers again and start their timers anew. A press while the
  set-up is unanswered says the receiver is open already: it stayed
  powered while the app started again (a crash, Reconnect, a reloaded
  protocol) and may not answer connect then (**unverified**), so the app
  starts it and sends every light, as after its answer to open. A
  receiver that loses power or its RJ45 plug while the USB side stays in,
  and says nothing, is not noticed: the cable gives no sign of it, and no
  message the app could ask for is known to be safe mid-show. The Receiver
  tab's Reconnect sets it up again as a replug would. A heartbeat (connect
  sent again every few seconds, the receiver lost after two unanswered) is
  for after the hardware test, if it shows that connect while open is
  answered and changes no colour, timer or press taking.
- The receiver takes presses after start `85`, which it echoes. Start, or
  the set-up again, makes every buzzer blink once and changes neither its
  colour nor its timer; what stop `87` does is **unverified**. The app
  starts it once it is open and never
  stops it, so a late team still registers between questions and the
  Receiver tab sees presses at any time; the game ignores the presses that
  do not count, and in fastest finger the colour a press shows tells a
  stale one (below). The mode goes before the lights (2.7).
- A colour is one byte: bit 6 bright, then red, green and blue at 0 to 3
  each (bit 7, which stores a colour without showing it, the app never
  sets). One buzzer's is `2D id%4 id/4 colour`, every buzzer's `74 colour`;
  the receiver answers the second with `74` and the first with nothing,
  and the app waits for neither. The protocol keeps the colour each buzzer
  should show and the one it was told, and sends only changes
  (`Enjoy303s::paint`). When the receiver opens, and on a reset (2.3), it
  first turns every buzzer dark with one order and then lights those the
  app names (`Enjoy303s::repaint`), so no buzzer keeps a colour from
  another program, show or set-up, and a round under way when the
  receiver came back starts its buzzers' timers anew together. Buzzers that turn
  ready together get it all at once, as their timers count from that
  moment (2.6); any other colour goes to all at once when that takes fewer
  orders, but not while a buzzer should show ready, which would then be
  lit again one by one; then each buzzer that should show another colour
  gets its own. A buzzer out of the round may so show ready for a moment,
  which counts for nothing: one judged wrong as the round goes on, say. No
  buzzer turns ready alone while others already count: a player who joins
  mid-round plays from the next question (2.7).
- A press, `32 id code colour t0 t1 t2` and three more bytes the app does
  not use (a second millisecond count, the same on every buzzer, most
  likely the receiver's clock): buzzer `id + 1`; button `code >> 4`, 1 to
  4 for A to D and 5 the big button (the low four bits were `C` on every
  press); the colour the buzzer showed; and its timer, little-endian,
  which counts milliseconds from the moment the buzzer's colour last
  changed (`Press::timer`). An order that repeats the colour a buzzer
  shows leaves its timer counting; 24 bits last 4.6 hours. A buzzer has
  no other key and no power switch, and a held button sends one press.
  Any other button code, a message too short to hold all
  that, and a buzzer outside 1 to 64 give no press (they show in the raw
  log only), so nothing unknown ever buzzes (code 6, a key F the buzzers
  do not have, included). In fastest finger, and while
  its buzz is judged, a press from a buzzer that was not ready is dropped
  too, by the colour it showed: dark, locked, first, right or wrong,
  brightness aside. It was pressed when the game would not take it, and
  arriving just after the step changed, before the receiver thread heard
  of it, it must not count in the next step either: a team hammering its
  locked buzzer while another team's answer is judged must not win as the
  round goes on. Its timer also counts from another moment than the
  others'. The raw log shows it as dropped. The byte was the colour shown
  in every press of the test (`43` in every buzz that counted, `00`
  before a question started, `7F` while a buzz was judged), and the drop
  worked in the app. A colour the app never sets, as another program
  might leave, lets the press through. Elsewhere the game
  alone decides, so any buzzer can register. The answers to setting a
  buzzer's ID (`17`, `24`) are skipped.
- The receiver runs no round itself and locks nobody out: the app lights
  every buzzer (2.7) and decides who buzzed first (2.6).
- No battery level: the 303S's SDK has none, and in the test no byte of a
  press differed between buzzers but the ID and the two counts, and the
  receiver sent nothing unasked in 7.5 idle minutes.
- The quizmaster's remote sends `34 key` with bit 7 set, keys 0 to 11, one
  message per press, none while held or at the release. The protocol reads
  it as a `RemoteKey` (2.10); a code no key has is nothing.

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

Not for the Enjoy cable: it passes nothing until the app opens its serial
line and sets the receiver up, which only the 303S driver does, so learn
mode would never see a press there (2.4).

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
buttons) are timed the same way: the time is read before the press or
command joins the show queue (`show_thread::press`, `show_thread::run`),
so a keyboard press made during a burst of buzzer presses counts as on
time. A press or show command (`execute_at`) stamped before the running
show began, while it started, was meant for no show or the one before, and
does not reach it; neither does an end of the show (`end_show`). Team name
edits skip that check: they are about the names prepared for any show, and
one made while a show starts reaches it.

Resuming (`resume_show`) reads the latest session folder: the one with the
highest number, as a folder is named by its show's start in milliseconds,
or a number past the last folder's when the clock went back, so two shows
never share one. The journal is read as bytes (a byte order mark and CRLF
line ends, which an editor may add, are taken as they come). Each line is
synced before the next is written, so only the last line can be one the app
never accepted: one without its newline, which was being written when the
app stopped, or a whole one that is no journal line at all, which a power
cut leaves when the disk kept the newline but not the bytes before it. A
journal line, as every version of the app writes it and every later one
must keep writing it, is a JSON object with `atMs` and `command`, and
never holds a NUL (`is_journal_line`); so a last line that is that, but
whose command this version does not know, was accepted by a newer app and
is not cut. Resume cuts a torn line off, after keeping the journal as it was in
`torn/journal-<n>.jsonl` (a copy per cut; when the copy fails, as on a full
disk, the log says so and the show resumes all the same). Any other line
that does not read as a command (one from a newer app, or one the disk
damaged before the end) refuses the resume with `journalDamaged` and leaves
the journal as it is, since replaying without it would rebuild another show.
A show that stopped mid-question has its question restarted
(`Game::recovery_command` gives `RestartQuestion`, which takes the
question's score changes back, so a wrong answer before the crash is not
charged twice), and that command is journaled before `show-changed`
reaches the windows, so no window shows the stopped question. One that
stopped while a step was coming by itself (QZ-11) has the step held
(`PauseClock`): the clock reads later after a restart, so the step would
otherwise come at once, before the quizmaster saw where the show stands. The
quizmaster can restart a question in play too (Restart question, after a
confirmation): it is the way to leave a question without scoring it, since
a jump is refused then. Its score changes and answering times are taken
back; score changes made by hand (`AdjustScore`) are not the question's and
stay. A closed question does not restart: the projector shows its results
then, which would give the answer away. So the Director pauses the clock
while the quizmaster confirms, and resumes it if they decline; a question
that closed meanwhile anyway (everyone answered) is not restarted, and the
Director says why.
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

Fastest-finger order is therefore the order the receiver thread passes
presses on in. For presses that carry the buzzer's own timer (the 303S's),
that is the timers' order: each timer counts from the moment its buzzer's
light last changed, and a round's lights turn ready on every buzzer with one
order (2.4), so the timers say who pressed first however the radio delivered
the presses. A timer starts again only when the colour really changes, so
every fastest-finger round starts with a change on every buzzer that can
buzz in it: a question always opens dark (`QuestionStep::Ready`, also when
it is restarted) and turns ready from there, a round that goes on after a
wrong answer turns ready from locked, and when the receiver comes back or
a show begins or resumes every buzzer goes dark before it lights (2.4,
2.7). A guard test in `quiz-core` and a check in every simulated show
(`show-sim`) hold the game to it. A buzzer that missed an order to all
would keep counting from its earlier change and lose a near tie; the
receiver confirms such an order, not each buzzer's taking it, and the test
saw none missed. The receiver reports on a 36 ms tick, lower IDs first,
so presses made at almost the same moment can reach the PC a
little apart and in either order (0 to 36 ms for buzzers 1 and 2 in the
test), so in a fastest-finger round the receiver
thread holds the first timed press for `timer_window` (100 ms, kept until
a test with buzzers far apart in ID measures their gap; a test keeps it
under `PRESS_GRACE_MS`), takes in those that arrive meanwhile, and then
passes them all on, the lowest timer first, equal timers in arrival order
(`timer_order.rs`). They are all stamped with the first one's arrival, the
earliest the app knew of a buzz, and the window is well under
`PRESS_GRACE_MS`, so a buzz on time is still applied before the question
closes. The window ends early when the round does (the buzzer mode changes)
or the receiver goes. That is the hardware's order, as BZ-5 asks; the PC
clock is never used for ranking. A keyboard or on-screen buzzer press
competes with the buzzers by the order the presses are applied in, so one
received a moment before a buzzer press but applied after it loses, and one
made while buzzer presses wait for the window wins (`at_ms` only decides
whether a press is on time). So does a quizmaster's command: a buzz still
waiting for the window when the quizmaster closes or restarts the question
is applied after that and does not count, though it arrived first.
Ranking across sources by `at_ms` would mean holding every buzz open for a
while and re-ordering presses on the PC, which BZ-5 rules out. So a
keyboard or on-screen buzzer standing in for a missing buzzer mid-show
(BZ-12) can win against a buzzer pressed up to `timer_window` before it,
and a quizmaster's Next inside the window comes before a buzz made just
earlier: one more reason to keep the window as short as the hardware
allows.

Which buttons buzz in depends on the question (`Button::buzzes_in`, rules
version 5, Jakub's choice): where every answer of a fastest-finger
question has its button (one to four answers, A to D), those buttons buzz
and the big button does not, so the first press is also the player's
answer; with no answers, or with E and F, which are read out, the big
button buzzes and A to D do not. The buzz keeps its button, and the
Director's status and the projector's buzz bar show the letter pressed;
the quizmaster still judges it. Shows begun under an older rules version
take any button, as they did. The keyboard keys `1`-`0` stand for the big
button, so they do not buzz where A to D do; the on-screen buzzers have
every button. Only the buzzers vote at Jakub's events, and the keys are a
stand-in when one fails (Jakub, 2026-10-21).

While learn mode is active, presses are shown in the Receiver tab but not
sent to the game. So that it can never mute the buzzers in a show, learn
mode cannot start while a show runs (`showRunning`), ends when a show
starts, when the receiver disconnects and when the Receiver tab closes, and
the Director shows a banner while it is on. The show state is taken before
the learn lock both when a show starts and when learning starts, so
neither can slip in between the other's check and its start.

### 2.7 Lights

`Game::apply` returns `Effect::Lights` with the light state of every affected
buzzer, from one function (`Game::light_of`): ready while a player can press,
locked once they answered or another player buzzed, first for the buzz being
judged, dark while a question is closed, and the result from the reveal. A
buzz judged wrong shows wrong while the others get the lights of the step
that follows (ready again, or dark when it closed the question), until the
next step's lights: another buzz, the question's close or its reveal. A
player who joins (switched on, or new to the show by a press or by being
added in the Teams tab) gets the light of the step at once; but one who
joins while a fastest-finger question is under way plays from the next
question and shows locked until then, and the Teams tab says so (rules
version 5, BZ-19): the others' buzzer timers count from when they lit
ready (2.6). Switching on a player who is on changes nothing, so Enable
on every selected team brings back only those switched off. One switched
off shows locked. The lights of one show change go to the driver at once,
a buzzer's last light winning (`Effect::lights`), so the driver is never
told the round's light for a buzzer judged wrong (an order to all may
still show it for a moment on the 303S, 2.4). When a show begins, resumes
or ends, the shell resets the lights (`BuzzerDriver::reset_lights`): every
buzzer dark, then each as the show then stands (`Game::lights`), so no
light is left from another show or another program, also after the app
restarted (the 2026-10-25 test found the lobby still lit blue by an
earlier program). A spare buzzer stays dark then, unless two or more
buzzers turn ready with that reset (a resume in the lobby or mid-round):
the order to all that lights them reaches it too, as in any round. Otherwise the shell passes the lights to `BuzzerDriver::set_lights`; for the hardware
driver that is a message to the receiver thread, which encodes it with the
active protocol and writes the reports. Light failures are logged and never
block the show. With the learned or raw protocol, `has_lights()` is false and
the Director says "this receiver does not light the buzzers yet".

The 303S shows them as colours: ready blue, locked dim white (Jakub's
choice, told apart from the red of a wrong answer next to it), first
white, right green, wrong red, off dark (`enjoy303s.rs`); no two share a
colour byte. Locked and first are both white, told apart by brightness
alone, told apart at arm's length in the 2026-10-25 test, and Jakub kept
dim white for locked after it (both read as white on the hardware;
an early note of them as purple was a misreading, 2026-10-26). A light that goes to all buzzers at once reaches spare
buzzers too, which the app never lit and so show whatever went to all
last: in a fastest-finger round they light ready, so a spare's press
registers a late team, which plays from the next question as any player
who joins then.

The driver also gets what the buzzers take as a whole. `Game::buzzer_mode`
derives it from the same phase as the lights: `BuzzerMode` is off, register
(the lobby), answers (everyone answers), fastest finger, buzzed (a buzz is
judged) or results. The shell sends it with `BuzzerDriver::set_mode` before
the lights of every show change and when a show begins or ends; with no
show the buzzers register, so the Receiver tab sees their presses. Before
the lights, so that by the time the buzzers light ready the receiver
thread holds a round's presses for the buzzers' timers (2.6), and the 303S
tells a press made in the step before by the light it showed (2.4). As the 303S runs no round
itself, a press the app ignores (from a player switched off or out for the
question, or just after time up) needs nothing sent back, and a keyboard or
on-screen buzzer stands in for a buzzer (BZ-12) in fastest finger as well:
the buzzers' lights are the app's either way.

### 2.8 Diagnostics

- The Receiver tab lists each buzzer heard since the receiver connected,
  with its last press, counted or not, and when, so a press of the big
  button on each buzzer shows which work and are in range (BZ-16). The
  list starts again whenever the receiver goes.
- The app keeps the last 2000 raw reports in memory; the Receiver tab shows the
  latest ones and can export all of them as text
  (`unix-ms <tab> hex bytes <tab> decoded presses <tab> dropped presses
  <tab> remote keys`, such as `3A@500` for buzzer 3, button A, with its
  timer at 500; dropped ones came from a buzzer that was not ready, 2.4).
- `cargo run -p buzzers --example hid-monitor` lists HID devices and, with a
  `VID:PID` argument, runs the app's receiver thread and protocol on that
  receiver without the app, printing the status, every report with its
  timestamp, the presses decoded, with the buzzer's timer, those
  dropped as not ready and the remote's keys. The buzzers
  show ready and take answers A to D; with a further `rush` argument they
  are in a fastest-finger round, whose presses come in the timers' order.
  With `lights` every buzzer shows each light of a show in turn, two
  seconds each, and a second into each, buzzers 2 and 6 alone take the
  next: an order to all, then one each. Every press is taken then.

### 2.9 Testing

`crates/buzzers/tests/receiver.rs` runs the real receiver thread against a
fake HID backend: connect and init, decode order, held-button repeats,
unknown reports, light and mode commands, the lights and mode replayed on
connect and after replugging, follow-up writes, the 303S's lights kept
until it answers its set-up, which is asked again from the cable's
settings while unanswered, the write gap, a fastest-finger buzz in the
timers' order, one passed on when the round ends, the receiver is lost or
deselected, a buzz from a buzzer that was not ready dropped and logged
while the next press after it counts, the timer window under the grace
after time up, the remote's keys after the presses of their report and
once, unplug and replug, open failure and retry, deselect. Unit
tests cover the edge filter, signatures, the recorder, conflict
detection, the CP2110 reports, the 303S's set-up, its steps asked again
and a receiver started again, orders, colours, when one goes to all, that
no two differ in brightness alone, its presses with timer and shown
colour, in fastest finger only from buzzers that were ready, button
codes, the remote's key codes, short messages, buzzer ID range, messages
split, spoiled and after noise, and the timer order; `quiz-core`'s `game_flow.rs` follows the
buzzer mode through a show, which buttons buzz in on which question, a
player switched on mid-question and the merged lights of a wrong
judgement. None of this proves the real
receiver's format: see [`hardware-test.md`](hardware-test.md).

### 2.10 The quizmaster's remote

The Galaxy set's 12-key remote talks to the same receiver as the buzzers
(specification BZ-21). Its keys take the presses' way to the app, but
never go into the game themselves:

- The 303S protocol reads each key as a `RemoteKey`, named after what is
  printed on it (2.4). The receiver thread sends a key after the presses
  of its report, and only once: the same key again within 700 ms is taken
  for the receiver sending it again, as it did with presses (BZ-6), and
  is only logged. A key pressed twice that fast counts once.
- What each key does (`RemoteAction` in `app/src-tauri/src/remote.rs`) is
  kept per computer with the receiver's settings in `receiver.json`: only
  the keys changed from their default. The Receiver tab lists the 12 keys
  with a choice each, marks the key last pressed, so the quizmaster can
  tell which key is which, and puts the defaults back.
- The Director does a key's action exactly as the keyboard key for that
  action (`commandForAction` in `keyboard.ts`): only when the step takes
  it, one step per press through the window's step guard, and journaled
  like any command. Previous slide goes where `Game::previous` says
  (`GameView.previousSlide`, `GoTo`): the slide before, or from the final
  ranking the last slide, and only between questions. A question that
  ended earlier in the show, closed or won by a fastest finger judged
  right (`Game::end_question`), comes back with its answer shown
  (`Game::closed_questions`, rules version 6): it takes no presses, shows
  no results (they are not kept), and its scores stand, so Back then
  Forward, as on a presenter's clicker, never asks it and scores it twice.
  The Director's Go to does the same. Projector opens or closes the projector window,
  as M does. The projector window does not listen, so no key acts twice.

| key | default action |
| --- | --- |
| back arrow | previous slide |
| forward arrow, middle arrow | next |
| target | end question |
| play / pause | pause the clock, or go on |
| trophy | leaderboard, shown or hidden |
| colour bars | play the video |
| red cross, green tick | wrong, right |
| power, grid, question mark | nothing |

Power does nothing by default, so a key pressed by mistake never closes
the projector mid-show. The three arrows' codes were named over chat in
the first hardware test; the next one checks them (`hardware-test.md`
section 3).

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
2. `installers` (matrix, after `check` and `windows-tests` (not `e2e`, so a flaky E2E run cannot hold back an installer), on tags `v*` and manual
   runs only, because runner minutes are paid and macOS minutes cost the most): Windows NSIS, macOS universal DMG
   (`aarch64` and `x86_64` targets), Linux deb and AppImage on Ubuntu 22.04
   so the AppImage runs on older distributions. Each uploads its files as a
   run artifact (`buzzdome-windows`, `buzzdome-macos`, `buzzdome-linux`).
3. `e2e` (`.github/workflows/e2e.yml`: every push on Ubuntu 22.04 with
   `WebKitWebDriver` under `xvfb-run`; on Windows with `msedgedriver` and
   WebView2 only on main, tags and "Run workflow", as Windows minutes cost more): the real app built with `--debug --no-bundle` and driven
   through `tauri-driver` (`app/e2e`): a whole demo show
   with the on-screen and keyboard buzzers and the projector checked at
   each step, crashes (the app killed) and resumes, every button on every
   Director tab and editor ribbon tab, the command panel step by step to
   the end, a quiz saved and opened through the system's file picker
   (`xdotool`, Linux only) and started again from the recent quizzes, a
   theme picked in the editor, the stage lights driving an Art-Net node
   and sending OSC on this machine (the tests listen on its UDP ports), a
   fixture from the library, and the AI tools: the app as an MCP server
   and the `buzzdome-quiz` command line writing quiz files the app then
   plays.
   Buzzers with no hardware: a debug build
   given `BUZZDOME_FAKE_RECEIVER=<host:port>` lists a fake 303S receiver
   in place of USB-HID (`buzzers::fake_receiver`), which the test plugs
   in, pulls out and presses over TCP, and which reports the lights the
   app gives the buzzers; the receiver thread and the 303S protocol above
   it are the real ones. Release builds never read the variable. With it
   the tests check automatic detection, the buzzers heard, the lights,
   fastest finger in timer order and a receiver pulled out mid-show
   (`receiver.e2e.ts`). On Windows the app ignores environment
   overrides of its data folders, so the tests use the real ones and wipe
   them; the harness refuses to run there except on a GitHub-hosted runner.
   The runner is elevated and an elevated WebView2 ignores the debugging
   port msedgedriver asks for, so the Windows tests run unelevated through
   `gsudo --integrity Medium`. macOS has no WebDriver
   for WKWebView. When a test fails, each window's screenshot and text go up
   as the `e2e-output` artifact.

A newer push to a branch cancels that branch's running checks; main checks
every merge.

`.github/workflows/nightly.yml` runs every night that main has a new commit
(and always on "Run workflow"):
30 000 random shows with up to 50 buzzers (`crates/show-sim`), 10 000 crash
drills through the real journal (`app/src-tauri/src/crash_drill.rs`), both in
release and from seeds no earlier night used, and the E2E suite five times on Linux and once on Windows.
A failing random show prints `SOAK_SEED=<n>`; that variable replays it alone.

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
| Results: what each question gave whom, joker, penalty for no answer, a question asked again, rounds, hand adjustments, reset; Results tab, CSV export and the print style | `cargo test` (`game_flow`, `quiz-file` sheet; show-sim checks that marks and adjustments add up to every score after every command), `npm test` (`results`), E2E (Results tab, CSV export through the picker), the print style in headless Chromium against the preview | pass; the print dialog itself on Windows and macOS **not tested** |
| Loudness measurement | `npm test`: a full-scale 997 Hz sine in one channel measures -3.01 LUFS as BS.1770-4 states; gating, channel sum, part, peak limit | pass |
| Trim and Normalize volume windows, video trim window, background video menu and options, slide video (intro pause, Play video, speed, volume, part, holding with the show, blur and mosaic, replay) | headless Chromium against the preview with generated test media (a song and a test-pattern video, not in the repo) | pass |
| Game rules under random shows: refused commands change nothing, controls match what the game accepts, scores add up, results only after close, places as the game ranks them (shared on a tie, 1, 1, 3), replay from the journal gives the same game; 1 to 50 buzzers, random quizzes of every slide kind | `crates/show-sim` (`cargo test -p show-sim`: 40 shows on every push; 30 000 and a 3000-step evening nightly); 30 000 shows locally | pass |
| Crash and resume through the real journal, incl. a torn, zeroed or garbage last line, with the random shows' rule checks after every step | `crash_drill.rs` (40 per push, 10 000 nightly) | pass |
| The real app end to end: show, crash and resume, every Director and editor button, file picker | `app/e2e` on Linux (WebKitGTK) and Windows (WebView2, no file picker), Linux on every push and five times nightly, Windows on main and once nightly | Linux pass; Windows see the PR's checks |
| Media through the asset protocol, autoplay of video and sound | needs the installed app on each OS | **not tested** |
| Windows and macOS installers | CI | see the PR's checks |
| Real receiver: detection, raw reports, presses in a show | Enjoy 303S on Windows 11, buzzers 1 and 2, `hardware-test.md` sections 2 to 6 in part (2026-10-25) | pass; macOS, high IDs, two at once in the app, failures (section 7) **not tested** |
| Enjoy 303S protocol: set-up, presses, timers, colours | `cargo test -p buzzers` against fakes; the receiver with `hid-monitor` and the app (2026-10-25) | pass; lights read from the back of a room **not tested** |
| Battery level | not in the 303S's SDK, and no press or other message carried one (2026-10-25) | **not buildable** for the 303S |
| Stage lights: stages from the game, setup checks, looks and effects, desk priority and fades, Art-Net, sACN, Enttec and SoundSwitch frames byte for byte, OSC and MIDI encoding, lights thread with fake outputs; Lights tab | `cargo test` (`quiz_core::stage`, `game_flow`, `lights`, shell `stage_lights`), `npm test` (`lights`), the tab in headless Chromium against the preview | pass |
| Stage lights on real interfaces and fixtures, OSC and MIDI into a real program | needs the hardware | **not tested** |

## 7. Stage lights and show messages

Spec section 12. The lights are a view of the show, like the projector: they
follow its state and are never journaled, so they cannot hold up a show or
change what a replay does.

### 7.1 Layers

- `quiz_core::stage` maps a game at a moment to a `Stage` (spec LT-4). Time
  matters only for the last seconds of the clock (`LAST_SECONDS_MS`).
- `crates/lights` is an I/O adapter that knows quiz-core's `Stage` and `Cue`
  but not the game: `setup` (the venue's setup and its checks), `look`
  (draws a look into a universe at a moment), `desk` (blackout > hold >
  flash > stage look, and fades as a blend of the old and new universe),
  `dmx` (frame formats as pure functions, tested byte for byte, and the
  outputs that move them), `signals` (OSC and MIDI) and `engine` (the
  lights thread). Serial and USB outputs sit behind the `devices` feature,
  so the crate's tests need no device libraries.
- `app/src-tauri/src/stage_lights.rs` keeps the setup in `lights.json`
  (`AppFile`, checked with `LightsSetup::check` before it is kept or used;
  `lights_store` writes off the windows' thread under a lock of its own
  held until the lights have the setup, so the last kept is the one used)
  and tells the lights thread what the show did: after every show change and
  every tick of the show thread it queues the cues of the change's effects,
  then the stage, slide and buzz, only when they changed. Sending is a
  channel push; the show thread never waits.
- The Director's Lights tab (`app/src/director/LightsTab.tsx`, changes as
  pure functions in `lights.ts`) keeps a change 400 ms after the last one
  and polls the universe for its preview four times a second.

### 7.2 Lights thread

Sends a frame every 25 ms (the output's `frame_every`), as fixtures expect
a steady signal. An output that fails is dropped and opened again a second
later; the status (`off`, `connected`, `lost` with why) goes to the Director
as the `lights-status` event. A frame's status and universe are shared
before it is sent, so the preview never lags what fixtures show. Messages
to OSC and MIDI go out on the same thread as they arrive; each target opens
and retries on its own. Nothing on the thread looks up a name: network
outputs and OSC take IPv4 addresses only (`SetupError::NotIpv4`), as a
lookup can block for seconds (glibc's `RES_TIMEOUT` 5 s, `RES_DFLRETRY`
2 tries, `<resolv.h>`), and an Open DMX signal, which the computer times,
would stop meanwhile.

Effects step at least every 500 ms (`MIN_EFFECT_MS`), at most 2 flashes a
second; sources in spec LT-3.

### 7.3 Fixture library

`crates/lights/data/fixture-library.json` is our own format (`library::Library`),
generated from a checkout of the Open Fixture Library
(<https://github.com/OpenLightingProject/open-fixture-library>, MIT, licence
in `data/OPEN-FIXTURE-LIBRARY-LICENSE`) by
`cargo run -p lights --example ofl-import -- <checkout>`. The OFL documents
its JSON format as one that "may introduce breaking, not-backwards-compatible
changes at any time" and asks programs to transform it into a stable format
(`docs/fixture-format.md` there), so the app only reads ours; refreshing
the library is rerunning the import, which writes one model per line so
its diff shows which models changed. `library::ofl` maps each definition
(rules in its module docs, unit-tested, after the OFL's
`docs/capability-types.md`):

- Fixtures in the OFL "Laser" category are left out (spec LT-9), with
  combined effects that have a laser.
- `Intensity` over the whole range is a dimmer, `ColorIntensity` red,
  green, blue or white (warm and cold too) that colour.
- A `ShutterStrobe` channel with an `Open` range and a dark value (a
  `Closed` range, or an `Intensity` range starting dark) is a `Shutter`,
  open while the fixture is lit, closed otherwise.
- A dimmer that does something else at its top (strobe, a program) is a
  `DimmerRange` from 0 to its brightest value below that, so fades, the
  master and blackout still work on it; one that gets darker as its value
  rises is a `DimmerRange` from 255 to 0.
- A lamp switch (`Intensity` ranges that are only "off" or "bright") is a
  `Shutter` too.
- Fine channels (also a template channel's, "Dimmer 1 fine") rest at 0.
- Anything else rests at its default value, unless by the data that
  value strobes (a shutter effect at a speed that is not stopped), runs a
  program (an `Effect` that is a preset, follows the sound or by its name
  runs on its own) or lights the fixture; then at the first value of the
  channel that does none of these. A held value other than 0 counts as
  dark only where the definition says so, since it is sent in a blackout
  too. A channel with no such value stays at its default and is listed in
  the mode's `check`, which the tab shows when the mode is picked.

Matrix channels are expanded per pixel or group, with the OFL's own keys
(`pixelKeys`, or the default keys of `pixelCount`; groups in file order),
so labels match the manual. A test builds a fixture from every mode and
runs the setup's checks on it; the guardian's
`ofl-library-audit.py` compares every resting value with the OFL data.
The library is read on first use (`OnceLock`) and searched off the
windows' thread (`lights_library`); its source and full MIT licence
(`LibrarySource.licence`, copied from the checkout by the import) are
shown in the tab before any search (`lights_library_info`). When a picked
mode has no channel a look can light (`Channel::follows_looks`, written to
`LightsConstants.ts` as `LIT_BY_LOOKS`), the tab says so.

The tab can bring the library up to date (`app/src-tauri/src/fixture_library.rs`,
`lights_library_update`): off the windows' thread it downloads
<https://open-fixture-library.org/download.ofl> (the site's "ofl" export
of every fixture as `<manufacturer>/<model>.json` plus
`manufacturers.json`, per `ui/api/download.js` and `plugins/ofl/export.js`
in the OFL repository) over HTTPS only, redirects included (at most 3),
and takes it only when it ends at that host, with a 64 MB cap and a
3-minute timeout, and converts it with `library::archive::from_zip`
through the same `Import` the built-in library is made with. Each
definition is capped at 4 MB unpacked and the zip at 256 MB. The export
links every file to the format version it was made with
(`$schema` ".../schema-12.6.0/..."), so `manufacturers.json` or a
definition that names a major version other than 12, or none, refuses the
whole download (`ArchiveError::FormatChanged`, "none" for no version); a
definition that does not read is skipped and counted; serde_json's nesting limit and the converter's caps
(16-bit shifts checked, at most 512 pixels, modes of at most 512 channels)
keep any input from panicking or running away. Fewer than 300 models are
refused as a cut-short download. The result is written atomically to
`fixture-library.json` in the app's data and used from then on, unless the
built-in library is newer (by `LibrarySource.date_ms`: the OFL commit's
time, or the download's) or the file does not read. A download keeps the
built-in library's source name, copyright and licence, with the origin
`Downloaded` instead of `BuiltIn { commit }`. `lights_library_reset` removes it. The show never touches any of
this.

### 7.4 Outputs

| Output | Transport | Frame |
| --- | --- | --- |
| Enttec DMX USB Pro | USB serial | 0x7E, label 6, length (513, low byte first), start code 0, 512 channels, 0xE7 (Enttec DMX USB Pro API) |
| Enttec Open DMX USB | FTDI serial at 250 kbit/s, 8N2 | break (about 150 µs), mark after break, start code 0, 512 channels; the computer times it, so jitter on a busy computer is possible |
| Art-Net | UDP 6454 | ArtDmx, protocol 14, 15-bit port address, sequence 1 to 255 (Art-Net 4) |
| sACN | UDP 5568, multicast 239.255.hi.lo or one host | E1.31 data packet, 638 bytes, priority 100, a CID from the computer's name |
| SoundSwitch Micro | USB bulk endpoint 0x01 (VID 0x15E4, PID 0x0053) | see 7.5 |

### 7.5 SoundSwitch Micro DMX Interface

SoundSwitch publishes no protocol. Ours is written, in our own words and
code, from the description in the independent Apache-2.0 QLC+ plug-in
qlcplus-soundswitch-integration
(<https://github.com/jloops412/qlcplus-soundswitch-integration>), and is
unverified on the hardware, so the Director calls it experimental:

1. Two 12-byte start packets, both starting with the tag `sTRt`
   (`crates/lights/src/dmx/soundswitch.rs`), then a 200 ms wait.
2. 50 frames of the setup's dark look. The output only says how many
   (`DmxOutput::warm_up_frames`); the lights thread sends `look::dark`, so
   a reversed dimmer, where 0 is full, stays dark.
3. A 522-byte frame every 25 ms: `sTRt`, 01 00 02 02, the port (0), a
   zero, then the 512 channels.
4. On close, three frames of the setup's dark look, which the lights
   thread sends before it lets the output go (`dark_frames_at_release`).
   The plug-in warms up with frames of zeros.

On Windows, libusb opens only a device bound to a driver it supports, such
as WinUSB (libusb wiki, Windows, <https://github.com/libusb/libusb/wiki/Windows>);
whether SoundSwitch's own driver allows that, or WinUSB has to be bound
(for example with Zadig), is to check with the hardware. Only one program
can hold the interface, so the SoundSwitch program must be closed.

### 7.6 Show messages

OSC 1.0 over UDP. Every event has an address of its own, so a program can
map each to a button; triggers carry the float 1.0.

| Event | OSC | MIDI (channel as set) |
| --- | --- | --- |
| Show started / ended | `/buzzdome/show/start`, `/buzzdome/show/end` | notes 70 / 71 |
| Stage | `/buzzdome/stage/<stage>` (`noShow`, `lobby`, `billboard`, `explanation`, `endOfRound`, `questionReady`, `answering`, `lastSeconds`, `buzzed`, `closed`, `revealed`, `leaderboard`, `finalBuildUp`, `winner`) | notes 36 to 49 in that order |
| Cue | `/buzzdome/cue/<cue>` (`clockStart`, `buzz`, `correct`, `wrong`, `timeUp`, `reveal`, `leaderboard`, `place`, `winner`) | notes 60 to 68 in that order |
| Slide | `/buzzdome/slide` with the slide number (int) | program change number − 1 (slides 1 to 128) |
| Buzz | `/buzzdome/buzz` with the buzzer number (int) | note 72 |

A note is a note on at velocity 127 followed at once by its note off. Each
message has its MIDI 1.0 length: 3 bytes for a note, 2 for a program change.
