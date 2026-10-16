---
id: TASK-106
title: Compose typed expression obligations through relational graphs
status: In Progress
assignee: []
created_date: '2026-10-13'
updated_date: '2026-10-16'
labels: []
milestone: m-3
dependencies:
  - TASK-105
references:
  - 'TASK-70'
  - 'TASK-68'
  - 'TASK-85'
  - 'TASK-86'
  - 'TASK-80'
  - 'TASK-81'
  - 'TASK-82'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Consume canonical expression and predicate proofs from TASK-101..105 across actual relational operators and physical-source DAG construction. Extend, do not replace, the existing TASK-68 generic graph/source realization engine.

**TASK-70 child, step 6/8.** Implement this task in a fresh PR targeting `main` once the previous dependency has merged. The TASK-70 parent owns the [dependency-ordered queue](task-70%20-%20solve-expression-and-predicate-variants.md); this child MUST NOT depend on the parent (no dependency cycle). A green, bounded increment is mergeable but does not complete this child unless every criterion below is independently verified.

**Release contract:** Produce canonical, source-independent, parser-agnostic typed outcomes with strongest provable value domains and deterministic proof strength (exact, impossible, residual) for SQL TRUE and NOT TRUE. Preserve explicit residuals and never move missing semantics to sql-tdg. Protocol 3.0.0 release remains gated by TASK-91.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Propagate typed expression truth, output domains and stable column references through WHERE/HAVING/QUALIFY, SELECT and computed aliases, JOIN keys/ON conditions, GROUP BY and aggregation, and computed INSERT/UPDATE/MERGE assignments where existing operator semantics permit.
- [ ] #2 Derive strongest justified output value domains, lower/upper bounds and inclusion/exclusion after each supported operator and materialization boundary; preserve lineage and producer reference identity without SQL AST or consumer heuristics.
- [ ] #3 Lift a single consistent, jointly satisfying typed source-row assignment through shared/multiple physical producers for positive and NOT TRUE rejected targets, including duplicate, NULL, grouping, join and write-state interactions.
- [ ] #4 Prove impossibility only for necessary contradictions or complete closed-world absence; ensure partial producers, unsupported aggregations, lossy inverses, missing source constraints and unverified DML semantics report typed residual, never fabricated feasible plans.
- [ ] #5 Exercise raw SQL/dbt with complete physical source rows, filtered joins, groups/HAVING, aliases, ordered DML and joint terminal negative tests against DuckDB where supported; update canonical witness/API, emitted schema, adapters, code map and coverage evidence.
<!-- AC:END -->

## Bounded implementation evidence (2026-10-16, PR #114)

Increment on `feat/task-106-composed-predicate-graphs`. The initial
implementation passed CI (fmt, strict Clippy, tests, docs,
no-default-features, dbt E2E); subsequent review fixes require revalidation.
This evidence does **not** discharge the five acceptance criteria:

- `src/physical_realization/graph.rs`: `joint_physical_filters` now tries the
  type-certified identity mapping first (identical datatypes at both chain
  ends, verified per hop), keeping the uncertified path for conditions
  needing no type evidence. Untyped chains behave exactly as before; no
  composed-semantics relabeling (Intermediate provenance contract kept).
- `tests/task_106_composed_predicates.rs` (new, public API only): chained
  range-filter conjunction with joint TRUE/NOT TRUE feasibility, single-layer
  and chained contradiction handling (Impossible vs Empty plus
  ConflictingDomains gap), filtered INNER JOIN key equalities with explicit
  single-source boundaries, GROUP BY/HAVING output domains with residual
  scope, computed cross-layer residual vs exact local affine proof, QUALIFY
  residual, INSERT-select composition with partial downstream, dbt parity,
  13-dialect parser/equivalence cells, DuckDB TRUE/IS NOT TRUE oracles.
- Promoted one pinned OR-filter chain
  (`tests/physical_realization/graph.rs`) from multiple-witnesses residual
  to joint feasibility; untyped chains keep the residual.
- `docs/semantics.md`, `docs/coverage-manifest.json` (both TASK-106
  families to bounded exact with residual) and
  `tests/task_70_predicates.rs` inventory updated. No protocol wire change.
  General multi-operator DAGs, computed cross-layer inversion, and vendor
  execution remain explicit residuals. TASK-106 must either implement its
  remaining operator/physical-source AC or explicitly rescope the child task
  before closure. TASK-107/108 are separate follow-up tasks, not substitutes
  for unverified TASK-106 acceptance criteria (TASK-91 stays blocked).

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** Reuse TASK-68/93..100 physical machinery and TASK-79 schema laws. This child owns the **missing computed-expression integration**, not an independent rewrite of joins, groups or DML nor global end-to-end TASK-89 certification.
- **Next:** Complete and independently verify every TASK-106 AC before marking Done; then pick **TASK-107**.
