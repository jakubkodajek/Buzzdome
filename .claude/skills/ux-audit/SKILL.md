---
name: ux-audit
description: Audit Buzzdome's windows (Director, quiz editor, projector scenes) in the browser preview for the quizmaster's flow, projector readability, layout, keyboard use and copy, with screenshots as evidence. Use when the user asks for a UX, UI, design or accessibility review.
argument-hint: "[window, scene or flow names] (default: every window in the preview)"
---

# UX / UI audit

Target: $ARGUMENTS

1. Slug `ux-<area>`; create `.work/<slug>/screens/`.
2. Start the preview (`make preview`, port 1420). If no browser tool is available,
   tell the user how to add Playwright MCP (see `docs/agent-workflow.md#tools`).
   Run the `buzzdome-layout` check for the windows in scope.
3. Delegate to **ux-ui-reviewer** with the windows and scenes, the flows to test,
   and the slug.
4. Delegate to **verifier** to drop findings that are not reproducible in the browser.
5. Report findings grouped by: blockers (the quizmaster cannot finish a step),
   projector readability, layout, consistency with the app's conventions, copy (Czech
   and English). Each with screenshot path and suggested fix.
