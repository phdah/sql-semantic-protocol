---
id: TASK-68
title: Prove end-to-end physical-source realizability across layers
status: In Progress
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-67
  - TASK-69
references: 
  - 'TASK-44'
  - 'TASK-58'
  - 'TASK-65'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Source generation must satisfy a complete DAG of dependent CTEs, dbt models, multi-branch queries and aliases simultaneously; merely carrying per-layer witness provenance is insufficient.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Compose row identity, predicate truth, lineage, projection, relational multiplicity and operator witness obligations from terminal outputs to independent controllable physical sources.
- [ ] #2 Handle multi-parent DAGs, repeated/shared sources, joins feeding GROUP BY/HAVING/QUALIFY/sets, aggregate outputs feeding filters, and derived tables with selective projections.
- [ ] #3 Preserve materialization-vs-inline semantics and producer write kinds; prove that zero-count or negative cases exclude all otherwise qualifying source rows.
- [ ] #4 Detect infeasible cycles, non-invertible projections, conflicting domains, unknown upstream cardinality and partial producers as explicit typed residual or unsatisfiable.
- [ ] #5 Use deterministic constructive plans and DuckDB end-to-end oracle tests for the exact committed sql-tdg fixture shapes, including jointly satisfied terminals and deliberate rejections.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Implementation progress (PR #92)

- Added a canonical, reference-based physical dependency graph in both the Rust
  API and emitted protocol JSON. Shared producers and physical leaves are
  defined only once and referenced by stable typed identities.
- Preserved definition writes and fail-closed producer resolution. Ambiguous,
  cyclic, partial and unproved producer paths return typed residual reasons.
- Reused proven local Boolean witnesses to classify individual source rows
  through safe projection/filter boundaries without treating intermediate
  relations as writable tables.
- Proved the independent zero-output construction for controlled, empty
  source inputs passed through row-preserving, filtering, or safely identified
  join producer chains, including shared/self-join physical sources. This proof requires both explicit exact zero-row bounds and
  entire-relation closed-world coverage.
- Added cross-dialect, NULL, DuckDB, metadata and schema-contract tests and
  extended the canonical emission examples and coverage inventory.

**Still blocking:** Multi-operator and multi-parent joint satisfiability,
mixed-join and aggregate/window/set DAG construction, positive nonzero
cardinality and distribution counts, general absence classification, and
full DML before/after state proofs. Release 3.0.0 remains held.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
