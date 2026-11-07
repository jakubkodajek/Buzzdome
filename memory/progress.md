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

## 2026-11-07 - read-aloud-quality
- Done: the voice says numbers, dates, ordinals (gender from the next
  word, case from a preposition), Roman numerals, units, currencies and
  common abbreviations as words (`crates/voice/src/speech/`); the
  question, each answer's letter ("Á", "Bé") and each answer render as
  separate parts, trimmed, levelled and joined with pauses
  (`crates/voice/src/audio.rs`); 12 denoising steps; quiz setting
  `voiceSpeed` (80-120 %); question field `readAs` (editor Text page);
  `voice_clip` returns the spoken words for the system voice.
- State: make check green but for the bindings step before the commit;
  branch `claude/project-thread-s4llq8`. Checked with the real model and
  Whisper small as listener (design.md 1.8).
- Next: guardian review, Jakub listens on Windows, then merge with his OK.
- Blockers / open questions: Czech locative nouns in -e ("ve 2. kole")
  read as feminine; `readAs` fixes such cases. Spreadsheet import has no
  read-as column yet.
