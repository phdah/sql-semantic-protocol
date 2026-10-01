---
id: TASK-16
title: Analyze SQL set operations
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-9
---

## Description

Replace the current explicit unsupported handling for SQL set operations with semantic analysis for UNION, UNION ALL, INTERSECT, and EXCEPT where supported by the selected sqlparser dialect.

Set-operation analysis must model positional output-column alignment, merged dependencies, lineage from all contributing branches, and conservative value-domain behavior.

## Acceptance Criteria

- [ ] UNION and UNION ALL produce output columns with deterministic positional lineage from all branches.
- [ ] INTERSECT and EXCEPT are represented semantically when supported by the selected dialect.
- [ ] Branch dependencies are preserved and merged deterministically.
- [ ] Output-column naming follows SQL semantics rather than inventing names from later branches.
- [ ] Arity mismatches or unresolved branch outputs are surfaced explicitly.
- [ ] Value domains are combined only when mathematically safe; otherwise they degrade conservatively to unknown/unbounded semantics.
- [ ] Set-level ORDER BY, LIMIT, or equivalent supported clauses remain associated with the correct result.
- [ ] Nested and chained set operations are covered by tests.
- [ ] Tests cover at least two dialects with different set-operation syntax or capabilities.
