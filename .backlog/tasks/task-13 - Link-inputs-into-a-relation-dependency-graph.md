---
id: TASK-13
title: Link inputs into a relation dependency graph
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-12
---

## Description

Resolve named relations produced by one input against relations consumed by other inputs and build one deterministic dependency graph across the complete analysis bundle.

Graph construction must be independent of the order in which SQL inputs were provided. It must support multiple disconnected pipelines and standalone queries in one protocol document.

Local SQL scopes such as CTEs, aliases, and derived tables must never be mistaken for globally produced datasets.

## Acceptance Criteria

- [ ] Consumed relations are linked to matching produced relations from other supplied inputs when resolution is unambiguous.
- [ ] Linking is independent of input order.
- [ ] Multiple disconnected transformation chains coexist in one graph.
- [ ] Relations without an in-bundle producer remain explicit external dependencies.
- [ ] CTE names, aliases, and derived-table names remain local to their query scope and do not create cross-input edges.
- [ ] Multiple supplied producers for the same relation generate an explicit ambiguity diagnostic rather than arbitrary selection.
- [ ] Cycles are detected and represented explicitly rather than causing recursion or incorrect ordering.
- [ ] Graph nodes and edges serialize deterministically.
- [ ] Tests cover a multi-stage chain, independent chains, external leaves, duplicate producers, and cycles.
