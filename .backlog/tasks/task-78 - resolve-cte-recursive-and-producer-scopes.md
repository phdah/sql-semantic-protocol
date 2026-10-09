---
id: TASK-78
title: Model recursive CTEs and producer/derived-scope exactness
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-76
  - TASK-77
references: 
  - 'TASK-31'
  - 'TASK-44'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Named producer layers, alias scopes, WITH RECURSIVE and row-changing CTEs require proofs that survive physical materialization boundaries.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Cover ordinary and nested CTEs, derived tables, correlated LATERAL, views, materialized relations, reuse, shadowing and multi-statement dependency ordering.
- [ ] #2 Handle supported recursive CTE base/recursive terms, UNION [ALL], cycle checks and termination bounds when a finite constructive proof exists; otherwise return a typed termination residual.
- [ ] #3 Prove upstream/downstream column mapping including computed/renamed columns, fanout, aggregation, filters, limits and repeated physical dependencies.
- [ ] #4 Preserve source and produced relation identities and producer read/write semantics; forbid treating an intermediate witness as a free source insertion.
- [ ] #5 E2E-test multi-step dbt and raw scripts with selective row-changing intermediate models and reused sources.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
