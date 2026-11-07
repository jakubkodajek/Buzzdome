#!/usr/bin/env bash
# PreToolUse hook for Bash: blocks destructive commands, release tags and
# printing secrets. Exit code 2 blocks the call and shows stderr to the agent.
set -euo pipefail
# shellcheck source-path=SCRIPTDIR source=lib.sh
. "$(dirname "$0")/lib.sh"

command=$(hook_field "$(cat)" command)

block() {
  echo "Blocked by .claude/hooks/guard-bash.sh: $1" >&2
  echo "If this is really needed, ask Jakub to run it." >&2
  exit 2
}

# Normalise whitespace for matching.
cmd=$(printf '%s' "$command" | tr -s '[:space:]' ' ')

case "$cmd" in
  *"rm -rf /"|*"rm -rf / "*|*"rm -rf /*"*|*"rm -rf ~"|*"rm -rf ~/"|*"rm -rf \$HOME"|*"rm -rf \$HOME/"|*"rm -rf ."|*"rm -rf . "*|*"rm -rf *"*)
    block "recursive delete of a root, home or the whole working directory" ;;
  *"git push --force"*|*"git push -f "*|*"git push -f"|*"git push"*" +"*)
    block "force push" ;;
  *"git push"*"--tags"*|*"git push"*" v"[0-9]*|*"git push"*"refs/tags/"*|*"gh release "*)
    block "a v* tag publishes a release to every installed app; only Jakub's go-ahead pushes one (docs/releasing.md)" ;;
  *"git reset --hard origin"*|*"git clean -fdx"*|*"git clean -xfd"*)
    block "discarding all local work" ;;
  *"git commit"*"--no-verify"*|*"git push"*"--no-verify"*)
    block "skipping git hooks" ;;
  *"git add"*"replica/sources"*|*"git add"*"replica/screens"*|*"git add -f"*|*"git add --force"*)
    block "force-adding ignored files (the reference manual copies must never be committed)" ;;
  *"curl "*"| sh"*|*"curl "*"| bash"*|*"wget "*"| sh"*|*"wget "*"| bash"*)
    block "piping a remote script into a shell" ;;
  *"chmod -R 777"*|*"mkfs"*|*"dd if="*" of=/dev/"*)
    block "dangerous system command" ;;
  *"cat .env"*|*"printenv"*|*"echo \$"*TOKEN*|*"echo \$"*KEY*|*"echo \$"*SECRET*|*"TAURI_SIGNING_PRIVATE_KEY"*)
    block "printing secrets" ;;
esac

exit 0
