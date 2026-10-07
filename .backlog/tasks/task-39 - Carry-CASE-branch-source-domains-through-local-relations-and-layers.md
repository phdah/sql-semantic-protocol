---
id: TASK-39
title: Carry CASE branch source domains through local relations and layers
status: Done
assignee: []
created_date: '2026-10-07 09:27'
updated_date: '2026-10-07'
labels: []
milestone: m-2
dependencies:
  - TASK-36
references:
  - TASK-32
  - sql-tdg TASK-21.7
priority: medium
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TASK-32 (#42) emits per-branch source domains on `CaseBranch::source_domains()` and `CaseExpression::else_source_domains()`. They are exact for supported comparisons, but they stop at the immediate input relation, so consumers cannot cover CASE branches of models written as CTE chains or as multi-layer pipelines.

Observed at d2b4de9:
- A CASE defined inside a CTE and projected by the outer query appears in the outer output only as a plain column reference with an unbounded domain and no branch information.
- Across composition layers the CASE expression is copied unchanged, so branch domains name the intermediate relation (for example `stage.a`) rather than physical leaves.
- Branch reachability ignores the query filters; a branch can be `Reachable` while impossible under the WHERE clause, so consumers must intersect themselves.
- For the reference dbt model daily_revenue every branch is `Unknown` because the CASE is over an aggregate. That is correct and must stay explicit.

Needed by sql-tdg TASK-21.7 (cover every CASE branch).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A CASE defined inside a CTE or derived table exposes its branch source domains on the outer output column that copies it
- [x] #2 Composed semantics express CASE branch source domains on physical source columns when every hop is a plain column copy
- [x] #3 Branch domains that cannot be mapped across a hop become Unknown with a reason rather than naming an intermediate relation as if physical
- [x] #4 The contract states whether branch reachability accounts for query filters, and if it does, branches made impossible by filters are reported Unreachable
- [x] #5 Tests cover searched CASE, simple CASE, and ELSE inside CTEs, derived tables, and multi-layer compositions, plus a CASE over an aggregate staying Unknown
- [x] #6 JSON Schema and protocol documentation describe the cross-layer branch domain behaviour
<!-- AC:END -->
