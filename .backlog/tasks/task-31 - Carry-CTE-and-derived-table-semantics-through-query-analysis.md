---
id: TASK-31
title: Carry CTE and derived-table semantics through query analysis
status: To Do
assignee: []
created_date: '2026-10-06 13:25'
labels: []
milestone: m-2
dependencies: []
priority: high
type: bug
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Query analysis currently reads CTE bodies and derived tables only to collect their physical dependencies (see analyze_query_relations in src/analysis.rs). Their joins, WHERE/HAVING/QUALIFY predicates, column domains, and output lineage are dropped, yet the statement and its composed semantics are still reported as resolved with no blocking diagnostics. Consumers therefore over-claim: sql-tdg generated rows violating `with x as (select a from t where a > 1000) select a from x` (a = -2147483648, 0, 2147483647) and could not coordinate the join inside a dbt model written as a CTE chain (the standard dbt style). sql-tdg now refuses any query source that is a local relation until this is fixed (sql-tdg TASK-21.4; real support is sql-tdg TASK-21.5).

Local relations must be analyzed as first-class semantic scopes so that their joins, predicates, domains, and lineage compose into the enclosing query exactly like in-bundle produced relations. Wildcard projections over CTEs and physical relations must resolve when source schema metadata is available (for example from the dbt catalog); today the dbt path reports `wildcard output cannot be resolved without source schema information` for `select * from {{ source(...) }}` inside a CTE. Any construct that still cannot be carried must surface as an explicit diagnostic or unresolved composition, never as resolved semantics.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Joins inside CTEs and derived tables are represented and resolve to physical source columns through lineage
- [ ] #2 WHERE, HAVING, and QUALIFY predicates inside CTEs and derived tables contribute column domains on physical source columns
- [ ] #3 Output column lineage resolves through CTE chains, nested CTEs, and derived tables to physical source columns
- [ ] #4 Wildcard projections over CTEs and physical relations resolve when catalog schema metadata is available, including the dbt adapter path
- [ ] #5 Composed semantics for a query reading local relations equal those of the equivalent query with the local relations inlined, for supported constructs
- [ ] #6 Local-relation constructs that cannot be carried produce an explicit diagnostic or unresolved composition rather than resolved semantics
- [ ] #7 Integration tests cover single and chained CTEs, derived tables, joins and filters inside CTEs, wildcard resolution, and a dbt-style CTE model end to end
- [ ] #8 JSON Schema, protocol documentation, and public API docs reflect the new representation
<!-- AC:END -->
