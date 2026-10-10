# Quiz file format (version 4)

A quiz travels as one file with the extension `.quiz`. It is a ZIP archive:

```text
my-quiz.quiz
├── quiz.json          the quiz: slides, questions, answers, scoring
└── media/             pictures, sounds and videos the quiz uses
    ├── flag.png
    ├── round-2/intro.mp4
    └── song.mp3
```

The app writes this file from the quiz editor. A plain `quiz.json` with a
`media/` folder next to it opens too; that is handy for writing quizzes by
hand or generating them with a script.

Rules for readers and writers (implemented in `crates/quiz-file`):

- `quiz.json` is UTF-8 JSON at the root of the archive. Its fields are below.
- Media files live under `media/`. Slides refer to them by their path in the
  archive, for example `"media/flag.png"`. Names may contain sub-folders.
  Every part must work on Windows, macOS and Linux alike, so these are
  refused: empty parts, `\ : * ? " < > |` and control characters, a
  trailing dot or space (which also rules out `.` and `..`), and names
  Windows reserves (`CON`, `PRN`, `AUX`, `NUL`, `COM0`-`COM9`,
  `LPT0`-`LPT9`, with any extension). The editor renames imported files to
  fit, and so it does when it opens a `.quiz` file an older version wrote
  with such names (see Versioning).
- ZIP entry names written with backslashes (some Windows tools do that) are
  read with `/` in their place.
- Entries outside `quiz.json` and `media/` (for example the `__MACOSX/`
  folder some ZIP tools add) are ignored.
- A file does not open at all if it unpacks to more than 4 GB or has more
  than 10,000 entries, if `quiz.json` is missing or is not JSON of this
  format, or if its `schemaVersion` is newer than the app's.
- The show also refuses a file that breaks any other rule in this
  document, for example a slide referring to a media file that is not in
  the archive. The editor opens such a file all the same and lists what to
  fix before the quiz can be played.
- Writers store media uncompressed (they are compressed already), save
  only the media the quiz uses, and refuse to write a file that readers
  would refuse for its size or number of entries.

When a show starts, the app unpacks the file into its data folder
(`quizzes/<content hash>/`) and plays the media from there, so the show
keeps working and can be resumed after a crash even if the original file
was moved or deleted.

## quiz.json

```json
{
  "schemaVersion": 4,
  "title": "Pub quiz, March",
  "theme": "builtin.night",
  "slides": [
    { "id": "round-1", "type": "billboard", "title": "Round 1: Geography",
      "picture": "media/map.jpg",
      "background": { "file": "media/stage.mp4", "repeat": true, "muted": true, "sync": false } },
    { "id": "q1", "type": "question", "mode": "everyone",
      "text": "Which city is the capital of Moravia-Silesia?",
      "answers": ["Opava", "Ostrava", "Frýdek-Místek", "Karviná"], "correct": 1,
      "timeLimitS": 20, "points": 100, "pointsEnd": 50, "penalty": 0,
      "notes": "Ostrava has been the regional capital since 2000." },
    { "id": "q1-why", "type": "explanation",
      "text": "The region was created in 2000 with Ostrava as its seat." },
    { "id": "q2", "type": "question", "mode": "fastestFinger",
      "text": "Name this song.",
      "sound": { "file": "media/song.mp3", "start": "withClock", "volume": 80,
                 "part": { "fromMs": 15000, "toMs": 45000 }, "gainDb": -2.5,
                 "replay": { "part": { "fromMs": 45000, "toMs": null }, "delayMs": 1000 } } },
    { "id": "end-1", "type": "endOfRound", "title": "End of round 1" },
    { "id": "final", "type": "final", "title": "Tonight's winners",
      "sound": { "file": "media/fanfare.mp3", "start": "onDemand", "volume": 90 } }
  ]
}
```

### Quiz

| Field | Type | Meaning |
| --- | --- | --- |
| `schemaVersion` | number | `4`. Version 1 to 3 files open unchanged; a file with a higher version is refused with a message. |
| `title` | text | Shown on the welcome screen and in the Director. |
| `theme` | text or `null` | Id of the look of the screens (`builtin.classic`, `builtin.studio`, `builtin.night`, `builtin.showtime`, `builtin.neon`, later installed themes, spec 5.2). Missing, `null` or not installed: the default look, so a quiz never fails to open or play over it. A value that is not a theme id (dot-separated parts of lower-case letters, digits, `-` and `_`, at least two parts, at most 100 bytes) reads as missing. |
| `slides` | list | The show in order. |

### Every slide

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `id` | text | required | Unique within the quiz. Keeps the show journal valid when slides are edited. |
| `type` | `billboard`, `question`, `explanation`, `endOfRound`, `final` | required | See below. |
| `notes` | text | `""` | For the quizmaster only. |
| `picture` | media path | none | PNG, JPEG, WebP, GIF or SVG. |
| `video` | object | none | Shown in place of a picture; a slide has one or the other. Once started it pauses while a player answers, while the clock is paused and once the question is over, and goes on after. |
| `video.file` | media path | required | MP4 (H.264) or WebM. |
| `video.autoplay` | yes/no | `true` | Starts by itself `introPauseMs` after the slide is shown; otherwise it waits for the quizmaster's Play video. Either way Play video starts it again from the start of `part`. |
| `video.introPauseMs` | 0 to 60000 | `0` | Milliseconds between showing the slide and starting the video, so the question can be read first. The editor gives a new video 1000. |
| `video.muted` | yes/no | `false` | |
| `video.repeat` | yes/no | `false` | Plays `part` again from its start until the slide ends. |
| `video.volume` | 0 to 100 | `100` | Percent. |
| `video.speed` | 25 to 250 | `100` | Playback speed in percent. |
| `video.part` | span | whole file | The part of the file that plays. The file itself is not changed. |
| `video.effect` | `blur`, `mosaic` or `null` | `null` | Questions only: hides the picture at full strength when the question starts and fades out as the clock runs down, gone when the time is up or the answer is revealed. Without a clock it stays at full strength until then. |
| `video.replay` | object | none | Questions only: once the answer is revealed, the video stops and plays this part once, as `sound.replay`. |
| `video.replay.part` | span | required | The part to play then. |
| `video.replay.delayMs` | 0 to 60000 | `0` | Milliseconds between the reveal and the replay. |
| `sound` | object | none | Played from the Director computer's sound output. |
| `sound.file` | media path | required | MP3, WAV, OGG or M4A. |
| `sound.start` | `onSlide`, `withClock`, `onDemand` | `onSlide` | When the slide opens; while the question clock runs (pauses with it; questions only); or only when the quizmaster presses Play. |
| `sound.repeat` | yes/no | `false` | Plays `part` again from its start until the slide ends. |
| `sound.volume` | 0 to 100 | `100` | Percent. |
| `sound.part` | span | whole file | The part of the file that plays. The file itself is not changed. |
| `sound.gainDb` | -30 to 20 | `0` | Loudness gain in dB on top of `volume`, to 0.1 dB: normalizing sets it so all sounds play equally loud (spec SN-9). |
| `sound.replay` | object | none | Questions only: played once after the answer is revealed. |
| `sound.replay.part` | span | required | The part to play then. |
| `sound.replay.delayMs` | 0 to 60000 | `0` | Milliseconds between the reveal and the replay. |
| `background` | object | none | Picture or video behind this slide, instead of the theme's. Versions 1 and 2 wrote the media path alone; it reads as a background with the default options. |
| `background.file` | media path | required | A picture or a video (formats as `picture` and `video.file`). |
| `background.repeat` | yes/no | `true` | Video only: loops. |
| `background.muted` | yes/no | `true` | Video only: plays without its sound. |
| `background.sync` | yes/no | `false` | Video only: pauses while a player answers, while the clock is paused and once the question is over. |

A **span** is `{ "fromMs": number, "toMs": number or null }`, milliseconds
from the start of the file; `toMs: null` runs to the end of the file, so the
whole file is `{ "fromMs": 0, "toMs": null }`. A span must not end before it
starts.

### `billboard`

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `title` | text | required | |
| `text` | text | `""` | Shown under the title (and under the picture). |

### `question`

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `text` | text | required | The question. |
| `mode` | `everyone`, `fastestFinger` | required | Everyone answers with A-D; or the first to buzz answers and the quizmaster judges. |
| `answers` | list of text | `[]` | 2 to 4 for `everyone` (one per answer button A-D). Up to 6 (A-F) for `fastestFinger`, which the player answers aloud; it may be empty. |
| `correct` | number | none | Index into `answers`, from 0. Required for `everyone`. |
| `timeLimitS` | number | `20` | Seconds, at most 3600; `0` means no clock (the quizmaster ends the question). |
| `points` | number | `10` | Points for a correct answer at the start of the clock, -100000 to 100000. |
| `pointsEnd` | number | none | Points when the clock runs out, same range; points fall evenly from `points` to this. |
| `penalty` | number | `0` | Points taken for a wrong answer, 0 to 100000. |

### `explanation`

Explains the answer of a question. It explains the nearest question before
it in the same round (an `endOfRound` or `final` slide ends the search), and the screen
shows that question with its correct answer above `text`. With no question
before it in its round, only `text` and the slide's picture or video are
shown. Players do not answer; the next step goes to the next slide.

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `text` | text | `""` | Why the answer is right. Line breaks are kept. |

### `endOfRound`

The leaderboard takes the place of a picture or video, so the slide's own
are not shown.

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `title` | text | required | Shown above the leaderboard. |

### `final`

The final ranking: a drum roll, then third, second and first place one by
one, then the whole ranking, one quizmaster step each. Its `sound` plays as
on any slide (when the slide opens, or on demand from the Director), so it
carries the winners' music, and its `background` replaces the theme's
`final` screen. It has no `picture` or `video`. A quiz without a `final` slide shows the ranking after its
last slide; with one, the show ends after its last slide, and the
quizmaster's "go to the final ranking" goes to the last `final` slide.

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `title` | text | `""` | Shown above the podium and the ranking; empty shows the default title. |

## Spreadsheets

Questions can also be written in a spreadsheet and imported into a quiz
(spec QZ-8), in the columns of the reference app's Excel template; an
export writes a quiz back in them. `quiz_core::sheet` turns rows into
slides and back, `quiz-file` reads and writes the files.

**Files.** Import reads `.xlsx`, `.xlsm`, `.xls`, `.ods` (every visible
sheet) and `.csv`. A CSV file is UTF-8 or UTF-16 with a byte order mark,
else UTF-8, else Windows-1250; its separator is whichever of `;`, tab and
`,` the headings row uses most. Export writes UTF-8 with a byte order mark
and CRLF line ends, separated by `;` when the app is in Czech and `,` in
English, as Excel expects in those locales.

**Sheets and rows.** The headings (any case) are in a sheet's first row
with values, a CSV file's first line. A sheet without a `Q` column is
skipped; with more than one sheet of questions, each is a round that a
billboard with the sheet's name opens. Each row with a value is a slide.

| Column | Value |
| --- | --- |
| `Q` | The text, after an optional prefix that gives the slide type (below). |
| `A1`-`A6` | Answers. `[Paris]` marks the correct one; with none marked, `A1` is correct and the answers are shuffled. |
| `P1` | The slide's picture. |
| `NOTES` | Notes for the quizmaster. |
| `SOUND`, `VIDEO` | The slide's sound and video. |
| `BACKGROUND`, `BACKGROUND-VIDEO` | The slide's background (a picture or video; a video in the second). |
| `ANSWER-TIME` | Seconds, 0 to 3600. |
| `POINTS-START`, `POINTS-END` | Points, falling from the start to the end value when both are given. |
| `PENALTY-POINTS-START` | Points taken for a wrong answer (either sign). |

**Prefixes.** `{Q}` question, `{T}` test question (imported with no
points), `{B}` billboard (the first line is its title, the rest its text),
`{E}` end of round, and this app's own `{X}` explanation and `{F}` final
results. A question takes the qualifier `EA` (everyone answers, 2 to 4
answers) or `FF` (fastest finger, up to 6): `{Q, FF}`. Without a prefix, a
row with text or answers is a question (everyone answers when it has 2 to
4 answers) and a row with only media a billboard.

**Media.** A cell names a file, relative to the sheet or as a full path;
a file not found there is looked for by its name next to the sheet. A file
taken from outside the sheet's folder (a full path, or one with `..`) is
listed in the import's notes. A network or device path
(`\\server\share\…`, `\\?\…`, `\\.\…`, `\??\…`) is never read, as
Windows would contact the server just to look; only its name is looked for
next to the sheet. The file is copied into the quiz (`{L}`, the
reference's mark for a linked file, is read and the file copied all the
same). Settings follow in braces,
separated by `;`: `intro.mp4{start-at=00:00:05;loop=false}`. Times are
`hh:mm:ss`, `mm:ss` or seconds, with a fraction; flags are `true`/`false`,
`yes`/`no` or `1`/`0`.

| Setting | On | Value |
| --- | --- | --- |
| `start-at`, `stop-at` | sound, video | The part that plays. |
| `loop` | sound, video, background video | Flag. |
| `volume` | sound, video | 0 to 100. |
| `silent` | video, background video | Flag. |
| `speed` | video | 25 to 250 (%). |
| `effect` | video | `blur`, `mosaic` or `none`. |
| `intro-pause` | video | Seconds before it starts (1 by default, as in the editor). |
| `sync-with-quiz` | background video | Flag: holds with the show. |
| `autoplay` | video | This app's own. Flag: starts by itself or waits for Play video. |
| `plays` | sound | This app's own. `on-slide`, `with-clock` (questions) or `on-demand`. |
| `gain` | sound | This app's own. Loudness gain in dB. |
| `replay`, `replay-start-at`, `replay-stop-at`, `replay-delay` | sound, video on a question | This app's own. The part played again after the reveal, and the seconds before it. |

Export writes only the settings that differ from these defaults, every
media file next to the CSV file under its own name (numbered when another
file has it), and the correct answer in brackets, so an exported quiz
imports as it was. A cell a spreadsheet program would run as a formula
(starting with `=`, `@`, a tab, or `+` or `-` not followed by just a
number) gets a leading `'`, so it opens as text; the import takes it off.

**What is left out.** The reference's other columns (`CAT`, `P2`-`P6`,
`A7`, `A8`, the bonus and evaluation columns, `STYLE`, `TRANSITION`...),
its other slide types (imported as billboards with their text), a second
language after `||`, a picture next to a video, media a slide cannot have
and files that are not there are left out, and the import lists each with
the rows it came up in.

## Playback notes

- Pictures and videos are shown by the system's web view (WebView2 on
  Windows, WebKit on macOS and Linux). On Linux, WebKit plays media through
  GStreamer and needs its plugins for MP3, M4A and MP4 (see tauri-apps/tauri
  issue 3725). The `.deb` recommends `gstreamer1.0-plugins-good`, `-bad` and
  `gstreamer1.0-libav`; with the AppImage, install them yourself.
- Media play without a click: Tauri's web view library (wry) allows
  autoplay by default, so a video with sound on the projector starts by
  itself. To verify on each OS with the installers.

## Versioning

| Version | Change |
| --- | --- |
| 1 | First version. |
| 2 | Adds `explanation` slides and, later within version 2, the quiz `theme` (builds from before that drop it when they save). Version 1 files read as version 2 unchanged. |
| 3 | `background` becomes an object with video options; sounds get `part`, `gainDb` and `replay`. Version 1 and 2 files read as version 3 unchanged. Later within version 3, a `fastestFinger` question may have up to 6 answers instead of 4; earlier builds list such a question as a problem. |
| 4 | Adds `final` slides. Version 1 to 3 files read as version 4 unchanged. Later within version 4, videos get `introPauseMs`, `volume`, `speed`, `part`, `effect` and `replay`; a video without them plays as before. |

Fields may be added within a version as long as older files keep their
meaning; readers ignore fields they do not know. Anything else (a new slide
type, a changed meaning, a rule that refuses files that used to play) needs
the next `schemaVersion` and a migration that opens older files unchanged.
Readers keep a quiz in the current version, so saving an older file writes
the current one.

Builds of the editor within version 4 kept a few fields when a slide changed
type that its new type cannot play: a `picture` or `video` on a `final`
slide, and `sound.start: withClock`, `sound.replay`, `video.replay` and
`video.effect` on a slide that is not a question. Readers drop them, and a
sound that waited for the clock plays when the slide opens, so such a file
opens and plays; saving writes it without them.

These limits were added within version 2, before that rule was kept, so a
version 1 or 2 file may break them. The editor opens such a file and lists
each problem; the show plays it once it is fixed and saved.

| Limit | An older file that breaks it |
| --- | --- |
| Media names work on every system (the name rules above). | In a `.quiz` file, the editor gives each such file the closest name that fits (`what?.png` becomes `what_.png`, with `-2` added if that name is taken) and points the slides to it; the quiz shows as unsaved until it is saved. A plain `quiz.json` is left as written and the names are listed. |
| An `everyone` question has at most 4 answers, one per answer button. | Listed; remove answers or make it `fastestFinger`, which allows 6. |
| `points`, `pointsEnd` and `penalty` within 100000, `timeLimitS` at most 3600. | Listed; the editor keeps new values within range. |
