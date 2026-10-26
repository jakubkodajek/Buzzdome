# Instructions for AI coding assistants

The project instructions are in [`CLAUDE.md`](CLAUDE.md). They apply to
every assistant, not only Claude: read that file first and follow its
rules. It lists the stack, the commands, the code layout and the UI
conventions, and the rules that must not be broken (clean room, no
trademarked names, live shows cannot fail).

Then read:

- [`.claude/ARCHITECTURE.md`](.claude/ARCHITECTURE.md): the architecture
  in one page, the invariants, and where to change what.
- [`.claude/skills/`](.claude/skills): checklists for common tasks, one
  folder each with a `SKILL.md`:

| Skill | Use it to |
| --- | --- |
| `buzzdome-verify` | run the local checks before a commit or PR |
| `buzzdome-backend-command` | add or change a Tauri command or backend event |
| `buzzdome-game-rules` | change game rules, scoring or the quiz file format |
| `buzzdome-ui-change` | change the Director, projector slides, editor or texts |
| `buzzdome-hardware` | work on the buzzers, the receiver or the stage lights |
| `buzzdome-quiz-authoring` | write or check a `.quiz` file from the repository |

Never commit the reference manual copies (`docs/` subfolders,
`replica/sources/`, `replica/screens/`, all gitignored) or any vendor SDK
file.
