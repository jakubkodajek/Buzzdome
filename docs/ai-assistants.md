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
app's data to `%LOCALAPPDATA%\Claude-Data\claude_pzs8sxrjxfjjc\`, where its
Settings › Developer › Edit Config now opens `claude_desktop_config.json`
(anthropics/claude-code#103860). Anthropic does not document it.

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
| `list_media_files` | The pictures, sounds and videos in a folder and the folders in it (4 deep, the nearest 1000 files; folders it cannot read are passed over). |
| `look_at_picture` | Shows the assistant a PNG, JPEG, GIF or WebP picture of up to 3 MB. |
| `add_media` | Copies files into a quiz folder and returns their names in the quiz (`media/...`). |
| `write_quiz` | Writes a whole quiz as the folder's `quiz.json` and lists its problems. |
| `read_quiz` | Returns a quiz folder's `quiz.json`. |
| `check_quiz` | Checks a quiz folder or `.quiz` file as the app does, without unpacking the file's media. |
| `save_quiz_file` | Makes the `.quiz` file; refuses a quiz with problems, and an existing file unless `replace` is given. |
| `unpack_quiz_file` | Unpacks a `.quiz` file into a new folder to change it. |

## After an update

The MCP server is the app itself, so an update brings it along. An AI app
keeps running the version it started until it is opened again: when one
is connected, the Director's update question says so.

From 0.2.0, which gave the server its folder on its command line, open
File › AI assistants once and connect each app again: the page takes over
the folder chosen then.
