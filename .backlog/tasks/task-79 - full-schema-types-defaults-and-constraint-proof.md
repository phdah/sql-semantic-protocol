---
id: TASK-79
title: Prove schema, datatype and constraint-feasible source constructions
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-66
  - TASK-67
  - TASK-69
references: 
  - 'TASK-30'
  - 'TASK-33'
  - 'TASK-57'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Logical witness feasibility can fail under physical types, keys, foreign keys, NOT NULL, CHECK, DEFAULT, GENERATED and dialect-specific coercions.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Expand canonical schema facts for type domains, nullability, precision/scale, collation, time zone, enums, arrays/maps/structs, and deterministic coercions.
- [ ] #2 Represent PK/unique/FK/composite relationships, enforced CHECK and accepted values, defaults, generated/identity columns, computed keys and target-side constraints with provenance and enforcement status.
- [ ] #3 Provide proof/unsatisfiable/residual for each physical witness under source and target constraints, including insertion against existing rows and FK cycles.
- [ ] #4 Align direct SQL DDL, dbt manifest/catalog, ODCS and named schema inputs wherever equivalent evidence exists; unknown enforcement remains explicit.
- [ ] #5 Cover Arrow-representable types, decimal/float/NaN/date/timestamp/JSON edge cases and cross-source constraints in differential fixtures.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
