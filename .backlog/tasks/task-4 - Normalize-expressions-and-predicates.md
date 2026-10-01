---
id: TASK-4
title: Normalize expressions and predicates
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-1
  - TASK-2
  - TASK-3
---

## Description

Represent SQL expressions and predicates as typed semantic protocol values instead of rendered SQL strings. The representation must preserve logical structure so consumers can reason about the actual predicate rather than a flattened list of comparisons.

The first prototype should cover the common predicate forms needed to describe query restrictions while preserving unknown expressions explicitly when analysis is incomplete.

## Acceptance Criteria

- [ ] Protocol expressions distinguish column references, literals, functions, unary operations, binary operations, and unresolved expressions.
- [ ] Predicate trees preserve nested `AND`, `OR`, and `NOT` semantics.
- [ ] Comparison operators are represented by typed protocol values rather than arbitrary strings.
- [ ] Common predicates including equality, inequality, ordered comparisons, `BETWEEN`, `IN`, and null checks are represented semantically.
- [ ] WHERE, HAVING, QUALIFY, and JOIN predicates can use the same semantic expression model without losing their clause context.
- [ ] Reversed comparisons such as `10 < a` retain equivalent semantics to `a > 10`.
- [ ] Unsupported expressions are retained as unknown or unsupported rather than dropped.
