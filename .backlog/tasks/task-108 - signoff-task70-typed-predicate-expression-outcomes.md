---
id: TASK-108
title: Sign off TASK-70 typed predicate and expression outcomes
status: Done
assignee: []
created_date: '2026-10-13'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-107
references:
  - 'TASK-70'
  - 'TASK-66'
  - 'TASK-88'
  - 'TASK-89'
  - 'TASK-90'
  - 'TASK-91'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Independent parent-criteria signoff after all TASK-101..107 implementation PRs are merged. Do not treat green CI or a narrow exact subset as proof of the parent requirements.

**TASK-70 child, step 8/8.** Implement this task in a fresh PR targeting `main` once the previous dependency has merged. The TASK-70 parent owns the [dependency-ordered queue](task-70%20-%20solve-expression-and-predicate-variants.md); this child MUST NOT depend on the parent (no dependency cycle). A green, bounded increment is mergeable but does not complete this child unless every criterion below is independently verified.

**Release contract:** Produce canonical, source-independent, parser-agnostic typed outcomes with strongest provable value domains and deterministic proof strength (exact, impossible, residual) for SQL TRUE and NOT TRUE. Preserve explicit residuals and never move missing semantics to sql-tdg. Protocol 3.0.0 release remains gated by TASK-91.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Verify TASK-101..107 are individually Done, merged on `main`, have complete child AC evidence and no unowned unverified expression/predicate variant; map each original TASK-70 criterion #1..#6 to concrete source modules, tests, adapter paths, docs and coverage rows.
- [x] #2 Audit arithmetic and overflow, typed casts, CASE/COALESCE/NULLIF, alias reversibility, cross-column SQL three-valued predicates, text/pattern/dialect/session law classification and verified fail-closed UDF/vendor/residual boundaries against the original TASK-70 description.
- [x] #3 Verify exact, impossible and residual positive/NOT TRUE outcomes, canonical output value domains, typed physical source witnesses, comparison assumptions and safe literal coercion through the supported relational DAGs without consumer reparsing.
- [x] #4 Verify all 13-dialect expression parser/equivalence cells and relevant direct SQL/dbt/ODCS adapter paths, plus reproducible DuckDB NULL/duplicate/boundary/negative/differential oracles, with no false exact claim or unsupported vendor engine certification.
- [x] #5 Re-run complete CI (fmt, strict Clippy, tests, docs, no-default-features, dbt E2E and active protocol JSON schema snapshots), update code map, docs/coverage-manifest and TASK-66/88/89/91 handoffs. Only then check all original TASK-70 AC and mark parent Done; TASK-91/3.0.0 release remains blocked until its separate signoff.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#5 remain required**. Interpret and verify them through the frozen TASK-108 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Independent audit: TASK-101..107 Done/merged, original TASK-70 six criteria each mapped to tests/modules/manifest rows and all promised families accounted for.
2. **Mandatory cross-operator / adapter evidence:** Evidence: representative exact, impossible and residual expression/physical outcome under SQL/dbt/ODCS, all 13 parser paths and reproducible DuckDB negative/domain oracles.
3. **Conditional and non-exact boundary:** Gate: full CI and schema snapshots; TASK-70 can close only after all original AC proved; TASK-88/89/91 independent release blockers remain open until their own checks.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#5 only after required cells and final CI are verified. Do not approve independent release gates through this task.

**Upstream signoff prerequisite:** Independently verify TASK-107 against
`docs/task-107-acceptance-freeze.json` (all 42 variants, 546 dialect cells,
14 family obligations, 9 goldens, 9 pairings, seeded oracles and adapter gates).
A frozen matrix alone is not evidence of passed semantics or TASK-107
completion. TASK-108 still needs its own frozen closure matrix before work and
must not inherit TASK-107's proof signoff automatically.

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** Parent TASK-70 closes only with original AC mapped to verifiable evidence. Do not mark all 3.0.0 dialect/feature inventory or downstream sql-tdg generation approved by completing this child.
- **Next:** After completion, verify the TASK-70 parent and mark its original AC Done only when fully evidenced.

## Independent frozen acceptance audit (2026-10-17)

The immutable closure matrix lives in [docs/task-108-acceptance-freeze.json](../../docs/task-108-acceptance-freeze.json). Its six-AC evidence map and merge/variant/dialect/adapter/oracle audit are documented in [docs/task-108-acceptance-audit.md](../../docs/task-108-acceptance-audit.md) and verified by `tests/task_108_parent_signoff.rs`. All nine final PR-head checks must pass before TASK-108 and TASK-70 are marked Done; TASK-88/89/90/91 retain independent gates.

## Final acceptance signoff (2026-10-17)

All five TASK-108 AC are verified against the frozen parent matrix and independent public SQL/dbt/ODCS, DuckDB and 13-dialect execution tests. The seven child PRs #109..#115 are merged on main with successful CI. TASK-108 PR #116 implementation head `fac7bba6834b05f1863c33aee00a33d0431f19c9` passed all nine checks in [run 38977162083](https://github.com/phdah/sql-semantic-protocol/actions/runs/38977162083). TASK-70 original six AC are independently mapped and signed off. TASK-88/89/90/91 and protocol 3.0.0 publication retain separate requirements.
