---
id: TASK-7
title: Derive column value domains
status: Done
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-4
  - TASK-5
---

## Description

Derive safe value-domain information for referenced columns from query predicates. A domain describes values that may satisfy the analyzed query and must remain conservative whenever exact inference is impossible.

The model must handle non-contiguous domains rather than assuming every restriction is a single interval.

## Acceptance Criteria

- [x] Ordered comparisons derive open or closed lower and upper bounds for a column when the bound is safely understood.
- [x] Equality, inequality, `BETWEEN`, `IN`, and null predicates contribute to the column domain where safe.
- [x] Conjunctions intersect compatible domains.
- [x] Disjunctions preserve unions or disjoint domains where safe rather than flattening them into an incorrect intersection.
- [x] Excluded values such as `a != 10` can be represented without losing the two allowed sides of the domain.
- [x] Contradictory predicates can produce an explicit empty or unsatisfiable domain.
- [x] A referenced column with no useful restriction remains explicitly unbounded or unknown as appropriate.
- [x] Column-to-column predicates and dynamic expressions are retained as relational constraints or unknown bounds when they cannot safely become scalar ranges.
- [x] Domains are derived for constrained source columns even when those columns are not part of the final output.
