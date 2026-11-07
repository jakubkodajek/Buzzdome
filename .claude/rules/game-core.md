---
paths:
  - "crates/quiz-core/**"
  - "crates/show-sim/**"
---

# Game core rules (quiz-core)

Follow the `buzzdome-game-rules` skill; the short version:

- No UI or I/O dependencies in `quiz-core`. Rules, points, correctness, `controls`
  and `following` live here and nowhere else.
- A change in how an existing journal replays bumps `RULES_VERSION` and gates the
  old behaviour on `self.rules_version`.
- A quiz format change is optional-with-default first; otherwise bump
  `SCHEMA_VERSION` and upgrade old files in `Quiz::from_json`. Update
  `docs/quiz-format.md`.
- A new `Command` gets a journal line in `tests/journal.rs`; shipped lines never change.
- Teach `show-sim` every new command or slide kind.
