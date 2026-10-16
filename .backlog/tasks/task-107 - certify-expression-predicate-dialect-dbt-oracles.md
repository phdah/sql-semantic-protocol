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


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#5 remain required**. Interpret and verify them through the frozen TASK-107 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Coverage: each exact expression family delivered by TASK-101..106 mapped to direct SQL, compiled dbt manifest+catalog and typed expected positive/NOT TRUE/domain behavior.
2. **Mandatory cross-operator / adapter evidence:** Dialect: all 13 exposed names covered by parser/equivalence classifications, with dialect assumptions for meaning-changing AST; DuckDB seeded boundary/NULL/duplicate/overflow and residual oracles.
3. **Conditional and non-exact boundary:** Adapters: ODCS/schema fact parity where equivalent metadata is available; no missing expected family, silent residual, unowned deferral or non-reproducible failing seed.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#5 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** Expression-specific conformance, not a duplicate of TASK-88's global dialect feature matrix or TASK-89's all-operator DML/DDL/dbt end-to-end acceptance.
- **Next:** After completion, pick **TASK-108**.
