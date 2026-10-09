---
id: TASK-64
title: Define optional exact output cardinality and distribution goals
status: Done
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
- [x] #2 Specify interaction with DISTINCT, join multiplicity, aggregation, windows and NULL, including unsatisfiable goal reporting.
- [x] #3 Preserve cross-layer and multi-outcome identity and distinguish semantic facts from caller-requested goals.
- [x] #4 Test output bounds and representative feasible/impossible goals against a SQL execution engine.
- [x] #5 Keep backward compatibility or document appropriate protocol versioning; sql-tdg TASK-30 consumes this feature.
<!-- AC:END -->

## Implementation notes (2026-10-09)

Optional `OutcomeGoal` requests remain distinct from SQL semantic facts and are
addressed by stable output-layer identity. Typed `OutcomeWitness` plans provide
generator-consumable, complete-source constructions for:

- direct row-preserving source counts and typed integer/NULL histograms;
- single-key GROUP BY counts, including exact COUNT(*) HAVING positive cases
  and standalone surviving-group goals;
- independent equi-join pairs for INNER, LEFT, RIGHT and FULL joins;
- ROW_NUMBER with exact QUALIFY rank filters and bounded unpartitioned output;
- UNION, INTERSECT and EXCEPT tuple multiplicities, including DISTINCT/ALL
  semantics and repeated typed histogram values through explicit frequency scaling.

All positive plans require proved operator witnesses and external controllable
sources. Integer-key cases require typed schema evidence and cannot bypass
physical constraints. The API returns `residual` rather than constructing
unproven combinations; incompatible count bounds, DISTINCT duplicates,
impossible global ranks and inconsistent distributions are `unsatisfiable`.

DuckDB execution tests exercise feasible and impossible requests, SQL NULL,
aggregate grouping, outer joins, rank limits and ALL/DISTINCT set multiplicity.
The feature is opt-in, does not alter ordinary protocol emission, and is
consumed from the same canonical `AnalysisBundle` API across evidence adapters.
