# Releasing

An installed app updates itself. When the Director opens, it asks whether
a newer version is out, and offers to install it while no show runs
(`app/src-tauri/src/updates.rs`, `UpdateBanner`). The AI tools come with it
(see below). Versions are published as
GitHub releases of this repository: each carries the installers and a
`latest.json` that names the version, its notes and, for each platform, the
file and its signature. The app reads
`https://github.com/jakubkodajek/Buzzdome/releases/latest/download/latest.json`
(`plugins.updater.endpoints` in `tauri.conf.json`), which always points at
the release marked latest; drafts and pre-releases are never offered.

The releases are built by `.github/workflows/release.yml` when a version tag
is pushed. The repository is public, so its releases are too: anyone who
finds them can download the installers.

## Publish a new version

1. Raise `version` in `app/src-tauri/tauri.conf.json` (`0.1.0` to `0.2.0`,
   say), and to the same in `Cargo.toml` (`[workspace.package]`) and
   `app/package.json`; `cargo build` and `npm install` (in `app/`) carry it
   into `Cargo.lock` and `app/package-lock.json`. Merge it to `main`. The
   app offers only a version higher than its own. Wait until the Build workflow's `check` and `windows-tests` have
   passed on that commit.
2. Tag that commit of main with the version, with what changed as the tag's
   message: the quizmaster reads it before installing, in the app's
   language. Write it in Czech, then a line `[en]` alone, then the same in
   English (each `-m` is a paragraph):

   ```sh
   git tag -a v0.2.0 -m "Opravy po akci" -m "- Rychlejší start" \
     -m "[en]" -m "Fixes after the event" -m "- Faster start" origin/main
   git push origin v0.2.0
   ```

   An app in Czech shows the part before `[en]`, one in English the part
   after it (`ReleaseNotes` in `app/src-tauri/src/updates.rs`). Notes in
   one language only (no `[en]`, or nothing after it) are shown in both,
   and the Release workflow warns that the English part is missing. Apps of
   version 0.7.0 and older show the message from its start, `[en]` line
   and English included, up to its first 1,000 characters
   (`MAX_NOTES_CHARS`). The
   GitHub release page shows the message as it is.

   Make the tag with git, not on GitHub's Releases page: the workflow makes
   the release itself.
3. Approve the run: GitHub mails you that the Release workflow waits for
   the `release` environment. In the run (Actions > Release), Review
   deployments > `release` > Approve and deploy. Approve only a run of a
   tag you pushed yourself: the approval is what lets a run use the
   release key.
4. The Release workflow refuses a tag that does not name the version the
   commit declares, a commit that is not on main or whose checks have not
   passed, and a version that is not higher than every release. It builds
   the Windows installer and the macOS app into a draft release, then checks
   each update the way the installed apps will: signed by the key whose
   public half is in that commit's `tauri.conf.json`, for the version it
   declares. Only then does it write `latest.json` and publish the release.
   From then on each app offers it the next time the Director opens without
   a show. A run that failed can be run again from the Actions tab ("Re-run
   all jobs"); it starts from a fresh draft.

The latest installers for a first install always have the same address:
`https://github.com/jakubkodajek/Buzzdome/releases/latest/download/Buzzdome_x64-setup.exe`
for Windows and `.../Buzzdome_universal.dmg` for macOS (Apple silicon and
Intel).

## What the app checks

- The download is signed with the release key: the app verifies it against
  the public key in `plugins.updater.pubkey` and installs nothing that does
  not match ([Tauri docs](https://v2.tauri.app/plugin/updater/)).
- The signature also names the version it was signed for, and it must be
  the version `latest.json` announces (`requireSignedVersion`), so an old
  release cannot be passed off as a new one.
- The update server is reached over HTTPS only.
- Nothing installs while a show runs, as installing ends the app; the show
  state is marked as updating while the new version goes in, so no show
  can begin meanwhile. A show that begins during the download puts the update off:
  the Director offers it again once the show ends. On Windows the installer runs in its passive mode (a progress
  bar, no questions) and starts the new version; on macOS the app replaces
  itself and starts again.
- Debug builds (`npm run tauri dev`, the E2E tests) never ask the server.

## The AI tools

The MCP server that `docs/ai-assistants.md` describes is the app itself
(`buzzdome --mcp`), which a connected AI app starts, so after an update it
runs the new version. It keeps the old one running until it is closed and
opened again: the Director says so before installing when an AI app is
connected. On Windows the installer ends the running server along with the
app.

A release build merges `app/src-tauri/tauri.release.conf.json` into the
configuration, which makes the bundler sign the updater files. Every other
build (`npm run tauri build`, the test installers of `build.yml`) needs no
key.

The jobs that hold the release key run no action from another repository,
only the runner's own tools and the app's locked dependencies, installed
without their install scripts: whoever could change such an action could
take the key and sign an update every installed app accepts.

## The voice libraries

`crates/voice` links prebuilt sherpa-onnx libraries (with ONNX Runtime)
into the app. The `sherpa-onnx-sys` build script would download them from
sherpa-onnx's GitHub releases without checking them, so every workflow that
builds Rust first runs `.github/scripts/sherpa-onnx-libs.sh` (in
`release.yml` directly, elsewhere through `.github/actions/voice-libs`
after the Rust cache): it downloads this runner's archives, refuses one
whose SHA-256 is not the one written in the script, removes the libraries
a cache may have kept unpacked in `target/sherpa-onnx-prebuilt` (the build
script links those without looking at the archive), and points the build
at the checked archives (`SHERPA_ONNX_ARCHIVE_DIR`, named for the run, so
the build script runs again and unpacks them). Raising `sherpa-onnx` in `Cargo.lock` changes
the archives' names, so the script fails until their new hashes are
written into it: download each archive, check it, and write its
`sha256sum`. A local build that cannot download (a proxy) works the same
way: run the script and export the folder it names. Only on CI does it
remove `target/sherpa-onnx-prebuilt`: locally the folder's name does not
change, so the build script would not run again to unpack the libraries.

## Code signing

The release key signs updates only. The installers themselves are not
code-signed (PL-5 in `docs/specification.md`): a first install shows the
operating system's warning once. On Windows, SmartScreen's "Windows protected
your PC" > More info > Run anyway; on macOS the app is ad-hoc signed, so
Apple silicon runs it, but Gatekeeper blocks its first start until Privacy &
Security > Open Anyway. Signing with an Apple Developer ID (and
notarization) or a Windows code-signing certificate would need the
certificate's secrets in the `release` environment and the bundler's
signing settings ([Tauri docs: macOS](https://v2.tauri.app/distribute/sign/macos/),
[Windows](https://v2.tauri.app/distribute/sign/windows/)).

## One-time setup

1. **Release key.** On your own computer, in `app/`:
   `npm run tauri signer generate -- -w ~/.tauri/buzzdome.key`, with a
   password. Keep the key file and its password safe and private: without
   them no installed app can be updated again, and anyone who has them can
   sign an update the apps accept. Put the public key (the content of
   `~/.tauri/buzzdome.key.pub`) in `plugins.updater.pubkey` in
   `app/src-tauri/tauri.conf.json`.
2. **An environment for the key.** In this repository's Settings >
   Environments, create `release`, and under Deployment branches and tags
   choose "Selected branches and tags" and add the tag rule `v*`. Under
   Required reviewers add yourself, and leave "Prevent self-reviews" off,
   or you cannot approve your own releases. Clear "Allow administrators to
   bypass configured protection rules": sessions that act as you are admins
   too. Every run that would use the
   key then waits for your approval, and GitHub mails you about each one.
   Both are free on a public repository
   ([GitHub docs: Deployments and environments](https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments)).
3. **Secrets** of that environment: `TAURI_SIGNING_PRIVATE_KEY` (the content
   of `~/.tauri/buzzdome.key`) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
4. **Who may push tags.** Settings > Rules > Rulesets > New tag ruleset:
   target `v*`, rules "Restrict creations", "Restrict updates" and
   "Restrict deletions", with Repository admin in the bypass list. That
   keeps collaborators from making version tags, but not Claude sessions:
   their GitHub access acts as you, an admin. A run for a tag pushed on
   any commit uses the workflow file of that commit, checks included
   ([GitHub docs: Events that trigger workflows](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows)),
   so what stops a release you did not mean is your approval (step 2): a
   run you did not start waits, and you reject it.

A changed key needs a manual reinstall on every computer, since the apps
there only accept the old one.
