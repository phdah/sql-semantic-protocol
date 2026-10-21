# TASK-189 pre-sign-off acceptance audit

**Scope:** TASK-189 only, child 3/5 of TASK-74. This is the implementation
evidence for an independent final review, not a merge or release approval.
Task status remains **In Progress** until that review verifies the final PR
head. TASK-74 stays open, TASK-127 stays blocked, and TASK-128 retains advanced
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

## Final review checklist

Review the actual final-head CI run and inspect regressions, including any
changes made after this audit. Confirm the reviewed 59 vendor/session Residual
cells do not imply native support. Only then check TASK-189's AC and change its
status to Done; keep TASK-74 open and do not merge until the user approves.
