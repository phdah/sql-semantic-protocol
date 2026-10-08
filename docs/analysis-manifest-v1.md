# Analysis manifest v1

The analysis manifest is a declarative input contract for larger SQL Semantic Protocol bundles. It configures analysis only; the emitted semantic protocol remains the active protocol contract documented in [the active protocol contract](protocol.md).

The manifest is JSON and is validated against `schema/analysis-manifest-v1.schema.json`. The active manifest version is `1`.

## Shape

```json
{
  "manifest_version": "1",
  "dialect": "generic",
  "catalog_relations": [
    "warehouse.raw.orders",
    "warehouse.stage.orders",
    "warehouse.mart.orders"
  ],
  "relation_context": {
    "default_catalog": "warehouse",
    "default_schema": "stage"
  },
  "output_scope": "targets",
  "targets": ["warehouse.mart.orders"],
  "inputs": [
    {
      "id": "stage-orders",
      "file": "sql/stage_orders.sql",
      "dialect": "snowflake"
    },
    {
      "id": "mart-orders",
      "sql": "CREATE TABLE mart.orders AS SELECT * FROM stage.orders",
      "dialect": "postgresql",
      "relation_context": {
        "default_catalog": "warehouse",
        "default_schema": "mart"
      }
    }
  ]
}
```

Each input requires a stable, unique `id` and exactly one of `sql` or `file`. Input order is preserved and drives deterministic layer ordering.

`dialect` at the root defaults to `generic`. An input-level `dialect` overrides that default for only that input. Dialect names are resolved through `sqlparser::dialect::dialect_from_str`; the project does not maintain a separate dialect whitelist.

Relative `file` paths are resolved relative to the manifest file's directory. The original manifest path string is retained as the input source identity, so moving an equivalent manifest tree does not change semantic output merely because its absolute filesystem location changed.

## Catalog-aware relation resolution

`catalog_relations` is an optional deterministic list of canonical relation identities. `relation_context` can provide `default_catalog`, `default_schema`, or both for the whole manifest. An input may declare its own `relation_context`; when present, it replaces the bundle-level context for that input.

The CLI constructs the same `RelationCatalog` and `RelationContext` values used by the public Rust API. Catalog metadata therefore affects canonical produced/consumed relation identities, graph linking, transitive lineage, and composed output domains identically in both paths. Ambiguous catalog matches and invalid metadata fail explicitly.

Catalog metadata is optional. Omitting both `catalog_relations` and `relation_context` preserves the existing textual relation identities.

## Output scope

`output_scope` is a presentation projection applied after the complete bundle has been parsed, linked, and composed.

- `all` is the default. It emits the complete analyzed bundle and requires `targets` to be empty.
- `targets` requires one or more exact relation identifiers in `targets`. It applies the same target projection as the public `select_targets` API, keeping each selected producer and every required in-bundle ancestor.

This does not reintroduce a reduced analysis mode. All inputs are analyzed before projection, consistent with the protocol's complete-bundle semantics.

## Validation

The manifest parser rejects malformed JSON, unknown fields, unsupported manifest versions, empty values, duplicate input IDs, duplicate targets, inputs that specify both or neither of `sql` and `file`, and invalid output-scope/target combinations.

File I/O remains outside the library manifest parser. The CLI reads the manifest and referenced files, resolves dialect implementations, then calls the configured multi-input analysis API.

## CLI

```sh
cargo run -- --manifest analysis.json
```

`--manifest` is mutually exclusive with direct analysis options such as `--dialect`, `--catalog-relation`, `--default-catalog`, `--default-schema`, `--target`, `--sql`, `--file`, `--dir`, and positional SQL. Output-format options remain CLI concerns and can still be combined with a manifest.

The equivalent direct-input metadata options are repeatable `--catalog-relation <relation>`, plus optional `--default-catalog <identifier>` and `--default-schema <identifier>`.
