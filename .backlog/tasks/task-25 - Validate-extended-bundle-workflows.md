---
id: TASK-25
title: Validate extended bundle workflows
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-20
  - TASK-21
  - TASK-22
  - TASK-23
  - TASK-24
---

## Description

Validate the complete extended bundle workflow after explicit targets, manifests, DML writes, and catalog-aware relation resolution are available.

This task is the completion gate for the broader project target: callers can describe a large heterogeneous SQL workload, select the outputs they care about, and receive one trustworthy semantic protocol across all required transformation layers.

## Acceptance Criteria

- [ ] A manifest-driven fixture contains multiple independent pipelines with mixed per-input dialects.
- [ ] At least one pipeline combines CREATE TABLE/VIEW transformations with INSERT-select or MERGE semantics.
- [ ] Catalog/schema context resolves otherwise ambiguous relation references deterministically.
- [ ] Explicit target selection returns only the requested target graphs while preserving all required upstream semantic composition.
- [ ] Both final and all-layer modes work with explicit targets.
- [ ] Direct API/CLI inputs and equivalent manifest inputs produce equivalent semantics.
- [ ] Unsupported or partially knowable DML semantics remain visible rather than being overstated.
- [ ] End-to-end validation asserts outcome value domains and interval bounds for final outputs and DML-written values, not only parsing, graph connectivity, or lineage.
- [ ] Repeated analysis produces byte-identical protocol JSON.
- [ ] README documentation shows the complete large-bundle workflow including manifest, targets, mixed dialects, and optional catalog context.
