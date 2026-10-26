---
name: buzzdome-game-rules
description: Changes Buzzdome's game rules, scoring, step flow or the .quiz file format safely, keeping old shows replayable and old quiz files readable. Use for any change in crates/quiz-core or the quiz format.
---

# Change a game rule or the quiz format

`crates/quiz-core` is the only place that knows the rules. Two things must
keep working after any change: a journal written by an older version
replays exactly as it was played, and a quiz file written by an older
version (or by a colleague or an AI assistant) still opens.

## Before you start

Read the rule in `replica/recon.md` ("Scoring rules worth getting right")
and its spec ID in `docs/specification.md`. Behaviour follows the
reference app unless Jakub decided otherwise (`ROADMAP.md`, the spec).

## A rule or step change (`game.rs`, `view.rs`)

1. Change `Game::apply`. Keep `Game::controls` in step: the UI offers
   only the commands `controls` allows, and a test over a whole show
   checks `controls` against `apply`.
2. If the change alters what replaying an existing journal produces, bump
   `RULES_VERSION` in `game.rs` and gate the old behaviour on
   `self.rules_version`, so a resumed old show plays on under its rules.
3. Values the UI shows (points, correctness, places) are computed here and
   sent in `GameView`; never recompute them in TypeScript.
4. Constants the UI needs go to `bindings/Constants.ts` through the
   quiz-core test that writes it; never copy a number into the UI.
5. Tests: unit tests next to the code, `crates/quiz-core/tests/game_flow.rs`
   for a whole show, and teach `crates/show-sim` any new command or slide
   kind so the random shows cover it.

## A quiz format change (`model.rs`)

1. Prefer an optional field with a default that keeps today's behaviour,
   and leave it out of the JSON when it holds the default.
2. A change older apps cannot read safely bumps `SCHEMA_VERSION` and
   upgrades older files when they are read (`Quiz::from_json`).
3. Update `docs/quiz-format.md`. The JSON Schema for AI assistants is
   generated from the model (`quiz_ai::quiz_schema`); check that the skill
   text in `crates/quiz-ai/src/skill/SKILL.md` still says what is true.
4. Check `samples/demo-quiz.json` still opens, and the spreadsheet import
   and export (`quiz-core/src/sheet.rs`, `quiz-file/src/sheet.rs`) if the
   field belongs in a sheet.

## After

Update `docs/specification.md` / `docs/design.md` where the rule is
described, set the row in `replica/features.csv`, and run the
`buzzdome-verify` skill including a longer soak:
`SOAK_SHOWS=5000 cargo test --release -p show-sim`.
