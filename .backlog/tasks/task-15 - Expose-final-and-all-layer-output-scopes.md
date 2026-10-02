---
id: TASK-15
title: Expose final and all-layer output scopes
status: Done
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-14
---

## Description

Allow callers to choose whether the protocol exposes only final outcomes or every transformation layer while still analyzing the complete input bundle.

Final-only output means terminal datasets from every dependency-graph component plus anonymous standalone query results. Intermediate transformations may be omitted from the rendered outcome list, but their semantics must still contribute to the composed final result.

## Acceptance Criteria

- [x] The library exposes a typed output-scope option with at least final and all-layer modes.
- [x] The CLI exposes a documented flag for selecting final versus all-layer output.
- [x] Final mode emits every terminal named dataset across all graph components.
- [x] Final mode also emits standalone anonymous query results that are not consumed by another supplied transformation.
- [x] Final outcomes retain transitive lineage and dependencies through omitted intermediate layers.
- [x] All-layer mode emits each transformation result and its graph relationships.
- [x] Single-query behavior remains intuitive and equivalent under both scopes where only one layer exists.
- [x] Scope selection does not change semantic analysis, only which analyzed outcomes are exposed.
- [x] Tests cover multiple final datasets produced from both related and unrelated input groups.
