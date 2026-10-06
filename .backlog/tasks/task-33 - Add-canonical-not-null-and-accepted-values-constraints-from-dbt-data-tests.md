---
id: TASK-33
title: Add canonical not-null and accepted-values constraints from dbt data tests
status: Done
assignee: []
created_date: '2026-10-06 13:25'
updated_date: '2026-10-06'
labels: []
milestone: m-2
dependencies:
  - TASK-30
priority: medium
type: feature
---

## Description

dbt generic data tests declared in YAML (not_null, accepted_values) state constraints that generated or analyzed data must satisfy. sql-tdg needs them through the protocol so generated sources pass dbt build (consumer: sql-tdg TASK-22).

Add source-format-independent canonical column constraints that reuse the provenance/enforcement and conflict model introduced by TASK-30. The dbt adapter translates generic tests into those canonical facts. TASK-35 later maps ODCS required and enum metadata into the same representation.

Unsupported or unparseable dbt tests must be reported explicitly rather than silently ignored.

## Acceptance Criteria

- [x] Canonical protocol types represent column not-null and accepted-values constraints independent of dbt.
- [x] Canonical column constraints reuse the provenance/enforcement evidence and conflict policy introduced by TASK-30 and DECISION-1.
- [x] The dbt adapter translates not_null and accepted_values tests on sources and models into those constraints.
- [x] Accepted values preserve literal types and quoting semantics from the dbt test arguments.
- [x] Multiple accepted-values constraints combine deterministically according to DECISION-1, including explicit unsatisfiable/conflict handling for an empty intersection.
- [x] Unsupported dbt test kinds or arguments are reported explicitly.
- [x] Constraints survive target selection and protocol emission for the relations they describe.
- [x] The dbt Core end-to-end fixture declares these tests and asserts the emitted constraints.
- [x] JSON Schema, protocol documentation, and public API docs reflect the new representation.
