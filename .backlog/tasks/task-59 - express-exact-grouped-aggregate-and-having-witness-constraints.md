---
id: TASK-59
title: Express exact grouped aggregate and HAVING witness constraints
status: Done
assignee: []
created_date: '2026-10-08'
labels: []
milestone: m-3
dependencies: []
references:
  - 'TASK-18'
  - 'TASK-43'
  - 'TASK-44'
  - 'sql-tdg TASK-28'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Aggregates, GROUP BY and HAVING are analyzed (TASK-18), but HAVING is residual because source-row domains do not constrain group cardinality or aggregate values. Provide a protocol-side declarative constraint on groups and supported aggregates, not an ad hoc sql-tdg parser.

SQL parsing, normalized semantics, lineage, and exactness remain owned by SQL Semantic Protocol. Preserve existing exact behavior while extending the canonical contract, and never mark unsupported cases exact. Changes must uphold the repository's outcome-first definition of done and cross-adapter parity.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Represent grouped output requirements from supported COUNT, SUM, MIN, MAX and simple HAVING comparisons as typed constraints on source groups and their aggregates.
- [x] #2 Carry group keys, distinctness, nullable input rules, and necessary/sufficient cardinality or sum bounds through supported input layers.
- [x] #3 Prove exactness only for explicitly supported constraint classes; disjunctive and noninvertible cases remain residual, not approximated.
- [x] #4 Test computed output-domain bounds, physical lineage and group witness constraints with DuckDB differential comparisons, including impossible outcomes.
- [x] #5 Document the canonical contract and evaluate dbt, direct SQL and other adapters; sql-tdg TASK-28 consumes it.
- [x] #6 Expose sufficient typed source-group obligations to construct qualifying and provably HAVING-rejected groups, including row contribution counts, aggregate bounds and group identity; mark any unprovable matching or rejected class residual instead of delegating aggregate interpretation to sql-tdg.
<!-- AC:END -->

## Implementation

- Typed `group_witness` on query statements gives sufficient source-group obligations for qualifying and rejected HAVING groups, explicit group keys, aggregate distinctness, null contributors and deterministic residuals for unsupported expressions.
- Resolved layer composition preserves source/intermediate provenance through `group_witnesses` and `boundary_kind`, without claiming a local intermediate proof guarantees physical materialization.
- Canonical JSON schema, documented direct-SQL/dbt/ODCS adapter behavior, COUNT/SUM/MIN/MAX DuckDB differential tests, and an updated dbt terminal-outcome snapshot are included. `sql-tdg TASK-28` is the downstream consumer.
