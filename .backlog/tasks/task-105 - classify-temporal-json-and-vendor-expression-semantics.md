---
id: TASK-105
title: Classify temporal, JSON and vendor expression outcomes
status: To Do
assignee: []
created_date: '2026-10-13'
updated_date: '2026-10-13'
labels: []
milestone: m-3
dependencies:
  - TASK-104
references:
  - 'TASK-70'
  - 'TASK-88'
  - 'TASK-92'
  - 'TASK-79'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Audit date/time, JSON/nested values and vendor functions against the existing schema-backed type evidence. Implement only independently certified deterministic semantics and retain auditable residuals for unsafe session-dependent behavior.

**TASK-70 child, step 5/8.** Implement this task in a fresh PR targeting `main` once the previous dependency has merged. The TASK-70 parent owns the [dependency-ordered queue](task-70%20-%20solve-expression-and-predicate-variants.md); this child MUST NOT depend on the parent (no dependency cycle). A green, bounded increment is mergeable but does not complete this child unless every criterion below is independently verified.

**Release contract:** Produce canonical, source-independent, parser-agnostic typed outcomes with strongest provable value domains and deterministic proof strength (exact, impossible, residual) for SQL TRUE and NOT TRUE. Preserve explicit residuals and never move missing semantics to sql-tdg. Protocol 3.0.0 release remains gated by TASK-91.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Inventory DATE/TIME/TIMESTAMP and interval expressions, casts/comparisons, calendar boundaries, time zones, DST and engine/session-dependent coercion laws by parser AST and dialect; certify exact typed result domains only where necessary execution laws and metadata are explicit.
- [ ] #2 Classify JSON path extraction, array/struct/nested comparisons, JSON SQL NULL versus JSON null and vendor function/operator families. Implement deterministic, provable typed subsets and retain explicit residuals for missing type/path/engine evidence.
- [ ] #3 Preserve extensibility for UDFs and opaque/vendor functions through typed residual or externally validated evidence; never silently evaluate a UDF or mark a stochastic/session-sensitive result exact.
- [ ] #4 For certified variants provide reversible constraints where possible, typed output domains, canonical TRUE/NOT TRUE/impossible witnesses and comparison assumptions; ensure all other inventoried variants remain fail-closed with traceable reasons.
- [ ] #5 Add direct SQL/dbt adapter parity and parser boundary tests for all relevant dialects, focused DuckDB differential oracles for executable semantics, residual regression tests, and machine-readable per-family coverage plus docs/schema changes.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** This task certifies **the safe subset and explicit residual boundary**, not universal timezone/vendor/JSON or engine execution. TASK-88 owns the global dialect matrix; TASK-92 owns later optional opaque-evidence research.
- **Next:** After completion, pick **TASK-106**.
