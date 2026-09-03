#!/usr/bin/env bash
# Commit only the given paths, retrying while another agent holds the index lock.
# usage: commit.sh "<message>" <path>...
set -euo pipefail
msg="$1"; shift
root="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
for attempt in $(seq 1 30); do
  if git -C "$root" add -- "$@" 2>/dev/null && git -C "$root" commit -q --only -m "$msg" -- "$@" 2>/dev/null; then
    git -C "$root" log -1 --oneline
    exit 0
  fi
  if ! git -C "$root" status --porcelain -- "$@" | grep -q .; then
    echo "nothing to commit for: $*"
    exit 0
  fi
  sleep $((RANDOM % 3 + 1))
done
echo "commit failed after retries: $msg" >&2
exit 1
