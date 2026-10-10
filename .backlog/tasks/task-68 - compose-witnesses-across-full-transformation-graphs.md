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
  relations as writable tables. Added a whole-path joint solver for sequences
  of NULL-sensitive, identity-mapped WHERE filters, preserving intermediate
  local witness boundaries until the complete path is proved.
- Proved the independent zero-output construction for controlled, empty
  source inputs passed through row-preserving, filtering, safely identified
  joins and ordinary GROUP BY/HAVING chains, including shared/self-join
  physical sources. Guarded row-local expressions against global aggregates
  nested inside arithmetic/functions; ROLLUP/CUBE remain residual. This proof requires both explicit exact zero-row bounds and
  entire-relation closed-world coverage.
- Composed multiple zero-output goals over the same canonical physical leaves,
  deduplicating closed-world row assignments across filtered and joined
  terminals and rejecting only necessary count conflicts as impossible.
- Proved zero surviving ordinary GROUP BY groups and complete empty output
  histograms from a single physical all-empty assignment, without treating
  these as evidence for positive group/distribution cardinality.
- Added source-independent, schema-defined per-set-branch
  `empty_input_preserving` evidence to separate row absence from local
  tuple-count membership proof. This includes set-level ORDER BY/LIMIT over
  named derived tables while rejecting source-free arms.
- Extended conservative zero-output proofs to safe DISTINCT, ranked QUALIFY,
  and set branches with verified single-relation row boundaries. Checked
  source-free set arms and global HAVING as explicit residual cases.
- Pinned sql-tdg raw-SQL multi-stage, boundary and set fixtures by upstream
  commit (90ec0e12a2d5), with DuckDB empty-input oracle assertions. These
  certify zero-state shapes only; they do not discharge positive membership,
  distribution or stateful DML fixture acceptance.
- Added exact schema-backed source count construction through fully
  row-preserving producer DAGs, including jointly compatible terminal
  outputs that share one physical source and explicit conflicts when
  requests cannot both be satisfied. Opt-in outcome-goal evaluation can now
  emit existing source_rows/empty_sources evidence for these transitive cases.
- Attached operator-local typed witnesses and pending producer obligations
  to canonical graph nodes so multi-parent and join/group/window/set facts
  remain visible without pretending that residuals are feasible.
- Added cross-dialect, NULL, DuckDB, metadata and schema-contract tests and
  extended the canonical emission examples and coverage inventory.
- Added positive single-source count construction across safely reversible
  WHERE/projection chains. All physically controlled rows must jointly satisfy
  a proven SQL-TRUE predicate, with typed universal closed-world count
  obligations and unconstrained schema evidence. Joint goals preserve the
  predicate and refrain from treating filtered count differences as impossible.
  The legacy outcome-goal SourceRows adapter remains residual when it cannot
  carry these predicates; no consumer-facing arbitrary source-row claim.

- Reused direct physically certified join-pair, grouped/HAVING, ranked
  window and set-tuple count constructions through single-parent,
  identity-only materialized projection chains. The producer must itself
  consume physical sources, and every later relation must preserve both
  cardinality and source-row identity; joined/filtered upstream branches
  remain residual rather than assuming a complete proof.
- Proven complete typed scalar histograms across renamed, row-preserving
  materialized producer chains with actual catalog evidence for each named
  producer output. Every distributed value resolves to the same physical
  source column, is schema-checked, and shares a full-source cardinality.
  Non-invertible projected values or unknown producer columns remain residual.

- Proven positive one-to-one equijoin count outcomes over two independent
  materialized parents whose physical integer keys and row counts remain
  unchanged through producer projections. Follow-on transparent materialized
  projections may reuse the same canonical physical join-pair construction.
- Added a separate nonempty closed-world negative filter plan: all declared
  physical source rows must be SQL NOT TRUE for the jointly proved predicate,
  fixing terminal cardinality to zero without pretending the source is empty.
  A positive transparent count and this negative zero count can share the
  same controlled source in a two-terminal proof. Unsupported combinations
  remain residual.

**Still blocking:** General multi-operator and multi-parent joint
satisfiability, mixed-join and aggregate/window/set DAG construction,
positive nonzero counts beyond the verified transparent-source subset,
complete output distributions, general negative closed-world exclusion,
full DML before/after state proofs, and exact sql-tdg fixture sign-off.
Release 3.0.0 remains held.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
