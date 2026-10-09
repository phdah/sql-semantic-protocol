---
id: TASK-67
title: Define a composable typed constructive witness algebra
status: In Progress
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-66
references: 
  - 'TASK-58'
  - 'TASK-59'
  - 'TASK-60'
  - 'TASK-61'
  - 'TASK-62'
  - 'TASK-63'
  - 'TASK-64'
  - 'TASK-65'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Existing witness types prove individual operators but do not compose into an executable complete-source plan. Establish a canonical, typed constraint IR with typed identities, shared variables, rows, groups and state, consumed verbatim by a generator.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Expose bounded existential/for-all source row obligations, equality/inequality and multi-column tuple constraints, candidate counts, absence/closed-world obligations, NULL truth rules, state transitions, output targets and provenance.
- [ ] #2 Support AND/OR/NOT over witness cases without interpreting AST in sql-tdg. Distinguish necessary versus sufficient, feasible versus impossible versus residual, and independent matching and rejected directions.
- [ ] #3 Represent intermediate boundaries as proof obligations with explicit physical-source realization, not writable standalone source tables.
- [ ] #4 Provide a deterministic Rust public API and versioned JSON schema; update protocol documentation and existing witness adapters without duplicate competing IRs.
- [ ] #5 Prove equivalence to existing local witness contracts on fixtures and add negative tests for impossible or unsatisfiable coupled obligations.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Implementation progress (2026-10-09)

- Introduced a typed source-independent obligation and proof-case algebra, inclusive cardinality invariants, shared row variables, tuple predicates, NULL-aware Boolean truth and physical/intermediate producer boundaries.
- Added source-local Boolean, Join, Group and Window normalization and an optional closed JSON emission contract; unlike producer-graph plans, this is deliberately only operator-local evidence.
- Added contradiction checks for incompatible output/state cardinalities and conflicting truth of the same predicate on the same row.
- Still open: fully canonical translations for set and subquery witnesses; proof-strength completeness; source-row existential/universal solver; full physical boundary realization and adapter-equivalence/differential certification. These acceptance criteria remain unchecked. TASK-68 owns graph composition, but TASK-67 must not be marked Done before its own remaining criteria and verification pass.
