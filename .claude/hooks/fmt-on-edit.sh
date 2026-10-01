#!/usr/bin/env bash
# PostToolUse hook: format the edited Rust file.
# Exit 2 feeds stderr back to Claude (e.g. a syntax error rustfmt can't parse).
set -uo pipefail

command -v jq >/dev/null || { echo "hook needs jq (apt/brew/winget install jq)" >&2; exit 1; }

file=$(jq -r '.tool_input.file_path // empty')
[[ "$file" == *.rs && -f "$file" ]] || exit 0

if ! out=$(rustfmt --quiet "$file" 2>&1); then
  echo "rustfmt failed on $file (usually a syntax error):" >&2
  echo "$out" | head -n 20 >&2
  exit 2
fi
exit 0
