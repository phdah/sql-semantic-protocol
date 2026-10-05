---
id: TASK-29
title: Evaluate external schema and data-contract metadata standards
status: To Do
assignee: []
created_date: '2026-10-05'
labels: []
milestone: m-2
dependencies: []
---

## Description

Research and define the external metadata formats that SQL Semantic Protocol should accept when SQL alone cannot provide enough schema semantics.

The protocol must keep one canonical, parser-independent representation. External formats are evidence sources and adapter boundaries, not competing protocol models. The goal of this task is to choose which standards and producer-specific artifacts should be supported, what semantics each can provide, and how they map into the same canonical protocol types.

Open Data Contract Standard (ODCS) must be evaluated as a primary candidate. Its current YAML contract format can describe schema properties, primary keys, uniqueness, and relationships. The legacy Data Contract Specification (DCS) should be evaluated only as a compatibility format because it is deprecated in favor of ODCS.

The research must also cover dbt artifact metadata and other relevant open or widely adopted schema/contract formats. Formats that only describe field shape but cannot faithfully represent relational constraints should be identified as such rather than treated as equivalent data-contract sources.

## Acceptance Criteria

- [ ] Add a checked-in decision document comparing candidate metadata inputs and their fit for SQL Semantic Protocol.
- [ ] Evaluate ODCS as the primary open data-contract candidate, including schema identity, datatypes, required/nullability metadata, primary keys, composite keys, uniqueness, and relationships.
- [ ] Evaluate the legacy Data Contract Specification and document whether compatibility support is worthwhile despite its deprecation.
- [ ] Evaluate dbt manifest/catalog metadata as an existing first-class producer-specific source, including which key and constraint facts are actually available in each artifact.
- [ ] Survey other relevant open or widely adopted formats such as SQL DDL/catalog metadata, JSON Schema, Avro, Protobuf, OpenAPI/AsyncAPI, or comparable standards, and explicitly distinguish schema-only formats from formats that can represent relational constraints.
- [ ] Define the canonical semantic facts adapters should be able to provide: relation identity, columns, datatypes, nullability/requiredness, primary keys, unique keys, foreign keys, composite key membership/order, referenced relation/columns, and any necessary provenance or enforcement status.
- [ ] Define deterministic precedence and conflict behavior when SQL analysis, dbt artifacts, catalog metadata, and external contracts provide overlapping or contradictory evidence. Conflicts must never be silently resolved by guessing.
- [ ] Recommend the concrete adapter set to implement for the 1.1.0 milestone. ODCS should be included unless the research identifies a material incompatibility.
- [ ] Keep dependency choices separate from the architectural decision. Any new Rust dependency still requires explicit approval before implementation, per AGENTS.md.
- [ ] Create or refine follow-up implementation tasks from the decision where needed rather than expanding one implementation task indefinitely.
