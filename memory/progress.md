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

## 2026-11-23 - windows-display-scale

- Done: a colleague's Lenovo laptop showed the app differently. The
  Director opens at 1280x800 logical pixels, which Windows' display scale
  enlarges past the work area of a full HD laptop at 150 % (about
  1280x690 above the taskbar) or an HD laptop at 100 %: its status bar sat
  under the taskbar or off the screen. `app_windows::fit_on_screen`
  maximizes a Director (and a projector window that is not full screen)
  larger than its screen's work area, and centres one that fits but sticks
  out. The buzzdome-layout check now runs at the sizes a maximized window
  has on Windows, at their display scale: laptops (960x600 up, full HD at
  125 % and 150 %), full HD, QHD, 4K and ultrawide monitors, 4:3 and 16:10
  projectors (14 sizes): no layout problems in any window.
- State: make check green; branch `claude/project-thread-tuspp5`.
- Next: Jakub's colleague confirms on the laptop (a screenshot if it still
  differs); the UI font is Segoe UI on Windows and the system font on
  macOS, so Windows and a Mac never look exactly alike.

## 2026-11-23 - text-limits (QZ-13)

- Done (Jakub's screenshot: a pasted question ran off the slide): each
  quiz text has a longest length (`quiz_core::text_limits`, `TextField`,
  `TEXT_LIMITS` in Constants.ts): question 250, answer 80, slide title 80,
  information/explanation text 400, quiz title and welcome heading 80,
  welcome footer 120, read-as 500, notes 2000. Measured in the preview: at
  these lengths six answers with a picture still fit at the fit-text floor.
  `TextTooLong` / `AnswerTooLong` keep a quiz from being saved and the AI
  tools from packing it, never from playing; the JSON Schema has
  `maxLength`; editor fields take no more and the properties panel counts
  characters from 80 %. Information and explanation slides now use
  `useFitText` too. File › AI assistants shows each app's model and company
  (`Assistant::maker`).
- State: make check green; branch `claude/project-thread-dv8265`.
- Next: guardian review, Jakub's OK to merge.
- Blockers / open questions: none.

## 2026-11-23 - local-ai-panel (QZ-14)

- Done (Jakub chose "local AI only", no API keys, no subscription logins:
  Anthropic and Google forbid them in other apps, OpenAI's is partner-only):
  Studio's AI assistant (Home › AI assistant, `editor/AiPanel.tsx`) takes
  pictures, videos and text files and a request, and asks a model on this
  computer through an OpenAI-compatible server (Ollama, LM Studio). The
  window shrinks pictures and takes video frames (canvas, `ai-files.ts`;
  CSP media-src now allows `blob:`); `editor_add_dropped_media` copies the
  media into the draft; `editor_ask_local_model` streams the reply with
  progress and a real cancel (`app/src-tauri/src/local_model.rs`); the
  questions become sheet rows read by `quiz_core::import_tables`
  (`quiz_ai::local_model::draft_table`) and are inserted like a sheet
  import. The model is chosen in File › AI assistants (`ai.json`
  `localModel`). Refused during a show.
- State: branch `claude/project-thread-5u62lr`; not tried against a real
  model (the container cannot reach Ollama's registry or Hugging Face).
- Next: Jakub tries it with Ollama on his Mac or Windows (a vision model
  such as qwen3-vl:8b or gemma3, context 16k); review; merge only with his OK.
- Blockers / open questions: Ollama's default 4k context on < 24 GiB VRAM
  limits how much a request may carry (limits in `local_model.rs`).

## 2026-11-23 - release-1.3.0

- Done: review of #83, #84 and #85 in fresh contexts (code, security, UX)
  before the release; #85 merged as it was, #83 and #84 through this
  branch. Fixed on #84: page fetches refuse a public name that resolves
  to a local address (`InternetOnly`, wider `web_page::is_local_address`);
  the unpacking mark is a count, so the voice and the speech recognition
  model can unpack at once without a show starting; `decode_text` keeps a
  Windows-1250 letter at a whole file's end (`decode_text_start` for the
  start of a file); a cancelled request is not sent; the panel shows a
  failure by its buttons; the AI assistant is spec QZ-14 (QZ-13 is the
  text lengths). Version raised to 1.3.0 (Jakub's OK).
- State: make check green on main + #85 + #83 + #84 with the fixes;
  layout check clean at 960x600, 1280x640, 1366x690 and 1920x1000; branch
  `claude/project-thread-il2cug`.
- Next: Jakub pushes tag `v1.3.0`.
- Blockers / open questions: left for later, none blocks the release: a
  very long recording is decoded whole before it is cut to 15 minutes
  (`ai-files.ts` `soundOf`); the panel's pinned footer is tall at
  1024x700; `maxLength` in `reply_schema` (BF212) was left out, as no real
  model could be asked with it here; the layout script does not open the
  AI panel; no test that a show's start cancels the panel's request.

## 2026-11-24 - ai-panel-chat (release 1.4.0)

- Done: Studio's AI panel (QZ-14) is a chat: talk, a proposal, refine it,
  then Make questions from the conversation (PR #88). One + button for all
  files; files sent move into the message's bubble; replies drawn with
  paragraphs, lists and bold, a copy button, Stop while the model works.
  Ollama is asked through its own `/api/chat` with `num_ctx` 16k/32k
  (8k when short of memory), so reasoning models (qwen3-vl) have room.
  Two review rounds (fresh context, then code and security) fixed: role
  alternation, one text budget, newest notes kept, local redirects only
  to addresses typed in the message, chat reply cap. Version 1.4.0
  (Jakub's OK, 2026-11-24).
- State: make check green; layout check clean (editor).
- Next: Jakub pushes tag `v1.4.0`; try qwen3-vl through Ollama on his Mac
  and on the event laptop (memory for 16k context).
- Blockers / open questions: pictures are sent with every turn and not
  dropped on the shorter retry; no UI test of the panel's run/cancel flow.

## 2026-11-24 - ai-quiz-changes (release 1.4.0)

- Done: the AI panel and the MCP server change anything the editor can
  (Jakub: "uplne vsechno jako v editoru", also fonts and looks). The
  model sees the whole quiz (`quiz_edits::quiz_view`) and replies with
  changes (add, edit as a JSON merge patch, delete, move, quiz fields);
  `quiz_edits::resolve` checks them with the quiz model and the editor
  applies them as one undo step (`editor/ai-changes.ts`). MCP gained
  `change_quiz` (same changes) and `list_fonts`. A test checks the guide
  names every field of the quiz format. Review findings CR-1..CR-11
  fixed (edits build on each other, insert order, deleted anchors, view
  budget, correct answers by text, stale snapshot kept as the
  quizmaster's, media and category checks).
- State: make check green before the fix commit; layout check clean (editor).
- Next: Jakub pushes tag `v1.4.0`; try "change" requests (time, theme,
  fonts, move a round) with a real model on his Mac.
- Blockers / open questions: MCP cannot measure loudness (gainDb is set
  blind), transcribe recordings, or render a slide; it works on quiz
  folders, not on the draft open in Studio.
