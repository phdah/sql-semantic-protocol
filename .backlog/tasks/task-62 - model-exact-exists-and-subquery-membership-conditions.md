---
id: TASK-62
title: Model exact EXISTS and subquery membership conditions
status: Done
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
- [x] #1 Represent provable EXISTS/NOT EXISTS and IN/NOT IN subquery conditions with correlation keys, inner predicates and relation-instance identities.
- [x] #2 Preserve SQL three-valued NULL behavior, empty-set behavior and duplicate-insensitive membership; never translate NOT IN to anti-join when NULLs make it unsafe.
- [x] #3 Carry constraints through nested subqueries, CTEs, models and layers with provenance and exactness diagnostics.
- [x] #4 Test positive and rejected witnesses, correlated and uncorrelated cases, nullable keys, and impossible configurations against DuckDB.
- [x] #5 Update protocol contract/docs, assess adapter parity; sql-tdg TASK-26 consumes the representation.
<!-- AC:END -->

## Implementation

- Added typed qualifying and rejected `subquery_witnesses` for EXISTS, NOT EXISTS, IN and NOT IN with physical source columns, source-instance identities, equality correlations, nested input domains, NULL-aware and empty-candidate cases. Duplicate candidates do not change membership; NOT IN is not reduced to an anti-join.
- Exact operator-local witness directions are emitted only for proven single-relation, candidate-preserving nested queries with plain keys and conjunctive supported correlations. Unsupported nested, CTE, computed, joined, row-shaping, alias-ambiguous and untyped cases remain explicit residual directions, rather than claiming false whole-query exactness.
- Composed outcomes carry source witness obligations from the originating transformation layer, retaining its layer ID and physical, intermediate or unresolved source boundary; the existing row-condition exactness contract remains authoritative for entire queries.
- Updated the active JSON schema, public API, protocol/semantics/adapter documentation, direct SQL and dbt snapshot. Added DuckDB differential tests for positive, rejected, nullable, duplicate, empty and impossible cases and tests for dialect variants, nested residuals and composition.
- Downstream consumer: sql-tdg TASK-26.
