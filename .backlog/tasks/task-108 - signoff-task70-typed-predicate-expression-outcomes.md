---
id: TASK-108
title: Sign off TASK-70 typed predicate and expression outcomes
status: To Do
assignee: []
created_date: '2026-10-13'
updated_date: '2026-10-13'
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
- [ ] #1 Verify TASK-101..107 are individually Done, merged on `main`, have complete child AC evidence and no unowned unverified expression/predicate variant; map each original TASK-70 criterion #1..#6 to concrete source modules, tests, adapter paths, docs and coverage rows.
- [ ] #2 Audit arithmetic and overflow, typed casts, CASE/COALESCE/NULLIF, alias reversibility, cross-column SQL three-valued predicates, text/pattern/dialect/session law classification and verified fail-closed UDF/vendor/residual boundaries against the original TASK-70 description.
- [ ] #3 Verify exact, impossible and residual positive/NOT TRUE outcomes, canonical output value domains, typed physical source witnesses, comparison assumptions and safe literal coercion through the supported relational DAGs without consumer reparsing.
- [ ] #4 Verify all 13-dialect expression parser/equivalence cells and relevant direct SQL/dbt/ODCS adapter paths, plus reproducible DuckDB NULL/duplicate/boundary/negative/differential oracles, with no false exact claim or unsupported vendor engine certification.
- [ ] #5 Re-run complete CI (fmt, strict Clippy, tests, docs, no-default-features, dbt E2E and active protocol JSON schema snapshots), update code map, docs/coverage-manifest and TASK-66/88/89/91 handoffs. Only then check all original TASK-70 AC and mark parent Done; TASK-91/3.0.0 release remains blocked until its separate signoff.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** Parent TASK-70 closes only with original AC mapped to verifiable evidence. Do not mark all 3.0.0 dialect/feature inventory or downstream sql-tdg generation approved by completing this child.
- **Next:** After completion, verify the TASK-70 parent and mark its original AC Done only when fully evidenced.
