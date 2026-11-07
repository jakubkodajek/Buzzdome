---
name: bug-hunt
description: Adversarial bug hunt on a module, feature, or the whole app - the bug-hunter tries to break it with reproducible inputs, the verifier confirms, the test-engineer turns confirmed bugs into regression tests. Use when the user asks to find bugs, stress-test, or harden code.
argument-hint: "[path, feature, or area]"
---

# Bug hunt

Target: $ARGUMENTS

1. Slug `bughunt-<area>`; create `.work/<slug>/`.
2. Delegate to **bug-hunter** with the target and slug. Ask for at least 10 attack
   ideas and reproductions for every failing one. For game rules include a soak
   (`SOAK_SHOWS=5000 cargo test --release -p show-sim`) and crash drills.
3. Delegate to **verifier** to confirm each bug.
4. For each CONFIRMED bug delegate to **test-engineer** to add a failing regression test.
5. Report the confirmed bugs ranked by severity with their reproduction commands.
   Ask the user whether to fix them now (then hand off to **implementer**) or open issues.
