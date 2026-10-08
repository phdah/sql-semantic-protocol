---
id: TASK-62
title: Model exact EXISTS and subquery membership conditions
status: To Do
assignee: []
created_date: '2026-10-08'
labels: []
milestone: m-3
dependencies: []
references:
  - 'TASK-19'
  - 'TASK-43'
  - 'TASK-44'
  - 'sql-tdg TASK-26'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Subquery AST, correlations and dependencies are described (TASK-19), but EXISTS, NOT EXISTS, correlated and subquery-based IN conditions are currently residual. Generate a canonical source-level membership/nonmembership contract.

SQL parsing, normalized semantics, lineage, and exactness remain owned by SQL Semantic Protocol. Preserve existing exact behavior while extending the canonical contract, and never mark unsupported cases exact. Changes must uphold the repository's outcome-first definition of done and cross-adapter parity.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Represent provable EXISTS/NOT EXISTS and IN/NOT IN subquery conditions with correlation keys, inner predicates and relation-instance identities.
- [ ] #2 Preserve SQL three-valued NULL behavior, empty-set behavior and duplicate-insensitive membership; never translate NOT IN to anti-join when NULLs make it unsafe.
- [ ] #3 Carry constraints through nested subqueries, CTEs, models and layers with provenance and exactness diagnostics.
- [ ] #4 Test positive and rejected witnesses, correlated and uncorrelated cases, nullable keys, and impossible configurations against DuckDB.
- [ ] #5 Update protocol contract/docs, assess adapter parity; sql-tdg TASK-26 consumes the representation.
<!-- AC:END -->
