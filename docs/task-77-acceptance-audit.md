# TASK-77 independent parent acceptance audit

**Decision: independently signed off TASK-77 original AC #1..#6 for the frozen portable scope.** TASK-137..140 are Done and PRs #144, #145, #146 and #149 are merged on main. This independently reviews the original parent acceptance criteria, not the separate TASK-91 release gate.

## Finite authoritative evidence

[Version 1](task-77-acceptance-freeze.json) reconciles **24 mandatory exact/impossible cases** and **10 reviewed conditional cases** from the child-owned matrices without copying or weakening the original SQL, full-source bags or terminal oracle values.

| Owner | Mandatory | Conditional | Main evidence |
| --- | ---: | ---: | --- |
| TASK-137 | 7 | 1 | VALUES, UNNEST/EXPLODE, ordinality, NULL and empty arrays, [audit](task-137-acceptance-audit.md) |
| TASK-138 | 7 | 3 | EXCEPT/EXCLUDE/REPLACE/RENAME, fixed PIVOT/UNPIVOT, [audit](task-138-acceptance-audit.md) |
| TASK-139 | 3 | 4 | Literal EXPLODE and JSON_TABLE/FLATTEN/opaque/variable-shape residual boundaries, [audit](task-139-acceptance-audit.md) |
| TASK-140 | 7 | 2 | Finite GROUP and equijoin bags, NULL TRUE/FALSE/UNKNOWN pairs, real dbt Core DuckDB execution, [audit](task-140-acceptance-audit.md) |

The [parent public regression](../tests/task_77_parent_closure.rs) checks every child case, stable classification, 13 dialect and three adapter cells, source/terminal bags, typed domains and NOT TRUE evidence. Two TASK-140 cases cite canonical peer domains rather than duplicating the same result: N-ABSENCE points to P-GROUP and P-JOIN-REVERSE points to P-JOIN-NULL. Native DuckDB cross-checks independently exercise group and star results.

## Original criteria review

| AC | Mandatory evidence | Conditional boundary |
| --- | --- | --- |
| #1 Source/table normalization | TASK-137 VALUES/UNNEST/ordinality; TASK-138 star, PIVOT/UNPIVOT; TASK-139 EXPLODE | Dynamic JSON and opaque generators |
| #2 Typed row fanout/output shape | NULL, empty arrays, ordinality, fixed-shape PIVOT/UNPIVOT and group counts | Unknown nested types or variable output shape |
| #3 Exact or residual separation | Exact finite sources, Impossible absent rows and negative groups | JSON_TABLE, FLATTEN, generator UDF, and non-equi operators remain owned Residual |
| #4 Composition | TASK-140 finite GROUP and JOIN physical inputs, duplicates, nullable keys, compiled dbt Core | ODCS metadata cannot attest arbitrary literal rows |
| #5 Dialect/execution oracles | Frozen 13-dialect parser matrices, independent full DuckDB source/terminal runs | Native non-DuckDB vendor/session laws are not certified |
| #6 Tests and adapters | Child public tests, compiled dbt, ODCS schema parity, schema/golden checks and parent guard | No fabricated vendor or dynamic-schema evidence |

## Review and verification

**Standards:** Pure task-owned metadata and an established public integration seam preserve the parser/analysis/protocol separation, no new dependency or wire format. Child freeze files stay canonical. Sorted sets guard deterministic identities.

**Spec:** Original AC #1..#6 map explicitly to required frozen case IDs and test owners, and all conditional runtime boundaries retain owners and reviewed dispositions. The parent regression fails on classification drift, missing cases or omitted parser/adapter cells, and independently executes native DuckDB source/terminal examples.

**Verification:** [Implementation CI 41095085160](https://github.com/phdah/sql-semantic-protocol/actions/runs/41095085160) passed all 9 required jobs (formatting, strict Clippy, both feature configurations, documentation, schema/golden and actual dbt Core E2E) on the final implementation head. The metadata signoff commit must also pass PR-head CI before merge. TASK-91 remains an independent release gate.
