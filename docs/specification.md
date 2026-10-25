# Specification

What the app must do, written as testable requirements. How it is built is in
[`design.md`](design.md); how to check the buzzer link on real hardware is in
[`hardware-test.md`](hardware-test.md). Last updated 2026-10-10.

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
| PL-7 | The app is called Buzzdome; its icon is a top view of a dome buzzer, orange by default. In Settings the quizmaster picks another of its six colours (orange, blue, green, purple, red, yellow) for the windows and their taskbar buttons, kept for the next start; installed shortcuts and the macOS Dock keep the orange one. | done |
| PL-8 | One copy of the app runs at a time, as two would write one show journal: starting it again brings the running Director forward. | done |

## 4. Buzzer link

The most important part of the app: a show is only as good as its buzzers.

### 4.1 Hardware

Wireless buzzers (A, B, C, D and one big button, with a light) talk by
radio to a receiver; in a fastest-finger question players buzz in with the
answer buttons where the question has answers A to D, else with the big
button (BZ-19); the receiver connects to the computer through a
"USB-HID cable" (USB on one end, RJ45 on the other). Details and sources are
in [`replica/hardware.md`](../replica/hardware.md).

The manufacturer (Enjoy Tech) answered Jakub's email in October 2026:

- the model is either 215 (dome) or 303S (RGB); the label under the buzzer's
  base says which;
- the receiver reports the press **time**;
- dome and RGB versions use **different protocols**;
- the protocol is described in their SDK, which they provide free of charge.

Jakub's own set is the 303S (confirmed 2026-10-21), and on 2026-10-21 he got
Enjoy's official SDK kit for it. Its protocol, written down in our own words
from the kit and the unofficial copy of the SDK Jakub approved on 2026-10-20
(`docs/design.md` 2.4), is what the app drives: presses with each buzzer's
own timer, by which fastest finger is ordered, and every light in colour. It
has no battery level. The 215 (dome) uses the same cable with another
protocol, which the app does not drive. The app reads buzzers in three ways:

1. **Learned mapping** (no vendor information needed): the user teaches the
   app which USB report each button of each buzzer sends. Presses work;
   lights do not. Teaching is per buzzer, which is fine for a test with a
   few buzzers but too slow for a full event. It needs a receiver that sends
   plain HID reports, so it does not cover Enjoy's `10c4:1819` cable: that
   cable is a USB-to-serial bridge whose serial line only the 303S driver
   opens (`docs/design.md` 2.4), and the app always uses that driver for
   it, which takes no teaching.
2. **Vendor protocol**: presses with buzzer ID, and lights; press time and
   battery level where the protocol has them. Selected automatically by the
   receiver's USB vendor and product ID. Built for the 303S (presses with
   the buzzer's timer, colours), to be checked on the hardware.
3. **Keyboard**: keys `1`-`0` (the big button, so they buzz in only where
   BZ-19 lets it) and the on-screen virtual buzzers (every button), always
   available, also covering the vendor's "keystroke" mode.

So learn mode is no fallback on Enjoy's cable: if the 303S driver does not
work with a set, only the keyboard and on-screen buzzers do until it is
fixed.

### 4.2 Requirements

| ID | Requirement | Status |
| --- | --- | --- |
| BZ-1 | The Director lists all USB HID devices (vendor and product ID, name, manufacturer, usage page, interface) and lets the user pick the receiver. Keyboards and mice are greyed out. | done |
| BZ-2 | The chosen receiver is remembered and reopened automatically at the next start. | done |
| BZ-3 | When the receiver is unplugged or fails, the Director shows it at once and the app reconnects by itself within 2 seconds of the receiver coming back, without losing the show. | done (simulated) for the USB cable; a 303S receiver that loses power or its RJ45 plug behind the cable is set up again only if it says so when it starts again (to check on the hardware), else with the Receiver tab's Reconnect |
| BZ-4 | Every press becomes a game command (buzzer ID, button) that is written to the show journal **before** it changes anything on screen, exactly like a keyboard press. After a crash the show resumes with those presses. | done |
| BZ-5 | Fastest-finger order is the order the receiver reports, or the receiver's press times when the protocol provides them. The PC never orders presses by anything else, its own clock included. Presses in one USB report keep their order within the report. | done: the receiver's order, and for the 303S the buzzers' own timers (`docs/design.md` 2.6), so a player switched on mid-question joins at the next; checked with buzzers 1 and 2 on 2026-10-25, buzzers far apart in ID still to check |
| BZ-6 | A button held down, or a receiver that repeats a report, counts as one press. A new press needs a release or a 150 ms pause. | done, also for a 303S press the receiver sends again with the same timer (seen 2026-10-25) |
| BZ-7 | Learn mode: for a chosen buzzer number and button, the user presses the button at least twice; the app derives a signature (which bytes matter) and stores it per receiver model. Conflicting signatures are refused with a message. While learning, presses do not reach the game. | done |
| BZ-8 | Diagnostics: the Director shows live presses and the raw reports (hex) with what they decoded to, and exports the raw log to a text file that can be sent to a developer. | done |
| BZ-9 | A command-line monitor (`hid-monitor`) lists devices and prints raw reports with timestamps, without the app. | done |
| BZ-10 | Lights follow the game: ready, locked out, first to buzz, correct, wrong (table 4.3). Dome buzzers use their fixed colours; RGB buzzers use the colours in table 4.3. | done for the 303S, in the colours of table 4.3, checked on the hardware on 2026-10-25 (the whites show as two purples, told apart at arm's length); every buzzer goes dark when a show begins or ends; dome buzzers are not driven |
| BZ-11 | Up to 100 dome buzzers or 64 RGB buzzers on one receiver (the receiver's own limits, see `replica/hardware.md`). Up to about 50 are used at our events. | to verify on hardware |
| BZ-12 | Keyboard and on-screen virtual buzzers work at the same time as the receiver, so a missing buzzer can be replaced mid-show. | done |
| BZ-13 | If another program (for example the vendor's software) holds the receiver, the error says so. | done |
| BZ-14 | Battery level of each buzzer, shown in the buzzer test view. | not possible with the 303S: its SDK has no battery level, and in the hardware test of 2026-10-25 no press or other message carried one |
| BZ-15 | Host screen shows a small "buzzers not connected" sign on the welcome screen while the chosen receiver is not connected (the reference app does this too). | planned |
| BZ-16 | Buzzer test view: a grid of buzzer numbers that light up with the pressed button, for range tests in the venue. | partial (Receiver tab shows presses as a list) |
| BZ-17 | Buzzer panel in the Director during a show: every known buzzer number with its state (registered, last button and when, light, battery when the protocol gives it), and the receiver state on top. A buzzer not seen for a long time is marked. | planned |
| BZ-18 | If the receiver reports which buzzers are switched on and in range without a press (open question, section 11), the registration screen and the buzzer panel show them as "on" before anyone presses. | waits for the hardware: the 303S's ping may give it, but what it answers is unknown |
| BZ-19 | Which buttons buzz in a fastest-finger question follows its answers (Jakub, 2026-10-21): where it has one to four answers, the answer buttons A to D do, so the first press is also the player's answer, and the Director and the projector show the letter pressed; with no answers, or with E and F, which are read out, the big button does. The quizmaster judges either way. A player who joins while a fastest-finger question is under way (switched on, or new to the show by a press or by being added) plays from the next question, as the others' buzzer timers count from the question's start; in other questions they play at once. | done (rules version 5; shows begun before take any button and let a joining player buzz at once) |

### 4.3 Light states

`Light` values come from the game core (`crates/quiz-core`). The RGB colours
are this app's defaults and will become part of the theme (section 5). The
303S's protocol has no flashing, so each light has a colour of its own.

| Game situation | `Light` | Dome buzzer | RGB buzzer |
| --- | --- | --- | --- |
| no question running | `Off` | off | off |
| buzzer may press | `Ready` | green | blue |
| out for this question (pressed wrong, or another team is answering) | `Locked` | red | dim white (Jakub's choice, told apart from wrong) |
| pressed first | `First` | white, flashing | white |
| answer judged correct | `Correct` | green, flashing | green |
| answer judged wrong | `Wrong` | red, flashing | red |

### 4.4 Acceptance test

The buzzer link is accepted when the checklist in
[`hardware-test.md`](hardware-test.md) passes on the event laptop (Windows)
and on one Mac, with at least four buzzers.

## 5. Backgrounds and themes

Users bring their own wallpapers and looks. This mirrors what the reference
app offers (background picture or looping video per slide, a gallery of
backgrounds, skins that inherit from a built-in skin and override only what
changes) with our own file format.

Built so far: backgrounds per slide, and twelve built-in themes (Classic,
Studio, Night, Showtime, Neon, Daylight, Aurora, Sunset, Chalkboard, Gala,
Bakchus, Bakchus Contours) chosen per quiz on the editor's Design tab, with a live preview while the
pointer is on one. Each question also picks how its clock is drawn (ring,
bar, digital, hourglass or stopwatch). The built-in themes are
theme packages in the layout of 5.2 (unzipped) and are read and resolved by
the same code that will read installed ones. Installing theme packages, the
background gallery and round or quiz backgrounds are planned.

### 5.1 Background files

| ID | Requirement |
| --- | --- |
| BG-1 | Image formats: PNG, JPEG, WebP (also GIF and SVG, which the app reads like any picture). Video formats: MP4 (H.264) and WebM. A video's sound plays only with Silent off (BG-4). |
| BG-2 | Aspect ratio 16:9. Recommended size 1920 x 1080 px; minimum 1280 x 720; maximum 3840 x 2160. Images up to 20 MB, videos up to 200 MB. |
| BG-3 | A background that is not 16:9 is scaled to cover the screen and cropped around its centre; the import dialog shows the crop and warns. Smaller than the minimum or larger than the limits is refused. |
| BG-4 | A background video starts as soon as its slide appears and has its own options: Loop (on), Silent (on) and Sync with quiz (off), which pauses it while a player answers, while the clock is paused and once the question is over. The editor shows its first frame with a video mark, and the slide's context menu can play or stop it there, or remove the background. Done. |
| BG-5 | Backgrounds sit behind all content. The app draws its own panels behind text, so any background stays readable; designers should still keep the middle band calm (layout guide in 5.4). |
| BG-6 | Choice per slide, per round, per quiz and per theme. The most specific wins: slide > round > quiz > theme > built-in. Slide, theme and built-in done; round and quiz to come. |
| BG-7 | A quiz file embeds every background it uses (in `media/`), so a quiz moved to another computer looks the same. Nothing is linked to an outside path during a show. |
| BG-8 | The Director has a background gallery: import files or a theme package, preview on a 16:9 frame with the layout guide, delete, and filter. |
| BG-9 | A broken or missing background never stops the show: the next level in BG-6 is used and the Director shows a warning. Done. |

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
    "wrong": "default"
  },
  "fonts": {
    "heading": "fonts/Heading.woff2",
    "body": "fonts/Body.woff2"
  },
  "corners": "square",
  "headings": "uppercase",
  "buzzerColors": {
    "ready": "#00ff00",
    "first": "#0000ff"
  }
}
```

| ID | Requirement |
| --- | --- |
| TH-1 | `format` is the theme format version; the app refuses a format it does not know, with a message. `id` is unique (reverse domain style); ids starting with `builtin.` belong to the app's own themes. Importing a theme with an existing `id` and a higher `version` updates it; a theme package that takes a reserved or taken id is left out with a message, and the other themes stay. |
| TH-2 | Inheritance: every key that is missing, or set to `"default"`, comes from the `parent` theme, recursively, ending at a built-in theme. A theme only lists what it changes. Backgrounds are the exception: a screen the theme leaves out takes the theme's own background of the screen it falls back to (TH-5) before any parent's, so a theme that sets only `default` is not mixed with its parent's pictures. |
| TH-3 | Paths are relative to the package root and must stay inside it. Every file is checked on import (format, size, BG-2). |
| TH-4 | Colours are `#rrggbb` or `#rrggbbaa`. Colour keys: `text` (on the background), `shadow` (under that text), `glow` (around slide titles), `panel` (boxes on the background), `bar` and `barEdge` (the bars at the top and bottom), `tile` (leaderboard rows), `accent` (scores and counters), `answerA` to `answerF` (A-D are the buzzer's buttons; E and F are read out in fastest finger questions), `correct`, `wrong`. The app picks white or dark text on filled areas itself, so text stays readable. Fonts are WOFF2, WOFF, TTF or OTF files, best variable fonts with a weight axis (a static font is used at its one weight); letters a font lacks come from system fonts. Fonts must be licensed for embedding; the import dialog reminds the user. `corners` is `round` or `square` (boxes, bars and badges on the stage; clock faces stay round); `headings` is `asWritten` or `uppercase` (the text in the heading font: titles, team names and big numbers). |
| TH-5 | Screen keys: `default` (every screen without its own), `welcome` (team registration before the first slide), `question` (a question being asked), `answer` (a question once its answer is shown, and explanation slides; falls back to `question`), `leaderboard` (the leaderboard shown over a slide), `roundEnd` (end-of-round slides; falls back to `leaderboard`), `final` (final results). Billboards use `default`. Unknown keys, and `corners` or `headings` values the app does not know (it uses the parent's), are ignored with a warning, so newer themes still load in older apps. |
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
| SF-4 | Registration: players press any button; their buzzer numbers light up in a grid on the projector and appear in the Director's Teams tab with an editable name. Late presses still register, between questions or in one (in fastest finger the late team plays from the next question, BZ-19). On the Enjoy 303S a buzzer showing dark registers only outside fastest finger and its judging, as a press there counts only from a ready buzzer (design 2.4). | done |
| SF-5 | Team names can be prepared in the Director's Teams tab, before a show starts or during it, and are kept for the next shows: typed, pasted (one name per line) or loaded from a text or CSV file (first column, UTF-8 or Windows-1250). A buzzer is paired with a team by dragging its number onto the name, or by clicking the number and then the name; its player takes the team's name (a player who had it loses it) and the registration screen shows the name under the number. Pairing works at any point of the show, for late teams too. | done; a saved buzzer-to-name list that pairs on the first press is planned |
| SF-6 | Slides, questions, judging, scores and the final ranking as in the reference app; all keyboard shortcuts work. | done for billboard, multiple choice and fastest finger |
| SF-7 | The projector screen is the user's own: theme, backgrounds and per-slide pictures or videos (section 5 and 9). | partial: per-slide pictures, videos and backgrounds done; twelve built-in themes (section 5.2), the Design tab gallery and five clock designs done; importing user themes (TH-1, TH-6) planned |

## 9. Quiz authoring and media

| ID | Requirement | Status |
| --- | --- | --- |
| QZ-1 | Quiz editor in the Director, after the reference editor's layout: ribbon (File, Home, Insert, Question, View, Quiz show, and a contextual tab for the selected picture, video or sound), slide thumbnails (add from a layout gallery, cut, copy, paste, duplicate, delete, reorder by drag), the slide as the audience sees it with its parts selectable, a properties panel (pages, foldable sections with More options for rarely used rows, search across all pages with accents ignored, descriptions or tooltips, dark panel, an alphabetical list, Reset, Copy and Paste of a value from a row's context menu, a slider next to bounded numbers, drag a number's name to change it), several slides selected at once (Ctrl and Shift click, Ctrl+A) to edit their shared properties, cut, copy, delete or move them together, an Edit question form, a table view for typing many questions, quizmaster notes under the slide. Undo and redo; the draft survives a crash. | done |
| QZ-2 | Slide types: billboard, question (multiple choice with 2 to 4 answers, one per answer button A-D, or fastest finger with up to 6 answers A-F or none, buzzed in as BZ-19 says), answer explanation, end of round, final results. An explanation is optional: the author inserts it after a question, and it shows that question with its right answers above the author's text, picture or video. A final results slide is optional too: it shows the final ranking where it stands (drum roll, third, second and first place, then everyone) with its own title, background and sound, so the winners' music plays when the slide opens or when the quizmaster plays it; without one, the ranking follows the last slide. More types come with the reference's other formats. | done |
| QZ-3 | Per question: text, answers, the right answers (one or several; on an everyone-answers question each gives all or a share of the points, e.g. 50 % for a nearly exact one), time limit, points (fixed or falling from a start to an end value), penalty, notes for the quizmaster. | done |
| QZ-4 | Media per slide: a picture, a sound clip and a video, each with its options (sound: when the slide opens, with the clock or on demand, loop, volume, the part that plays, loudness gain, a replay part; video: starting by itself after an intro pause or when the quizmaster plays it from the Director, muted, loop, volume, speed 25 to 250 %, the part that plays, a blur or mosaic effect that fades as the question's time runs out, a replay part after the reveal; it pauses and goes on with the show). A background per slide, with its video options, overrides the theme's (BG rules in section 5). | done |
| QZ-5 | Quiz file: one file (`.quiz`, a ZIP) with `quiz.json` and a `media/` folder, so a quiz with all its pictures and sounds can be copied to the event laptop as one file. Format version in `quiz.json`; older versions keep opening. | done: [`quiz-format.md`](quiz-format.md) |
| QZ-6 | The editor checks a quiz before saving and before starting (missing media, no correct answer, empty answers) with the same rules the game core uses. | done: the editor cannot make an unplayable question; other problems (e.g. a missing media file) are listed under File › Info and marked on the thumbnails |
| QZ-7 | Test run: play the quiz from the editor with virtual buzzers, without touching the last real show. | done: from the start (F5) or from the selected slide (Shift+F5); the Director's End show becomes Back to editor |
| QZ-8 | Questions from a spreadsheet: File › Import reads an Excel workbook (`.xlsx`, `.xlsm`, `.xls`), an OpenDocument sheet (`.ods`) or a CSV file in the columns of the reference app's template (`Q`, `A1`-`A6`, `P1`, `NOTES`, `SOUND`, `VIDEO`, `BACKGROUND`, `BACKGROUND-VIDEO`, `ANSWER-TIME`, `POINTS-START`, `POINTS-END`, `PENALTY-POINTS-START`) and inserts the slides after the selected one as one undo step. Each sheet with a `Q` column is a round that a billboard with the sheet's name opens; a prefix in `Q` gives the slide type, brackets mark the right answers (unmarked, `A1` is right and the answers are shuffled), and media files are copied into the quiz with their settings. What the app cannot take over is listed afterwards with its rows. File › Export writes the quiz as a CSV file that Excel opens, with the media next to it, and File › Import offers a sample sheet to start from. | done: columns and settings in [`quiz-format.md`](quiz-format.md#spreadsheets); `CAT` names a category of the quiz (QZ-10); the reference's other columns (e.g. `P2`-`P6`) and slide types are reported, not imported |
| QZ-9 | A slide's look over its theme, set in the properties panel: a gradient behind the slide, and for the question or title, the text and every answer alike the font (a list of the installed fonts), size, bold, italic, underline, colour, alignment, shadow and its colour, a fill gradient and its transparency. A colour picker (theme colour, palette of shades, standard and recent colours, custom colour with saturation and brightness, hue, opacity and hex code, a screen pipette where the web view has one), gradient presets and up to five own gradients; Copy and Paste of a value and Ctrl+C / Ctrl+V carry a colour as `#1E4A6B` to and from other programs. | done: the pipette only on Windows (the web views of macOS and Linux have none) |
| QZ-10 | Categories: named looks a quiz keeps, each a background (a picture, or a looping video with its options) and a slide style as in QZ-9. A Categories view lists them as sample slides and edits the one shown in the properties panel (name, background, looks); Home and Insert make a new one, and its context menu sets its background or deletes it. The Design tab's gallery, or a slide's Category row, gives the selected slides a category's look: its style and background are copied onto them, their picture, video and sound stay, and they remember it, so Apply again to slides refreshes them after the category changed. A slide's look can be saved as a new category. A spreadsheet's `CAT` column names a category of the quiz, whose look the imported rows take on, a row's own background winning. | done: copied on applying, as the reference's manual describes; format in [`quiz-format.md`](quiz-format.md#categories) |

## 10. Sounds and music

| ID | Requirement | Status |
| --- | --- | --- |
| SN-1 | Built-in default sounds for every game event: clock start, last 5 seconds countdown, buzz, correct, wrong, time up, answer reveal, leaderboard, final ranking. They ship with the app, are our own or CC0, and their licence is recorded in the repository. | partial: placeholder tones for six events |
| SN-2 | Background music: one track or playlist for the welcome screen and one for between questions, looped, faded out when a question starts and back in after it. | planned |
| SN-3 | Every built-in sound and music slot can be replaced by the user's own file (MP3, WAV, OGG, M4A) in the show settings; the choice is saved with the quiz or with the show settings, not in the app. | planned |
| SN-4 | Volume per group (effects, music, question clips) and one master mute key in the Director. | partly: one player volume for everything the show plays (SN-7); groups and the mute key planned |
| SN-5 | Sounds play from the app itself, not from a window, so they keep playing when the projector window is closed or moved, and they never play twice. | planned |
| SN-6 | A theme can bring its own sound set (section 5.2) that replaces the defaults. | planned |
| SN-7 | Media player and sound board, in the Director's Media player tab. A library of pictures, sounds and videos kept on the computer, shared by every quiz: Add copies files in (also dropped from a folder), Remove deletes the copy; a search above the list filters it by any part of a file name (Esc clears it; show keys do not fire while typing). Play, Pause, Stop and Rewind act on the selected file; sounds play in the Director, pictures and videos over the whole projector stage until stopped. A volume slider next to them sets the level everything the show plays sounds at (the slides' and the player's sounds and videos, the sound cues), at once while something plays; it is kept for the next start. The sound board is a grid of 3 × 4 to 8 × 8 pads (pads keep their place when it changes size): one click plays a pad's file, or the slide's sound or video; a playing pad is lit and counts down, a paused one shows a pause sign, an idle one its file's length; a click on a playing pad stops, restarts or pauses it, as the board is set. Files are dragged onto pads from the list or a folder, pads onto pads to swap; a pad has a label and a colour (or its column's). A new board fills itself with the library until the quizmaster arranges it. List and board can be shown alone or side by side with a divider; the choice is remembered. A jingle per team when it buzzes first is still planned. | done: player and board; team jingles planned |
| SN-8 | Trim: a slide's sound plays only a chosen part of its file, set on a waveform in the Trim window (sound ribbon › Timing, the sound's context menu, or the Part row in the properties). Drag to select, click to place the cursor, Play selection (Space) or from the cursor, Set start (I) and Set end (O) while listening, times typed as `75`, `1:15`, `0:01:15` or `1:15.5`, mouse wheel to zoom, Shift + wheel or the scroll bar to pan. On a question the sound can also replay a part once, after an optional delay, when the answer is revealed. The file is not changed; OK is one undo step. | done |
| SN-9 | Normalize volume: measures the loudness of the part of each sound that plays (ITU-R BS.1770-4, gated, in LUFS) and sets its loudness gain (-30 to +20 dB, on top of Volume) to a target: Standard -16 LUFS, Quieter -18 or Broadcast -23 (EBU R 128); the choice is remembered on the computer. Raising stops where the sample peak would pass -1 dBFS (status "limited"). Home › Tools › Normalize volume lists every sound with its loudness, peak, change and status, ticked where it changes, and applies the ticked ones as one undo step; Normalize on the sound ribbon or context menu does one sound at the remembered target. A trim re-measures a sound that has a gain. Files are not changed. | done |

## 11. Open questions

| Question | Who | Why it matters |
| --- | --- | --- |
| Which model is it: 215 (dome) or 303S (RGB)? Answered 2026-10-21: Jakub's set is the 303S (RGB, 64 colours, up to 64 buzzers per receiver). | Jakub | Decides the protocol and whether lights are colours. |
| USB vendor and product ID of the cable (Receiver tab or `hid-monitor`). | Jakub | Selects the protocol automatically. |
| The SDK files and protocol documentation for that model and the HID cable. Received 2026-10-21 for the 303S; it has no battery level. | Jakub, from Enjoy | Lights, press times, battery level. |
| What the 303S's SDK leaves open: the big button's code, the colour a press reports, the unit of the buzzer's timer, how far apart presses made together arrive, how quickly the lights follow with 64 buzzers taken, what the receiver does when its power or RJ45 plug goes. Answered 2026-10-25 on Windows with buzzers 1 and 2: the big button sends 5, a press reports the colour shown, the timer counts milliseconds from the last colour change (a colour repeated leaves it running), presses arrive on a 36 ms tick (0 to 36 ms apart for buzzers 1 and 2), the buzzer has no power switch, no battery level anywhere. Still open: the gap for buzzers far apart in ID and whether a higher ID's timer starts later, the light lag, the power or RJ45 plug, macOS. | hardware test (`hardware-test.md` sections 3, 6 and 7) | Fastest-finger order and its wait (`docs/design.md` 2.6). |
| SDK terms for using it in this app. Decided 2026-10-09: the app is a private copy for Jakub, not open source and not distributed, so publishing the protocol is no longer a question; Enjoy is asked for the SDK for personal use. | Jakub, from Enjoy | The vendor protocol may then live in this (private) repository. |
| Does the receiver report one press per USB report, or several? | hardware test | Affects how ties within one report are ordered. |
| Does the receiver know which buzzers are switched on and in range before anyone presses, and their battery level? Not asked in the October email; worth a follow-up if the SDK does not say. | Jakub, from Enjoy | Shows connected buzzers before registration (BZ-18). |
