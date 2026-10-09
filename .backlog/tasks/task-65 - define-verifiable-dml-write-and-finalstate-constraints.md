---
id: TASK-65
title: Define verifiable DML write and final-state constraints
status: In Progress
assignee: []
created_date: '2026-10-08'
labels: []
milestone: m-3
dependencies: []
references:
  - 'TASK-23'
  - 'TASK-43'
  - 'sql-tdg TASK-31'
priority: medium
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
INSERT and MERGE are represented in the transformation graph (TASK-23), but partial writes and untouched pre-existing rows mean full final-state generation cannot be inferred. Provide a canonical before/after effect contract for supported mutations.

SQL parsing, normalized semantics, lineage, and exactness remain owned by SQL Semantic Protocol. Preserve existing exact behavior while extending the canonical contract, and never mark unsupported cases exact. Changes must uphold the repository's outcome-first definition of done and cross-adapter parity.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Represent initial-state assumptions, inserted/updated/deleted rows and post-state invariants for an explicitly supported class of INSERT, UPDATE, DELETE and MERGE.
- [ ] #2 Preserve match/unmatched branches, predicate domains, key constraints, affected row counts and conflicts without inventing pre-existing rows.
- [ ] #3 Mark unsupported branches and nonprovable post-state claims as partial or residual with actionable reasons.
- [ ] #4 Add deterministic execution-based tests for initial state, mutation and observed final state, including duplicate-key edge cases. Assert idempotence only for mutation forms and initial-state assumptions that explicitly guarantee it; do not require or claim idempotence for general INSERT, UPDATE, DELETE or MERGE.
- [ ] #5 Document contract/versioning and adapter consistency; sql-tdg TASK-31 consumes the DML semantics.
<!-- AC:END -->

## Implementation in progress (2026-10-09)

The initial scope adds typed partial write obligations for INSERT SELECT, direct
single-target UPDATE and DELETE, and existing MERGE branches, including external
target snapshots, affected-row lower/upper bounds, NULL-aware predicate
diagnostics, key-conflict uncertainty, and proven empty/idempotent unconditional
DELETE. The schema and Rust API use the same representation and execution tests
cover initial state, final state, and duplicate-key rejection.

Keep acceptance checkboxes open pending compile, lint, schema, conformance and
adapter verification. Multi-table writes and unsupported mutation modifiers
remain explicit, not silently interpreted as exact.
