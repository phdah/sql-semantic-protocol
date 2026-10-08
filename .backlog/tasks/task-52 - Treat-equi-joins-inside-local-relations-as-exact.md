---
id: TASK-52
title: Treat equi-joins inside local relations as exact
status: To Do
assignee: []
created_date: '2026-10-08 09:06'
updated_date: '2026-10-08 09:06'
labels: []
milestone: m-2
dependencies:
  - TASK-51
references:
  - TASK-44
  - TASK-45
  - sql-tdg TASK-21.5
priority: high
type: bug
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Why
Composed `join_equalities` correctly resolves equi-joins inside local relations to physical columns, but the same scope marks the join as residual. Consumers must treat residual scopes as unsupported, so CTE-chain models, which are the standard dbt style, can never be generated. TASK-44 required equivalence with the inlined query for every allow-listed shape; that does not hold.

Observed at 2bc99b3 with typed schemas for raw.orders and raw.items (all integer columns):
- `WITH x AS (SELECT o.order_id, i.qty FROM raw.orders o JOIN raw.items i ON o.order_id = i.order_id) SELECT order_id FROM x` is residual (`column_comparison`, clause `on`) while emitting join equality `raw.orders.order_id = raw.items.order_id`.
- `WITH o AS (SELECT order_id FROM raw.orders), i AS (SELECT order_id, qty FROM raw.items) SELECT o.order_id FROM o JOIN i ON o.order_id = i.order_id` behaves the same.
- `SELECT d.order_id FROM (SELECT o.order_id FROM raw.orders o JOIN raw.items i ON o.order_id = i.order_id) d` behaves the same.
- The inlined `SELECT o.order_id FROM raw.orders o JOIN raw.items i ON o.order_id = i.order_id` is exact.
- A daily-revenue chain (CTEs over orders, items, products; two inner equi-joins; integer filters; GROUP BY; CASE over SUM in the projection) emits both join equalities plus four duplicate `column_comparison` residuals.

## Outcome
Composed exactness for queries reading local relations is identical to that of the equivalent inlined query for every allow-listed shape. Join equalities and exactness can never disagree: an equality that is emitted as a resolved physical join equality is never also reported as a residual.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Inner equi-joins inside CTEs, chained CTEs, nested CTEs, and derived tables whose columns map through plain-copy lineage are exact and appear only in join_equalities
- [ ] #2 Every reproduction in the description is exact with the expected join equalities and column domains
- [ ] #3 The daily-revenue chain with integer filters is exact; its GROUP BY and its CASE over an aggregate in the projection do not make conditions residual
- [ ] #4 An equality reported in join_equalities is never also reported as a residual condition, in any scope or composed result
- [ ] #5 Equi-joins over computed, aggregated, or ambiguous local columns remain residual with a reason naming the unmappable column
- [ ] #6 Tests cover each case and pass in the completeness suite
<!-- AC:END -->
