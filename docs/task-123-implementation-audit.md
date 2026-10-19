# TASK-123 implementation audit (in progress)

Owner: TASK-123. Parent: TASK-73 (remains open).
PR: https://github.com/phdah/sql-semantic-protocol/pull/133

## Frozen cases and semantic review

- `73-C123-P-join`: expected complete INNER JOIN + WHERE selected pair bag contains five physical pairs, with the key=1 group contributing four pairs and the key=2 group contributing one. HAVING >= 2 retains only key=1, with terminal `(k=1, row_count=4, count_x=2)`.
- `73-C123-N-rejected`: same complete source populations, HAVING COUNT(*) > 4 rejects both groups and produces an empty terminal value domain. The SQL predicate classifications must not treat NULL equality as TRUE.
- `73-C123-P-empty-global`: an empty left source yields no join pairs but global COUNT(*) still emits exactly one row with zero.
- `GROUPING SETS`, `ROLLUP`, `CUBE`, `GROUP BY ALL`: only explicit parser/analysis or typed Residual classification. No native/session grouping identity, subtotal NULL, or GROUPING(...) bit law has been certified. All conditional judgments remain owned by TASK-123.

All physical sources, exact required domain values, dialect identities and adapter paths are recorded in `docs/task-123-acceptance-freeze.json`.

## Standards review

The implementation is in a focused physical engine, with no SQL reparsing, dependencies or protocol JSON contract mutation. It reuses TASK-122's shared predicate shape and SQL three-valued truth rules. Input physical rows are validated by the canonical closed-world schema validator; unproved expressions and decimal/overflow laws return Residual. Public docs, code map, and public API exports have been updated.

**Pending:** cargo fmt, Clippy, tests, docs and all workflow checks. No local Rust toolchain was available to execute them in this session.

## Spec review and signoff blockers

- TASK-123 AC #1: task-specific finite matrix recorded, but parser classifications, mandatory dialect/adapter cells and conditional release dispositions are not independently signed off.
- AC #2: implementation covers directly joined two-source INNER JOIN, WHERE, one integer GROUP key or global aggregate, HAVING and supported integer aggregate families. General projection-layer compositions, chained/multi-join aggregate producers and grouping extensions are not proven.
- AC #3: independent complete DuckDB source/terminal oracle tests and source-pair/group identity tests were added but have not passed CI.
- AC #4: 13-dialect direct-SQL and SQL/dbt/ODCS parity tests added; no validated green evidence on current PR head. Native vendor engines are not claimed.
- AC #5: no passing final-head CI or independent acceptance-cell signoff; **TASK-123 remains In Progress**, PR remains draft and **TASK-73 remains open**.

Do not check any remaining task AC, mark Done or merge based on this document.
