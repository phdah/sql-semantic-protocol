---
id: TASK-29
title: Evaluate external schema and data-contract metadata standards
status: Done
assignee: []
created_date: '2026-10-05'
updated_date: '2026-10-06'
labels: []
milestone: m-2
dependencies: []
---

## Description

Research and define the external metadata formats that SQL Semantic Protocol should accept when SQL alone cannot provide enough schema semantics.

The protocol must keep one canonical, parser-independent representation. External formats are evidence sources and adapter boundaries, not competing protocol models. The goal of this task is to choose which standards and producer-specific artifacts should be supported, what semantics each can provide, and how they map into the same canonical protocol types.

Open Data Contract Standard (ODCS) must be evaluated as a primary candidate. Its current YAML contract format can describe schema properties, primary keys, uniqueness, and relationships. The legacy Data Contract Specification (DCS) should be evaluated only as a compatibility format because it is deprecated in favor of ODCS.

The research must also cover dbt artifact metadata and other relevant open or widely adopted schema/contract formats. Formats that only describe field shape but cannot faithfully represent relational constraints should be identified as such rather than treated as equivalent data-contract sources.

Decision: [DECISION-1: External metadata sources and conflict policy](../decisions/decision-1%20-%20External-metadata-sources-and-conflict-policy.md).

## Acceptance Criteria

- [x] Add a checked-in decision document comparing candidate metadata inputs and their fit for SQL Semantic Protocol.
- [x] Evaluate ODCS as the primary open data-contract candidate, including schema identity, datatypes, required/nullability metadata, primary keys, composite keys, uniqueness, and relationships.
- [x] Evaluate the legacy Data Contract Specification and document whether compatibility support is worthwhile despite its deprecation.
- [x] Evaluate dbt manifest/catalog metadata as an existing first-class producer-specific source, including which key and constraint facts are actually available in each artifact.
- [x] Survey other relevant open or widely adopted formats such as SQL DDL/catalog metadata, JSON Schema, Avro, Protobuf, OpenAPI/AsyncAPI, or comparable standards, and explicitly distinguish schema-only formats from formats that can represent relational constraints.
- [x] Define the canonical semantic facts adapters should be able to provide: relation identity, columns, datatypes, nullability/requiredness, primary keys, unique keys, foreign keys, composite key membership/order, referenced relation/columns, and any necessary provenance or enforcement status.
- [x] Define deterministic precedence and conflict behavior when SQL analysis, dbt artifacts, catalog metadata, and external contracts provide overlapping or contradictory evidence. Conflicts must never be silently resolved by guessing.
- [x] Recommend the concrete adapter set to implement for the 1.1.0 milestone. ODCS should be included unless the research identifies a material incompatibility.
- [x] Keep dependency choices separate from the architectural decision. Any new Rust dependency still requires explicit approval before implementation, per AGENTS.md.
- [x] Create or refine follow-up implementation tasks from the decision where needed rather than expanding one implementation task indefinitely.
