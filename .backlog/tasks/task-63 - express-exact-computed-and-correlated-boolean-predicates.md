---
id: TASK-63
title: Express exact computed and correlated boolean predicates
status: In Progress
assignee: []
created_date: '2026-10-08'
labels: []
milestone: m-3
dependencies: []
references:
  - 'TASK-43'
  - 'TASK-46'
  - 'TASK-54'
  - 'sql-tdg TASK-27'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Computed filters such as CAST(a AS INT) > 5, LIKE 'x%', cross-column disjunctions and functional predicates are residual because independently sampled column domains cannot preserve their correlations.

SQL parsing, normalized semantics, lineage, and exactness remain owned by SQL Semantic Protocol. Preserve existing exact behavior while extending the canonical contract, and never mark unsupported cases exact. Changes must uphold the repository's outcome-first definition of done and cross-adapter parity.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Define typed relational/predicate constraints for a deliberately scoped invertible computed-expression class, safe LIKE-prefix conditions, and correlated AND/OR combinations.
- [ ] #2 Preserve correlations across columns and NULL, source datatype, collation, casting and comparison assumptions; no accidental Cartesian widening.
- [ ] #3 Retain default-deny residual diagnostics for unknown, noninvertible or dialect-sensitive cases instead of producing false exactness.
- [ ] #4 Assert minimal safe output domains and exact constraints through composition and physical lineage, using DuckDB differential cases.
- [ ] #5 Document the representation and tests for dialect variants; sql-tdg TASK-27 depends on this contract.
- [ ] #6 Supply typed, jointly satisfiable positive and provably rejected source witness obligations for supported expressions, retaining cross-column coupling and explicit complement/NULL semantics; mark directions that cannot be inverted exactly as residual so sql-tdg never infers correlated predicates itself.
<!-- AC:END -->

## Implementation in progress

- Added typed single-source boolean witness trees for cross-column OR, preserving same-row AND/OR coupling and FALSE/UNKNOWN rejection.
- NULL tests and catalog-proven signed-integer/literal comparisons can emit exact operator-local directions; untyped or unsupported expressions retain residual diagnostics.
- Retained witness origin and boundary across composition and documented the initial public schema; added direct, cross-dialect and DuckDB tests.
- **Still required before Done:** invertible computed expressions/CAST, safe LIKE prefixes, mixed predicate feasibility, deeper physical-lineage inversion, adapter parity fixtures and fuller differential conformance.
