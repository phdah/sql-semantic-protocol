---
id: TASK-32
title: Emit per-branch source-column domains for CASE output expressions
status: Done
assignee: []
created_date: '2026-10-06 13:25'
updated_date: '2026-10-06 17:08'
labels: []
milestone: m-2
dependencies:
  - TASK-31
priority: medium
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Output columns defined by CASE expressions carry their branches and conditions, but consumers cannot derive which physical source values reach each branch. sql-tdg wants generated data that exercises every reachable branch (for example a dbt model's revenue_category 'high'/'medium'/'low' currently only produces 'high'); per sql-tdg's architecture, predicate-to-domain derivation belongs exclusively to this protocol (consumer: sql-tdg TASK-21.7). For each CASE branch, including ELSE, emit the source-column domains under which that branch is selected, accounting for the negation of earlier branch conditions and for lineage through CTEs. Branches whose domains cannot be derived safely or are unreachable must be marked explicitly.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Searched and simple CASE output expressions expose per-branch domains on physical source columns, including ELSE
- [x] #2 Each branch domain accounts for earlier branch conditions not matching
- [x] #3 Unreachable branches are identified explicitly
- [x] #4 Branches whose domains cannot be derived safely are marked unknown with a reason rather than omitted
- [x] #5 Branch domains resolve through CTE and derived-table lineage once local-relation semantics are carried
- [x] #6 Integration tests cover searched CASE, simple CASE, ELSE, overlapping conditions, unreachable branches, and non-derivable conditions
- [x] #7 JSON Schema, protocol documentation, and public API docs reflect the new representation
<!-- AC:END -->
