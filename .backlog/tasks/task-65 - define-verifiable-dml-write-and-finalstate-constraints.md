---
id: TASK-65
title: Define verifiable DML write and final-state constraints
status: Done
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
- [x] #1 Represent initial-state assumptions, inserted/updated/deleted rows and post-state invariants for an explicitly supported class of INSERT, UPDATE, DELETE and MERGE.
- [x] #2 Preserve match/unmatched branches, predicate domains, key constraints, affected row counts and conflicts without inventing pre-existing rows.
- [x] #3 Mark unsupported branches and nonprovable post-state claims as partial or residual with actionable reasons.
- [x] #4 Add deterministic execution-based tests for initial state, mutation and observed final state, including duplicate-key edge cases. Assert idempotence only for mutation forms and initial-state assumptions that explicitly guarantee it; do not require or claim idempotence for general INSERT, UPDATE, DELETE or MERGE.
- [x] #5 Document contract/versioning and adapter consistency; sql-tdg TASK-31 consumes the DML semantics.
<!-- AC:END -->

## Implemented (2026-10-09)

- [x] Parser-independent `WriteStateEffect` with explicit caller-supplied complete
  initial target state, logical inserted/updated/deleted target-row counts,
  unchanged-row preservation, SQL-ordered MERGE matched/unmatched branches,
  full ON condition and typed action values, and standalone UPDATE/DELETE predicates.
- [x] Canonical predicate domains and conservative written-value domains;
  `WriteCardinalityRule` and `WriteRowCounts` validate exact conditional
  count conservation without inventing source matches or pre-existing rows.
  Unconditional DELETE alone proves an empty poststate and idempotence.
- [x] Bundle-level `write_effects` binds resolved input/target identities to
  the existing canonical target constraint set when available, including
  primary/unique/foreign keys, provenance, and enforcement metadata.
  Missing target evidence stays `null` and never means collision-free.
- [x] Default-deny residuals for unknown row counts, predicate exactness,
  MERGE multiplicities, potential key conflicts, unsupported actions and
  INSERT's implicit target-column mapping. Unsupported UPDATE/DELETE
  multi-table/subquery forms remain explicit unsupported statements.
- [x] DuckDB initial/mutation/final-state checks for INSERT, UPDATE, DELETE and
  MERGE, NULL filters, conflicting INSERT/MERGE primary keys, negative
  arithmetic cases, idempotence and shared-dialect parser boundaries;
  versioned schema and protocol/semantics/adapter documentation updated.

The exposed cardinality equations are **conditional** on a successful
mutation and verified logical action counts. This project does not execute
mutations, fabricate initial rows or independently prove engine-specific
constraint feasibility. SQL and metadata evidence use canonical representations;
dbt compiled SELECT models and ODCS metadata do not supply executed DML statements.

Implementation: PR #86. This task targets sql-tdg TASK-31.
