# Specification

What the app must do, written as testable requirements. How it is built is in
[`design.md`](design.md); how to check the buzzer link on real hardware is in
[`hardware-test.md`](hardware-test.md). Last updated 2026-10-09.

Requirement IDs (`BZ-3`, `BG-2`, ...) are stable: refer to them in issues,
commits and tests. Each requirement says whether it is **done**, **partial**
or **planned**.

## 1. Goal and scope

A desktop quiz and game-show app for live events: questions on a projector,
players answering with wireless buzzers, the quizmaster in control from a
laptop. It should behave almost identically to the reference app
(QuizXpress 8.5, see [`replica/recon.md`](../replica/recon.md)) for the
formats we use, so that a quizmaster who knows the reference app can run a
show without training.

"Almost identical" means the same screens, flows, scoring rules and buzzer
behaviour. It does not mean the same code, wording, artwork, sounds, skins or
example quizzes: those are rebuilt from the documented behaviour only
(clean-room rule in `CLAUDE.md`). No trademarked product or format names
appear in the product.

Later, users can add their own **plugins** (section 6) and their own
**backgrounds and themes** (section 5) without rebuilding the app. Their file
formats are specified here so that a colleague can start preparing them.

Out of scope: phones as keypads, cloud services, the reference app's content
subscriptions (see `replica/recon.md`, "Out of scope").

## 2. Product shape

- **Director**: the quizmaster's window on the laptop. Runs the show, edits
  scores and teams, shows buzzer status, hosts receiver setup and (later) the
  quiz editor.
- **Host screen**: a full-screen window on the projector. A view of the game
  state, never its source of truth.
- Both windows run in one app on one computer. No network is needed during a
  show.

Feature-level parity is tracked row by row in
[`replica/features.csv`](../replica/features.csv) (`clone` column).

## 3. Platforms and installation

| ID | Requirement | Status |
| --- | --- | --- |
| PL-1 | Installers for Windows 10/11 (x64, NSIS `.exe`), macOS (one universal `.dmg` for Apple silicon and Intel) and Linux (x64 `.deb` and `.AppImage`). | done: CI builds them on `main`, on `v*` tags and on demand |
| PL-2 | No driver installation for the buzzer receiver on any OS. | done by design, to verify on hardware |
| PL-3 | Linux: the app shows the exact udev rule that lets a normal user open the receiver, with a copy button. | done |
| PL-4 | macOS: if the OS blocks the receiver, the error names the setting to change (Privacy & Security > Input Monitoring). | done |
| PL-5 | Builds are not code-signed yet. The install notes explain the one-time OS prompt (Windows SmartScreen "More info > Run anyway"; macOS "Open Anyway" in Privacy & Security). The macOS build is ad-hoc signed so Apple silicon Macs will run it. | done |
| PL-6 | Signed and notarized builds (Windows code-signing certificate, Apple Developer ID). | planned, needs accounts |

## 4. Buzzer link

The most important part of the app: a show is only as good as its buzzers.

### 4.1 Hardware

Wireless buzzers (A, B, C, D and one big fastest-finger button, with a light)
talk by radio to a receiver; the receiver connects to the computer through a
"USB-HID cable" (USB on one end, RJ45 on the other). Details and sources are
in [`replica/hardware.md`](../replica/hardware.md).

The manufacturer (Enjoy Tech) answered Jakub's email in October 2026:

- the model is either 215 (dome) or 303S (RGB); the label under the buzzer's
  base says which;
- the receiver reports the press **time**;
- dome and RGB versions use **different protocols**;
- the protocol is described in their SDK, which they provide free of charge.

Until the SDK arrives the exact report format is unknown. The app is
therefore built to work in three stages, each a fallback for the next:

1. **Learned mapping** (works now, no vendor information needed): the user
   teaches the app which USB report each button of each buzzer sends.
   Presses work; lights do not. Teaching is per buzzer, which is fine for a
   test with a few buzzers but too slow for a full event.
2. **Vendor protocol** (when the SDK arrives): presses with buzzer ID and
   press time, lights, battery level. Selected automatically by the
   receiver's USB vendor and product ID.
3. **Keyboard**: keys `1`-`0` and the on-screen virtual buzzers, always
   available, also covering the vendor's "keystroke" mode.

### 4.2 Requirements

| ID | Requirement | Status |
| --- | --- | --- |
| BZ-1 | The Director lists all USB HID devices (vendor and product ID, name, manufacturer, usage page, interface) and lets the user pick the receiver. Keyboards and mice are greyed out. | done |
| BZ-2 | The chosen receiver is remembered and reopened automatically at the next start. | done |
| BZ-3 | When the receiver is unplugged or fails, the Director shows it at once and the app reconnects by itself within 2 seconds of the receiver coming back, without losing the show. | done (simulated) |
| BZ-4 | Every press becomes a game command (buzzer ID, button) that is written to the show journal **before** it changes anything on screen, exactly like a keyboard press. After a crash the show resumes with those presses. | done |
| BZ-5 | Fastest-finger order is the order the receiver reports, or the receiver's press times when the protocol provides them. The PC never re-orders presses. Presses in one USB report keep their order within the report. | done for receiver order; press times wait for the protocol |
| BZ-6 | A button held down, or a receiver that repeats a report, counts as one press. A new press needs a release or a 150 ms pause. | done |
| BZ-7 | Learn mode: for a chosen buzzer number and button, the user presses the button at least twice; the app derives a signature (which bytes matter) and stores it per receiver model. Conflicting signatures are refused with a message. While learning, presses do not reach the game. | done |
| BZ-8 | Diagnostics: the Director shows live presses and the raw reports (hex) with what they decoded to, and exports the raw log to a text file that can be sent to a developer. | done |
| BZ-9 | A command-line monitor (`hid-monitor`) lists devices and prints raw reports with timestamps, without the app. | done |
| BZ-10 | Lights follow the game: ready, locked out, first to buzz, correct, wrong (table 4.3). Dome buzzers use their fixed colours; RGB buzzers use the colours in table 4.3. | partial: the app sends light states; the encoding waits for the protocol |
| BZ-11 | Up to 100 dome buzzers or 64 RGB buzzers on one receiver (the receiver's own limits, see `replica/hardware.md`). Up to about 50 are used at our events. | to verify on hardware |
| BZ-12 | Keyboard and on-screen virtual buzzers work at the same time as the receiver, so a missing buzzer can be replaced mid-show. | done |
| BZ-13 | If another program (for example the vendor's software) holds the receiver, the error says so. | done |
| BZ-14 | Battery level of each buzzer, shown in the buzzer test view. | planned, needs the protocol |
| BZ-15 | Host screen shows a small "buzzers not connected" sign on the welcome screen while the chosen receiver is not connected (the reference app does this too). | planned |
| BZ-16 | Buzzer test view: a grid of buzzer numbers that light up with the pressed button, for range tests in the venue. | partial (Receiver tab shows presses as a list) |
| BZ-17 | Buzzer panel in the Director during a show: every known buzzer number with its state (registered, last button and when, light, battery when the protocol gives it), and the receiver state on top. A buzzer not seen for a long time is marked. | planned |
| BZ-18 | If the receiver reports which buzzers are switched on and in range without a press (open question, section 11), the registration screen and the buzzer panel show them as "on" before anyone presses. | waits for the protocol |

### 4.3 Light states

`Light` values come from the game core (`crates/quiz-core`). The RGB colours
are this app's defaults and will become part of the theme (section 5).

| Game situation | `Light` | Dome buzzer | RGB buzzer |
| --- | --- | --- | --- |
| no question running | `Off` | off | off |
| buzzer may press | `Ready` | green | green |
| out for this question (pressed wrong, or another team is answering) | `Locked` | red | red |
| pressed first | `First` | white, flashing | blue, flashing |
| answer judged correct | `Correct` | green, flashing | green, flashing |
| answer judged wrong | `Wrong` | red, flashing | red, flashing |

### 4.4 Acceptance test

The buzzer link is accepted when the checklist in
[`hardware-test.md`](hardware-test.md) passes on the event laptop (Windows)
and on one Mac, with at least four buzzers.

## 5. Backgrounds and themes

Users bring their own wallpapers and looks. This mirrors what the reference
app offers (background picture or looping video per slide, a gallery of
backgrounds, skins that inherit from a built-in skin and override only what
changes) with our own file format.

Built so far: backgrounds per slide, and five built-in themes (Classic,
Studio, Night, Showtime, Neon) chosen per quiz on the editor's Design tab,
with a live preview while the pointer is on one. Theme packages, the
background gallery and round or quiz backgrounds are planned.

### 5.1 Background files

| ID | Requirement |
| --- | --- |
| BG-1 | Image formats: PNG, JPEG, WebP. Video formats: MP4 (H.264) and WebM. A video's sound plays only with Silent off (BG-4). |
| BG-2 | Aspect ratio 16:9. Recommended size 1920 x 1080 px; minimum 1280 x 720; maximum 3840 x 2160. Images up to 20 MB, videos up to 200 MB. |
| BG-3 | A background that is not 16:9 is scaled to cover the screen and cropped around its centre; the import dialog shows the crop and warns. Smaller than the minimum or larger than the limits is refused. |
| BG-4 | A background video starts as soon as its slide appears and has its own options: Loop (on), Silent (on) and Sync with quiz (off), which pauses it while a player answers, while the clock is paused and once the question is over. The editor shows its first frame with a video mark, and the slide's context menu can play or stop it there, or remove the background. Done. |
| BG-5 | Backgrounds sit behind all content. The app draws its own panels behind text, so any background stays readable; designers should still keep the middle band calm (layout guide in 5.4). |
| BG-6 | Choice per slide, per round, per quiz and per theme. The most specific wins: slide > round > quiz > theme > built-in. |
| BG-7 | A quiz file embeds every background it uses (in `media/`), so a quiz moved to another computer looks the same. Nothing is linked to an outside path during a show. |
| BG-8 | The Director has a background gallery: import files or a theme package, preview on a 16:9 frame with the layout guide, delete, and filter. |
| BG-9 | A broken or missing background never stops the show: the next level in BG-6 is used and the Director shows a warning. |

### 5.2 Theme package

A theme bundles backgrounds, colours, fonts and (later) sounds. It is a ZIP
file with the extension `.qstheme` and this layout:

```text
autumn.qstheme
├── theme.json
├── backgrounds/   images and videos
├── fonts/         .woff2 or .ttf, optional
└── preview.png    optional, 640 x 360, shown in the gallery
```

`theme.json`:

```json
{
  "format": 1,
  "id": "cz.example.autumn",
  "version": "1.0.0",
  "name": { "cs": "Podzim", "en": "Autumn" },
  "author": "Name <email>",
  "parent": "builtin.classic",
  "backgrounds": {
    "default": "backgrounds/main.jpg",
    "welcome": "backgrounds/welcome.mp4",
    "question": "backgrounds/question.jpg",
    "leaderboard": "backgrounds/scores.jpg",
    "roundEnd": "backgrounds/scores.jpg",
    "final": "backgrounds/final.jpg"
  },
  "colors": {
    "text": "#ffffff",
    "panel": "#101830cc",
    "accent": "#f2a900",
    "answerA": "#2e7dd7",
    "answerB": "#e0a100",
    "answerC": "#2fa84f",
    "answerD": "#d64545",
    "correct": "#2fa84f",
    "wrong": "#d64545"
  },
  "fonts": {
    "heading": "fonts/Heading.woff2",
    "body": "fonts/Body.woff2"
  },
  "buzzerColors": {
    "ready": "#00ff00",
    "first": "#0000ff"
  }
}
```

| ID | Requirement |
| --- | --- |
| TH-1 | `format` is the theme format version; the app refuses a format it does not know, with a message. `id` is unique (reverse domain style); importing a theme with an existing `id` and a higher `version` updates it. |
| TH-2 | Inheritance: every key that is missing, or set to `"default"`, comes from the `parent` theme, recursively, ending at a built-in theme. A theme only lists what it changes. Backgrounds are the exception: a screen the theme leaves out takes the theme's own background of the screen it falls back to (TH-5) before any parent's, so a theme that sets only `default` is not mixed with its parent's pictures. |
| TH-3 | Paths are relative to the package root and must stay inside it. Every file is checked on import (format, size, BG-2). |
| TH-4 | Colours are `#rrggbb` or `#rrggbbaa`. Fonts must be licensed for embedding; the import dialog reminds the user. |
| TH-5 | Screen keys: `default` (every screen without its own), `welcome` (team registration before the first slide), `question` (a question being asked), `answer` (a question once its answer is shown, and explanation slides; falls back to `question`), `leaderboard` (the leaderboard shown over a slide), `roundEnd` (end-of-round slides; falls back to `leaderboard`), `final` (final results). Billboards use `default`. Unknown keys are ignored with a warning, so newer themes still load in older apps. |
| TH-6 | Installed themes live in the app data folder under `themes/<id>/`; the quiz file stores the theme `id` and embeds the theme's files it uses (BG-7). |

### 5.3 What a designer gets

The app will ship a template package (`theme-template.qstheme`) with a
`theme.json` that lists every key, and a 1920 x 1080 PNG layout guide that
marks where the question, answers, clock and team panels sit on each screen.

### 5.4 Layout guide (1920 x 1080)

| Area | Position (x, y, width, height) | Notes |
| --- | --- | --- |
| question band | 80, 60, 1760, 300 | dark panel drawn by the app |
| media area | 560, 380, 800, 380 | picture or video of the question |
| answers A-D | 80, 780, 1760, 240 | four answer panels |
| clock and counters | 1700, 60, 160, 160 | top right |

These numbers are the target for the host screen; they are fixed when the
host screen layout is final and then published with the template.

## 6. Plugins (planned)

A plugin adds a mini game, a show element or an integration (for example a
prize wheel, an auction, a lighting desk trigger). It mirrors the reference
app's extensions (configure, launch, end, extra Director buttons, "next") and
its slide scripts (handlers for slide, clock, answer and score events).

### 6.1 Package

A ZIP file with the extension `.qsplugin`:

```text
prize-wheel.qsplugin
├── plugin.json
├── screen.html      shown full screen on the host screen (kind "screen")
├── panel.html       optional control panel inside the Director
├── *.js, *.css, assets/
└── icon.png         128 x 128
```

`plugin.json`:

```json
{
  "format": 1,
  "apiVersion": 1,
  "id": "cz.example.prize-wheel",
  "version": "1.0.0",
  "name": { "cs": "Kolo s cenami", "en": "Prize wheel" },
  "author": "Name <email>",
  "kind": "screen",
  "entry": "screen.html",
  "panel": "panel.html",
  "permissions": ["teams", "scores", "buzzers", "lights"],
  "settings": [
    { "key": "segments", "type": "number", "label": { "cs": "Počet polí", "en": "Segments" }, "default": 8, "min": 2, "max": 24 },
    { "key": "title", "type": "text", "label": { "cs": "Nadpis", "en": "Title" }, "default": "" }
  ]
}
```

### 6.2 Requirements

| ID | Requirement |
| --- | --- |
| PG-1 | Two kinds: `screen` takes over the host screen while it runs (a mini game); `service` has no screen and reacts to show events (an integration, a slide script). |
| PG-2 | The Director has an Extensions view: install from a `.qsplugin` file, list, configure, launch, end, remove. A plugin can have several named configurations (create, clone, rename, delete); the selected one is used at launch. |
| PG-3 | A plugin can be launched from the Extensions view when no clock is running, or placed in the quiz as a slide that launches it with a given configuration. |
| PG-4 | While a `screen` plugin runs it may add buttons to the Director; "Next" (button, space bar or presenter remote) goes to the plugin first, and the plugin says whether it handled it. "End" always returns to the normal show, whatever the plugin does. |
| PG-5 | Isolation: plugin code runs in a sandboxed frame with its own content security policy. It has no access to files, the network or the app's internals, only to the message API in 6.3, limited by its declared `permissions`. Installing shows the permissions and asks for confirmation. |
| PG-6 | Every change a plugin makes to the show (scores, teams, next slide) goes through the same game commands as the Director and is journaled before it is shown. A plugin can store its own state with `storage.set`; it is journaled too, so a plugin resumes after a crash. |
| PG-7 | An error or a hang in a plugin never stops the show: the Director shows the error, and the plugin can be ended or restarted. A plugin that does not answer within 2 seconds is treated as hung. |
| PG-8 | `apiVersion` is the plugin API version. The app runs plugins of every API version it supports and refuses newer ones with a message. |
| PG-9 | Permission `network` (planned, opt-in) allows `net.fetch` with a 10-second timeout and `net.send` over UDP, TCP or OSC to a configured address, for lighting and sound desks. |

### 6.3 Message API (version 1)

The plugin and the app exchange JSON messages through `window.postMessage`. A
small helper script (`plugin-api.js`, shipped with the template) wraps them
into functions and events.

Events from the app to the plugin:

| Event | Payload | Permission |
| --- | --- | --- |
| `launch` | `{ settings, locale, theme }` | none |
| `end` | `{}` | none |
| `next` | `{}`; the plugin answers `{ handled: true \| false }` | none |
| `slide.shown`, `slide.left` | `{ index, kind, question, answers }` | none |
| `clock.started`, `clock.tick`, `clock.stopped` | `{ secondsLeft, paused }` | none |
| `buzzer.pressed` | `{ buzzer, button, order }` in receiver order | `buzzers` |
| `answer.received` | `{ team, answer, correct }` | `buzzers` |
| `fastest.first` | `{ team }` | `buzzers` |
| `answer.judged` | `{ team, correct }` | `scores` |
| `score.changed` | `{ team, old, new }` | `scores` |
| `teams.changed` | `{ teams: [{ id, name, buzzer, score, active }] }` | `teams` |

Requests from the plugin to the app (each returns a result or an error):

| Request | Effect | Permission |
| --- | --- | --- |
| `teams.list` | the teams with scores | `teams` |
| `scores.add(team, points)` | adds points (negative to subtract) | `scores` |
| `lights.set([{ buzzer, light }])` | sets buzzer lights while the plugin runs | `lights` |
| `show.next()`, `show.goto(slide)` | moves the show | `show` |
| `director.buttons([{ id, label }])` | sets the plugin's Director buttons | none |
| `storage.get(key)`, `storage.set(key, value)` | plugin state, journaled | none |
| `log(level, text)` | writes to the Director's plugin console | none |

### 6.4 Developer support

The template package contains `plugin-api.js`, an example `screen` plugin and
an example `service` plugin. The Director's plugin console shows log lines and
errors; "Test" runs a plugin against ten virtual teams without touching the
running show.

## 7. Languages

| ID | Requirement | Status |
| --- | --- | --- |
| LN-1 | All UI text in Czech (default) and English, switchable at run time, including errors, quiz problems and receiver states (the backend sends codes, the UI words them). | done for existing screens, including the Receiver tab |
| LN-2 | Theme and plugin names and labels are given per language (`{ "cs": ..., "en": ... }`); a missing language falls back to English, then to the first one given. | planned |

## 8. Show flow

This is the path through a live show, as in the reference app (recon map
S13 to S20). The quizmaster prepares the quiz at home, then at the venue
opens it, starts it and runs it from the Director while the projector shows
the Host screen.

| ID | Requirement | Status |
| --- | --- | --- |
| SF-1 | Start: open a quiz file (or a recent one) and start the show. The last unfinished show can be resumed instead. | done (`.quiz` files and plain JSON); recent files planned |
| SF-2 | Launch check before the welcome screen: receiver connected (or keyboard only), projector window open on the chosen monitor, sound output working. Problems are listed with a fix; the show can start anyway. | planned |
| SF-3 | Welcome screen on the projector: quiz title, the theme's background and logo, optional welcome music (section 10). A small sign while the receiver is not connected (BZ-15). | partial: title and background colour only |
| SF-4 | Registration: players press any button; their buzzer numbers light up in a grid on the projector and appear in the Director's Teams tab with an editable name. Late presses still register. | done |
| SF-5 | Team names can be prepared before the show (buzzer number to name) and loaded with it, so a registered buzzer shows its team name at once. | planned |
| SF-6 | Slides, questions, judging, scores and the final ranking as in the reference app; all keyboard shortcuts work. | done for billboard, multiple choice and fastest finger |
| SF-7 | The projector screen is the user's own: theme, backgrounds and per-slide pictures or videos (section 5 and 9). | partial: per-slide pictures, videos and backgrounds done; themes planned |

## 9. Quiz authoring and media

| ID | Requirement | Status |
| --- | --- | --- |
| QZ-1 | Quiz editor in the Director, after the reference editor's layout: ribbon (File, Home, Insert, Question, View, Quiz show, and a contextual tab for the selected picture, video or sound), slide thumbnails (add from a layout gallery, cut, copy, paste, duplicate, delete, reorder by drag), the slide as the audience sees it with its parts selectable, a properties panel (pages, foldable sections, search, descriptions), an Edit question form, a table view for typing many questions, quizmaster notes under the slide. Undo and redo; the draft survives a crash. | done |
| QZ-2 | Slide types: billboard, question (2 to 4 answers, one per answer button A-D; multiple choice or fastest finger), answer explanation, end of round. An explanation is optional: the author inserts it after a question, and it shows that question with its correct answer above the author's text, picture or video. More types come with the reference's other formats. | done |
| QZ-3 | Per question: text, answers, correct answer, time limit, points (fixed or falling from a start to an end value), penalty, notes for the quizmaster. | done |
| QZ-4 | Media per slide: a picture, a sound clip and a video, each with its options (sound: when the slide opens, with the clock or on demand, loop, volume, the part that plays, loudness gain, a replay part; video: autoplay, muted, loop). A background per slide, with its video options, overrides the theme's (BG rules in section 5). | done |
| QZ-5 | Quiz file: one file (`.quiz`, a ZIP) with `quiz.json` and a `media/` folder, so a quiz with all its pictures and sounds can be copied to the event laptop as one file. Format version in `quiz.json`; older versions keep opening. | done: [`quiz-format.md`](quiz-format.md) |
| QZ-6 | The editor checks a quiz before saving and before starting (missing media, no correct answer, empty answers) with the same rules the game core uses. | done: the editor cannot make an unplayable question; other problems (e.g. a missing media file) are listed under File › Info and marked on the thumbnails |
| QZ-7 | Test run: play the quiz from the editor with virtual buzzers, without touching the last real show. | done: from the start (F5) or from the selected slide (Shift+F5); the Director's End show becomes Back to editor |

## 10. Sounds and music

| ID | Requirement | Status |
| --- | --- | --- |
| SN-1 | Built-in default sounds for every game event: clock start, last 5 seconds countdown, buzz, correct, wrong, time up, answer reveal, leaderboard, final ranking. They ship with the app, are our own or CC0, and their licence is recorded in the repository. | partial: placeholder tones for six events |
| SN-2 | Background music: one track or playlist for the welcome screen and one for between questions, looped, faded out when a question starts and back in after it. | planned |
| SN-3 | Every built-in sound and music slot can be replaced by the user's own file (MP3, WAV, OGG, M4A) in the show settings; the choice is saved with the quiz or with the show settings, not in the app. | planned |
| SN-4 | Volume per group (effects, music, question clips) and one master mute key in the Director. | planned |
| SN-5 | Sounds play from the app itself, not from a window, so they keep playing when the projector window is closed or moved, and they never play twice. | planned |
| SN-6 | A theme can bring its own sound set (section 5.2) that replaces the defaults. | planned |
| SN-7 | Sound pad: up to 12 short sounds the quizmaster can fire from the Director. Jingle per team when it buzzes first. | planned (could) |
| SN-8 | Trim: a slide's sound plays only a chosen part of its file, set on a waveform in the Trim window (sound ribbon › Timing, the sound's context menu, or the Part row in the properties). Drag to select, click to place the cursor, Play selection (Space) or from the cursor, Set start (I) and Set end (O) while listening, times typed as `75`, `1:15`, `0:01:15` or `1:15.5`, mouse wheel to zoom, Shift + wheel or the scroll bar to pan. On a question the sound can also replay a part once, after an optional delay, when the answer is revealed. The file is not changed; OK is one undo step. | done |
| SN-9 | Normalize volume: measures the loudness of the part of each sound that plays (ITU-R BS.1770-4, gated, in LUFS) and sets its loudness gain (-30 to +20 dB, on top of Volume) to a target: Standard -16 LUFS, Quieter -18 or Broadcast -23 (EBU R 128); the choice is remembered on the computer. Raising stops where the sample peak would pass -1 dBFS (status "limited"). Home › Tools › Normalize volume lists every sound with its loudness, peak, change and status, ticked where it changes, and applies the ticked ones as one undo step; Normalize on the sound ribbon or context menu does one sound at the remembered target. A trim re-measures a sound that has a gain. Files are not changed. | done |

## 11. Open questions

| Question | Who | Why it matters |
| --- | --- | --- |
| Which model is it: 215 (dome) or 303S (RGB)? Label under the buzzer base. | Jakub | Decides the protocol and whether lights are colours. |
| USB vendor and product ID of the cable (Receiver tab or `hid-monitor`). | Jakub | Selects the protocol automatically. |
| The SDK files and protocol documentation for that model and the HID cable. | Jakub, from Enjoy | Lights, press times, battery level. |
| SDK terms for using it in this app. Decided 2026-10-09: the app is a private copy for Jakub, not open source and not distributed, so publishing the protocol is no longer a question; Enjoy is asked for the SDK for personal use. | Jakub, from Enjoy | The vendor protocol may then live in this (private) repository. |
| Does the receiver report one press per USB report, or several? | hardware test | Affects how ties within one report are ordered. |
| Does the receiver know which buzzers are switched on and in range before anyone presses, and their battery level? Not asked in the October email; worth a follow-up if the SDK does not say. | Jakub, from Enjoy | Shows connected buzzers before registration (BZ-18). |
