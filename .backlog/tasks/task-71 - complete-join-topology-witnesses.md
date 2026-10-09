---
id: TASK-71
title: Construct exact join trees and dialect join variants
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
  - 'TASK-61'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
One-binary-join proofs cannot generate exact datasets for realistic multi-join, self-join and outer-join pipelines.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Cover INNER/LEFT/RIGHT/FULL/CROSS, SEMI/ANTI, SELF, USING/NATURAL, composite ON, OR/non-equi/null-safe conditions, mixed and chained join trees, alias reuse and many-to-many multiplicities.
- [ ] #2 Evaluate ASOF/range/time joins and LATERAL/APPLY constructs for each parser-supported dialect, translating supported semantics into the same typed plan.
- [ ] #3 Construct matched, unmatched, NULL-extended and rejected rows with explicit source instance identity and complete absence obligations; never treat null-extension as inserted physical NULL rows.
- [ ] #4 Compose upstream filters and downstream row-shaping, keys/FKs, nullable join columns and duplicate matches while satisfying output cardinality.
- [ ] #5 Verify exact output row identities/counts in DuckDB and dialect engines where available; unsupported engine semantics retain traceable residuals.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
