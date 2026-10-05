---
id: TASK-30
title: Add canonical primary, unique, and foreign key constraints
status: To Do
assignee: []
created_date: '2026-10-05'
labels: []
milestone: m-2
dependencies:
  - TASK-29
---

## Description

Extend SQL Semantic Protocol with canonical relation-key metadata so consumers can reason about primary keys, unique keys, and foreign-key relationships without depending on the source format that supplied those facts.

The canonical protocol representation must support both single-column and composite constraints. Producer-specific representations from SQL, dbt, ODCS, or other adapters selected by TASK-29 must normalize into the same protocol model before emission.

SQL parsing should contribute constraints when they are explicitly present in supported DDL, but SQL text is not expected to be the only or primary evidence source. The dbt adapter must consume the authoritative constraint metadata available in dbt artifacts, and the external metadata adapter(s) selected by TASK-29 must be able to enrich the same relation schemas.

This task is about faithfully representing declared/proven key constraints. It must not infer uniqueness or key preservation through arbitrary transformations unless that property can be proven safely. Unknown or conflicting evidence remains explicit.

## Acceptance Criteria

- [ ] Add parser-independent canonical protocol types for primary keys, unique keys, and foreign keys.
- [ ] Support single-column and composite primary/unique keys without flattening composite semantics into unrelated per-column flags.
- [ ] Foreign keys identify the local columns, referenced relation, and referenced columns in deterministic order.
- [ ] Normalize column-level and relation/model-level source metadata into the same canonical relation constraint representation.
- [ ] Preserve any provenance or enforcement distinction required by the TASK-29 decision so a declared but unenforced constraint is not silently represented as stronger evidence than the source provides.
- [ ] Extend direct SQL analysis to capture supported PRIMARY KEY, UNIQUE, and FOREIGN KEY DDL constraints when sqlparser exposes them safely.
- [ ] Extend the dbt adapter to translate authoritative dbt key/constraint metadata into the canonical representation, including composite constraints where available.
- [ ] Implement the external metadata adapter path selected by TASK-29, with ODCS support unless TASK-29 documents a material reason not to include it.
- [ ] Allow external metadata to enrich analysis without introducing source-format-specific types into the core public protocol model.
- [ ] Define deterministic relation matching between SQL/dbt relations and external contract schemas; ambiguous matches fail or remain explicitly unresolved.
- [ ] Contradictory constraints from multiple evidence sources produce an explicit error or diagnostic according to TASK-29's conflict policy; no source silently overrides another.
- [ ] Key constraints survive target selection and protocol emission for the relation they describe.
- [ ] Do not propagate or invent primary/unique keys through projections, joins, aggregations, set operations, or other transformations unless preservation is formally proven. Unsafe cases remain absent/unknown rather than over-claimed.
- [ ] Update the active JSON Schema, protocol documentation, README examples, and public Rust API documentation.
- [ ] Add integration tests for single-column and composite primary keys, single-column and composite unique keys, foreign keys, self-references, multi-column foreign keys, conflicting metadata, unresolved relation references, and deterministic emission.
- [ ] Extend the dbt Core end-to-end fixture with representative key constraints and assert the emitted canonical protocol metadata.
- [ ] Add end-to-end coverage for the selected external contract format using a real contract fixture rather than only hand-constructed internal structs.
