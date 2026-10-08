---
id: TASK-57
title: Emit typed schemas for constraint-only dbt source relations
status: To Do
assignee: []
created_date: '2026-10-08'
updated_date: '2026-10-08'
labels: []
milestone: m-2
dependencies: []
references:
  - TASK-30
  - TASK-33
  - TASK-40
  - TASK-49
  - sql-tdg TASK-22
  - src/dbt.rs
  - docs/adapters.md
  - docs/protocol.md
priority: high
type: bug
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
### Problem

A dbt source may be referenced solely as the target of a built-in `relationships` test (or a declared foreign key), without any compiled model SQL consuming it. For example, `raw.order_items.order_id` references `raw.orders.order_id`, but no model depends on `raw.orders`.

The dbt adapter resolves the canonical foreign-key relation, but the catalog-less source-schema fallback in `relation_schemas_from_artifacts` currently discovers manifest-only resources through `manifest.models[*].dependencies`. A constraint-only parent therefore can be absent from the emitted `source_schemas` even when its source YAML declares complete `data_type` values. `validate_schema_coverage` currently checks physical relations consumed by SQL layers, not those required by canonical relation constraints. The same requirement must be checked when catalog evidence exists.

Consumers such as sql-tdg (TASK-22) need the *typed schema* for both sides of a physical foreign-key relationship to generate rows satisfying dbt data tests. They must not read dbt metadata independently or guess the parent column types.

### Desired outcome

The dbt adapter includes correctly typed, canonically named `source_schemas` for physical source relations referenced through canonical constraints, including foreign-key targets that never appear in compiled SQL or model dependencies. Reuse the existing schema and constraint model: catalog types remain authoritative, complete manifest-declared `data_type` values are the supported fallback, and emitted semantics remain adapter-independent. Produced model relations must retain their correct produced-layer identity rather than being fabricated as physical sources.

If required schema evidence is absent, incomplete, contradictory, or ambiguous, report a deterministic, actionable error or explicit diagnostic according to the existing completeness and constraint contracts. Do not silently omit the schema or infer datatypes from constraint literals. Preserve current handling of unresolvable and unattributed dbt tests.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 A catalog-less dbt manifest with a source-to-source `relationships` test emits typed `source_schemas` for a parent relation referenced *only* by the test, with no compiled model SQL dependency on that parent.
- [ ] #2 The same constraint-only coverage holds for declared foreign keys and for relevant physical child/source relations whose constraint evidence is otherwise disconnected from model SQL.
- [ ] #3 With a `catalog.json`, referenced-only physical relations use warehouse-introspected column types; when absent from the catalog, complete manifest-declared source column types provide the fallback, preserving the existing authority order and provenance.
- [ ] #4 Missing or incomplete evidence for a required physical relation names the relation and affected column(s) in a deterministic failure/diagnostic. Unresolved, ambiguous, or unattributed constraint targets remain explicit and never become guessed schema identities.
- [ ] #5 Canonical foreign-key identities and typed referenced columns agree with emitted source schemas; produced models are not incorrectly emitted as independent physical sources. Outcomes, value domains, constraint semantics, and deterministic JSON remain intact.
- [ ] #6 Focused library tests cover a parent referenced only by a `relationships` test, a declared foreign key, the catalog and catalog-less paths, and missing/contradictory schema evidence. Assert both canonical constraints and serialized `source_schemas`.
- [ ] #7 A dbt Core fixture using actual compiled manifest artifacts reproduces the no-SQL-dependency relationship and validates library and CLI output, providing an upstream regression case for sql-tdg TASK-22.
- [ ] #8 Update adapter/protocol documentation as appropriate and evaluate equivalent canonical schema-coverage semantics across the supported SQL and ODCS evidence adapters; do not introduce a dbt-only protocol representation.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The objective is an upstream protocol fix. Do not change sql-tdg here; after this task ships in a released protocol version, sql-tdg can update its dependency and rerun TASK-22 acceptance tests, including the original paper_trail fixture.
<!-- SECTION:NOTES:END -->
