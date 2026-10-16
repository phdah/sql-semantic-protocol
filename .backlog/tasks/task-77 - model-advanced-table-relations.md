---
id: TASK-77
title: Cover nonstandard relational table transformations
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-66
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
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
- [ ] #1 Inventory and normalize VALUES, LATERAL/UNNEST/EXPLODE, array and JSON table expansion, PIVOT/UNPIVOT, generator/table functions and dialect-supported SELECT star variants (EXCEPT/REPLACE/RENAME).
- [ ] #2 Model row fanout, flattening, positional ordinality, shape-dependent output types, NULL/empty arrays and pivot-generated columns with typed witness obligations.
- [ ] #3 Support safe deterministic built-in shapes while marking opaque table functions, dynamic-schema sources or nondeterministic operators residual with stable reasons.
- [ ] #4 Compose witnesses through joins, aggregates, subqueries and dbt models; never infer nested data types or fabricated columns.
- [ ] #5 Add per-dialect parser fixture and executable oracle coverage for deterministic supported variants.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-77 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: VALUES multirow columns, deterministic array UNNEST/EXPLODE with NULL/empty and WITH ORDINALITY when parsed, plus supported SELECT star EXCEPT/REPLACE/RENAME.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: one deterministic fanout through join or aggregation with preserved source-instance and result types; PIVOT/UNPIVOT fixed-schema values tested.
3. **Conditional and non-exact boundary:** Inventory conditional: dynamic JSON tables, opaque generator/table functions, variable pivot schema and vendor-specific expansion, with explicit schema and determinism guards.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
