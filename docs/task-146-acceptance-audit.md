# TASK-146 independent acceptance audit

Scope: TASK-80 child 2/4, fixtures `80-C146-P` and `80-C146-N`. This
audit maps the complete bounded INSERT SELECT / CTE / target-reorder /
FK / CHECK inventory to original TASK-80 AC #1..#6. Parent TASK-80 and
release gate TASK-91 are independent and remain unsigned.

## Acceptance inventory

The immutable preimplementation v1 inventory is preserved by Git history;
`docs/task-146-acceptance-freeze.json` v2 extends it with the previously
missing NULL, FK and ordered-write oracles, independent source row identities,
closed TRUE/NOT TRUE partitions, intermediate CTE bags and typed output bounds.
There are 11 mandatory and two conditional physical cases, 169 parser-dialect
cells and 39 SQL/dbt/ODCS evidence cells. The tests use the actual frozen SQL
text and declared complete populations, not a consumer-side SQL interpreter.

## Verification by original child criterion

| AC | Evidence | Gate |
| --- | --- | --- |
| #1 Freeze two fixed fixture IDs, full variants, domains, comparison assumptions, source/intermediate/target bags, conditional dispositions | `docs/task-146-acceptance-freeze.json`; `docs/coverage-manifest.json` | Verify every frozen cell |
| #2 Canonical row-local SELECT and one nonrecursive CTE, column reorder, FK/PK/CHECK, ordered poststates, typed fail-closed unproved stages | `src/analysis/writes.rs`, `src/protocol/writes.rs`, `src/physical_realization/write_states/{projection,operators}.rs`; `tests/task_146_insert_select.rs` | Exact/Impossible/Residual reviewed |
| #3 Complete native DuckDB source, local CTE, prior/final target bags, null and duplicate multiplicities, positive and rejected rows, typed inclusive bounds | `tests/task_146_acceptance_matrix.rs`; `tests/task_146_insert_select.rs` | All 11 mandatory case oracles pass |
| #4 Thirteen parser dialect classifications and direct SQL/dbt warehouse catalog/ODCS v3.2 equivalent schema evidence; vendor boundary fail-closed | `tests/task_146_dialect_adapter.rs` | All 169 parser and 39 adapter cells independently checked |
| #5 fmt, strict default/no-default Clippy, full tests, docs, dbt Core E2E, active wire schema, independent standards/spec review | `tests/task_146_insert_select.rs` and GitHub Actions PR #157 | Final-head all 9 jobs green, all mandatory cells reviewed |

## Conditional and vendor execution boundaries

- A missing complete physical source or target is `Residual`. The local
  CTE cannot be passed to the physical-state solver as an independent source.
- Recursive/multiple CTEs, nested unsupported row shaping, and opaque
  comparisons remain explicit `Residual` or canonical unsupported. A
  certified row-local projection must not silently execute an ORDER BY.
- An attested enforced PK/FK/CHECK contradiction is `Impossible`; missing
  enforcement, collation or type proof cannot be promoted to an Exact state.
- Parser acceptance in 13 sqlparser dialects does **not** certify native
  vendor execution. Only independently executed DuckDB laws are certified.
  TASK-146 owns outstanding native-variant and session evidence; conflict,
  overwrite, returning and upsert belong to TASK-147.
- dbt compiled SELECT models do not execute caller SQL INSERT statements.
  dbt and ODCS parity here attests equivalent warehouse source/target typed
  schema evidence for the same canonical INSERT SQL.

## Final-head release gate

All bounded child criteria remain **open until the actual final PR head CI
and frozen-cell reconciliation have passed**. TASK-80 parent and TASK-91
release approval are not implied by a green TASK-146 child.
