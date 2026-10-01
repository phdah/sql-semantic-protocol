---
id: TASK-15
title: Expose final and all-layer output scopes
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-14
---

## Description

Allow callers to choose whether the protocol exposes only final outcomes or every transformation layer while still analyzing the complete input bundle.

Final-only output means terminal datasets from every dependency-graph component plus anonymous standalone query results. Intermediate transformations may be omitted from the rendered outcome list, but their semantics must still contribute to the composed final result.

## Acceptance Criteria

- [ ] The library exposes a typed output-scope option with at least final and all-layer modes.
- [ ] The CLI exposes a documented flag for selecting final versus all-layer output.
- [ ] Final mode emits every terminal named dataset across all graph components.
- [ ] Final mode also emits standalone anonymous query results that are not consumed by another supplied transformation.
- [ ] Final outcomes retain transitive lineage and dependencies through omitted intermediate layers.
- [ ] All-layer mode emits each transformation result and its graph relationships.
- [ ] Single-query behavior remains intuitive and equivalent under both scopes where only one layer exists.
- [ ] Scope selection does not change semantic analysis, only which analyzed outcomes are exposed.
- [ ] Tests cover multiple final datasets produced from both related and unrelated input groups.
