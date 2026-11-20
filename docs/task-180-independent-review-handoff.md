# TASK-180 implementation and complete source-review handoff

**Target:** [PR #206](https://github.com/phdah/sql-semantic-protocol/pull/206), TASK-180 AC #2 and #5. This is evidence for the *separate independent final signoff agent*, not a claim that TASK-180 is Done.

## Scope and review accounting

The immutable original matrix is **163 SQL cases × 13 parser dialects = 2,119 cells**. Existing TASK-180 predicate reviews cover 25 × 13 = 325 cells, of which 130 have frozen exact typed domains and 195 have explicit source-bounded conditional output claims.

The five new complete family ledgers cover the remaining **138 SQL cases, 1,794 cells**:

| Family | Original cases | Dialect cells | Source-bounded Exact candidates | Conditional Residual candidates |
| --- | ---: | ---: | ---: | ---: |
| TASK-192 predicates/types | 18 | 234 | 0 | 234 |
| TASK-193 joins/instances | 24 | 312 | 273 | 39 |
| TASK-194 set operators | 22 | 286 | 260 | 13 Residual + 13 Impossible |
| TASK-195 grouping/windows/order | 36 | 468 | 363 | 105 |
| TASK-196 DML/DDL/schema | 38 | 494 | 376 | 118 |
| **Remaining family reviews** | **138** | **1,794** | **1,272** | **509 Residual + 13 Impossible** |
| **All source-bounded records including predicates** | **163** | **2,119** | **1,402** | **704 Residual + 13 Impossible** |

A candidate **Exact** refers only to the explicitly frozen SQL, typed/source conditions, fixed full-source DuckDB bag, and matching public portable canonical law. It **does not certify vendor-native runtime equivalence**, open-world finite output min/max, nondeterministic row-instance membership, or missing source types. The 481 named vendor/session cells have separately approved **Residual** dispositions, not native runtime attestation.

## Case and dialect review files

- `docs/task-180-pending-review-task-192.json`: remaining comparison/environment, string, temporal, overflow and structural cases. Unattested runtime/collation/environment law remains Residual.
- `docs/task-180-pending-review-task-193.json`: original SQL joins, distinct physical source instances, independent join output bags, typed source domains. LEFT SEMI/ANTI original DuckDB spelling and LATERAL are conditional.
- `docs/task-180-pending-review-task-194.json`: complete independent typed source-arm tuple bags, terminal/non-emitted candidate multisets, parser CORRESPONDING and incompatible set arity Residual.
- `docs/task-180-pending-review-task-195.json`: ranked/grouped source identities, full original deterministic DuckDB outputs, typed algebraic domains, explicit stochastic/sample/order and parser drift boundaries.
- `docs/task-180-pending-review-task-196.json`: source-authoritative DML/DDL schema, complete native before/after state, independent constraint/write laws, and named unproved vendor/conflict/parser/metadata conditions.

`tests/task_180_remaining_source_reviews.rs` independently cross-checks every candidate case, immutable original SQL/feature/owner/dialect, typed/native evidence and source-instance identity against the separate completed TASK-192..196 reviewer oracles. It also independently checks source tuple conservation for 20 executable set cases: all emitted plus non-emitted candidate occurrences equal the complete source candidates, preserving duplicates and NULL. Its coverage set, combined with the existing 325 predicate review records, must equal all 2,119 frozen identifiers.

The existing `tests/task_180_independent_dialect_gate.rs`, `tests/task_180_scalar_native.rs`, `tests/task_180_original_write_states.rs`, `tests/task_180_alter_set_default.rs`, `tests/task_180_live_canonical.rs` and TASK-197 adapter tests execute the original DuckDB, typed, parser and cross-adapter oracles. These runtime checks are independent of merely carrying labels in the ledger.

## Remaining semantic disposition, not unproved Exact

The original exceptional 105 SQL/dialect cells remain distinguished from their source-typed or reviewed normalizations:

- S199 REPLACE INTO: the database's replacement action, constraint enforcement and target conflict keys are not attested. **No general canonical Exact** from parsing alone.
- S200 INSERT OVERWRITE: unproven whole-relation vs partition replacement scope and mutation order. **No general canonical Exact** without target/partition authority.
- S203/S206 UPDATE/DELETE IN-subquery: the **source-typed** public physical realization is independently proven with complete original NULL/TRUE/NOT TRUE and duplicate target bags; the raw original without source authority remains **Unsupported**.
- S220 ALTER SET DEFAULT: the **source-typed** canonical schema transition and future omitted-column default are implemented and independently replayed; raw catalog-free SQL remains **Unsupported**.
- S084 CORRESPONDING: parser rejects the original, so borrowing BY NAME semantics would invent parser/runtime equivalence. S086 UNION incompatible 1-versus-2 column arity is proved Impossible (not a mere Residual) for all 13 parser dialects by the public zero-output plus diagnostic proof.
- S100/S101/S104: 21 historical grouping AST drifts independently normalized to one public canonical grouping law are source-bounded Exact candidates under the original complete DuckDB bag, with a named reconciliation witness. S120 Snowflake WINDOW, S179 structural CAST, S189 generated-column parser cases retain their remaining parser/reviewer boundaries.
- S152 MSSQL LATERAL: actual sqlparser Boolean(TRUE) vs identifier(TRUE) semantic divergence, conditionally reviewed as **Residual**, not overwritten by ANSI semantics.
- S198 conflict iteration order and S208 delete cascade depend on native/constraint laws; retain conditional source-boundary dispositions.
- TASK-195 LIMIT/TOP/tie/order and TABLESAMPLE cases are conditionally reviewed whenever the physical result is non-deterministic or original native spelling unproven.

No native vendor runtime or additional SQL family was invented. `docs/task-180-exception-and-gap-ledger.json` and `docs/task-180-final-signoff-evidence.json` continue to block false release promotion.

## Independent final agent instructions

1. Review the *actual operator implementation* and strongest provable typed, correlated and physical-domain claims for the 1,794 source-bounded candidates, not just their copied oracle references. Reject/fix any candidate for which its conditional boundary should be stronger or weaker. Confirm historical U/R/D remains properly Residual.
2. Verify all 2,119 candidate identities, the 481 vendor/session Residual dispositions and the original TASK-88 mapped acceptance, including all valid/invalid native outputs. A candidate status is **not** permission to tick the corresponding final release cell.
3. Only after independent semantic verification, migrate the 1,794 candidate cells into `approved_cells[]` in `docs/task-180-final-signoff-evidence.json`, maintaining the existing 325; update the gate so it refuses unproven source fields, invalid native vendors, duplicate or swapped approval identities. Keep manifest and TASK status synchronized.
4. Run **exact-final-head** fmt, strict Clippy, full Rust default/no-default tests/docs, dbt E2E, active schema/goldens and downstream `sql-tdg` pinned to that SHA. Only if all mandatory semantics and justified conditional dispositions satisfy AC #2/#5, mark TASK-180 Done and clear the task's own release gate. Do not close TASK-88 or TASK-91 automatically.

Source review preparation is complete, but **final independent verification and acceptance are deliberately not claimed**.
