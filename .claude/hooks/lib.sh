#!/usr/bin/env bash
# Shared helper for the hooks: reads one field of the hook's JSON input.
# Uses jq when installed, python3 otherwise (macOS before 15 has no jq).

# hook_field <json> <field> [<fallback field>]
hook_field() {
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$1" | jq -r --arg a "$2" --arg b "${3:-$2}" '.tool_input[$a] // .tool_input[$b] // ""'
  else
    printf '%s' "$1" | python3 -c 'import json,sys; t=json.load(sys.stdin).get("tool_input",{}); print(t.get(sys.argv[1]) or t.get(sys.argv[2]) or "")' "$2" "${3:-$2}"
  fi
}
