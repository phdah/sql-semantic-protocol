---
id: TASK-44
title: Carry the exactness contract through local relations and composition
status: To Do
assignee: []
created_date: '2026-10-07 18:11'
updated_date: '2026-10-07 18:11'
labels: []
milestone: m-2
dependencies:
  - TASK-43
  - TASK-47
references:
  - TASK-36
  - TASK-37
  - TASK-38
  - TASK-39
  - sql-tdg TASK-21.5
  - src/domain.rs
priority: high
type: bug
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Why
The exactness contract (previous task) must hold for the composed semantics consumers actually use: through CTEs, derived tables, and multi-layer pipelines such as dbt projects. Local-relation and composition paths currently drop information silently.

Observed at 2e6998e:
- `ValueDomain::Unknown` intersected with a known domain yields the known domain (`intersect_domains`, src/domain.rs). So `WITH x AS (SELECT a - 10 AS b FROM t WHERE a > 3) SELECT b FROM x WHERE b BETWEEN 0 AND 5` composes to `t.a > 3` with no diagnostic; generated rows violated the query 8 of 8 times. The same happens for SUM, ROW_NUMBER, and CASE columns when the physical column also has a known filter.
- An outer predicate on a computed CTE column gets no diagnostic; `unresolved_local_predicate` is only raised for predicates inside CTE bodies.
- `SELECT b FROM (SELECT a - 10 AS b FROM t WHERE a > 3) d WHERE b BETWEEN 0 AND 5` gives output b the domain (3,5], which excludes producible values such as b = 1.
- `unresolved_local_predicate` is emitted on the statement while composed semantics stay `resolved` with no diagnostics.
- Diagnostics from unreferenced CTEs (`unresolved_wildcard`, `ambiguous_output_lineage`) still reach the enclosing query.

## Outcome
Composed semantics are exact only when every contributing scope (every referenced CTE and derived table in every ancestor layer, and every plain-copy hop between them) is exact. Residual conditions from any contributing scope surface on the composed semantics with the layer and scope they came from. Output domains are always sound supersets of the values the expression can produce.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Intersecting Unknown with any domain never yields a domain that presents as known; the composed domain stays Unknown with a reason
- [ ] #2 Predicates on computed, aggregate, window, or CASE columns of a local relation, whether written inside or outside the local relation, are residual on the composed semantics
- [ ] #3 Composed semantics of a layer are exact only if every contributing scope across all ancestor layers is exact, and they list every residual condition with its originating layer and scope
- [ ] #4 Output domains of computed columns in CTEs, derived tables, and producer layers never exclude a value the expression can produce for rows satisfying the conditions
- [ ] #5 Unreferenced CTEs contribute no diagnostics, residual conditions, dependencies, or joins
- [ ] #6 Composed semantics for a query reading local relations equal those of the equivalent query with the local relations inlined, for every allow-listed shape
- [ ] #7 Each reproduction listed in the description has a regression test at the composed level
- [ ] #8 Protocol docs describe how exactness and residual conditions compose
<!-- AC:END -->
