---
name: buzzdome-ui-change
description: Changes Buzzdome's React UI (Director, projector slides, quiz editor, themes, texts) following the app's one-way-of-doing-each-thing conventions. Use for any change in app/src.
---

# Change the UI

The UI in `app/src` is a view. It asks the backend and draws what it
gets. `AGENTS.md` lists the conventions; this is the checklist.

1. **Data.** Backend calls go through `api`, events through
   `BackendEvents` (`app/src/api.ts`); components use the hooks in
   `app/src/hooks.ts`. Shared types come from `app/src/types.ts` (generated
   bindings). Change a type in Rust, never in `app/src/bindings`.
2. **Show steps.** Buttons and keys offer only what `GameView.controls`
   allows, and send game actions through `useDispatch()`. Points and
   correctness come from the backend.
3. **Texts.** Every visible text is a key in `app/src/i18n.ts` with Czech
   (default) and English wording, read with `t` from `useApp()`. Backend
   codes are worded in `app/src/messages.ts`. No trademarked names
   (`AGENTS.md`, Rules). Czech addresses the quizmaster formally
   (vykání) and uses „…“, … and –; English is British (colour) and
   writes 50% and ID. One thing keeps one word:

   | English | Czech |
   | --- | --- |
   | Director | režie |
   | quizmaster | moderátor |
   | buzzer, big button | bzučák, velké tlačítko |
   | receiver, remote | přijímač, ovladač |
   | slide, projector | snímek, projektor |
   | everyone-answers question | otázka, na kterou odpovídají všichni |
   | fastest-finger question | otázka s rychlou odpovědí |
   | theme | motiv (téma is a quiz's topic) |
   | clock | časomíra |
   | ranking, final ranking | pořadí, konečné pořadí |
   | unsaved draft | rozpracovaný kvíz |
   | automatically | automaticky (not sám or samo) |

   The strings have no plural forms, so a Czech count follows a colon
   ("počet kroků: {count}") or "a ještě {n}".
4. **Messages.** Tell the quizmaster something with `notify` (green: done),
   `caution` (amber: done only in part, or not done though nothing broke)
   or `fail` (red: failed; also what `useReport()` shows), all from
   `useApp()`. They show as a toast in the Director (`director/Toasts.tsx`).
   What stays true for a while is a `NoticeBox` where it applies, and
   saving that does not work is the `NotSavedBanner`; both are in
   `notice.tsx`. Every message keeps that one plain shape: a small tone
   dot, the text, the detail under it (the not-saved banner is a red bar
   without the dot). No signs, tinted panels or coloured edges (`docs/design.md` §1.1, How messages look). The projector shows
   no messages; it reports them to the Director.
5. **Icons.** Use or add a line icon in `icons.tsx` (inline SVG drawn for
   this app, centred in its 24 px box), never a typed glyph such as `+`,
   `×`, `…` or `›`. An icon-only button centres its icon as a grid
   (`place-items: center`), with even padding or none.
6. **Shared helpers.** Use the existing one before writing a new one:
   `answers.ts`, `format.ts`, `quiz.ts`, `keyboard.ts`, `messages.ts`,
   `store.ts`, `theme/color.ts`.
7. **Slides** are drawn only by `host/SlideScreen.tsx` inside
   `host/Stage.tsx`. Stage CSS takes colours and fonts from `--theme-*`
   variables only; a slide's own style comes through `--style-*` variables
   (`theme/slide-style.ts`). Never write a colour a theme should decide.
8. **Editor.** A slide changes only through the pure functions in
   `editor/slides.ts`. Editor commands are `Action`s in the record built in
   `editor/EditorScreen.tsx`. Property rows come from
   `editor/properties.tsx` / `editor/style-properties.tsx`, built with
   `editor/row-builders.tsx`; `editor/property-pages.ts` decides which rows
   show.
9. **Dialogs and files.** Confirm with `confirmAction` (`confirm.ts`);
   files are picked by the backend (`dialogs.rs`), never by a page. ESLint
   refuses `alert`, `confirm`, `prompt`, the dialog plugin outside
   `confirm.ts`, and `Date.now` / `performance.now`: count on the show
   clock with `showClockNow` or `heardNow` (`media-player.ts`).
10. **Check it.** Add the state to a preview scene in
   `app/src/dev/preview.tsx` if none shows it, look at it in Czech and English,
   and run the `buzzdome-verify` skill. A part that can grow taller than the
   window scrolls in its own box (`overflow: auto`), so the bars stay put;
   the `buzzdome-layout` skill checks every window for that.
