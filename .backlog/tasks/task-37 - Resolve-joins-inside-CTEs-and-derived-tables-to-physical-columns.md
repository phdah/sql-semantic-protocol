---
id: TASK-37
title: Resolve joins inside CTEs and derived tables to physical columns
status: To Do
assignee: []
created_date: '2026-10-07 09:27'
labels: []
milestone: m-2
dependencies: []
references:
  - TASK-31
  - sql-tdg TASK-21.5
priority: high
type: bug
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Joins inside CTEs and derived tables are now reported by `QueryStatement::joins()`, but their sides are `RelationRef`s that may name the local relation and their conditions use the inner aliases. Consumers cannot map join columns to physical source columns without re-deriving query semantics themselves, which consumers such as sql-tdg must not do.

Observed at d2b4de9:
- A dbt-style CTE chain (daily_revenue: CTEs over sources, `orders` joined to `order_items`) reports the join as `orders AS o JOIN order_items AS oi ON o.order_id = oi.order_id`, where `orders` is the CTE, not `target.main.orders`. CTE output columns are not exposed anywhere, so the qualifier cannot be resolved.
- A CTE that joins physical tables directly still uses inner aliases that do not correspond to any `sources()` entry; sql-tdg fails with `join column qualifier "o" does not resolve to a protocol source`.
- Joins and dependencies of a CTE that the query never references leak into the outer query.

This is the remaining gap in TASK-31 AC #1 (marked Done with unchecked criteria) and blocks sql-tdg TASK-21.5 (coordinating join keys inside CTE chains).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every join reported for a query exposes its equality columns resolved to physical source relations and columns when lineage is a plain column copy
- [ ] #2 Joins whose columns cannot be resolved to physical columns produce an explicit diagnostic or unresolved composition
- [ ] #3 Joins, predicates, and dependencies of CTEs that the query does not reference do not appear in the query semantics
- [ ] #4 Integration tests cover joins inside single CTEs, chained CTEs, derived tables, and a dbt-style CTE chain model joining two sources
- [ ] #5 JSON Schema, protocol documentation, and public API docs describe how local-relation joins map to physical columns
<!-- AC:END -->
