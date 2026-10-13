---
id: TASK-102
title: Prove CASE, COALESCE, NULLIF and computed alias outcomes
status: To Do
assignee: []
created_date: '2026-10-13'
updated_date: '2026-10-13'
labels: []
milestone: m-3
dependencies:
  - TASK-101
references:
  - 'TASK-70'
  - 'TASK-96'
  - 'TASK-87'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Promote computed expressions from the currently proven NULL-test subset to typed, reference-based result domains and reversible constraints. Reuse TASK-101 arithmetic and conversion semantics instead of introducing a second evaluator.

**TASK-70 child, step 2/8.** Implement this task in a fresh PR targeting `main` once the previous dependency has merged. The TASK-70 parent owns the [dependency-ordered queue](task-70%20-%20solve-expression-and-predicate-variants.md); this child MUST NOT depend on the parent (no dependency cycle). A green, bounded increment is mergeable but does not complete this child unless every criterion below is independently verified.

**Release contract:** Produce canonical, source-independent, parser-agnostic typed outcomes with strongest provable value domains and deterministic proof strength (exact, impossible, residual) for SQL TRUE and NOT TRUE. Preserve explicit residuals and never move missing semantics to sql-tdg. Protocol 3.0.0 release remains gated by TASK-91.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Normalize searched/simple CASE, COALESCE and NULLIF with typed branches, lazy/short-circuit guards and SQL TRUE/FALSE/UNKNOWN fallthrough; preserve output NULLability and nonliteral computed arm domains.
- [ ] #2 Represent derived SELECT columns and aliases using stable canonical references across nested projections and source columns, without leaking sqlparser AST types into the emitted domain model.
- [ ] #3 Propagate precise typed output domains, inclusivity and excluded values through conditional branches; derive reversible source constraints for certified guards/arms and explicitly residualize lossy, ambiguous or noninvertible mappings.
- [ ] #4 Produce positive, NOT TRUE, impossible and residual predicate witnesses for comparisons and IS NULL/IS NOT NULL over supported computed values and aliases, including nested AND/OR/NOT and NULL branches.
- [ ] #5 Test direct SQL, dbt compiled SQL/catalog parity, alias shadowing and duplicated names, numeric boundaries, contradictory branches, NULLs and DuckDB computed-value oracles across parser-capable dialects. Update APIs/schema/docs/coverage where semantics change.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** Expression evaluation and alias/reference semantics, not general multi-relation proof. TASK-103 owns joint correlated predicates; TASK-106 consumes the canonical computed expression facts through JOIN/GROUP/write DAGs.
- **Next:** After completion, pick **TASK-103**.
