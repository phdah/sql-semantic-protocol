---
id: TASK-15
title: Keep complete outcomes and identify terminal outcomes
status: Done
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-14
---

## Description

Emit one complete protocol document containing every analyzed transformation outcome and explicitly identify which of those outcomes are terminal.

Each transformation layer keeps its composed semantics so consumers can generate data for any intermediate or terminal outcome. Terminal datasets remain identified through `graph.components[].final_outcomes`. Choosing all outcomes, terminal outcomes, or one particular outcome is a consumer responsibility and must not change protocol generation.

## Acceptance Criteria

- [x] The protocol always emits every analyzed transformation layer.
- [x] Every emitted layer retains its composed semantics for outcome-focused consumption.
- [x] `graph.components[].final_outcomes` identifies every terminal named dataset across independent components.
- [x] Standalone anonymous query results are identified as terminal outcomes.
- [x] Terminal layers retain transitive lineage and physical dependencies through intermediate transformations.
- [x] The library and CLI do not expose producer-side final-versus-all output selection.
- [x] A consumer can select terminal layers from one complete protocol document without re-analysis.
- [x] Tests cover related, unrelated, named, and anonymous outcomes.
