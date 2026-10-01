# sql-semantic-protocol Agent Instructions

See [README.md](README.md) for the project overview: take a SQL string in any [sqlparser](https://docs.rs/sqlparser/latest/sqlparser/) dialect and
emit an opinionated, human/AI readable protocol describing the query's output (column
intervals, surviving columns, upstream dependencies). This file holds principles only.

## Guiding Principles

**Never over-claim** The protocol is only useful if it can be trusted. When the analysis
cannot determine something (an unsupported statement, an opaque function, a construct not
yet handled), represent it explicitly as unknown or unsupported in the output, or return
an error. Never silently drop it, and never guess. A conservative answer (e.g. "interval
unbounded") is always preferable to a wrong precise one.

**Separation of concerns** Keep three layers apart: parsing (sqlparser produces the AST),
analysis (walks the AST and builds the semantic model), and emission (serializes the model
into the protocol format). The CLI in `main.rs` only reads input, calls the library, and
writes output. It holds no analysis logic.

**Library first, thin binary** Core logic lives in a library crate (`src/lib.rs`) with a
small public API, e.g. a function taking SQL text plus a dialect and returning
`Result<Protocol, Error>`. The binary is a thin wrapper around it. This keeps the analysis
reusable and testable without spawning a process.

**Contain the sqlparser dependency** sqlparser AST types are an input format, not part of
our domain. They may appear in the analysis layer, but never in the public protocol types.
Conversion from AST to domain model happens at one explicit boundary, so a sqlparser
upgrade only touches that boundary.

**Dialect is injected, never hardcoded** The dialect is chosen by the caller and passed
in. Analysis code must not assume a specific dialect. Dialect-specific behavior, when
unavoidable, is isolated and named for the dialect it belongs to.

**Strict types** Model the domain with types, not strings. Use enums for any fixed set of
values (comparison operators, clause kinds, bound inclusivity), newtypes for identifiers
that must not be mixed up (e.g. `ColumnName` vs `TableName`), and structs with named
fields over tuples. Prefer exhaustive `match`; avoid `_ =>` catch-alls on domain enums so
that adding a variant forces every call site to be revisited. When matching on large
sqlparser enums, a catch-all is acceptable only if it maps to an explicit
unsupported/unknown result, never to a silent no-op.

**Make invalid states unrepresentable** Parse, don't validate. Construct domain values
through constructors that enforce their invariants (e.g. an interval whose lower bound
does not exceed its upper bound), and keep fields private when an invariant must hold.
Once a value exists, it is valid.

**Immutability and ownership** Bindings are immutable by default; reach for `mut` only
when needed and keep its scope small. Borrow (`&str`, `&[T]`, `&T`) in function arguments;
return owned values. Clone deliberately, not to appease the borrow checker; if cloning
feels forced, revisit the design.

**Error handling** Library code returns `Result` with a domain-specific error type whose
variants name the failure (e.g. `ParseError`, `UnsupportedStatement`), and implements
`std::error::Error` and `Display`. No `unwrap()`, `expect()`, `panic!`, or indexing that
can panic in library code, except for true internal invariants, which use
`expect("reason the invariant holds")`. Use `?` for propagation. `Box<dyn Error>` is fine
in `main.rs` only.

**Deterministic output** The same input must always produce byte-identical output. Use
`BTreeMap`/`BTreeSet` (or explicitly sorted `Vec`s) for anything that is serialized; never
let `HashMap` iteration order leak into the protocol.

**The protocol format is a public contract** Changes to the emitted format are breaking
changes for consumers. Make them deliberately, cover them with tests that assert the full
output for representative queries, and update the README when the documented shape
changes.

**One application/protocol version** The application/crate version and emitted protocol version are one shared version identity. Every supported invocation and public emission path emits that version and the same root document shape. `PROTOCOL_VERSION` must derive from the Cargo package version rather than being independently hard-coded. Historical schemas and documentation may remain in the repository, but current runtime code must not emit historical versions. When a new version becomes active, migrate all active protocol artifacts together.

**Small, pure functions** Analysis functions take inputs and return values, with no I/O,
global state, or environment reads. If something is hard to test, the design is wrong.

**Composition over inheritance** Use traits to define contracts, not to share
implementation. Prefer plain functions and structs; introduce a trait only when there are,
or will concretely be, multiple implementations. Avoid generics and trait objects that
exist only for hypothetical flexibility.

**Readability over cleverness** Favor clear, explicit code over dense iterator chains or
macro tricks. Name things for what they do. Write `///` doc comments on every public item
explaining what it represents and any invariants; use comments in code to explain *why*,
not *what*.

**Module navigation** `src/lib.rs` is the index. Its `//!` module docs list the public
entry points with a one-line description each, and each submodule's `//!` docs describe
its responsibility. When adding, renaming, or removing public items, keep these docs and
the `pub use` re-exports in sync.

**No `unsafe`**
This crate has no reason to use `unsafe`. Do not introduce it.

**Minimize dependency footprint** Add a crate only when the benefit clearly outweighs the
coupling, and confirm with the user before adding one. The standard library, sqlparser,
and serde should cover most needs. Enable only the crate features actually used.

**Tooling** Use `cargo` for everything. Read `Cargo.toml` before writing code; it is the
authoritative source for edition, dependencies, and profiles. Code is formatted with
`rustfmt` defaults and must pass `clippy` with no warnings.

Don't add `#[allow(...)]` attributes or lint ignores without confirmation from the user.
Fix the underlying warning, or consult the user for explicit guidance.

**Semantic versioning and releases**
The Cargo package version, emitted `protocol_version`, active protocol contract, Git tag, and GitHub release represent the same application version. A breaking change to the emitted protocol contract requires a major SemVer bump. Other externally breaking public API changes also follow normal SemVer and require a major bump. Backward-compatible protocol or application features use a minor bump; backward-compatible fixes and internal changes use a patch bump. An application-only change does not become breaking merely because it creates a release, but the emitted protocol version advances with the application release because they share one version identity. Release Please is the intended release mechanism after the 1.0.0 bootstrap and Conventional Commits are the source for release classification.

**Conventional commits**
Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/): `<type>(<optional scope>): <description>`, with the description in imperative mood and lowercase, e.g. `feat(analysis): derive intervals from BETWEEN`. Common types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `build`, `ci`, `perf`. Mark breaking changes (public API or protocol format) with `!` after the type/scope, e.g. `feat(protocol)!: rename interval bounds`, and explain them in a `BREAKING CHANGE:` footer.

## Verification

Run checks proportional to the change. Before considering a change done:

```console
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Use `cargo doc --no-deps` when public docs change, to catch broken intra-doc links.

## Test conventions

**Unit tests live next to the code** Put unit tests in a `#[cfg(test)] mod tests` block at
the bottom of the module they test. They may exercise private functions.

**Integration tests use the public API only** Put end-to-end tests in `tests/`. They call
the library's public entry point with SQL text and assert on the resulting protocol,
exactly as a consumer would.

**One behavior per test, SQL as the fixture** Each test states its input SQL inline (or
loads it from a fixture file for longer queries) and asserts one semantic property: an
interval, the set of surviving columns, or the set of dependencies. Name tests for the
behavior, e.g. `between_produces_closed_interval`.

**Cover the unsupported path** For every construct the analysis does not handle, add a
test asserting it is reported as unsupported/unknown rather than silently ignored.

**Shared helpers over repetition** When several tests build the same input or expected
values, extract a helper (in the test module, or `tests/common/mod.rs` for integration
tests) instead of copy-pasting setup.

## Task tracking

Todos, planned work, and decisions for this project are tracked in a local
[Backlog.md](https://github.com/MrLesk/Backlog.md) board stored in `.backlog/`. Use the
`backlog_*` MCP tools or the `backlog` CLI when available; otherwise edit the Markdown
files under `.backlog/` directly, following the format of existing files. The board is
versioned with the repo, so commit task changes alongside the work they describe.
