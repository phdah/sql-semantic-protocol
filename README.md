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
