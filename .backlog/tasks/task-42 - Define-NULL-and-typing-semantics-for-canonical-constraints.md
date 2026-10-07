---
id: TASK-42
title: Define NULL and typing semantics for canonical constraints
status: In Progress
assignee: []
created_date: '2026-10-07 09:27'
updated_date: '2026-10-07'
labels: []
milestone: m-2
dependencies: []
references:
  - TASK-30
  - TASK-33
  - sql-tdg TASK-22
priority: medium
type: enhancement
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Canonical constraints from TASK-30 and TASK-33 leave two semantic points undefined, so consumers must guess or re-interpret dbt behaviour.

- NULL semantics: in dbt, NULL rows pass unique, accepted_values, and relationships tests. The protocol does not state whether `UniqueKey`, `AcceptedValues`, and `ForeignKey` constraints admit NULL.
- accepted_values typing: values are typed (`ConstraintValue`) but not checked against the column datatype. With `quote: false`, string values are raw SQL (for example `["1", "2"]` is emitted as `String("1")`, `String("2")`), which consumers must not parse.

Consumers such as sql-tdg (TASK-22) need these to generate data that passes `dbt build`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Protocol documentation and public API docs state whether unique, accepted values, and foreign key constraints admit NULL values
- [ ] #2 Accepted values are expressed in a form consumers can compare with the column datatype without parsing SQL, or unrepresentable values produce an explicit diagnostic
- [ ] #3 quote: false accepted values are never emitted as raw SQL text presented as string values
- [ ] #4 Tests cover NULL handling and quoted and unquoted accepted values for string and numeric columns
<!-- AC:END -->
