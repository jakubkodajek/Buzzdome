---
name: buzzdome-verify
description: Runs Buzzdome's local checks (Rust tests and lints, UI lint, type-check, unit tests, E2E) before a commit or pull request, and says what each one catches. Use when asked to verify, test or check a change, or before opening a PR.
---

# Verify a Buzzdome change

Run the checks from the repository root. CI (`.github/workflows/build.yml`,
`e2e.yml`) runs the same ones; GitHub Actions minutes are limited, so a PR
should be green locally first.

## Setup (once per machine or container)

- Rust comes from `rust-toolchain.toml` (rustup fetches it). On Jakub's
  Mac, put `/opt/homebrew/opt/rustup/bin` on `PATH` first.
- Node.js 22. Install the UI as locked: `cd app && npm ci`.
- Linux needs the system libraries in `.github/actions/linux-deps/action.yml`
  (WebKitGTK, `libudev-dev`, `libasound2-dev`, ...). E2E also needs
  `tauri-driver`, `webkit2gtk-driver`, `xvfb` and `xdotool`.
- A clean `target/` takes about 12 GB. If the disk fills, delete
  `target/debug/{deps,build,incremental,.fingerprint}`.

## Checks, in order

1. `cargo test --workspace`
   Rust tests, 40 random shows (`show-sim`), 40 crash drills. It also
   rewrites `app/src/bindings` from the Rust types. Commit the bindings it
   changed; CI fails when the committed ones are stale.
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cd app && npm run lint` (ESLint with the React hooks rules and the
   app's own bans: no `alert`/`confirm`, no wall-clock reads in the UI).
4. `cd app && npm run build` (type-check and production build).
5. `cd app && npm test` (vitest: UI logic).
6. When the change touches a window, a show flow, files or the receiver:
   `cd app && npm run tauri build -- --debug --no-bundle && xvfb-run -a npm run e2e`.
   Linux only; Windows E2E runs on CI only, because it wipes the real app data.
7. For a screen change, look at it in the browser preview:
   `cd app && npm run dev`, then
   `/preview.html?window=<director|host>&scene=<name>` (scenes in
   `app/src/dev/preview.tsx`). Keep any Playwright scripts outside the repo.

Deeper runs when the game core changed:
`SOAK_SHOWS=5000 cargo test --release -p show-sim`. A failing show prints
`SOAK_SEED=<n>`; replay it alone with that variable.

## Report

Say which checks ran and passed, with counts (tests, E2E), and name any
check you skipped and why. Never skip, disable or loosen a test to get green.
