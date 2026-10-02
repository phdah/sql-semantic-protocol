---
id: TASK-16
title: Analyze SQL set operations
status: Done
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-9
---

## Description

Replace the current explicit unsupported handling for SQL set operations with semantic analysis for UNION, UNION ALL, INTERSECT, and EXCEPT where supported by the selected sqlparser dialect.

Set-operation analysis must model positional output-column alignment, merged dependencies, lineage from all contributing branches, and conservative value-domain behavior.

## Acceptance Criteria

- [x] UNION and UNION ALL produce output columns with deterministic positional lineage from all branches.
- [x] INTERSECT and EXCEPT are represented semantically when supported by the selected dialect.
- [x] Branch dependencies are preserved and merged deterministically.
- [x] Output-column naming follows SQL semantics rather than inventing names from later branches.
- [x] Arity mismatches or unresolved branch outputs are surfaced explicitly.
- [x] Value domains are combined only when mathematically safe; otherwise they degrade conservatively to unknown/unbounded semantics.
- [x] Set-level ORDER BY, LIMIT, or equivalent supported clauses remain associated with the correct result.
- [x] Nested and chained set operations are covered by tests.
- [x] Tests cover at least two dialects with different set-operation syntax or capabilities.
