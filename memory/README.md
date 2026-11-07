# Project memory

Durable, version-controlled memory shared by humans and every AI agent. Agents start
each session with an empty context; this folder is how knowledge survives between
sessions, tools (Claude Code, Codex, Copilot) and people.

| File | What goes in | Who writes | Loaded |
| --- | --- | --- | --- |
| `project-context.md` | Product goals, users, constraints, glossary | Jakub, rarely changed | Every Claude session (imported by `CLAUDE.md`) |
| `lessons-learned.md` | Short imperative rules learned from mistakes | `docs-keeper`, humans | Every Claude session (imported) |
| `decisions.md` | One-line log of decisions, links to ADRs | `docs-keeper`, `adr` skill | On demand |
| `progress.md` | Append-only session log / handoff notes | `docs-keeper`, `handoff` skill | Read latest entries at session start |
| `features.json` | Open work toward the next event, with a `passes` flag (the full matrix is `replica/features.csv`) | Jakub adds items; agents only flip `passes` | On demand |

## Rules

- Keep always-loaded files short (target < 100 lines each). Long instruction files
  reduce adherence (Claude Code docs, "How Claude remembers your project":
  https://code.claude.com/docs/en/memory).
- Facts that only matter for part of the code belong in `.claude/rules/` with `paths:`
  frontmatter, procedures belong in skills.
- Never store secrets or personal data here.
- `features.json` is JSON on purpose: Anthropic found models are less likely to
  inappropriately rewrite JSON than Markdown task lists
  (https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents).
