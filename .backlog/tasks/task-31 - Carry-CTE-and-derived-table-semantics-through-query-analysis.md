---
id: TASK-31
title: Carry CTE and derived-table semantics through query analysis
status: Done
assignee: []
created_date: '2026-10-06 13:25'
updated_date: '2026-10-07 18:18'
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
- [x] #1 Joins inside CTEs and derived tables are represented and resolve to physical source columns through lineage
- [x] #2 WHERE, HAVING, and QUALIFY predicates inside CTEs and derived tables contribute column domains on physical source columns
- [x] #3 Output column lineage resolves through CTE chains, nested CTEs, and derived tables to physical source columns
- [x] #4 Wildcard projections over CTEs and physical relations resolve when catalog schema metadata is available, including the dbt adapter path
- [x] #5 Composed semantics for a query reading local relations equal those of the equivalent query with the local relations inlined, for supported constructs
- [x] #6 Local-relation constructs that cannot be carried produce an explicit diagnostic or unresolved composition rather than resolved semantics
- [x] #7 Integration tests cover single and chained CTEs, derived tables, joins and filters inside CTEs, wildcard resolution, and a dbt-style CTE model end to end
- [x] #8 JSON Schema, protocol documentation, and public API docs reflect the new representation
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Acceptance criteria verified on 2026-10-07 against 2e6998e. #1-#4, #7, #8 are met by #40 and follow-ups TASK-36 to TASK-39 (tests/local_relations.rs, docs/protocol.md). #5 and #6 are met for the shapes covered here, but a later review found remaining gaps: Unknown domains lost in intersections, outer predicates on computed local columns without diagnostics, derived-table output domain narrowing, and unreferenced-CTE diagnostic leaks. Those are re-specified under the exactness contract in TASK-44 rather than reopening this task.
<!-- SECTION:NOTES:END -->
