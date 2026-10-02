# SQL Semantic Protocol

SQL Semantic Protocol is a dialect-independent semantic representation of SQL queries.

Its purpose is to translate SQL syntax into a stable, deterministic, machine-readable description of what a query means, rather than how the query was written.

The project includes a SQL parser and semantic analyzer that accepts SQL from supported dialects, analyzes the parsed query, and emits the SQL Semantic Protocol. The parser is a producer of the protocol. Consumers should depend on the protocol rather than on the parser's AST or the syntax of the original SQL.

The protocol describes semantics such as:

- the relations and columns a query depends on
- the columns produced by the query and their lineage
- the constraints placed on values by predicates
- the allowed value domains of columns, including bounded, unbounded, excluded, or disjoint ranges
- relationships between columns and relations introduced by joins and predicates
- other query semantics required to understand the resulting rows and output schema
- semantics that could not be resolved, represented explicitly as unknown or unsupported

The protocol deliberately separates SQL parsing from applications that need to reason about SQL. A consumer should not need to understand a Snowflake `WHERE` clause, a PostgreSQL AST, or a particular SQL parser. It should instead operate on the normalized semantic representation.

The first consumer is `sql-tdg`, the SQL Test Data Generator. It will use the protocol's value-domain and constraint information to determine which input values can satisfy a query and generate appropriate test data. This replaces the SQL parsing and interval derivation currently implemented inside `sql-tdg`.

The protocol is intentionally broader than test-data generation. Future consumers can use the same semantic representation for query analysis, lineage, validation, rewriting, and compilation. One planned use case is a SQL compiler that consumes the protocol and produces equivalent SQL according to a target dialect, formatting rules, or other output restrictions.

The protocol is therefore the contract between SQL and applications that need to reason about SQL semantics:

`SQL -> parser/analyzer -> SQL Semantic Protocol -> consumers`

## Protocol contract

Protocol version `0.2.0` is the single active contract emitted by the library and CLI. It represents one or many SQL inputs with the same root document shape: `inputs`, `layers`, and `graph`. A single SQL string is therefore represented as one element in `inputs`, not by switching to a different protocol version.

The active contract is defined by [`schema/protocol-v0.2.schema.json`](schema/protocol-v0.2.schema.json), documented in [`docs/protocol-v0.2.md`](docs/protocol-v0.2.md), and demonstrated by [`examples/protocol-v0.2.json`](examples/protocol-v0.2.json) and [`examples/protocol-v0.2-simple.json`](examples/protocol-v0.2-simple.json).

Version `0.1.0` files remain in the repository only as historical references. Current runtime code does not emit `0.1.0`.

TASK-13 resolves transformation layers into a deterministic relation dependency graph. TASK-14 composes semantics through that graph: final outputs expose transitive physical lineage, value domains propagate through safe direct projections and renames, and ambiguous, cyclic, or non-invertible paths remain explicit instead of being guessed. Disconnected pipelines compose independently.

### Outcome selection

The protocol always contains every analyzed transformation outcome. Each entry in `layers` carries its own composed semantics, while `graph.components[].final_outcomes` identifies the terminal datasets for each independent graph component.

Protocol generation does not have a final-only or all-layer mode. Choosing whether to consume every layer, only terminal outcomes, or a particular named outcome is a consumer concern. This keeps one complete protocol document as the source of truth and lets downstream applications, including test-data generators, choose the outcomes they need without re-analysis.

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

## Versioning

SQL Semantic Protocol uses one version for the application and the protocol. The Cargo package version, emitted `protocol_version`, active protocol contract, Git tag, and GitHub release are the same release identity.

SemVer compatibility is defined primarily by the public protocol contract. A breaking protocol change requires a major version bump. Backward-compatible protocol or application features use a minor bump, while compatible fixes and internal application changes use a patch bump. Non-protocol implementation changes therefore do not require a breaking release, but every release still advances the shared application/protocol version.

The current development version is `0.2.0`. The current roadmap targets the first stable `1.0.0` release, which will bootstrap Release Please for subsequent automated release PRs and GitHub releases.

## CLI

The CLI analyzes SQL and writes the SQL Semantic Protocol JSON document to standard output.

```text
sql-semantic-protocol [--dialect <name>] [--sql <SQL>]... [--file <path>]... [--dir <path>]... [SQL ...]
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

For multiple inputs, repeat `--sql`, `--file`, and `--dir` in any mixture. `--dir` recursively discovers regular files whose extension is `.sql` case-insensitively and ignores all other files:

```sh
cargo run -- \
  --sql "SELECT id FROM raw.orders" \
  --file sql/enrich_orders.sql \
  --dir sql/reporting \
  --sql "SELECT customer_id FROM raw.customers"
```

Explicit inputs are analyzed in command-line occurrence order and receive deterministic IDs `input-0001`, `input-0002`, and so on. Each `--dir` expands at its command-line position into all recursively discovered SQL files sorted lexicographically by path, so filesystem traversal order cannot affect protocol output. Discovered file paths are retained as source identity. The ID width expands when necessary, so there is no fixed input-count limit. Parse, file, and analysis failures identify the affected input or path.

Positional SQL represents one legacy input and cannot be mixed with `--sql`, `--file`, or `--dir`. If neither explicit input nor positional SQL is supplied, the CLI reads one input from standard input:

```sh
printf '%s\n' 'SELECT a FROM t WHERE a > 10' | cargo run -- --dialect duckdb
```

Every successful invocation emits protocol `0.2.0`. One input produces an `inputs` array with one element; multiple inputs use the same document shape with additional elements.

Query-backed DDL is analyzed through its defining query and records the created relation as the layer output. For example, `CREATE TABLE mart.orders AS SELECT ...` produces `mart.orders`, while a bare `SELECT` produces an anonymous layer result.

### Example output

For:

```sql
SELECT t.b FROM t WHERE t.a > 10
```

the protocol still uses the active `0.2.0` envelope even though there is only one input:

```json
{
  "protocol_version": "0.2.0",
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

This is the fixture stored in `examples/protocol-v0.2-simple.json`.

Successful runs emit protocol JSON only. Input errors, SQL parse errors, and analysis failures are written to standard error and use distinct non-zero exit codes.

