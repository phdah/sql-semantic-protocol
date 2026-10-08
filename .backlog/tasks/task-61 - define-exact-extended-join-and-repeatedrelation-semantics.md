---
id: TASK-61
title: Define exact extended join and repeated-relation semantics
status: To Do
assignee: []
created_date: '2026-10-08'
labels: []
milestone: m-3
dependencies: []
references:
  - 'TASK-43'
  - 'TASK-45'
  - 'TASK-52'
  - 'sql-tdg TASK-25'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Canonical physical equality joins and their exactness are supported (TASK-45, TASK-52), but outer, semi, anti, non-equality and repeated/self-joins are residual. Provide a typed row-membership contract that distinguishes matched and unmatched row obligations.

SQL parsing, normalized semantics, lineage, and exactness remain owned by SQL Semantic Protocol. Preserve existing exact behavior while extending the canonical contract, and never mark unsupported cases exact. Changes must uphold the repository's outcome-first definition of done and cross-adapter parity.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Specify canonical exact-match, non-match, null-extension and instance identity obligations for LEFT/RIGHT/FULL, SEMI and ANTI joins where provable.
- [ ] #2 Model simple non-equality join constraints and repeated/self-join relation instances without conflating constraints on different aliases.
- [ ] #3 Preserve physical lineage and multi-layer composition, with explicit residuals for complex or unsupported shapes.
- [ ] #4 Test NULL, missing parent, duplicate key, multi-match and relation alias cases against DuckDB, including dialect-compatible variants.
- [ ] #5 Document protocol semantics and consumer requirements; sql-tdg TASK-25 depends on this task.
<!-- AC:END -->
