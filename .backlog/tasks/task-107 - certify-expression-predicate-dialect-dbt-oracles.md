---
id: TASK-107
title: Certify expression and predicate SQL/dbt/dialect oracles
status: To Do
assignee: []
created_date: '2026-10-13'
updated_date: '2026-10-13'
labels: []
milestone: m-3
dependencies:
  - TASK-106
references:
  - 'TASK-70'
  - 'TASK-88'
  - 'TASK-89'
  - 'TASK-90'
  - 'TASK-87'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Close expression-specific acceptance evidence for TASK-101..106. Treat parsing, canonical equivalence, physical proof and executed output as distinct evidence classes; this suite is the canonical reference for consumers.

**TASK-70 child, step 7/8.** Implement this task in a fresh PR targeting `main` once the previous dependency has merged. The TASK-70 parent owns the [dependency-ordered queue](task-70%20-%20solve-expression-and-predicate-variants.md); this child MUST NOT depend on the parent (no dependency cycle). A green, bounded increment is mergeable but does not complete this child unless every criterion below is independently verified.

**Release contract:** Produce canonical, source-independent, parser-agnostic typed outcomes with strongest provable value domains and deterministic proof strength (exact, impossible, residual) for SQL TRUE and NOT TRUE. Preserve explicit residuals and never move missing semantics to sql-tdg. Protocol 3.0.0 release remains gated by TASK-91.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Build parameterized direct SQL and dbt compiled-manifest/catalog fixtures for every expression family **claimed exact** in TASK-101..106, plus representative unsupported variants; require full typed outcome-domain bounds, inclusivity, positive/NOT TRUE and exact/impossible/residual assertions.
- [ ] #2 For all 13 exposed dialects classify whether shared/special syntax parses; compare identical parser-independent semantic outcomes for meaning-preserving AST variants, and record dialect/session assumptions for non-equivalent forms without inventing vendor engine execution.
- [ ] #3 Add reproducible DuckDB differential and randomized bounded-domain/property oracles for combinations, output domains, feasible/impossible/NULL/duplicate/overflow/unknown/residual and physical-source/negative cases; preserve seeds and minimal failure SQL.
- [ ] #4 Certify source-independent parity for representable SQL, dbt and ODCS/schema metadata and canonical protocol JSON. Update API/schema/docs/examples and machine-readable coverage manifest for each advertised exact and explicitly deferred cell.
- [ ] #5 Fail the suite on false exactness, undocumented gaps, untested supported expression variants or silent adapter regressions. Run complete fmt/clippy/test/doc, no-default-features and dbt E2E checks; retain ownership of global multi-feature and consumer gates in TASK-88/89/91.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** Expression-specific conformance, not a duplicate of TASK-88's global dialect feature matrix or TASK-89's all-operator DML/DDL/dbt end-to-end acceptance.
- **Next:** After completion, pick **TASK-108**.
