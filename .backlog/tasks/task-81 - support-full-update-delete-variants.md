---
id: TASK-81
title: Prove UPDATE and DELETE with joined/correlated source selection
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
references: 
  - 'TASK-65'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
UPDATE FROM and DELETE USING, multiple aliases and ordered predicates are not captured by the narrow current write subset.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Support UPDATE SET with computed/DEFAULT expressions, UPDATE FROM, DELETE USING, joins/subqueries/CTEs, RETURNING, qualified targets and dialect-permitted ORDER/LIMIT/partition conditions.
- [ ] #2 Represent row identity and matched source cardinality, one target row's action precedence, NULL/UNKNOWN predicate truth and cross-table mutation constraints.
- [ ] #3 Construct exact affected/unchanged/prestate/poststate and positive/negative rows with key/FK and type checks; reject dialect ambiguous multiple-match updates rather than guess.
- [ ] #4 Prove interaction with INSERT/MERGE in ordered scripts and output domains for RETURNING where supported.
- [ ] #5 Test multi-table and correlated UPDATE/DELETE in engine fixtures including non-matches, duplicates, unknown comparison assumptions and constraints.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
