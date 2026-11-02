# TASK-158 acceptance audit (2026-11-01)

**Status: In Progress, not signed off.** PR #179 implements the typed core and new acceptance oracles. This record deliberately distinguishes implementation from verified release evidence. Parent TASK-83 remains open; TASK-159 and TASK-160 own subsequent lifecycle and full cross-object graph closure.

## Original child AC mapping

| AC | Implemented evidence | Remaining verification |
| --- | --- | --- |
| #1 Frozen complete matrix | v1 and v2 retained; active v3 eight-case typed domains, exact row-frequency and truth inventories, two fixed fixture ID families and 104 frozen shared-syntax dialect AST cells | Full mandatory/conditional SQL, dbt, ODCS, every cell's source+target typed domains, TRUE/NOT TRUE and native vendor decisions must be reconciled |
| #2 Canonical source-independent typed ALTER | `src/analysis/alter_columns.rs`, `src/protocol/writes.rs`, `src/physical_realization/write_states.rs`; complete-bag ADD/DROP/RENAME, identical-TYPE and provably lossless signed integer widening; null/default, duplicate preservation, before/after exact schema; typed cast residual | Key/FK/check/default preservation when provable, constrained schema rewrites, transaction and lifecycle precedence, unsupported semantics and post-ALTER SELECT composition remain bounded; TASK-160 is the whole-program closure |
| #3 Physical exact/negative/full bag oracle | `tests/task_158_alter_column_snapshot.rs`, `tests/task_158_ordered_alter.rs`, `tests/task_158_frozen_cases.rs`; native DuckDB scripted CREATE/INSERT/ALTER, preserved bag frequencies and schema ordinals; Impossible/Residual checks | Every frozen row must have independent native source/intermediate/target and domain/dual-truth oracle, including constrained cases, beyond the existing selected scripts |
| #4 Dialects + SQL/dbt/ODCS | `tests/task_158_dialect_matrix.rs` 13x8 shared syntax, `tests/task_158_adapter_parity.rs` direct/dbt/ODCS equivalence of canonical outcomes for representative ALTER variants | Vendor-specific AST/runtime/cast/constraint boundaries, complete adapter matrix, evidence provenance and all conditional release dispositions |
| #5 Review/CI/signoff | `tests/task_158_wire_contract.rs`, active protocol JSON schema, code map, doc and unmerged draft PR | Final-head nine-job CI verification, fixed-point standards/spec review, every child AC check and parent mapping; no intermediate release |

## Scope and proof boundaries

The complete-bag function proves schema and row projection only when the caller or previously executed plain `CREATE TABLE` supplies a complete, constraint-free relation. It never assumes unknown constraints do not exist. Lossless signed integer widening retains original scalar values and the widened typed schema. Unsupported conversions retain typed before and attempted post schemas in a `Residual` proof. Unsupported SQL statements block ordered execution; accepted AST syntax is not an independent vendor engine-law oracle. Metadata sources contribute schema only, not lifecycle or physical row evidence.

The 13-dialect matrix presently certifies **SQL parser and canonical AST normalization** for common forms, not vendor execution. The adapter tests certify equivalent canonical row and column outcomes, excluding intentionally different provenance categories. Test logs and review must establish exact counts before publishing mandatory-cell verification.

## Signoff policy

The following flags remain false until supported by actual final-head evidence: `child_signed_off`, `mandatory_cells_verified`, `parent_signed_off`. TASK-83 #2/#4/#5/#6 cannot be checked through TASK-158 alone. A future reviewer must not reinterpret `Residual` as a mandatory Exact or erase a required variant to gain a green test.

## Review follow-up (2026-11-02, v4)

This PR now includes regression-proven persistence of authoritative ALTER ADD scalar
defaults through later INSERT and RENAME, dropping stale default metadata on
DROP/re-ADD, and NULL backfill for omitted nullable ALTER-added columns. It
retains constraint/source provenance in the execution-only canonical bundle;
unproved generated/identity or opaque-default transitions are residual, not
guessed. Exact RENAME duplicate-target classification and conditional wire
schema requirements have also been fixed.

The native DuckDB matrix now asserts actual pre-ALTER bags for the executable
frozen cases, both portable positive and contradiction snapshots, and separate
cast/PRIMARY KEY conditional runtime evidence. The v4 matrix retains v1-v3,
pins 104 AST parser cells and 27 representative SQL/dbt/ODCS prestate and
constraint-boundary comparison cells. Parser acceptance and adapter metadata
do not imply vendor DDL execution laws.

Remaining signoff gates are final-head CI for v4, a genuinely independent
standards/spec review, and maintainer-approved dispositions for cross-vendor
cast/key/transaction semantics. TASK-158 must remain In Progress until
those gates are independently certified. TASK-83 remains open regardless.


**dbt compiled SQL boundary:** A compiled dbt model is SELECT-backed and
cannot be used as an arbitrary ALTER DDL program. A deliberately invalid
model containing ALTER is now asserted to fail explicitly. The meaningful
parity guarantee is the canonical replay of independent SQL ALTER inputs
against equivalent dbt catalog/manifest or ODCS typed prestate evidence;
neither adapter is claimed to execute database migrations.


## Independent-evidence remediation (2026-11-02, v4)

The first eight-case acceptance matrix conflated a nonempty constraint inventory
with an actually enforced key: the `83-C158-N-key` test supplied an empty
constraint set and happened to return Residual. The public-API acceptance
test now supplies an explicit `PrimaryKey(key)` with SQL DDL provenance and
Enforced status. It cannot pass merely because a caller supplied any nonempty
inventory.

Every frozen case now has complete *pre*-ALTER typed domain frequencies,
NULL membership and inclusive integer bounds, asserted independently of its
poststate typed domains. Exact transformations assert occurrence-preserving
row order and the complete poststate bag, and Impossible outcomes verify
that the native DuckDB source bag and schema remain unchanged. Conditional
Residuals keep a third `unknown_ordinals` partition. They are not falsely
promoted to proven NOT TRUE outcomes.

Four additional frozen, owner-attributed vendor parser boundaries cover
`ADD IF NOT EXISTS`, `DROP IF EXISTS`, `DROP ... CASCADE`, and
`ALTER TYPE ... USING`. Every exposed dialect either rejects parsing
or produces an explicit canonical Unsupported statement, never a false
Exact operation. These are parser and structural-law classifications,
not engine-level support claims.

### Conditional release-disposition proposals

| Variant | Portable claim | Evidence | Owner | Disposition |
| --- | --- | --- | --- | --- |
| INTEGER to VARCHAR type change | `Residual`, typed before/attempted schema | DuckDB concrete-row cast succeeds, but no portable cast/constraint law is proved | TASK-158 | Accept only as a documented conditional residual; additional engine laws require separately reviewed evidence |
| DROP enforced PRIMARY KEY | `Residual`, no committed after-state | Independently enforced `PrimaryKey` evidence and native DuckDB rejects the operation | TASK-158 | Accept only as a documented conditional residual; full constraint/dependency rewrites are not claimed |
| Vendor DDL transactions and existence/dependency options | `Unsupported` or `Residual` | Explicit AST parser boundaries; no vendor transaction/session execution oracle | TASK-158, with broader lifecycle work in TASK-159/160 and TASK-84 | Do not release any exact vendor/session proof without a separately reviewed engine-law fixture |

These are **proposed dispositions**, not a fabricated maintainer approval.
Parent TASK-83 still requires TASK-159/160 and separate parent acceptance
closure. The task is eligible for child signoff only once final-head CI
and the genuinely independent standards/spec review have verified this
specific change set and the conditional dispositions are approved.
