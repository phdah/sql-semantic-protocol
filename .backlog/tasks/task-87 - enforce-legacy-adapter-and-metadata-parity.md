---
id: TASK-87
title: Ensure SQL, dbt, ODCS and schema evidence parity
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-66
  - TASK-68
  - TASK-79
  - TASK-80
  - TASK-81
  - TASK-82
  - TASK-83
references: 
  - 'TASK-27'
  - 'TASK-35'
  - 'TASK-57'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
New canonical semantics must flow through every source that can supply equivalent facts and must not regress rich dbt artifacts or warehouse metadata.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Map source of truth per fact: compiled model SQL, dependency graph, declared constraints/tests, catalog warehouse schema, ODCS schema evidence and direct SQL DDL.
- [ ] #2 Implement same canonical typed evidence across adapters where available; report absent inputs or unsupported metadata as explicit diagnostics, not guessed contracts.
- [ ] #3 Cover dbt ephemeral/view/table/incremental materialization evidence without treating SELECT-only manifests as executed stateful writes.
- [ ] #4 Resolve schema-qualified identities, quoted names, model tests, FK-only sources, source/target types and cross-layer cardinality obligations.
- [ ] #5 Add paired SQL/dbt/ODCS equivalence snapshots and adapter-specific inability/residual tests for new fields.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
