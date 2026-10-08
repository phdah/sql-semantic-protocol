---
id: TASK-64
title: Define optional exact output cardinality and distribution goals
status: To Do
assignee: []
created_date: '2026-10-08'
labels: []
milestone: m-3
dependencies: []
references:
  - 'TASK-28'
  - 'TASK-43'
  - 'TASK-59'
  - 'TASK-60'
  - 'sql-tdg TASK-30'
priority: medium
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The existing row-condition exactness contract describes qualifying source-row combinations, not requested output cardinality, group counts or distributions. A generator must distinguish source row counts from final results.

SQL parsing, normalized semantics, lineage, and exactness remain owned by SQL Semantic Protocol. Preserve existing exact behavior while extending the canonical contract, and never mark unsupported cases exact. Changes must uphold the repository's outcome-first definition of done and cross-adapter parity.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add a separate optional typed outcome-goal contract for feasible result cardinalities, group cardinalities and distributions without redefining source-row exactness.
- [ ] #2 Specify interaction with DISTINCT, join multiplicity, aggregation, windows and NULL, including unsatisfiable goal reporting.
- [ ] #3 Preserve cross-layer and multi-outcome identity and distinguish semantic facts from caller-requested goals.
- [ ] #4 Test output bounds and representative feasible/impossible goals against a SQL execution engine.
- [ ] #5 Keep backward compatibility or document appropriate protocol versioning; sql-tdg TASK-30 consumes this feature.
<!-- AC:END -->
