---
id: TASK-72
title: Prove full set-operation trees with aligned values and bags
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
  - TASK-70
references: 
  - 'TASK-58'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Existing set witness proof is narrow and optional cardinality construction handles just one-column two-branch physical sets.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Cover UNION/INTERSECT/EXCEPT DISTINCT and ALL, dialect MINUS, nested/chained multi-branch trees, multiple columns, NULL tuple equality, duplicates and compatible mixed types.
- [ ] #2 Model positional and dialect-supported CORRESPONDING/BY NAME alignment, aliases, coercions, overlapping/disjoint values, branch-local filters/joins/groups/limits and shared physical sources.
- [ ] #3 Prove both membership directions and exact tuple frequencies at physical sources through producer layers, not independent intermediate inserts.
- [ ] #4 Respect set-level and branch-level ORDER/LIMIT/FETCH and value/collation compatibility; downgrade only specific unsupported dialect variants.
- [ ] #5 Test executable entire tree results, impossible counts, NULLs, shared sources, multi-column histograms, and dbt set model fixtures.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
