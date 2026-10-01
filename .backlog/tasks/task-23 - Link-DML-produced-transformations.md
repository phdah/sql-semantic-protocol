---
id: TASK-23
title: Link DML-produced transformations
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-13
  - TASK-14
---

## Description

Extend cross-query composition beyond query-backed DDL so transformations that write into existing relations can participate in the dependency graph.

Initial coverage should include INSERT INTO ... SELECT and MERGE statements where the selected dialect/sqlparser representation provides enough semantics to determine the written relation and its source query semantics safely.

Unlike CREATE TABLE/VIEW, these statements mutate an existing target. The protocol must therefore distinguish produced/replaced datasets from appended or conditionally mutated datasets rather than pretending every write defines a complete relation from scratch.

## Acceptance Criteria

- [ ] INSERT INTO ... SELECT records both the written target relation and the source-query semantics.
- [ ] MERGE records the target relation, source dependencies, match condition, and supported update/insert/delete actions.
- [ ] The semantic model distinguishes replacement/definition writes from append and conditional mutation semantics.
- [ ] Cross-query graph edges can link downstream readers to DML-written relations without claiming the DML fully defines the target when it does not.
- [ ] Transitive lineage is composed only where write semantics justify it; otherwise the result degrades explicitly to partial/unknown lineage.
- [ ] Dialect-specific DML syntax remains isolated at the AST-to-domain boundary.
- [ ] Unsupported write operations remain explicit diagnostics.
- [ ] Tests cover INSERT-select and MERGE in supported dialects, including downstream readers.
