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
  - TASK-93
  - TASK-94
  - TASK-95
  - TASK-96
  - TASK-97
  - TASK-98
  - TASK-99
  - TASK-100
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

## Parent task and ordered implementation queue

**TASK-68 is the parent, not the next standalone implementation task.** PR #92
delivers the canonical physical-source DAG and currently proven foundational
constructive subset only; after it passes CI and is merged, TASK-68 stays **In
Progress**. Its original six acceptance criteria remain unchecked until the
children establish the full proof obligations.

**Next task after PR #92 merges: TASK-93.** When asked to work on the "next
task", take the first unfinished, dependency-ready child in this queue **before
unrelated backlog TASK-70..92**. Child PRs target `main` independently and are
merged only after each child's own acceptance criteria and CI pass. The
parent's dependencies on the children are for closure, **not** prerequisites
of the children. Child tasks must never depend on TASK-68, which would create a
cycle.

- **[TASK-93](task-93%20-%20compose-joint-physical-source-dag-constraints.md)**: Compose jointly satisfiable physical-source DAG constraints
- **[TASK-94](task-94%20-%20realize-multiparent-join-and-shared-source-dags.md)**: Realize multi-parent joins and repeated-source DAGs
- **[TASK-95](task-95%20-%20compose-group-window-and-set-dags.md)**: Compose grouping, ranking and set operators across producer DAGs
- **[TASK-96](task-96%20-%20resolve-projection-inversion-and-output-distributions.md)**: Prove safe projection inversion and complete output distributions
- **[TASK-97](task-97%20-%20prove-complete-cardinality-and-negative-closed-world-outcomes.md)**: Prove complete cardinality and negative closed-world outcomes
- **[TASK-98](task-98%20-%20realize-ordered-dml-before-after-source-states.md)**: Realize ordered DML and DDL before/after physical states
- **[TASK-99](task-99%20-%20certify-sql-tdg-physical-fixture-integration.md)**: Certify physical-source proofs against committed sql-tdg fixture shapes
- **[TASK-100](task-100%20-%20signoff-task68-end-to-end-physical-realizability.md)**: Sign off TASK-68 end-to-end physical-source realizability

Execution order is enforced through the sequential child dependencies:
TASK-93 is unlocked by completed TASK-67/69; TASK-94 requires TASK-93;
and each next child requires its immediate predecessor. A failed or blocked
child remains incomplete and must not be skipped or declared Done. The
existing TASK-70..91 continue to own their local semantic variants, dialect
law checks, generator work and release gates; this queue owns **transitive
physical-source composition** and does not waive the other tasks' acceptance.

### Original acceptance-to-child mapping

| Original TASK-68 criterion | Child proof owners |
| --- | --- |
| #1 Source identity, truth, lineage, multiplicity and witnesses | TASK-93, TASK-94, TASK-95, TASK-96, TASK-97 |
| #2 Multi-parent, joins, group/HAVING/QUALIFY/sets, projections | TASK-94, TASK-95, TASK-96 |
| #3 Materialization, writes, exact negative/zero exclusion | TASK-93, TASK-97, TASK-98 |
| #4 Cycles, conflicting domains, unknown cardinality and partial producers | TASK-93, TASK-96, TASK-97, TASK-98 |
| #5 Pinned sql-tdg exact shape / joint terminal DuckDB oracles | TASK-99, TASK-100 |
| #6 Cross-dialect, unit, differential, API/schema/docs and adapter parity | Each child, verified at TASK-100 |

All eight children must be Done, and TASK-100 must explicitly verify **all
six original criteria**, before TASK-68 changes to Done. TASK-91 and the
protocol 3.0.0 release remain separate, still-blocked gates.

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

- Expanded joint shared-source count construction from a two-terminal
  positive/rejected pair to multiple transparent N-row outputs and multiple
  filtered zero-row outputs. Each rejected terminal must independently prove
  the identical exact physical-row SQL NOT TRUE predicate. One closed-world
  physical assignment satisfies all verified goals; different rejection
  predicates remain residual. Added cross-dialect and DuckDB oracle tests.
- Added exact typed before/after state realization for unconditional DELETE
  of one schema-backed, unconstrained physical target with no competing
  in-bundle writer. The closed-world initial target of N rows is fully
  enumerated, and the canonical DML law proves post-state zero. Other
  UPDATE, conditional DELETE, INSERT and MERGE action constructions remain
  release-blocking and residual.

**Still blocking:** General multi-operator and multi-parent joint
satisfiability, mixed-join and aggregate/window/set DAG construction,
positive nonzero counts beyond the verified transparent-source subset,
complete output distributions, general negative closed-world exclusion,
full DML before/after state proofs, and exact sql-tdg fixture sign-off.
Release 3.0.0 remains held.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
