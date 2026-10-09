---
id: TASK-67
title: Define a composable typed constructive witness algebra
status: Done
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
- [x] #1 Expose bounded existential/for-all source row obligations, equality/inequality and multi-column tuple constraints, candidate counts, absence/closed-world obligations, NULL truth rules, state transitions, output targets and provenance.
- [x] #2 Support AND/OR/NOT over witness cases without interpreting AST in sql-tdg. Distinguish necessary versus sufficient, feasible versus impossible versus residual, and independent matching and rejected directions.
- [x] #3 Represent intermediate boundaries as proof obligations with explicit physical-source realization, not writable standalone source tables.
- [x] #4 Provide a deterministic Rust public API and versioned JSON schema; update protocol documentation and existing witness adapters without duplicate competing IRs.
- [x] #5 Prove equivalence to existing local witness contracts on fixtures and add negative tests for impossible or unsatisfiable coupled obligations.
- [x] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Acceptance verification (2026-10-09)

**TASK-67 complete as the canonical operator-local proof algebra**, not as full transitive generator sign-off. See [PR #89](https://github.com/phdah/sql-semantic-protocol/pull/89).

- **#1** `src/constructive.rs` provides typed shared source-row identities, bounded existential/universal sets, correlated equality/inequality and NULL-sensitive tuples, absent partners, group/rank cardinality, typed states, output targets and originating-layer provenance. Existing `WriteStateEffect` remains authoritative for actual DML branches and before/after law; `StateRows` represents a compatible count target without duplicating mutation semantics.
- **#2** Directional logical AND/OR/NOT with SQL three-valued guards, independent matching/rejection, necessary/sufficient/equivalent strengths, and feasible/impossible/residual results. Unproven joint satisfiability of *distinct sufficient examples* stays residual rather than guessed feasible/impossible.
- **#3** `local_pending_producers` emits explicit intermediate boundaries and required physical leaf dependencies. They remain producer-realization obligations, **never directly writable sources**; full cross-layer discharge belongs to TASK-68.
- **#4** Deterministic Rust APIs (`local_constructive_witnesses`, `local_pending_producers`), optional canonical `constructive_witnesses` and `constructive_pending_producers` JSON with closed active-schema definitions, and documentation in `docs/protocol.md`. All new evidence is derived from existing local witness owners, not a second independent analyzer.
- **#5** Tests assert lossless operator-local truth direction and case count/bounds for Boolean, Join, Group, Window, Subquery and Set witnesses, and reject conflicting counts, contradictory SQL truth, unsupported forms and insufficient conjunctive proof.
- **#6** Cross-dialect Boolean fixtures use all exposed dialect names, existing set fixtures cover dialect/operator laws, DuckDB executes positive/rejected/NULL/duplicate SQL outcomes, and tests compare direct SQL with dbt artifact metadata and canonical schema-source kinds. Regular CI checks formatting, lint, unit/integration tests and dbt E2E.

**Scope boundary:** Local witness feasibility does not certify shared-row physical-source realizability across producer DAGs, every expression variant, terminal-level rejection completeness, or all dialect/engine laws. Those are explicitly release-blocking under TASK-68..91 and downstream sql-tdg m-3. Release Please PR #79 remains on hold.
