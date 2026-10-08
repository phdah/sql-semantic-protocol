---
id: TASK-51
title: Make the conformance suite enforce exactness completeness
status: Done
assignee: []
created_date: '2026-10-08 09:06'
updated_date: '2026-10-08'
labels: []
milestone: m-2
dependencies: []
references:
  - TASK-43
  - TASK-44
  - TASK-47
  - sql-tdg TASK-21.5
priority: high
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Why
The differential suite from TASK-47 checks only soundness: every scope claimed exact is confirmed by DuckDB. It never checks completeness, meaning that allow-listed shapes are actually claimed exact. Over-conservative regressions therefore pass CI and only surface downstream. At 2bc99b3, every inner equi-join inside a CTE or derived table is residual (`column_comparison`) while the inlined query is exact, and the suite stayed green. That breaks TASK-44 AC #6 and blocks sql-tdg TASK-21.5.

## Outcome
The conformance suite enforces both directions of the contract:
- **Soundness:** exact claims are confirmed by the engine.
- **Completeness:** every allow-listed shape is claimed exact, with the expected domains and join equalities, in every supported location.
- **Consistency invariants:** two that hold by contract are checked automatically for every generated case: local-relation equivalence with the inlined query, and typed-versus-untyped schema agreement.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 For every allow-listed condition shape documented in docs/protocol.md, tests assert exact status, the expected column domains, and the expected join equalities at top level, in inner-join ON, inside a CTE, inside chained CTEs, inside a derived table, and through multi-layer composition
- [x] #2 The seeded generator also wraps generated queries in CTEs, chained CTEs, and derived tables, and asserts that composed exactness status, residual reasons, column domains, and join equalities equal those of the inlined query whenever every hop is a plain column copy
- [x] #3 The seeded generator covers inner equi-joins across two and three relations, both explicit and implicit, inside and outside local relations, and verifies join equality claims with the engine
- [x] #4 The seeded generator covers typed columns of every datatype family the contract can make exact, and asserts that analysis with and without schema evidence never disagrees on exactness except for documented typed-literal rules
- [x] #5 Reproductions from this review are part of the suite: CTE and derived-table equi-joins residual, typed string, float, and timestamp predicates residual, and a daily-revenue CTE chain over three joined sources
- [x] #6 Completeness failures print the query, the location, the expected exact representation, and the actual residual reasons
- [x] #7 The suite remains deterministic and part of the standard CI check
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
PR #63 extends differential conformance with mandatory exactness completeness across scalar predicates, plain-copy CTE/derived/multi-layer paths, seeded predicate trees, two-/three-relation joins, and typed schema evidence. CI gates the same test suite in both feature configurations. The derived join-alias mapping and exactness classification uncovered by these assertions are corrected in the analyzer/composition layers. The CI suite validates the completed acceptance matrix, including deterministic seeded joins, typed scalar families, row-condition residual preservation, and an aggregate-based three-source CTE reproduction. Unsupported INTERVAL literal normalization remains an explicit residual rather than an unsupported exactness claim.
<!-- SECTION:NOTES:END -->
