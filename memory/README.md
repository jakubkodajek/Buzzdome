# Project memory

Durable, version-controlled memory shared by humans and every AI agent. Agents start
each session with an empty context; this folder is how knowledge survives between
sessions, tools (Claude Code, Codex, Copilot) and people.

| File | What goes in | Who writes | Loaded |
| --- | --- | --- | --- |
| `project-context.md` | Product goals, users, constraints, glossary | Jakub, rarely changed | Every Claude session (imported by `CLAUDE.md`) |
| `lessons-learned.md` | Short imperative rules learned from mistakes | `docs-keeper`, humans | Every Claude session (imported) |
| `progress.md` | Append-only session log / handoff notes | `docs-keeper`, `handoff` skill | Read latest entries at session start |

## Rules

- Keep the memory files loaded every session short (target < 100 lines each;
  `AGENTS.md` holds the project's rules and is longer by design). Long instruction files
  reduce adherence (Claude Code docs, "How Claude remembers your project":
  https://code.claude.com/docs/en/memory).
- Facts that only matter for part of the code belong in `.claude/rules/` with `paths:`
  frontmatter, procedures belong in skills.
- Never store secrets or personal data here.
- Nothing here copies a fact that has a home elsewhere: decisions are ADRs in
  `docs/adr/` and the spec, what is built is `replica/features.csv`.
