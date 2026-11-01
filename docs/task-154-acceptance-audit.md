# TASK-154 acceptance audit (implementation branch)

**Scope:** TASK-82 child 2/4. The original parent AC #1..#6 remain separate and unsigned.

## Frozen contract

The task-specific preimplementation v1 matrix is `docs/task-154-acceptance-freeze.json`, linked from `docs/coverage-manifest.json`. It owns 12 full-bag cases (9 mandatory Exact, 2 Residual, 1 Impossible), 65 parser/dialect classifications, source/target SQL TRUE and NOT TRUE partitions, 3 SQL/dbt/ODCS metadata paths, explicit nullable INTEGER bounds/frequencies, target prestate ordinals and atomicity oracles.

## Ownership and evidence

- AC #1: `tests/task_154_acceptance_inventory.rs` verifies fixture IDs, finite cells, typed outcomes, dual-direction ON partitions and coverage manifest invariants.
- AC #2: canonical `src/physical_realization/write_states/operators.rs::merge_rows` owns first-applicable MATCHED, NOT MATCHED [BY TARGET] and NOT MATCHED BY SOURCE actions against complete source/target preimages. The implementation reuses the established task-153 interpreter, no second copy of physical state.
- AC #3: `tests/task_154_merge_unmatched.rs` compares complete native DuckDB and canonical poststates, exact per-column domains and NULL frequencies, selected/rejected source and target ordinals, counts, unchanged source and enforced PK impossibility.
- AC #4: `tests/task_154_dialect_adapter.rs` checks each of 13 exposed parser dialects and SQL/dbt/ODCS schema-equivalent outcomes; native vendor runtime laws remain unclaimed.
- AC #5: final PR-head `cargo fmt`, strict Clippy, Rust tests with both feature sets, docs, dbt Core E2E and protocol/schema snapshots are required prior to signoff.

**Current disposition:** Implementation under review; all AC and CI gates remain pending verification. A parsed vendor dialect does not attest its native engine semantics. Source ambiguity, incomplete population and enforced key conflicts must fail closed. Do not mark TASK-154 Done or start TASK-155 before a green implementation head and independent review; then merge to main. TASK-82 and TASK-91 remain open.
