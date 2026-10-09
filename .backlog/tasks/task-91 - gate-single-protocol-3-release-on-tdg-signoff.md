---
id: TASK-91
title: Block 3.0.0 release until downstream generator acceptance
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
  - TASK-89
  - TASK-90
references: 
  - 'sql-tdg TASK-24..36'
  - 'PR #79'
priority: high
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
One consolidated 3.0.0 release is requested. The existing Release Please PR #79 must remain unmerged until the generator-relevant protocol contract is complete and verified.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 All protocol TASK-66..90 and release-blocking coverage-manifest entries are Done; no unchecked acceptance criteria or undocumented exclusions remain.
- [ ] #2 Candidate Rust fmt/clippy/test/doc, dbt Core E2E, no-default-features, schema snapshots, dialect and cross-feature differential tests are green.
- [ ] #3 sql-tdg integration branch consumes pinned *protocol release-candidate Git SHA*, with no dependency on an already published 3.0.0 crate, and passes generator TASK-24..31, TASK-35 and TASK-36 acceptance where upstream-dependent.
- [ ] #4 Decide and document final scope for arbitrary UDFs, stochastic operators, nonterminating recursion and unavailable vendor engines with explicit fail-closed behavior. No universal-support claims.
- [ ] #5 Maintainer signs off complete feature/dialect matrix and final dbt Makefile workflow before PR #79 is made ready/merged. Only then publish v3.0.0 once and switch sql-tdg to the published crate.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
