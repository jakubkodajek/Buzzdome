# Quizzes written by AI assistants

An AI app on the quizmaster's computer can write a Buzzdome quiz:
questions, answers, points, pictures, sounds and videos. The app is the AI
app's MCP server (Model Context Protocol): its tools give the AI the
quizmaster's instructions and the documented format
([`quiz-format.md`](quiz-format.md)), find and show it the quizmaster's
media, and write, check and pack the quiz with the app's own rules. The
quizmaster opens the result in the editor (File › Open), checks it and
changes what they like before the show (spec QZ-12).

Connect an AI app once, then type a topic in its chat, such as "European
capitals": the instructions in the app say how to make the rest.

| AI app | How to connect |
| --- | --- |
| Claude Desktop (Windows, macOS; Linux in beta) | File › AI assistants › Connect next to Claude Desktop. |
| Antigravity | File › AI assistants › Connect next to Antigravity. |
| Codex (the ChatGPT desktop app, the Codex command line and its IDE extension) | File › AI assistants › Connect next to Codex. |
| Another app that runs local MCP servers | File › AI assistants › Copy settings, then add the `buzzdome` entry to its MCP servers (`mcpServers` in most apps, such as Cursor, https://cursor.com/docs/mcp; `servers` in VS Code's `mcp.json`, https://code.visualstudio.com/docs/agents/reference/mcp-configuration), keeping the servers already there. |

Assistants in a web browser cannot use it: ChatGPT adds MCP servers by URL
over HTTP or through OpenAI's Secure MCP Tunnel (OpenAI, "Custom MCP
server", https://developers.openai.com/api/docs/guides/custom-mcp-server),
and Gemini's web app by URL only (Google, "Connect and manage custom apps",
https://support.google.com/gemini/answer/17209137). A desktop app that works
offline offers neither.

## Connecting

1. Install the AI app and start it once.
2. In Buzzdome, open File › AI assistants. The AI works only in the folder
   shown there: the `Buzzdome` folder of your Documents, which the app
   makes, unless you choose another with *Change…*. Put the pictures,
   sounds and videos for your quizzes there.
3. Press *Connect* next to the AI app. Its row then says *Connected*.
4. Let the AI app read its new settings:
   - Claude Desktop reads them when it starts. On macOS quit it with ⌘Q;
     on Windows quit it from its icon by the clock (right click, Quit), as
     closing its window keeps it running; then open it again ("Connect to
     local MCP servers", https://modelcontextprotocol.io/docs/develop/connect-local-servers).
     With its Developer Mode on, Developer › Reload MCP Configuration does
     the same without a restart (https://claude.com/docs/connectors/building/mcp-apps/troubleshooting).
   - Antigravity: open Settings › Customizations and press Refresh under
     Installed MCP Servers (https://antigravity.google/docs/mcp), or open it
     again.
   - Codex: quit the ChatGPT app (on macOS ⌘Q) and open it again, or start a
     new Codex session in the command line or your editor. The Codex app is
     part of the ChatGPT desktop app since 2026-07-09
     (https://learn.chatgpt.com/docs/whats-new).
5. Type a topic in the AI app's chat. It makes the quiz in a new folder in
   the Buzzdome folder and saves the `.quiz` file there.

Connecting adds one entry, `buzzdome`, to the AI app's own settings file
and keeps everything else in it (in Codex's TOML file its comments too):
the app's path as the command, with `--mcp` as its only argument. The AI
app starts that command and talks to it over standard input and output.
Started so, Buzzdome opens no window and does nothing else, and a show
does not need it.

| AI app | Settings file | Entry |
| --- | --- | --- |
| Claude Desktop | macOS `~/Library/Application Support/Claude/claude_desktop_config.json`; Windows `%APPDATA%\Claude\claude_desktop_config.json`; Linux (in beta, https://code.claude.com/docs/en/desktop-linux) `~/.config/Claude/claude_desktop_config.json`, inferred: Anthropic names `~/.config/Claude` for its logs only (https://claude.com/docs/third-party/claude-desktop/configuration-format) | `mcpServers.buzzdome` |
| Antigravity | `~/.gemini/config/mcp_config.json` (https://antigravity.google/docs/mcp); on Windows `%USERPROFILE%\.gemini\config\mcp_config.json`, where `~` is the user's folder, which its documentation does not spell out | `mcpServers.buzzdome` |
| Codex | `~/.codex/config.toml`, on Windows `%USERPROFILE%\.codex\config.toml`, or in the folder `CODEX_HOME` names (https://learn.chatgpt.com/docs/extend/mcp, https://learn.chatgpt.com/docs/config-file/config-advanced) | `[mcp_servers.buzzdome]` |

Claude Desktop from the Windows installer at claude.ai/download is a
packaged (MSIX) app. It reads its own copy of its folder,
`%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\Claude\`
(Anthropic names it, https://claude.com/docs/cowork/hipaa-setup), whenever
the file is there (Microsoft, "Understanding how packaged desktop apps run
on Windows", https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-behind-the-scenes;
anthropics/claude-code#26073).

Users report that an update in October 2026 (3.1623) moved the packaged
app's data to `%LOCALAPPDATA%\Claude-Data\claude_pzs8sxrjxfjjc\`
(anthropics/claude-code#103860), and that its Settings › Developer › Edit
Config then opens `claude_desktop_config.json` there (PixelHabits/mx#13).
Anthropic does not document it.

So Buzzdome adds itself to every one of these files that Claude Desktop may
read, as it cannot tell which one it does.

Each app can be connected once it is installed and has been started once:
Buzzdome knows it by the folder it makes then (Claude Desktop's above,
`~/.gemini` for Antigravity, `~/.codex` or `CODEX_HOME` for Codex). The
settings file itself is made when it is not there yet. A settings file that
is a link to one kept elsewhere is changed where the link leads, and keeps
its permissions.

A row says *Needs connecting again* when the entry starts another program:
Buzzdome moved (a reinstall to another folder), or an earlier version
connected it with the folder on its command line. Connect it again. A
folder chosen in Buzzdome 0.2.0, which its entry names, becomes the page's
folder when the page opens, unless another was chosen since.

*Copy settings* gives the whole settings file with only this entry, with
the app's real path. Add its `buzzdome` entry to the other app's settings
rather than replacing them:

```json
{ "mcpServers": { "buzzdome": { "command": "C:\\Users\\Eva\\AppData\\Local\\Buzzdome\\buzzdome.exe", "args": ["--mcp"] } } }
```

## Your instructions for quizzes

The page's instructions say what a quiz looks like when the quizmaster
gives only a topic: how many rounds and questions, which kinds, time and
points, difficulty, and where to save it. The defaults are in the app's
language (`crates/quiz-ai/src/instructions/`) and make three rounds of six
questions; the quizmaster changes them in the page (up to 4000 characters)
and *Restore default* brings the defaults back. What they ask for in the
chat wins over them.

The app keeps them in `ai.json` in its data folder, with the folder the AI
works in. The server's `quiz_format` tool gives them first, before the
format guide and the schema, and reads them each time it is called, so a
change applies from the next quiz without restarting the AI app. When the
AI app connects, the server tells it to call `quiz_format` before making a
quiz and, given only a topic, to make the whole quiz by the instructions
without asking. The format's rules are not part of the instructions: the
tools check every quiz with the app's own rules whatever the instructions
say.

## What the assistant may touch

The tools read and write only inside the folder chosen in the app, which
the server reads from `ai.json` when it starts, as the reference
filesystem server touches only the folders on its command line
(https://github.com/modelcontextprotocol/servers/tree/main/src/filesystem).
The MCP specification leaves confirming each tool call to the assistant
and asks the server to validate its inputs and control access
(https://modelcontextprotocol.io/specification/2025-06-18/server/tools,
"Security Considerations"), so the server does not rely on the assistant
asking first:

- A path must be absolute and without `..`, and it counts by where it
  really leads, once links are followed: a path through a link that leads
  out of the folder is refused.
- In a quiz folder, `quiz.json`, the `media` folder and every media file
  the quiz names must stay in the folder: a link or junction there that
  leads out of it is refused, so no tool reads, packs or writes through
  one.
- Buzzdome's own data (the running show and its journal, editor drafts,
  settings, `ai.json` too) is never the folder: the app refuses a folder
  that holds it or is inside it, and so the assistant cannot change a
  show or its own folder and instructions, even by mistake.
- `write_quiz` and `add_media` write only into a new or empty folder, a
  quiz folder (one with a `quiz.json`), or one with only media; never
  among other files.
- `save_quiz_file` does not replace a file unless it is told to
  (`replace`), which the assistant is told to do only when the quizmaster
  asked.
- `look_at_picture` sends the assistant's model only a file that is a PNG,
  JPEG, GIF or WebP picture by its contents, not just by its name.

## Tools

All paths are absolute, since the server does not know the assistant's
working folder, and inside the folder. A quiz is worked on as a folder,
`quiz.json` with its media in `media/`, which `save_quiz_file` packs into
a `.quiz` file.

| Tool | What it does |
| --- | --- |
| `quiz_format` | The quizmaster's instructions, then the format guide and the JSON Schema of `quiz.json`. |
| `list_fonts` | The fonts installed on this computer, which a slide's or category's style can name. |
| `list_media_files` | The pictures, sounds and videos in a folder and the folders in it (4 deep, the nearest 1000 files; folders it cannot read are passed over). |
| `look_at_picture` | Shows the assistant a PNG, JPEG, GIF or WebP picture of up to 3 MB. |
| `add_media` | Copies files into a quiz folder and returns their names in the quiz (`media/...`). |
| `write_quiz` | Writes a whole quiz as the folder's `quiz.json` and lists its problems. |
| `change_quiz` | Changes a folder's `quiz.json` as Studio's editor does: adds, edits (a JSON merge patch, RFC 7386), deletes and moves slides by their number, and changes the quiz's title, theme, settings, sounds, welcome screen and categories; the same changes Studio's AI assistant makes (`quiz_ai::quiz_edits`). A slide given a category takes on its look, as in the editor. It says how many were made and left out, and lists the quiz's problems. |
| `read_quiz` | Returns a quiz folder's `quiz.json`. |
| `check_quiz` | Checks a quiz folder or `.quiz` file as the app does, without unpacking the file's media. |
| `save_quiz_file` | Makes the `.quiz` file; refuses a quiz with problems, and an existing file unless `replace` is given. |
| `unpack_quiz_file` | Unpacks a `.quiz` file into a new folder to change it. |

## Studio's AI assistant: a model on this computer

Studio has an AI assistant of its own (Home › AI assistant, spec QZ-14). It
asks a model that runs on the quizmaster's computer, through a server that
speaks the OpenAI chat completions API, with no account and no API key:

| Server | Address | Source |
| --- | --- | --- |
| Ollama | `http://localhost:11434/v1` (the default) | Ollama, "OpenAI compatibility", https://docs.ollama.com/api/openai-compatibility |
| LM Studio (start its server) | `http://localhost:1234/v1` | LM Studio, "OpenAI Compatibility Endpoints", https://lmstudio.ai/docs/developer/openai-compat |
| llama.cpp's `llama-server`, vLLM and others | their `/v1` address | |

File › AI assistants lists the server's models (`GET /models`) and keeps
the one chosen in `ai.json` (`localModel`). Signing in with a ChatGPT,
Claude or Gemini subscription is not offered: Anthropic and Google forbid
using those logins in other apps (The Register, 2026-02-20,
https://www.theregister.com/software/2026/02/20/anthropic-clarifies-ban-on-third-party-tool-access-to-claude/5014546;
Gemini CLI terms, https://geminicli.com/docs/resources/tos-privacy/), and
OpenAI's "Sign in with ChatGPT" is for its partners only (The New Stack,
https://thenewstack.io/sign-in-with-chatgpt/). Those subscriptions reach
Buzzdome through their own apps and the MCP server above.

Which model to use is the quizmaster's choice; the app and these docs
name none.

The panel is a chat (Jakub's choice, 2026-11-24). The quizmaster says
what quiz they want ("a quiz about Prague for a company party, 3 rounds of
5"), adds pictures, videos, sound recordings and text files (`.txt`,
`.md`, `.csv`; at most 12) with the + button or by dropping them on the
panel, and sends the message (Enter; Shift+Enter starts a new line). The
model replies with a proposal: rounds, their topics and numbers of
questions, a few example questions, and a question or two where something
is unclear (`editor_chat_local_model`, `quiz_ai::local_model::chat_request`:
the conversation so far, newest turns first within the text budget, then
the message with the files; plain text, streamed word by word to the
panel). The quizmaster refines it in more messages. The model sees the whole quiz
in every message (`quiz_edits::quiz_view`: the quiz's own fields as
`quiz.json` writes them, each slide on a line numbered from 1; within the
text budget the slides nearest the selected one in full, then in short,
and the farthest left out by their numbers; the themes and the fonts
installed on this computer), so it can talk
about changing what is there as well as about new questions. Make changes
(Ctrl+Enter) then has the model make the changes agreed in the
conversation: anything the editor can do (Jakub, 2026-11-24):

1. Pictures and videos are copied into the draft (`editor_add_dropped_media`),
   so the slides can show them.
2. The window makes each picture at most 768 px on its longest side and
   takes three frames of each video, both as JPEG data URLs. A video the
   window cannot play goes by its name only.
3. `editor_ask_local_model` sends one chat completion request
   (`quiz_ai::local_model::request`): instructions for a buzzer quiz in the
   quiz's language with a guide to the slide types and their fields and
   to the quiz's settings, sounds, welcome screen, categories and slide
   styles (`quiz_edits::format_guide`, from the quiz model's own JSON
   Schemas, so it follows the format), the quiz, the request, each file as
   "Attachment n" with its pictures, and a small JSON Schema of the reply
   (`quiz_edits::edits_schema`) as `response_format`, at a low temperature
   (0.2). The reply is a list of changes and a summary for the
   quizmaster: `add` (slides after slide N), `edit` (fields of slide N as a
   JSON merge patch, RFC 7386: `null` removes a field), `delete`, `move`
   and `quiz` (the quiz's own fields). A new question may be written with
   the fact that answers it (`reason`, kept in the notes) before the right
   answer and the wrong ones: stating the fact first is a chain of thought
   (Wei et al., "Chain-of-Thought Prompting Elicits Reasoning in Large
   Language Models", https://arxiv.org/abs/2201.11903); with gemma3:4b on a
   16 GB Mac, answers written first marked a wrong answer right in 1 or 2
   of 4 questions. A server that refuses the format is asked again
   without it. The reply is streamed, so the panel shows how many
   changes the model has begun. Cancel frees the panel at once; the
   request's connection closes as the server next writes, which stops the
   server too (while a model is still loading or reading a long request,
   it writes nothing for a while, and keeps the processor busy until it
   does). A show that begins cancels the request the same way and says so
   with a caution, as it does for a recording being written down.
4. The changes are read leniently (`quiz_edits::read_edits`: words such
   as `remove` or `insert`, numbers written as text) and resolved against
   the slides' ids (`quiz_edits::resolve`): every slide and value is read
   by the quiz model itself, so the model can write only what a quiz file
   holds, and a change it cannot read, or of a slide that is not there,
   is left out and counted. Slide numbers are those of the quiz the model
   saw. `right` and `wrong` become shuffled answers with the right one
   correct; `attachment:N` becomes the file copied into the draft (a
   picture, video or sound as what the file is); a theme must be one the
   app has; a new category gets an id. A second change of a slide or of
   the settings builds on the first, slides added at one place keep the
   order written, and a place after a slide deleted is after the slide
   before it. A slide keeps only files in the quiz's media folder, a
   picture or a video, and a category the quiz has. A test checks that the
   guide names every field of the quiz format (fonts, colours, fills,
   clocks, clip parts, sounds of each moment), so the model can set
   whatever the editor can.
5. The editor makes the changes as one undo step (`editor/ai-changes.ts`,
   a `batch` of its own edits): new slides get fresh ids and their
   category's look, the categories change first, and what was added is
   selected. A slide or field of the quiz the quizmaster changed while the
   model wrote keeps the quizmaster's change. The panel shows the model's
   summary and how many changes were made, left out and kept as the
   quizmaster had them. Changes asked for one quiz are not made in another
   opened meanwhile.

A request stays small (at most 16 pictures and 20,000 characters of
text), as the model reads it within its context: Ollama gives a model 4k
tokens on a computer with less than 24 GiB of graphics memory, and a longer
one is set in its app (Settings › Context length) or with
`OLLAMA_CONTEXT_LENGTH` (Ollama, "Context length",
https://docs.ollama.com/context-length). A request is refused during a show: a model takes the computer's processor
and memory for minutes. Small models (under about 7B parameters) may not
keep to the schema (LM Studio, "Structured Output",
https://lmstudio.ai/docs/developer/openai-compat/structured-output); the
reply is read also when the JSON is wrapped in a code block or a sentence
or follows a reasoning model's `<think>` block, and a field written oddly
(`null`, a number for a text) loses only its change. An answer given twice
or that is itself a question is dropped from `right` and `wrong`.
An error the server sends midway (a model out of memory) is shown as such,
and a model that does not stop is stopped after 400,000 characters. The
address must be `http` or `https` with no user name or password, and
redirects are not followed. Text files are read as UTF-8, as UTF-16 with a
byte order mark, or else as Windows-1250 (Czech Excel's CSV).
Ollama takes pictures but no video (Ollama, "Ollama's new engine for
multimodal models", https://registry.ollama.ai/blog/multimodal-models), hence
the frames.

A request that does not fit (`asking::write_changes`) is asked once
more with its texts and what was found on the internet cut to 4,000
characters (`SMALL_TEXT_CHARS`) and with `reasoning_effort: "none"`,
which Ollama turns into thinking off for a model that can turn it off
(Ollama, "OpenAI compatibility",
https://docs.ollama.com/api/openai-compatibility). It does not fit when
the server says it overflows the model's context (Ollama, llama.cpp, LM
Studio and vLLM each say so in their own words, `context_full`), when a
reasoning model reasons up to its limit without answering, or when the
reply is cut off while the texts were longer than that: in Ollama's
default 4k tokens, the request, the reasoning and the reply share the
context. If the second request fails too, the panel says so (raise the
context length). The research plan is always asked for with
`reasoning_effort: "none"`: it is a few search queries, and a reasoning
model otherwise spent 97 to 157 s on it in our test. Not every model
can stop reasoning: in our test on 2026-11-23 a vision model of 8B
parameters in Ollama 4k context reasoned with "none" too (and with the
native API's `think: false`), until its limit at every step; such a model
needs a longer context. A server that
refuses that field, or the JSON Schema, is asked again without them. A
reasoning model's reasoning, which Ollama, llama.cpp and LM Studio stream
apart from the answer (`reasoning`, `reasoning_content`), shows as "the
model is thinking"; one that reasons up to its limit without answering is
told apart from a cut-off reply. The instructions keep questions off counting things in a picture
(small models miscount: 2 of 12 answers wrong in our test with a 4B model,
both counts) and off registration and phone numbers.

### Sound

The local servers' models read no sound, so the app writes down what is
said in a sound recording, and in a video's sound track, before it asks:
offline, with Whisper large-v3-turbo (Radford et al., "Robust Speech
Recognition via Large-Scale Weak Supervision",
https://arxiv.org/abs/2212.04356) as k2-fsa exported it for sherpa-onnx
(`voice::LISTENER`, release `asr-models`, 564 MB, checked by its SHA-256 as
the reading voice is). The panel offers the download the first time a
recording or video is added; it lives in the app's local data
(`listener/`). The window decodes the sound (Web Audio `decodeAudioData`)
to 16 kHz mono, at most 15 minutes, and sends it as raw samples to
`editor_transcribe`, which writes it down in a process of its own
(`buzzdome --listen <folder> <language>`, `app/src-tauri/src/listen.rs`)
in pieces of at most 28 s cut at the quietest moment (Whisper hears 30 s
at once). The words go to the model as text with the attachment; music
without speech is said to be such. In a 4-core container, 5.2 s of Czech
speech took 2.0 s, the model loaded in 1.8 s, and the sentence came back
word for word (`crates/voice/tests/listen.rs`, with `LISTENER_MODEL_ROOT`
and `VOICE_MODEL_ROOT`: the reading voice says it, the listener writes it).

### The internet

With "Look up on the internet" ticked (the default), the request may name
a company, a person or an event, and the app looks it up before the model
writes. No address is needed:

1. The model is asked, in a short request with the prompt and the files'
   names only, for at most 3 web searches (`research_plan_request`), each
   with the subject's whole name and town. A model that gives none (it
   reasoned past its limit, say) has the prompt itself searched.
2. Addresses written in the prompt are read, with two of their pages that
   say most about a company (about us, history, products, contact;
   `links_to_follow`). The model's guesses at an address are not used: a
   guessed domain led to another company of the same name.
3. Each search goes to DuckDuckGo's HTML page, which needs no key (Open
   WebUI's web search uses it the same way), and its two best results are
   read, at most 6 pages in all. Legal pages (privacy, cookies, terms) are
   passed over, and a page is read once however its address is written.
4. The search results and the pages' text, without menus repeated from
   page to page, go to the model after the files, at most 14,000
   characters, each page its share.

Pages on this computer or the local network are read only when the request
names their address. A search result, a link or a redirect cannot lead
there: not by an address (`web_page::may_read`: loopback, private,
link-local and the like, a name without a dot, `.local`, `.lan`), and not
by a public name that resolves to such an address (`InternetOnly` in
`app/src-tauri/src/local_model.rs` checks the addresses it connects to).
Behind a proxy the proxy looks names up, so only the first check applies.

The panel shows each step and lists the pages read with the questions; a
search or page that failed is said so. With it unticked, nothing leaves
the computer.

### Ollama's own API

Ollama's OpenAI-compatible API cannot set a model's context length (Ollama,
"OpenAI compatibility", https://docs.ollama.com/api/openai-compatibility),
and Ollama's default is 4096 tokens on a computer with less than 24 GiB of
graphics memory (https://docs.ollama.com/context-length). A reasoning model
reasons for thousands of tokens before it answers, so in 4k it ran out of
room at every step in our test (2026-11-23), even when asked not to reason
(`reasoning_effort: "none"`, and `think: false` in Ollama's own API: the
model reasoned anyway). So when `/api/version` at the server's address
without `/v1` answers as Ollama does, the panel talks to Ollama's own chat
API (`/api/chat`, Ollama, "Chat", https://docs.ollama.com/api/chat;
`quiz_ai::local_model::for_ollama`) and sets `num_ctx` to 16,384 tokens, or
32,768 for a long request, never more than the model's own context
(`/api/show`, `model_info.<architecture>.context_length`). The same value
for most requests, as Ollama loads a model again for another. Other servers
keep the OpenAI API and the context they were started with. A request that
would not fit that context is asked again shorter, as Ollama would
otherwise cut its start (the instructions) without an error
(`local_model::ollama_fits`); a computer without the memory for 16,384
tokens ("requires more system memory") is asked again with 8,192.

The conversation, the files' text and what was found share one text
budget, the conversation up to half of it, newest turns first. Turns are
sent with alternating roles, as strict chat templates refuse anything else:
files sent without words count as a turn of the quizmaster's, and a last
message never answered (questions asked for) joins the next. A turn cut off
at the model's length limit is shown as it is, marked, not asked again.

The research plan stops a model that reasons over it for more than 3,000
characters (`asking::Reading::PLAN`): then nothing is looked up for that
message in the conversation, and Make changes searches the quizmaster's
messages as they are written. What was found during the conversation goes
back with each message and with Make changes (`notes`, the newest 14,000
characters), so the changes are written from the same pages as the
proposal. Page text in the notes can try to steer the model (a prompt
injection) for the rest of the conversation; the model has no tools, reads
only sites the quizmaster typed and searches only DuckDuckGo, and
everything it writes is checked as slides, so New conversation clears it.

### Checking it with a real model

The tests use a fake server on this computer
(`app/src-tauri/src/local_model.rs`). With Ollama running and a model that
reads pictures pulled (`ollama pull <model>`; which one is the user's
choice), one test asks the real model too:

```sh
BUZZDOME_LIVE_MODEL=<model> cargo test -p buzzdome live_model -- --nocapture
```

`BUZZDOME_LIVE_URL` gives another server (`http://localhost:1234/v1` for LM
Studio). Without `BUZZDOME_LIVE_MODEL` the test does nothing. By hand, in
`npm run tauri dev`:

1. File › AI assistants: Load models lists Ollama's models; choose one and Use.
2. In an empty quiz, Home › AI assistant: add two photos, a short video and
   a `.txt` file with +, write "8 questions in two rounds" and send it; the
   model proposes the rounds. Ask for a change, then press Make changes.
   The status says the model reads, then counts changes; the slides come
   in after the selected one: sections, questions with the photos and the
   video, an end of round after each round and the results.
3. Undo (Ctrl+Z) takes all of them out as one change. Then ask for "give
   questions 2 to 4 20 seconds and delete the last slide", "move the
   second round to the start", "use another theme" and "rename the quiz":
   each is made as one undo step.
4. Ask again and press Cancel while the model reads: the panel is free at
   once and Make changes works again.
5. Quit Ollama and ask: the panel says the server does not answer, with its
   address. Choose a model name that is not pulled: it says so.
6. Start a show while a request runs: the request is cancelled, and a
   caution says that the show stopped the panel.

## After an update

The MCP server is the app itself, so an update brings it along. An AI app
keeps running the version it started until it is opened again: when one
is connected, the Director's update question says so.

From 0.2.0, which gave the server its folder on its command line, open
File › AI assistants once and connect each app again: the page takes over
the folder chosen then.
