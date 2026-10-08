# SQL Semantic Protocol

[![CI](https://github.com/phdah/sql-semantic-protocol/actions/workflows/ci.yml/badge.svg)](https://github.com/phdah/sql-semantic-protocol/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/sql-semantic-protocol.svg)](https://crates.io/crates/sql-semantic-protocol)
[![docs.rs](https://docs.rs/sql-semantic-protocol/badge.svg)](https://docs.rs/sql-semantic-protocol)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Turn SQL into a deterministic, parser-independent description of its outcomes.**

SQL Semantic Protocol analyzes SQL and optional external schema metadata into JSON describing which relations and columns an output depends on, what values it can contain, and which claims cannot be proven. It supports multiple SQL dialects and multi-step transformations. Consumers can reason about semantics without parsing SQL themselves.

```text
SQL + schema evidence → semantic analysis → protocol JSON → downstream consumers
```

The first downstream consumer is [sql-tdg](https://github.com/phdah/sql-tdg), which uses value-domain constraints to generate SQL test data.

## Highlights

- **Outcome-focused:** source predicates, allowed and excluded value ranges, output column domains, and physical lineage.
- **Compositional:** resolve dependencies across multiple SQL statements, CTEs, derived tables, and named transformation layers.
- **Honest about uncertainty:** unsupported constructs, missing schema evidence, and conditional comparison assumptions are surfaced rather than guessed.
- **Extensible evidence:** analyze SQL directly, dbt compiled artifacts, or Open Data Contract Standard (ODCS) v3.2 metadata.
- **Interoperable:** deterministic JSON contract with an optional OpenLineage export.
- **Library and CLI:** use the Rust API in a consumer or call the standalone binary.

## Quick start

Install the current published CLI with Cargo:

```sh
cargo install sql-semantic-protocol
```

Analyze a SQL query:

```sh
sql-semantic-protocol --dialect postgresql \
  'SELECT customer_id FROM orders WHERE amount >= 100'
```

The JSON result has `inputs` (source statements), `layers` (local and composed outcomes), and `graph` (dependencies and terminal outcomes). In this example, the analyzer records the lower bound on `orders.amount` and the projected `customer_id` lineage. It does not mistake the filtered input domain for the output column's own value domain.

Analyze multiple files, or target only one output after the full dependency graph is built:

```sh
sql-semantic-protocol --dialect snowflake \
  --file sql/stage.sql --file sql/mart.sql \
  --target mart.orders
```

For dbt, first compile the project and use its generated artifacts:

```sh
dbt compile
sql-semantic-protocol --dbt-manifest target/manifest.json
```

The adapter uses compiled model SQL. It reads `catalog.json` alongside the manifest when available, or requires complete manifest-declared source datatypes as fallback. See the [adapter guide](docs/adapters.md) for details.

### Rust library

```sh
cargo add sql-semantic-protocol
```

The library exposes analysis APIs separately from the CLI. See the [Rust API documentation](https://docs.rs/sql-semantic-protocol) and [semantic model guide](docs/semantics.md) for the domain types and composition behavior.

## Documentation

| Guide | What you'll find |
| --- | --- |
| [CLI and workflows](docs/cli.md) | Flags, multiple inputs, manifests, dialects, and complete output example |
| [Semantics](docs/semantics.md) | Constraints, value domains, composition, exactness, and supported SQL constructs |
| [Adapters](docs/adapters.md) | dbt, ODCS metadata, and OpenLineage export |
| [Protocol contract](docs/protocol.md) | Detailed current representation and guarantees |
| [JSON schema](schema/protocol.schema.json) | Machine-readable active protocol contract |
| [Examples](examples/protocol-simple.json) | An emitted protocol document |
| [Versioning](docs/releasing.md) | SemVer policy, Release Please, and upcoming breaking release |
| [Documentation index](docs/README.md) | References and historical contracts |

The application and emitted `protocol_version` use one shared version. **Version 2.0.0 is proposed**, not yet published, in [release PR #41](https://github.com/phdah/sql-semantic-protocol/pull/41) because the next protocol change is breaking. Check [releases](https://github.com/phdah/sql-semantic-protocol/releases) for published versions.

## Contributing

Issues and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for development commands, tests, documentation conventions, and contribution expectations. Review [AGENTS.md](AGENTS.md) for repository-specific implementation rules.

## License

Licensed under the [MIT License](LICENSE).
