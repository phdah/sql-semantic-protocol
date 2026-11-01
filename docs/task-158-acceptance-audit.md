# TASK-158 acceptance audit (2026-11-01)

**Status: In Progress, not signed off.** PR #179 implements the typed core and new acceptance oracles. This record deliberately distinguishes implementation from verified release evidence. Parent TASK-83 remains open; TASK-159 and TASK-160 own subsequent lifecycle and full cross-object graph closure.

## Original child AC mapping

| AC | Implemented evidence | Remaining verification |
| --- | --- | --- |
| #1 Frozen complete matrix | v1 and v2 retained; active v3 eight-case typed domains, exact row-frequency and truth inventories, two fixed fixture ID families and 104 frozen shared-syntax dialect AST cells | Full mandatory/conditional SQL, dbt, ODCS, every cell's source+target typed domains, TRUE/NOT TRUE and native vendor decisions must be reconciled |
| #2 Canonical source-independent typed ALTER | `src/analysis/alter_columns.rs`, `src/protocol/writes.rs`, `src/physical_realization/write_states.rs`; complete-bag ADD/DROP/RENAME and identical-TYPE projection; null/default, duplicate preservation, before/after exact schema; typed cast residual | Key/FK/check/default preservation when provable, constrained schema rewrites, transaction and lifecycle precedence, unsupported semantics and post-ALTER SELECT composition remain bounded; TASK-160 is the whole-program closure |
| #3 Physical exact/negative/full bag oracle | `tests/task_158_alter_column_snapshot.rs`, `tests/task_158_ordered_alter.rs`, `tests/task_158_frozen_cases.rs`; native DuckDB scripted CREATE/INSERT/ALTER, preserved bag frequencies and schema ordinals; Impossible/Residual checks | Every frozen row must have independent native source/intermediate/target and domain/dual-truth oracle, including constrained cases, beyond the existing selected scripts |
| #4 Dialects + SQL/dbt/ODCS | `tests/task_158_dialect_matrix.rs` 13x8 shared syntax, `tests/task_158_adapter_parity.rs` direct/dbt/ODCS equivalence of canonical outcomes for representative ALTER variants | Vendor-specific AST/runtime/cast/constraint boundaries, complete adapter matrix, evidence provenance and all conditional release dispositions |
| #5 Review/CI/signoff | `tests/task_158_wire_contract.rs`, active protocol JSON schema, code map, doc and unmerged draft PR | Final-head nine-job CI verification, fixed-point standards/spec review, every child AC check and parent mapping; no intermediate release |

## Scope and proof boundaries

The complete-bag function proves schema and row projection only when the caller or previously executed plain `CREATE TABLE` supplies a complete, constraint-free relation. It never assumes unknown constraints do not exist. For type conversion, the typed before and attempted post schemas remain available in a `Residual` proof. Unsupported SQL statements block ordered execution; accepted AST syntax is not an independent vendor engine-law oracle. Metadata sources contribute schema only, not lifecycle or physical row evidence.

The 13-dialect matrix presently certifies **SQL parser and canonical AST normalization** for common forms, not vendor execution. The adapter tests certify equivalent canonical row and column outcomes, excluding intentionally different provenance categories. Test logs and review must establish exact counts before publishing mandatory-cell verification.

## Signoff policy

The following flags remain false until supported by actual final-head evidence: `child_signed_off`, `mandatory_cells_verified`, `parent_signed_off`. TASK-83 #2/#4/#5/#6 cannot be checked through TASK-158 alone. A future reviewer must not reinterpret `Residual` as a mandatory Exact or erase a required variant to gain a green test.
