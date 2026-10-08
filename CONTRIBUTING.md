# Contributing

Thanks for contributing to SQL Semantic Protocol. Please open an issue for a proposed feature or behavior change before starting a substantial implementation, especially when it changes the public protocol contract.

## Development

The project is a Rust library with a thin CLI. Follow [AGENTS.md](AGENTS.md) for the complete implementation standards and [docs/README.md](docs/README.md) for the documentation map.

```sh
cargo build
make fmt
make lint
make test
make doc
```

`make fmt` checks formatting. CI runs Rust checks independently plus the dbt end-to-end fixture. For the dbt integration test, run `make dbt-e2e`.

## Pull requests

1. Explain the observed or desired behavior and why it matters to a protocol consumer.
2. Keep parsing, semantic analysis, and protocol emission separate. Reuse canonical types instead of introducing adapter-specific protocol concepts.
3. Include tests for successful and unsupported paths. For semantic changes, assert resulting value domains and exactness, not only AST normalization or lineage.
4. Update the current [protocol documentation](docs/protocol.md), examples, or schema when the externally visible contract changes. Keep the root README approachable and put detailed reference material in `docs/`.
5. Use [Conventional Commits](https://www.conventionalcommits.org/). Breaking public API or protocol changes require a major version, including a `BREAKING CHANGE:` footer or `!` in the commit type.

The protocol must never silently overstate confidence. Unknown, unsupported, and conditional outcomes are intentional parts of its contract. See [versioning](docs/releasing.md) for the release process.

## Bug reports

Include the SQL input, dialect, known relation schemas or adapter inputs, expected outcome domain, actual protocol output (or explicit error), and application version. Remove secrets or customer data before attaching inputs.

## License

Contributions are accepted under the repository's [MIT license](LICENSE).
