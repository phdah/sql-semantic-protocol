---
id: TASK-70
title: Construct typed correlated predicate and expression outcomes
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-67
  - TASK-79
references: 
  - 'TASK-46'
  - 'TASK-63'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Current boolean proof supports a restricted scalar subset. Expand safe, typed expression constraints without requiring sql-tdg to infer SQL semantics.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Cover arithmetic and overflow, unary/binary comparisons, BETWEEN/IN lists, IS NULL / IS DISTINCT FROM, NULL-safe equality, CASE, COALESCE, NULLIF, supported CAST/TRY_CAST, string LIKE/NOT LIKE/ESCAPE and expressions in join, filter, grouping and assignments.
- [ ] #2 Support mixed AND/OR/NOT, cross-column comparisons, computed projections and aliases with SQL three-valued logic and reversible typed constraints.
- [ ] #3 Catalog which dialect-sensitive regex/pattern, collations, implicit casts, date/time, JSON or vendor function families have safe exact semantics; opaque UDFs must retain explicit residuals.
- [ ] #4 Give canonical positive and NOT TRUE negative witnesses, impossible cases, output domains, comparison settings and deterministic literal coercion.
- [ ] #5 Add direct SQL, dbt and representative per-dialect parser/oracle tests for each supported expression family and combined predicates.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
