## What and why
<!-- What changes and why. Link the spec ID or `replica/features.csv` row. -->

## How it was verified
<!-- Paste the output. "Tests pass" without output is not evidence. -->
- [ ] `make check` passes (or the `buzzdome-verify` checks that apply, listed below)
- [ ] New or changed behaviour has tests that fail on the old code
- [ ] Rust types changed: the regenerated `app/src/bindings` are committed
- [ ] Screen change: seen in the preview in Czech and English; layout change: `make layout`
- [ ] Touches `app/e2e` or Windows-only code: a manual "Run workflow" of Build on this branch
- [ ] Hardware: steps added to `docs/hardware-test.md` for Jakub

```
<check output>
```

## Agent review
- [ ] `/review` run; CONFIRMED findings fixed or listed below
- Remaining PLAUSIBLE / minor findings:

## Risk
- [ ] Live show: persisted before shown, resumes after a crash, works without hardware
- [ ] Replays and quiz files: `RULES_VERSION` / `SCHEMA_VERSION` bumped and gated where needed
- [ ] No secrets, no weakened tests, no unexplained new dependencies, nothing from the reference app
