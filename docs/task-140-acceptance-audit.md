# TASK-140 acceptance audit

Task: [TASK-140](../.backlog/tasks/task-140%20-%20compose-table-fanout-with-joins-dbt-and-certify-task-77.md). Parent: TASK-77 (separate, original six-criterion closure after all four child PRs merge).

## Frozen test boundary

- Frozen matrix: `docs/task-140-acceptance-freeze.json` (nine bounded direct SQL cases with fixture families `77-C140-P`, `77-C140-N`).
- Canonical AST boundary: `src/analysis/finite_tables.rs`.
- Canonical parser-independent data: `src/protocol/finite_table.rs`, including complete joined source bags and exact match pairs.
- Physical proof: `src/physical_realization/finite_tables.rs`; exhaustive TRUE and NOT TRUE source pair identities, terminal multiset, typed values, and negative membership.
- Native DuckDB source and terminal oracle: `tests/task_140_finite_group_bags.rs`.
- Parser and adapter matrix: `tests/task_140_dialect_adapter_matrix.rs` (117 SQL dialect cells and 27 adapter cells).
- Real dbt Core models: `tests/fixtures/dbt_core_project/models/task140_{grouped,joined}_fanout.sql`; `tests/task_140_dbt_core_e2e.rs`.
- Source-free ODCS schema contracts do not themselves attest literal VALUES/array populations. No literal values may be inferred from ODCS metadata.

## Required signoff evidence

| Child AC | Acceptance evidence | Closure |
| --- | --- | --- |
| #1 | Frozen nine-case matrix, complete source IDs, NULL and inclusive domain bounds, selected/rejected pair identities, 117 parser and 27 adapter cells, reviewed conditional owners | Verified: 9 frozen cases, 117 parser cells and 27 adapter cells with owned native-law residual boundaries |
| #2 | Canonical typed VALUES, array fanout, deterministic INNER equijoin, grouped COUNT, full physical source bags | Verified: canonical one-/two-input typed finite bag, COUNT(*), exact joined/NULL identity laws |
| #3 | Exact/Impossible + DuckDB complete left/right source bags, terminal bags, duplicates, NULL/UNKNOWN rejected pairs | Verified: exact/Impossible, full DuckDB source/output snapshots and SQL TRUE/FALSE/UNKNOWN partition |
| #4 | 13 dialect identities, direct SQL, synthetic compiled dbt, actual dbt Core and DuckDB, schema-only ODCS limitations | Verified: source-independent direct/compiled dbt parity, executed dbt Core DuckDB models and fail-closed ODCS schema evidence |
| #5 | fmt, strict Clippy, full default/no-default Rust tests, documentation, dbt Core execution and protocol schema snapshots on final head | Verified: [all 9 required CI jobs](https://github.com/phdah/sql-semantic-protocol/actions/runs/40242206911) passed on implementation head `b8a0a01461f22e16a13e3ea8de8d463df61a98fd` |

## Original parent AC mapping

TASK-77 #1, #2 and #3 are bounded by children TASK-137, TASK-138 and TASK-139 together with TASK-140; #4 composition specifically depends on TASK-140 exact fanout through join or group; #5 and #6 depend on all four children, their matrix/oracles and finalized public schema/adapter evidence. None of these parent AC is automatically signed off by a child PR. Parent closure requires an independent review after all four child PRs are merged.

## Explicit conditional limits

A physical SQL source holding an opaque array, dynamically shaped JSON/table-function output, unknown type coercion, unsupported ON expression or engine/session-dependent array syntax does not become an exact finite population. These cases remain named TASK-140 residuals or parser boundaries. Parser-supported syntax alone is not a claim of native vendor execution.

## Independent implementation review

**Standards:** Reviewed changes against `AGENTS.md` and `CONTRIBUTING.md`. Parsing, canonical semantic representation, emission and physical proof remain separate; new fixed typed `Joined`/ `Grouped` operators own one authoritative complete source bag, while consumer-specific SQL rendering stays absent. The canonical model is parser-independent, serialization deterministic, public JSON schema and docs synchronized, new APIs documented, and breaking additions use a `feat(protocol)!` PR. Explicit unsupported casts, JOIN expressions, alias ambiguity and vendor runtime behavior fail closed. Source alias validation has a regression test.

**Spec:** Independently checked every TASK-140 AC and both `77-C140-P/N` fixture families against 9 frozen cases, 117 dialect statuses (72 Exact, 36 reviewed Residual, 9 parser boundaries), 27 SQL/dbt/ODCS cells, full native DuckDB bags, NULL/duplicate physical source identities, numeric COUNT and inclusive typed result domains, SQL three-valued proof, requested Impossible output and actual compiled dbt E2E. The conditional limitations are source or engine evidence limitations, never substitutes for an unimplemented required exact DuckDB case.

**Final implementation CI:** [https://github.com/phdah/sql-semantic-protocol/actions/runs/40242206911](https://github.com/phdah/sql-semantic-protocol/actions/runs/40242206911), commit `b8a0a01461f22e16a13e3ea8de8d463df61a98fd`, all 9 jobs successful. The final metadata-only signoff commit must pass its own PR-head merge gate.

**Status:** TASK-140 bounded child AC #1..#5 **verified Done pending merge**. TASK-77 parent original AC #1..#6 remain unsigned until PR #149 merges and parent closure is independently checked.
