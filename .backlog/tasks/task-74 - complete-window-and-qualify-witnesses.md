---
id: TASK-74
title: Prove rank, analytic window and QUALIFY output witnesses
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
  - 'TASK-60'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Current ROW_NUMBER-only strict ordering contract is insufficient for common ranking and analytic filters.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Support ROW_NUMBER, RANK, DENSE_RANK, NTILE, LAG/LEAD, FIRST_VALUE/LAST_VALUE and framed aggregate windows for parser-supported SQL shapes.
- [ ] #2 Represent PARTITION BY, multiple ORDER BY keys, ASC/DESC, tie ordering, NULLS defaults per engine, COLLATE, ROWS/RANGE/GROUPS frame boundaries and peer groups.
- [ ] #3 Construct positive/rejected survivors for QUALIFY and outer projected-window predicates, including multiple window filters and interactions with grouped, joined and filtered input.
- [ ] #4 Explicitly model nondeterminism when order is not total and do not claim specific rank, frame or survivor without proven order laws.
- [ ] #5 Oracle-check complete row and rank outputs in DuckDB, plus parser and engine-specific QUALIFY variants and the current ranked_orders.sql fixture.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
