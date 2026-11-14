---
name: buzzdome-layout
description: Checks every Buzzdome window (Director tabs, quiz editor, projector scenes) for broken layout: a window that scrolls as a whole, bars that leave their edge or cover content, controls drawn under others or out of reach. Use after any change to app/src layout or CSS, before a release, or when a screen looks misplaced.
---

# Check the layout of every window

The Director, the editor and the projector are fixed frames: bars stay on
their edges (the Director's tabs on top, its status bar at the bottom, the
command panel on the left; the editor's ribbon and status bar), and only a
part inside scrolls. When a part grows taller than the window without its
own scroll box, the whole window scrolls instead: the status bar ends up in
the middle of the window over the content and the command panel stops
short. `check-layout.mjs` finds that and similar faults in the browser
preview.

## Run it

1. `cd app && npm ci` (once), then `npm run dev` and leave it running.
2. From the repository root:
   `node .claude/skills/buzzdome-layout/check-layout.mjs --shots <scratch dir>`
   (`--only director|editor|host` and `--lang cs|en` narrow it, `--url`
   points at another dev server). It needs Playwright: the cloud container
   has it; elsewhere `npm i -g playwright && npx playwright install chromium`.
   A full run takes about five minutes.
3. It opens each Director tab with no show, in the lobby, while a
   question runs (also with 20 long team names, and a long buzz queue), light and dark, in Czech and English; each ribbon tab and
   properties page of the editor; and every projector scene of
   `app/src/dev/preview.tsx` (new scenes are picked up). Each at
   1024x640, 1366x768 and 1920x1080. It exits 1 and prints one line per
   problem. Look at the screenshots of anything it reports, and of the
   screens you changed, even when it reports nothing.

## What it reports, and the usual fix

| report | what is wrong | fix |
| --- | --- | --- |
| `page-scroll` | the window itself scrolls | the part that grows gets its own scroll box: `overflow: auto` on the tab's root (as `.receiver`, `.lights`, `.settings`), or `flex: 1; min-height: 0` down the flex chain to the element that scrolls. A row of buttons that is too wide wraps (`flex-wrap: wrap`) |
| `bar` | a bar is not on its edge or not fully on screen | as for `page-scroll`; a new bar goes in `BARS` in the script |
| `covered` | a control is drawn under another element | a bar or panel overlaps content: find what grows past its box. Toasts and popups float on purpose and are skipped |
| `off-screen` | a control lies outside the window with no scroll box to reach it | as for `page-scroll` |

Never hide the problem with `overflow: hidden` on the window or a bar:
content would be cut off where the quizmaster cannot reach it.

## Keep it working

- A new tab, ribbon tab or properties page is found by its `role="tab"`.
- A new kind of window or screen (a dialog that is its own page, a new
  editor mode) gets a block in the script's main loop.
- A new projector state gets a preview scene (`buzzdome-ui-change`, step
  10), which the script then checks.
- This script is the one browser check kept in the repository; other
  Playwright scripts stay outside it (`buzzdome-verify`, step 7).
