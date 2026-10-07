---
id: TASK-38
title: Surface predicates inside local relations that cannot be carried
status: Done
assignee: []
created_date: '2026-10-07 09:27'
updated_date: '2026-10-07 13:17'
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
The protocol must never report resolved semantics while dropping a condition. Inside local relations some predicates currently disappear with no predicate, domain, or diagnostic, so consumers cannot detect them and generate rows that violate the query.

Observed at d2b4de9:
- `EXISTS (...)` inside a CTE produces nothing; it is not exposed as an inner predicate, so consumer guards against unsupported relational predicates cannot see it.
- `HAVING SUM(a) > 5` inside a CTE is dropped the same way.
- An `OR` inside a CTE becomes unbounded domains with no diagnostic.
- `SELECT ... FROM (SELECT a FROM t) d WHERE a > 5` leaves the domain on `subquery.a` rather than `t.a`; composition fails with `missing_lineage_edge`, which is explicit but means a supported shape is not carried.

Completes TASK-31 AC #2 and #6.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 WHERE, HAVING, and QUALIFY predicates inside CTEs and derived tables are either carried as domains on physical columns or reported through an explicit diagnostic or unresolved composition
- [x] #2 EXISTS, IN-subquery, OR, and aggregate predicates inside local relations are visible to consumers in the same way as at the top level of a query
- [x] #3 Outer filters on derived-table columns that are plain copies map to physical source columns
- [x] #4 Tests assert a diagnostic for each unsupported predicate kind inside a CTE and inside a derived table
- [x] #5 Protocol documentation lists which local-relation predicates are carried and which are diagnosed
<!-- AC:END -->
