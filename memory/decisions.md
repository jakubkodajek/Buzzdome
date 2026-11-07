# Decision log

One line per decision. Significant or hard-to-reverse decisions get an ADR in
`docs/adr/`; the reasoning behind most design choices is in `docs/design.md`.
Format: `- YYYY-MM-DD: <decision> - <reason> [ADR-NNNN | source]`

Collected on 2026-11-07 from `AGENTS.md`, `ROADMAP.md`, `docs/specification.md`,
`docs/design.md` and `replica/hardware.md`; newest last.

- 2026-10-05: Scope changed from a browser app with phones to a desktop app with hardware buzzers, Windows first, Czech + English - the event laptop runs Windows and players use buzzers only [ROADMAP.md, Phase 1]
- (date not recorded): Stack is Tauri 2 with a Rust core that owns all state and a React + TypeScript UI [ROADMAP.md, Phase 2]
- 2026-10-09: The app is a private copy for Jakub, not open source and not distributed; Enjoy is asked for the SDK for personal use [docs/specification.md, open questions; replica/hardware.md]
- 2026-10-10: The app is called Buzzdome; bundle id `cz.kodajek.quizshow` and `quiz-show.*` keys keep the working title so saved data stays put [AGENTS.md, Product shape]
- 2026-10-20: Enjoy's protocol may also be read from an unofficial SDK copy for this private app; no file or code from it enters the repository [AGENTS.md, Rules]
- 2026-10-21: Jakub's set is the Enjoy 303S; the RF215 dome driver was dropped [docs/specification.md; ROADMAP.md M2]
- 2026-10-21: In fastest-finger questions with one to four answers the answer buttons buzz; otherwise the big button [docs/specification.md BZ-19]
- 2026-10-25: A receiver whose protocol is known is taken automatically when plugged in, also mid-show [docs/specification.md BZ-20]
- 2026-10-26: The quizmaster's remote drives the show, each key's action chosen in the Receiver tab [docs/specification.md BZ-21]
- 2026-11-02: The quizmaster picks each light's colour for RGB buzzers; black turns a buzzer off; ready must stay distinct [docs/specification.md BZ-22]
- 2026-11-02: Messages to the quizmaster keep one minimal plain shape [docs/design.md §1.1]
- 2026-11-06: Every fastest-finger buzz waits its turn in a queue; a wrong answer passes to the next team (rules version 8) [docs/specification.md BZ-23]
- 2026-11-06: No licence text or notice is shown for the voice model (private, non-commercial app) [docs/design.md, reading aloud]
- 2026-11-06: The clock runs while a question is read aloud, unless configured otherwise [docs/design.md, reading aloud]
- 2026-11-07: In the buzz queue the answering team lights white, waiting teams yellow, wrong teams red [docs/specification.md BZ-23, table 4.3]
- 2026-11-07: Adopt the multi-agent workflow from `dev_template`; `AGENTS.md` becomes the canonical agent instructions, `CLAUDE.md` imports it [ADR-0001, proposed]
