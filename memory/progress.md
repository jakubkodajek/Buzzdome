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
  The spreadsheet carries it as a `READ-AS` column. Version 1.1.0.
- State: make check green; branch `claude/project-thread-s4llq8`, PR #79,
  merged with main after #78; guardian BF181-BF186 fixed. Checked with the
  real model and Whisper small as listener (design.md 1.8).
- Next: merge with Jakub's OK (given for 1.1.0); Jakub pushes the tag and
  listens on Windows.
- Blockers / open questions: Czech locative nouns in -e ("ve 2. kole")
  read as feminine; `readAs` fixes such cases.

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

## 2026-11-10 - simpler-sheet-import
- Done: spreadsheet columns have full-word headings in English and Czech
  (`Question`/`Otázka`, `Answer 1`/`Odpověď 1`...) next to the reference's
  short ones; an import knows any of them whatever their case, accents,
  spaces and dashes (`text::heading_key`), and notes name a column as the
  sheet does. Export and the template write the headings in the app's
  language (`editor_export_sheet` takes `language`), leave the answer time,
  points and penalty empty at their defaults, and write a `{Q, EA}`/`{Q, FF}`
  prefix only where the row would read otherwise. Import help, template
  notes and docs/quiz-format.md say only the question and answers are needed.
- State: make check green; branch `claude/project-thread-28dmvy`.
- Next: Jakub's review of the PR, then his OK to merge.
- Blockers / open questions: none.

## 2026-11-14 - sheet-import-new-format-only
- Done (Jakub's OK of the plan): the import reads only the English and Czech
  headings; the reference's short headings (`Q`, `A1`...), `{B}`/`{Q, FF}`
  prefixes, `{L}`, `||` and the "unsupported column" list are gone (their
  `ImportIssue` codes too). A Type/Typ column (Section, Explanation, End of
  round, Results, Question, Fastest finger, either language) replaces the
  prefixes; an unknown type is noted (`unknownType`). Export and template
  write the basic columns (Type to Points) and the others only when the
  quiz uses them. File › Import shows a short hint, then a guide with an
  example sheet whose headings and types come from Rust
  (`sheet::sheet_words`, `SHEET_WORDS` in Constants.ts).
- State: make check green; branch `claude/project-thread-28dmvy`, PR #80.
- Next: guardian review of the new commit, then Jakub's OK to merge.
- Blockers / open questions: none.
