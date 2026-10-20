---
id: TASK-189
title: Prove common aggregate window functions and deterministic ROWS frames
status: In Progress
assignee: []
created_date: '2026-10-20'
updated_date: '2026-10-20'
labels: []
milestone: m-3
dependencies:
  - TASK-126
references:
  - TASK-74
  - TASK-73
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-74 child 3/5 (added after the original four-child queue).** Prove the five most common aggregate window-function families: `COUNT`, `SUM`, `AVG`, `MIN`, and `MAX`. This is a separate *window evaluation* task, not a repeat of TASK-73's grouped/global aggregates or TASK-126's navigation and bounded-ROWS baseline.

Deliver a source-independent, canonical protocol and physical proof for deterministic partition-wide, cumulative, and moving-window aggregates. The protocol must retain complete physical row identities and the strongest sound typed output domains so sql-tdg and other consumers do not need to reparse SQL or infer frame contents. Preserve the original TASK-74 acceptance criteria. Implement this task in a dedicated PR after TASK-126 has merged; TASK-127 depends on its completion. Do not sign off TASK-74 through this task alone.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `74-C189-P` and `74-C189-N` (positive/membership and NULL/empty/impossible/rejected counterexamples). Freeze all expanded variants and executable expectations before semantic edits.
- **Mandatory functions:** `COUNT(*) OVER (...)`, `COUNT(expr) OVER (...)`, `SUM(expr) OVER (...)`, `AVG(expr) OVER (...)`, `MIN(expr) OVER (...)`, `MAX(expr) OVER (...)`.
- **Mandatory window shapes:** `OVER (PARTITION BY k)` with whole-partition semantics; `OVER (PARTITION BY k ORDER BY stable_key ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)`; and a bounded moving `ROWS BETWEEN 1 PRECEDING AND CURRENT ROW`. Include a deterministic forward-looking `ROWS BETWEEN 1 FOLLOWING AND 1 FOLLOWING` empty-frame case. Demonstrate at least two partitions, deterministic ordering, duplicates, and input NULLs.
- **Independent concrete oracle baseline:** For one partition with stable ordered values `[10, NULL, 30]`, running unbounded-preceding to current-row outputs are `COUNT(*)=[1,2,3]`, `COUNT(expr)=[1,1,2]`, `SUM=[10,10,40]`, `AVG=[10,10,20]`, `MIN=[10,10,10]`, `MAX=[10,10,30]`. Bounded one-preceding-to-current `COUNT(*)=[1,2,2]`, `COUNT(expr)=[1,1,1]`, `SUM=[10,10,30]` and `AVG=[10,10,30]`. An empty forward-only frame on the final row yields `COUNT=0`, `SUM/AVG/MIN/MAX` results `NULL`. A partition-wide frame without `ORDER BY` yields one constant result per partition.
- **Typed semantics:** Preserve NULL exclusion for expression aggregates, row inclusion for `COUNT(*)`, duplicate multiplicity, numeric and DECIMAL precision/scale, fractional `AVG` (not truncated integer), SQL NULL output for `SUM/AVG/MIN/MAX` of an all-NULL or empty frame, `COUNT=0` for an empty frame, inclusive domain bounds, and explicit numeric overflow or engine precision uncertainty. Avoid false `Exact` for coercion, non-finite floating-point inputs, or frame membership where tie ordering is unproven.
- **Conditional inventory owned by TASK-189:** `DISTINCT` within aggregate windows, `FILTER (WHERE ...)` within aggregate windows, non-ROWS implicit ordered-window defaults, ambiguous ordering, and dialect-specific numeric promotion or overflow. Freeze per-dialect parser status and an explicit execution/session disposition; no unverified exactness. `RANGE/GROUPS`, `COLLATE`, and engine-default NULL ordering remain owned by TASK-128. `NTILE` and value/navigation windows remain with their existing owners. `PERCENT_RANK`, `CUME_DIST`, `NTH_VALUE`, statistical and collection aggregates are outside this bounded core and require separately approved inventory changes if needed.
- **Original TASK-74 AC mapping:** #1, #2, #4, #5, #6 (basic `QUALIFY` true/NOT TRUE evidence here; cross-operator composition and multiple window filters remain TASK-127 under #3).
- **Dependency:** TASK-126, which establishes canonical deterministic `ROWS` frame evaluation. TASK-127 must depend on TASK-189, then TASK-128 remains final child; neither depends on TASK-74.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 **Freeze the outcome-first contract before implementation.** Record every mandatory and conditional variant, both fixture IDs, exact SQL, stable source and column identities, frame definitions, parser/dialect classifications, expected complete physical/terminal bags, typed domains including NULL/inclusivity, `TRUE` and `NOT TRUE` witness strength, adapter cells and explicit conditional release disposition in a versioned task-owned fixture matrix linked from `docs/coverage-manifest.json`. Unsupported mandatory variants cannot be silently changed to `Residual`.
- [ ] #2 **Prove all six required spellings / five families.** In the canonical source-independent physical model, produce exact partition-only, cumulative and bounded `ROWS` results for `COUNT(*)`, `COUNT(expr)`, `SUM`, `AVG`, `MIN` and `MAX` on complete known source bags. Preserve duplicates, NULL/non-NULL contributions, source-instance identities, numeric/DECIMAL types, precise finite per-column domains and empty-frame rules. Emit `Impossible` for provable source contradictions and typed `Residual` for genuinely unprovable ties, coercion, overflow or session laws; do not claim mere parser or lineage evidence as physical feasibility.
- [ ] #3 **Independent full-bag oracles.** Check the stated `[10,NULL,30]` example, a distinct second partition, duplicate values, cumulative and moving frames, empty and all-NULL frames, and at least one `QUALIFY` positive/rejected filter using DuckDB-produced complete input/output row snapshots. Public API integration tests must assert each projected value, row identity, source membership, multiplicity, `TRUE` versus `NOT TRUE` semantics and typed domain, not only counts.
- [ ] #4 **Parser/adapter parity and conditional ownership.** Test all 13 exposed parser dialect paths, direct SQL, compiled dbt manifests/catalog, and ODCS schema evidence wherever equivalent. Independently inventory `DISTINCT`, `FILTER`, implicit ordered frames, and numeric engine/session laws with explicit per-cell `Exact`, justified `Residual`, `Impossible` or parser boundary; native engine support is never asserted without executable evidence. Keep TASK-128's advanced-frame ownership intact.
- [ ] #5 **No regression in composition or public contracts.** Verify the new facts compose with existing ranking/navigation and basic `QUALIFY` without broadening row counts or degrading safely established value domains. Update code map, semantic/protocol/API docs and schema/golden snapshots if the public contract changes; test that unsupported forms fail closed rather than silently disappear.
- [ ] #6 **Independent sign-off.** On the final implementation PR head pass `cargo fmt --check`, strict Clippy, full Rust tests, `cargo doc --no-deps`, no-default-features, dbt E2E, and schema/golden checks. Independently audit every frozen mandatory cell and mapped TASK-74 criterion before marking TASK-189 Done; do not merge or close TASK-74 without its own later review.
<!-- AC:END -->

## Next

After TASK-189 is **Done and merged**, implement [TASK-127](task-127%20-%20compose-qualify-and-upstream-grouped-or-joined-row-shaping.md), then TASK-128, then independently sign off parent TASK-74. TASK-91 remains a separate release gate.


## Implementation in progress (2026-10-20)

- Task-owned corrective v4 matrix: [TASK-189 v4](../../docs/task-189-acceptance-freeze.json), with complete partition/running/moving/forward source and terminal bags, typed domains, and a COUNT QUALIFY survivor/NOT TRUE oracle (`74-C189-P5`).
- Canonical implementation: `src/physical_realization/window_bags.rs`; public DuckDB, 13-dialect and adapter tests: `tests/task_189_window_aggregates.rs`, `tests/task_189_window_adapters.rs`.
- **No AC signoff yet.** Basic COUNT QUALIFY TRUE/NOT TRUE is covered. The v3 acceptance-freeze audit corrected two v2 conditional fixtures that were incorrectly labeled as unproved numeric laws despite using exact integer-only cases: `C5` now genuinely exercises repeating 5/3 AVG, while `C6` declares a DECIMAL(10,2) column with fractional inputs. Conditional physical regression checks exercise all 13 dialects and demand fail-closed proofs for parser-accepted variants. The conditional SQL parser matrix has now been executed and frozen as 71 accepted parser paths and 7 parser boundaries (CI run `39445313819`). Parser acceptance is **not** native engine/session attestation; independent vendor/adapter execution oracles are still required. DECIMAL precision/scale, complete QUALIFY/composition, task-specific independent dbt execution, all mandatory AC and independent final-head CI signoff remain blocked. **SemVer disposition:** adding `terminal_row_ids` and `rejected_row_ids` to the public constructible `PhysicalFiniteWindowExact` is a Rust source-breaking change. Release only as a major version (breaking Conventional Commit); do not conflate an unchanged JSON shape with Rust compatibility. No release is performed by this PR. Do not mark Done or approve a Residual for any missing mandatory cell.
- **Continuation implementation (v4):** Added finite binary-exact DECIMAL physical SUM/AVG/MIN/MAX proofs (including terminating fractional AVG), portable numeric precision/overflow guards, nullable SUM/AVG/MIN/MAX QUALIFY NOT TRUE identities, and deterministic ranking/navigation/aggregate composition with full-bag DuckDB regressions. The extended oracle submatrix has five Exact DECIMAL cases (partition, fractional AVG, running, moving and empty forward) and two Residual counterexamples; the six conditional cases now have individually classified native DuckDB test oracles and explicit non-DuckDB vendor/session Residual cells. See `tests/task_189_decimal_windows.rs`, `tests/task_189_decimal_adapters.rs`, and `tests/task_189_native_duckdb_matrix.rs`. Independent dbt Core **executed model** equivalence and live non-DuckDB vendor/session proof are not yet established. The 13 parser-dialect paths alone do not count as runtime session coverage. No premature AC signoff.\n- **Real dbt Core fixture:** A typed DECIMAL seed and executable aggregate model, generated dbt manifest/catalog, real DuckDB source and terminal relation checks, and canonical physical proof are wired to `make dbt-e2e` using `tests/task_189_dbt_core_e2e.rs`. Final-head execution is required before this may be counted as verified acceptance evidence.
- TASK-74 remains open and TASK-127 remains blocked until this child is Done and merged.
