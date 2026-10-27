---
name: buzzdome-backend-command
description: Adds or changes a Tauri command or backend event in Buzzdome, end to end from Rust to the React hook, with permissions, error codes and bindings. Use when the UI needs something new from the backend.
---

# Add a backend command

The UI never reads files, devices or the clock itself; it asks the Rust
shell. Every step below is needed, and tests check most of them.

1. **Type.** Define request and response types in Rust. Types that cross to
   the UI derive `serde` and `ts_rs::TS` with `#[ts(export)]`. Game types
   belong in `crates/quiz-core`; shell-only types in `app/src-tauri/src`.
2. **Command.** Write the `#[tauri::command]` in the module that owns the
   state (see the module list in `CLAUDE.md`). A command that writes a
   file, lists USB devices or takes the show state is `async` and runs off
   the windows' thread. Never hold a lock while the disk works; take locks
   in the order in `docs/design.md` §1.1 (Threads).
3. **Errors.** Return `CmdResult<T>`. Failures are `AppError` codes with
   parameters (`app/src-tauri/src/error.rs`), never an English sentence.
   Add the wording in Czech and English to `app/src/i18n.ts` and map the
   code in `app/src/messages.ts`.
4. **Register** the command in the same order in three places:
   `COMMANDS` in `app/src-tauri/build.rs`, `generate_handler!` in
   `app/src-tauri/src/lib.rs`, and `capabilities/director.json` (plus
   `capabilities/projector.json` only if the projector needs it). The test
   `the_windows_get_exactly_the_commands_there_are` keeps them in step.
5. **Show changes.** A command that changes a running show does not change
   it directly: it goes through the show thread queue as a `Command`, so it
   is journaled before it is shown. If it is a game action, it is a new
   `quiz_core::Command` handled by `Game::apply`, not a new Tauri command.
6. **UI.** Add one entry to `api` in `app/src/api.ts`, and an event to
   `BackendEvents` there if the backend pushes one. Components use it
   through a hook in `app/src/hooks.ts` and report errors with `useRun()` or
   `useReport()` (`app/src/app-context.tsx`).
7. **Preview.** Add the command to the mocked backend in
   `app/src/dev/preview.tsx` so the browser preview keeps working.
8. **Bindings and docs.** Run `cargo test --workspace` to write the
   bindings, then update `docs/design.md` (§1.1 event table, the module's
   section) and `CLAUDE.md` if a new module was added.

Then run the `buzzdome-verify` skill.
