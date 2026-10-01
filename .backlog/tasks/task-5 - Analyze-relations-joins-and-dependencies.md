---
id: TASK-5
title: Analyze relations joins and dependencies
status: Done
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

- [x] Multi-part relation names and aliases are represented without collapsing distinct identifiers.
- [x] Physical tables or views referenced anywhere in a query are exposed as deterministic upstream dependencies.
- [x] CTEs and subqueries are represented as local query relations rather than incorrectly reported as physical dependencies.
- [x] Nested subqueries and CTEs contribute their physical dependencies recursively.
- [x] JOIN kind, participating relations, and ON or USING semantics are represented in the protocol.
- [x] Multiple joins and self-joins remain distinguishable through relation aliases or equivalent semantic identity.
- [x] Unsupported table factors or join forms produce explicit diagnostics.
