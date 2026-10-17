---
id: TASK-73
title: Prove grouped aggregates and HAVING across relational pipelines
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
  - TASK-71
  - TASK-121
  - TASK-122
  - TASK-123
  - TASK-124
references: 
  - 'TASK-59'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
GROUP BY/HAVING proof currently excludes WHERE/JOIN inputs and common aggregate variants.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Support COUNT(*)/COUNT(expr), SUM, MIN, MAX, AVG and supported typed aggregate families with DISTINCT, FILTER, multiple aggregates, aliases and grouped HAVING Boolean trees.
- [ ] #2 Prove contributions through upstream WHERE, multi-joins and projection layers, including duplicate amplification, null contributors, impossible aggregates, and multiple group identities.
- [ ] #3 Cover GROUPING SETS, ROLLUP, CUBE, GROUP BY ALL, global aggregation/empty groups and dialect-specific GROUPING semantics where parseable.
- [ ] #4 Provide exact positive and rejected existing-group constructions and final group/aggregate column value domains with typed arithmetic and overflow/decimal rules.
- [ ] #5 Validate with DuckDB full snapshots for the committed aggregate_summary.sql and independent cross-operator fixtures.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-73 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: COUNT(*), COUNT(expr), SUM, MIN, MAX, AVG with typed result domains and NULL/duplicate behavior, both grouped and global empty-input cases.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: one filtered join feeding GROUP BY/HAVING with two aggregates, DISTINCT or FILTER, and a verified positive and rejected group.
3. **Conditional and non-exact boundary:** Inventory conditional: GROUPING SETS, ROLLUP, CUBE, GROUP BY ALL and vendor GROUPING syntax; each has a parser/evaluation classification and representative oracle.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Dependency-ordered bounded child queue (2026-10-17)

**TASK-73 is now a parent signoff, not one implementation PR.** Its original AC remain unchecked. Every child has its own finite variant/fixture IDs, complete source/terminal oracle and explicit conditional boundaries in `docs/backlog-child-queue-plan.json`. Select the first unfinished dependency-ready child; never add a parent dependency to a child. Do not sign off the parent merely because one PR passes CI.

- **[TASK-121](task-121%20-%20prove-basic-grouped-and-global-aggregate-value-domains.md)** (1/4): COUNT(*), COUNT(expr), SUM, MIN, MAX, AVG; NULL/duplicate; empty global versus grouped. Fixtures `73-C121-P`, `73-C121-N`.
- **[TASK-122](task-122%20-%20prove-distinct-filter-having-and-multiple-group-identities.md)** (2/4): COUNT DISTINCT; FILTER WHERE; mixed HAVING predicates; two aggregates and aliases. Fixtures `73-C122-P`, `73-C122-N`.
- **[TASK-123](task-123%20-%20compose-aggregates-after-filtered-joins-and-classify-grouping-variants.md)** (3/4): WHERE then JOIN then GROUP/HAVING; GROUPING SETS, ROLLUP, CUBE, GROUP BY ALL. Fixtures `73-C123-P`, `73-C123-N`.
- **[TASK-124](task-124%20-%20independently-certify-task-73-group-and-aggregate-oracles.md)** (4/4): committed aggregate_summary.sql; direct/dbt/ODCS; all exposed dialect paths. Fixtures `73-C124-P`, `73-C124-N`.

**Original acceptance mapping:** #1 -> TASK-121, TASK-122, TASK-124; #2 -> TASK-122, TASK-123, TASK-124; #3 -> TASK-121, TASK-123, TASK-124; #4 -> TASK-121, TASK-122, TASK-123, TASK-124; #5 -> TASK-121, TASK-122, TASK-123, TASK-124; #6 -> TASK-121, TASK-122, TASK-123, TASK-124.

**Closure gate:** All child PRs merged, every original AC independently verified, every mandatory outcome/execution oracle green, conditional vendor/session cases owner-reviewed, adapter and 13-dialect matrix complete, final CI green. Neither parser coverage nor residual classification alone excuses a required exact variant. TASK-91 remains separately release-blocked.
