---
id: TASK-76
title: Handle row selection, ordering, pagination and sampling exactly
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
  - TASK-74
references: 
  - 'TASK-43'
  - 'TASK-64'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ORDER BY/LIMIT/OFFSET/FETCH/TOP, DISTINCT ON and sampling can invalidate seemingly exact row-membership and output counts.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Normalize LIMIT/OFFSET/FETCH WITH TIES/TOP, ORDER BY, DISTINCT/DISTINCT ON, QUALIFY placement and dialect-specific syntactic equivalents as typed row-selection semantics.
- [ ] #2 Derive exact candidate ranks, ties, offset windows and qualifying/rejected memberships with unique-order preconditions when necessary.
- [ ] #3 Describe deterministic versus stochastic TABLESAMPLE/SAMPLE with seed and sampling semantics; explicitly residual when exact output membership is inherently unprovable.
- [ ] #4 Compose inner/outer row limits across CTEs, set branches and producer layers, including empty and impossible output sizes.
- [ ] #5 Execute dialect-compatible full result tests under deterministic settings and add residual/no-overclaim cases.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
