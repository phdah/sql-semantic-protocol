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

The SQL Semantic Protocol remains the authoritative representation. Its lineage model should be designed so dataset-level and column-level lineage can be exported cleanly to OpenLineage without constraining the protocol to OpenLineage's model. Outcome semantics that OpenLineage cannot represent, such as predicates, value domains, and richer transformation semantics, remain native protocol concepts.

## Acceptance Criteria

- [ ] Final columns can expose transitive lineage through any number of linked intermediate datasets.
- [ ] Transitive physical dependencies resolve back to external/base relations where possible.
- [ ] Safe column renames and direct projections preserve column identity through multiple layers.
- [ ] Predicate/value-domain information is propagated across layers only when the transformation preserves the required semantics.
- [ ] Non-invertible or unsupported expressions stop precise propagation and produce explicit unknown/diagnostic information.
- [ ] Join-derived and multi-source lineage remains correctly multi-valued after composition.
- [ ] Dataset and column identities used by the lineage model can be mapped deterministically to OpenLineage dataset and field-level lineage concepts.
- [ ] An OpenLineage export adapter can emit the lineage information representable by OpenLineage without making OpenLineage types part of the core protocol model.
- [ ] OpenLineage export does not discard, weaken, or replace richer outcome semantics in the SQL Semantic Protocol.
- [ ] Composition works independently for every disconnected graph component.
- [ ] Composition does not depend on the order of supplied SQL inputs.
- [ ] Tests cover at least three consecutive transformations and verify final transitive lineage and constraints.
- [ ] Tests verify representative dataset-level and column-level lineage can be exported to OpenLineage while the original protocol retains its richer semantic information.
