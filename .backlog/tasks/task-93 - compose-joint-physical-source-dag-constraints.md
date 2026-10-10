---
id: TASK-93
title: Compose jointly satisfiable physical-source DAG constraints
status: Done
assignee: []
created_date: '2026-10-10'
updated_date: '2026-10-10'
labels: []
milestone: m-3
dependencies:
  - TASK-67
  - TASK-69
references:
  - 'TASK-68'
  - 'TASK-70'
  - 'TASK-85'
  - 'TASK-86'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Finish the physical-source-level constraint solver for multiple dependent terminals and branches. Compose existing canonical local facts only; expression semantics owned by TASK-70 and final outcome goals owned by TASK-85/86 are not reimplemented here.

**TASK-68 child, execution step 1/8:** After foundational PR #92 is merged, implement this task in its own PR against `main`. Follow the ordered child queue declared in TASK-68; do not work around a blocked prerequisite by marking it Done.

**Release contract:** Preserve the canonical source-independent relational DAG and typed source-level constructive proof. Reuse upstream operator-local semantics and their authoritative typed evidence; do not duplicate them here or reparse SQL in sql-tdg. Prove feasible versus impossible versus residual soundly, preserve strongest justified outcome domains, and keep release-blocking coverage unverified until independently certified.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Resolve a topologically ordered reference DAG to independent, uniquely identified physical sources; preserve repeated/shared source-row identities, producer boundaries, materialization versus inline scope and write kinds.
- [x] #2 Conjoin physical-row truth, domains, NULL-sensitive predicates, cross-row correlation and full-source completeness obligations across several producers and terminal goals; do not combine independent feasible examples as proof.
- [x] #3 Prove feasible plans only with one consistent assignment to each shared physical row/relation; classify contradictions as impossible and unresolved/cyclic/partial/unknown evidence as typed residual.
- [x] #4 Provide deterministic canonical Rust and protocol JSON proof/diagnostic representation usable by consumers without SQL parsing; preserve source-level lineage and proof strength.
- [x] #5 Test compatible and incompatible filters, shared/disjoint sources, unknown counts, aliases, NULL, duplicates, multi-terminal branches and reference cycles across exposed dialects with DuckDB differential oracles.
<!-- AC:END -->

## Completed implementation and acceptance evidence (PR #93)

- **#1 DAG and identity:** `PhysicalJointSourcePlan` walks a single stable,
  producer-first canonical reference graph, deduplicating shared physical
  leaves and materialized layers. Local operator witnesses, write kinds and
  pending producer boundaries remain attached to their own graph nodes.
  Structural missing, ambiguous, cyclic and partial producers fail closed.
- **#2 Joint proof obligations:** exact schema-backed integer and SQL
  NULL-sensitive predicates are evaluated together against the *same*
  physical row identity. Positive requirements use SQL TRUE; zero-output
  requirements use SQL NOT TRUE (including UNKNOWN). Universal `Rows`
  bounds and `ClosedWorld(EntireRelation)` constrain every physical row,
  not independent examples. Type-certified identity projections traverse
  several named producers, checking every edge. Existing all-empty
  join/group branches share complete physical-row absence obligations;
  unsupported nonempty cross-row correlations retain operator-local
  evidence and an explicit `unproved_cross_row_correlation` residual,
  pending operator-specific implementation in TASK-94/95.
- **#3 Proof strength and uncertainty:** exact source populations with
  incompatible requirements are impossible; incompatible sufficient-only
  filtered populations remain residual because disjoint extra rows might
  exist. Unknown type evidence, unresolved lineage, unsupported shapes,
  partial/ambiguous/cyclic producers and unproved row multiplicity cannot
  produce a feasible plan.
- **#4 Public contract:** typed Rust `physical_joint_source_plan`, stable
  `PhysicalRowTarget` / `PhysicalJointSourcePlan` accessors, and optional
  `graph.physical_joint_count_plan` JSON for row-only goals, including
  canonical node references, one typed outcome and diagnostic gap. Active
  schema, protocol docs, semantic docs and machine-readable coverage
  inventory are updated.
- **#5 Verification:** `tests/physical_realization.rs` proves direct and
  multi-producer, alias, shared/disjoint source, duplicate, NULL, positive,
  deliberately rejected, impossible, residual, cycle, partial writer and
  DuckDB physical fixture cases across exposed dialects. The canonical
  schema-provenance parity test checks dbt catalog, dbt manifest and
  external metadata representations. Full CI on the branch covers format,
  Clippy, tests, docs, no-default-features and dbt Core E2E.

TASK-93's generic graph-and-obligation composer is complete for the
currently certified canonical facts. This does **not** claim exact realization
of unresolved physical join-key bijections, grouped/window/set multiplicity,
general output distributions, stateful DML or all sql-tdg fixtures. Those
remain typed residuals and are specifically owned by TASK-94..100 under
parent TASK-68; neither the parent nor TASK-91's 3.0.0 release gate is
unblocked by this child alone.

## Implementation and verification

- **Parent:** [TASK-68](task-68%20-%20compose-witnesses-across-full-transformation-graphs.md); parent remains In Progress until all eight children are Done.
- **Delivery:** A focused PR into `main` with code, tests, API/schema/docs/adapter updates proportional to changed semantics, and passing CI.
- **Next:** After this task is Done, proceed to TASK-94; do not skip ahead.
- **Scope boundary:** Other TASK-70..91 work still owns individual missing operator variants, exhaustive conformance, the downstream generator, and protocol 3.0.0 release sign-off.

