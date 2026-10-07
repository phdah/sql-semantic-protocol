---
id: TASK-49
title: Analyze dbt manifests with declared schemas when no catalog exists
status: To Do
assignee: []
created_date: '2026-10-07 18:11'
labels: []
milestone: m-2
dependencies: []
references:
  - TASK-34
  - sql-tdg TASK-21.2
priority: medium
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Why
Projects whose sources exist only in YAML (sql-tdg TASK-21.2) have no warehouse, so they may not have a `catalog.json` at all. `analyze_dbt_artifacts` requires a `DbtCatalog`, so consumers must fabricate an empty catalog document to use manifest-declared schemas. `analyze_dbt_manifest` analyzes without schemas, so it cannot provide typed sources either.

## Outcome
A supported library and CLI path analyzes a dbt manifest with typed source schemas taken from manifest-declared column datatypes when no catalog is available. Precedence, completeness checks, and errors are identical to the existing fallback; schemas keep `source_kind: dbt_manifest`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The library exposes a way to analyze dbt artifacts with an absent catalog that uses manifest-declared datatypes as schema evidence, without callers constructing a placeholder catalog
- [ ] #2 The CLI supports the same path when catalog.json is absent, and errors name each relation or column lacking declared types
- [ ] #3 Results equal those of analyze_dbt_artifacts with an empty catalog
- [ ] #4 Tests cover a YAML-only project, partially declared relations, and a relation without declared columns
- [ ] #5 README and protocol docs describe catalog-less dbt analysis
<!-- AC:END -->
