# Releasing

An installed app updates itself. When the Director opens with no show
running, it asks the update server whether a newer version is out and offers
to install it (`app/src-tauri/src/updates.rs`, `UpdateBanner`). The update
server is a public GitHub repository holding releases only,
`jakubkodajek/buzzdome-updates`: each release carries the installers and a
`latest.json` that names the version, its notes and, for each platform, the
file and its signature. The app reads
`https://github.com/jakubkodajek/buzzdome-updates/releases/latest/download/latest.json`
(`plugins.updater.endpoints` in `tauri.conf.json`).

The releases are built there, by its Release workflow, from this repository's
code: GitHub Actions minutes are free on a public repository and scarce on
this private one ([GitHub docs](https://docs.github.com/en/billing/concepts/product-billing/github-actions)).
The updates repository holds no source code, but its releases and build logs
are public: anyone who finds it can download the installers.

## Publish a new version

1. Raise `version` in `app/src-tauri/tauri.conf.json` (`0.1.0` to `0.2.0`,
   say) and merge it to `main`. The app offers only a version higher than
   its own.
2. In `buzzdome-updates`: Actions > Release > Run workflow. `ref` is the
   branch, tag or commit to build (`main`); `notes` say what changed, in
   Czech: the quizmaster reads them before installing.
3. The workflow refuses a version that is not higher than the latest
   release, builds the Windows installer, then the macOS one (one after the
   other, as each adds its platforms to the release's `latest.json`),
   checks that `latest.json` names every platform and only then publishes
   the release. From then on each app offers it the next time the Director
   opens without a show.

The release page also serves a first install:
`Buzzdome_<version>_x64-setup.exe` for Windows, `Buzzdome_<version>_universal.dmg`
for macOS.

## What the app checks

- The download is signed with the release key: the app verifies it against
  the public key in `plugins.updater.pubkey` and installs nothing that does
  not match ([Tauri docs](https://v2.tauri.app/plugin/updater/)).
- The signature also names the version it was signed for, and it must be
  the version `latest.json` announces (`requireSignedVersion`), so an old
  release cannot be passed off as a new one.
- The update server is reached over HTTPS only.
- Nothing installs while a show runs, as installing ends the app; the show
  state stays locked while the new version goes in, so no show can begin
  meanwhile. On Windows the installer runs in its passive mode (a progress
  bar, no questions) and starts the new version; on macOS the app replaces
  itself and starts again.
- Debug builds (`npm run tauri dev`, the E2E tests) never ask the server.

A release build merges `app/src-tauri/tauri.release.conf.json` into the
configuration, which makes the bundler sign the updater files. Every other
build (`npm run tauri build`, the installers job in `build.yml`) needs no key.

## One-time setup

1. **Release key.** On your own computer, in `app/`:
   `npm run tauri signer generate -- -w ~/.tauri/buzzdome.key`, with a
   password. Keep the key file and its password safe and private: without
   them no installed app can be updated again, and anyone who has them can
   sign an update the apps accept. Put the public key (the content of
   `~/.tauri/buzzdome.key.pub`) in `plugins.updater.pubkey` in
   `app/src-tauri/tauri.conf.json`.
2. **The updates repository.** Create the public repository
   `jakubkodajek/buzzdome-updates` with its `.github/workflows/release.yml`.
3. **Read access to this repository.** A fine-grained personal access token
   (GitHub > Settings > Developer settings) with access to
   `jakubkodajek/QuizXpress` only and the Contents permission read-only. It
   expires; renew it before then.
4. **Secrets** of `buzzdome-updates` (Settings > Secrets and variables >
   Actions): `SOURCE_TOKEN` (the token), `TAURI_SIGNING_PRIVATE_KEY` (the
   content of `~/.tauri/buzzdome.key`) and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

A changed key needs a manual reinstall on every computer, since the apps
there only accept the old one.
