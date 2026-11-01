# TASK-157 bounded acceptance audit

- **Task:** [TASK-157](../.backlog/tasks/task-157%20-%20prove-create-ctas-view-and-materialization-identities.md)
- **Parent:** TASK-83 (independent parent signoff remains open)
- **PR:** [#178](https://github.com/phdah/sql-semantic-protocol/pull/178)
- **Frozen matrix:** `docs/task-157-acceptance-freeze.json` v2, preserved v1 history
- **Fixture family:** `83-C157-P` and `83-C157-N`

## Original child and parent acceptance mapping

| Child AC | TASK-83 original AC | Required proof/evidence |
| --- | --- | --- |
| 1 | 1,3,4,5,6 | Nine immutable typed source/target cases; 117 explicit parser/canonical verdicts; 27 equivalent adapter cells; no discretionary deletion of mandatory cases |
| 2 | 1,3,4 | Canonical empty CREATE, CTAS, replace, qualified identity, logical view read-time evaluation, frozen MV state; typed Exact/Impossible/Residual and target existence |
| 3 | 4,5,6 | Independent native DuckDB exact table/catalog/NULL/duplicate/negative and source TRUE/NOT TRUE oracles; materialized view uses explicit CTAS stored-snapshot equivalence because native DuckDB MV DDL is unavailable |
| 4 | 1,3,6 | Thirteen sqlparser dialects, compiled dbt catalog schema, ODCS v3.2 equivalent schema, conditional parser/session/vendor distinctions |
| 5 | 5,6 | Final-head fmt, strict all-target Clippy with and without ODCS, full Rust suite with and without ODCS, rustdoc, dbt Core E2E, JSON schema and protocol snapshots, independent review |

## Concrete implementation seams

- `src/analysis.rs` records ordinary schema-only CREATE and distinguishes `CreateView`/`ReplaceView` from `CreateMaterializedView`/`ReplaceMaterializedView`.
- `src/protocol/writes.rs`, `src/emission/statements.rs`, `schema/protocol.schema.json` and `docs/protocol.md` own the typed lifecycle contract. New wire enum values must be treated as a breaking contract evolution for release planning.
- `src/physical_realization/write_states.rs` validates future CREATE schema constraints without constructing independently usable pre-CREATE rows. `write_states/operators.rs` constructs empty definitions, CTAS and stored MV snapshots from the attested source population, returns exact original source-row occurrence IDs for each produced row, and rejects duplicate object identity.
- `src/physical_realization/view_bags.rs` re-reads a logical view after an optional one-leaf canonical INSERT using original physical occurrence identities; a stored MV is not a logical view.
- `tests/task_157_empty_create.rs`, `tests/task_157_state_oracles.rs`, `tests/task_157_dialect_matrix.rs`, `tests/task_157_adapter_parity.rs` and `tests/task_157_acceptance_inventory.rs` are the task-owned executable acceptance suite.

## Soundness boundaries

- **Materialized VIEW:** canonical snapshot creation is physically modeled using complete typed rows; DuckDB CTAS is a store-once snapshot oracle, not proof that DuckDB implements vendor MATERIALIZED VIEW syntax. Refresh, session placement, storage options and engine-specific DDL transaction behavior remain typed Residual under TASK-157.
- **Logical VIEW:** it has no independently controlled physical rows. The positive proof re-evaluates on the complete original leaf or its one certified appended state; unsupported arbitrary view DML, source mutations and non-row-local operators remain Residual.
- **Conditional CREATE:** IF NOT EXISTS needs catalog existence state and cannot silently inherit exact unconditional CREATE semantics. Extra vendor lifecycle laws are assigned to TASK-157 for residual evidence or to later TASK-159 when the parent owns them.
- **Release:** TASK-83 remains open; TASK-157 cannot be Done merely because this audit, parser coverage or earlier-commit CI exists.

## Final-head gates

The following must be verified against the final implementation commit before checking child AC and setting Done:

- [x] All 9 frozen row-state cases verified against independent source/terminal oracles
- [x] 117 frozen dialect outcomes match the observed classifications
- [x] 27 direct SQL/dbt/ODCS evidence cells verified, including no-default-features build
- [x] fmt, strict Clippy, full tests, rustdoc, dbt E2E and schema snapshots green on final head
- [ ] Independent Matt standards/spec review has no blocking findings

**Verification evidence:** [CI run 41344772278](https://github.com/phdah/sql-semantic-protocol/actions/runs/41344772278) passed on implementation head `87615da40a496ba3231b48d3c9bbaf0aacdb573c`, covering every configured Rust and dbt E2E job. The following audit update modifies documentation only and requires a final-head CI rerun.

**Implementer review (not an independent signoff):** Matt's `implement` and `code-review` standards/spec checks found and addressed empty-state validation, materialized-vs-live view identity, unproved temporary/external storage laws, ambiguous adapter catalogs, source-row occurrence provenance, and assertions for SQL TRUE/NOT TRUE. No known mandatory implementation cell remains; a distinct independent reviewer must approve before child Done or merge.

**Disposition:** Implementation and mandatory oracle checks verified; independent review remains open. Parent TASK-83 stays open.
