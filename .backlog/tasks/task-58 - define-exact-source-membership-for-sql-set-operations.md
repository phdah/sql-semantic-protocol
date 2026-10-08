---
id: TASK-58
title: Define exact source membership for SQL set operations
status: In Progress
assignee: []
created_date: '2026-10-08'
labels: []
milestone: m-3
dependencies: []
references:
  - 'TASK-16'
  - 'TASK-43'
  - 'TASK-44'
  - 'sql-tdg TASK-24'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Set-operation AST, lineage, and output domains exist (TASK-16), but the row-condition exactness contract marks UNION, UNION ALL, INTERSECT and EXCEPT residual. Model branch-specific source conditions, row-set membership and duplicate/multiplicity semantics without collapsing branch alternatives into independent per-column intervals.

SQL parsing, normalized semantics, lineage, and exactness remain owned by SQL Semantic Protocol. Preserve existing exact behavior while extending the canonical contract, and never mark unsupported cases exact. Changes must uphold the repository's outcome-first definition of done and cross-adapter parity.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Define a canonical, typed branch-aware semantic contract for UNION ALL, UNION, INTERSECT and EXCEPT, including branch identity, positional alignment, duplicate semantics and NULL behavior.
- [ ] #2 Emit exactness only where membership can be proven for full branch combinations; conflicting, missing or ambiguous evidence remains an explicit residual with origin.
- [ ] #3 Compose the contract through CTEs, producer layers and dbt compiled model graphs; preserve strong output domains.
- [ ] #4 Add paired exact and residual tests, including DuckDB differential tests for overlapping and disjoint branches, duplicates and NULL, plus applicable dialect variants.
- [ ] #5 Update schema, protocol docs and consumer compatibility/versioning guidance; sql-tdg TASK-24 consumes this contract.
- [ ] #6 Expose typed qualifying and, where provable, non-qualifying branch witness obligations at physical-source or intermediate boundaries, including branch identity and duplicate counts; do not make consumers infer set-operation semantics to generate rejected rows. Report unsupported witness directions as residual with a reason.
<!-- AC:END -->

## Implementation status

Draft PR #78 introduces independent branch evidence, NULL-safe tuple multiplicity rules, and transitive operation provenance, with schema, docs, and differential tests. This is a partial implementation only: neither positive nor negative physical-source witness obligations are proven. Exact set membership, complete local-relation remapping and consumer adoption remain open; do not mark the task Done or enable generator exactness until these are tested.
