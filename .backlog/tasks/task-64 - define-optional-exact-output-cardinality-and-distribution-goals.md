---
id: TASK-64
title: Define optional exact output cardinality and distribution goals
status: In Progress
assignee: []
created_date: '2026-10-08'
labels: []
milestone: m-3
dependencies:
  - TASK-58
  - TASK-59
  - TASK-60
references:
  - 'TASK-58'
  - 'TASK-61'
  - 'TASK-28'
  - 'TASK-43'
  - 'TASK-59'
  - 'TASK-60'
  - 'sql-tdg TASK-30'
priority: medium
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The existing row-condition exactness contract describes qualifying source-row combinations, not requested output cardinality, group counts or distributions. A generator must distinguish source row counts from final results.

The full TASK-64 acceptance scope depends on exact set membership (TASK-58), grouped aggregates (TASK-59) and window ranking (TASK-60). Existing exact inner joins can participate without TASK-61; goals involving extended join semantics remain residual until TASK-61 exposes and releases the required match/multiplicity contract. A narrower implementation may be staged, but this task is not Done until its advertised supported classes satisfy the full acceptance criteria.

SQL parsing, normalized semantics, lineage, and exactness remain owned by SQL Semantic Protocol. Preserve existing exact behavior while extending the canonical contract, and never mark unsupported cases exact. Changes must uphold the repository's outcome-first definition of done and cross-adapter parity.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add a separate optional typed outcome-goal contract for feasible result cardinalities, group cardinalities and distributions without redefining source-row exactness.
- [ ] #2 Specify interaction with DISTINCT, join multiplicity, aggregation, windows and NULL, including unsatisfiable goal reporting.
- [x] #3 Preserve cross-layer and multi-outcome identity and distinguish semantic facts from caller-requested goals.
- [ ] #4 Test output bounds and representative feasible/impossible goals against a SQL execution engine.
- [x] #5 Keep backward compatibility or document appropriate protocol versioning; sql-tdg TASK-30 consumes this feature.
<!-- AC:END -->

## Implementation notes (2026-10-09)

The opt-in Rust API `AnalysisBundle::set_outcome_goals` evaluates exact output-row,
surviving-group and complete typed histogram requests by stable output layer.
The current proofs cover literal/global-aggregate singleton rows, simple GROUP BY
row-to-group consistency, single-column DISTINCT/NULL uniqueness, complete-histogram
arithmetic, and empty direct physical-source projections. Unsupported combinations,
including more complex join/window multiplicities and nonliteral distribution
feasibility, retain actionable residuals rather than claiming exactness.

Remaining acceptance work: broaden constructive cardinality and distribution
witnesses for supported grouping/set/window/join classes, and extend differential
execution coverage to those cases before marking the task Done.
