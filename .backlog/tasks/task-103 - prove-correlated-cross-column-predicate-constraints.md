---
id: TASK-103
title: Prove correlated cross-column and mixed Boolean constraints
status: In Progress
assignee: []
created_date: '2026-10-13'
updated_date: '2026-10-15 16:53'
labels: []
milestone: m-3
dependencies:
  - TASK-102
references:
  - TASK-70
  - TASK-68
  - TASK-86
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Research boolean_witness joint enumeration limits (done: existence-only, has_complete_scalar_regions false for distinct cross-column, conjoin/jointly_satisfiable guard on contains_column_comparison). 2. Implement symbolic joint unsatisfiability prover in src/boolean_witness.rs: flatten All into null + equality/union-find + difference-constraints graph with CONST node for literals (strict < as <=-1), self-contradiction, disjoint accepted sets, deterministic caps (<=32 cols, <=128 atoms). Prove qualifying Impossible when enumeration empty but symbolic unsat; prove rejected Impossible via not_true_constraint unsat for tautologies. 3. Wire prover into recheck_truth_directions and into conjoin_physical_row_truths/jointly_satisfiable_physical_truths (return Some(false) on symbolic proof instead of None). 4. TDD: new tests/task_103_cross_column.rs covering mixed AND/OR/NOT, BETWEEN/IN NULL members, null-safe, computed affine/CASE aliases, impossible cycles (a>b AND b>a, 3-cycle, literal-bound cycles, equality+disequality), tautologies (null-safe complements), satisfiable controls, FALSE vs UNKNOWN witnesses, DuckDB differential oracles, dbt parity, all-dialect parser shape. Update mutually_exclusive test to Impossible. 5. Update docs/semantics.md, docs/coverage-manifest.json (signed_integer_cross_column to bounded_joint_exact), docs/code-map.md if ownership shifts, schema only if needed (no wire change expected). 6. Verify: cargo fmt, clippy -D warnings, single-file tests, full cargo test, cargo doc if docs change.
<!-- SECTION:PLAN:END -->

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** Canonical per-row joint expression proof only. TASK-106 composes it across relational source populations; existing TASK-68 owns generic shared-source identity and physical graph machinery.
- **Next:** After completion, pick **TASK-104**.
