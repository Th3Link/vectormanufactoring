#!/usr/bin/env bash
# Stop/SubagentStop hook: run clippy and tests if Rust code changed.
# Exit 2 blocks finishing and feeds stderr back to Claude.
set -uo pipefail

command -v jq >/dev/null || { echo "hook needs jq (apt/brew/winget install jq)" >&2; exit 1; }

input=$(cat)

# Second stop after a failed gate: let it through to avoid an endless loop.
# CI and the tester still catch what is left.
[[ "$(jq -r '.stop_hook_active // false' <<<"$input")" == "true" ]] && exit 0

# Run in the agent's working directory (may be a worktree).
cwd=$(jq -r '.cwd // empty' <<<"$input")
cd "${cwd:-$CLAUDE_PROJECT_DIR}" 2>/dev/null || exit 0
root=$(git rev-parse --show-toplevel 2>/dev/null) || exit 0
cd "$root"
[[ -f Cargo.toml ]] || exit 0   # no workspace yet (docs-only phase)

# Only gate if Rust sources or manifests changed (tracked or untracked).
changed=$(
  { git diff --name-only HEAD 2>/dev/null
    git ls-files --others --exclude-standard
  } | grep -E '(\.rs|Cargo\.toml|Cargo\.lock)$' || true
)
[[ -n "$changed" ]] || exit 0

if ! out=$(cargo clippy --workspace --all-targets --quiet --message-format=short -- -D warnings 2>&1); then
  echo "Quality gate failed: clippy. Fix these before finishing:" >&2
  echo "$out" | head -n 40 >&2
  exit 2
fi

if cargo nextest --version >/dev/null 2>&1; then
  test_cmd=(cargo nextest run --workspace --cargo-quiet --no-fail-fast
            --status-level fail --final-status-level fail --hide-progress-bar)
else
  test_cmd=(cargo test --workspace --quiet)
fi

if ! out=$("${test_cmd[@]}" 2>&1); then
  echo "Quality gate failed: tests. Fix these before finishing:" >&2
  echo "$out" | tail -n 60 >&2
  exit 2
fi
exit 0
