---
id: TASK-47
title: Add a differential conformance suite against a SQL engine
status: In Progress
assignee: []
created_date: '2026-10-07 18:11'
updated_date: '2026-10-07'
labels: []
milestone: m-2
dependencies:
  - TASK-43
references:
  - sql-tdg tests/advanced_raw_sql.rs
  - sql-tdg TASK-21.5
  - sql-tdg TASK-21.7
priority: high
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Why
Every round so far found soundness bugs by hand, using probe programs and DuckDB checks. Without an automated oracle the next feature will reintroduce the same class of bug. The guarantee is checkable mechanically: run the query on rows built from the emitted semantics and compare.

## Outcome
A deterministic differential conformance suite runs in CI and checks the protocol's claims against a real SQL engine. It covers both a curated matrix and seeded random predicate trees. Any exactness claim, output domain, or CASE branch domain that the engine contradicts fails the build.

DuckDB as a dev-only dependency is the proposed engine (sql-tdg already uses `duckdb` ~1.10506 for the same purpose). Per AGENTS.md, adding it needs maintainer confirmation before implementation starts.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The engine dependency is confirmed by the maintainer and added only as a dev-dependency with minimal features
- [ ] #2 For every scope marked exact, rows sampled inside all domains and equalities are returned by the engine, and rows violating exactly one column domain or one equality are not
- [ ] #3 Output values the engine computes on sampled data always lie within the emitted output domains
- [ ] #4 Rows sampled inside a CASE branch selection domain produce that branch result in the engine
- [ ] #5 A curated matrix covers each allow-listed condition shape and each residual shape, crossed with the locations top-level WHERE, inner-join ON, CTE, chained CTE, derived table, multi-layer composition, and set-operation branch, with and without NULLs
- [ ] #6 A seeded generator produces at least several thousand random AND/OR/NOT predicate trees over typed columns and literals; every case is checked and failures print a minimal reproducible query and seed
- [ ] #7 Every reproduction from TASK-36 to TASK-42 and the exactness, composition, join-equality, and literal-typing tasks is part of the suite
- [ ] #8 The suite is deterministic, runs in the standard CI check, and is documented in the README
<!-- AC:END -->


## Progress

- Maintainer approved DuckDB as a dev-only dependency on 2026-10-07, with the explicit requirement that DuckDB is not compiled from source.
- The implementation keeps duckdb-rs default features disabled and uses `DUCKDB_DOWNLOAD_LIB=1` to link its prebuilt library.
