# TASK-189 independent acceptance sign-off

**Scope:** TASK-189 only, child 3/5 of TASK-74. This is the independent final
acceptance review, not a merge or 3.0.0 release approval. TASK-74 stays open,
TASK-127 remains blocked until this PR merges, and TASK-128 retains advanced
frame/session ownership.

## Frozen evidence

The [v5 task-owned matrix](task-189-acceptance-freeze.json) contains **10
mandatory Exact complete source/terminal bags**: 5 INTEGER cases (partition,
running, moving, forward, COUNT QUALIFY) and 5 finite DECIMAL cases (partition,
fractional AVG, running, moving, empty forward). Every mandatory case freezes
SQL, complete source identities, terminal values and typed inclusive domains,
and 13 parser + 3 direct/dbt/ODCS adapter obligations. The matching integration
oracles live in `tests/task_189_window_aggregates.rs`,
`tests/task_189_decimal_windows.rs`, `tests/task_189_window_adapters.rs`
and `tests/task_189_decimal_adapters.rs`. The v5 completeness/ownership
invariant is executable in `tests/task_189_conditional_parser_matrix.rs`.

All **6 conditional SQL fixtures** C1-C6 and **2 numeric counterexamples**
N2-N3 are separately classified as `Residual` in the portable physical API.
These are *not* mandatory Exact cells. All 13 parser dialect statuses and
direct/dbt/ODCS adapter expectations are frozen; no missing mandatory variant
may be relabeled to Residual to pass a gate.

## Native runtime evidence and limitation

- `tests/task_189_native_duckdb_matrix.rs`: six conditional DuckDB execution
  oracles, independent source and terminal snapshots; tied peers retain only
  the provable partial witness.
- `tests/task_189_decimal_windows.rs`: native DuckDB precision and
  accumulator counterexamples, full source/terminal bags.
- `tests/native/test_task_189_sqlite_native.py`: second native engine, invoked
  in the CI dbt Core E2E job. SQLite executes five conditional output laws and
  both numeric counterexamples; its native `DISTINCT OVER` rejection is an
  *engine* boundary despite sqlparser accepting that SQL.
- The other **59 parser-accepted non-DuckDB/non-SQLite vendor/session cells**
  remain explicitly unexecuted `Residual`. They are NOT certified as native
  supported, tested or portable. It is safe to accept these *conditional*
  residuals because no exact native portability is claimed, but this cannot
  excuse a missing mandatory exact cell. A later native engine capability
  requires independent executable engine/session oracles before promotion.

The real dbt Core seed/model executes in a DuckDB warehouse and is checked
against the compiled manifest, catalog, complete physical source/terminal
bags, source identities and independently frozen typed domains
(`tests/task_189_dbt_core_e2e.rs`). ODCS is a typed **schema evidence
adapter**, never an executable SQL engine; equivalent ODCS canonical physical
proof is asserted through the adapter tests.

## Original child AC implementation evidence

| Child AC | Evidence to verify at final review |
| --- | --- |
| #1 Freeze | v5 has 10 mandatory Exact plus 8 explicitly reviewed Residual cases, SQL/identities/domains, 13 parser paths, 3 adapters, owner and session disposition; guarded in `task_189_conditional_parser_matrix.rs` |
| #2 Core | `window_bags.rs` uses closed-world source bags, deterministic ROWS membership, exact integer/finite-decimal laws, all six required spellings, impossible schema contradictions and fail-closed ties/coercion/overflow/rounding |
| #3 Independent oracles | Complete DuckDB integer/DECIMAL input/output bags, two partitions, duplicates, NULL, empty frames, positive/NOT TRUE QUALIFY and exact value domains in `task_189_window_aggregates.rs` and `task_189_decimal_windows.rs` |
| #4 Parity/conditionals | 13 parser paths, 3 canonical adapters including both numeric Residuals; executed native DuckDB/SQLite conditional matrix; all other vendor cells explicitly unverified Residual rather than claims of support |
| #5 Composition/contracts | `task_189_window_aggregates.rs` includes ranking/navigation/window composition, positive and rejected aggregate QUALIFY, ambiguous-peer Residual. Rust `PhysicalFiniteWindowExact` gained two publicly constructible fields and needs a **major** version upon release; no serialized schema change |
| #6 Independent sign-off | Verify final-head formatting, strict Clippy (both feature sets), full Rust test suites, docs, dependencies, real dbt E2E and SQLite oracle in GitHub Actions, then review this audit and mark the six child AC Done. Parent TASK-74 must remain open |

The child maps to TASK-74 AC #1, #2, #4, #5 and #6 only. The
upstream grouped/joined QUALIFY and multiple-window-filter proof is TASK-127,
and advanced RANGE/GROUPS, COLLATE, NULL defaults and NTILE are TASK-128.
Neither may be silently marked complete here. This PR is not the 3.0.0 release
gate and carries no independent release approval.

## Independent final acceptance review (2026-10-21)

**Disposition: AC #1 through #6 independently verified for TASK-189's
explicitly bounded exact core and reviewed conditional Residual inventory.**
Reviewed the canonical physical evaluator and public source-bag identities,
all ten frozen Exact cases, 13 parser paths, three canonical adapters,
conditional and numeric Residual classification, parent TASK-74 scope mapping,
and the actual GitHub Actions final-head test logs. None of the ten required
Exact cases was reclassified as Residual.

**Final verified semantic head:**
`fac7855209b1c094dd110d4cb40955daa500bdf2`.
**CI run:** https://github.com/phdah/sql-semantic-protocol/actions/runs/39562505282
(completed successfully, **9/9 jobs**). Verified `Rust / fmt`, strict
`Rust / lint`, `Rust / test`, `Rust / doc`, no-default-features lint,
tests, docs and dependencies, and executed dbt Core E2E.

Task-specific log assertions confirmed:
- `exact_mandatory_cells_and_reviewed_residuals_are_distinct`, which guards
  10 Exact and 8 reviewed Residual obligations: passed.
- `every_conditional_native_session_cell_has_a_conservative_owned_disposition`:
  passed, including 71 parser-accepted/7 parser-boundary cells.
- `fixed_point_decimal_results_agree_across_direct_compiled_dbt_and_odcs`:
  passed, including both numeric Residual counterexamples.
- SQLite 3.45.1 native suite: all four tests passed, covering eight independent
  runtime/boundary cases.
- `real_compiled_dbt_decimal_windows_have_exact_physical_and_warehouse_bags`:
  passed after executing the real dbt model and warehouse oracle.
- The standard Rust test suite also covers complete DuckDB bags, schema/golden
  checks, exact source/terminal identities, nullable QUALIFY, and mixed
  ranking/navigation/aggregate regressions. The full default and
  no-default-feature suites passed.

### Child AC decision

| AC | Independent decision |
| --- | --- |
| #1 | **PASS:** versioned v5 freeze inventories exact and conditional SQL, stable source/column identities, frames, complete oracle bags, true/not-true rows, inclusive typed domains and reviewed parser/session/adapter dispositions |
| #2 | **PASS:** source-independent closed-world physical evaluator returns `Exact` for the six mandatory spellings and four frame shapes, `Impossible` for contradictory physical sources and `Residual` for unsupported numeric/session/order cases |
| #3 | **PASS:** independently executed DuckDB full input/output oracles, duplicate/NULL/empty and second-partition witnesses, complete row membership, typed domains and QUALIFY survivor/rejection evidence |
| #4 | **PASS:** 13 parser dialect paths, direct/compiled dbt/ODCS parity for exact and Residual numeric cases, conditional inventory with DuckDB/SQLite runtime evidence and explicit unexecuted vendor/session Residual cells; no universal native support claim |
| #5 | **PASS:** mixed ranking/navigation/aggregate plus bounded QUALIFY regressions and fail-closed peer cases; source-breaking `PhysicalFiniteWindowExact` additions require a future major release, protocol JSON schema is unchanged |
| #6 | **PASS:** independent original-AC crosswalk and code/test inspection plus 9/9 successful final semantic-head CI jobs; all child obligations checked |

**Portability limitation deliberately retained:** the 59 parser-accepted
vendor/session cells outside DuckDB and SQLite are conditional unexecuted
`Residual`, not native-supported exact claims. The reviewer accepts this
bounded conditional disposition; any later expansion to `Exact` requires
independent native vendor/session execution. This does not excuse missing
mandatory exact semantics and does not certify advanced TASK-128 frames or
upstream TASK-127 composition.

**Post-sign-off metadata note:** The status, checkboxes, this audit record,
and the coverage manifest are documentation-only changes after the reviewed
semantic head. Verify CI again on the resulting PR head before merging.
TASK-189 may be marked **Done pending merge**; TASK-74 remains To Do.
