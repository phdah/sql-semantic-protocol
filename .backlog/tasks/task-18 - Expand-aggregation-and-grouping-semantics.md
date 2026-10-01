---
id: TASK-18
title: Expand aggregation and grouping semantics
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-9
---

## Description

Expand query semantics beyond the current conservative GROUP BY handling so grouped outputs and aggregate expressions can be represented directly.

The implementation should cover common portable constructs first and preserve explicit unsupported diagnostics for dialect-specific extensions that cannot yet be modeled safely.

## Acceptance Criteria

- [ ] Aggregate functions are represented as aggregate semantics rather than only generic function calls.
- [ ] GROUP BY expressions are represented and linked to output lineage.
- [ ] HAVING semantics are evaluated in the grouped-query scope.
- [ ] DISTINCT output semantics are represented explicitly.
- [ ] Aggregate FILTER clauses are represented where supported.
- [ ] GROUPING SETS, ROLLUP, and CUBE are supported where the selected dialect/sqlparser representation permits it, or remain explicitly unsupported.
- [ ] Grouped queries preserve correct physical dependencies and output-column lineage.
- [ ] Value-domain derivation remains conservative across aggregation.
- [ ] Tests cover grouped aggregates, DISTINCT, HAVING, and at least one advanced grouping construct.
