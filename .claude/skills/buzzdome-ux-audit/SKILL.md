---
name: buzzdome-ux-audit
description: Hunts the whole Buzzdome UI (Director, quiz editor, projector) for UX details such as missing state feedback, wrong disabled states, keys that do nothing, unsafe one-click actions and inconsistent wording, with parallel read-only agents, then fixes the findings. Use before a release, when the owner asks to "find everything in the UI", or after a large UI change.
---

# Audit the UI for UX details

The owner notices details: Save that stays enabled after saving, a button
whose label does not say what it will do, a key that silently does nothing
on a Czech keyboard. This skill finds such details across the whole UI and
gets them fixed. It complements `buzzdome-layout` (geometry: bars, scroll
boxes, overlap) and follows `buzzdome-ui-change` for every fix.

## 1. Audit with parallel read-only agents

Start a Vite dev server once (`cd app && npm ci && npm run dev`) and give
every agent the same brief (below), its own area, an ID prefix, its own
scratch folder and its own findings file. Agents read the code, check the
preview with Playwright, and **edit nothing in the repository**.

| prefix | area |
| --- | --- |
| EF | editor file flow: start dialog, Backstage pages, save / open / close, draft, test run and back |
| ES | editor slide editing: ribbon, thumbnails, canvas, properties, table and category views, pickers, dialogs, context menus, editor keys |
| DS | running a show: command panel, Questions / Responses / Results / Teams tabs, virtual buzzers, launch check, toasts, show keys, start screen |
| DT | Director setup tabs: Settings, Receiver, buzzer colours, Lights, Media player, sound board, voices |
| X | across the UI: cs/en wording and the word table, icon buttons' title and aria-label, async buttons and double clicks, `confirmAction` use, projector, window titles |

What every agent looks for:

- **State feedback.** After an action, does the UI show it happened (saved,
  applied, copied, running, failed)? Is a toggle's state visible (pressed
  style, a label that says what a click will do)? Is a dirty state marked?
- **Disabled states.** A control that would do nothing, or harm, in this
  state is disabled (and says why where that is not obvious); a control
  that works is not disabled. In a show this comes from `GameView.controls`.
- **Keys.** Shortcuts shown in tooltips and menus work; Esc cancels, Enter
  confirms; keys use `event.code` where the layout matters (digits on a
  Czech layout); Delete or Backspace never act on a slide while a control
  has the focus; dialogs keep and give back the focus.
- **Safety.** A step that cannot be undone asks first, with a question that
  names what is lost; nothing destructive sits one slip away from a
  harmless control; unsaved work is never dropped without a question.
- **Words.** One word per thing in Czech and English (`buzzdome-ui-change`
  word table), "…" on buttons that open a dialog or picker, no typed glyphs
  as icons, units on numbers.
- **Live use.** What would confuse a quizmaster at an event: a label that
  does not say what Next will do now, a slot whose button changes under the
  mouse, a preview that is not what the projector shows.

Each finding: `### <prefix><n>: <title>`, then Severity (high: misleads
live or risks data; medium; low: polish), Where (file:line, scene), What
the quizmaster sees, Why it matters, Proposed fix (in the repo's
conventions), Effort (S/M/L). Agents verify each finding in the code (and
a screenshot where it is visual) and drop what they cannot confirm. Keep
the findings files outside the repository (project files or scratch).

## 2. Fix

- Triage: everything high and medium, and the cheap low ones. A change of
  game rules (journal replay, `RULES_VERSION`) is the owner's decision.
- Fix in parallel only with disjoint file ownership (one agent per area, in
  its own git worktree, UI files only); new i18n keys go next to related
  keys, not at the end of a section, so the branches merge cleanly. Rust
  changes and wording that spans areas stay with the main thread.
- Put finding IDs in commit messages, never in code or test names.
- Update E2E tests that rely on changed labels or new confirms.

## 3. Check

Run `buzzdome-verify` and `buzzdome-layout`, and look at every changed
screen in Czech and English.
