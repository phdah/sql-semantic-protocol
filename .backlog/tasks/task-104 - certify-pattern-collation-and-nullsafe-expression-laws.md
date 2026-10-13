---
id: TASK-104
title: Certify pattern, collation and null-safe expression laws
status: To Do
assignee: []
created_date: '2026-10-13'
updated_date: '2026-10-13'
labels: []
milestone: m-3
dependencies:
  - TASK-103
references:
  - 'TASK-70'
  - 'TASK-88'
  - 'TASK-66'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Complete the scoped text/pattern and dialect-specific comparison audit. Exact claims require explicit operator, datatype, collation, padding and dialect/session evidence; unsupported regex or pattern modes remain intentionally residual.

**TASK-70 child, step 4/8.** Implement this task in a fresh PR targeting `main` once the previous dependency has merged. The TASK-70 parent owns the [dependency-ordered queue](task-70%20-%20solve-expression-and-predicate-variants.md); this child MUST NOT depend on the parent (no dependency cycle). A green, bounded increment is mergeable but does not complete this child unless every criterion below is independently verified.

**Release contract:** Produce canonical, source-independent, parser-agnostic typed outcomes with strongest provable value domains and deterministic proof strength (exact, impossible, residual) for SQL TRUE and NOT TRUE. Preserve explicit residuals and never move missing semantics to sql-tdg. Protocol 3.0.0 release remains gated by TASK-91.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Implement exact LIKE/NOT LIKE, supported ESCAPE variants and provably safe nonprefix patterns where typed binary/no-padding and wildcard/escape laws suffice; preserve SQL UNKNOWN for NULL operands. Audit ILIKE and regex-family spellings and flags for each parser-capable dialect.
- [ ] #2 Represent collation, case folding, CHAR padding, Unicode, comparison ordering and null-safe equality assumptions as explicit canonical evidence/preconditions, not inferred vendor defaults; handle MySQL `<=>` and relevant AST variants only under proven laws.
- [ ] #3 Prove TRUE/NOT TRUE, impossible, residual, precise value-domain constraints and output comparison settings for safe text patterns, cross-text-column equality where attested and interactions with mixed Boolean predicates.
- [ ] #4 Enumerate and explicitly classify every inventoried dialect-sensitive text/operator variant as exact under certified conditions or fail-closed residual with a reason. Never claim regex engine or vendor collation parity based solely on a successful parser.
- [ ] #5 Add shared cross-dialect canonical equivalence, syntax-boundary cases, dbt evidence parity, DuckDB pattern/collation comparison oracles and negative/NULL/ESCAPE counterexamples. Update `docs/coverage-manifest.json`, contract, schema and tests.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** Local comparison/operator laws only. Full 13-dialect matrix remains TASK-88; external unavailable vendor-engine execution remains unverified rather than guessed.
- **Next:** After completion, pick **TASK-105**.
