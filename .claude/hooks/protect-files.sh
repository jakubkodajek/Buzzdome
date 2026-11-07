#!/usr/bin/env bash
# Runs protect_files.py, the hook's checks. Fails closed: with no python3 the call is
# blocked (exit 2) instead of let through.
if ! command -v python3 >/dev/null 2>&1; then
  echo "Blocked by .claude/hooks/protect-files.sh: python3 is needed to check this call." >&2
  exit 2
fi
exec python3 "$(dirname "$0")/protect_files.py"
