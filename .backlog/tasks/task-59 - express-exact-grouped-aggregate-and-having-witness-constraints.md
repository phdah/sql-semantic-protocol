---
id: TASK-59
title: Express exact grouped aggregate and HAVING witness constraints
status: To Do
assignee: []
created_date: '2026-10-08'
labels: []
milestone: m-3
dependencies: []
references:
  - 'TASK-18'
  - 'TASK-43'
  - 'TASK-44'
  - 'sql-tdg TASK-28'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Aggregates, GROUP BY and HAVING are analyzed (TASK-18), but HAVING is residual because source-row domains do not constrain group cardinality or aggregate values. Provide a protocol-side declarative constraint on groups and supported aggregates, not an ad hoc sql-tdg parser.

SQL parsing, normalized semantics, lineage, and exactness remain owned by SQL Semantic Protocol. Preserve existing exact behavior while extending the canonical contract, and never mark unsupported cases exact. Changes must uphold the repository's outcome-first definition of done and cross-adapter parity.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Represent grouped output requirements from supported COUNT, SUM, MIN, MAX and simple HAVING comparisons as typed constraints on source groups and their aggregates.
- [ ] #2 Carry group keys, distinctness, nullable input rules, and necessary/sufficient cardinality or sum bounds through supported input layers.
- [ ] #3 Prove exactness only for explicitly supported constraint classes; disjunctive and noninvertible cases remain residual, not approximated.
- [ ] #4 Test computed output-domain bounds, physical lineage and group witness constraints with DuckDB differential comparisons, including impossible outcomes.
- [ ] #5 Document the canonical contract and evaluate dbt, direct SQL and other adapters; sql-tdg TASK-28 consumes it.
<!-- AC:END -->
