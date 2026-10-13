# TASK-79 bounded physical schema proof contract

TASK-79 certifies the **canonical schema facts and physical proof outcomes**
below. It does not certify the entire SQL language, physical row generation
by a downstream consumer, or protocol 3.0.0 release readiness. Unsupported or
unattested execution semantics produce explicit residual outcomes. No dialect
name alone establishes an enforcement, cast, text-comparison or time-zone law.

## Acceptance evidence

| AC | Constructive claim and explicit limit | Tests |
|---|---|---|
| 1 | Typed relation/column schemas preserve precision, scale, nullable and nested structure, collation and time-zone identities. Exact bounded scalar representations can be certified; lossy implicit casts, unspecified collation and vendor time-zone conversion remain residual. | `source_schema_contract.rs`, `physical_schema_edge_oracles.rs`, `physical_typed_populations.rs` |
| 2 | Canonical PK/UNIQUE/FK/composite/CHECK/accepted-values and value-production facts retain source identities, provenance and enforcement. Generated/identity facts do not imply executable allocation without attested state. | `relation_constraints.rs`, `dbt_constraints.rs`, `physical_write_states.rs` |
| 3 | Complete simultaneous source/target bags prove feasible, impossible or residual, including preexisting keys, computed columns, composite/cyclic FKs and bounded ordered DML. A typed INSERT may omit columns only when a canonical *authoritative exact scalar default* exists; nested expressions/unknown defaults remain residual. | `physical_write_states.rs`, `physical_typed_populations.rs`, `physical_schema_adapter_parity.rs` |
| 4 | Equivalent facts from SQL DDL, dbt manifest/catalog, ODCS and caller schemas share canonical meaning. Provenance and enforcement are retained independently; a dbt declaration or ODCS contract does not become an executed SQL default. Conflicts and missing types are explicit. | `physical_schema_adapter_parity.rs`, `dbt_constraint_schemas.rs`, `dbt_constraints.rs`, `odcs.rs` |
| 5 | Arrow-like binary/array/map/struct, exact decimal, finite IEEE754 bit words, JSON documents, calendars and clock times have differential/oracle examples. NaN, signed zero, fraction-to-binary rounding and offset conversion are residual without engine-specific attestations. | `physical_schema_edge_oracles.rs`, `physical_typed_serialization.rs`, `physical_typed_populations.rs` |
| 6 | Parser/analysis and independent DuckDB tests cover sound positive, impossible and residual outcomes with NULL, duplicates, cross-relations and dialect boundaries. Protocol schema, typed sidecar schema, code map and manifest document the same bounded public contract. | `coverage_manifest.rs`, `physical_write_states.rs`, `source_schema_contract.rs`, `physical_typed_serialization.rs` |

The evidence is executable regression coverage, not a claim that every possible
datatype value or native vendor SQL runtime has been tested.

## Intentionally unproved semantics

- Opaque UDFs, side effects, unseeded functions, coercions without a lossless
  type law and arbitrary CHECK subqueries or computed expressions
- Session/environment-dependent collation, time-zone conversion, default
  evaluation, transaction/deferral and identity allocation without explicit state
- General nested-valued SQL UPDATE/MERGE expressions and native Arrow IPC writing
  (the typed sidecar is deterministic JSON, not an Arrow IPC stream)
- Complete upstream-to-downstream data generation and per-dialect engine
  execution beyond the independent DuckDB oracles

For each unproved case the protocol must retain unknown/unsupported/residual
evidence, not silently omit an obligation or fabricate a value. New verified
subsets can be added later without changing this rule.

## Separate release gates

`docs/coverage-manifest.json` retains unverified **whole-workload physical
source generation** and per-variant dialect execution cells. These belong to
the broader TASK-66/87/88/89/91 and sql-tdg conformance/signoff work.
TASK-79's local source-independent schema witness contract can be implemented
and validated without advancing those unrelated release cells to green.
