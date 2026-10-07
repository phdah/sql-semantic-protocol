---
id: TASK-45
title: Expose canonical physical join equalities on composed semantics
status: Done
assignee: []
created_date: '2026-10-07 18:11'
updated_date: '2026-10-08'
labels: []
milestone: m-2
dependencies:
  - TASK-43
  - TASK-47
references:
  - TASK-37
  - sql-tdg TASK-21.5
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Why
Generators must coordinate join keys across physical sources, including joins inside CTEs and in upstream layers. Today consumers rebuild that themselves: they walk every ancestor layer's `QueryStatement::joins()`, parse condition trees, and map aliases to relations. That re-derives semantics outside the protocol and is where alias mistakes happen.

Observed at 2e6998e:
- `FROM t, u WHERE t.x = u.y` (implicit join) yields Unknown domains on both columns and no join, so a supported equi-join cannot be coordinated.
- Join conditions mix equalities with other conditions in one tree (`ON t.x = u.y AND t.a > 5`), so consumers must split them.
- Self-joins rewritten to physical columns lose alias identity: `emp a JOIN emp b ON a.manager_id = b.id` becomes `emp.manager_id = emp.id`.
- Joins inside CTEs keep logical `left`/`right` participants that can name a CTE, while the condition names physical columns.

## Outcome
Resolved composed semantics expose the complete set of equality relationships that the exactness guarantee relies on: physical relation, column, and relation instance on each side, plus the join kind and originating layer. Consumers no longer need per-layer join trees to generate data.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Resolved composed semantics list every equality relationship that participates in the exactness guarantee, with physical relation, column, relation instance identity, join kind, and originating layer on each entry
- [x] #2 Equi-joins written in WHERE over comma or CROSS joins are represented as inner equality relationships, not Unknown domains
- [x] #3 Equalities from joins inside referenced CTEs, derived tables, and every ancestor layer are included after plain-copy mapping; unmappable equalities are residual
- [x] #4 Self-joins and repeated relation instances keep distinct instance identities on equalities and on column domains, or are residual
- [x] #5 Outer joins are listed with their kind, and their conditions are residual until an exact outer-join contract exists
- [x] #6 Tests cover explicit and implicit inner joins, multi-column keys, joins inside single and chained CTEs, joins in upstream layers, self-joins, and a dbt-style CTE chain joining three sources
- [x] #7 JSON Schema and protocol docs describe the equality relationship representation
<!-- AC:END -->


## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented in PR #58. Resolved composed semantics now expose deterministic physical join equalities with relation-instance identity, join kind, and origin layer. Explicit joins, implicit WHERE equi-joins, local CTE/derived-table joins, and upstream producer equalities compose through proven plain-copy lineage. Outer joins retain their kind while remaining residual, and repeated/self-join cases remain residual when instance identity cannot be proven safely. Schema, docs, unit/conformance coverage, and dbt Core E2E coverage were updated.
<!-- SECTION:NOTES:END -->
