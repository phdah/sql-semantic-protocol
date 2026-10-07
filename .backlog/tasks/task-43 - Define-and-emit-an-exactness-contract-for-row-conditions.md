---
id: TASK-43
title: Define and emit an exactness contract for row conditions
status: Done
assignee: []
created_date: '2026-10-07 18:11'
updated_date: '2026-10-07'
labels: []
milestone: m-2
dependencies: []
references:
  - TASK-31
  - TASK-36
  - TASK-38
  - sql-tdg TASK-21.5
  - docs/protocol.md
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Why
sql-tdg (and any test-data generator) samples source rows from `composed_semantics.column_domains` plus join equalities and needs a guarantee: every generated row satisfies the query, and every row deliberately violating one column domain fails it. The protocol today has no such guarantee. Per-column domains are conservative projections, which is correct for describing a query but unsafe for generating from it. Consumers cannot tell "this domain is the whole condition" from "this domain is a loose projection of a condition we could not represent". Every review round so far found another construct where the difference is silent. Fixing constructs one at a time will not converge; the contract itself must make the difference explicit.

Observed at 2e6998e (single-relation queries, composed status `resolved`, zero composed diagnostics in every case):
- `WHERE (a = 1 AND b = 2) OR (a = 3 AND b = 4)` emits independent sets a in {1,3}, b in {2,4}, which admits (1,4).
- `WHERE a = 1 OR b = 2` emits both columns unbounded.
- `WHERE name LIKE 'x%'` and `WHERE CAST(a AS INT) > 5` emit no domain for the column; the only signal is a statement-level `unsupported_expression`.
- `HAVING count(*) > 2`, `QUALIFY row_number() OVER (ORDER BY a) = 1`, and `TABLESAMPLE` produce no diagnostic anywhere; LIMIT/OFFSET/FETCH only a statement-level diagnostic.
- `JOIN u ON t.x = u.y AND t.a > 5` emits no domain for `t.a`.

## Outcome
Every query scope and every resolved composed semantics states whether its row conditions are exactly represented, and if not, lists the residual conditions that are not. The representation and names are the implementer's decision; the guarantee below is fixed.

**Exactness guarantee.** When a scope is marked exact, a combination of physical source rows (one row per physical source instance) contributes to the scope result before row-set shaping if and only if every column value lies in its column domain and every reported join equality holds. "Row-set shaping" means operators that do not decide whether an individual combination qualifies, such as ORDER BY, projection, GROUP BY without HAVING, and DISTINCT. Operators that drop qualifying combinations based on other rows (HAVING, QUALIFY, LIMIT/OFFSET/FETCH, DISTINCT ON, TABLESAMPLE, EXCEPT/INTERSECT) are residual.

**Default deny.** Exactness is granted only to an explicit allow-list of condition shapes. Anything not on the list, including every construct added in the future, is residual with a reason. A conservative per-column domain may still be emitted next to a residual condition, but the scope is then never marked exact.

This task covers single-statement scopes (top-level WHERE, inner-join ON, HAVING, QUALIFY, set operations, row-set operators, nested subqueries). Carrying the contract through local relations and multi-layer composition, and exposing join equalities, are separate dependent tasks.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Protocol docs define the exactness guarantee, the row-set shaping vs residual operator classification, and the allow-list of exactly represented condition shapes
- [x] #2 Every query scope exposes an exactness status and a deterministic list of residual conditions, each with a stable reason code, its clause (WHERE, ON, HAVING, QUALIFY, set operation, row-set operator), and enough identity to locate it
- [x] #3 Cross-column disjunctions, mixed AND/OR trees that are not reducible to independent per-column domains, NOT over non-invertible operands, column-vs-column comparisons outside equi-joins, computed-expression comparisons, LIKE/ILIKE/SIMILAR/regex, CAST and other functions in conditions, and subquery predicates are residual
- [x] #4 HAVING, QUALIFY, LIMIT, OFFSET, FETCH, DISTINCT ON, TABLESAMPLE, EXCEPT, INTERSECT, and UNION branches with differing constraints are residual; ORDER BY, plain projection, GROUP BY without HAVING, and DISTINCT are not
- [x] #5 Non-equality conditions in an inner-join ON clause are carried as column domains when they reduce safely, otherwise residual; outer-join ON clauses are residual
- [x] #6 A query that reads the same physical relation through more than one instance (self-join, repeated derived table) is residual unless domains and equalities identify the instance they apply to
- [x] #7 Every statement-level and nested-subquery diagnostic either maps to a residual condition or is on a documented list of diagnostics that cannot affect row membership
- [x] #8 Each reproduction listed in the description has a test asserting it is residual, and each allow-listed shape has a test asserting exactness with its domain
- [x] #9 JSON Schema, protocol docs, README, and public API docs describe the exactness status and residual conditions
- [x] #10 Protocol docs define NULL membership for every domain kind (unbounded, ranges, include and exclude sets, empty, unknown), and tests assert it for IS NULL, IS NOT NULL, <>, NOT IN, IS DISTINCT FROM, and ranges
<!-- AC:END -->
