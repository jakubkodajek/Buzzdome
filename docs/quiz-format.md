# Quiz file format (version 2)

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
  fit.
- ZIP entry names written with backslashes (some Windows tools do that) are
  read with `/` in their place.
- Entries outside `quiz.json` and `media/` (for example the `__MACOSX/`
  folder some ZIP tools add) are ignored.
- A file is refused if it unpacks to more than 4 GB or has more than 10,000
  entries, if `quiz.json` is missing or invalid, or if a slide refers to a
  media file that is not in the archive.
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
  "schemaVersion": 2,
  "title": "Pub quiz, March",
  "theme": "builtin.night",
  "slides": [
    { "id": "round-1", "type": "billboard", "title": "Round 1: Geography",
      "picture": "media/map.jpg", "background": "media/stage.mp4" },
    { "id": "q1", "type": "question", "mode": "everyone",
      "text": "Which city is the capital of Moravia-Silesia?",
      "answers": ["Opava", "Ostrava", "Frýdek-Místek", "Karviná"], "correct": 1,
      "timeLimitS": 20, "points": 100, "pointsEnd": 50, "penalty": 0,
      "notes": "Ostrava has been the regional capital since 2000." },
    { "id": "q1-why", "type": "explanation",
      "text": "The region was created in 2000 with Ostrava as its seat." },
    { "id": "q2", "type": "question", "mode": "fastestFinger",
      "text": "Name this song.",
      "sound": { "file": "media/song.mp3", "start": "withClock", "volume": 80 } },
    { "id": "end-1", "type": "endOfRound", "title": "End of round 1" }
  ]
}
```

### Quiz

| Field | Type | Meaning |
| --- | --- | --- |
| `schemaVersion` | number | `2`. Version 1 files open unchanged; a file with a higher version is refused with a message. |
| `title` | text | Shown on the welcome screen and in the Director. |
| `theme` | text or `null` | Id of the look of the screens (`builtin.classic`, `builtin.studio`, `builtin.night`, `builtin.showtime`, `builtin.neon`, later installed themes, spec 5.2). Missing, `null` or not installed: the default look, so a quiz never fails to open or play over it. |
| `slides` | list | The show in order. |

### Every slide

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `id` | text | required | Unique within the quiz. Keeps the show journal valid when slides are edited. |
| `type` | `billboard`, `question`, `explanation`, `endOfRound` | required | See below. |
| `notes` | text | `""` | For the quizmaster only. |
| `picture` | media path | none | PNG, JPEG, WebP, GIF or SVG. |
| `video` | object | none | Shown in place of a picture; a slide has one or the other. |
| `video.file` | media path | required | MP4 (H.264) or WebM. |
| `video.autoplay` | yes/no | `true` | Starts when the slide is shown. |
| `video.muted` | yes/no | `false` | |
| `video.repeat` | yes/no | `false` | Loops until the slide ends. |
| `sound` | object | none | Played from the Director computer's sound output. |
| `sound.file` | media path | required | MP3, WAV, OGG or M4A. |
| `sound.start` | `onSlide`, `withClock`, `onDemand` | `onSlide` | When the slide opens; while the question clock runs (pauses with it); or only when the quizmaster presses Play. |
| `sound.repeat` | yes/no | `false` | Loops until the slide ends. |
| `sound.volume` | 0 to 100 | `100` | Percent. |
| `background` | media path | none | Picture or looping muted video behind this slide, instead of the theme's. |

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
| `answers` | list of text | `[]` | 2 to 4 for `everyone` (one per answer button A-D), at most 4 for `fastestFinger`, where it may be empty (verbal answer). |
| `correct` | number | none | Index into `answers`, from 0. Required for `everyone`. |
| `timeLimitS` | number | `20` | Seconds, at most 3600; `0` means no clock (the quizmaster ends the question). |
| `points` | number | `10` | Points for a correct answer at the start of the clock, -100000 to 100000. |
| `pointsEnd` | number | none | Points when the clock runs out, same range; points fall evenly from `points` to this. |
| `penalty` | number | `0` | Points taken for a wrong answer, 0 to 100000. |

### `explanation`

Explains the answer of a question. It explains the nearest question before
it in the same round (an `endOfRound` slide ends the search), and the screen
shows that question with its correct answer above `text`. With no question
before it in its round, only `text` and the slide's picture or video are
shown. Players do not answer; the next step goes to the next slide.

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `text` | text | `""` | Why the answer is right. Line breaks are kept. |

### `endOfRound`

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `title` | text | required | Shown above the leaderboard. |

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
| 2 | Adds `explanation` slides and the quiz `theme`. Version 1 files read as version 2 unchanged. |

Fields may be added within a version as long as older files keep their
meaning; readers ignore fields they do not know. Anything else (a new slide
type, a changed meaning) needs the next `schemaVersion` and a migration that
opens older files unchanged. Readers keep a quiz in the current version, so
saving an older file writes the current one.
