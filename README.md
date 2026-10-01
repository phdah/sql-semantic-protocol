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

Protocol version `0.1.0` is the current single-input contract emitted by the library and CLI. It is defined by [`schema/protocol-v0.schema.json`](schema/protocol-v0.schema.json), documented in [`docs/protocol-v0.md`](docs/protocol-v0.md), and demonstrated by [`examples/protocol-v0.json`](examples/protocol-v0.json).

Protocol version `0.2.0` defines the next multi-input composition contract. It can represent arbitrarily many related or independent SQL inputs, transformation layers, dependency graph components, composed semantics, and per-component final outcomes. See [`schema/protocol-v0.2.schema.json`](schema/protocol-v0.2.schema.json), [`docs/protocol-v0.2.md`](docs/protocol-v0.2.md), and [`examples/protocol-v0.2.json`](examples/protocol-v0.2.json).

Multi-input analysis now emits the `0.2.0` envelope with ordered analyzed inputs. Transformation layers, dependency edges, graph components, and transitive composition are not populated yet; the emitted graph carries an explicit `multi_input_composition_pending` diagnostic until those later tasks are implemented. Existing single-input analysis continues to emit `0.1.0`.

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

Single-input invocations emit protocol `0.1.0`. Invocations with two or more explicit inputs emit one protocol `0.2.0` document containing all analyzed inputs in deterministic order.

### Example output

For:

```sql
SELECT t.b FROM t WHERE t.a > 10
```

the protocol output is:

```json
{
  "protocol_version": "0.1.0",
  "source": {
    "dialect": "generic"
  },
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
```

This is the same schema-valid fixture stored in `examples/protocol-v0-simple.json`.

Successful runs emit protocol JSON only. Input errors, SQL parse errors, and analysis failures are written to standard error and use distinct non-zero exit codes.

