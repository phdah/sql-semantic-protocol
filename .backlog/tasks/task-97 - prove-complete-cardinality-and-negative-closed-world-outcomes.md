---
id: TASK-97
title: Prove complete cardinality and negative closed-world outcomes
status: Done
assignee: []
created_date: '2026-10-10'
updated_date: '2026-10-12'
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
- [x] #1 Construct exact positive and zero terminal cardinalities across composed joins, grouping, QUALIFY, sets and projections where operator laws are constructive; reconcile all shared physical input counts.
- [x] #2 Prove whole-relation negative cases exclude every otherwise qualifying row and path, including alternative join branches, aggregate grouping, NULL UNKNOWN and duplicate scenarios.
- [x] #3 Jointly realize multiple positive/rejected terminals with compatible, potentially different predicates only after proving the full shared-source Boolean and membership conjunction.
- [x] #4 Distinguish impossible counts from residual insufficiency; preserve explicit full-source closed-world coverage and all necessary source constraints, never infer complete absence from one negative sample.
- [x] #5 Provide complete direct SQL/dbt, cross-dialect and DuckDB difference tests for feasible, impossible, residual, zero/nonzero, NULL, duplicate and alternative-path outcomes.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-68](task-68%20-%20compose-witnesses-across-full-transformation-graphs.md); parent remains In Progress until all eight children are Done.
- **Delivery:** A focused PR into `main` with code, tests, API/schema/docs/adapter updates proportional to changed semantics, and passing CI.
- **Next:** After this task is Done, proceed to TASK-98; do not skip ahead.
- **Scope boundary:** Other TASK-70..91 work still owns individual missing operator variants, exhaustive conformance, the downstream generator, and protocol 3.0.0 release sign-off.


## Completion evidence: PR #103

[PR #103](https://github.com/phdah/sql-semantic-protocol/pull/103)
completes the **physically constructive operator classes already certified by the
canonical local witnesses**. The solver never equates a parser-accepted
shape with a proven physical population. Unverified SQL variants remain
typed residuals and must not be considered supported.

1. **#1 Positive and zero counts:** `physical_realization/joins.rs`
   reconciles necessary positive source counts with a complete, zero-row
   INNER JOIN from an explicitly empty partner. The same code preserves
   source-row identities across named materializations. `sets.rs` and
   `composition.rs` scale lossless shared-bag EXCEPT/EXCEPT ALL cancellation
   to exact positive input counts; prior `grouping.rs`, `ranking.rs` and
   `joint_operators.rs` retain complete group/rank/set population proofs.
2. **#2 Exhaustive negative coverage:** `joint_counts.rs` proves every
   row in one closed physical source rejects *all* zero-count terminal
   predicates with SQL NOT TRUE, retaining FALSE/UNKNOWN semantics and
   the independent all-empty alternative. Exact empty-join-partner
   populations exclude all matches across the complete opposite side.
   Shared EXCEPT/EXCEPT ALL branches close the entire NULL/duplicate bag.
   The existing grouped COUNT/HAVING and partitioned ranking witnesses
   retain complete, not sampled, source and operator cardinalities.
3. **#3 Shared positive/rejected terminals:** The existing typed coupled
   Boolean solver conjoins different row predicates over an identical
   source-row identity; the new tests require a transparent three-row
   source to coexist with several independently rejected outputs. Join
   and set operator cases only combine with transparent count terminals
   after full physical count/identity reconciliation; unresolved
   multi-operator correlations remain residual.
4. **#4 Impossible versus residual:** A necessary positive preserved-side
   source count makes a pure LEFT/RIGHT/FULL join's zero output
   **impossible**. A zero INNER join with a positive opposite-side source
   remains feasible through an empty partner. Conflicting necessary
   transparent counts are impossible; unavailable mixed operator
   populations remain residual. Explicit entire-relation closed-world
   source obligations accompany every constructive case.
5. **#5 Verification:** New
   `tests/physical_realization/negative_cardinality.rs` exercises all
   exposed dialects, deterministic joint goal order, zero/nonzero,
   contradictory, NULL, duplicate and alternative join/set population
   cases with DuckDB execution. `tests/dbt_manifest.rs` compares
   dbt authoritative catalog/manifest and equivalent direct-SQL
   source-count/negative proof obligations. Existing
   `tests/physical_realization/{mixed,joint_operators,joint_filters,joins,group_rank_set}.rs`
   cover grouped, ranked, NULL-sensitive, multi-parent and correlated
   operator results. Full CI on the implementation branch includes
   format, default/minimal-feature lint/test/doc, dependencies and
   dbt Core E2E.

**Audited residual boundaries:** Arbitrary overlapping operator
populations without a common constructive witness, mismatched/opaque
schema evidence, unresolved/cyclic/partial producers, transformations
whose physical row identities cannot be inverted, and uncertified local
SQL variants remain residual. Per-terminal randomized negative
alternatives belong to TASK-86. Ordered DML/DDL before/after source
states belong to TASK-98, pinned sql-tdg fixture certification to TASK-99,
and the complete TASK-68 release-blocking audit to TASK-100.

**Queue:** TASK-97 Done after PR #103 merges. TASK-98 is next.
TASK-68 remains In Progress and the separate TASK-91 3.0.0 release
gate remains blocked.
