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

## 2026-11-14 - audit-2026-11-14 (20-team readiness)
- Done (Jakub asked for a full audit and a 20-player test): six review
  agents (show, hardware, editor/import, security, every screen and button,
  bug hunt) and a verifier: nothing critical or major; findings and repros
  in `.work/audit-2026-11-14/` (this container only). Fixed: a buzzer of the
  receiver that joins once the slides began is named in the Director
  (`Game::joins_late`, `PressEvent.joinedLate`; Jakub chose "add and warn"),
  projector ranking and buzz queue and the Teams tab fit 20 long names
  (`longCrowd*` preview scenes, checked by buzzdome-layout), team files are
  read in Rust (`add_team_file`, `quiz_file::team_names`: UTF-16, quoted
  cells), on-screen buzzers reach the highest registered buzzer, vite/zod
  advisories, show-sim gives `TakeQuiz` and the crash drill resumes with
  edits. New E2E `app/e2e/scale.e2e.ts`: 24 teams on the fake receiver, 30
  questions, every round's presses in one write, a crash and resume
  (`SCALE_SEED` replays). Sheet-import findings went into PR #80.
- State: make check green; branch `claude/project-thread-0oi7d3`, stacked on
  PR #80.
- Next: merge #80, rebase, CI, merge, raise the version, Jakub tags.
- Blockers / open questions: hardware-only checks are `docs/hardware-test.md`
  section 8a (full-room rehearsal). Running E2E from a copy of the binary
  needs it under a `target/debug/` folder with a `.cargo-lock` file beside it
  and the `themes/` folder, or Tauri looks for resources in /usr/lib.

## 2026-11-14 - sheet-template-time-points (1.2.1)

- Done: the import template's first question has a 30 s answer time and
  its notes say the time is in seconds (empty = 20 s, 0 = no clock); the
  second has 20 points and its notes say points are what a team gets for a
  correct answer (empty = 10). File › Import's example shows both columns
  and two bullets explain them, with the defaults read from `Constants.ts`.
  Version raised to 1.2.1 (Jakub's OK).
- State: make check green; branch `claude/project-thread-28dmvy`.
- Next: guardian review, CI, merge, Jakub pushes tag `v1.2.1`.
