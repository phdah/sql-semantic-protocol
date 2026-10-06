---
id: TASK-30
title: Add canonical primary, unique, and foreign key constraints
status: Done
assignee: []
created_date: '2026-10-05'
updated_date: '2026-10-06'
labels: []
milestone: m-2
dependencies:
  - TASK-29
---

## Description

Extend SQL Semantic Protocol with canonical relation-key metadata so consumers can reason about primary keys, unique keys, and foreign-key relationships without depending on the source format that supplied those facts.

The canonical protocol representation must support both single-column and composite constraints. SQL DDL and dbt metadata normalize into the same protocol model. ODCS uses the same canonical model but is implemented separately by TASK-35 so this task remains focused on the core representation and existing first-class producers.

This task is about faithfully representing declared/proven key constraints. It must not infer uniqueness, primary keys, or key preservation through arbitrary transformations unless that property can be proven safely. Unknown or conflicting evidence remains explicit.

Follow the provenance, enforcement, datatype precedence, relation matching, and conflict policy in DECISION-1.

## Acceptance Criteria

- [x] Add parser-independent canonical protocol types for primary keys, unique keys, and foreign keys.
- [x] Support single-column and composite primary/unique keys without flattening composite semantics into unrelated per-column flags.
- [x] Foreign keys identify the local columns, referenced relation, and referenced columns in deterministic order.
- [x] Add shared provenance/enforcement evidence types usable by key constraints and later column constraints without source-format-specific protocol types.
- [x] Normalize column-level and relation/model-level source metadata into the same canonical relation constraint representation.
- [x] Preserve declared versus proven enforcement according to DECISION-1; do not infer warehouse enforcement from a source format alone.
- [x] Extend direct SQL analysis to capture supported PRIMARY KEY, UNIQUE, and FOREIGN KEY DDL constraints when sqlparser exposes them safely.
- [x] Extend the dbt adapter to translate explicit dbt model/column constraints into the canonical representation, including composite constraints where available.
- [x] Translate dbt generic unique and relationships tests into canonical uniqueness/foreign-key evidence while preserving that they are test declarations rather than warehouse enforcement.
- [x] Do not infer a primary key merely from dbt unique plus not_null tests.
- [x] Contradictory constraints from multiple evidence sources produce an explicit error or diagnostic according to DECISION-1; no source silently overrides another.
- [x] Key constraints survive target selection and protocol emission for the relation they describe.
- [x] Do not propagate or invent primary/unique keys through projections, joins, aggregations, set operations, or other transformations unless preservation is formally proven. Unsafe cases remain absent/unknown rather than over-claimed.
- [x] Expose an adapter-neutral enrichment boundary that TASK-35 can use to add ODCS evidence without adding ODCS-specific public protocol types.
- [x] Update the active JSON Schema, protocol documentation, README examples, and public Rust API documentation.
- [x] Add integration tests for single-column and composite primary keys, single-column and composite unique keys, foreign keys, self-references, multi-column foreign keys, conflicting metadata, unresolved relation references, deterministic emission, and provenance/enforcement.
- [x] Extend the dbt Core end-to-end fixture with representative explicit constraints and generic unique/relationships tests and assert the emitted canonical protocol metadata.
