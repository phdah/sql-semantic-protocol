---
id: TASK-103
title: Prove correlated cross-column and mixed Boolean constraints
status: To Do
assignee: []
created_date: '2026-10-13'
updated_date: '2026-10-13'
labels: []
milestone: m-3
dependencies:
  - TASK-102
references:
  - 'TASK-70'
  - 'TASK-68'
  - 'TASK-86'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extend bounded existence-only cross-column assignments to sound typed joint constraint and impossibility reasoning. Never substitute one satisfying row per atom for a single jointly satisfying physical row.

**TASK-70 child, step 3/8.** Implement this task in a fresh PR targeting `main` once the previous dependency has merged. The TASK-70 parent owns the [dependency-ordered queue](task-70%20-%20solve-expression-and-predicate-variants.md); this child MUST NOT depend on the parent (no dependency cycle). A green, bounded increment is mergeable but does not complete this child unless every criterion below is independently verified.

**Release contract:** Produce canonical, source-independent, parser-agnostic typed outcomes with strongest provable value domains and deterministic proof strength (exact, impossible, residual) for SQL TRUE and NOT TRUE. Preserve explicit residuals and never move missing semantics to sql-tdg. Protocol 3.0.0 release remains gated by TASK-91.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Normalize mixed AND/OR/NOT, BETWEEN, IN/NOT IN (including NULL members), NULL-safe comparisons and typed computed expressions into one canonical three-valued joint constraint problem.
- [ ] #2 Reason soundly over relations between two or more typed columns, aliases and bounded computed expressions; distinguish a shared satisfying assignment from individually feasible but mutually contradictory atoms.
- [ ] #3 Prove impossible and tautological cases only from exhaustive, justified typed domains or symbolic invariants; apply deterministic resource limits and preserve typed residual for underconstrained, nonfinite or unresolved cases.
- [ ] #4 Provide deterministic TRUE and NOT TRUE witnesses, including FALSE versus UNKNOWN causes, and propagate strongest safely justified per-column and output domains; verify NULL, duplicates and comparison settings rather than assuming binary/coercion laws.
- [ ] #5 Add property/differential DuckDB checks and hand-constructed counterexamples for multi-column conjunctions/disjunctions, satisfiable/impossible/unknown, computed aliases and dbt parity; validate schema/API/manifest changes.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** Canonical per-row joint expression proof only. TASK-106 composes it across relational source populations; existing TASK-68 owns generic shared-source identity and physical graph machinery.
- **Next:** After completion, pick **TASK-104**.
