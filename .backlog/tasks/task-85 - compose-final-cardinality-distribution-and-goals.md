---
id: TASK-85
title: Construct exact final cardinality and distribution targets across DAGs
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
  - TASK-71
  - TASK-72
  - TASK-73
  - TASK-74
  - TASK-75
  - TASK-76
  - TASK-79
references: 
  - 'TASK-64'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Current OutcomeWitness provides disconnected recipes for narrow standalone shapes; downstream TASK-30 needs cross-operator, multi-column output goals.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Support requested rows/groups/distincts, full histograms, NULL rates, duplicate frequencies and multi-column correlations across connected terminal transformations.
- [ ] #2 Combine join multiplicity, WHERE/QUALIFY, aggregates/HAVING, set ALL/DISTINCT, windows/frames, LIMIT/OFFSET and schema constraints into a whole-physical-source witness.
- [ ] #3 Prove feasible, impossible and residual separately; distinguish output cardinality from source row counts and expose stable offending constraints for conflicts.
- [ ] #4 Allow shared source assignments and multiple compatible terminal goals with global feasibility rather than independently satisfiable recipes.
- [ ] #5 Execute full output histograms and counts across a cross-operator suite; include impossible target and shared-source conflict fixtures.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
