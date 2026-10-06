---
id: TASK-33
title: Add canonical not-null and accepted-values constraints from dbt data tests
status: To Do
assignee: []
created_date: '2026-10-06 13:25'
labels: []
milestone: m-2
dependencies:
  - TASK-29
priority: medium
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
dbt generic data tests declared in YAML (not_null, accepted_values) state constraints that generated or analyzed data must satisfy. TASK-29/TASK-30 cover primary, unique, and foreign-key (relationships) constraints but not these. sql-tdg needs all of them through the protocol so generated sources pass `dbt build` (consumer: sql-tdg TASK-22). Normalize dbt not_null and accepted_values tests on sources and models (column-level, including arguments such as values and quote) into canonical, source-format-independent relation constraints following the provenance and conflict policy decided in TASK-29. Unsupported or unparseable dbt tests must be reported explicitly rather than silently ignored.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Canonical protocol types represent column not-null and accepted-values constraints independent of dbt
- [ ] #2 The dbt adapter translates not_null and accepted_values tests on sources and models into those constraints
- [ ] #3 Accepted values preserve literal types and quoting semantics from the dbt test arguments
- [ ] #4 Constraints follow the TASK-29 provenance and conflict policy and are not represented as stronger than the source evidence
- [ ] #5 Unsupported dbt test kinds or arguments are reported explicitly
- [ ] #6 Constraints survive target selection and protocol emission for the relations they describe
- [ ] #7 The dbt Core end-to-end fixture declares these tests and asserts the emitted constraints
- [ ] #8 JSON Schema, protocol documentation, and public API docs reflect the new representation
<!-- AC:END -->
