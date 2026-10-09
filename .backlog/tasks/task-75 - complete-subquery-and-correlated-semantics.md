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

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
