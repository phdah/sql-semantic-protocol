---
id: TASK-9
title: Validate the working protocol prototype
status: Done
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-8
---

## Description

Validate the first end-to-end prototype against a representative SQL corpus through the public API. The goal is not to claim complete semantic understanding of every SQL construct, but to guarantee that queries supported by the selected sqlparser dialect either produce trustworthy semantic protocol data or explicitly identify what remains unknown or unsupported.

This task is the prototype completion gate.

## Acceptance Criteria

- [x] Integration tests exercise the public API only and assert complete protocol output for representative queries.
- [x] Coverage includes projections and aliases, joins, nested subqueries, CTEs, WHERE predicates, GROUP BY and HAVING, QUALIFY or window usage where supported, functions, and set operations or an explicit unsupported result.
- [x] Tests cover at least two materially different sqlparser dialect selections.
- [x] Representative queries verify dependencies, output columns, lineage, predicate structure, and value domains together.
- [x] Unsupported but successfully parsed constructs remain visible through diagnostics and do not silently disappear.
- [x] Parse failures are tested separately from unsupported semantic analysis.
- [x] Repeated analysis of the same query proves deterministic JSON output.
- [x] The README contains a minimal example showing SQL input and the corresponding protocol JSON once the prototype behavior is stable.
