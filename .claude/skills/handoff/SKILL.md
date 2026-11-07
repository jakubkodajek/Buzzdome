---
name: handoff
description: End-of-session handoff - make the working tree clean, record progress and lessons in memory/ (decisions as ADRs) so the next session (human or agent) can continue without context. Use when the user says they are done, before context runs out, or after finishing a task.
---

# Session handoff

1. Run `git status` and `make check` (or the `buzzdome-verify` checks that apply). If the tree is dirty, either commit work that
   passes or describe exactly what is unfinished.
2. Delegate to **docs-keeper** with: what was done this session, the task slugs used,
   the final `make check` result, and open questions.
3. Make sure the new `memory/progress.md` entry answers: what changed, current state
   (green/red), the next concrete step, and blockers.
4. Show the user the progress entry and the list of changed memory files.
