# TASK-179 acceptance audit

**Status: In Progress. None of AC #1..#5 is signed off.** This PR does not approve TASK-88, TASK-91 or any external vendor runtime.

## Frozen, versioned scope and executable evidence

- Initial 14 case / 182 dialect-cell native control matrix: `docs/task-179-acceptance-freeze.json`.
- Extended 37 named original/synthetic law variants x 13 dialects = **481 cells**: `docs/task-179-variant-matrix-v2.json`. Its 35 original SQL shapes link to TASK-177's immutable parser/analysis masks; two identifier probes have task-local provenance. Native vendor status is always unapproved unless separately executed.
- Existing independently reviewed typed source semantics: `docs/task-179-reconciled-proofs-v3.json` maps 20 TASK-192, 5 TASK-195, 6 TASK-196, 4 TASK-163 transaction, and 2 TASK-179 identifier cases without constructing unsupported external vendor laws.
- `tests/task_179_vendor_law_oracles.rs` checks complete typed source snapshots and bounded native terminal/rollback controls. `tests/task_179_native_counterexamples.rs` adds independent native NULL-order session toggles, LIKE/ILIKE/case/padding, invalid implicit/TRY_CAST, finite MERGE, transactional DDL rollback, quoted identifier collision, and FLOAT NaN vs SQL NULL negative probes.
- `tests/task_179_original_transactions.rs` executes all original S221..224 source and final relation bags using frozen original SQL.
- `tests/task_179_dialect_law_matrix.rs` executes the exact 35 original SQL shapes against all 13 public parsers (455 cells), checked against immutable TASK-177 parser/analyzer status masks, and ensures no input was reinterpreted as external runtime execution.
- `tests/task_179_semantic_fail_closed.rs` checks 13-parser conditional string laws and source-typed positive/NOT TRUE witnesses plus unproved transaction session laws.
- `tests/task_179_adapter_handoff.rs` reconciles 31 original variants to TASK-197's full direct SQL / compiled dbt manifest+catalog / ODCS source-fact parity. Four transaction scripts and two new identifier probes do **not** inherit false adapter execution facts.
- `tests/task_179_reconciled_proofs.rs` checks every original typed/source/terminal expectation against its existing independently reviewed task-owner evidence.

## Exact outstanding acceptance gates

| AC | State | Must still happen |
| --- | --- | --- |
| #1 | Partially complete | Review the entire 37 x 13 owned matrix and freeze exact case-specific expected typed domains and independent positive/NOT TRUE/Impossible/Residual outcomes in every mandatory cell. Current ledgers accurately link inherited proofs but keep vendor/session runtime unapproved. |
| #2 | Partially complete | Independently adjudicate whether each unproved core semantic variant is an approved conditional vendor law or missing required implementation. Only native DuckDB controls and existing task-specific source-typed facts are certified. |
| #3 | Expanded, not signed off | Review and green-test all newly added native examples, including complete source and terminal negative/error oracles. Reconcile all 37 variants to native meaningfully executable evidence versus documented conditional/nonexecuting boundaries. |
| #4 | Partially complete | Freeze and independently review per-cell vendor/session release dispositions; source-fact adapter parity is inherited for 31 source-representable original cases only, with explicit nonrepresentable transaction/identifier cases. Other engines remain unexecuted. |
| #5 | Pending | Exact-final-head rustfmt, strict Clippy, full and no-default Rust tests/docs, dbt E2E, pinned sql-tdg consumer CI and independent two-axis code/spec review. Mark task Done only after all other ACs pass. |

## Non-negotiable boundaries

- DuckDB 1.10506 (pinned dev dependency) is the only executed database. ANSI/generic are parser dialects, not execution engines. Every non-DuckDB vendor law requires engine/version/session attestation before native exactness can be advertised.
- Source metadata from dbt manifest, catalog and ODCS **cannot** attest a transaction, collation, time zone, implicit cast or identifier-folding execution law.
- Negative comparative controls are not automatically SQL NOT TRUE/Impossible proofs.
- TASK-191 (original parser rejects and native-unproven cells), TASK-180 (full independent dialect signoff), TASK-88 and TASK-91 remain separate gates.
