# TASK-159 independent acceptance audit

**Scope:** TASK-83 child 3/4, existing PR #180. The parent TASK-83 remains open for TASK-160 and independent original-AC review.

## Immutable acceptance lineage

- Preimplementation fixture freeze: `docs/task-159-acceptance-freeze-v1.json` (exact copy of originally committed v1).
- Active v2 fixture freeze: `docs/task-159-acceptance-freeze.json`. V2 adds precise source/post typed domains, full programs, SQL NULL and duplicate occurrence identities, native DuckDB expectations, and mandatory negative contradictions without weakening the original seven mandatory cases. The eighth mandatory cell is RENAME collision.
- `tests/task_159_frozen_cases.rs` interprets every mandatory v2 case as a public SQL program and compares each before/after complete bag against native DuckDB; it also asserts typed domain frequencies, minimum/maximum, NULL membership, inclusive bounds, affected ordinals, named relations, and truth partitions. Impossible cases prove the native statement rejects without altering existing states.
- The freeze is test data, not a source of execution law. No dynamic acceptance snapshots are generated from the analyzer.

## AC mappings and evidence

| Child criterion | Evidence | Required result |
| --- | --- | --- |
| #1 Pre-frozen matrix | v1 history, v2 active freeze, manifest link and source/target expectation oracles | Every mandatory case independently checked; conditional cases retain own owner/disposition |
| #2 Canonical lifecycle | `src/protocol/writes.rs`, `src/analysis/ddl_lifecycle.rs`, `src/physical_realization/write_states/{lifecycle.rs,../write_states.rs}`, `src/bundle/state.rs` | DROP absent, TRUNCATE empty typed table, RENAME exactly preserved bag only with proven destination absence; IF EXISTS no-op only with known absence |
| #3 Physical/native oracle | `tests/task_159_frozen_cases.rs`, `tests/task_159_lifecycle.rs`, `tests/task_159_lifecycle_defaults.rs` | Full DuckDB source/intermediate/terminal rows and schema, duplicate occurrences and NULL, Impossible collision and nonexistent DROP |
| #4 Dialects and adapters | `tests/task_159_dialect_matrix.rs`, `tests/task_159_adapter_parity.rs` | Every shared AST or explicit parser boundary in all 13 dialects; equivalent direct/dbt catalog/ODCS prestate evidence; vendor/session variants remain Residual |
| #5 Final CI/review and parent | GitHub Actions PR CI and standards/spec review at fixed final head | All nine CI jobs green, every child mandatory cell covered, signoff in child only; parent TASK-83 AC remain open |

## Conservative execution boundaries

- Parser acceptance does not imply engine execution or transaction portability.
- Missing caller snapshots are not evidence that a relation does not exist. DROP IF EXISTS after an already witnessed DROP is an exact no-op, but an isolated DROP IF EXISTS is Residual.
- RENAME is Exact only if a prior full DROP of the destination proves the name is free. Collision with an existing target is Impossible; absent destination evidence remains Residual.
- Normalized scalar defaults are migrated with the physical table identity; DROP clears the physical constraint/default evidence. Re-CREATE after DROP is Residual pending generation-aware schema declarations owned by TASK-160. This prevents a stale ALTER schema from producing a false Exact result.
- DROP TABLE and TRUNCATE are never approximated by DELETE WHERE, and RENAME is never approximated by copying rows. Complete physical bags preserve duplicate and SQL NULL occurrences, including before/after typed domains.
- CLONE/LIKE, TEMP/TRANSIENT object lifetimes, vendor DROP VIEW and materialized storage, conditional CREATE, CASCADE, restart identity, and DDL transactions are conditionally non-exact. Owner TASK-159; no vendor portability claim can be released without separately verified engine/version evidence.
- dbt's compiled SELECT model is not an arbitrary DDL script. dbt catalog/manifest and ODCS provide equivalent typed metadata to independent SQL lifecycle operations but do not independently execute them.

## Review and signoff

Review the actual final PR head against `main` using Matt's code-review standards and spec axes. A passing CI is necessary but not sufficient. Sign off only when all frozen mandatory cells, parser/adapter contract, independent review and final-head CI are true. Cross-object graph closure and full CREATE/ALTER/SELECT/DROP composition remain TASK-160; no TASK-83 parent checkbox is auto-signed.


## Independent semantics review correction

The initial lifecycle analyzer incorrectly advertised the *old* table as the
producer after RENAME and the destroyed table as a producer after DROP. The
public graph seam now makes RENAME produce only its new relation name, DROP
produce no datasets (including anonymous), and TRUNCATE retain its table
identity. `tests/task_159_lifecycle_graph.rs` verifies the public graph and
wire output and protects downstream consumers from phantom sources.
