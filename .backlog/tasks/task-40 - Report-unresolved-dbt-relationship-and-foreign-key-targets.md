---
id: TASK-40
title: Report unresolved dbt relationship and foreign key targets
status: To Do
assignee: []
created_date: '2026-10-07 09:27'
labels: []
milestone: m-2
dependencies: []
references:
  - TASK-30
  - sql-tdg TASK-22
  - docs/protocol.md
priority: medium
type: bug
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
docs/protocol.md promises that unresolved foreign-key metadata fails or emits an explicit diagnostic. The dbt adapter does not meet that promise.

Observed at d2b4de9:
- A self-referencing `relationships` test (the only dependency is the attached node) falls back to the raw `to` kwarg and emits `referenced_relation: "ref('stg_orders')"` with no error or diagnostic.
- A dbt `foreign_key` constraint whose `to` is neither a unique ID nor an exact relation name is passed through as raw text the same way.
- `RelationCatalog::resolve` returns unmatched text unchanged, so the bogus target looks like a valid relation identifier.

Consumers such as sql-tdg (TASK-22) need foreign-key targets that match source schema or produced relation names exactly.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Self-referencing dbt relationships tests resolve to the attached relation
- [ ] #2 A relationships test or foreign_key constraint whose target cannot be resolved to a canonical relation fails or emits an explicit constraint diagnostic, never raw Jinja or unmatched text
- [ ] #3 Every emitted foreign key referenced relation equals a canonical relation name known to the bundle
- [ ] #4 Tests cover self-references, ref and source targets, and unresolvable targets on the dbt path
<!-- AC:END -->
