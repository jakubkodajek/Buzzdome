---
paths:
  - "app/src/**/*.ts"
  - "app/src/**/*.tsx"
  - "app/src/**/*.css"
---

# UI rules (app/src)

Follow the `buzzdome-ui-change` skill; the short version:

- Backend through `api` / `BackendEvents` (`api.ts`) and the hooks in `hooks.ts`;
  game actions through `useDispatch()`; offer only `GameView.controls`.
- Never edit `app/src/bindings` (ts-rs output); change the Rust type.
- Every visible text is a key in `i18n.ts` in Czech and English; backend codes are
  worded in `messages.ts`. No trademarked names.
- Messages are `notify` / `caution` / `fail` from `useApp()`; icons come from
  `icons.tsx`, never typed glyphs.
- Stage CSS takes colours and fonts only from `--theme-*` variables; a slide's own
  style (`--style-*`, from `theme/slide-style.ts`) is read only by `host/slide.css`.
- No wall-clock reads (`Date.now`, `performance.now`) and no `alert` / `confirm`:
  ESLint refuses them; use the show clock and `confirmAction`.
- A part that can grow scrolls in its own box; never `overflow: hidden` on a window
  or bar (`buzzdome-layout`).
