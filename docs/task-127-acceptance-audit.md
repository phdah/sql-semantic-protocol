# TASK-127 independent acceptance review (PR #138)

**Scope:** the bounded TASK-127 child (fourth of five TASK-74 children), not the complete TASK-74 parent or a protocol 3.0.0 release approval. TASK-74 retains all original unchecked acceptance criteria, and TASK-128 owns additional vendor/frame/session and `ranked_orders.sql` gates.

**Review date:** 2026-10-21. Frozen matrix: `docs/task-127-acceptance-freeze.json` (created before semantic implementation). The original frozen SQL, physical input bags, terminal bags, typed domains, and positive/nonmembership fixture IDs have not been changed during this iteration; only observed evidence/dispositions were closed. Independent verification uses executable assertions rather than the matrix's review status.

## Independently checked fixed-case matrix

| Case | Required outcome | Canonical physical proof and Oracle | Adapter/parsing disposition |
| --- | --- | --- | --- |
| P1 | Exact ordinary ROW_NUMBER QUALIFY, two partitions with complete rejections | `tests/task_127_qualify_bags.rs` validates public API row identities, full bag and DuckDB output | SQL/dbt/ODCS Exact |
| P2 | Exact conjunction of multiple ranking comparisons | Same public API and independent DuckDB exact bag/typed domain regression | SQL/dbt/ODCS Exact |
| P3 | Exact RANK/DENSE_RANK comparison and membership | Same, including SQL peer grouping | SQL/dbt/ODCS Exact |
| N1 | Exact empty result with complete NOT TRUE rejected witnesses | Same, independently verified zero-row output domain | SQL/dbt/ODCS Exact |
| P6 | Exact tied RANK/DENSE_RANK multiplicity, with duplicate-order peers | Same, full source and bag identities | SQL/dbt/ODCS Exact |
| N2 | Typed **Residual** for underordered ROW_NUMBER physical peer identity | `tests/task_127_qualify_bags.rs` asserts Residual, not arbitrarily selected winning rows | SQL/dbt/ODCS Residual |
| P4 | **Exact** two-source INNER equality JOIN, then QUALIFY ROW_NUMBER | `src/physical_realization/window_composition.rs` constructs each joined pair from the full attested `l` and `r` bags, including three ON-rejected Cartesian pairs, two selected joined rows and one QUALIFY-rejected pair. `tests/task_127_upstream_native_oracles.rs` independently executes DuckDB JOIN, read-back source bags, membership/ranks/terminal. `tests/task_127_qualify_bags.rs` asserts identical exact source pair IDs, contributors, output bag and finite typed domains | `tests/task_127_composed_adapter_parity.rs` verifies SQL/dbt/ODCS |
| P5 | **Exact** derived GROUP BY `k`/COUNT(*) producing two grouped rows, then QUALIFY ROW_NUMBER | `src/analysis/queries.rs` retains certified canonical derived COUNT(*) producer metadata. The composed evaluator consumes all six physical `s` rows into two groups with complete contributor identities, one TRUE survivor, one NOT TRUE group, and exact typed COUNT/terminal domains. `tests/task_127_upstream_native_oracles.rs` independently executes full DuckDB aggregate/rank/terminal | `tests/task_127_composed_adapter_parity.rs` verifies SQL/dbt/ODCS |

Additional adversarial `tests/task_127_qualify_bags.rs` cases certify SQL NULL JOIN rejection, NULL GROUP BY key as its own group, unproved underordered joined peers as Residual, unsupported COUNT(expr) producer as Residual, unsupported LEFT JOIN as Residual, and contradictory source datatypes as Impossible. No derived or joined row is mislabeled as an independently controlled physical table: `PhysicalFiniteWindowExact` exposes separate controllable physical source names and complete source row IDs, materialized producer IDs, upstream JOIN rejects, QUALIFY rejects, and full contributor mapping.

All **8** cases traverse all **13** parser/canonical QUALIFY entrypoints (`tests/task_127_dialect_classifications.rs`, **104 classified cells**), with all observed cells `parsed`. All 8 cases have **3** corresponding adapter evidence paths (**24 verified cells**), with seven mandatory cases Exact and one conditional case Residual (`tests/task_127_adapter_parity.rs`, `tests/task_127_composed_adapter_parity.rs`). `tests/task_127_acceptance_gate.rs` checks the machine-readable register, exact cell counts, distinct IDs, fixture contents and conditional release ownership. **Parser/canonical parity does not certify native execution for 13 database engines; only DuckDB is an executable native oracle here.**

## Original TASK-74 AC scope mapping

- **#1, ranked QUALIFY composition:** ROW_NUMBER/RANK/DENSE_RANK within P1–P6 and N1/N2, including conjunction and per-partition proof. Other parent function families remain the responsibility of TASK-125/126/189/128.
- **#3, survivor/rejection composition:** P4 JOIN and P5 aggregate each have a complete physical source assignment, intermediate producer bag and final TRUE/NOT TRUE classification. Other parent cases remain for TASK-128.
- **#4, nondeterministic ordering:** N2 and the adversarial joined-tie case fail closed; RANK/DENSE_RANK preserve peer equivalence without assuming deterministic ROW_NUMBER peer IDs.
- **#5, DuckDB and ranked outputs:** independent full physical-source and SQL terminal execution for the frozen portable child cases. TASK-128 retains `ranked_orders.sql` and native vendor-law boundaries.
- **#6, typed, dialect, adapter and negative evidence:** public API exact/residual/impossible classifications; complete integer/bounded/null domains; 104 parser cells; SQL/dbt/ODCS parity with all frozen IDs. Parent protocol-schema/release requirements are not signed off here.

## Reviewed conditional boundaries, owned by TASK-127

1. **Peer/tie order:** ROW_NUMBER without total ORDER BY has no certified physical winner. Typed Residual, not an invented exact member. RANK/DENSE_RANK with ties are supported.
2. **Native vendor/session QUALIFY:** parsing succeeds in all 13 dialect entrypoints, but no evidence of execution compatibility across 13 actual engines or implicit collation/NULL ordering. Retain Residual outside the certified portable DuckDB oracle.
3. **Unsupported producer/output expressions:** only the explicitly certified bounded two-source INNER equality JOIN and derived single-key COUNT(*) GROUP BY paths are Exact. Other GROUP BY expressions, non-COUNT aggregates, LEFT JOIN, computed output projections, missing physical types and unbounded operator structures remain typed Residual (or Impossible for enforced schema contradictions).

All three residual dispositions are owner-reviewed in the fixture matrix with `release_review=reviewed_bounded_residual_pending_release`. They are not used to excuse any named mandatory exact case.

## Five independent child acceptance decisions

1. **AC #1, frozen matrix:** both named fixture IDs and eight variants, exact original SQL, full input/output bags, physical and output identities, typed domains and inclusivity, 104 parser and 24 adapter cells, explicit conditional release ownership: **verified**.
2. **AC #2, canonical physical model:** exact JOIN and GROUP source-attested intermediate producers, SQL NULL/equality/group semantics, stable source/producer IDs, complete rejection and output domains, Exact/Impossible/Residual: **verified**.
3. **AC #3, independent end-to-end oracle:** complete DuckDB source read-back, joined-pair identities, grouped contributors, window ranks, terminal bags and typed bounds, plus NULL/peer/unsupported counterexamples in public API: **verified**.
4. **AC #4, adapters and dialects:** all 104 canonical parser cells independently classified; 24/24 adapter cells checked for exact or residual parity, with session/native engine limits recorded: **verified**.
5. **AC #5, final CI and metadata signoff:** **pending final PR-head CI verification**. All format, strict Clippy, full default and no-default Rust tests, docs, dbt Core E2E and existing schema/golden regression suites must pass on the last signoff commit before this can be marked Done or merged.

**Release approval:** false. **Parent TASK-74:** open and unchanged. Neither task metadata signoff nor the review of this child is evidence that every TASK-74 function, engine or frame is complete.
