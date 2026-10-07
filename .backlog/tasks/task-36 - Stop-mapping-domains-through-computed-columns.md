---
id: TASK-36
title: Stop mapping domains through computed columns
status: Done
assignee: []
created_date: '2026-10-07 09:27'
updated_date: '2026-10-07'
labels: []
milestone: m-2
dependencies: []
references:
  - TASK-31
  - TASK-32
  - sql-tdg TASK-21.5
  - sql-tdg TASK-21.7
  - src/analysis.rs
priority: high
type: bug
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Since TASK-31 (#40), analysis maps domains between an output column and its single lineage source without checking that the output column is a plain column copy. This produces unsound domains, so consumers over-claim. Three paths are affected: output domain refinement (`refine_output_domains_from_column_domains`), local-relation domain remapping (`remap_local_column_domains`), and CASE source column resolution (`resolve_case_source_column`). Composition already restricts pass-through to plain column copies.

Observed at d2b4de9:
- `SELECT amount + 1 AS x FROM t WHERE amount = 100` gives x the domain {100}; `upper(name)` with `name = 'x'` gives {'x'}.
- `CASE WHEN amount >= 100 THEN 'high' ELSE 'standard' END ... WHERE amount = 100` gives an empty domain instead of {'high', 'standard'}. This breaks sql-tdg test `advanced_raw_sql::cli_pipeline_executes_from_physical_sources_and_intermediate_boundary` (passes against 1.0.1).
- `WITH x AS (SELECT a - 10 AS b FROM t) SELECT b FROM x WHERE b BETWEEN 0 AND 5` gives `t.a in [0, 5]`; sql-tdg generated a = 2, 4, 3, all violating the query.
- `COUNT(a) AS c ... WHERE c = 3` gives `t.a = 3`; `SUM(a) > 100` gives `a > 100`; `ROW_NUMBER() ... rn = 1` gives `a = 1`.
- A CASE over a computed CTE column (`a - 10 AS b`, `SUM(a) AS total`) yields branch domains on `t.a` as if the column were copied.

This blocks the 1.1.0 release: consumers cannot upgrade from 1.0.x without regressions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Output column domains are refined from source column domains only when the output is a plain column copy
- [x] #2 Filters on computed, aggregated, or window columns of CTEs and derived tables are never mapped onto physical source columns as if copied
- [x] #3 CASE branch source domains are never attributed to a physical column through a computed local-relation column
- [x] #4 Constructs that can no longer be mapped surface as explicit unknown domains or diagnostics rather than being dropped
- [x] #5 Regression tests cover arithmetic, function, CASE, aggregate, and window columns for top-level queries, CTEs, and derived tables
- [x] #6 Composed semantics for a CASE output over a filtered source are non-empty and match the 1.0.1 behaviour for the reproduction above
<!-- AC:END -->
