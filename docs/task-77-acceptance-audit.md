# TASK-77 independent parent acceptance audit

**Decision: independently signed off TASK-77 original AC #1..#6 for the frozen portable scope.** TASK-137..140 are Done and PRs #144, #145, #146 and #149 are merged on main. This independently reviews the original parent acceptance criteria, not the separate TASK-91 release gate.

## Finite authoritative evidence

[Version 1](task-77-acceptance-freeze.json) reconciles the four child-owned **24 mandatory exact/impossible cases** and **10 reviewed conditional cases**, plus one mandatory derived subquery case and one conditional LATERAL boundary in v2 from the child-owned matrices without copying or weakening the original SQL, full-source bags or terminal oracle values.

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

## Independent follow-up findings (2026-10-30)

The previous parent AC #4 mapping omitted the originally required subquery composition and the AC #1 inventory omitted an explicit LATERAL guard. Previous 9/9 CI was valid for the tested 24/10 inventory, but did not justify signing off the broader parent. v2 freezes `77-PARENT-DERIVED-P` and `77-PARENT-LATERAL-R`. Parent signoff is granted after both added regression contracts and all nine implementation-head CI checks passed. Final metadata PR-head CI remains a merge gate.

**Derived-subquery dialect signoff correction:** The new parent SQL preserves the TASK-137 frozen UNNEST parser dispositions: 11/13 portable `Exact`; SQLite and MSSQL are reviewed lexical/residual boundaries because bracket notation is not a certified array constructor there. The parent test directly reconciles the child-owned 13-dialect matrix instead of assuming all parser-accepted syntax is semantically equivalent.

## Final original-AC signoff

**All six original criteria are verified.** AC #1 now includes the formerly omitted LATERAL boundary with stable fail-closed Residual classification. AC #4 now covers an independently exact, noncorrelated derived typed UNNEST subquery with complete source and terminal NULL/duplicate identities and exact/Impossible membership, in addition to the established joins, GROUP and compiled dbt evidence. The exact derived case is certified in 11 parser dialect paths with reviewed SQLite/MSSQL lexical boundaries. Original AC #2, #3, #5 and #6 retain the child-owned frozen evidence and parent-level regression guard. Unsupported vendor/session behavior is never asserted exact.

**Verification:** [Implementation-head CI 41114331483](https://github.com/phdah/sql-semantic-protocol/actions/runs/41114331483) passed 9/9 jobs including formatting, strict Clippy, both Rust test feature configurations, docs, dependency checks and dbt Core E2E. TASK-77 is Done on this PR branch, with metadata PR-head CI before merge. TASK-91 remains the independent release gate.
