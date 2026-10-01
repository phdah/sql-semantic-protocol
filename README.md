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

Protocol version `0.1.0` is defined by [`schema/protocol-v0.schema.json`](schema/protocol-v0.schema.json). See [`docs/protocol-v0.md`](docs/protocol-v0.md) for semantic and deterministic-ordering rules and [`examples/protocol-v0.json`](examples/protocol-v0.json) for a representative document.

## CLI

The CLI analyzes SQL and writes the SQL Semantic Protocol JSON document to standard output.

```text
sql-semantic-protocol [--dialect <name>] [--file <path>] [SQL ...]
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

SQL can be supplied directly:

```sh
cargo run -- --dialect postgresql "SELECT a FROM t WHERE a > 10"
```

from a file:

```sh
cargo run -- --dialect snowflake --file query.sql
```

or through standard input:

```sh
printf '%s\n' 'SELECT a FROM t WHERE a > 10' | cargo run -- --dialect duckdb
```

If no SQL argument and no `--file` are supplied, the CLI reads SQL from standard input. Use `--` before positional SQL if the SQL text starts with a dash.

Successful runs emit protocol JSON only. Input errors, SQL parse errors, and analysis failures are written to standard error and use distinct non-zero exit codes.

