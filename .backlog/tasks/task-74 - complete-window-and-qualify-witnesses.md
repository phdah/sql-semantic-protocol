---
id: TASK-74
title: Prove rank, analytic window and QUALIFY output witnesses
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
references: 
  - 'TASK-60'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Current ROW_NUMBER-only strict ordering contract is insufficient for common ranking and analytic filters.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Support ROW_NUMBER, RANK, DENSE_RANK, NTILE, LAG/LEAD, FIRST_VALUE/LAST_VALUE and framed aggregate windows for parser-supported SQL shapes.
- [ ] #2 Represent PARTITION BY, multiple ORDER BY keys, ASC/DESC, tie ordering, NULLS defaults per engine, COLLATE, ROWS/RANGE/GROUPS frame boundaries and peer groups.
- [ ] #3 Construct positive/rejected survivors for QUALIFY and outer projected-window predicates, including multiple window filters and interactions with grouped, joined and filtered input.
- [ ] #4 Explicitly model nondeterminism when order is not total and do not claim specific rank, frame or survivor without proven order laws.
- [ ] #5 Oracle-check complete row and rank outputs in DuckDB, plus parser and engine-specific QUALIFY variants and the current ranked_orders.sql fixture.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-74 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: ROW_NUMBER, RANK, DENSE_RANK on explicit total or tied ordering; LAG/LEAD, FIRST/LAST_VALUE and a bounded ROWS frame, retaining NULL and peer semantics.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: PARTITION BY plus QUALIFY and one upstream filter/join or aggregate on a complete known bag, with both survivor and rejected output.
3. **Conditional and non-exact boundary:** Inventory conditional: NTILE, RANGE/GROUPS frames, COLLATE and engine-default NULL ordering; unproven ties/order/session assumptions must not claim exact ranks.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
