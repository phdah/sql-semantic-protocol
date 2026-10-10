---
id: TASK-97
title: Prove complete cardinality and negative closed-world outcomes
status: To Do
assignee: []
created_date: '2026-10-10'
updated_date: '2026-10-10'
labels: []
milestone: m-3
dependencies:
  - TASK-96
references:
  - 'TASK-68'
  - 'TASK-69'
  - 'TASK-85'
  - 'TASK-86'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Complete physical-source realizability for positive counts, exact zero outcomes and deliberate negative cases across proven composable operators. Per-terminal rejection alternative enumeration/sampling remains owned by TASK-86.

**TASK-68 child, execution step 5/8:** After foundational PR #92 is merged, implement this task in its own PR against `main`. Follow the ordered child queue declared in TASK-68; do not work around a blocked prerequisite by marking it Done.

**Release contract:** Preserve the canonical source-independent relational DAG and typed source-level constructive proof. Reuse upstream operator-local semantics and their authoritative typed evidence; do not duplicate them here or reparse SQL in sql-tdg. Prove feasible versus impossible versus residual soundly, preserve strongest justified outcome domains, and keep release-blocking coverage unverified until independently certified.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Construct exact positive and zero terminal cardinalities across composed joins, grouping, QUALIFY, sets and projections where operator laws are constructive; reconcile all shared physical input counts.
- [ ] #2 Prove whole-relation negative cases exclude every otherwise qualifying row and path, including alternative join branches, aggregate grouping, NULL UNKNOWN and duplicate scenarios.
- [ ] #3 Jointly realize multiple positive/rejected terminals with compatible, potentially different predicates only after proving the full shared-source Boolean and membership conjunction.
- [ ] #4 Distinguish impossible counts from residual insufficiency; preserve explicit full-source closed-world coverage and all necessary source constraints, never infer complete absence from one negative sample.
- [ ] #5 Provide complete direct SQL/dbt, cross-dialect and DuckDB difference tests for feasible, impossible, residual, zero/nonzero, NULL, duplicate and alternative-path outcomes.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-68](task-68%20-%20compose-witnesses-across-full-transformation-graphs.md); parent remains In Progress until all eight children are Done.
- **Delivery:** A focused PR into `main` with code, tests, API/schema/docs/adapter updates proportional to changed semantics, and passing CI.
- **Next:** After this task is Done, proceed to TASK-98; do not skip ahead.
- **Scope boundary:** Other TASK-70..91 work still owns individual missing operator variants, exhaustive conformance, the downstream generator, and protocol 3.0.0 release sign-off.

