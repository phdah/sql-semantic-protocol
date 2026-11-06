# TASK-164 mixed-script acceptance audit

**Implementation PR:** https://github.com/phdah/sql-semantic-protocol/pull/191 (draft until verified). **Status:** In Progress. TASK-84 original parent acceptance and the TASK-91 release gate remain separately open.

## Frozen scope

- The original pre-semantic v1 acceptance freeze is immutable in \`docs/task-164-acceptance-freeze-v1.json\`. The archived v2 corrects only the first CREATE checkpoint's existence representation, retaining eight named cases; v3 pins all 598 parser classifications. The active `docs/task-164-acceptance-freeze.json` v4 additionally pins every Exact case's complete per-step schema and bag, mutation counts, original affected row ordinals and TRUE/NOT TRUE source ordinals; immutable v1, v2 and v3 are archived; deliberately red observation evidence is https://github.com/phdah/sql-semantic-protocol/actions/runs/42202839057.
- The six mandatory Exact/Impossible cases cover native DuckDB CREATE, two INSERTs, matched MERGE, DELETE, SELECT, repeated/idempotent MERGE, NULL/duplicate rejection, whole-script ROLLBACK and cross-SQL-unit ALTER scalar defaults, plus impossible repeated CREATE. Both conditional Residual cases are separately owned: unsupported native SAVEPOINT by TASK-162 and vendor/session DDL laws by TASK-163.
- SQL TRUE/NOT TRUE source occurrence identities, typed INTEGER/NULL frequencies and inclusive numeric bounds, all before/after full catalogs, deterministic mutation counts and expected terminal row bags are fixed in the task-owned inventory. There is no consumer SQL reparsing or independent synthetic intermediate state.

## Public API and external oracles

- \`src/physical_realization/transactions.rs\`: only after the complete \`PhysicalSequentialStateGraphProof::Exact\` does the transaction path advance canonical \`ColumnAlterStep\` scalar default evidence on the certified in-script relation generation. The full constraint/default catalog is copied at BEGIN and restored by ROLLBACK.
- \`tests/task_164_mixed_scripts.rs\`: native pinned DuckDB executes every SQL statement, comparing each complete physical live relation bag, ordered schema names/types and physical catalog to the independently frozen exact checkpoint for every step. Read TRUE/NOT TRUE membership, NULLs and duplicates, affected counts, final typed output bounds, rollback state and unsupported laws are asserted.
- \`tests/task_164_dialect_matrix.rs\`: evaluates all thirteen named dialects, distinguishing parser/canonical unsupported forms from the native DuckDB execution oracle. This does not imply vendor engine certification.
- \`tests/task_164_adapter_parity.rs\`: canonical source metadata from direct SQL, compiled dbt manifest/catalog and ODCS contract is compared through the same MERGE, DELETE and SELECT physical outcomes. Adapter metadata does not certify native transaction session or vendor DDL laws.

## Signoff gate state

| Child AC | Technical evidence | Status |
| --- | --- | --- |
| #1 Freeze | v1, v2, v3 immutable; active v4 linked to coverage manifest with 598 parser cells and every mandatory exact checkpoint/ordinal/count | Prepared, needs review |
| #2 Canonical semantics | Complete typed transactions, ALTER default propagation, Exact/Impossible/Residual | Implemented, unverified |
| #3 DuckDB source/checkpoint/terminal oracles | Eight-case source-complete matrix and native tests | Committed, unverified |
| #4 Dialects and adapters | 13 parser classifications plus SQL/dbt/ODCS parity | Committed, unverified |
| #5 Implementation-head CI and review | fmt, strict Clippy, all/no-default tests, docs, dbt E2E, schema/golden and two-axis review | Pending |

**Important:** No mandatory criterion is signed off and the child remains In Progress until the final PR-head CI and an independent review establish complete coverage. In particular, a parser-successful vendor syntax form cannot become a cross-engine execution proof. A separate human reviewer or explicitly approved reviewer-process exception may be required. No change to the active JSON protocol root/schema is intended.
