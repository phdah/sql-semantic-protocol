#!/usr/bin/env bash
# Provision a version-matched native DuckDB library before Rust CI builds.
# Avoid libduckdb-sys' ephemeral OUT_DIR download-path inference. This is
# CI-only and does not change the crate's public dependencies or semantics.
set -euo pipefail

# duckdb = "~1.10506.0" in Cargo.toml links to DuckDB native v1.5.6.
grep -Fq 'duckdb = { version = "~1.10506.0"' Cargo.toml || {
  echo "DuckDB Rust dependency changed: update the native CI pin too" >&2
  exit 1
}

root="${RUNNER_TEMP:?RUNNER_TEMP required}/sql-semantic-protocol-libduckdb-1.5.6"
archive="${RUNNER_TEMP}/libduckdb-linux-amd64-v1.5.6.zip"
url="https://github.com/duckdb/duckdb/releases/download/v1.5.6/libduckdb-linux-amd64.zip"

mkdir -p "$root"
if [[ ! -f "$root/libduckdb.so" || ! -f "$root/duckdb.h" ]]; then
  curl --fail --location --retry 4 --retry-all-errors --retry-delay 2 --silent --show-error \
    "$url" --output "$archive"
  unzip -oq "$archive" -d "$root"
fi

test -f "$root/libduckdb.so"
test -f "$root/duckdb.h"

# GitHub Actions consumes these for subsequent steps. LD_LIBRARY_PATH is
# needed at test execution time in addition to the build-script link path.
{
  printf 'DUCKDB_LIB_DIR=%s\n' "$root"
  printf 'DUCKDB_INCLUDE_DIR=%s\n' "$root"
  printf 'LD_LIBRARY_PATH=%s%s\n' "$root" "${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
} >> "${GITHUB_ENV:?GITHUB_ENV required}"
