---
id: TASK-19
title: Expand subquery and table-source semantics
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-9
---

## Description

Strengthen semantic analysis for advanced nested relational constructs so complex real-world SQL can participate in both single-query and multi-query analysis.

Focus on preserving scope, correlation, dependencies, and lineage for scalar and predicate subqueries, lateral/derived sources, and table-producing expressions supported by sqlparser.

## Acceptance Criteria

- [ ] Correlated subqueries preserve references to the correct outer scope.
- [ ] EXISTS and IN/NOT IN subqueries preserve predicate structure and physical dependencies.
- [ ] Scalar subqueries contribute lineage to the expression that consumes them.
- [ ] Derived tables preserve their internal lineage while exposing only their projected output columns to the parent scope.
- [ ] LATERAL sources are represented with correct outer-scope dependencies where supported.
- [ ] Table functions and UNNEST-like sources are represented where supported, or diagnosed explicitly when semantics remain unresolved.
- [ ] Local scopes cannot accidentally resolve to unrelated global inputs in multi-query analysis.
- [ ] Nested unsupported constructs remain visible rather than disappearing from the protocol.
- [ ] Tests cover correlated, scalar, EXISTS/IN, lateral, and table-producing source cases across supported dialects.
