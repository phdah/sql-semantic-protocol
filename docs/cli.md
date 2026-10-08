# CLI and input workflows

Run `sql-semantic-protocol --help` for the currently supported flags. The CLI emits one protocol JSON document to stdout; failures are written to stderr.

## CLI

The CLI analyzes SQL and writes the SQL Semantic Protocol JSON document to standard output.

```sh
sql-semantic-protocol --help
```

The dialect defaults to `generic`. The CLI delegates dialect selection to `sqlparser::dialect::dialect_from_str`, so it accepts any built-in dialect recognized by the pinned `sqlparser` version rather than maintaining a separate dialect list.

With `sqlparser` 0.58, the following built-in dialects are available:

| Dialect | CLI name |
| --- | --- |
| ANSI | `ansi` |
| BigQuery | `bigquery` |
| ClickHouse | `clickhouse` |
| Databricks | `databricks` |
| DuckDB | `duckdb` |
| Generic | `generic` |
| Hive | `hive` |
| Microsoft SQL Server | `mssql` |
| MySQL | `mysql` |
| PostgreSQL | `postgresql`, `postgres` |
| Redshift | `redshift` |
| Snowflake | `snowflake` |
| SQLite | `sqlite` |

Legacy single-input SQL can still be supplied positionally:

```sh
cargo run -- --dialect postgresql "SELECT a FROM t WHERE a > 10"
```

A single file remains unchanged:

```sh
cargo run -- --dialect snowflake --file query.sql
```

For multiple inputs, repeat `--sql`, `--file`, and `--dir` in any mixture. `--dir` recursively discovers regular files whose extension is `.sql` case-insensitively and ignores all other files. Repeat `--catalog-relation` to supply canonical catalog metadata and use `--default-catalog`/`--default-schema` when direct inputs need qualification before graph linking.

Repeat `--target` to project the completed analysis onto one or more named output relations:

```sh
cargo run -- \
  --sql "SELECT id FROM raw.orders" \
  --file sql/enrich_orders.sql \
  --dir sql/reporting \
  --sql "SELECT customer_id FROM raw.customers"
```

For example, to expose only `mart.customer_summary` and its required in-bundle ancestors while still analyzing every supplied input:

```sh
cargo run -- \
  --target mart.customer_summary \
  --file sql/stage_orders.sql \
  --file sql/core_orders.sql \
  --file sql/customer_summary.sql \
  --file sql/unrelated_pipeline.sql
```

Direct CLI inputs are analyzed in command-line occurrence order and receive deterministic IDs `input-0001`, `input-0002`, and so on. Manifest inputs retain their explicit stable IDs. Each `--dir` expands at its command-line position into all recursively discovered SQL files sorted lexicographically by path, so filesystem traversal order cannot affect protocol output. Discovered file paths are retained as source identity. The ID width expands when necessary, so there is no fixed input-count limit. Parse, file, and analysis failures identify the affected input or path.

Positional SQL represents one legacy input and cannot be mixed with `--sql`, `--file`, or `--dir`. If neither explicit input nor positional SQL is supplied, the CLI reads one input from standard input:

```sh
printf '%s\n' 'SELECT a FROM t WHERE a > 10' | cargo run -- --dialect duckdb
```

Every successful invocation emits the current Cargo package version as `protocol_version`. One input produces an `inputs` array with one element; multiple inputs use the same document shape with additional elements.

Query-backed DDL is analyzed through its defining query and records the created relation as the layer output. For example, `CREATE TABLE mart.orders AS SELECT ...` produces `mart.orders`, while a bare `SELECT` produces an anonymous layer result.

### Example output

For:

```sql
SELECT t.b FROM t WHERE t.a > 10
```

the protocol still uses the active version envelope even though there is only one input:

```json
{
  "protocol_version": "<package-version>",
  "inputs": [
    {
      "id": "input-0001",
      "source": {
        "kind": "inline",
        "label": null
      },
      "dialect": "generic",
      "statements": [
        {
          "kind": "query",
          "sources": [
            {
              "kind": "relation",
              "name": "t",
              "alias": null
            }
          ],
          "dependencies": ["t"],
          "joins": [],
          "predicates": {
            "where": {
              "kind": "comparison",
              "left": {
                "kind": "column",
                "relation": "t",
                "name": "a"
              },
              "operator": "gt",
              "right": {
                "kind": "literal",
                "type": "integer",
                "value": 10
              }
            },
            "having": null,
            "qualify": null
          },
          "column_domains": [
            {
              "column": {
                "relation": "t",
                "name": "a"
              },
              "domain": {
                "kind": "ranges",
                "ranges": [
                  {
                    "lower": {
                      "value": {
                        "kind": "literal",
                        "type": "integer",
                        "value": 10
                      },
                      "inclusive": false
                    },
                    "upper": null
                  }
                ]
              }
            }
          ],
          "output": {
            "columns": [
              {
                "name": "b",
                "expression": {
                  "kind": "column",
                  "relation": "t",
                  "name": "b"
                },
                "lineage": [
                  {
                    "relation": "t",
                    "column": "b"
                  }
                ]
              }
            ]
          },
          "diagnostics": []
        }
      ]
    }
  ],
  "layers": [
    {
      "id": "layer-0001",
      "statement": {
        "input_id": "input-0001",
        "statement_index": 0
      },
      "produces": [
        {
          "kind": "anonymous",
          "layer_id": "layer-0001"
        }
      ],
      "consumes": ["t"],
      "composed_semantics": {
        "status": "resolved",
        "dependencies": ["t"],
        "column_domains": [
          {
            "column": {
              "relation": "t",
              "name": "a"
            },
            "domain": {
              "kind": "ranges",
              "ranges": [
                {
                  "lower": {
                    "value": {
                      "kind": "literal",
                      "type": "integer",
                      "value": 10
                    },
                    "inclusive": false
                  },
                  "upper": null
                }
              ]
            }
          }
        ],
        "output": {
          "columns": [
            {
              "name": "b",
              "expression": {
                "kind": "column",
                "relation": "t",
                "name": "b"
              },
              "domain": {
                "kind": "unbounded"
              },
              "lineage": [
                {
                  "relation": "t",
                  "column": "b"
                }
              ]
            }
          ]
        },
        "diagnostics": []
      }
    }
  ],
  "graph": {
    "edges": [
      {
        "consumer_layer_id": "layer-0001",
        "relation": "t",
        "resolution": "external",
        "producer_layer_ids": []
      }
    ],
    "components": [
      {
        "id": "component-0001",
        "layer_ids": [
          "layer-0001"
        ],
        "final_outcomes": [
          {
            "kind": "anonymous",
            "layer_id": "layer-0001"
          }
        ],
        "diagnostics": []
      }
    ],
    "diagnostics": []
  }
}
```

This shape is locked by the active fixture stored in `examples/protocol-simple.json`.

Successful runs emit protocol JSON only. Input errors, SQL parse errors, and analysis failures are written to standard error and use distinct non-zero exit codes.

## Analysis manifests

Large bundles can be declared in a versioned JSON manifest instead of repeating every analysis input and option on the command line. Manifest v1 is defined by [`schema/analysis-manifest-v1.schema.json`](../schema/analysis-manifest-v1.schema.json) and documented in [`docs/analysis-manifest-v1.md`](../docs/analysis-manifest-v1.md).

Each manifest input has a stable unique `id`, exactly one inline `sql` string or `file` path, and an optional per-input `dialect`. The root `dialect` defaults to `generic` and supplies the default for inputs that omit an override. Dialect names continue to resolve through sqlparser rather than a project-maintained list. Relative file paths resolve relative to the manifest file while the declared path is retained as source identity.

Manifests may also declare `catalog_relations` and a bundle-level `relation_context` with `default_catalog` and/or `default_schema`. An input can replace that context for its own analysis. The CLI maps this metadata to the same `RelationCatalog` and `RelationContext` API used by direct Rust callers, so canonical graph links, lineage, and composed output domains are equivalent.

The default `output_scope` is `all`, which preserves the complete analyzed bundle. `output_scope: "targets"` requires one or more exact relation names and applies target projection only after every input has been analyzed and composed.

```json
{
  "manifest_version": "1",
  "dialect": "generic",
  "catalog_relations": [
    "warehouse.stage.orders",
    "warehouse.mart.orders",
    "warehouse.raw.orders"
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

Run it with:

```sh
cargo run -- --manifest analysis.json
```

The checked-in `tests/fixtures/extended_bundle/analysis-all.json` fixture demonstrates the complete bundle workflow: multiple independent pipelines, PostgreSQL/Generic/Snowflake/MySQL inputs, CREATE TABLE/VIEW layers, INSERT-select semantics, catalog-aware canonical identities, and per-input relation context. Its companion `analysis-target.json` applies target projection only after the whole workload has been analyzed and composed.

```sh
cargo run -- --manifest tests/fixtures/extended_bundle/analysis-all.json
cargo run -- --manifest tests/fixtures/extended_bundle/analysis-target.json
```

The full fixture preserves every layer and identifies terminal outcomes in `graph.components[].final_outcomes`. The targeted fixture keeps only `warehouse.analytics.final_orders` and its required in-bundle ancestors while retaining the complete analyzed input evidence.

`--manifest` is mutually exclusive with `--dbt-manifest` and direct analysis options such as `--dialect`, `--catalog-relation`, `--default-catalog`, `--default-schema`, `--target`, `--sql`, `--file`, `--dir`, and positional SQL. `--dbt-manifest` is mutually exclusive with direct SQL/catalog options but may be combined with `--target`. Output-format options such as OpenLineage export remain CLI-level options and can be combined with either manifest form.
