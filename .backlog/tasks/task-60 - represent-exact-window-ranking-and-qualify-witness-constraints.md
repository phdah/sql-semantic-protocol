---
id: TASK-60
title: Represent exact window ranking and QUALIFY witness constraints
status: To Do
assignee: []
created_date: '2026-10-08'
labels: []
milestone: m-3
dependencies: []
references:
  - 'TASK-17'
  - 'TASK-43'
  - 'TASK-44'
  - 'sql-tdg TASK-29'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Window syntax, named windows, frame metadata and QUALIFY are analyzed (TASK-17), but QUALIFY or downstream rank filters are residual. Represent safe partition and ordering requirements for generator-driven row witnesses.

SQL parsing, normalized semantics, lineage, and exactness remain owned by SQL Semantic Protocol. Preserve existing exact behavior while extending the canonical contract, and never mark unsupported cases exact. Changes must uphold the repository's outcome-first definition of done and cross-adapter parity.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Define canonical partition, ordering, tie and rank requirements for a minimal supported class such as ROW_NUMBER() = 1 and <= N.
- [ ] #2 Connect QUALIFY and safe projected-window filters to source partitions and physical column lineage without guessing ordering or tie semantics.
- [ ] #3 Describe exactness preconditions for null ordering, collation, frame and dialect-specific behavior; unsupported cases stay residual.
- [ ] #4 Test positive, negative and impossible rank conditions against real SQL execution and across parser-supported dialects.
- [ ] #5 Preserve existing window expression and output-domain contracts, and document the new semantics; sql-tdg TASK-29 depends on it.
<!-- AC:END -->
