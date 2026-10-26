---
name: buzzdome-quiz-authoring
description: Writes, checks, packs or unpacks Buzzdome .quiz files from this repository with the buzzdome-quiz command line. Use when asked to create, change or validate a quiz while working in the Buzzdome repo.
---

# Write a quiz from the repository

The app ships its own skill for AI assistants that write quizzes. Its
text is the source of truth and is not repeated here:
[`crates/quiz-ai/src/skill/SKILL.md`](../../../crates/quiz-ai/src/skill/SKILL.md)
(its `{{...}}` placeholders are filled from quiz-core's limits when the
package is built). Read it, then follow it with the command line instead
of its Python script:

```sh
cargo run -p quiz-ai -- schema                       # the quiz JSON Schema
cargo run -p quiz-ai -- check <folder or file>       # check a quiz folder or .quiz
cargo run -p quiz-ai -- pack <folder> <file.quiz> [--replace]
cargo run -p quiz-ai -- unpack <file.quiz> <folder>
cargo run -p quiz-ai -- skill <file.zip>             # build the skill package for chat assistants
```

- The format is `docs/quiz-format.md`; `samples/demo-quiz.json` is a
  complete example.
- `check` runs the app's own rules, so a quiz it accepts opens in the app.
- Name only media files you really have, as `media/<file name>`; leave a
  `TODO:` note on a slide that still needs one.
- Never copy questions, texts or media from the reference app.

Quizmasters who are not in the repository use the app instead: File › AI
assistants (Connect Claude Desktop, or Save the AI skill for Claude, Gemini
or ChatGPT). See `docs/ai-assistants.md`.
