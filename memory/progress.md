# Progress log

Append-only. Newest entry at the bottom. Read the last 2-3 entries at session start.

Template:

```
## YYYY-MM-DD - <task-slug>
- Done: what changed (commits)
- State: make check green/red; branch name
- Next: the single next concrete step
- Blockers / open questions:
```

## 2026-11-07 - agent-workflow
- Done: added the multi-agent setup from `dev_template`, tailored to Buzzdome:
  `AGENTS.md` canonical (content moved from `CLAUDE.md`), `CLAUDE.md` imports it
  and the memory; 9 subagents and 6 workflow skills in `.claude/` next to the
  `buzzdome-*` skills; hooks (dangerous commands, release tags, secrets,
  lockfiles, generated bindings, manual copies); `.claude/rules/`; `memory/`;
  `Makefile`; `REVIEW.md`; `docs/agent-workflow.md`; `.claude/ARCHITECTURE.md`
  moved to `docs/architecture.md`; `scripts/sync_agents.py` with Codex and
  Copilot copies; `.github/workflows/agents.yml` (agent config only, path-filtered);
  optional Claude review and `@claude` workflows, off until enabled.
- State: no application code changed; branch `claude/agent-workflow`.
- Next: Jakub reviews the PR and confirms the acceptance criteria in
  `memory/features.json` (drafted from `ROADMAP.md`).
- Blockers / open questions: ADR-0001 is Proposed until Jakub accepts it.

## 2026-11-07 - resume-quiz-edits
- Done: a resumed show can take its quiz file as saved since the show began
  (Jakub's choice): `Quiz::shape_change`, journaled `Command::TakeQuiz` with the
  whole quiz, `SessionSummary.edited`, `resume_last_session(takeEdits)`,
  `Session::rewrite_meta` for the new media folder; the Director asks With the
  edits / Without / Cancel when Resume is pressed. BF173: the restart warning
  only once the recovery is journaled (`resume_session` test with a broken journal).
- State: make check green; branch `claude/project-thread-u9fky8`, PR #78.
  Guardian BF178 (journaled quiz read through `Quiz::read_value`, migrated)
  and BF179 (the scored question on screen keeps its slide as played,
  `editedSlideKept`) fixed; BF180 this line.
- Next: guardian review of the PR, then Jakub's OK to merge.
- Blockers / open questions: none.
