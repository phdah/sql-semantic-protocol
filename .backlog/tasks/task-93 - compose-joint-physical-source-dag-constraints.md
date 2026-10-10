---
id: TASK-93
title: Compose jointly satisfiable physical-source DAG constraints
status: In Progress
assignee: []
created_date: '2026-10-10'
updated_date: '2026-10-10'
labels: []
milestone: m-3
dependencies:
  - TASK-67
  - TASK-69
references:
  - 'TASK-68'
  - 'TASK-70'
  - 'TASK-85'
  - 'TASK-86'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Finish the physical-source-level constraint solver for multiple dependent terminals and branches. Compose existing canonical local facts only; expression semantics owned by TASK-70 and final outcome goals owned by TASK-85/86 are not reimplemented here.

**TASK-68 child, execution step 1/8:** After foundational PR #92 is merged, implement this task in its own PR against `main`. Follow the ordered child queue declared in TASK-68; do not work around a blocked prerequisite by marking it Done.

**Release contract:** Preserve the canonical source-independent relational DAG and typed source-level constructive proof. Reuse upstream operator-local semantics and their authoritative typed evidence; do not duplicate them here or reparse SQL in sql-tdg. Prove feasible versus impossible versus residual soundly, preserve strongest justified outcome domains, and keep release-blocking coverage unverified until independently certified.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Resolve a topologically ordered reference DAG to independent, uniquely identified physical sources; preserve repeated/shared source-row identities, producer boundaries, materialization versus inline scope and write kinds.
- [ ] #2 Conjoin physical-row truth, domains, NULL-sensitive predicates, cross-row correlation and full-source completeness obligations across several producers and terminal goals; do not combine independent feasible examples as proof.
- [ ] #3 Prove feasible plans only with one consistent assignment to each shared physical row/relation; classify contradictions as impossible and unresolved/cyclic/partial/unknown evidence as typed residual.
- [ ] #4 Provide deterministic canonical Rust and protocol JSON proof/diagnostic representation usable by consumers without SQL parsing; preserve source-level lineage and proof strength.
- [ ] #5 Test compatible and incompatible filters, shared/disjoint sources, unknown counts, aliases, NULL, duplicates, multi-terminal branches and reference cycles across exposed dialects with DuckDB differential oracles.
<!-- AC:END -->

## Implementation progress

This PR adds focused typed joint-positive and joint-negative filter
proofs over one shared physical source. Source-schema-backed integer and
NULL-sensitive Boolean conditions are conjoined on the *same row identity*;
one canonical `RowTruth` requires every positive filter TRUE, or every
negative filter SQL NOT TRUE via their disjunction. Provably disjoint
conjunctions are impossible
only with an exact necessary source count, otherwise residual. Independent
physical sources continue to use separate closed-world assignments.

Remaining TASK-93 acceptance includes mixed positive/negative filters,
full multi-branch producer DAG constraints, row correlations, cardinality
interactions and comprehensive cyclic/partial cross-operator oracles.
Do not close this task or unblock TASK-94 until those gaps are verified.

## Implementation and verification

- **Parent:** [TASK-68](task-68%20-%20compose-witnesses-across-full-transformation-graphs.md); parent remains In Progress until all eight children are Done.
- **Delivery:** A focused PR into `main` with code, tests, API/schema/docs/adapter updates proportional to changed semantics, and passing CI.
- **Next:** After this task is Done, proceed to TASK-94; do not skip ahead.
- **Scope boundary:** Other TASK-70..91 work still owns individual missing operator variants, exhaustive conformance, the downstream generator, and protocol 3.0.0 release sign-off.

