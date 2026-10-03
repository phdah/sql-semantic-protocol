---
id: TASK-20
title: Validate multi-query and advanced SQL target
status: Done
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
  - TASK-28
---

## Description

Validate the expanded protocol as an end-to-end completion gate for multi-query composition and advanced SQL.

The representative corpus proves that one invocation can combine related and unrelated SQL inputs into one trustworthy protocol document, resolve DDL-backed transformation chains, compose semantics through intermediate layers, retain every analyzed outcome while identifying terminal outcomes, and analyze the advanced SQL constructs introduced by the preceding tasks.

The producer does not expose final-only versus all-layer modes. TASK-15 established one complete document containing every layer, with `graph.components[].final_outcomes` identifying terminal datasets for consumer-side selection.

## Acceptance Criteria

- [x] A fixture with at least three consecutive CREATE TABLE/VIEW transformations resolves into one composed final outcome.
- [x] The same invocation also contains at least one unrelated transformation chain or standalone transformation and produces all corresponding final outcomes.
- [x] The corpus mixes SQL strings and file inputs.
- [x] One complete document exposes every intermediate layer while `graph.components[].final_outcomes` identifies terminal outcomes with transitive lineage and dependencies.
- [x] Set operations and window functions appear inside the composed multi-query corpus.
- [x] Advanced grouping and nested subquery constructs are covered by representative fixtures.
- [x] Derived output-domain semantics are covered by representative fixtures, including CASE-derived boolean output and a constrained ROW_NUMBER outcome.
- [x] Tests exercise every dialect exposed by the project through `sqlparser::dialect::dialect_from_str`, without a production dialect whitelist.
- [x] A high-input-count test proves there is no artificial fixed query-count limit in the public API.
- [x] Repeated analysis of the same bundle produces byte-identical JSON.
- [x] Representative emitted documents validate against the current JSON Schema, including definitions referenced from the historical base schema.
- [x] The README documents mixed multi-input CLI usage, DDL linking, terminal-outcome identification, and a composed example.
