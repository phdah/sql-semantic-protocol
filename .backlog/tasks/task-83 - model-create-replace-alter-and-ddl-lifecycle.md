---
id: TASK-83
title: Model complete relation-producing and destructive DDL effects
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-77
  - TASK-78
  - TASK-79
references: 
  - 'TASK-12'
  - 'TASK-23'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CREATE/REPLACE alters identities and state differently from CTAS and VIEW; dropping, renaming, truncating and schema changes break downstream dependencies.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Normalize CREATE [OR REPLACE] TABLE AS, VIEW, MATERIALIZED VIEW, TEMP/TEMPORARY/TRANSIENT TABLE, LIKE/CLONE where supported, and schema-qualified definitions.
- [ ] #2 Represent DROP, TRUNCATE, ALTER TABLE ADD/DROP/RENAME/TYPE columns, RENAME TABLE/VIEW and replacement/overwrite effects on named datasets and constraints.
- [ ] #3 Distinguish logical view definitions, materialized snapshots and physical stored tables, scoping of temporary objects, IF [NOT] EXISTS and dialect-specific DDL transaction rules.
- [ ] #4 Expose exact before/after schema and relation existence, source lineage, data preservation/loss and read dependencies; fail explicitly on unmodeled structural changes.
- [ ] #5 Execute statement sequences and verify relation catalogs plus data snapshots for portable DuckDB cases and dialect-specific variants.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
