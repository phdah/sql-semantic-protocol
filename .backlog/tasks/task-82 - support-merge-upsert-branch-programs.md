---
id: TASK-82
title: Prove complete MERGE and UPSERT branch/state semantics
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
  - TASK-75
  - TASK-79
  - TASK-80
  - TASK-81
references: 
  - 'TASK-65'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Current MERGE only carries conditional effects and leaves match multiplicity, branch selection and conflicts residual.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Normalize ordered WHEN MATCHED/NOT MATCHED BY TARGET/BY SOURCE clauses, UPDATE/DELETE/INSERT actions, additional predicates, ON expressions and dialect UPSERT equivalents.
- [ ] #2 Prove complete source-target match partitions and branch precedence, repeated source matches, target uniqueness, NULL match semantics and ambiguous engine behaviors.
- [ ] #3 Expose source and initial target construction plus exact final rows and independent rejected/nonmatching cases with conflict and idempotence outcomes.
- [ ] #4 Cover computed assignments, UPDATE/DELETE/INSERT branch combinations, partial target constraints and multiple sequential MERGEs.
- [ ] #5 DuckDB execute complete before/action/after snapshots, then versioned per-dialect syntax and available-engine checks; no promise of portability where semantics differ.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
