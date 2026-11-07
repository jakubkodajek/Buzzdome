---
name: review
description: Multi-agent review of the current branch or a PR - parallel specialist reviewers find issues, an independent verifier filters false positives, results are deduplicated and ranked. Use when asked to review code, a diff, a branch, or a PR.
argument-hint: "[PR number | branch | diff range] (default: current branch vs origin/main)"
---

# Find → Verify → Rank review

Target: $ARGUMENTS (if empty: `git diff --merge-base origin/main`).

1. **Scope.** Determine the diff and a `<task-slug>` (`review-<branch>`). Create
   `.work/<task-slug>/`. List changed files and classify them: `quiz-core` rules,
   shell (`app/src-tauri`), drivers (`crates/buzzers`, `crates/lights`), files and
   formats (`crates/quiz-file`, `crates/quiz-ai`), UI (`app/src`), tests, CI/config, docs.
2. **Find (parallel).** In one message launch: `code-reviewer`, `bug-hunter`, plus
   `security-reviewer` and/or `ux-ui-reviewer` if relevant files changed. Give each
   the slug, the diff range, and `REVIEW.md`.
3. **Verify.** Launch `verifier` on all findings files.
4. **Rank & report.** Present only CONFIRMED and PLAUSIBLE findings, most severe
   first, each with `file:line`, failure scenario and suggested fix. State how many
   were REJECTED so the user sees the filter worked.
5. If the target is a PR and the user asked for comments, post the CONFIRMED findings
   as inline comments with `gh` (one comment per finding). Never approve or merge.
