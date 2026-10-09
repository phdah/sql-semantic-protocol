---
id: TASK-63
title: Express exact computed and correlated boolean predicates
status: Done
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
- [x] #1 Define typed relational/predicate constraints for a deliberately scoped invertible computed-expression class, safe LIKE-prefix conditions, and correlated AND/OR combinations.
- [x] #2 Preserve correlations across columns and NULL, source datatype, collation, casting and comparison assumptions; no accidental Cartesian widening.
- [x] #3 Retain default-deny residual diagnostics for unknown, noninvertible or dialect-sensitive cases instead of producing false exactness.
- [x] #4 Assert minimal safe output domains and exact constraints through composition and physical lineage, using DuckDB differential cases.
- [x] #5 Document the representation and tests for dialect variants; sql-tdg TASK-27 depends on this contract.
- [x] #6 Supply typed, jointly satisfiable positive and provably rejected source witness obligations for supported expressions, retaining cross-column coupling and explicit complement/NULL semantics; mark directions that cannot be inverted exactly as residual so sql-tdg never infers correlated predicates itself.
<!-- AC:END -->

## Acceptance status

- #1 complete: typed same-source boolean trees, catalog-backed integer comparisons, standalone and coupled LIKE prefixes under binary-collation/no-padding attestations, lossless signed casts, identity arithmetic, and overflow-free constant offsets on widened casts.
- #2 complete: one-row AND/OR trees retain SQL NULL/UNKNOWN, repeated-column and cross-column coupling, signed source bounds, comparison assumptions, and enforced accepted-values/NOT NULL/PK restrictions without Cartesian decomposition.
- #3 complete: unsupported functions, noninvertible expressions, unproven collation, narrowing/overflow-prone casts, untyped comparisons, oversized search spaces, and unresolved relational constraints remain explicit residuals.
- #4 complete for the proven subset: exact qualifying conjunctions refine physical source and projected output value domains without splitting ORs; composition retains transitive bounds, guards against row-changing producers, and keeps typed witnesses intermediate unless physical schema parity is proven. DuckDB row-level differential checks cover SQL TRUE, FALSE, UNKNOWN, LIKE, invertible offsets, and mixed AND/OR.
- #5 complete: schema, adapter, and semantics docs describe the versioned generator-facing contract, cross-dialect NULL behavior, direct SQL/dbt compiled-SQL parity, and strict conservative boundary rules.
- #6 complete: each supported expression carries typed one-row qualifying and NOT TRUE rejected obligations, with separate proof/feasibility status and enforced-schema rechecks. Impossible directions and unsupported external/relational dependencies remain residual so sql-tdg TASK-27 does not reparse or infer SQL.

## Verification

- Rust formatting, lint, docs, tests (including no-default-features), and dbt Core end-to-end checks pass on the implemented test and proof scope.
- Exactness is deliberately limited to explicitly proven relations, casts, string comparisons, and source-row operators. Unsupported general SQL functions and foreign-key satisfiability remain residual, not silently certified.
