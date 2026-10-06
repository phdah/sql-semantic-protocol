---
id: TASK-35
title: Add ODCS v3.2 external metadata adapter
status: Done
assignee: []
created_date: '2026-10-06'
updated_date: '2026-10-06'
labels: []
milestone: m-2
dependencies:
  - TASK-30
  - TASK-33
priority: medium
type: feature
---

## Description

Add Open Data Contract Standard (ODCS) v3.2 as the external vendor-neutral metadata adapter selected by DECISION-1.

ODCS is evidence only. It must enrich the same canonical RelationSchema, datatype, key, not-null, accepted-values, provenance, and enforcement model used by SQL and dbt. No ODCS-specific type may leak into the public protocol contract.

The adapter must follow the deterministic relation matching and conflict policy from DECISION-1. YAML support is required for practical ODCS use. If implementation needs a new Rust dependency for YAML parsing, obtain explicit user approval first as required by AGENTS.md.

## Acceptance Criteria

- [x] Parse and validate ODCS v3.2 contracts from YAML, with explicit version errors for unsupported ODCS versions.
- [x] Map ODCS schema object physicalName when present, otherwise name, into canonical relation identity through the existing relation resolver.
- [x] Ambiguous or unresolved relation matches fail or remain explicitly unresolved; no fuzzy matching or arbitrary binding.
- [x] Map property physicalType to the canonical DataType model when safely parseable and use logicalType only as lower-authority fallback evidence.
- [x] Map required into the canonical not-null constraint model from TASK-33.
- [x] Map primaryKey and primaryKeyPosition into ordered single/composite canonical primary keys.
- [x] Map property unique into canonical unique-key constraints without inventing composite uniqueness not represented by ODCS.
- [x] Map property-level and schema-level relationships into canonical foreign keys, including ordered composite relationships and referenced columns.
- [x] Map ODCS enum values into canonical accepted-values constraints while preserving scalar literal types.
- [x] Preserve ODCS provenance and unknown enforcement according to DECISION-1.
- [x] Merge ODCS evidence with SQL/dbt/catalog evidence using DECISION-1 datatype precedence and constraint conflict semantics, surfacing every contradiction explicitly.
- [x] Resolve in-contract ODCS relationship references deterministically; external cross-contract references require explicitly supplied referenced contracts and otherwise produce an explicit unsupported/unresolved result.
- [x] Support target selection and deterministic protocol emission after ODCS enrichment.
- [x] Add real ODCS fixture coverage for single and composite primary keys, unique keys, single and composite foreign keys, required fields, enums, datatypes, ambiguous relation identity, conflicting evidence, and deterministic output.
- [x] Update README, protocol documentation, and public API docs for the ODCS adapter.
