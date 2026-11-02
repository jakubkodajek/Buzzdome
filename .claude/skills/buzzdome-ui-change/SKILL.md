---
name: buzzdome-ui-change
description: Changes Buzzdome's React UI (Director, projector slides, quiz editor, themes, texts) following the app's one-way-of-doing-each-thing conventions. Use for any change in app/src.
---

# Change the UI

The UI in `app/src` is a view. It asks the backend and draws what it
gets. `CLAUDE.md` lists the conventions; this is the checklist.

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
   (`CLAUDE.md`, Rules). Czech addresses the quizmaster formally
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
4. **Shared helpers.** Use the existing one before writing a new one:
   `answers.ts`, `format.ts`, `quiz.ts`, `keyboard.ts`, `messages.ts`,
   `store.ts`, `theme/color.ts`.
5. **Slides** are drawn only by `host/SlideScreen.tsx` inside
   `host/Stage.tsx`. Stage CSS takes colours and fonts from `--theme-*`
   variables only; a slide's own style comes through `--style-*` variables
   (`theme/slide-style.ts`). Never write a colour a theme should decide.
6. **Editor.** A slide changes only through the pure functions in
   `editor/slides.ts`. Editor commands are `Action`s in the record built in
   `editor/EditorScreen.tsx`. Property rows come from
   `editor/properties.tsx` / `editor/style-properties.tsx`, built with
   `editor/row-builders.tsx`; `editor/property-pages.ts` decides which rows
   show.
7. **Dialogs and files.** Confirm with `confirm.ts`; files are picked by
   the backend (`dialogs.rs`), never by a page. ESLint refuses `alert`,
   `confirm`, `prompt` and reading the wall clock.
8. **Check it.** Add the state to a preview scene in
   `app/src/dev/preview.tsx` if none shows it, look at it in Czech and English,
   and run the `buzzdome-verify` skill.
