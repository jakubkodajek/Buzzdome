---
paths:
  - "**/tests/**"
  - "**/*.test.ts"
  - "app/e2e/**"
  - "crates/show-sim/**"
---

# Testing rules

- One behaviour per test; the name states it (`wrong_buzz_passes_to_next_in_queue`).
- No wall clock, real network or unseeded randomness: pass `at_ms`, use the fakes
  (fake receiver, fake HID), fixed seeds (`SOAK_SEED`).
- Never delete, skip, `#[ignore]` or loosen an existing test or assertion to make a
  change pass; never change a shipped line in `crates/quiz-core/tests/journal.rs`.
- A bug fix starts with a regression test that fails on the old code.
- Windows E2E wipes the real app data: run it on CI only (`app/e2e/harness.ts`).
