---
id: TASK-69
title: Prove bag, duplicate and closed-world row-count semantics
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-67
references: 
  - 'TASK-58'
  - 'TASK-61'
  - 'TASK-64'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Set duplicate arithmetic and local match counts do not suffice for joins/groups/windows with global row conservation or negative witnesses.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Model multiplicity of row/tuple identities under projection, DISTINCT, joins, set operators, aggregation, rank filtering, insert/delete/update and no-match conditions.
- [ ] #2 Define exact cardinality transfer functions and admissible bounds under NULL-aware equality, bag semantics, duplicate keys, zero rows, and cross-row correlations.
- [ ] #3 Expose complete-physical-relation closed-world obligations for absence and anti-join/subquery/EXCEPT cases; distinguish empty relation from absent candidate.
- [ ] #4 Cover feasible/impossible count goals and preserve constraints on copies, self joins and aliases without inventing independent physical tables.
- [ ] #5 Differential-test all counts and output histograms on DuckDB including many-to-many joins, duplicate cancellations, empty sets and NULL tuples.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
