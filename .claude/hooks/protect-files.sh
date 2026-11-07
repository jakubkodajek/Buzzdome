#!/usr/bin/env bash
# Runs protect_files.py, the hook's checks. Fails closed: with no python3, or when the
# script fails in any way, the call is blocked (exit 2) instead of let through
# (Claude Code blocks only on exit 2).
if ! command -v python3 >/dev/null 2>&1; then
  echo "Blocked by .claude/hooks/protect-files.sh: python3 is needed to check this call." >&2
  exit 2
fi
python3 "$(dirname "$0")/protect_files.py"
status=$?
if [ "$status" -ne 0 ]; then
  [ "$status" -eq 2 ] || echo "Blocked by .claude/hooks/protect-files.sh: protect_files.py failed (exit $status)." >&2
  exit 2
fi
