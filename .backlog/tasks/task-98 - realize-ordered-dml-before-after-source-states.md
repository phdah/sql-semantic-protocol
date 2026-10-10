---
id: TASK-98
title: Realize ordered DML and DDL before/after physical states
status: To Do
assignee: []
created_date: '2026-10-10'
updated_date: '2026-10-10'
labels: []
milestone: m-3
dependencies:
  - TASK-97
references:
  - 'TASK-68'
  - 'TASK-65'
  - 'TASK-80'
  - 'TASK-81'
  - 'TASK-82'
  - 'TASK-83'
  - 'TASK-84'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Lift already-defined write effects and branch programs into complete, ordered physical-source state constructions. Individual DML/DDL variant semantics remain owned by TASK-80..84.

**TASK-68 child, execution step 6/8:** After foundational PR #92 is merged, implement this task in its own PR against `main`. Follow the ordered child queue declared in TASK-68; do not work around a blocked prerequisite by marking it Done.

**Release contract:** Preserve the canonical source-independent relational DAG and typed source-level constructive proof. Reuse upstream operator-local semantics and their authoritative typed evidence; do not duplicate them here or reparse SQL in sql-tdg. Prove feasible versus impossible versus residual soundly, preserve strongest justified outcome domains, and keep release-blocking coverage unverified until independently certified.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Construct exact before/after rows, domains and complete state assignments for proven INSERT, UPDATE, conditional/unconditional DELETE, MERGE/upsert and supported CREATE/REPLACE/ALTER lifecycle branches.
- [ ] #2 Preserve SQL statement order, prior target state, materialized writes, first-applicable MERGE clause precedence, branch predicates and source/target alias sharing; prohibit independent intermediate-table writes.
- [ ] #3 Compose physical source and target constraints with operator witnesses, NULL/uniqueness/FK/default requirements, row-affect counts and transaction visibility only where provable.
- [ ] #4 Classify contradictory state sequences as impossible; retain typed residual for unresolved actions, partial producers, unsupported transaction/session laws and missing schema evidence.
- [ ] #5 Add scripted DuckDB E2E and cross-dialect direct SQL/dbt proof tests for multiple writes, repeated targets, duplicates, zero/negative branches and exact final state.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-68](task-68%20-%20compose-witnesses-across-full-transformation-graphs.md); parent remains In Progress until all eight children are Done.
- **Delivery:** A focused PR into `main` with code, tests, API/schema/docs/adapter updates proportional to changed semantics, and passing CI.
- **Next:** After this task is Done, proceed to TASK-99; do not skip ahead.
- **Scope boundary:** Other TASK-70..91 work still owns individual missing operator variants, exhaustive conformance, the downstream generator, and protocol 3.0.0 release sign-off.

