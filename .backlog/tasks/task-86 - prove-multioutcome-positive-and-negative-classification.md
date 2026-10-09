---
id: TASK-86
title: Define exact multi-terminal qualifying and rejected witness contracts
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
  - TASK-85
references: 
  - 'sql-tdg TASK-35'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Whole-project dbt generation must preserve all terminals while producing deliberate per-outcome negatives. The upstream contract must express the chosen semantics, not let downstream infer rejection.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Coordinate with sql-tdg TASK-35: record maintainer-approved meaning of rejected source rows across terminals; do not choose semantics implicitly.
- [ ] #2 Represent per-source-row membership vector for each terminal outcome with positive, FALSE/UNKNOWN nonmembership, proven absence, shared lineage and counterfactual membership where relevant.
- [ ] #3 Prove jointly feasible positive/negative rows for shared and disjoint sources, conflicting terminal filters, grouping and row-shaping, maintaining all enforced constraints.
- [ ] #4 Expose per-terminal provenance and stable outcome identities and independently classify impossible vs residual negative directions.
- [ ] #5 Add executable full-db-project fixture tests showing exact presence/absence in every targeted terminal output.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
- [ ] #7 Enumerate all provably constructive per-terminal rejecting alternatives with typed predicate/column identities (including composed Boolean expressions), independent FALSE/UNKNOWN/absence proofs and provenance; the generator may choose among them without SQL reparsing.
- [ ] #8 Specify a deterministic seeded-randomized selection contract that can reach every feasible alternative across seeds; demonstrate different rejected columns, preserve shared constraints, and distinguish globally impossible versus residual alternatives.
- [ ] #9 Counterexamples such as falsifying one arm of A OR B while B passes must **not** be considered rejected; prove nonmembership of each selected terminal after all alternative branches/lineage paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Maintainer decision (2026-10-09)

Per-terminal rejected-row classification and **seeded randomized rejecting predicate/column selection** are approved in [docs/coverage-signoff.md](../../docs/coverage-signoff.md). The protocol owns enumerated constructive alternatives and proof; sql-tdg TASK-35 owns unbiased or explicitly disclosed sampling over eligible choices using its injected RNG. Approval completes decision criterion #1 only; no implementation or E2E acceptance is implied.
