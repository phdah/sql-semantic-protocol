---
id: TASK-107
title: Certify expression and predicate SQL/dbt/dialect oracles
status: In Progress
assignee: []
created_date: '2026-10-13'
updated_date: '2026-10-16'
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

## Frozen TASK-107 acceptance contract (2026-10-16)

**Normative machine-readable freeze:** [TASK-107 acceptance matrix](../../docs/task-107-acceptance-freeze.json)
and [evidence owners](../../docs/task-107-oracle-matrix.json), tied to the
TASK-66 baseline commit `b75b16a29234fc207af0014bf662d39223b2502f`.
The original AC #1..#5 above remain unchanged and additive.

The exact **completion boundary** is:
- **AC #1:** 42 explicit TASK-66 predicate/expression variants, 14 separately
  owned TASK-70 family obligations and 9 closed-world golden fixtures;
  independently assert typed domains/bounds/inclusivity/NULL and both SQL
  TRUE/NOT TRUE proof directions. Mandatory exactness is never met by residual.
- **AC #2:** all 42 × 13 = **546** named dialect/parser-canonical cells
  classified with executable fixture evidence, semantic equivalence where
  applicable, or documented non-equivalent/session assumptions. No vendor
  runtime inference from parsing.
- **AC #3:** 9 frozen composition pairings plus 4 deterministic seeds
  (`0, 1, 42, 20261016`), including known NULL/duplicate/overflow/boundary
  input cases. All executable physical positive and negative source/terminal
  cases must match independent DuckDB outputs; residual/unknown paths must
  fail closed.
- **AC #4:** direct SQL + compiled dbt manifest/catalog parity and ODCS when
  equivalent metadata can supply the fact; protocol output + emitted JSON,
  schema/docs and explicit adapter exceptions recorded per applicable cell.
- **AC #5:** **zero** unverified required cells, unreviewed conditional
  dispositions, silent residuals or false exactness; final full CI on the
  implementation SHA: fmt, strict clippy, regular and no-default tests, docs,
  dbt E2E and active schema snapshots.

Each case has a frozen stable ID. The machine-readable contract is the
enumerated minimum, not permission to skip originally named features: changes
to an original obligation require a separate explicit contract review. New
unrelated variants discovered after the freeze belong in new tasks. A change
that exposes a false existing proof must be fixed before signoff, never
deferred. `tests/task_107_acceptance_freeze.rs` rejects inventory drift
and premature closure; semantic oracle tests must separately pass. **Current
cell classifications and golden proof checks remain unverified**; all five
original acceptance checkboxes remain unchecked. TASK-108 and TASK-91 have
independent signoffs and remain blocked.

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** Expression-specific conformance, not a duplicate of TASK-88's global dialect feature matrix or TASK-89's all-operator DML/DDL/dbt end-to-end acceptance.
- **Next:** After completion, pick **TASK-108**.

## Implementation increment (2026-10-16)

- Registered versioned representative expression family evidence in
  `docs/task-107-oracle-matrix.json`, linked from the canonical coverage manifest.
  Each referenced existing test is checked by name, with explicit domains,
  TRUE / NOT TRUE classifications, and named owners for conditional boundaries.
- Added `tests/task_107_expression_oracles.rs` with direct SQL and dbt
  manifest/catalog equality for typed output domains and witnesses, canonical
  parser-equivalence assertions for all 13 advertised dialect names, bounded
  interval inclusivity, and deterministic multi-seed DuckDB positive, FALSE,
  UNKNOWN, contradiction, tautology, duplicate and NULL source-oracles.
- **This is not TASK-107 sign-off.** The new matrix explicitly lists the
  missing full TASK-66 inventory closure, every exact family/variant, dialect
  assumptions, physical complete-source differential oracle expansion,
  ODCS adapter parity, and full verification gates.
- Original AC #1..#5 remain unchecked. Do not mark Done or release 3.0.0
  until the original task and frozen acceptance contract are independently
  satisfied and CI passes.

## Final implement-skill verification pass (2026-10-16)

This PR now has executable, independent acceptance evidence rather than only
registry presence checks:

- **AC #1:** `tests/task_107_full_variant_adapters.rs` analyzes all **28**
  frozen `exact_required` variants through independently constructed direct
  SQL and dbt manifest/catalog adapters, comparing typed output/input domains,
  proof strength, both Boolean directions and emitted canonical JSON.
  Original TASK-101..106 expression-family tests remain the named exactness
  owners; adapter agreement alone cannot certify exactness.
- **AC #2:** `tests/task_107_dialect_classifications.rs` executes each of the
  **546** parser/canonical cells and detects inventory drift. The pinned matrix
  records **540** parsed/canonically equivalent outcomes and **6** explicit
  parser rejections, none of which claim vendor runtime execution.
- **AC #3:** `tests/task_107_frozen_execution_oracles.rs` executes all
  **nine** golden full-source/TRUE/NOT TRUE/UNKNOWN assertions and independent
  positive/rejected row-ID membership for all **28** mandatory variants;
  nine operator composition regression tests and the four frozen random seeds
  cover cross-feature cases, duplicates, NULL, INT32 boundaries, overflow and
  safe BIGINT widening.
- **AC #4:** `tests/task_107_expression_oracles.rs` checks canonical
  direct-SQL/dbt/ODCS parity on representable typed source metadata.
  `tests/task_107_full_variant_adapters.rs` also compares serialized
  SQL/dbt semantic JSON, not solely Rust internal values.
- **AC #5:** Task remains **In Progress** pending a final all-green CI result
  on the last SHA **and maintainer review** of the **14 conditional**
  vendor/session/unknown-semantic release dispositions. Conditional parser
  acceptance is not warehouse engine authority.

The five original AC boxes remain unchecked until independent signoff.
This does not waive or change the frozen minimum, mark TASK-108 done or
approve protocol 3.0.0.
