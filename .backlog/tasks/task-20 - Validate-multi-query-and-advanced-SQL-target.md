---
id: TASK-20
title: Validate multi-query and advanced SQL target
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-15
  - TASK-16
  - TASK-17
  - TASK-18
  - TASK-19
---

## Description

Validate the expanded protocol as an end-to-end completion gate for multi-query composition and advanced SQL.

The representative corpus must prove that one invocation can combine related and unrelated SQL inputs into one trustworthy protocol document, resolve DDL-backed transformation chains, compose semantics through intermediate layers, switch between final-only and all-layer output, and analyze the advanced SQL constructs introduced by the preceding tasks.

## Acceptance Criteria

- [ ] A fixture with at least three consecutive CREATE TABLE/VIEW transformations resolves into one composed final outcome.
- [ ] The same invocation also contains at least one unrelated transformation chain or standalone query and produces all corresponding final outcomes.
- [ ] The corpus mixes SQL strings and file inputs.
- [ ] Final mode emits only terminal outcomes while preserving transitive lineage and dependencies.
- [ ] All-layer mode emits every intermediate transformation in deterministic graph order.
- [ ] Set operations and window functions appear inside composed multi-query fixtures.
- [ ] Advanced grouping and nested subquery constructs are covered by representative fixtures.
- [ ] Tests exercise multiple sqlparser dialects and confirm that dialect selection remains delegated rather than hardcoded.
- [ ] A high-input-count test proves there is no artificial fixed query-count limit in the public API.
- [ ] Repeated analysis of the same bundle produces byte-identical JSON.
- [ ] Representative final and all-layer documents validate against the current JSON Schema.
- [ ] The README documents multi-input CLI usage, final/all-layer scope selection, DDL linking, and at least one composed example.
