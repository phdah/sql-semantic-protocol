---
id: TASK-89
title: Differentially certify integrated transformation pipelines
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-68
  - TASK-69
  - TASK-70
  - TASK-71
  - TASK-72
  - TASK-73
  - TASK-74
  - TASK-75
  - TASK-76
  - TASK-77
  - TASK-78
  - TASK-79
  - TASK-80
  - TASK-81
  - TASK-82
  - TASK-83
  - TASK-84
  - TASK-85
  - TASK-86
  - TASK-87
  - TASK-88
references: 
  - 'sql-tdg TASK-36'
priority: high
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Operator-local unit tests are insufficient. Sign-off requires holistic constructive plans that generate a complete physical state and produce exact SQL results.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Add deterministic small-instance and seeded/property differential tests validating positive, rejected, impossible and residual claims against DuckDB (and available dialect engines).
- [ ] #2 Cover combinations: filtered joins -> groups/HAVING -> windows/QUALIFY -> sets, correlated subqueries after joins, repeated/self joins, multi-source OR, target constraints, DML/DDL and ordered scripts.
- [ ] #3 Run paired protocol + sql-tdg integration against the committed m-3 dbt fixture, including source constraints, inferred source counts, all terminal outputs and expected negatives.
- [ ] #4 Persist reproducible SQL, seed, input data, protocol snapshot, output snapshots and reason diagnostics for every counterexample.
- [ ] #5 No false exact result allowed; fail CI on any semantic mismatch, missing outcome coverage or disabled fixture.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
