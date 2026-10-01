---
id: TASK-14
title: Compose semantics across transformation layers
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-13
---

## Description

Compose semantic information through linked intermediate datasets so a final output can describe its transitive meaning rather than stopping at the immediately preceding table or view.

Composition must preserve trustworthiness. Direct renames, projections, predicates, and other semantics that can be propagated safely should be composed. Any transformation that prevents a precise conclusion must degrade to explicit unknown or unsupported semantics rather than inventing precision.

## Acceptance Criteria

- [ ] Final columns can expose transitive lineage through any number of linked intermediate datasets.
- [ ] Transitive physical dependencies resolve back to external/base relations where possible.
- [ ] Safe column renames and direct projections preserve column identity through multiple layers.
- [ ] Predicate/value-domain information is propagated across layers only when the transformation preserves the required semantics.
- [ ] Non-invertible or unsupported expressions stop precise propagation and produce explicit unknown/diagnostic information.
- [ ] Join-derived and multi-source lineage remains correctly multi-valued after composition.
- [ ] Composition works independently for every disconnected graph component.
- [ ] Composition does not depend on the order of supplied SQL inputs.
- [ ] Tests cover at least three consecutive transformations and verify final transitive lineage and constraints.
