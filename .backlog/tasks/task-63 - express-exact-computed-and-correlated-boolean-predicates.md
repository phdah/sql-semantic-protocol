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
- [x] #3 Retain default-deny residual diagnostics for unknown, noninvertible or dialect-sensitive cases instead of producing false exactness.
- [ ] #4 Assert minimal safe output domains and exact constraints through composition and physical lineage, using DuckDB differential cases.
- [x] #5 Document the representation and tests for dialect variants; sql-tdg TASK-27 depends on this contract.
- [ ] #6 Supply typed, jointly satisfiable positive and provably rejected source witness obligations for supported expressions, retaining cross-column coupling and explicit complement/NULL semantics; mark directions that cannot be inverted exactly as residual so sql-tdg never infers correlated predicates itself.
<!-- AC:END -->

## Acceptance status

- #3 complete: unsupported expressions, unknown datatypes and dialect-sensitive constructs retain residual proofs rather than being certified exact.
- #5 complete for the introduced contract subset: schema and adapter/semantics documentation, shared-dialect NULL predicate tests, catalog-backed integer cases, and dbt/direct parity tests.
- #1 remains incomplete: ordinary lossless integer CASTs and identity arithmetic are supported, but safe LIKE prefixes and other computed classes are not yet proven.
- #2 remains incomplete: same-row correlations, signed source datatype, lossless casting, and SQL NULL are preserved; collation evidence for LIKE is not.
- #4 remains incomplete: DuckDB differential checks cover NULL, AND/OR, signed comparisons and casts; identity-only physical lineage is mapped, but full minimal output-domain and broader lineage conformance remain missing.
- #6 remains incomplete: jointly satisfiable signed integer/NULL witnesses honor enforced NOT NULL, primary-key and finite accepted-values constraints, including after adapter enrichment; unsupported expressions and unprovable relational constraints still cannot supply exact positive/negative obligations.

## Implementation in progress

- Implemented coupled single-source `AND`/`OR` trees and bounded joint satisfiability for repeated columns, preserving SQL FALSE and UNKNOWN rejection.
- Added typed source-signed integer and NULL semantics, overflow-free identity arithmetic and ordinary lossless 16-/32-/64-bit signed CAST inversion; unsupported cast variants remain residual.
- Recheck qualifying/rejected directions against enforced NOT NULL, primary-key and accepted-values constraints after adapter enrichment; unknown or dependent foreign-key evidence stays residual.
- Map coupled conditions across *identity-only* intermediate projections to one proven physical relation; nonidentity lineage retains the intermediate boundary.
- Added direct/adapter parity, cross-dialect and DuckDB row-level differential tests; regenerated the dbt terminal-outcome golden to account for newly supported conjunctions.
- **Still required before Done:** scoped safe LIKE prefixes with explicit collation and padding attestations, nonidentity computed forms with provable inversion, end-to-end minimal output-domain guarantees and more complete physical-lineage/schema-conformance proof.
