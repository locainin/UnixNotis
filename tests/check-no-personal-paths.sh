#!/usr/bin/env bash
set -euo pipefail

# Only the neutral fixture home may appear in tracked text
matches=$(git grep -I -n '/home/' -- . ':(exclude)tests/check-no-personal-paths.sh' || true)
unexpected=$(printf '%s\n' "$matches" | grep -v '/home/user/' || true)

if [[ -n "$unexpected" ]]; then
  printf '%s\n' 'tracked files contain a non-generic absolute home path:' >&2
  printf '%s\n' "$unexpected" >&2
  exit 1
fi
