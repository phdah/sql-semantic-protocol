# TASK-117 independent acceptance audit

## Frozen scope

- Parent: TASK-72 (remains open). Child: TASK-117.
- Immutable fixtures: `72-C117-P`, `72-C117-N`.
- Machine-readable matrix: [task-117-acceptance-freeze.json](task-117-acceptance-freeze.json), registered by `docs/coverage-manifest.json`.
- Six set laws, each on two-column complete signed-integer/NULL bags: UNION/INTERSECT/EXCEPT DISTINCT/ALL. Twelve concrete cases.
- Positive base: A=[(1,NULL),(1,NULL),(2,4)]; B=[(1,NULL),(3,4)]. UNION ALL=5, UNION DISTINCT=3, INTERSECT ALL/DISTINCT=1, EXCEPT ALL=2, EXCEPT DISTINCT=1.
- Negative source B=[(1,NULL),(1,NULL),(1,NULL),(2,4)] proves EXCEPT ALL and EXCEPT DISTINCT terminal absence without mistaking NULL equality for SQL predicate `=`.

## Public implementation and independent evidence

- `src/physical_realization/set_bags.rs` checks two complete schema-backed, independent physical source instances and the canonical normalized set operation; no SQL reparsing. Complete source-row IDs and full nullable **tuple** histograms determine result frequency. An unknown alignment, unresolved producer or nonattested scalar type is `Residual`, not `Exact`. Infeasible enforced physical schemas remain `Impossible`.
- `tests/task_117_set_bags.rs` asserts every frozen output frequency, total row bag count, all source-row IDs, nullable typed bounds and independently executes all twelve SQL bag oracles in DuckDB. Includes a separate full nonmembership/absence test.
- `tests/task_117_dialect_matrix.rs` checks all six positional laws across 13 project dialect parser identities.
- `tests/task_117_set_adapters.rs` checks direct SQL, compiled dbt manifest and schema-equivalent ODCS source evidence against one physical tuple-proof API.
- `src/lib.rs`, `src/physical_realization.rs` expose the proof without changing the serialized protocol. `docs/code-map.md` indexes the canonical owner.

## Standards review

- No `unsafe`, dependency additions, dialect-specific semantics in the consumer or independent column-frequency guessing.
- Snapshot IDs are proof witnesses, not assumed unique SQL keys; ambiguous IDs are `Residual` rather than logically impossible.
- Public API is documented, source construction deterministic, output domains derived only from complete surviving row bags.

## Spec review and boundaries

- TASK-117 owns exactly the six portable two-column set laws, plus a rejected/NULL/duplicate counterpart and mandatory dialect/adapter cells. Parent TASK-72 retains nested sets, BY NAME/CORRESPONDING, MINUS, ordering and arbitrary casts in TASK-118..120.
- These portable parser paths and DuckDB oracles do **not** certify native vendor engine runtime/coercion/collation/session laws. Those stay conditionally reviewed and release-gated, with TASK-117 as named owner for its bounded slice.
- Original TASK-72 mapped AC #1/#3/#5/#6 remain open until the entire parent queue closes.

## Verification and closure

- CI [run 39212686864](https://github.com/phdah/sql-semantic-protocol/actions/runs/39212686864): nine successful jobs covering `cargo fmt --check`, strict Clippy, full Rust tests, no-default-features tests, Rust docs, dependencies and dbt Core E2E. This run includes the new dialect and adapter tests, as well as the twelve full-source DuckDB oracle cases.
- Review axes: **Standards: 0 unresolved findings; Spec: 0 unresolved bounded TASK-117 findings.** Duplicate physical row IDs were downgraded from Impossible to Residual in review; duplicate output names also fail closed. No release or parent signoff is inferred from this child.
- Final-head CI after this documentation and matrix-status commit must pass before merge. If it fails, reopen TASK-117 and resolve the failing check rather than treating this audit as sufficient.
