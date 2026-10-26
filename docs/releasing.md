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
   say) and merge it to `main`. The app offers only a version higher than
   its own. Wait until the Build workflow's `check` and `windows-tests` have
   passed on that commit.
2. Tag that commit of main with the version, with what changed as the tag's
   message, in Czech: the quizmaster reads it before installing.

   ```sh
   git tag -a v0.2.0 -m "Opravy po akci" -m "- Rychlejší start" origin/main
   git push origin v0.2.0
   ```

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

What `docs/ai-assistants.md` describes updates with the app, as far as it
can:

- **MCP server.** Claude Desktop starts the app itself (`buzzdome --mcp`),
  so after an update it runs the new version. It keeps the old one running
  until it is closed and opened again: the Director says so before
  installing when Claude Desktop is connected. On Windows the installer
  ends the running server along with the app.
- **Command line.** A release installs `buzzdome-quiz` beside the app
  (`externalBin` in `tauri.release.conf.json`; `release.yml` builds it, for
  both Mac architectures in one file), so updates replace it too. It is in
  the app's folder: `C:\Program Files\Buzzdome\buzzdome-quiz.exe` on
  Windows (or where the app was installed),
  `/Applications/Buzzdome.app/Contents/MacOS/buzzdome-quiz` on macOS.
- **Skill.** A skill uploaded to Claude, Gemini or ChatGPT stays as it was
  when saved, in that assistant's account, where the app cannot reach. The
  app remembers which package it saved last (`ai-skill.json` in its data
  folder); when an update changes the skill, the Director says so and
  offers to save it again, to upload in place of the old one.

A release build merges `app/src-tauri/tauri.release.conf.json` into the
configuration, which makes the bundler sign the updater files and install
the command line. Every other
build (`npm run tauri build`, the test installers of `build.yml`) needs no
key.

The jobs that hold the release key run no action from another repository,
only the runner's own tools and the app's locked dependencies, installed
without their install scripts: whoever could change such an action could
take the key and sign an update every installed app accepts.

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
   or you cannot approve your own releases. Every run that would use the
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
