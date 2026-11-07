---
name: adr
description: Write an Architecture Decision Record for a technical choice (framework, database, API style, infra) with context, options, decision and consequences. Use when a significant or hard-to-reverse technical decision is made or proposed.
argument-hint: "[decision title]"
---

# Architecture Decision Record

Decision: $ARGUMENTS

1. Find the next number: `ls docs/adr/ | sort | tail -1`.
2. Copy `docs/adr/template.md` to `docs/adr/NNNN-<kebab-title>.md`.
3. Fill in context, at least two considered options with pros/cons, the decision,
   and consequences. Cite sources (docs, benchmarks) for factual claims.
4. Status starts as `Proposed`; only the user sets `Accepted`.
5. Add a one-line entry to `memory/decisions.md` linking the ADR. If the decision
   changes behaviour, also update `docs/design.md` where the choice is described.
