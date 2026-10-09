---
id: TASK-77
title: Cover nonstandard relational table transformations
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-66
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
references: 
  - 'TASK-19'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
UNNEST, EXPLODE, VALUES, table functions and PIVOT are realistic source-to-output transformations absent from complete constructive analysis.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Inventory and normalize VALUES, LATERAL/UNNEST/EXPLODE, array and JSON table expansion, PIVOT/UNPIVOT, generator/table functions and dialect-supported SELECT star variants (EXCEPT/REPLACE/RENAME).
- [ ] #2 Model row fanout, flattening, positional ordinality, shape-dependent output types, NULL/empty arrays and pivot-generated columns with typed witness obligations.
- [ ] #3 Support safe deterministic built-in shapes while marking opaque table functions, dynamic-schema sources or nondeterministic operators residual with stable reasons.
- [ ] #4 Compose witnesses through joins, aggregates, subqueries and dbt models; never infer nested data types or fabricated columns.
- [ ] #5 Add per-dialect parser fixture and executable oracle coverage for deterministic supported variants.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
