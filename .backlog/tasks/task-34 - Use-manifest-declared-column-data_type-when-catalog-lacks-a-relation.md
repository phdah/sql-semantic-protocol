---
id: TASK-34
title: Use manifest-declared column data_type when catalog lacks a relation
status: To Do
assignee: []
created_date: '2026-10-06 13:25'
labels: []
milestone: m-2
dependencies: []
priority: medium
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
analyze_dbt_artifacts builds physical relation schemas only from catalog.json. When source tables do not yet exist in the warehouse, `dbt docs generate` writes an empty catalog and analysis fails with `dbt catalog has no warehouse schema for physical dependency`, even when the dbt YAML declares column `data_type` (present in manifest.json). Declared types are legitimate schema evidence and should be usable as a fallback (consumer: sql-tdg TASK-21.2).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Physical dependencies absent from catalog.json use manifest-declared column data_type when every referenced column declares one
- [ ] #2 Catalog types take precedence over declared types when both exist
- [ ] #3 A relation missing types for any required column still fails with an explicit error naming the relation and columns
- [ ] #4 Schema provenance (catalog versus declared) is preserved or documented so consumers can tell the evidence apart
- [ ] #5 Tests cover catalog-only, declared-only, mixed precedence, and missing-type failures, including a dbt end-to-end case with sources defined only in YAML
<!-- AC:END -->
