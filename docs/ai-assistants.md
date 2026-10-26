# Quizzes written by AI assistants

An AI assistant can write a Buzzdome quiz: questions, answers, points,
pictures, sounds and videos. It writes the quiz in the documented format
([`quiz-format.md`](quiz-format.md)) and checks it with the app's own rules,
and the quizmaster opens the result in the editor (File › Open), checks it
and changes what they like before the show (spec QZ-12).

There are two ways, depending on the assistant:

| Assistant | How | Media |
| --- | --- | --- |
| Claude Desktop (Windows, macOS) | The app is its MCP server: File › AI assistants › Connect Claude Desktop. | Taken from folders on the computer; Claude can look at pictures. |
| Claude, Gemini or ChatGPT in the browser or their apps | The skill package: File › AI assistants › Save the AI skill, then upload it to the assistant as a skill. | Uploaded to the chat, or pictures the assistant makes; videos and the rest are added in the editor afterwards. |

Why two: in October 2026 only Claude Desktop runs an MCP server on the
user's computer (as a `.mcpb` bundle or a configured command, Anthropic,
"Build a desktop extension with MCPB",
https://claude.com/docs/connectors/building/mcpb). ChatGPT adds MCP servers
only by URL over HTTP (OpenAI, "Custom MCP server",
https://developers.openai.com/api/docs/guides/custom-mcp-server), and so
does Gemini, in its web app and only in the US for now (Google, "Connect and
manage custom apps", https://support.google.com/gemini/answer/17209137). A
desktop app that works offline offers no such URL. Skills, a `SKILL.md` with
the files it uses, all three take: Claude on every plan with code execution
on (https://support.claude.com/en/articles/12512180), Gemini for personal
accounts, with scripts that may not use the internet
(https://support.google.com/gemini/answer/17094296), ChatGPT on Business,
Enterprise and Edu plans (https://help.openai.com/en/articles/20001066).

## Claude Desktop

1. Install Claude Desktop and start it once.
2. In Buzzdome, File › AI assistants › Connect Claude Desktop.
3. Close Claude Desktop and open it again. Its tools list now has
   Buzzdome.
4. Ask, for example: "Write a pub quiz of three rounds about Czech films
   with the pictures in C:\Quizzes\films, and save it as
   C:\Quizzes\Films.quiz."

Connecting adds the app to Claude Desktop's configuration file
(`claude_desktop_config.json`: `%APPDATA%\Claude\` on Windows,
`~/Library/Application Support/Claude/` on macOS) under `mcpServers`, as
the command `<path of Buzzdome> --mcp`; everything else in the file stays.
Claude Desktop starts that command when it starts and talks to it over
standard input and output. Started so, Buzzdome opens no window and does
nothing else: it does not touch a running show, and a show does not need
it. After Buzzdome moves (a reinstall to another folder), connect again.

The same entry by hand, for another assistant that runs local MCP servers:

```json
{ "mcpServers": { "buzzdome": { "command": "C:\\Program Files\\Buzzdome\\buzzdome.exe", "args": ["--mcp"] } } }
```

### Tools

All paths are absolute, since the server does not know the assistant's
working folder. A quiz is worked on as a folder, `quiz.json` with its media
in `media/`, which `save_quiz_file` packs into a `.quiz` file.

| Tool | What it does |
| --- | --- |
| `quiz_format` | The format guide and the JSON Schema of `quiz.json`. |
| `list_media_files` | The pictures, sounds and videos in a folder and the folders in it (4 deep, 1000 files). |
| `look_at_picture` | Shows the assistant a PNG, JPEG, GIF or WebP picture of up to 3 MB. |
| `add_media` | Copies files into a quiz folder and returns their names in the quiz (`media/...`). |
| `write_quiz` | Writes a whole quiz as the folder's `quiz.json` and lists its problems. |
| `read_quiz` | Returns a quiz folder's `quiz.json`. |
| `check_quiz` | Checks a quiz folder or `.quiz` file as the app does. |
| `save_quiz_file` | Makes the `.quiz` file; refuses a quiz with problems. |
| `unpack_quiz_file` | Unpacks a `.quiz` file into a new folder to change it. |

## Chat assistants: the skill

File › AI assistants › Save the AI skill writes `buzzdome-quiz.zip`:

```text
buzzdome-quiz/
├── SKILL.md                  when and how to write a quiz
├── reference/quiz-format.md  the format guide
├── reference/quiz.schema.json
└── scripts/make_quiz.py      checks a quiz folder and packs the .quiz file
```

Upload it as a skill: in Claude, Customize › Skills › Upload a skill; in
Gemini and ChatGPT, in their skills settings. Gemini wants `SKILL.md` in the
main folder of what is uploaded: if it refuses the ZIP, unpack it and upload
the `buzzdome-quiz` folder. The assistant then writes `quiz.json` in its sandbox, puts
the files the quizmaster uploaded into `media/`, and runs
`make_quiz.py`, which checks the quiz against the schema and its media and
writes the `.quiz` file to download. The script uses only Python's standard
library. It checks the quiz's shape, not every rule; the editor lists the
rest under File › Info when the file opens, and the show refuses a quiz
until they are fixed. A slide that needs a picture the assistant does not
have gets a note starting with "TODO:" instead.

Without code execution, the assistant writes `quiz.json` into the chat; the
quizmaster saves it in a folder with a `media` folder next to it and opens
that `quiz.json`.

## Command line

`buzzdome-quiz` (crate `quiz-ai`, `cargo run -p quiz-ai -- <command>`) does
the same for scripts and for assistants that run commands, such as Claude
Code:

```text
buzzdome-quiz check <folder or file>
buzzdome-quiz pack <folder> <file.quiz>
buzzdome-quiz unpack <file.quiz> <folder>
buzzdome-quiz schema
buzzdome-quiz skill <file.zip>
buzzdome-quiz mcp
```

It is not in the installer yet; the app's `--mcp` is.
