---
id: TASK-58
title: Define exact source membership for SQL set operations
status: Done
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
- [x] #2 Emit exactness only where membership can be proven for full branch combinations; conflicting, missing or ambiguous evidence remains an explicit residual with origin.
- [x] #3 Compose the contract through CTEs, producer layers and dbt compiled model graphs; preserve strong output domains.
- [x] #4 Add paired exact and residual tests, including DuckDB differential tests for overlapping and disjoint branches, duplicates and NULL, plus applicable dialect variants.
- [x] #5 Update schema, protocol docs and consumer compatibility/versioning guidance; sql-tdg TASK-24 consumes this contract.
- [x] #6 Expose typed qualifying and, where provable, non-qualifying branch witness obligations at physical-source or intermediate boundaries, including branch identity and duplicate counts; do not make consumers infer set-operation semantics to generate rejected rows. Report unsupported witness directions as residual with a reason.
<!-- AC:END -->

## Implementation status

Implemented in PR #78: typed leaf identities, positional source and intermediate boundaries, NULL-safe tuple matching, duplicate-count rules for all six DISTINCT/ALL forms, and independent qualifying and non-qualifying exact-count obligation cases. Exact witness status requires independently controllable physical dependencies, row-preserving plain-column branches, proven branch filters, supported alignment, and no row-set limits. Disjoint simultaneous positive domains are excluded; nested trees through four leaves are evaluated recursively, including count-two duplicate and cancellation cases. Ambiguous projections, shared sources, unsupported operators, limits, missing boundary evidence, and infeasible witness directions explicitly remain residual with a stable reason and origin. Composition retains operations and origin layers through producer/CTE/dbt graphs, and intermediate obligations are clearly distinguished from physical insertions. Public Rust types, active JSON schema, protocol/semantic documentation, compatibility guidance, Rust/DuckDB differential tests, dbt E2E golden fixtures, and dialect checks are updated. Query-level independent-domain exactness remains conservatively residual for set operations even when the separate membership witness is exact.
