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

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
