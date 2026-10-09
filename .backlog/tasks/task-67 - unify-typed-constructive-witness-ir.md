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

- Added typed canonical case obligations for shared source rows, quantified candidate sets, joins, groups, ordered window predecessors, correlated EXISTS/IN truth, NULL-safe set tuple multiplicities, output and state targets and provenance.
- Added SQL three-valued AND/OR/NOT directional composition, bounded case enumeration, direct contradiction checks, and explicit intermediate producer obligations. No intermediate relation is deemed a directly writable source.
- Added optional active JSON schema fields and deterministic Rust public entry points. Original operator-local witnesses are preserved during the migration.
- Added Rust unit cases, cross-dialect Boolean fixtures, DuckDB set/NULL differential evidence and direct-SQL/dbt adapter parity tests. CI status must be verified on the final branch commit.
- **Still pending acceptance verification:** complete schema and structural invariants, all expected adapter equivalences, deterministic proof parity on all relevant operator fixtures and all relevant cross-dialect/differential cases. TASK-68 owns full DAG realizability; TASK-67 must not claim it is solved by local proof normalization.
