---
id: TASK-17
title: Analyze window functions and window specifications
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-9
---

## Description

Add semantic support for window functions and their window specifications rather than treating them only as generic functions or unsupported query structure.

The protocol should preserve the function arguments, PARTITION BY dependencies, ORDER BY dependencies, named windows, frame definitions, and QUALIFY relationships needed to understand the output and row-selection semantics.

## Acceptance Criteria

- [ ] Window-function expressions are distinguishable from ordinary scalar or aggregate functions in the semantic model.
- [ ] Function arguments contribute to output lineage.
- [ ] PARTITION BY expressions and ORDER BY expressions are represented and contribute dependencies/lineage.
- [ ] Supported ROWS, RANGE, and GROUPS frame definitions are represented without parser-specific types.
- [ ] Named windows are resolved deterministically within their local query scope.
- [ ] QUALIFY predicates can reference window outputs without losing lineage or dependencies.
- [ ] Window functions do not imply value-domain constraints unless those constraints can be proven safely.
- [ ] Unsupported window options remain explicit diagnostics rather than being silently dropped.
- [ ] Tests cover ranking, aggregate windows, named windows, frames, and QUALIFY where supported.
