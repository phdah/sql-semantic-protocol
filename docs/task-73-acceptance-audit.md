# TASK-73 independent parent acceptance audit

**State:** Original AC #1..#6 mapped to a finite portable proof floor. Parent status remains In Progress until this PR's final-head CI passes. TASK-91 release approval is independent.

## Source of truth

The [versioned parent freeze](task-73-acceptance-freeze.json) owns the six original AC mappings and pins eight child fixture families, 12 exact source/terminal cases and seven conditional boundaries. It references the merged, independently frozen TASK-121..124 cases without copying their expected physical bags. The public guard is `tests/task_73_parent_closure.rs`.

All four child PRs are merged on `main`: #131 (basic aggregates), #132 (DISTINCT/FILTER/HAVING), #133 (filtered joins), #134 (independent full-bag/dialect/adapter audit). The independent oracle is `tests/task_124_parent_aggregate_oracles.rs`, adapter parity is `tests/task_124_aggregate_adapters.rs`, and the materialized `aggregate_summary.sql` oracle is `tests/protocol_conformance_dbt_e2e.rs`.

## Original acceptance review

| AC | Mandatory evidence | Reviewed, non-promoted boundary |
| --- | --- | --- |
| #1 COUNT/SUM/MIN/MAX/AVG, aliases, DISTINCT/FILTER/HAVING | Global/grouped nullable duplicates; DISTINCT, FILTER and Boolean HAVING exact frozen bags | FILTER's dialect-specific parser boundary does not imply a runtime guarantee |
| #2 WHERE/JOIN/projection and group provenance | Complete two-relation join, duplicate amplification, rejected group, empty global join and per-source IDs | No claim of arbitrary unconstrained producer graph synthesis |
| #3 Empty groups and grouping extensions | Empty global yields one group; empty GROUP BY yields none | GROUPING SETS, ROLLUP, CUBE, GROUP BY ALL are parser-bound or typed Residual |
| #4 Positive/rejected typed domains and overflow | Exact typed inclusive bounds, NULL sets and TRUE/FALSE/UNKNOWN HAVING, closed physical and terminal bags | Nonintegral AVG, overflow and unproven numeric/collation semantics remain explicitly non-Exact |
| #5 DuckDB and dbt independent execution | 12 complete DuckDB physical/terminal oracles and real materialized `aggregate_summary.sql` expected `north:1:42` | No fabricated database-native outcomes outside DuckDB |
| #6 Dialects, adapters, public contract and regressions | 156 mandatory 13-dialect parser classifications; 36 SQL/dbt/ODCS adapter cells; schema and golden CI | 52 conditional parser and 12 adapter boundary cells remain non-Exact |

The required numeric result and complete input/output bag expectations are owned by the child freeze; this parent inventory pins fixture IDs, original AC mapping, case counts, proof class and owners. The regression guard rejects removal of a mandatory fixture, omission of a dialect/adapter or an unreviewed Exact promotion. The existing executable child tests must also pass; parent metadata alone does not certify semantics.

## Conditional disposition

The four grouping extensions are owned by TASK-124. Unknown subtotal/group identity, inferred grouping keys and vendor/session behavior are non-Exact pending a separately attested native execution law. Three numeric boundaries (nonintegral AVG scaling, SUM overflow and noninteger/decimal/collated MIN/MAX) remain owned by TASK-121 and fail closed. Their presence does not turn an unsupported mandatory portable case into a permissible residual. SQL parsing in 13 dialects is not evidence of 13 real native engines.

## Review and merge gate

**Standards axis:** This change is confined to a parent acceptance freeze, public integration guard, audit, coverage registration and Backlog.md status. No production semantic rule or protocol shape changes, no new crate dependency, and no duplicate bag oracle.

**Spec axis:** Each original AC has a pinned mandatory case mapping; both positive and rejected cases retain exact physical bags, typed domains and adapter/parser classifications. The seven conditional families are owner-reviewed, non-promoted and distinct from mandatory Exact scope.

Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `cargo doc --no-deps`, no-default-features tests, schema/golden checks and real dbt E2E on the final PR head. Parent Done and checked ACs require all checks green; this PR does not approve TASK-91.
