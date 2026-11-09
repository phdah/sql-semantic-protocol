#!/usr/bin/env bash
# Retry only the external libduckdb download bootstrap, never failing tests.
set -euo pipefail

log="$(mktemp)"
trap 'rm -f "$log"' EXIT

for attempt in 1 2 3; do
  if "$@" 2>&1 | tee "$log"; then
    exit 0
  fi

  if ! grep -q 'Failed to set up libduckdb:' "$log"; then
    echo "Command failed for a reason other than libduckdb bootstrap; not retrying" >&2
    exit 1
  fi

  if [[ "$attempt" -eq 3 ]]; then
    echo "libduckdb bootstrap failed after 3 attempts" >&2
    exit 1
  fi

  echo "libduckdb binary download failed; retrying (${attempt}/3)" >&2
  sleep $((attempt * 10))
done
