---
id: TASK-41
title: Report dbt tests and constraints the adapter does not carry
status: Done
assignee: []
created_date: '2026-10-07 09:27'
updated_date: '2026-10-07'
labels: []
milestone: m-2
dependencies: []
references:
  - TASK-33
  - sql-tdg TASK-22
  - docs/protocol.md
priority: medium
type: bug
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The dbt adapter extracts built-in generic tests (unique, not_null, accepted_values, relationships) and emits `unsupported_dbt_test` for other generic tests, but several kinds of test and constraint metadata are skipped with no diagnostic. Consumers that must honour every dbt data test (sql-tdg TASK-22) cannot tell that a test was ignored.

Observed at d2b4de9:
- Singular tests (no `test_metadata`) are skipped silently.
- Test config such as `where`, `severity`, `warn_if`, `error_if`, `limit`, and `fail_calc` is never read; a unique test with `where: "id > 5"` is emitted as a plain unique key.
- dbt model constraints of type `check` and `custom` are skipped silently (the SQL DDL path does emit `unsupported_check_constraint`).
- An unsupported test attached to a node without a relation is skipped silently.
- `unsupported_dbt_test` appears only on `RelationConstraintSet::diagnostics()`, not in bundle-level diagnostics.

Dropping `where` makes the emitted constraint stronger than the real test, which is safe for generated data but can cause false conflicts; the decision on how to represent this belongs in this task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Singular dbt tests attached to a relation produce an explicit constraint diagnostic
- [x] #2 Built-in tests with config that changes their meaning (for example where) are either represented faithfully or reported with a diagnostic naming the ignored config
- [x] #3 dbt check and custom constraints produce an explicit diagnostic consistent with the SQL DDL path
- [x] #4 Unsupported tests are never skipped without a diagnostic, including tests attached to relation-less nodes
- [x] #5 Protocol documentation lists exactly which dbt tests and configs are carried and how the rest are reported
- [x] #6 Tests cover each reported case
<!-- AC:END -->
