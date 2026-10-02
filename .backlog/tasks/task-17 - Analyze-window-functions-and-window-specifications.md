---
id: TASK-17
title: Analyze window functions and window specifications
status: Done
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-9
---

## Description

Add semantic support for window functions and their window specifications rather than treating them only as generic functions or unsupported query structure.

The protocol should preserve the function arguments, PARTITION BY dependencies, ORDER BY dependencies, named windows, frame definitions, and QUALIFY relationships needed to understand the output and row-selection semantics.

## Acceptance Criteria

- [x] Window-function expressions are distinguishable from ordinary scalar or aggregate functions in the semantic model.
- [x] Function arguments contribute to output lineage.
- [x] PARTITION BY expressions and ORDER BY expressions are represented and contribute dependencies/lineage.
- [x] Supported ROWS, RANGE, and GROUPS frame definitions are represented without parser-specific types.
- [x] Named windows are resolved deterministically within their local query scope.
- [x] QUALIFY predicates can reference window outputs without losing lineage or dependencies.
- [x] Window functions do not imply value-domain constraints unless those constraints can be proven safely.
- [x] Unsupported window options remain explicit diagnostics rather than being silently dropped.
- [x] Tests cover ranking, aggregate windows, named windows, frames, and QUALIFY where supported.
