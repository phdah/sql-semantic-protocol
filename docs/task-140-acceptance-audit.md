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
| #1 | Frozen nine-case matrix, complete source IDs, NULL and inclusive domain bounds, selected/rejected pair identities, 117 parser and 27 adapter cells, reviewed conditional owners | Awaiting final dialect exact/conditional reconciliation |
| #2 | Canonical typed VALUES, array fanout, deterministic INNER equijoin, grouped COUNT, full physical source bags | Awaiting final focused native oracle |
| #3 | Exact/Impossible + DuckDB complete left/right source bags, terminal bags, duplicates, NULL/UNKNOWN rejected pairs | Awaiting final focused and full CI |
| #4 | 13 dialect identities, direct SQL, synthetic compiled dbt, actual dbt Core and DuckDB, schema-only ODCS limitations | Awaiting actual dbt Core execution + observed matrix |
| #5 | fmt, strict Clippy, full default/no-default Rust tests, documentation, dbt Core execution and protocol schema snapshots on final head | Awaiting final-head CI and independent review |

## Original parent AC mapping

TASK-77 #1, #2 and #3 are bounded by children TASK-137, TASK-138 and TASK-139 together with TASK-140; #4 composition specifically depends on TASK-140 exact fanout through join or group; #5 and #6 depend on all four children, their matrix/oracles and finalized public schema/adapter evidence. None of these parent AC is automatically signed off by a child PR. Parent closure requires an independent review after all four child PRs are merged.

## Explicit conditional limits

A physical SQL source holding an opaque array, dynamically shaped JSON/table-function output, unknown type coercion, unsupported ON expression or engine/session-dependent array syntax does not become an exact finite population. These cases remain named TASK-140 residuals or parser boundaries. Parser-supported syntax alone is not a claim of native vendor execution.

**Status:** Implementation and final verification in progress. No TASK-140 AC or TASK-77 parent AC is signed off by this draft audit.
