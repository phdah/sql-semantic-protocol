---
id: TASK-5
title: Analyze relations joins and dependencies
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-4
---

## Description

Analyze the relational inputs of a query and expose them through the protocol. The analyzer must distinguish physical upstream dependencies from local query relations such as CTEs and subqueries while retaining aliases and join relationships.

Nested queries should contribute their physical dependencies recursively.

## Acceptance Criteria

- [ ] Multi-part relation names and aliases are represented without collapsing distinct identifiers.
- [ ] Physical tables or views referenced anywhere in a query are exposed as deterministic upstream dependencies.
- [ ] CTEs and subqueries are represented as local query relations rather than incorrectly reported as physical dependencies.
- [ ] Nested subqueries and CTEs contribute their physical dependencies recursively.
- [ ] JOIN kind, participating relations, and ON or USING semantics are represented in the protocol.
- [ ] Multiple joins and self-joins remain distinguishable through relation aliases or equivalent semantic identity.
- [ ] Unsupported table factors or join forms produce explicit diagnostics.
