---
id: DECISION-1
title: "External metadata sources and conflict policy"
date: '2026-10-06'
status: Accepted
---

# External metadata sources and conflict policy

## Context

SQL text alone cannot provide every schema fact needed by protocol consumers. SQL Semantic Protocol therefore needs adapter boundaries for metadata that is declared or observed outside query text while keeping one canonical, parser-independent protocol model.

This decision evaluates candidate metadata sources for the 1.1.0 milestone and defines how overlapping evidence is normalized, ranked, and reported.

## Decision

The 1.1.0 metadata surface will use these evidence sources:

1. SQL DDL parsed by the existing analyzer for constraints that are explicitly present in SQL.
2. dbt manifest.json as the first-class source for dbt resource identity, declared column metadata, model constraints, dependencies, and generic data-test declarations.
3. dbt catalog.json as the authoritative dbt source for warehouse-introspected physical columns and datatypes when a catalog entry exists.
4. Open Data Contract Standard (ODCS) v3.2 as the external, vendor-neutral contract format.

The legacy Data Contract Specification (DCS) will not receive a native 1.1.0 adapter. It is deprecated in favor of ODCS and existing tooling can convert DCS contracts to ODCS. Native DCS support should only be reconsidered if concrete compatibility demand justifies the additional surface.

JSON Schema, Avro, Protobuf, OpenAPI, and AsyncAPI are useful schema sources but are not selected as relational metadata adapters for 1.1.0 because they do not natively model the complete primary-key, unique-key, and foreign-key semantics needed by the protocol.

## Capability comparison

| Source | Relation identity | Datatypes | Required / nullability | Primary key | Unique key | Foreign key | Accepted values | 1.1.0 role |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| SQL DDL | Yes | Yes when declared | Yes when declared | Yes | Yes | Yes | Possible through CHECK semantics, not selected here | Native analyzer evidence |
| dbt manifest.json | Yes | Declared data_type | Declared constraints/tests | Explicit model/column constraints | Explicit constraints and generic unique tests | Explicit constraints and generic relationships tests | Generic accepted_values tests | Native dbt evidence |
| dbt catalog.json | Yes through manifest unique_id pairing | Warehouse-introspected | No portable nullability field in the catalog contract | No | No | No | No | Physical schema authority |
| ODCS v3.2 | name / physicalName | logicalType and physicalType | required | primaryKey plus primaryKeyPosition supports composite order | unique supports property-level uniqueness | relationships supports single and composite foreign keys | enum | External contract adapter |
| Legacy DCS | Yes | Yes | Yes | Yes | Format-dependent | Format-dependent | Format-dependent | Compatibility only through conversion to ODCS |
| JSON Schema | Object/schema identity only | Yes | required | No relational key semantics | No relational key semantics | No | enum / const | Not a relational adapter |
| Avro | Record identity | Yes | Nullable unions | No | No | No | enum symbols | Not a relational adapter |
| Protobuf | Message identity | Yes | Presence/cardinality | No | No | No | enum symbols | Not a relational adapter |
| OpenAPI / AsyncAPI | API/message identity | JSON-Schema-like payload schemas | JSON-Schema-like | No relational key semantics | No relational key semantics | No | JSON-Schema-like enum | Not a relational adapter |

ODCS v3.2 has one material limitation for this milestone: it can represent composite primary keys through primaryKeyPosition and composite foreign keys through relationship arrays, but its standard unique flag is property-level rather than a general composite unique-key construct. The canonical protocol must still support composite unique keys because SQL and dbt can provide them.

## Canonical semantic facts

Adapters normalize into protocol-owned facts. Source-specific structs must not leak into the public protocol model.

The canonical metadata model must be able to represent:

- canonical relation identity
- ordered columns and canonical datatypes
- explicit required / nullability evidence
- primary keys, including ordered composite membership
- unique keys, including ordered composite membership
- foreign keys with ordered local columns, referenced relation, and ordered referenced columns
- accepted-value constraints where available
- provenance for every declared constraint
- enforcement strength when the source can establish it

Constraint provenance and enforcement are separate concepts. A dbt generic test declaration, an ODCS declaration, and a SQL DDL clause may assert the same semantic fact but do not prove the same runtime enforcement. The canonical representation should therefore preserve the source kind and source identity, while enforcement is one of enforced, not_enforced, or unknown when that status can be established safely. A declaration must never be promoted to enforced merely because the source format supports enforcement in some engines.

## Relation matching

Canonical relation identity continues to be owned by SQL analysis, dbt relation metadata, and RelationCatalog.

For ODCS schema objects:

1. Use physicalName when present, otherwise name.
2. Resolve that identifier through the same canonical relation-resolution rules used elsewhere in the project.
3. Exact canonical matches are preferred.
4. A partially qualified value may resolve only when the existing resolver finds one unique match.
5. Ambiguous matches are errors or explicit unresolved diagnostics. No fuzzy or arbitrary matching is allowed.

ODCS relationship references must resolve deterministically to schema objects and properties. Composite foreign-key arrays preserve declared order. External cross-contract references may only be followed when the referenced contract is explicitly supplied to the adapter; missing external documents are reported rather than fetched or guessed.

## Precedence and conflict policy

There is no global last-write-wins precedence between metadata sources. Facts are normalized independently and then merged by semantic identity.

### Datatypes

Datatype evidence has an authority order because the sources describe different levels of reality:

1. warehouse-introspected dbt catalog type
2. dbt manifest declared data_type
3. ODCS physicalType
4. ODCS logicalType or another schema-only logical type

The highest-authority available value is the operational datatype. A contradictory lower-authority value is still surfaced as a diagnostic with both evidence sources; precedence must never make a disagreement silent. This policy is consistent with TASK-34, where catalog types take precedence over manifest-declared types.

### Constraints

Constraint evidence is conjunctive, not overriding.

- Identical constraints from multiple sources are coalesced while preserving every provenance entry.
- Omission from one source is not contradictory evidence.
- Different unique keys and different foreign keys may coexist.
- Two different primary-key definitions for the same relation conflict.
- An explicit nullable claim and an explicit not-null claim conflict.
- Accepted-value sets from independent sources combine by intersection. An empty intersection is an explicit unsatisfiable conflict.
- A relationship whose local/referenced arity differs, whose reference cannot be resolved, or whose evidence points to incompatible targets is unresolved/error rather than guessed.

Adapters may report either a typed error or a protocol diagnostic according to the surrounding API, but no contradiction may be silently discarded.

## dbt artifact ownership

dbt artifacts have distinct responsibilities:

- manifest.json owns dbt resource identity, model SQL, dependency metadata, declared column data_type, model/column constraints, and generic data-test declarations.
- catalog.json owns warehouse-introspected columns and datatypes.
- catalog.json is not treated as a source of key or test constraints because its portable artifact contract does not expose those facts.

For model constraints, dbt platform enforcement differs by adapter. The manifest declaration therefore supplies the constraint fact and provenance, while enforcement remains unknown unless the artifact itself provides enough evidence to prove it.

Generic tests are semantic assertions, not proof that a warehouse constraint exists or that a test run passed. unique and relationships belong to key/relationship evidence; not_null and accepted_values belong to column constraints. A primary key must not be inferred merely from unique plus not_null unless a source explicitly declares it as a primary key.

## ODCS adapter scope

ODCS v3.2 is the selected external adapter for 1.1.0 because it directly covers the relational facts needed by this milestone:

- property names and logical/physical datatypes
- required
- primaryKey and primaryKeyPosition
- unique
- foreign-key relationships, including composite relationships
- enum accepted values

The adapter must preserve ODCS as evidence only. The emitted protocol remains the same regardless of whether a fact came from SQL, dbt, or ODCS.

YAML support is required for practical ODCS use. This decision does not select a Rust YAML dependency. AGENTS.md requires explicit user approval before adding a new dependency, so the implementation task must request approval if the standard library, serde_json, and existing dependencies are insufficient.

## Follow-up tasks

The implementation is split so one task does not own every adapter and constraint type:

- TASK-30: add canonical primary/unique/foreign-key constraints and provenance/enforcement infrastructure, with SQL DDL and dbt support.
- TASK-33: add canonical not-null and accepted-values constraints and dbt generic-test support. It depends on TASK-30 so both features reuse one evidence/conflict model.
- TASK-34: keep the existing independent dbt datatype fallback from catalog to manifest-declared data_type.
- TASK-35: add the ODCS v3.2 adapter after the canonical key and column-constraint models exist.

No first-class DCS, JSON Schema, Avro, Protobuf, OpenAPI, or AsyncAPI adapter is planned for 1.1.0.

## Sources

Research checked on 2026-10-06:

- ODCS v3.2 schema: https://bitol-io.github.io/open-data-contract-standard/v3.2.0/schema/
- ODCS v3.2 references and relationships: https://bitol-io.github.io/open-data-contract-standard/v3.2.0/references/
- ODCS changelog: https://bitol-io.github.io/open-data-contract-standard/v3.2.0/changelog/
- DCS to ODCS migration: https://docs.datacontract.com/migrate-dcs-to-odcs
- dbt manifest artifact: https://docs.getdbt.com/reference/artifacts/manifest-json
- dbt catalog artifact: https://docs.getdbt.com/reference/artifacts/catalog-json
- dbt model constraints: https://docs.getdbt.com/reference/resource-properties/constraints
- dbt generic data tests: https://docs.getdbt.com/docs/build/data-tests
- Apache Avro specification: https://avro.apache.org/docs/1.12.0/specification/
- Protocol Buffers language guide: https://protobuf.dev/programming-guides/proto3/
- OpenAPI 3.1 specification: https://spec.openapis.org/oas/v3.1
- AsyncAPI specification: https://github.com/asyncapi/spec/blob/master/spec/asyncapi.md
