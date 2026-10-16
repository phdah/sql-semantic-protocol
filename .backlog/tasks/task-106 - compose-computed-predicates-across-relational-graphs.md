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
- [ ] #1 **Typed operator propagation:** For the **frozen TASK-106 matrix below**, propagate SQL TRUE/NOT TRUE, typed output domains and physical column references through the enumerated transparent/affine projection, filtered equijoin, grouping/HAVING, ROW_NUMBER/QUALIFY and computed INSERT/UPDATE/MERGE cases. Do not promise arbitrary combinations.
- [ ] #2 **Strongest domain within certified rules:** For each enumerated producer/materialization case, preserve the strongest value set/interval proven by its documented transfer laws, including bound inclusivity, NULL, integer width/overflow, lineage and stable producer identity; emit unknown/residual when the rule's preconditions fail. Do not require novel symbolic solvers.
- [ ] #3 **Joint physical assignments within the matrix:** Prove one consistent complete typed source-row assignment for the enumerated positive and SQL NOT TRUE targets, compatible filtered joins, grouping/ranking combinations and ordered writes. Other correlated DAG shapes must remain typed residuals.
- [ ] #4 **Soundness and fail-closed behavior:** For matrix contradiction/closed-world cases require `Impossible`; for certified constructive cases require complete `Feasible`/exact plans; for the named unsafe/unsupported cases require `Residual` rather than fabricated feasibility. Every constructive plan must honor all represented physical relation constraints.
- [ ] #5 **Finite conformance and sign-off:** Each matrix case must identify existing public-API regression evidence; constructive cases require an independent DuckDB oracle where executable, and applicable SQL/dbt, parser-capable dialect, schema, adapter and documentation parity checks must pass. Full Rust/Clippy/fmt/docs/no-default/dbt E2E CI is the final gate, run after canonical changes.
<!-- AC:END -->

## Frozen TASK-106 acceptance scope (2026-10-16)

The matrix below is the **entire finite proof-coverage obligation for TASK-106**. Rows are independent certified behaviors, **not** an implicit Cartesian product of SQL operators. A case counts only when its named public-API test(s) assert the canonical outcome, proof strength and/or physical source assignment, with an independent DuckDB oracle for supported constructive executions. The coverage manifest and semantic contract retain the implementation-level support boundaries. Existing unsound certified behavior remains a bug even outside these examples.

| ID | Required certified behavior or safe boundary | Expected classification | Public regression evidence |
| --- | --- | --- | --- |
| F1 | Typed conjunctive filters through transparent materializations, including contradictions, positive and NOT TRUE targets | Feasible / Impossible | `contradictory_chain_filters_prove_impossible_true_and_feasible_rejection`, `duckdb_joint_positive_and_negative_rows_match_composed_filters` |
| F2 | Signed, catalog-typed widening CAST and nested constant-affine +/-/* output-domain transport, including negative scales and open bounds | Exact bounded domain; overflow/noncertified expressions residual | `nested_affine_chain_transports_bounded_domain`, `scaled_affine_chain_transports_bounded_domain`, `negative_scaled_affine_preserves_open_interval_boundaries`, `overflowing_scaled_affine_stays_residual` |
| F3 | Computed aliases retain lineage when no certified source inverse exists | Typed residual, no fabricated inverse | `computed_alias_output_domain_stays_unknown_with_lineage`, `division_and_modulo_stay_residual_across_materialization` |
| J1 | Complete filtered INNER equijoin population, independently certified scalar ON/WHERE TRUE conjuncts and same-bag NOT TRUE terminal requests | Feasible for compatible; Impossible for necessary contradiction | `filtered_inner_join_builds_one_complete_typed_population`, `filtered_join_and_negative_terminals_share_one_complete_assignment`, `multiple_join_source_filters_with_incompatible_negative_terminal_are_impossible` |
| J2 | Multiple independent or overlapping filtered equijoins sharing complete physical leaf identities | Feasible only with a jointly satisfying row assignment; otherwise residual | `independent_filtered_join_dags_share_one_joint_closed_physical_plan`, `overlapping_filtered_joins_reconcile_one_shared_physical_source`, `conflicting_overlapping_filtered_join_populations_are_not_fabricated` |
| G1 | Certified GROUP BY/HAVING outputs with complete physical source rows | Exact bounded outcome and feasible supported population | `grouped_having_duckdb_oracle_matches_output_domains`, `supported_grouped_having_reaches_complete_physical_rows` |
| G2 | Identical non-unit grouping histograms and independently certified distinct singleton keys share source rows | Feasible for compatible histograms; residual otherwise | `identical_nonunit_group_histograms_share_one_source_bag`, `distinct_group_keys_share_one_joint_source_assignment`, `nonunit_group_histograms_with_different_multiplicities_stay_residual` |
| R1 | Existing certified ROW_NUMBER/QUALIFY population without unsupported correlated rank predecessors | Feasible supported rank; residual unsafe rank | `supported_qualify_reaches_complete_ranked_rows`, `rank_requiring_predecessor_stays_residual_against_singleton_groups` |
| R2 | Singleton grouping plus compatible same-key partitioned or all-row unpartitioned rank | Feasible for certified matches; residual otherwise | `singleton_group_and_matching_rank_partitions_share_one_source_bag`, `singleton_groups_share_bag_with_unpartitioned_rank`, `rank_partition_on_different_key_stays_residual_against_singleton_groups` |
| JG1 | Distinct matched join key composed with same-key singleton grouping | Feasible with one closed physical source bag; other-key uniqueness not inferred | `join_population_and_group_of_same_key_share_one_complete_source_bag`, `grouping_a_non_join_key_does_not_inherit_join_key_uniqueness` |
| W1 | Computed INSERT SELECT and ordered INSERT/UPDATE/MERGE canonical write states | Exact where catalog/write-state proofs certify; otherwise residual | `computed_insert_select_arithmetic_uses_catalog_domains`, `tests/physical_write_states.rs`, `tests/physical_write_adapter_parity.rs` |
| X1 | Unsupported partial producers, outer-join post-null-extension filters, lossy/narrowing inverses, missing metadata, independently constrained stages and opaque arithmetic | Explicit typed residual; no fabricated physical plan | `partial_unsupported_lossy_and_missing_evidence_stay_residual`, `outer_join_where_must_not_use_null_extended_rows_as_survivors`, `independent_materialized_check_laws_cannot_disappear_during_identity_inversion` |
| A1 | Applicable raw SQL/dbt model and warehouse-catalog adapter parity for canonical domains and closed source proofs | Same canonical outcomes when equivalent evidence is available | `direct_sql_and_dbt_agree_on_composed_range_filters`, `direct_sql_and_dbt_agree_on_nested_affine_domains`, `tests/physical_write_adapter_parity.rs` |
| A2 | Every project dialect on shared parser-accepted variants, with explicit parser rejection boundaries | Same canonical outcomes on parser-accepted syntax | `parser_capable_dialects_share_composed_filter_outcomes`, `parser_capable_dialects_share_nested_affine_domains`, `parser_capable_dialects_share_identical_group_joint_plans` |
| A3 | Stable public protocol/emission contract and accurate operator/adapter evidence | Schema-valid output and documented boundary | `tests/fixtures/protocol-signed-cast.json`, `docs/coverage-manifest.json`, `docs/semantics.md`, `docs/code-map.md` |

**Not required for TASK-106 completion:** general multi-join/correlated DAG solving, rank-over-arbitrary-join results, arbitrary window/aggregate functions, non-linear or multi-column symbolic arithmetic, non-total inverses, all SQL dialect syntax, and every cross-product of the listed cases. These are explicit residual capabilities or follow-up tickets, **not silent claims of support**.

**Closed sign-off rule:** (1) the finite matrix is frozen for this PR; (2) every row has verified code/tests and its expected proof-strength class; (3) existing supported capabilities are not regressed; (4) required final CI passes; (5) AC #1-#5 are checked and status is Done only after evidence is verified. Future new exactness requests become separate tasks, unless fixing a soundness defect in the certified scope. This supersedes the historical *strict acceptance reopening* notes retained below as implementation history; those open-ended requirements are no longer release criteria for TASK-106.

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

Group/rank sharing evidence (2026-10-16): the HEAD `compatible_operator_populations` Groups+Ranked arm shipped without regression coverage. Added 2 tests in `tests/task_106_composed_predicates.rs` at the public `physical_joint_row_count_plan` seam: singleton GROUP BY plus same-key ROW_NUMBER share one bag (2 OperatorPopulation, 1 ClosedWorld, 2 OutputRows, order-independent, DuckDB oracle returns (2,2) on {(0,10),(1,20)}), while a rank partitioned on a different key stays residual. Local full-suite execution remains blocked on this host (no prebuilt libduckdb for aarch64-linux-android); verification via fmt parse, lib check/clippy, and CI workflow after push.

Code-review follow-up (2026-10-16): two-axis review found no scope creep but flagged (i) single-dialect QUALIFY tests, (ii) a `_ => false` catch-all on the domain enum, and (iii) a suspected soundness hole for multi-row rank partitions. Addressed: explicit QUALIFY parser-boundary notes in the tests; exhaustive match (behavior-identical, compiler-guarded for future variants) plus documented soundness argument (singleton keys yield one-row partitions where ROW_NUMBER is identically 1, passing any certified nonzero-limit first-row filter; all Ranked producers enforce those invariants); new `rn > 1` joint residual test pins the predecessor boundary. The suspected hole was a false positive: joint proofs need one bag satisfying both count goals, not reproduction of each example population. CI green on eb4c088 (fmt, strict clippy, full tests, doc, no-default-features, dbt E2E). Declined as out of scope: shared obligation-count test helpers and splitting the positive test (file-wide precedent inlines both).

Full-local-TDD session (2026-10-16, commits e996ca1, 66bebe8, a6d079d): local execution established via the Ubuntu Rust 1.99 toolchain with CARGO_TARGET_DIR under /tmp (Android target cannot run DuckDB tests; Termux cc must not be used for the GNU target). Strict-AC slices, each red-then-green with focused single-file runs:
- Slice 1 (AC2): nested constant-affine CAST/+/- spines and recast-free references to computed producer outputs transport finite domains stepwise at each hop's execution width (additions keep left width, casts must not narrow, overflow fail-closes). Legacy single-hop outcomes verbatim unchanged. Narrowing casts, non-constant operands, unknown producers stay residual. 4 new tests with DuckDB oracles.
- Slice 2 (AC3): identical (groups, rows-per-group) histograms share one bag via block-diagonal pairing across keys; singleton groups share with unpartitioned ranks covering all rows. A same-key-conflict impossibility was designed then rejected as unsound (a HAVING-filtered third bag can satisfy both goals, so residual is correct). 4 new tests with DuckDB oracles.
- Slice 3 (AC5): all-dialect parity for nested affine domains and identical-group joint plans (canonical-equality, Parse-tolerant), direct-SQL/dbt artifact parity for a nested affine model (exact finite set {8..13}); coverage manifest families and semantics guide updated. 3 new tests.
Gates: local fmt, clippy --all-targets --all-features -D warnings, doc, no-default check green; full suite via PR CI green on every slice commit. 11 new tests total, no wire/schema change.
Still open for Done: general multi-join DAGs, correlated sources, general aggregates/windows, rank-over-join-output proofs, nonlinear/multi-column symbolic domains, and the full TASK-101..105 family support matrix with complete SQL/dbt/DuckDB conformance. AC boxes stay unchecked.
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

### Strict graph integration work (2026-10-16, PR #114)

Added source-independent, protocol-owned proof paths beyond the bounded
review baseline:

- `src/composition.rs` transports catalog-independent, *bounded* signed
  integer interval/set outcomes through an explicitly typed widening CAST
  plus constant affine arithmetic on a resolved producer reference. This
  retains open/closed bounds and stable physical lineage, checks width and
  overflow, and never substitutes an arbitrary unbounded producer domain.
- `src/physical_realization/joins.rs` now reuses a single complete
  source-row assignment across multiple overlapping filtered equijoins,
  requiring independently certified non-key TRUE predicates to pass the
  existing coupled satisfiability solver. Contradictory or unproved
  conjunctions remain residual. It additionally composes a certified
  `DistinctMatched` equijoin with singleton GROUP BY on its *same* key.
- `src/physical_realization/composition.rs` deduplicates identical
  group/operator histograms while retaining both terminal output goals.
  Independently certified, singleton/distinct-key group histograms may
  share one physical bag when the complete source row count matches.
  Higher group multiplicities or incompatible keys remain residual.
- `tests/task_106_composed_predicates.rs` exercises overlapping positive
  joins, contradictory filters, reused operator histograms, distinct
  group keys, join/group key correlations, affine producer domains and
  inclusive/exclusive bounds; where supported, DuckDB executes the same
  complete source bags. The semantic guide, coverage manifest and code map
  reflect those stronger contracts.

**Strict completion still requires an independently verified finite
support matrix for all canonical TASK-101..105 expression families and
supported cross-operator compositions**, especially correlated shared
join/group/window/write-state populations that cannot be represented
by the currently certified singleton-group and equijoin patterns.
The acceptance boxes remain unchecked pending that evidence; avoid
representing the mere number of new fixtures or a green bounded build as
universal proof coverage.

### Strict-scope progress: constant-scale affine transport (2026-10-16)

Next missing proof path under strict AC1/AC2, TDD red-then-green at the
public analyze + composed-domain seam (`tests/task_106_composed_predicates.rs`
with DuckDB oracles): `src/composition.rs certified_affine` now transports
constant scales (`CAST(a AS BIGINT) * 2 + 1` over `[5,10]` certifies
`[11,21]`) stepwise at each hop's execution width. Negative scales use the
unary-minus spelling (`* -2` certifies `[-20,-10]`, ends swapped with
inclusivity kept); overflow fail-closes, a zero scale stays residual
(collapse needs nullability evidence), and division/modulo stay residual
(portability). `affine_expression_width` and the semantics guide plus
coverage manifest scope updated to match. 4 new tests; focused file green
(65 passed), clippy/doc/no-default green.
Still open for Done: multi-column/nonlinear symbolic domains, general
multi-join DAGs, correlated sources, general aggregates/windows,
rank-over-join proofs, and the full family support matrix.
