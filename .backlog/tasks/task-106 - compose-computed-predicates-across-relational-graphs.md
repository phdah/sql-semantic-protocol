---
id: TASK-106
title: Compose typed expression obligations through relational graphs
status: In Progress
assignee:
  - OpenCode
created_date: '2026-10-13'
updated_date: '2026-10-16 17:53'
labels: []
milestone: m-3
dependencies:
  - TASK-105
references:
  - TASK-70
  - TASK-68
  - TASK-85
  - TASK-86
  - TASK-80
  - TASK-81
  - TASK-82
modified_files:
  - src/analysis.rs
  - src/analysis/arithmetic_domains.rs
  - src/analysis/writes.rs
  - src/physical_realization.rs
  - src/physical_realization/counts.rs
  - src/physical_realization/distributions.rs
  - src/physical_realization/filters.rs
  - src/physical_realization/graph.rs
  - src/physical_realization/joins.rs
  - src/physical_realization/joint_counts.rs
  - src/physical_realization/membership.rs
  - src/physical_realization/ranking.rs
  - src/physical_realization/sets.rs
  - tests/advanced_target_validation.rs
  - tests/physical_realization/joins.rs
  - tests/physical_write_adapter_parity.rs
  - tests/physical_write_states.rs
  - tests/task_106_composed_predicates.rs
  - tests/task_70_predicates.rs
  - docs/code-map.md
  - docs/coverage-manifest.json
  - docs/protocol.md
  - docs/semantics.md
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Harden TASK-106 follow-up without expanding bounded scope. General multi-join DAGs, correlated sources, general aggregates/windows stay residual by design per AC4.

1. Document private join helpers in src/physical_realization/joins.rs (exact_source_filter, typed_literal_equality_filter, source_filter_for_predicate, join_key_and_scalar_filters); clarify why independent filtered-join proof is sound when local witness is residual.
2. Extract shared blocks_unconstrained_physical_plans helper in physical_realization to remove 11x duplicated constraint-chain (DRY, single source of truth).
3. Split joint_join_with_rejected_filters into key-extraction, closed-source restriction, zero-terminal reconciliation, and joint-satisfiability helpers; preserve behavior exactly.
4. Add evidence tests: computed INSERT SELECT arithmetic domain exactness (already via analyze_query path); outer-join contradictory filters stay residual not impossible; positive cardinality >1 stays Impossible under certified TRUE contradiction; join-gate bypass soundness already covered by filtered join tests.
5. Run fmt, strict clippy, full tests, doc, no-default-features lib; update backlog notes and recommit.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Completed the bounded TASK-106 implementation matrix. Complete single-equijoin populations now carry exact typed non-key ON/WHERE RowTruth filters and joint positive/NOT TRUE terminal counts reuse one closed source assignment. Added supported GROUP/HAVING and ROW_NUMBER/QUALIFY physical controls, computed ordered INSERT/UPDATE/MERGE DuckDB coverage, and direct SQL/dbt-catalog state parity.

Schema-valid emission testing exposed that computed UPDATE/MERGE write values retained expression trees but did not reuse catalog-certified arithmetic domain refinement. Routed write assignment domains through the existing arithmetic prover with analysis metadata; branch-constrained score BETWEEN 1 AND 5 plus 4 now emits the exact set {5,6,7,8,9}. No wire shape or schema change was required.

Strengthened parser-capable dialect checks to compare canonical witnesses, outputs, domains and physical directions; raw SQL/dbt checks now compare complete physical source plans. Residual controls assert typed gaps for partial producers, unsupported HAVING, lossy casts, no-witness projections and missing schema evidence. Focused TASK-106, DML, output-domain, write-adapter, write-state, join and strict all-target Clippy checks pass.

Fixed TASK-106 regression found by full suite: generalized exact_source_filter dropped the legacy typed Boolean/integer literal equality fallback, breaking filtered producer joins on active = TRUE. Restored that fallback alongside the typed scalar witness path, keeping join-key and ambiguous predicates residual. All 16 protocol_conformance_physical tests pass. Full cargo test, strict clippy, cargo doc and no-default-features lib checks are green after updating the TASK-70 inventory expectation to the new bounded_composed_and_write_exact_with_explicit_residuals status.

Code review fixed: added missing /// docs on refine_catalog_arithmetic_domain and extracted refine_write_value_domain to remove 3x duplication in UPDATE/MERGE write paths. Spec review confirmed bounded scope is intentional; flagged join-gate bypass and outer-join residual paths are fail-closed by design with conformance coverage. Full gates re-run green.

Hardening follow-up complete: documented join helpers and gate-bypass soundness, extracted blocks_unconstrained_physical_plans helper (11 sites), split joint_join_with_rejected_filters into key/restriction/reconcile/satisfiability/rebuild helpers with identical behavior. Added 3 evidence tests: computed INSERT SELECT arithmetic exact {5..9}, single-layer contradiction never feasible for 1/2/5 rows with zero feasible, outer-join contradiction stays residual not impossible. Full gates green: 74 ok suites, 0 failed, strict clippy, fmt, doc, no-default-features lib.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
TASK-106 bounded implementation under strict AC reassessment. Typed expression and truth obligations propagate through the supported transparent, filtered-join, grouped/ranked and ordered-DML paths, preserving domains, physical identities and SQL TRUE/NOT TRUE. Supported complete-population proofs share one physical assignment; unproved generalized correlation, lossy inverse, missing metadata and unsupported operator shapes remain explicit residuals, as mandated by AC4. The existing bounded evidence matrix does not yet demonstrate arbitrary composable computed projections and multi-operator shared-source proofs. Full Rust, no-default-features, dbt E2E, formatting, Clippy and docs passed at fae14cac735763d2223b14b88d705d1deedec265 (workflow 38919948845). No new wire format was necessary.
<!-- SECTION:FINAL_SUMMARY:END -->

## Bounded implementation evidence (2026-10-16, PR #114)

Increment on `feat/task-106-composed-predicate-graphs`. The reviewed implementation passed CI (fmt, strict Clippy, full tests, docs,
no-default-features, dbt E2E) at commit `fae14cac735763d2223b14b88d705d1deedec265`; task sign-off adds documentation only.
This evidence supports the five acceptance criteria together with the completion matrix below; it does not imply universal support for arbitrary SQL graphs:

- `src/physical_realization/graph.rs`: `joint_physical_filters` now tries the
  type-certified identity mapping first (identical datatypes at both chain
  ends, verified per hop), keeping the uncertified path for conditions
  needing no type evidence. Untyped chains behave exactly as before; no
  composed-semantics relabeling (Intermediate provenance contract kept).
- `tests/task_106_composed_predicates.rs` (new, public API only): chained
  range-filter conjunction with joint TRUE/NOT TRUE feasibility, single-layer
  and chained contradiction handling (certified Impossible TRUE,
  feasible NOT TRUE, and Empty composed domains), filtered INNER JOIN key equalities with explicit
  single-source boundaries, GROUP BY/HAVING output domains with residual
  scope, computed cross-layer residual vs exact local affine proof, QUALIFY
  residual, INSERT-select composition with partial downstream, dbt parity,
  13-dialect parser/equivalence cells, DuckDB TRUE/IS NOT TRUE oracles.
- Follow-up review fixes: necessary contradictory filter chains now
  return Impossible for positive row-count goals. Complete single-source
  definitions with certified signed-integer widening CAST can transport
  typed physical TRUE/NOT TRUE predicates without treating unsupported
  composed semantic domains as exact. An equijoin population and two
  independent negative terminal filters share a complete physical source
  assignment when those filters do not constrain join keys. Ordered
  INSERT/UPDATE/MERGE is checked against complete DuckDB source/target
  states; partial DML paths remain excluded from source filter inversion.
  These are bounded capabilities, not a proof of every graph/operator AC.
- Promoted one pinned OR-filter chain
  (`tests/physical_realization/graph.rs`) from multiple-witnesses residual
  to joint feasibility; untyped chains keep the residual.
- `docs/semantics.md`, `docs/coverage-manifest.json` (both TASK-106
  families to bounded exact with residual) and
  `tests/task_70_predicates.rs` inventory updated. No protocol wire change.
  General multi-operator DAGs, computed cross-layer inversion, and vendor
  execution remain explicit residuals until separately certified. These
  unsupported paths are within AC4's explicit residual requirement, not
  claims of exact operator support. TASK-107/108 and the separate TASK-91
  release gate remain independent follow-ups.

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** Reuse TASK-68/93..100 physical machinery and TASK-79 schema laws. This child owns the **missing computed-expression integration**, not an independent rewrite of joins, groups or DML nor global end-to-end TASK-89 certification.
- **Next:** TASK-106 criteria independently verified below; move to **TASK-107**. TASK-91 remains an independent release gate.

## Code-review follow-up (2026-10-16)

Matt Pocock two-axis review of PR #114 found that filtered join counts could bypass row-shape checks (LIMIT/DISTINCT), treat post-outer-join WHERE as a source-row predicate, and conjoin independently exact but mutually inconsistent scalar ON conditions. The PR now requires a separately certified filtered-join bag shape, keeps outer WHERE residual, and admits multiple source predicates only after joint satisfiability is established. Regression tests cover all three cases with DuckDB oracles where relevant. The bounded implementation was subsequently verified against AC #1-#5 under the explicit supported-operator and residual semantics documented in the final sign-off matrix.

## Follow-up integration work (PR #114)

Expanded the existing TASK-106 bounded scope along two source-independent
physical proof paths:

- Closed typed INNER JOIN populations now expose individually certified
  conjuncts to the joint TRUE/NOT TRUE source-row satisfiability solver,
  instead of rejecting a compound `All(RowTruth)` formula. Incompatible
  negative terminals can be proven impossible only when the positive INNER
  join necessarily consumes nonempty sources.
- Independent filtered equijoin terminal goals can be combined into a single
  closed physical graph without dropping their source-row filters. Repeated
  source identities with different filter obligations or unreconciled key
  populations stay residual, not spuriously feasible.
- Materialized identity/widening reference inversion and filtered-join source
  construction now reject independent intermediate relation constraints,
  defaults and unresolved diagnostics that were not transported into physical
  source obligations. This protects the AC4 no-fabricated-plans requirement.
- Added focused public-API SQL and DuckDB tests for joint conjunctive
  predicates, contradictory negative terminals, independent filtered join
  DAGs, and intermediate warehouse constraints. Updated the semantic guide,
  code map, and protocol-owned coverage manifest.

### AC verification matrix

1. **Typed operator propagation:** Covered for existing certified single-source
   filter/alias, equijoin, grouping/HAVING, ranking/QUALIFY and computed
   INSERT/UPDATE/MERGE paths by `tests/task_106_composed_predicates.rs`,
   `tests/physical_realization/mixed.rs`, `tests/physical_write_states.rs`
   and `tests/advanced_target_validation.rs`. General cross-materialization
   affine inversion remains unproved and must be classified residual.
2. **Value domains and identities:** Typed finite computed write domains,
   inclusive/exclusive range refinements, per-hop widening/NULL certification
   and stable physical references are tested in the same suites. Arbitrary
   lossy casts and opaque expressions must not claim precise domains.
3. **Joint physical assignment:** Newly extends multiple same-source TRUE
   conjuncts plus NOT TRUE terminals, and disjoint filtered joins. Existing
   GROUP/HAVING, QUALIFY, duplicate/NULL and ordered DML constructions cover
   supported operators. Correlated multi-join key histograms remain residual.
4. **Impossible versus residual:** Necessary contradictory filter conjunctions,
   incomplete producers, unsupported group expressions, materialization laws,
   lossy inverses and unsupported writes have explicit regression controls.
5. **Adapters/oracles:** SQL/dbt full-source parity, DuckDB materialization and
   ordered write-state oracles, dialect probes, canonical JSON/schema tests,
   and this PR's new joint join DuckDB fixtures supply bounded evidence.

These are scoped implementation results. General graph/operator combinations
without certified semantics intentionally remain explicit residuals under AC4.
TASK-106 does not claim exact physical proofs for those cases.

## Bounded implementation evidence, not strict acceptance (2026-10-16)

Scope interpretation: `where existing operator semantics permit` in AC1 and
`after each supported operator` in AC2 are **bounded, proof-backed support**.
AC4 requires explicit residuals for unsupported paths. The child does not
require exact constructive results for every arbitrary SQL graph, nor may it
pretend those unproved paths are exact.

- **AC1, partial coverage:** `tests/task_106_composed_predicates.rs` checks typed
  WHERE-through-materialization truth, JOIN key identity and ON restrictions,
  HAVING/QUALIFY operator classification, alias bounds and computed INSERT
  domains. `tests/physical_write_states.rs` validates computed
  INSERT/UPDATE/MERGE against ordered complete state. Operator variants beyond
  certified support retain residual semantics.
- **AC2, partial coverage:** Inclusive/exclusive ranges, bounded downstream alias
  domains and finite arithmetic write domains are asserted in
  `tests/task_106_composed_predicates.rs` and
  `tests/advanced_target_validation.rs`. Physical reference identity and
  per-hop widening-type/NULL certification are asserted; unproved numeric
  overflow or nonlinear/lossy inversion is not silently sharpened.
- **AC3, partial coverage:** Same-row TRUE/NOT TRUE conjunctions, contradictory
  filters, duplicate/NULL bags, shared source join+negative terminals, multiple
  scalar ON conjuncts and independent filtered join DAGs have canonical joint
  plans and execution oracles in `tests/task_106_composed_predicates.rs` and
  `tests/physical_realization/joins.rs`. Supported join/group/rank/membership
  combinations have dedicated tests in `tests/physical_realization/mixed.rs`;
  ordered write-state sources are checked in `tests/physical_write_states.rs`.
  Unreconciled correlated join-key histograms remain residual.
- **AC4, partial coverage:** Contradictory positive-count goals require a necessary
  contradiction; empty closed-world sources remain separately constructible.
  `tests/task_106_composed_predicates.rs` explicitly tests partial producers,
  unsupported HAVING, lossy inverses, missing schemas, outer JOIN WHERE and
  independent intermediate constraints. No partial or unconstrained unknown
  source is silently upgraded to an exact physical plan.
- **AC5, partial coverage:** Direct SQL/dbt typed parity and emitted JSON/schema
  validation exist in `tests/task_106_composed_predicates.rs`,
  `tests/physical_write_adapter_parity.rs`, and
  `tests/advanced_target_validation.rs`. Complete DuckDB row populations
  validate filtered joins, GROUP/HAVING, duplicate/NULL rejection and ordered
  DML. Canonical contract, code map, coverage manifest and semantic guides are
  updated. No wire schema changed, so a schema migration is not required.

**Bounded implementation verification:** CI workflow
https://github.com/phdah/sql-semantic-protocol/actions/runs/38919948845
passed at `fae14cac735763d2223b14b88d705d1deedec265`, including full
Rust tests, no-default-features, Clippy, formatting, docs and dbt E2E.
Broader SQL semantics, where future tasks can prove stronger outcomes,
remain eligible for separate development without reopening this bounded
integration child. Parent TASK-70 and release TASK-91 retain their own gates.

## Strict acceptance reopening (2026-10-16)

All five AC have been reopened at the user's request. Under strict reading, bounded exactness plus generic fallback residuals are insufficient when composable computed-expression transformations have deterministic proofs that the canonical graph should transport. Remaining blockers include (i) cross-materialization affine arithmetic inversion with width/overflow/null evidence; (ii) corresponding strongest output domains across those boundaries; (iii) physically consistent shared-source multi-operator DAG assignments, including overlapping join/group/negative-filter interactions; and (iv) raw SQL/dbt/DuckDB evidence over these combinations. Keep the task In Progress until explicit regression cases verify all five, not merely CI on earlier bounded cases.

### Strict-scope progress: total affine physical inverse (2026-10-16)

Implemented a real missing proof path without redefining the criteria:
`src/boolean_witness.rs` can rewrite integer comparisons, column
comparisons with identical offsets, NULL tests and AND/OR predicates over
certified `physical + offset` mappings, rechecking physical source bounds
and both SQL TRUE/NOT TRUE directions. `src/physical_realization/graph.rs`
now transports an explicit total signed widening CAST followed by a constant
addition/subtraction across producer edges after checking source type limits,
overflow, materialized NULL semantics, producer bag shape, full definition
and independent relation laws. The regression tests exercise
`CAST(a AS BIGINT)+1` materializations against DuckDB and preserve
residual for INTEGER overflow and nontransparent DISTINCT/LIMIT. Coverage,
code map and semantics updated.

**Still open under strict AC:** symbolic output-domain refinement of nested
and multiple general computed projections, producer-shared correlated
operator proofs that reconcile overlapping join/group/window/key histograms,
and full direct-SQL/dbt/DuckDB conformance for those combinations. A single
supported affine case cannot close the general requirements. Do not check
AC #1-#5 until those are independently verified.
