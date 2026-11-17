# TASK-179 acceptance audit

Status: **In Progress; no acceptance criterion signed off**. This file records the first bounded implementation slice. It is not a TASK-88, TASK-179 or v3 release approval.

## Initial executable fixture inventory

- `docs/task-179-acceptance-freeze.json`: versioned 14-case / 182 parser-cell initial matrix (seven positive and seven comparative/rejected controls).
- `tests/task_179_vendor_law_oracles.rs`: independent fixed DuckDB tables, complete typed source bags and selected, comparative or rejected terminal IDs, including transaction poststate, covering six mandatory *family headings*.
- Identical portable NULL predicate control across 13 sqlparser dialects, without claiming NULL-sort order or vendor-specific evaluation laws.
- No other vendor execution is attested. Every parser cell and non-DuckDB runtime law remains explicitly unapproved. Original TASK-177 IDs are references to related variants, **not** claims that a different SQL spelling was executed.

## Remaining work to finish TASK-179

| AC | State | Required remaining evidence |
| --- | --- | --- |
| 1 | Incomplete | Freeze **all** named family variants and review every dialect/parser/adapter/terminal cell, including ILIKE/SIMILAR/regex, locale collations, fixed-char padding, coercion errors, NaN, overflow, DST, session defaults, MERGE multiplicity, DML autocommit and engine identifier folding. The current 14 cases are only an initial bounded subset. |
| 2 | Incomplete | Verify strongest safe canonical output domains, NULL/bound inclusivity, sound TRUE/NOT TRUE and Impossible/Residual physical witnesses; implement any identified missing semantics in their canonical owners. |
| 3 | Partial | The initial DuckDB controls execute on fixed physical tables and check selected/rejected row occurrence IDs or complete transaction after-state. Initial full source-bag snapshots are now asserted alongside selected/rejected terminal IDs. Add independently reviewed source/terminal snapshots and counterexamples for all remaining mandatory named variants, including engine-native negative/error cases. |
| 4 | Incomplete | Reconcile every parser identity against actual parse success/rejection, test direct SQL / compiled dbt catalog/manifest / ODCS representable facts, and record review dispositions and real external-engine/session evidence or named unapproved residuals. |
| 5 | Incomplete | Require final-head fmt, strict Clippy, full and no-default Rust tests/docs, dbt E2E, active schema/goldens, pinned sql-tdg compatibility, independent review of each required cell. Do not check ACs or mark Done before verification. |

## Explicit boundaries

- DuckDB's case-insensitive quoted identifier matching, `NOCASE`, signed-zero and implicit cast behavior may differ from other engines.
- A fixed UTC `TIMESTAMPTZ` equality does not prove default session timezone, DST transitions, or implicit local-time casts.
- A one-source UPDATE + COMMIT/ROLLBACK does not prove MERGE duplicate matches, DDL auto-commit or cross-engine isolation.
- The 13-dialect shared NULL predicate test proves portable canonical analysis only, not that any warehouse executed the syntax.

## Review gate

Matt Pocock's `implement` process uses narrow behavioral checks and follows with a code review. This slice is intended for implementation CI and review before expansion. **TASK-197 is merged; TASK-179 is In Progress; TASK-191 and TASK-180 remain blocked.**
