---
id: TASK-75
title: Prove correlated, quantified and nested subqueries
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
  - TASK-129
  - TASK-130
  - TASK-131
  - TASK-132
references: 
  - 'TASK-62'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Current subquery witness supports simple EXISTS/IN on direct physical single-relation inputs only.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Support EXISTS/NOT EXISTS, IN/NOT IN, scalar subqueries, ANY/SOME/ALL comparisons, correlated and uncorrelated forms, nesting, multiple correlation keys and aggregation or joins inside subqueries.
- [ ] #2 Prove outer/inner row multiplicity, scalar zero/one/multiple-row semantics, NULL UNKNOWN behavior, empty results and full anti-membership absence through intermediate producers.
- [ ] #3 Support semantically equivalent LATERAL/APPLY where parser-supported and keep all source/alias scopes unambiguous.
- [ ] #4 Expose exact qualifying and rejected cases, impossible cardinalities, typed correlation values and schema/comparison assumptions without delegating operator inference to generator.
- [ ] #5 DuckDB-test full results for the committed subquery_orders.sql and negative/null/duplicate/correlated fixtures.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-75 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: correlated and uncorrelated EXISTS/NOT EXISTS, IN/NOT IN with NULL UNKNOWN, and scalar subquery zero/one/multiple rows.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: correlated one-key and composite-key membership through a join or grouped inner query with explicit outer-row identities and rejected/absence proofs.
3. **Conditional and non-exact boundary:** Inventory conditional: ANY/SOME/ALL, nested correlation and LATERAL/APPLY parser variants, with multiplicity and engine ambiguity explicitly captured.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Dependency-ordered bounded child queue (2026-10-17)

**TASK-75 is now a parent signoff, not one implementation PR.** Its original AC remain unchecked. Every child has its own finite variant/fixture IDs, complete source/terminal oracle and explicit conditional boundaries in `docs/backlog-child-queue-plan.json`. Select the first unfinished dependency-ready child; never add a parent dependency to a child. Do not sign off the parent merely because one PR passes CI.

- **[TASK-129](task-129%20-%20prove-exists-and-not-exists-correlated-uncorrelated.md)** (1/4): single-key correlation; duplicate inner bags; empty inner; TRUE/NOT TRUE absence. Fixtures `75-C129-P`, `75-C129-N`.
- **[TASK-130](task-130%20-%20prove-in-and-not-in-three-valued-membership.md)** (2/4): correlated/uncorrelated IN/NOT IN; NULL inner or outer; compound key. Fixtures `75-C130-P`, `75-C130-N`.
- **[TASK-131](task-131%20-%20model-scalar-subqueries-any-all-and-nested-correlation-boundaries.md)** (3/4): scalar subquery zero/one/multiple rows; ANY/SOME/ALL; nesting; LATERAL/APPLY. Fixtures `75-C131-P`, `75-C131-N`.
- **[TASK-132](task-132%20-%20compose-grouped-joined-correlations-and-independently-certify-task-75.md)** (4/4): composite correlations through JOIN and GROUP inner; subquery_orders.sql; dbt/ODCS. Fixtures `75-C132-P`, `75-C132-N`.

**Original acceptance mapping:** #1 -> TASK-129, TASK-130, TASK-131, TASK-132; #2 -> TASK-129, TASK-130, TASK-131, TASK-132; #3 -> TASK-131, TASK-132; #4 -> TASK-129, TASK-130, TASK-131, TASK-132; #5 -> TASK-129, TASK-130, TASK-131, TASK-132; #6 -> TASK-129, TASK-130, TASK-131, TASK-132.

**Closure gate:** All child PRs merged, every original AC independently verified, every mandatory outcome/execution oracle green, conditional vendor/session cases owner-reviewed, adapter and 13-dialect matrix complete, final CI green. Neither parser coverage nor residual classification alone excuses a required exact variant. TASK-91 remains separately release-blocked.
