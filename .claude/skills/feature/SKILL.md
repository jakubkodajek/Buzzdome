---
name: feature
description: Run the full multi-agent pipeline for one feature - plan, test-first, implement, parallel review (code, security, UX, bug-hunt), independent verification, fix loop, handoff. Use when the user asks to build or change a feature.
argument-hint: "[feature description or features.json id]"
---

# Feature pipeline

Task: $ARGUMENTS

You are the **orchestrator**. You coordinate subagents and keep the main context
small: delegate heavy reading to subagents and keep only their reports.
Full protocol and the handoff report format: `docs/agent-workflow.md`.

## 0. Set up
- Pick a short kebab-case `<task-slug>`. Create `.work/<task-slug>/`.
- If the task matches an item in `memory/features.json`, use that id; otherwise add
  a new item with `"passes": false`. Note its spec ID and `ROADMAP.md` line.
- Create a branch: `git switch -c feat/<task-slug>` (cloud sessions use the branch
  they were given).

## 1. Plan
Delegate to **planner** with: the slug, the task description, the spec ID and the
`buzzdome-*` skill that likely applies.
Read the returned plan. If it lists open questions that change the design, ask the
user before continuing.

## 2. Tests first
Delegate to **test-engineer** with the slug. Confirm the report shows new tests
failing for the right reason.

## 3. Implement
Delegate to **implementer** with the slug. Require `make check` output (or the
`buzzdome-verify` checks that apply) in the report.

## 4. Review in parallel
Launch these in parallel (one message, multiple subagent calls), each with the slug
and the diff range:
- **code-reviewer** (always)
- **bug-hunter** (always)
- **security-reviewer** if the change touches files, ZIPs, child processes, Tauri
  commands or capabilities, the MCP server, updates, downloads, dependencies or CI
- **ux-ui-reviewer** if the change touches `app/src`

## 5. Verify
Delegate to **verifier** with the slug. It writes `verified-findings.md`.

## 6. Fix loop (evaluator-optimizer, max 3 rounds)
If there are CONFIRMED findings with severity critical/major:
1. **test-engineer** writes regression tests for confirmed bugs.
2. **implementer** fixes the CONFIRMED findings.
3. Re-run step 4 only for the reviewers whose findings were fixed, then step 5.
Stop after 3 rounds and escalate remaining findings to the user.

## 7. Finish
- Run `make check` yourself and show the output. Hardware-only criteria become a
  section in `docs/hardware-test.md` for Jakub (`buzzdome-hardware` skill).
- Delegate to **docs-keeper** with the slug and all reports.
- Summarise for the user: what was built, evidence, remaining PLAUSIBLE/minor findings,
  and suggested PR title. Open the PR only if the user asked for it. Never merge, bump
  the version or push a tag.
