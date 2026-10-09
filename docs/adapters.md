# Input adapters and interoperability

SQL, dbt artifacts, and ODCS metadata feed one canonical protocol. Adapters provide evidence; they do not define different output contracts. For an introduction and commands, see the [README](../README.md) and [CLI guide](cli.md).

## ODCS v3.2 metadata

The ODCS YAML adapter is enabled by the default Cargo feature `odcs`.
The dependency examples below target the upcoming 2.0.0 release; use `cargo add sql-semantic-protocol` to select the latest available published version.
It is the only part of the library that needs the optional `saphyr` dependency.
Consumers that do not read ODCS contracts can exclude the adapter and its YAML
dependencies:

```toml
sql-semantic-protocol = { version = "2", default-features = false }
```

To enable ODCS explicitly when default features are disabled:

```toml
sql-semantic-protocol = { version = "2", default-features = false, features = ["odcs"] }
```

The ODCS public API (`parse_odcs_yaml`, `parse_odcs_documents`, and
`OdcsDocument` and related types) is only available when `odcs` is enabled.
All other analysis, dbt, and protocol APIs work without it.


Open Data Contract Standard v3.2 YAML can enrich the same canonical schema and constraint model
used by SQL and dbt. Use `parse_odcs_yaml` for one self-contained contract or
`parse_odcs_documents` with explicitly named `OdcsDocument` values when relationships cross
contract boundaries. The adapter never fetches external contracts.

ODCS schema `physicalName` is used when present, otherwise `name`, and is resolved through the
existing `RelationCatalog` rules. Property `physicalType` is the preferred datatype evidence;
`logicalType` is lower-authority fallback evidence. Bundle enrichment preserves the overall
datatype authority order: dbt catalog, dbt manifest, ODCS physical type, then ODCS logical type.
A contradictory lower-authority ODCS datatype is returned explicitly rather than silently
discarded.

ODCS `required`, `primaryKey`/`primaryKeyPosition`, property `unique`, relationships, and
`enum` declarations normalize into the existing not-null, primary-key, unique-key, foreign-key,
and accepted-values constraints. Evidence uses `external_metadata` provenance with unknown
enforcement. Same-contract and explicitly supplied cross-contract foreign-key references resolve
deterministically; missing, ambiguous, or unsupported references fail rather than being guessed.

## dbt artifact adapter

dbt is a first-class protocol input. The complete adapter consumes `manifest.json` and
optionally `catalog.json`, using each artifact only for the evidence it authoritatively owns:

- `manifest.json`: model unique IDs, canonical relation identities, compiled SQL, relation context,
  declared dependency metadata, explicit model/column key constraints, and built-in `unique` /
  `relationships` generic-test declarations.
- `catalog.json`: warehouse-introspected physical columns and database datatypes for models,
  seeds, snapshots, and sources.

dbt leaves `attached_node` empty for tests declared on sources. The adapter then identifies the
tested resource from the test's rendered `kwargs.model` argument (for example
`{{ get_where_subquery(source('raw', 'orders')) }}`), which must resolve to exactly one declared
dependency; this also covers self-referencing `relationships` tests. Without a usable `model`
argument, a single-dependency test uses that dependency, and a `relationships` test uses its
declared `to` reference to separate the parent from exactly one remaining child dependency. A
built-in test whose tested resource still cannot be identified is reported as
`unattributed_dbt_test` in the bundle-level `constraint_diagnostics` and contributes no
constraint, rather than aborting analysis or assigning the constraint to a guessed relation.

The adapter joins the artifacts by dbt resource `unique_id`, normalizes catalog datatypes into the
same parser-independent `DataType` model used by direct callers, and runs compiled model SQL
through the ordinary analyzer, graph builder, composition, and outcome-domain pipeline. dbt-specific
artifact types never appear in the emitted protocol.

The CLI looks for `catalog.json` next to `manifest.json` by default. When it is absent,
complete manifest-declared column `data_type` values for each physical source suffice,
including sources declared in dbt YAML without a warehouse. An explicitly supplied
`--dbt-catalog` path must exist:

```sh
cargo run -- --dbt-manifest target/manifest.json
```

Use `--dbt-catalog` when the catalog artifact is stored elsewhere:

```sh
cargo run -- \
  --dbt-manifest artifacts/manifest.json \
  --dbt-catalog warehouse/catalog.json
```

Target projection remains a consumer-side operation and can be applied after complete dbt analysis:

```sh
cargo run -- \
  --dbt-manifest target/manifest.json \
  --target warehouse.analytics.customer_summary
```

The library API keeps artifact loading and dialect selection explicit:

```rust
use sql_semantic_protocol::{
    analyze_dbt_artifacts, parse_dbt_catalog, parse_dbt_manifest, to_bundle_json,
};
use sqlparser::dialect::dialect_from_str;

let manifest_json = std::fs::read_to_string("target/manifest.json")?;
let catalog_json = std::fs::read_to_string("target/catalog.json")?;
let manifest = parse_dbt_manifest(&manifest_json)?;
let catalog = parse_dbt_catalog(&catalog_json)?;
let dialect = dialect_from_str(manifest.adapter_type())
    .ok_or("dbt adapter type is not a sqlparser dialect")?;
let bundle = analyze_dbt_artifacts(
    &manifest,
    &catalog,
    manifest.adapter_type(),
    dialect.as_ref(),
)?;

// For catalog-less projects, use the same typed analysis without a placeholder catalog:
let bundle = sql_semantic_protocol::analyze_dbt_manifest_with_schemas(
    &manifest,
    manifest.adapter_type(),
    dialect.as_ref(),
)?;

println!("{}", to_bundle_json(&bundle));
```

The manifest adapter accepts schema versions v10, v11, and v12. The catalog adapter accepts v0 and
v1. Catalog columns are ordered by their warehouse ordinal and their dialect-specific type strings
are normalized through the selected dbt adapter dialect. Catalog schema evidence takes precedence
when present. If a physical dependency or physical relation named only by a canonical
constraint is absent from `catalog.json`, the adapter falls back to column `data_type`
declarations in `manifest.json` when the declared schema is complete. This includes both sides
of a foreign key defined by a built-in `relationships` test or a declared constraint, even if
neither source appears in compiled model SQL. Missing declared datatypes are reported with the
relation and affected columns. A constraint-required physical column missing from the chosen
schema also fails with its relation and column names, rather than silently losing the foreign key.
Produced dbt models remain transformation outcomes, not newly synthesized physical sources. Catalog-reported metadata
query errors, catalog resources absent from the paired manifest, missing relation identities,
invalid datatypes, or dependencies with neither usable catalog nor manifest schema evidence fail
explicitly rather than producing an apparently complete protocol.

`analyze_dbt_manifest` remains available as a compatibility API for manifest-only semantic
analysis, but it cannot emit complete typed relation schemas. For typed analysis without a
catalog, use `analyze_dbt_manifest_with_schemas`; this produces the same output and
missing-schema errors as `analyze_dbt_artifacts` with an empty catalog. With a catalog,
use `analyze_dbt_artifacts`. Both typed paths require complete schema evidence for all
physical dependencies and preserve `source_kind: dbt_manifest` on manifest-only sources.

Model identity comes from dbt `unique_id`, and produced dataset identity comes from
`relation_name`; filenames are retained only as source metadata. Model analysis uses compiled SQL from `compiled_code`; uncompiled Jinja is not valid SQL input. Python models,
relation-less models such as ephemerals, missing dependency relations, dependency cycles, and
uncompiled Jinja fail explicitly rather than being guessed or silently omitted.

dbt `depends_on.nodes` provides deterministic model ordering and is mapped to canonical relation
identities from the manifest. The adapter verifies that every declared dependency is also present
in the dependency graph derived from analyzed SQL. Source schemas use catalog metadata when
available and manifest-declared column types only as fallback evidence for missing physical
relations. Both use the same canonical relation identities, so relation resolution, lineage, value
domains, and typed source schemas describe one consistent graph.

CI exercises a complete dbt Core project rather than relying only on hand-authored fixtures.
`make dbt-e2e` runs seed and model execution, generates `catalog.json` from the real warehouse,
then analyzes the generated manifest/catalog pair through both the library and CLI. The fixture
covers predicate and output domains, CASE and arithmetic expressions, joins, window functions,
QUALIFY, named windows, UNION/INTERSECT/EXCEPT, aggregation, HAVING, DISTINCT, ROLLUP, subqueries,
derived/lateral tables, transitive composition, disconnected components, incremental model
configuration, terminal outcome snapshots, and warehouse source-schema datatype emission.

## OpenLineage export

The SQL Semantic Protocol remains the authoritative semantic representation. The library function `to_openlineage_json` maps resolved named layers to OpenLineage 2.0.2 DatasetEvents using the current Lineage Dataset Facet for dataset-level and field-level lineage. OpenLineage types do not appear in the core protocol model.

The caller supplies one OpenLineage namespace and event timestamp:

```rust
let json = sql_semantic_protocol::to_openlineage_json(
    &bundle,
    "postgresql://warehouse",
    "2026-10-02T07:00:00Z",
)?;
```

Only semantics OpenLineage can represent are exported. Predicate trees, value domains, and other richer outcome semantics remain in the SQL Semantic Protocol. Anonymous outputs and unresolved layers are omitted rather than assigned invented dataset identity or lineage.

The CLI can select the same adapter with `--format openlineage`. A namespace is required because SQL alone cannot determine an OpenLineage dataset namespace. The event time may be supplied explicitly for reproducible output; otherwise the CLI uses the current UTC time:

```sh
cargo run -- \
  --dialect postgresql \
  --format openlineage \
  --namespace postgresql://warehouse \
  --file sql/stage.sql \
  --file sql/core.sql \
  --file sql/mart.sql
```

For byte-reproducible output, add `--event-time 2026-10-02T07:00:00Z`.


## Group witness adapter parity

Group witness semantics come from normalized SQL after the common parser boundary. Raw SQL, composed SQL and dbt compiled model SQL use the same canonical `group_witness` contract. Resolved layer composition retains per-origin `group_witnesses` and `boundary_kind` (physical, intermediate, or unresolved), including upstream dbt model layers, without claiming that an arbitrary downstream layer preserves the same group or that intermediate relations can be freely generated. The dbt manifest provides compiled SQL and graph identity, while catalog metadata can provide types and schema constraints; the adapter does not independently derive grouped witness semantics.

ODCS v3.2 supplies metadata and schema evidence, not executable HAVING SQL, so it cannot itself create group witness obligations. Constraints from any evidence source remain mandatory when checking whether an exact witness **can actually be materialized**. For downstream generation, `sql-tdg TASK-28` consumes source-group obligations and must not infer aggregate proofs by reparsing SQL.

## EXISTS and IN membership witness adapter parity

Direct SQL and dbt compiled-model SQL share the same canonical nested-query
analyzer. Both expose the optional typed `subquery_witnesses` contract for
provable EXISTS, NOT EXISTS, IN and NOT IN and default-deny residual directions
for unsupported relation shapes, unproved correlations or comparison semantics.
The dbt Core end-to-end outcome snapshot asserts the composed witnesses for
compiled EXISTS, IN and NOT IN queries, including the physical/intermediate
origin boundary. ODCS enriches relation metadata but does not synthesize SQL
predicates or invent membership witnesses. No adapter requires SQL text
reparsing by downstream generators.

### Coupled boolean witnesses and adapter evidence

The SQL and dbt compiled-SQL paths share one typed `boolean_witness`
analysis. Exact signed-integer comparison branches require source datatypes
from authoritative relation schema/catalog evidence. A plain SQL invocation
without a catalog cannot prove those branches exact, and dbt adapters must
not infer warehouse datatypes from compiled SQL or unconstrained manifest
text. Null-test-only boolean trees need no datatype evidence. Unsupported
casts, unattested collation-dependent LIKE and noninvertible computed
functions stay residual. Overflow-free identity arithmetic and signed
constant offsets following lossless widening CASTs can normalize to their
original source columns if the full typed source range proves no overflow.
Physical remapping additionally requires row-preserving intermediate producers;
column-only identity is insufficient for exact source witness obligations.
