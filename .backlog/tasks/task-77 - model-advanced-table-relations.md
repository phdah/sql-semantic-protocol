---
id: TASK-77
title: Cover nonstandard relational table transformations
status: Done
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-30'
labels: []
milestone: m-3
dependencies: 
  - TASK-66
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
  - TASK-137
  - TASK-138
  - TASK-139
  - TASK-140
references: 
  - 'TASK-19'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
UNNEST, EXPLODE, VALUES, table functions and PIVOT are realistic source-to-output transformations absent from complete constructive analysis.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Inventory and normalize VALUES, LATERAL/UNNEST/EXPLODE, array and JSON table expansion, PIVOT/UNPIVOT, generator/table functions and dialect-supported SELECT star variants (EXCEPT/REPLACE/RENAME).
- [x] #2 Model row fanout, flattening, positional ordinality, shape-dependent output types, NULL/empty arrays and pivot-generated columns with typed witness obligations.
- [x] #3 Support safe deterministic built-in shapes while marking opaque table functions, dynamic-schema sources or nondeterministic operators residual with stable reasons.
- [x] #4 Compose witnesses through joins, aggregates, subqueries and dbt models; never infer nested data types or fabricated columns.
- [x] #5 Add per-dialect parser fixture and executable oracle coverage for deterministic supported variants.
- [x] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-77 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: VALUES multirow columns, deterministic array UNNEST/EXPLODE with NULL/empty and WITH ORDINALITY when parsed, plus supported SELECT star EXCEPT/REPLACE/RENAME.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: one deterministic fanout through join or aggregation with preserved source-instance and result types; PIVOT/UNPIVOT fixed-schema values tested.
3. **Conditional and non-exact boundary:** Inventory conditional: dynamic JSON tables, opaque generator/table functions, variable pivot schema and vendor-specific expansion, with explicit schema and determinism guards.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Dependency-ordered bounded child queue (2026-10-17)

**TASK-77 is a parent signoff, not a child implementation PR.** Original AC #1..#6 are independently verified in the parent closure. Every child has its own finite variant/fixture IDs, complete source/terminal oracle and explicit conditional boundaries in `docs/backlog-child-queue-plan.json`. Select the first unfinished dependency-ready child; never add a parent dependency to a child. Do not sign off the parent merely because one PR passes CI.

- **[TASK-137](task-137%20-%20prove-values-and-array-table-fanout-with-ordinality.md)** (1/4): multirow VALUES; array UNNEST/EXPLODE; NULL/empty arrays; ordinality. Fixtures `77-C137-P`, `77-C137-N`.
- **[TASK-138](task-138%20-%20implement-star-transforms-and-fixed-schema-pivot-unpivot.md)** (2/4): SELECT * EXCEPT/REPLACE/RENAME; PIVOT and UNPIVOT fixed columns. Fixtures `77-C138-P`, `77-C138-N`.
- **[TASK-139](task-139%20-%20classify-json-table-generators-and-vendor-functions.md)** (3/4): JSON_TABLE/FLATTEN/EXPLODE; generator/table UDF; variable shape. Fixtures `77-C139-P`, `77-C139-N`.
- **[TASK-140](task-140%20-%20compose-table-fanout-with-joins-dbt-and-certify-task-77.md)** (4/4): VALUES/UNNEST fanout feeding join or group; dbt adapter; 13 dialects. Fixtures `77-C140-P`, `77-C140-N`.

**Original acceptance mapping:** #1 -> TASK-137, TASK-138, TASK-139, TASK-140; #2 -> TASK-137, TASK-138, TASK-139, TASK-140; #3 -> TASK-137, TASK-138, TASK-139, TASK-140; #4 -> TASK-140 and the parent-owned derived subquery case; #5 -> TASK-137, TASK-138, TASK-139, TASK-140; #6 -> TASK-137, TASK-138, TASK-139, TASK-140.

**Closure gate:** All child PRs merged, every original AC independently verified, every mandatory outcome/execution oracle green, conditional vendor/session cases owner-reviewed, adapter and 13-dialect matrix complete, final CI green. Neither parser coverage nor residual classification alone excuses a required exact variant. TASK-91 remains separately release-blocked.

## Independent parent closure review (2026-10-30)

TASK-137..140 are Done and merged on main. The [parent acceptance freeze](../../docs/task-77-acceptance-freeze.json) reconciles 25 mandatory and 11 conditional cases, while the [audit](../../docs/task-77-acceptance-audit.md) independently maps original AC #1..#6. [Public/native regression](../../tests/task_77_parent_closure.rs) and final PR-head CI are required for closure. TASK-91 release authorization remains separate.

## Review correction (2026-10-30)

The initial parent signoff was incomplete: original AC #4 explicitly requires subquery composition and AC #1 includes LATERAL. The initial 24/10 case matrix omitted both. The [v2 parent acceptance freeze](../../docs/task-77-acceptance-freeze.json) adds a mandatory exact source-free derived subquery (including duplicate/NULL producer identities and NOT TRUE proof), and an owned conditional correlated LATERAL fail-closed case. AC #1 and #4 passed independent regression and full implementation CI. Preserve all original child evidence; TASK-91 remains separately release-blocked.

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
TASK-77 original AC #1..#6 independently verified and Done for the frozen portable scope. Merged children TASK-137..140 contribute 24 mandatory and 10 conditional cases; the parent adds one mandatory Exact/Impossible derived UNNEST subquery preserving complete typed NULL/duplicate source and terminal bags, plus one reviewed correlated LATERAL Residual boundary. The v2 inventory therefore holds 25 mandatory and 11 conditional cases, all 13 parser dialect paths (11 exact derived UNNEST paths with explicit SQLite/MSSQL lexical boundaries) and SQL/dbt/ODCS adapter evidence. The independent native DuckDB, typed finite-domain and physical membership tests passed. Implementation CI [41114331483](https://github.com/phdah/sql-semantic-protocol/actions/runs/41114331483) passed 9/9 checks, including both Rust feature configurations, fmt, Clippy, docs and dbt Core E2E. The final signoff-metadata PR-head CI remains a merge gate, and TASK-91 release authorization is separate.
<!-- SECTION:FINAL_SUMMARY:END -->
