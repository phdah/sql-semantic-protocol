# TASK-196 acceptance audit (implementation in progress)

Original task: `.backlog/tasks/task-196 - certify-schema-write-and-ddl-state-outcomes.md`.

## Bounded source of truth

- 38 original immutable TASK-178 cases: 11 schema, 18 DML, 9 DDL; 13 named parser dialects each, **494 cells**.
- Historical cell types are **428 C**, **11 R**, **55 U**. C means canonical-document equality, **not** vendor-native execution, strongest typed state-effects, or independent semantic approval.
- `docs/task-196-acceptance-freeze.json` pins every SQL, variant, parser cell, named source identity and review gate. `tests/task_196_acceptance_matrix.rs` rejects drift and missing original cells.
- 16 original DML and 18 original schema/DDL DuckDB closed-world source/terminal oracle programs already existed in TASK-177. `tests/task_196_original_native_states.rs` replays all 34 against the exact original SQL, including three RETURNING bags, unchanged unrelated sources, negative constraint/lifetime effects, and explicit native rejection of REPLACE/OVERWRITE.
- Original S191 (dbt manifest/catalog) and S193 (missing/conflicting schema) are authority-bound metadata sources, not fabricated native SQL poststates. They must be checked at direct/dbt/ODCS source-parity seams.
- Original S199 (REPLACE INTO) and S200 (INSERT OVERWRITE) remain **unapproved** vendor-specific or unsupported physical DML outcomes, with native DuckDB syntax rejection. Parser/session/vendor disposition remains TASK-179/TASK-191.

## Acceptance tracking

| AC | Current bounded evidence | Still blocking completion |
| --- | --- | --- |
| #1 | Original 38 SQL / 494 parser cells, 34 native fixture anchors and four named boundaries frozen | Independently specify affected/rejected occurrence identities, exhaustive source-to-poststate and typed constraint/default expectations in all 38 rows |
| #2 | Original native whole source/terminal and RETURNING executions replayed via public SQL fixture | Strongest typed protocol state/effect proof against each native original, including multiplicity and negative/NULL/default cases |
| #3 | S199/S200 native rejection and explicit unapproved parser/vendor boundaries | Reviewed, release-owned dispositions with no mandatory core upgrade to approved Residual |
| #4 | Frozen original SQL and task-owner handoff to TASK-197/TASK-180 | Authoritative direct SQL/dbt catalog/manifest/ODCS parity matrix |
| #5 | Task-owned tests + freeze; coverage manifest explicitly in progress | CI exact-head fmt, strict Clippy, Rust default/no-default, docs, dbt E2E, pinned sql-tdg, independent 494-cell review and evidence links |

**No signoff asserted** until all remaining required gates are independently verified. TASK-197 is blocked until this task is Done **and merged**; TASK-180, TASK-88 and TASK-91 retain their independent gates. The agent/reviewer and DuckDB fixture cannot attest other vendor execution laws.
