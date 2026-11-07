---
name: buzzdome-verify
description: Runs Buzzdome's local checks (Rust tests and lints, UI lint, type-check, unit tests, E2E) before a commit or pull request, and says what each one catches. Use when asked to verify, test or check a change, or before opening a PR.
---

# Verify a Buzzdome change

Run the checks from the repository root. CI (`.github/workflows/build.yml`,
`e2e.yml`) runs the same ones; GitHub Actions minutes are limited, so a PR
should be green locally first. A PR's own runs skip the Windows E2E tests
(they run on main and on "Run workflow"), so a PR that touches `app/e2e`
or Windows-only code also needs a manual "Run workflow" of Build on its
branch before merging. Push first: a push cancels that branch's running
manual Build.

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

`make check` (`Makefile`) runs steps 1 to 5 as CI's check job does
(`build.yml`): the agent-config check (`python3 scripts/sync_agents.py --check`),
the voice libraries, `npm ci`, then the steps, with the UI built before any
Rust is compiled, and the bindings check last.

Rust builds link the sherpa-onnx libraries (`crates/voice`). Before the
first build, run `.github/scripts/sherpa-onnx-libs.sh .` and export
`SHERPA_ONNX_ARCHIVE_DIR` as the folder it names (`docs/releasing.md`,
The voice libraries); without it the build downloads unchecked archives,
or fails behind a proxy.

1. `cd app && npm run lint` (ESLint with the React hooks rules and the
   app's own bans: no `alert`/`confirm`, no wall-clock reads in the UI).
2. `cd app && npm test` (vitest: UI logic).
3. `cd app && npm run build` (type-check and production build; Tauri
   embeds it, so Rust needs it first).
4. `cargo clippy --workspace --all-targets -- -D warnings`
5. `cargo test --workspace`
   Rust tests, 40 random shows (`show-sim`), 40 crash drills. It also
   rewrites `app/src/bindings` from the Rust types. Commit the bindings it
   changed; CI fails when the committed ones are stale.
6. When the change touches a window, a show flow, files or the receiver:
   `cd app && npm run tauri build -- --debug --no-bundle && xvfb-run -a npm run e2e`.
   Linux only; Windows E2E runs on CI only, because it wipes the real app data.
7. For a screen change, look at it in the browser preview:
   `cd app && npm run dev`, then
   `/preview.html?window=<director|host>&scene=<name>` (scenes in
   `app/src/dev/preview.tsx`). For a layout or CSS change, also run the
   `buzzdome-layout` skill's check of every window. Keep any other
   Playwright scripts outside the repo.

Deeper runs when the game core changed:
`SOAK_SHOWS=5000 cargo test --release -p show-sim`. A failing show prints
`SOAK_SEED=<n>`; replay it alone with that variable.

## Report

Say which checks ran and passed, with counts (tests, E2E), and name any
check you skipped and why. Never skip, disable or loosen a test to get green.
