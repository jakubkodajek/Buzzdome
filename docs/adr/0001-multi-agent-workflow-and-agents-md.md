# ADR-0001: Multi-agent workflow, with AGENTS.md as the canonical instructions

- **Status:** Proposed
- **Date:** 2026-11-07
- **Deciders:** Jakub

## Context
Buzzdome is built mostly by AI coding agents. The instructions lived in
`CLAUDE.md`, with `AGENTS.md` pointing to it, an architecture page in
`.claude/ARCHITECTURE.md`, and task checklists in `.claude/skills/buzzdome-*`.
There was no shared way for agents to plan, test first, review each other
and keep memory between sessions, and nothing deterministic stopped an agent
from editing generated bindings or pushing a release tag.

## Options considered
1. **Keep the current files** - no churn; but no review loop, no memory, and
   other tools (Codex, Copilot) only get a pointer.
2. **Adopt `dev_template`'s workflow, tailored** - one canonical `AGENTS.md`,
   subagents with separate duties, find-verify-rank reviews, hooks, project
   memory in `memory/`; costs more tokens per feature.

## Decision
Option 2. `AGENTS.md` holds the shared instructions (moved from `CLAUDE.md`
unchanged in substance); `CLAUDE.md` imports it with the always-loaded memory.
Agents and skills are authored in `.claude/` and generated for Codex and
Copilot by `scripts/sync_agents.py`. The architecture page moves to
`docs/architecture.md`. The `buzzdome-*` skills stay the checklists that the
workflow agents follow.

## Consequences
- Positive: reviews by a fresh context with false-positive filtering;
  hooks are a speed bump against edits to bindings, lockfiles and manual
  copies and against pushing tags (CI, `.gitignore` and the `release`
  approval stay the real guards);
  memory survives sessions; one source for every tool.
- Negative: multi-agent runs cost many more tokens (Anthropic measured about
  15x for its research system), so `/feature` is for real features and small
  fixes use `/review` once. Generated copies must be re-synced (CI checks).
- Follow-ups: Jakub sets this ADR to Accepted or rejects it; optional Claude
  review on PRs needs the `ENABLE_CLAUDE_REVIEW` variable and an API key.

## Sources
- OpenAI Codex, AGENTS.md: https://developers.openai.com/codex/guides/agents-md
- Claude Code, memory and imports: https://code.claude.com/docs/en/memory
- Claude Code, hooks: https://code.claude.com/docs/en/hooks
- Anthropic, multi-agent research system: https://www.anthropic.com/engineering/multi-agent-research-system
