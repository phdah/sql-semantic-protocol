---
id: TASK-6
title: Analyze output columns and lineage
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-4
  - TASK-5
---

## Description

Describe the final columns produced by a query and where their values originate. Output analysis must preserve projection order, aliases, semantic expressions, and source-column lineage across direct references, expressions, subqueries, and CTEs where resolution is possible.

The output section describes the columns that remain after the complete query rather than every intermediate column encountered during analysis.

## Acceptance Criteria

- [ ] Final projection columns are emitted in query output order with their resolved names or aliases.
- [ ] Each output column retains its semantic expression.
- [ ] Direct column projections resolve to their source relation and source column.
- [ ] Expressions can report lineage from every source column that contributes to the result.
- [ ] Lineage is propagated through resolvable CTEs and subqueries.
- [ ] Aggregate and window expressions retain their source-column lineage where it can be determined.
- [ ] Wildcards that cannot be expanded without external schema information are represented explicitly as unresolved rather than guessed.
- [ ] Columns used only for filtering or joins are not incorrectly reported as final output columns.
