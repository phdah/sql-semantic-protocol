# TASK-147 acceptance audit

The canonical exact slice covers explicitly targeted `ON CONFLICT(id) DO NOTHING` with a complete physical target, an enforced matching primary/unique key, and proven comparisons. Different INSERT source rows sharing a key but carrying distinct payloads are Residual without an attested winner order. The poststate retains untouched rows; only actually inserted rows belong to `RETURNING`. A frozen mandatory NULL/WHERE case attests both FALSE and UNKNOWN as NOT TRUE, preserving original source rows and rejecting both from the INSERT input. The complete DuckDB oracle and typed/negative tests live in `tests/task_147_insert_conflict.rs`.

All other named INSERT variants remain typed unsupported/residual until versioned native engine/session evidence proves their semantics. In particular, IGNORE is not interchangeable with DO NOTHING, REPLACE is not a blind append, OVERWRITE needs a known replacement scope, inferred conflict targets are not uniquely resolved, and nontrivial RETURNING projections need independent value-domain proof.

The v1 immutable inventory is `docs/task-147-acceptance-freeze.json`, referenced by the coverage manifest. Parser dialect tests are `tests/task_147_dialect_matrix.rs`.

**Child is not signed off.** Final-head fmt, Clippy, full cargo tests, no-default-features, docs, schema, dbt E2E, complete 13-dialect and three-adapter cells, and independent source/target oracle review remain required. TASK-80 stays open.
