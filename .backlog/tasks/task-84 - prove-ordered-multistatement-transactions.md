---
id: TASK-84
title: Prove stateful SQL scripts and ordered read/write compositions
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-78
  - TASK-80
  - TASK-81
  - TASK-82
  - TASK-83
  - TASK-161
  - TASK-162
  - TASK-163
  - TASK-164
references: 
  - 'TASK-23'
  - 'TASK-65'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Isolated DML effects do not prove a multi-statement dataset's final output or generate prestate for migration and incremental model workloads.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Define typed script state graph with schema and row-state before/after each statement, dependency ordering, multi-write targets, create/drop/replace lifecycle and statement-local snapshots.
- [ ] #2 Support transactional BEGIN/COMMIT/ROLLBACK/SAVEPOINT where engine evidence permits; model dialect auto-commit, temporary tables and isolation as assumptions/residuals.
- [ ] #3 Prove consistent initial and final source/target datasets and negative cases under sequential INSERT/UPDATE/DELETE/MERGE/DDL scripts, including overwrite and idempotence.
- [ ] #4 Expose deterministic ordered change plans and exact output/mutation counts without requiring consumer SQL parsing or replay of an inferred IR.
- [ ] #5 E2E execute interleaved writes/reads/DDL and compare all intermediate and final state snapshots, including failing/rolled-back transactions.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-84 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: deterministic script with INSERT, UPDATE, DELETE, MERGE and table lifecycle, checking complete before/after snapshots at each step.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: transaction BEGIN/COMMIT/ROLLBACK and nested SAVEPOINT on a supported DuckDB path, verifying rollback and independent output reads.
3. **Conditional and non-exact boundary:** Inventory conditional: engine isolation, implicit DDL commits, deferred FK checks and concurrent transactions; no fabricated cross-engine transaction equivalence.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Bounded dependency-ordered child implementation queue (2026-10-17)

TASK-84 is a **parent signoff**, not a single implementation PR. Preserve all original AC and only sign them off independently after all children are Done and merged. Pick the first unfinished dependency-ready child. Parent depends on the children only for closure; no child depends on its parent. Scope and fixed fixture IDs are in `docs/backlog-child-queue-plan.json`.

- **[TASK-161](task-161%20-%20define-sequential-typed-state-graph-across-dml-ddl.md)** (1/4): INSERT/UPDATE/DELETE/MERGE/DDL; per-step before/after, producer/read identity. IDs `84-C161-P`, `84-C161-N`.
- **[TASK-162](task-162%20-%20prove-duckdb-begin-commit-rollback-savepoint-transitions.md)** (2/4): BEGIN/COMMIT; ROLLBACK; SAVEPOINT/release/rollback-to; negative replay. IDs `84-C162-P`, `84-C162-N`.
- **[TASK-163](task-163%20-%20classify-auto-commit-implicit-ddl-and-engine-isolation-boundaries.md)** (3/4): DDL implicit commits; temp tables; FK deferral; concurrent isolation; dialect transaction laws. IDs `84-C163-P`, `84-C163-N`.
- **[TASK-164](task-164%20-%20certify-ordered-mixed-script-state-and-final-targets.md)** (4/4): mixed DDL,DML,MERGE/read; output counts; dbt/ODCS; dialects. IDs `84-C164-P`, `84-C164-N`.

**Original AC mapping:** #1 -> TASK-161, TASK-163, TASK-164; #2 -> TASK-162, TASK-163, TASK-164; #3 -> TASK-161, TASK-162, TASK-163, TASK-164; #4 -> TASK-161, TASK-162, TASK-164; #5 -> TASK-161, TASK-162, TASK-163, TASK-164; #6 -> TASK-161, TASK-162, TASK-163, TASK-164.

**No premature completion:** Each child freezes and verifies its own exact expected source/target data, output domains, dual-direction witnesses, parser/dialect/adapter cells, and final CI; the parent gets separate whole-AC closure. A conditional vendor limitation must name an owner and approved evidence boundary; it cannot hide a mandatory unimplemented family.
