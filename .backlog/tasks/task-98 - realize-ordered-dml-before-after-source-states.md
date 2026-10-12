---
id: TASK-98
title: Realize ordered DML and DDL before/after physical states
status: Done
assignee: []
created_date: '2026-10-10'
updated_date: '2026-10-12'
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
- [x] #1 Construct exact before/after rows, domains and complete state assignments for proven INSERT, UPDATE, conditional/unconditional DELETE, MERGE/upsert and supported CREATE/REPLACE/ALTER lifecycle branches.
- [x] #2 Preserve SQL statement order, prior target state, materialized writes, first-applicable MERGE clause precedence, branch predicates and source/target alias sharing; prohibit independent intermediate-table writes.
- [x] #3 Compose physical source and target constraints with operator witnesses, NULL/uniqueness/FK/default requirements, row-affect counts and transaction visibility only where provable.
- [x] #4 Classify contradictory state sequences as impossible; retain typed residual for unresolved actions, partial producers, unsupported transaction/session laws and missing schema evidence.
- [x] #5 Add scripted DuckDB E2E and cross-dialect direct SQL/dbt proof tests for multiple writes, repeated targets, duplicates, zero/negative branches and exact final state.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-68](task-68%20-%20compose-witnesses-across-full-transformation-graphs.md); parent remains In Progress until all eight children are Done.
- **Delivery:** A focused PR into `main` with code, tests, API/schema/docs/adapter updates proportional to changed semantics, and passing CI.
- **Next:** After this task is Done, proceed to TASK-99; do not skip ahead.
- **Scope boundary:** Other TASK-70..91 work still owns individual missing operator variants, exhaustive conformance, the downstream generator, and protocol 3.0.0 release sign-off.


## Implementation and verification (2026-10-12)

Implemented in PR #104. The canonical ordered-state planner constructs complete
before/after physical row bags and NULL-aware exact value domains for schema-backed
INSERT SELECT, UPDATE, DELETE, MERGE and CTAS/REPLACE TABLE operations. It retains
original SQL statement order, source/target aliases, first-match branch precedence,
prior target populations, DML counts and enforced constraints. Contradictions are
impossible; unknown or unsupported upstream laws remain typed residual rather than
false feasible, including unmodeled ALTER/transactions, default values and dialect-
specific constraint differences. The downstream SQL TDG and 3.0.0 sign-off stay blocked
on their own tasks.

Verification: Rust tests, no-default-features tests, Rustfmt, both Clippy jobs,
documentation, dependency checks and dbt Core E2E all passed on the final
implementation in CI run 38294494171. DuckDB data snapshots, direct SQL and dbt
schema-provenance parity, repeated writes, nulls, duplicates, negative/zero branches,
key/FK/default residuals, and exact before/after states are covered.
